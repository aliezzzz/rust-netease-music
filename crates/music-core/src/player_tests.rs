use super::*;

#[test]
fn http_cache_rejects_previews_and_stalled_source_can_stop() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    gstreamer::init().unwrap();
    std::env::set_var("NCM_AUDIO_SINK", "fakesink");
    let temp = tempfile::tempdir().unwrap();
    let samples = 8000u32 * 4;
    let mut wave = Vec::new();
    wave.extend_from_slice(b"RIFF");
    wave.extend_from_slice(&(36 + samples * 2).to_le_bytes());
    wave.extend_from_slice(b"WAVEfmt ");
    wave.extend_from_slice(&16u32.to_le_bytes());
    for value in [1u16, 1] {
        wave.extend_from_slice(&value.to_le_bytes());
    }
    wave.extend_from_slice(&8000u32.to_le_bytes());
    wave.extend_from_slice(&16000u32.to_le_bytes());
    for value in [2u16, 16] {
        wave.extend_from_slice(&value.to_le_bytes());
    }
    wave.extend_from_slice(b"data");
    wave.extend_from_slice(&(samples * 2).to_le_bytes());
    wave.resize(44 + samples as usize * 2, 0);
    for (index, expected, stalled) in [(0, 4000, false), (1, 180000, false), (2, 4000, true)] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let uri = format!("http://{}/tone.wav", listener.local_addr().unwrap());
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let (connected, connection) = std::sync::mpsc::sync_channel(1);
        let bytes = wave.clone();
        let server = std::thread::spawn(move || {
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(20) {
                if stopped.try_recv().is_ok() {
                    return;
                }
                if let Ok((mut stream, _)) = listener.accept() {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut request = [0; 4096];
                    if stream.read(&mut request).is_ok() {
                        let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nAccept-Ranges: none\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len());
                        if stalled {
                            let _ = stream.write_all(&bytes[..44]);
                        } else {
                            // A single localhost write can finish before the
                            // downloadbuffer has learned Content-Length, so GST
                            // may reach EOS without its cache-complete message.
                            // Exercise a real streaming transfer deterministically.
                            for chunk in bytes.chunks(4096) {
                                if stream.write_all(chunk).is_err() {
                                    break;
                                }
                                std::thread::sleep(Duration::from_millis(10));
                            }
                        }
                        let _ = connected.try_send(());
                        if stalled {
                            let _ = stopped.recv_timeout(Duration::from_secs(20));
                            return;
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let destination = temp.path().join(format!("cache-{index}"));
        let (tx, _rx) = mpsc::unbounded_channel();
        let engine =
            Engine::new(&uri, 0.5, index, tx, Some((destination.clone(), expected))).unwrap();
        engine.play.play();
        connection.recv_timeout(Duration::from_secs(10)).unwrap();
        if !stalled {
            let started = Instant::now();
            loop {
                let downloaded = engine.downloaded.lock().unwrap().is_some();
                let duration = engine.play.duration();
                if downloaded && duration.is_some() {
                    break;
                }
                assert!(
                    started.elapsed() < Duration::from_secs(10),
                    "HTTP download did not complete (cache event: {downloaded}, duration: {duration:?})"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        let started = Instant::now();
        drop(engine);
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "stopping stalled HTTP took too long"
        );
        if index == 0 {
            let cached = std::fs::read(&destination).unwrap();
            assert_eq!(cached.len(), wave.len(), "cached download was truncated");
            assert!(cached == wave, "cached download bytes differ");
        } else {
            assert!(
                !destination.exists(),
                "preview/incomplete source was cached"
            );
        }
        assert!(!std::fs::read_dir(temp.path()).unwrap().any(|entry| entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|e| e == "part")));
        let _ = stop.send(());
        server.join().unwrap();
    }
}

// Drive the same actor with an already-resolved recommendation response:
// no account/network fixture can own queue progression or hide stale results.
#[tokio::test]
async fn recommendations_replace_exhausted_batch_and_reject_cancelled_response() {
    let temp = tempfile::tempdir().unwrap();
    let paths = Paths::isolated(temp.path().into()).unwrap();
    let api = Api::new(paths.clone(), "").unwrap();
    let saved = Saved {
        continuous: Continuous::Fm,
        ..Saved::default()
    };
    let settings = Arc::new(RwLock::new(saved.settings.clone()));
    let (tx, rx) = mpsc::unbounded_channel();
    let (events, state) = watch::channel(Arc::new(Snapshot {
        revision: 0,
        queue_revision: 0,
        queue: Arc::new(Vec::new()),
        current_id: None,
        playback: Playback::Loading,
        position_ms: 0,
        duration_ms: 0,
        volume: 0.8,
        repeat: crate::Repeat::Off,
        shuffle: false,
        continuous: Continuous::Fm,
        error: None,
    }));
    let mut player = Player {
        saved,
        paths,
        api: api.clone(),
        shared_settings: settings.clone(),
        tx: tx.clone(),
        events,
        engine: None,
        queue: Arc::new(Vec::new()),
        revision: 0,
        queue_revision: 0,
        generation: 10,
        status: Playback::Loading,
        desired_playing: false,
        duration_ms: 0,
        error: None,
        pending_seek: None,
        failures: 0,
        last_save: Instant::now(),
        task: None,
    };
    player.saved.queue.replace(
        vec![Track {
            id: 1,
            ..Default::default()
        }],
        0,
    );
    player.queue_changed();
    let core = Core {
        tx: tx.clone(),
        state,
        settings,
        api,
    };
    // These are intentionally unavailable: no network or audio is needed to
    // exercise generation checks and recommendation queue replacement.
    tx.send(Message::Refilled(
        9,
        Ok(vec![Track {
            id: 99,
            ..Default::default()
        }]),
    ))
    .unwrap();
    tx.send(Message::Refilled(
        10,
        Ok(vec![Track {
            id: 2,
            ..Default::default()
        }]),
    ))
    .unwrap();
    let task = tokio::spawn(player.run(rx));
    core.command(Command::Pause).await.unwrap();
    assert_eq!(
        core.snapshot()
            .queue
            .iter()
            .map(|t| t.id)
            .collect::<Vec<_>>(),
        [2]
    );
    assert_eq!(core.snapshot().current_id, Some(2));
    core.command(Command::Clear).await.unwrap();
    tx.send(Message::Refilled(
        10,
        Ok(vec![Track {
            id: 3,
            ..Default::default()
        }]),
    ))
    .unwrap();
    core.command(Command::Pause).await.unwrap();
    assert!(core.snapshot().queue.is_empty());
    assert_eq!(core.snapshot().continuous, Continuous::None);
    core.command(Command::Shutdown).await.unwrap();
    task.await.unwrap();
}

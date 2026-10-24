#![cfg(feature = "audio")]
use music_core::{Command, Core, Paths, Playback, Track};
use std::{io::Write, time::Duration};

fn tone(path: &std::path::Path, seconds: u32) {
    let samples = seconds * 8000;
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + samples * 2).to_le_bytes()).unwrap();
    file.write_all(b"WAVEfmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    for value in [1u16, 1] {
        file.write_all(&value.to_le_bytes()).unwrap();
    }
    file.write_all(&8000u32.to_le_bytes()).unwrap();
    file.write_all(&16000u32.to_le_bytes()).unwrap();
    file.write_all(&2u16.to_le_bytes()).unwrap();
    file.write_all(&16u16.to_le_bytes()).unwrap();
    file.write_all(b"data").unwrap();
    file.write_all(&(samples * 2).to_le_bytes()).unwrap();
    for i in 0..samples {
        file.write_all(
            &((i as f64 * 440.0 * std::f64::consts::TAU / 8000.0)
                .sin()
                .mul_add(5000.0, 0.0) as i16)
                .to_le_bytes(),
        )
        .unwrap();
    }
}

async fn wait(core: &Core, predicate: impl Fn(&music_core::Snapshot) -> bool) {
    let mut updates = core.subscribe();
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            if predicate(&updates.borrow_and_update()) {
                break;
            }
            updates.changed().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "playback did not reach expected state: {:?}",
            core.snapshot()
        )
    });
}

#[tokio::test]
async fn real_gstreamer_without_ui_advances_seeks_and_restores() {
    std::env::set_var("NCM_AUDIO_SINK", "fakesink");
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("tone.wav");
    tone(&path, 3);
    let source = url::Url::from_file_path(&path).unwrap().to_string();
    let paths = Paths::isolated(temp.path().join("state")).unwrap();
    let core = Core::start(paths.clone()).unwrap();
    let songs = (1..=2)
        .map(|id| Track {
            id,
            name: format!("tone {id}"),
            source: Some(source.clone()),
            duration_ms: 3000,
            playable: true,
            ..Default::default()
        })
        .collect();
    core.command(Command::Replace {
        tracks: songs,
        index: 0,
    })
    .await
    .unwrap();
    wait(&core, |s| {
        s.playback == Playback::Playing && s.position_ms >= 250
    })
    .await;
    core.command(Command::Pause).await.unwrap();
    wait(&core, |s| s.playback == Playback::Paused).await;
    core.command(Command::Seek { position_ms: 2000 })
        .await
        .unwrap();
    core.command(Command::Volume { value: 0.37 }).await.unwrap();
    core.command(Command::Play).await.unwrap();
    wait(&core, |s| {
        s.current_id == Some(2) && s.playback == Playback::Playing
    })
    .await;
    // A newly-created subscriber recovers the state without owning the player.
    assert_eq!(core.subscribe().borrow().current_id, Some(2));
    core.command(Command::Pause).await.unwrap();
    core.command(Command::Seek { position_ms: 1200 })
        .await
        .unwrap();
    core.command(Command::Shutdown).await.unwrap();
    let restored = Core::start(paths).unwrap();
    assert_eq!(restored.snapshot().current_id, Some(2));
    assert_eq!(restored.snapshot().position_ms, 1200);
    assert_eq!(restored.snapshot().volume, 0.37);
    assert_eq!(restored.snapshot().playback, Playback::Stopped);
    // Late results from rapid selections must not overwrite the final selection.
    for id in [1, 2, 1, 2] {
        restored.command(Command::Select { id }).await.unwrap();
    }
    wait(&restored, |s| {
        s.current_id == Some(2) && s.playback == Playback::Playing
    })
    .await;
    restored.command(Command::Clear).await.unwrap();
    assert!(restored.command(Command::Play).await.is_err());
    restored
        .command(Command::Append {
            tracks: vec![Track {
                id: 3,
                source: Some(source),
                playable: true,
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    restored.command(Command::Play).await.unwrap();
    wait(&restored, |s| {
        s.current_id == Some(3) && s.playback == Playback::Playing
    })
    .await;
    restored.command(Command::Shutdown).await.unwrap();
}

//! Opt-in native lifecycle verification; excluded from normal/release builds.
use anyhow::{Context, Result};
use music_core::{Command, Core, Playback, Track};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tauri::Manager;

pub fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let result = run(&app).await;
        match &result {
            Ok(()) => println!("NATIVE_SMOKE_OK"),
            Err(error) => eprintln!("NATIVE_SMOKE_FAILED: {error:#}"),
        }
        if let Some(path) = std::env::var_os("NCM_NATIVE_SMOKE_REPORT") {
            let _ = std::fs::write(
                path,
                match &result {
                    Ok(()) => "PASS".into(),
                    Err(e) => format!("FAIL: {e:#}"),
                },
            );
        }
        let core = app.state::<Core>().inner().clone();
        let _ = core.command(Command::Shutdown).await;
        app.state::<crate::Lifecycle>()
            .quitting
            .store(true, std::sync::atomic::Ordering::SeqCst);
        app.exit(if result.is_ok() { 0 } else { 1 });
    });
}

async fn until(mut predicate: impl FnMut() -> bool, description: &str) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(20), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .with_context(|| format!("Timed out: {description}"))?;
    Ok(())
}

async fn frontend(app: &tauri::AppHandle, cycle: usize) -> Result<()> {
    let core = app.state::<Core>();
    let file = core.api.paths.config.join("ui.json");
    let script = format!(
        r#"
      (async () => {{
        if (!document.querySelector('.player-bar') || !document.querySelector('.connection .online')) return;
        const invoke = window.__TAURI_INTERNALS__.invoke;
        const s = await invoke('player_snapshot');
        if (!document.querySelector('.track-info strong')?.textContent?.includes('Native smoke')) return;
        if ({cycle} > 0 && (!document.querySelector('.settings-page') || !document.querySelector('.queue-panel'))) return;
        const saved = await invoke('ui_state');
        await invoke('ui_state', {{ value: {{ ...saved, smokeCycle: {cycle}, currentId: s.current_id, position: s.position_ms }} }});
      }})().catch(console.error);
    "#
    );
    let window = app
        .get_webview_window("main")
        .context("main window missing")?;
    // Wait for the real application DOM, not just native window creation.
    let start = Instant::now();
    loop {
        window.eval(&script)?;
        tokio::time::sleep(Duration::from_millis(150)).await;
        if let Ok(bytes) = std::fs::read(&file) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if value["smokeCycle"].as_u64() == Some(cycle as u64) {
                    anyhow::ensure!(
                        value["currentId"].as_u64() == core.snapshot().current_id,
                        "snapshot does not match core"
                    );
                    println!(
                        "Window {cycle} IPC restored in {} ms",
                        start.elapsed().as_millis()
                    );
                    return Ok(());
                }
            }
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(20),
            "WebView IPC failed to initialize"
        );
    }
}

async fn run(app: &tauri::AppHandle) -> Result<()> {
    anyhow::ensure!(
        std::env::var_os("NCM_DATA_DIR").is_some(),
        "native smoke requires isolated data"
    );
    let file = PathBuf::from(std::env::var("NCM_NATIVE_SMOKE_AUDIO")?);
    let core = app.state::<Core>().inner().clone();
    let source = tauri::Url::from_file_path(file.canonicalize()?)
        .map_err(|_| anyhow::anyhow!("Invalid local audio path"))?
        .to_string();
    let tracks = (1..=2)
        .map(|id| Track {
            id,
            name: format!("Native smoke {id}"),
            artist: "Integration test".into(),
            source: Some(source.clone()),
            playable: true,
            duration_ms: 6000,
            ..Default::default()
        })
        .collect();
    core.command(Command::Replace { tracks, index: 0 }).await?;
    until(
        || core.snapshot().playback == Playback::Playing,
        "initial playback",
    )
    .await?;
    frontend(app, 0).await?;
    app.get_webview_window("main").unwrap().eval(
        "location.hash = '/settings'; document.querySelector('[aria-label=\"播放队列\"]').click();",
    )?;
    until(
        || {
            std::fs::read(core.api.paths.config.join("ui.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .is_some_and(|v| v["route"] == "/settings" && v["queue"] == true)
        },
        "persist real navigation and queue panel",
    )
    .await?;
    core.command(Command::Seek { position_ms: 5000 }).await?;
    app.get_webview_window("main")
        .context("main window missing")?
        .close()?;
    until(
        || app.webview_windows().is_empty(),
        "all WebViews destroyed",
    )
    .await?;
    until(
        || core.snapshot().current_id == Some(2) && core.snapshot().playback == Playback::Playing,
        "EOS auto-next without a window",
    )
    .await?;
    core.command(Command::Mode {
        repeat: music_core::Repeat::One,
        shuffle: false,
    })
    .await?;
    #[cfg(target_os = "linux")]
    mpris("org.mpris.MediaPlayer2.Player.Pause").await?;
    #[cfg(not(target_os = "linux"))]
    core.command(Command::Pause).await?;
    until(
        || core.snapshot().playback == Playback::Paused,
        "MPRIS pause without UI",
    )
    .await?;
    #[cfg(target_os = "linux")]
    mpris("org.mpris.MediaPlayer2.Player.Play").await?;
    #[cfg(not(target_os = "linux"))]
    core.command(Command::Play).await?;
    until(
        || core.snapshot().playback == Playback::Playing,
        "MPRIS play without UI",
    )
    .await?;
    #[cfg(target_os = "linux")]
    mpris("org.mpris.MediaPlayer2.Raise").await?;
    #[cfg(target_os = "windows")]
    {
        anyhow::ensure!(
            app.tray_by_id("music-player").is_some(),
            "Windows tray missing"
        );
        crate::show_main(app);
    }
    until(
        || app.get_webview_window("main").is_some(),
        "MPRIS Raise recreates window",
    )
    .await?;
    frontend(app, 100).await?;
    app.get_webview_window("main").unwrap().close()?;
    until(|| app.webview_windows().is_empty(), "close raised window").await?;
    tokio::task::spawn_blocking(|| -> Result<()> {
        let mut process = std::process::Command::new(std::env::current_exe()?)
            .env_remove("NCM_NATIVE_SMOKE")
            .spawn()?;
        let start = Instant::now();
        loop {
            if let Some(status) = process.try_wait()? {
                anyhow::ensure!(status.success(), "second instance failed");
                break;
            }
            if start.elapsed() > Duration::from_secs(10) {
                process.kill()?;
                anyhow::bail!("second instance remained running");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    })
    .await??;
    until(
        || app.get_webview_window("main").is_some(),
        "second instance raises existing window",
    )
    .await?;
    frontend(app, 101).await?;
    app.get_webview_window("main").unwrap().close()?;
    until(
        || app.webview_windows().is_empty(),
        "close single-instance window",
    )
    .await?;
    for cycle in 1..=30 {
        let app_show = app.clone();
        app.run_on_main_thread(move || crate::show_main(&app_show))?;
        until(
            || app.get_webview_window("main").is_some(),
            "window recreated",
        )
        .await?;
        frontend(app, cycle).await?;
        app.get_webview_window("main").unwrap().close()?;
        until(|| app.webview_windows().is_empty(), "window destroyed").await?;
        anyhow::ensure!(
            core.snapshot().current_id == Some(2),
            "current song changed during reconstruction"
        );
        #[cfg(target_os = "linux")]
        if cycle % 5 == 0 {
            let status = std::fs::read_to_string("/proc/self/smaps_rollup")?;
            if let Some(pss) = status.lines().find(|line| line.starts_with("Pss:")) {
                println!("Cycle {cycle}: {pss}");
            }
        }
    }
    if let Ok(seconds) = std::env::var("NCM_NATIVE_SOAK_SECONDS") {
        let duration = Duration::from_secs(seconds.parse()?);
        let started = Instant::now();
        let mut last_progress = Instant::now();
        let mut previous = core.snapshot().position_ms;
        let mut cycles = 0;
        while started.elapsed() < duration {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let state = core.snapshot();
            anyhow::ensure!(
                app.webview_windows().is_empty(),
                "soak unexpectedly has WebViews"
            );
            anyhow::ensure!(
                state.playback != Playback::Error,
                "soak playback failed: {:?}",
                state.error
            );
            if state.position_ms != previous {
                last_progress = Instant::now();
            }
            if state.position_ms < previous {
                cycles += 1;
            }
            previous = state.position_ms;
            anyhow::ensure!(
                last_progress.elapsed() < Duration::from_secs(15),
                "soak playback stalled"
            );
            if started.elapsed().as_secs().is_multiple_of(60) {
                println!(
                    "SOAK {}s: completed loops {cycles}",
                    started.elapsed().as_secs()
                );
            }
        }
        anyhow::ensure!(cycles > 0, "soak did not cross EOS");
        println!(
            "SOAK_OK: {} seconds, {cycles} loops without WebViews",
            started.elapsed().as_secs()
        );
    }
    core.command(Command::Pause).await?;
    until(
        || core.snapshot().playback == Playback::Paused,
        "pause without UI",
    )
    .await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    Ok(())
}

#[cfg(target_os = "linux")]
async fn mpris(method: &'static str) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        let out = std::process::Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.mpris.MediaPlayer2.NeteaseCloudMusicDesktop",
                "--object-path",
                "/org/mpris/MediaPlayer2",
                "--method",
                method,
            ])
            .output()?;
        anyhow::ensure!(
            out.status.success(),
            "MPRIS {method}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        println!("MPRIS passed: {method}");
        Ok(())
    })
    .await?
}

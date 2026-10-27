//! Platform-specific shell integration; playback stays in music-core.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod startup;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod tray_icon;

use music_core::{Command, Core};

fn dispatch(core: &Core, command: Command) {
    let core = core.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = core.command(command).await {
            log::warn!("媒体操作失败: {error}");
        }
    });
}

pub fn start(app: tauri::AppHandle, core: Core) -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    linux::start(app, core)?;
    #[cfg(target_os = "windows")]
    windows::start(app, core)?;
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let _ = (app, core);
    Ok(())
}

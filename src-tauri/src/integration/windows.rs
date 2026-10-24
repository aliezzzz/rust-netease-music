use anyhow::Context;
use music_core::{Command, Core, Playback};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use super::dispatch;

/// Called from Tauri setup on the event-loop thread, once per process.
/// A failed tray is fatal on Windows: do not leave a hidden, unreachable player.
pub fn start(app: tauri::AppHandle, core: Core) -> anyhow::Result<()> {
    let title = MenuItem::with_id(&app, "track", "网易云音乐", false, None::<&str>)?;
    let show = MenuItem::with_id(&app, "show", "打开主窗口", true, None::<&str>)?;
    let previous = MenuItem::with_id(&app, "previous", "上一首", true, None::<&str>)?;
    let toggle = MenuItem::with_id(&app, "toggle", "播放", true, None::<&str>)?;
    let next = MenuItem::with_id(&app, "next", "下一首", true, None::<&str>)?;
    let quit = MenuItem::with_id(&app, "quit", "退出", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(&app)?;
    let last_separator = PredefinedMenuItem::separator(&app)?;
    let menu = Menu::with_items(
        &app,
        &[
            &title,
            &separator,
            &show,
            &previous,
            &toggle,
            &next,
            &last_separator,
            &quit,
        ],
    )?;
    let commands = core.clone();
    let tray = TrayIconBuilder::with_id("music-player")
        .icon(app.default_window_icon().context("缺少托盘图标")?.clone())
        .tooltip("网易云音乐")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => crate::show_main(app),
            "previous" => dispatch(&commands, Command::Previous),
            "toggle" => dispatch(&commands, Command::Toggle),
            "next" => dispatch(&commands, Command::Next),
            "quit" => crate::request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                crate::show_main(tray.app_handle());
            }
        })
        .build(&app)?;

    // Keep handles alive even after the WebView is destroyed. Only update native
    // menu items when relevant state changes, not on each progress notification.
    let mut updates = core.subscribe();
    tauri::async_runtime::spawn(async move {
        let mut last = None;
        loop {
            let state = updates.borrow_and_update().clone();
            let track = state.queue.iter().find(|t| Some(t.id) == state.current_id);
            let label = track
                .map(|t| format!("{} — {}", t.name, t.artist))
                .unwrap_or_else(|| "网易云音乐".into());
            let playing = state.playback == Playback::Playing;
            let enabled = track.is_some();
            let key = (label.clone(), playing, enabled);
            if last.as_ref() != Some(&key) {
                // Windows tooltips have a small UTF-16 limit. Use a conservative
                // scalar limit, preserving UTF-8 boundaries (including emoji).
                let tooltip: String = label.chars().take(60).collect();
                let result = (|| -> tauri::Result<()> {
                    title.set_text(&label)?;
                    tray.set_tooltip(Some(&tooltip))?;
                    toggle.set_text(if playing { "暂停" } else { "播放" })?;
                    toggle.set_enabled(enabled)?;
                    previous.set_enabled(enabled)?;
                    next.set_enabled(enabled)?;
                    Ok(())
                })();
                if let Err(error) = result {
                    log::warn!("更新 Windows 托盘失败: {error}");
                }
                last = Some(key);
            }
            if updates.changed().await.is_err() {
                break;
            }
        }
    });
    Ok(())
}

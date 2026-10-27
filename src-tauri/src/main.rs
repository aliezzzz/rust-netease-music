#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod integration;
mod runtime;
#[cfg(feature = "native-smoke")]
mod smoke;

use music_core::{api::Query, persistence::atomic_write, Command, Core, Paths, Settings, Snapshot};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{Emitter, Manager};

struct Lifecycle {
    quitting: AtomicBool,
}
#[derive(Default)]
struct UiPersistence(tokio::sync::Mutex<()>);
type Response<T> = Result<T, String>;
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
fn player_snapshot(core: tauri::State<'_, Core>) -> Arc<Snapshot> {
    core.snapshot()
}

#[tauri::command]
async fn player_command(core: tauri::State<'_, Core>, command: Command) -> Response<()> {
    if matches!(command, Command::Shutdown) {
        return Err("请使用应用退出命令".into());
    }
    core.command(command).await.map_err(error)
}

#[tauri::command]
async fn browse(core: tauri::State<'_, Core>, query: Query) -> Response<music_core::Page> {
    core.api.query(query).await.map_err(error)
}

#[tauri::command]
async fn lyrics(
    core: tauri::State<'_, Core>,
    id: u64,
) -> Response<Vec<music_core::lyrics::LyricLine>> {
    core.api.lyrics(id).await.map_err(error)
}

#[tauri::command]
fn settings(core: tauri::State<'_, Core>) -> Settings {
    core.settings()
}

#[tauri::command]
async fn save_settings(core: tauri::State<'_, Core>, settings: Settings) -> Response<()> {
    core.command(Command::Settings { settings })
        .await
        .map_err(error)
}

#[tauri::command]
async fn account(
    core: tauri::State<'_, Core>,
    operation: String,
    args: Option<Value>,
) -> Response<Value> {
    let api = &core.api;
    let client = api.client();
    let args = args.unwrap_or(json!({}));
    let text = |key: &str| args[key].as_str().unwrap_or_default().to_owned();
    let value = match operation.as_str() {
        "status" => {
            serde_json::to_value(client.login_status().await.map_err(error)?).map_err(error)?
        }
        "qr" => {
            let (url, key) = client.login_qr_create().await.map_err(error)?;
            let svg = music_core::api::qr_svg(&url).map_err(error)?;
            json!({"key": key, "svg": svg})
        }
        "qr_check" => {
            let result = client.login_qr_check(text("key")).await.map_err(error)?;
            if result.code == 803 {
                api.save_session().map_err(error)?;
            }
            serde_json::to_value(result).map_err(error)?
        }
        "captcha" => {
            client
                .captcha(text("country"), text("phone"))
                .await
                .map_err(error)?;
            Value::Null
        }
        "login" => {
            let result = client
                .login_cellphone(text("country"), text("phone"), text("captcha"))
                .await
                .map_err(error)?;
            if result.code == 200 {
                api.save_session().map_err(error)?;
            }
            serde_json::to_value(result).map_err(error)?
        }
        "logout" => {
            api.logout().await.map_err(error)?;
            Value::Null
        }
        "likes" => {
            let uid = api.user_id().await.map_err(error)?;
            json!(client.user_song_id_list(uid).await.map_err(error)?)
        }
        "favorite_id" => json!(api.favorite_id().await.map_err(error)?),
        _ => return Err("未知账户操作".into()),
    };
    Ok(value)
}

#[tauri::command]
async fn favorite(
    core: tauri::State<'_, Core>,
    kind: String,
    id: u64,
    liked: bool,
) -> Response<()> {
    let api = core.api.client();
    let success = match kind.as_str() {
        "song" => api.like_v1(liked, id).await,
        "playlist" => api.song_list_like(liked, id).await,
        "album" => api.album_like(liked, id).await,
        "fm_trash" => api.fm_trash(id).await,
        _ => return Err("未知收藏类型".into()),
    };
    if success {
        Ok(())
    } else {
        Err("操作失败，请检查登录状态后重试".into())
    }
}

#[tauri::command]
async fn ui_state(
    core: tauri::State<'_, Core>,
    persistence: tauri::State<'_, UiPersistence>,
    value: Option<Value>,
) -> Response<Value> {
    let _guard = persistence.0.lock().await;
    let path = core.api.paths.config.join("ui.json");
    tokio::task::spawn_blocking(move || {
        if let Some(value) = value {
            atomic_write(&path, &serde_json::to_vec(&value).map_err(error)?).map_err(error)?;
            Ok(value)
        } else {
            Ok(std::fs::read(path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or(json!({})))
        }
    })
    .await
    .map_err(error)?
}

#[tauri::command]
async fn cache(core: tauri::State<'_, Core>, clear: bool) -> Response<u64> {
    let dir = core.api.paths.cache.clone();
    tokio::task::spawn_blocking(move || -> Response<u64> {
        let mut size = 0;
        for entry in std::fs::read_dir(dir).map_err(error)? {
            let entry = entry.map_err(error)?;
            let meta = entry.metadata().map_err(error)?;
            if meta.is_file() {
                if clear {
                    std::fs::remove_file(entry.path()).map_err(error)?;
                } else {
                    size += meta.len();
                }
            }
        }
        Ok(size)
    })
    .await
    .map_err(error)?
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    request_quit(&app);
}

pub fn show_main(app: &tauri::AppHandle) {
    show_main_with_activation(app, None);
}

pub(crate) fn show_main_with_activation(app: &tauri::AppHandle, activation_token: Option<String>) {
    // Capture tray tokens in the callback, before leaving ksni's event scope.
    // Serialize lookup+creation and GTK activation on the UI thread.
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if !handle.state::<Lifecycle>().quitting.load(Ordering::SeqCst) {
            show_main_on_ui_thread(&handle, activation_token.as_deref());
        }
    }) {
        log::error!("恢复主窗口失败: {error}");
    }
}

fn show_main_on_ui_thread(app: &tauri::AppHandle, activation_token: Option<&str>) {
    let window = match app.get_webview_window("main") {
        Some(window) => window,
        None => {
            let result = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("网易云音乐")
            .decorations(false)
            .inner_size(1280.0, 840.0)
            .min_inner_size(760.0, 560.0)
            // A tray activation must reach GTK before the first map/present.
            .visible(activation_token.is_none())
            .build();
            match result {
                Ok(window) => window,
                Err(error) => {
                    log::error!("创建主窗口失败: {error}");
                    return;
                }
            }
        }
    };
    #[cfg(target_os = "linux")]
    if let Some(token) = activation_token {
        use gtk::prelude::GtkWindowExt;
        match window.gtk_window() {
            Ok(native) => native.set_startup_id(token),
            Err(error) => log::warn!("设置窗口激活令牌失败: {error}"),
        }
    }
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn request_quit(app: &tauri::AppHandle) {
    if app
        .state::<Lifecycle>()
        .quitting
        .swap(true, Ordering::SeqCst)
    {
        return;
    }
    let app = app.clone();
    let core = app.state::<Core>().inner().clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = core.command(Command::Shutdown).await {
            log::error!("保存退出状态失败: {error}");
        }
        app.exit(0);
    });
}

fn startup_error(message: &str) {
    eprintln!("{message}");
    #[cfg(windows)]
    {
        let title: Vec<u16> = "网易云音乐启动失败\0".encode_utf16().collect();
        let text: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        // Release is a GUI-subsystem executable: errors must not disappear into
        // an invisible stderr when plugins or tray initialization fail.
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                windows_sys::Win32::UI::WindowsAndMessaging::MB_OK
                    | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
    }
}

fn main() {
    // Set up plugin discovery before any GStreamer initialization or worker thread.
    if let Err(error) = runtime::prepare() {
        startup_error(&format!("音频运行环境初始化失败: {error:#}"));
        std::process::exit(1);
    }
    env_logger::init();
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_main(app)))
        .manage(Lifecycle { quitting: AtomicBool::new(false) })
        .manage(UiPersistence::default())
        .invoke_handler(tauri::generate_handler![player_snapshot, player_command, browse, lyrics, settings, save_settings, account, favorite, ui_state, cache, quit])
        .setup(|app| {
            let core = tauri::async_runtime::block_on(async { Core::start(Paths::discover()?) })?;
            app.manage(core.clone());
            let handle = app.handle().clone(); let mut updates = core.subscribe();
            tauri::async_runtime::spawn(async move {
                let mut queue_revision = u64::MAX;
                while updates.changed().await.is_ok() {
                    if handle.get_webview_window("main").is_none() { continue; }
                    let snapshot = updates.borrow_and_update().clone();
                    // Progress ticks carry only scalar state; large queues travel on change.
                    let mut value = json!({
                        "revision": snapshot.revision, "queue_revision": snapshot.queue_revision,
                        "current_id": snapshot.current_id, "playback": snapshot.playback,
                        "position_ms": snapshot.position_ms, "duration_ms": snapshot.duration_ms,
                        "volume": snapshot.volume, "repeat": snapshot.repeat, "shuffle": snapshot.shuffle,
                        "continuous": snapshot.continuous, "error": snapshot.error,
                    });
                    if queue_revision != snapshot.queue_revision { value["queue"] = json!(snapshot.queue); }
                    queue_revision = snapshot.queue_revision;
                    let _ = handle.emit_to("main", "player-state", value);
                }
            });
            integration::start(app.handle().clone(), core)?;
            show_main_on_ui_thread(app.handle(), None);
            #[cfg(feature = "native-smoke")]
            if std::env::var_os("NCM_NATIVE_SMOKE").is_some() {
                smoke::start(app.handle().clone());
            }
            Ok(())
        })
        .build(tauri::generate_context!());
    match result {
        Ok(app) => app.run(|handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !handle.state::<Lifecycle>().quitting.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        }),
        Err(error) => {
            startup_error(&format!("应用启动失败: {error:#}"));
            std::process::exit(1);
        }
    }
}

use ksni::TrayMethods;
use mpris_server::{LoopStatus, Metadata, PlaybackStatus, Player, Time, TrackId};
use music_core::{Command, Core, Playback, Repeat};

use super::dispatch;

struct Tray {
    app: tauri::AppHandle,
    core: Core,
    title: String,
    playing: bool,
    icons: Vec<ksni::Icon>,
}
impl Tray {
    fn show_main(&self) {
        // The token is task-local to this tray/menu event. Looking it up later
        // on Tauri's UI thread would lose the desktop's startup notification.
        crate::show_main_with_activation(&self.app, ksni::current_xdg_activation_token());
    }

    fn without_window(&self, action: impl FnOnce(tauri::AppHandle, Core) + Send + 'static) {
        let token = ksni::current_xdg_activation_token();
        let app = self.app.clone();
        let core = self.core.clone();
        tauri::async_runtime::spawn(async move {
            // Finish the desktop's launch feedback even though this action
            // does not map a window. In particular, do this before app.exit().
            super::startup::complete(&app, token).await;
            action(app, core);
        });
    }
}
impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "netease-cloud-music-desktop".into()
    }
    fn title(&self) -> String {
        self.title.clone()
    }
    fn icon_name(&self) -> String {
        // A theme name takes precedence over IconPixmap on many desktops.
        // Leave it empty so the supplied artwork is actually used.
        String::new()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icons.clone()
    }
    fn activate(&mut self, _: i32, _: i32) {
        self.show_main();
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::{menu::StandardItem, MenuItem};
        vec![
            StandardItem {
                label: self.title.clone(),
                activate: Box::new(|s: &mut Self| s.show_main()),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "上一首".into(),
                icon_name: "media-skip-backward-symbolic".into(),
                activate: Box::new(|s: &mut Self| s.without_window(|_, core| dispatch(&core, Command::Previous))),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: if self.playing { "暂停" } else { "播放" }.into(),
                icon_name: if self.playing {
                    "media-playback-pause-symbolic"
                } else {
                    "media-playback-start-symbolic"
                }.into(),
                activate: Box::new(|s: &mut Self| s.without_window(|_, core| dispatch(&core, Command::Toggle))),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "下一首".into(),
                icon_name: "media-skip-forward-symbolic".into(),
                activate: Box::new(|s: &mut Self| s.without_window(|_, core| dispatch(&core, Command::Next))),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "打开主窗口".into(),
                icon_name: "window-new-symbolic".into(),
                activate: Box::new(|s: &mut Self| s.show_main()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "退出".into(),
                icon_name: "application-exit-symbolic".into(),
                activate: Box::new(|s: &mut Self| s.without_window(|app, _| crate::request_quit(&app))),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn start(app: tauri::AppHandle, core: Core) -> anyhow::Result<()> {
    let tray = Tray {
        app: app.clone(),
        core: core.clone(),
        title: "网易云音乐".into(),
        playing: false,
        icons: super::tray_icon::linux_icons()?,
    };
    let mut updates = core.subscribe();
    tauri::async_runtime::spawn(async move {
        // The isolated native test has no desktop shell, but still exercises
        // the real StatusNotifierItem activation methods over D-Bus.
        let tray = tray.assume_sni_available(
            cfg!(feature = "native-smoke") && std::env::var_os("NCM_NATIVE_SMOKE").is_some(),
        );
        match tray.spawn().await {
            Ok(handle) => {
                let mut last = (None, Playback::Stopped);
                while updates.changed().await.is_ok() {
                    let state = updates.borrow_and_update().clone();
                    let next = (state.current_id, state.playback);
                    if last == next {
                        continue;
                    }
                    last = next;
                    let title = state
                        .queue
                        .iter()
                        .find(|t| Some(t.id) == state.current_id)
                        .map(|t| format!("{} — {}", t.name, t.artist))
                        .unwrap_or_else(|| "网易云音乐".into());
                    handle
                        .update(move |tray| {
                            tray.title = title;
                            tray.playing = state.playback == Playback::Playing;
                        })
                        .await;
                }
            }
            Err(error) => log::warn!("系统托盘不可用: {error}"),
        }
    });
    // mpris-server::Player is local/Rc-based. Keep it on one dedicated executor;
    // only the thread-safe core command handle crosses this boundary.
    std::thread::Builder::new()
        .name("mpris".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("MPRIS runtime");
            let local = tokio::task::LocalSet::new();
            local.block_on(&runtime, async move {
                if let Err(error) = run_mpris(app, core).await {
                    log::error!("MPRIS 启动失败: {error}");
                }
            });
        })
        .expect("MPRIS thread");
    Ok(())
}

async fn run_mpris(app: tauri::AppHandle, core: Core) -> anyhow::Result<()> {
    let player = Player::builder("NeteaseCloudMusicDesktop")
        .identity("网易云音乐")
        .desktop_entry("netease-cloud-music-desktop")
        .can_raise(true)
        .can_quit(true)
        .can_control(true)
        .can_play(true)
        .can_pause(true)
        .can_go_next(true)
        .can_go_previous(true)
        .can_seek(true)
        .build()
        .await?;
    let app_show = app.clone();
    player.connect_raise(move |_| crate::show_main(&app_show));
    player.connect_quit(move |_| crate::request_quit(&app));
    let c = core.clone();
    player.connect_play_pause(move |_| dispatch(&c, Command::Toggle));
    let c = core.clone();
    player.connect_play(move |_| dispatch(&c, Command::Play));
    let c = core.clone();
    player.connect_pause(move |_| dispatch(&c, Command::Pause));
    let c = core.clone();
    player.connect_stop(move |_| dispatch(&c, Command::Stop));
    let c = core.clone();
    player.connect_next(move |_| dispatch(&c, Command::Next));
    let c = core.clone();
    player.connect_previous(move |_| dispatch(&c, Command::Previous));
    let c = core.clone();
    player.connect_set_volume(move |_, value| {
        dispatch(
            &c,
            Command::Volume {
                value: value.clamp(0.0, 1.0),
            },
        )
    });
    let c = core.clone();
    player.connect_seek(move |_, offset| {
        let position_ms = (c.snapshot().position_ms as i64)
            .saturating_add(offset.as_millis())
            .max(0) as u64;
        dispatch(&c, Command::Seek { position_ms });
    });
    let c = core.clone();
    player.connect_set_position(move |_, track_id, position| {
        let state = c.snapshot();
        if state
            .current_id
            .is_some_and(|id| track_id.as_str() == format!("/net/netease/track/{id}"))
            && position.as_millis() >= 0
        {
            dispatch(
                &c,
                Command::Seek {
                    position_ms: position.as_millis() as u64,
                },
            );
        }
    });
    let c = core.clone();
    player.connect_set_loop_status(move |_, status| {
        dispatch(
            &c,
            Command::Mode {
                repeat: match status {
                    LoopStatus::None => Repeat::Off,
                    LoopStatus::Track => Repeat::One,
                    LoopStatus::Playlist => Repeat::All,
                },
                shuffle: c.snapshot().shuffle,
            },
        )
    });
    let c = core.clone();
    player.connect_set_shuffle(move |_, shuffle| {
        dispatch(
            &c,
            Command::Mode {
                repeat: c.snapshot().repeat,
                shuffle,
            },
        )
    });
    tokio::task::spawn_local(player.run());
    let mut updates = core.subscribe();
    let mut previous_id = None;
    let mut previous_duration = 0;
    let mut previous_position = 0;
    loop {
        let state = updates.borrow_and_update().clone();
        if previous_id != state.current_id || previous_duration != state.duration_ms {
            let mut metadata = Metadata::new();
            if let Some(track) = state.queue.iter().find(|t| Some(t.id) == state.current_id) {
                metadata.set_trackid(
                    TrackId::try_from(format!("/net/netease/track/{}", track.id)).ok(),
                );
                metadata.set_title(Some(track.name.clone()));
                metadata.set_artist(Some(vec![track.artist.clone()]));
                metadata.set_album(Some(track.album.clone()));
                metadata.set_art_url(Some(track.cover.clone()));
                metadata.set_length(Some(Time::from_millis(state.duration_ms as i64)));
            } else {
                metadata.set_trackid(Some(TrackId::NO_TRACK));
            }
            player.set_metadata(metadata).await?;
        }
        player
            .set_playback_status(match state.playback {
                Playback::Playing | Playback::Loading => PlaybackStatus::Playing,
                Playback::Paused => PlaybackStatus::Paused,
                _ => PlaybackStatus::Stopped,
            })
            .await?;
        player.set_volume(state.volume).await?;
        player.set_shuffle(state.shuffle).await?;
        player
            .set_loop_status(match state.repeat {
                Repeat::Off => LoopStatus::None,
                Repeat::One => LoopStatus::Track,
                Repeat::All => LoopStatus::Playlist,
            })
            .await?;
        player.set_can_play(state.current_id.is_some()).await?;
        player.set_can_seek(state.current_id.is_some()).await?;
        let position = Time::from_millis(state.position_ms as i64);
        player.set_position(position);
        if previous_id == state.current_id && state.position_ms.abs_diff(previous_position) > 1500 {
            player.seeked(position).await?;
        }
        previous_id = state.current_id;
        previous_duration = state.duration_ms;
        previous_position = state.position_ms;
        if updates.changed().await.is_err() {
            break;
        }
    }
    Ok(())
}

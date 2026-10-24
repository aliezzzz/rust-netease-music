// Playback derived from gmg137/netease-cloud-music-gtk:
// upstream src/gui/player_controls.rs and src/audio/playlist.rs.
// https://github.com/gmg137/netease-cloud-music-gtk
// Copyright (C) 2022 gmg137. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    persistence::Saved, Api, Command, Continuous, Paths, Playback, Settings, Snapshot, Track,
};
use anyhow::{Context, Result};
use gstreamer::{prelude::*, ClockTime};
use gstreamer_play::{Play, PlaySignalAdapter, PlayState, PlayVideoRenderer};
use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot, watch};

#[derive(Clone)]
pub struct Core {
    tx: mpsc::UnboundedSender<Message>,
    state: watch::Receiver<Arc<Snapshot>>,
    settings: Arc<RwLock<Settings>>,
    pub api: Api,
}

#[cfg(test)]
#[path = "player_tests.rs"]
mod tests;

enum Message {
    Command(Command, oneshot::Sender<Result<()>>),
    Resolved(u64, Result<String>),
    Refilled(u64, Result<Vec<Track>>),
    Engine(u64, EngineEvent),
}
enum EngineEvent {
    End,
    State(PlayState),
    Error(String),
    Seeked(u64),
}

struct Engine {
    play: Play,
    _adapter: PlaySignalAdapter,
    context: gstreamer::glib::MainContext,
    cache: Option<(std::path::PathBuf, u64)>,
    downloaded: Arc<std::sync::Mutex<Option<CachedDownload>>>,
}
struct CachedDownload {
    file: std::fs::File,
    temp: std::path::PathBuf,
}
impl Engine {
    fn new(
        uri: &str,
        volume: f64,
        generation: u64,
        tx: mpsc::UnboundedSender<Message>,
        cache: Option<(std::path::PathBuf, u64)>,
    ) -> Result<Self> {
        let play = Play::new(None::<PlayVideoRenderer>);
        // Signals are converted to plain messages. No UI/GLib main loop owns playback.
        let adapter = PlaySignalAdapter::new_sync_emit(&play);
        let (context_tx, context_rx) = std::sync::mpsc::sync_channel(1);
        adapter.connect_uri_loaded(move |_, _| {
            if let Some(context) = gstreamer::glib::MainContext::thread_default() {
                let _ = context_tx.try_send(context);
            }
        });
        let sender = tx.clone();
        adapter.connect_end_of_stream(move |_| {
            let _ = sender.send(Message::Engine(generation, EngineEvent::End));
        });
        let sender = tx.clone();
        adapter.connect_state_changed(move |_, state| {
            let _ = sender.send(Message::Engine(generation, EngineEvent::State(state)));
        });
        let sender = tx.clone();
        adapter.connect_error(move |_, error, _| {
            let _ = sender.send(Message::Engine(
                generation,
                EngineEvent::Error(error.to_string()),
            ));
        });
        adapter.connect_seek_done(move |_, position| {
            let _ = tx.send(Message::Engine(
                generation,
                EngineEvent::Seeked(position.mseconds()),
            ));
        });
        let mut config = play.config();
        config.set_position_update_interval(250);
        config.set_seek_accurate(true);
        play.set_config(config)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let downloaded = Arc::new(std::sync::Mutex::new(None));
        if let Some((destination, _)) = &cache {
            let destination =
                destination.with_extension(format!("{}.{generation}.part", std::process::id()));
            let downloaded = downloaded.clone();
            let pipeline = play.pipeline();
            let flags = pipeline.property_value("flags");
            if let Some(class) = gstreamer::glib::FlagsClass::with_type(flags.type_()) {
                if let Some(builder) = class.builder_with_value(flags) {
                    if let Some(value) = builder.set_by_nick("download").build() {
                        pipeline.set_property_from_value("flags", &value);
                    }
                }
            }
            if let Some(bus) = pipeline.bus() {
                // GstPlay already enables sync emission and owns this bus's handler.
                // Connect a signal instead of replacing its stream-selection handler.
                bus.connect_sync_message(Some("element"), move |_, message| {
                    if let gstreamer::MessageView::Element(element) = message.view() {
                        if let Some(s) = element
                            .structure()
                            .filter(|s| s.name() == "GstCacheDownloadComplete")
                        {
                            if let Ok(location) = s.get::<String>("location") {
                                // DownloadBuffer posts this before flushing its FILE buffer.
                                // Retain a descriptor (also across unlink on Linux), and copy
                                // only after stopping the pipeline has closed the writer.
                                if let Ok(file) = std::fs::File::open(location) {
                                    *downloaded.lock().unwrap() = Some(CachedDownload {
                                        file,
                                        temp: destination.clone(),
                                    });
                                }
                            }
                        }
                    }
                });
            }
        }
        if std::env::var("NCM_AUDIO_SINK").as_deref() == Ok("fakesink") {
            let sink = gstreamer::ElementFactory::make("fakesink")
                .property("sync", true)
                .build()?;
            play.pipeline().set_property("audio-sink", sink);
        }
        play.set_volume(volume);
        play.set_uri(Some(uri));
        // URI assignment runs on GstPlay's private loop. Keep its context so
        // destruction can wait until the queued stop has actually completed.
        let context = context_rx
            .recv_timeout(Duration::from_secs(5))
            .context("播放线程初始化超时")?;
        Ok(Self {
            play,
            _adapter: adapter,
            context,
            cache,
            downloaded,
        })
    }
    fn commit_cache(&self, duration_ms: u64) {
        if let Some((destination, expected_ms)) = &self.cache {
            if let Some(mut download) = self.downloaded.lock().unwrap().take() {
                // The service may return a short preview. Never persist it as
                // the full track, which would mask access after later login.
                if duration_ms > 0 && duration_ms.saturating_add(2000) >= *expected_ms {
                    let result = (|| -> std::io::Result<()> {
                        let mut file = std::fs::File::create(&download.temp)?;
                        std::io::copy(&mut download.file, &mut file)?;
                        file.sync_all()?;
                        drop(file);
                        std::fs::rename(&download.temp, destination)
                    })();
                    if let Err(error) = result {
                        log::debug!("缓存写入失败: {error}");
                    }
                }
                let _ = std::fs::remove_file(download.temp);
            }
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let duration_ms = self.play.duration().map(|t| t.mseconds()).unwrap_or(0);
        // Flush while we still own Play. Otherwise a racing signal's GstMessage
        // may hold the last reference and dispose Play on its own main thread.
        self.play.message_bus().set_flushing(true);
        let (ready, pending) = std::sync::mpsc::sync_channel(1);
        self.context.invoke(move || {
            let _ = ready.send(());
        });
        let _ = pending.recv();
        // Let an in-flight preroll finish before asking uridecodebin3 to tear
        // down its dynamic pads (rapid track selection can otherwise race it).
        let _ = self.play.pipeline().state(ClockTime::from_seconds(5));
        self.play.stop();
        let (done, stopped) = std::sync::mpsc::sync_channel(1);
        self.context.invoke(move || {
            let _ = done.send(());
        });
        let _ = stopped.recv();
        // Messages already being emitted can still reference Play after flushing.
        // Keep our owning reference until they finish, so finalization joins the
        // private loop from this thread instead of disposing it inside itself.
        while self.play.ref_count() > 1 {
            std::thread::sleep(Duration::from_micros(50));
        }
        self.commit_cache(duration_ms);
    }
}

struct Player {
    saved: Saved,
    paths: Paths,
    api: Api,
    shared_settings: Arc<RwLock<Settings>>,
    tx: mpsc::UnboundedSender<Message>,
    events: watch::Sender<Arc<Snapshot>>,
    engine: Option<Engine>,
    queue: Arc<Vec<Track>>,
    revision: u64,
    queue_revision: u64,
    generation: u64,
    status: Playback,
    desired_playing: bool,
    duration_ms: u64,
    error: Option<String>,
    pending_seek: Option<u64>,
    failures: usize,
    last_save: Instant,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl Core {
    /// Must be called within a Tokio runtime. The returned handle never owns a window.
    pub fn start(paths: Paths) -> Result<Self> {
        gstreamer::init()?;
        let mut saved = paths.load()?;
        if saved.settings.mute_start {
            saved.settings.volume = 0.0;
        }
        let api = Api::new(paths.clone(), &saved.settings.proxy)?;
        let cleanup_api = api.clone();
        let days = saved.settings.cache_days;
        tokio::spawn(async move {
            if let Err(e) = cleanup_api.cleanup_cache(days).await {
                log::warn!("清理过期缓存失败: {e}");
            }
        });
        let settings = Arc::new(RwLock::new(saved.settings.clone()));
        let queue = Arc::new(saved.queue.ordered_tracks());
        let duration_ms = saved.queue.current().map(|s| s.duration_ms).unwrap_or(0);
        let initial = Arc::new(Snapshot {
            revision: 0,
            queue_revision: 0,
            queue: queue.clone(),
            current_id: saved.queue.current,
            playback: Playback::Stopped,
            position_ms: saved.position_ms,
            duration_ms,
            volume: saved.settings.volume,
            repeat: saved.queue.repeat,
            shuffle: saved.queue.shuffled,
            continuous: saved.continuous.clone(),
            error: None,
        });
        let (events, state) = watch::channel(initial);
        let (tx, rx) = mpsc::unbounded_channel();
        let player = Player {
            saved,
            paths,
            api: api.clone(),
            shared_settings: settings.clone(),
            tx: tx.clone(),
            events,
            engine: None,
            queue,
            revision: 0,
            queue_revision: 0,
            generation: 0,
            status: Playback::Stopped,
            desired_playing: false,
            duration_ms,
            error: None,
            pending_seek: None,
            failures: 0,
            last_save: Instant::now(),
            task: None,
        };
        tokio::spawn(player.run(rx));
        Ok(Self {
            tx,
            state,
            settings,
            api,
        })
    }
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<Arc<Snapshot>> {
        self.state.clone()
    }
    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }
    pub async fn command(&self, command: Command) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::Command(command, tx))
            .context("播放服务已退出")?;
        rx.await.context("播放服务已退出")?
    }
}

impl Player {
    fn publish(&mut self) {
        self.revision += 1;
        self.events.send_replace(Arc::new(Snapshot {
            revision: self.revision,
            queue_revision: self.queue_revision,
            queue: self.queue.clone(),
            current_id: self.saved.queue.current,
            playback: self.status,
            position_ms: self.saved.position_ms,
            duration_ms: self.duration_ms,
            volume: self.saved.settings.volume,
            repeat: self.saved.queue.repeat,
            shuffle: self.saved.queue.shuffled,
            continuous: self.saved.continuous.clone(),
            error: self.error.clone(),
        }));
    }
    fn queue_changed(&mut self) {
        self.queue_revision += 1;
        self.queue = Arc::new(self.saved.queue.ordered_tracks());
    }
    fn cancel(&mut self) {
        self.generation += 1;
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.engine.take();
    }
    fn load(&mut self, restore: bool) -> Result<()> {
        let track = self
            .saved
            .queue
            .current()
            .cloned()
            .context("播放队列为空")?;
        self.cancel();
        if !restore {
            self.saved.position_ms = 0;
        }
        self.pending_seek = (self.saved.position_ms > 0).then_some(self.saved.position_ms);
        self.duration_ms = track.duration_ms;
        self.status = Playback::Loading;
        self.error = None;
        let api = self.api.clone();
        let quality = self.saved.settings.quality.clone();
        if self.saved.settings.desktop_lyrics && track.source.is_none() {
            let api = api.clone();
            let track = track.clone();
            tokio::spawn(async move {
                if let Err(e) = api.export_desktop_lyrics(&track).await {
                    log::debug!("桌面歌词: {e}");
                }
            });
        }
        let generation = self.generation;
        let tx = self.tx.clone();
        if !track.playable && !self.saved.settings.try_unavailable && track.source.is_none() {
            let _ = tx.send(Message::Resolved(
                generation,
                Err(anyhow::anyhow!("歌曲不可播放；可在设置中启用尝试灰色歌曲")),
            ));
            return Ok(());
        }
        self.task = Some(tokio::spawn(async move {
            let result =
                tokio::time::timeout(Duration::from_secs(30), api.resolve(&track, &quality))
                    .await
                    .map_err(|_| anyhow::anyhow!("获取播放地址超时"))
                    .and_then(|r| r);
            let _ = tx.send(Message::Resolved(generation, result));
        }));
        Ok(())
    }
    fn refill(&mut self) {
        self.cancel();
        self.status = Playback::Loading;
        let mode = self.saved.continuous.clone();
        let api = self.api.clone();
        let tx = self.tx.clone();
        let generation = self.generation;
        let current = self.saved.queue.current.unwrap_or(0);
        self.task = Some(tokio::spawn(async move {
            let request = async {
                let songs = match mode {
                    Continuous::Fm => api.client().personal_fm().await?,
                    Continuous::Heartbeat { playlist_id } => {
                        api.client()
                            .playmode_intelligence_list(current, playlist_id)
                            .await?
                    }
                    Continuous::None => Vec::new(),
                };
                Ok(songs.into_iter().map(Into::into).collect())
            };
            let result = tokio::time::timeout(Duration::from_secs(30), request)
                .await
                .map_err(|_| anyhow::anyhow!("获取连续推荐超时"))
                .and_then(|r| r);
            let _ = tx.send(Message::Refilled(generation, result));
        }));
    }
    fn next(&mut self, automatic: bool) -> Result<()> {
        if self.saved.continuous != Continuous::None {
            // Continuous recommendations run before list wrapping.
            let repeat = self.saved.queue.repeat;
            self.saved.queue.repeat = crate::Repeat::Off;
            let next = self.saved.queue.advance(false);
            self.saved.queue.repeat = repeat;
            if next.is_none() {
                self.refill();
                return Ok(());
            }
        } else if self.saved.queue.advance(automatic).is_none() {
            self.cancel();
            self.desired_playing = false;
            self.status = Playback::Stopped;
            return Ok(());
        }
        self.load(false)
    }
    fn fail(&mut self, error: anyhow::Error) {
        self.error = Some(error.to_string());
        self.failures += 1;
        // Bound skipping to one pass through the queue, including repeat modes.
        if self.failures < self.saved.queue.tracks.len() && self.desired_playing {
            if let Err(error) = self.next(false) {
                self.error = Some(error.to_string());
                self.status = Playback::Error;
            }
        } else {
            self.cancel();
            self.status = Playback::Error;
            self.desired_playing = false;
        }
    }
    fn apply(&mut self, command: Command) -> Result<bool> {
        match command {
            Command::Replace { tracks, index } => {
                anyhow::ensure!(
                    !tracks.is_empty() && index < tracks.len(),
                    "无效的播放列表位置"
                );
                self.saved.queue.replace(tracks, index);
                self.saved.continuous = Continuous::None;
                self.queue_changed();
                self.desired_playing = true;
                self.failures = 0;
                self.load(false)?;
            }
            Command::Append { tracks } => {
                self.saved.queue.append(tracks);
                self.queue_changed();
            }
            Command::Select { id } => {
                anyhow::ensure!(self.saved.queue.select(id), "队列中找不到歌曲");
                self.desired_playing = true;
                self.failures = 0;
                self.load(false)?;
            }
            Command::Remove { id } => {
                let active = self.saved.queue.remove(id);
                self.queue_changed();
                if active {
                    if self.saved.queue.current().is_some() {
                        self.load(false)?;
                    } else {
                        self.cancel();
                        self.status = Playback::Stopped;
                        self.saved.position_ms = 0;
                        self.duration_ms = 0;
                    }
                }
            }
            Command::Clear => {
                self.cancel();
                self.saved.queue.replace(Vec::new(), 0);
                self.queue_changed();
                self.saved.continuous = Continuous::None;
                self.saved.position_ms = 0;
                self.duration_ms = 0;
                self.desired_playing = false;
                self.status = Playback::Stopped;
            }
            Command::Toggle => {
                return self.apply(if self.desired_playing {
                    Command::Pause
                } else {
                    Command::Play
                });
            }
            Command::Play => {
                anyhow::ensure!(self.saved.queue.current().is_some(), "播放队列为空");
                self.desired_playing = true;
                if let Some(e) = &self.engine {
                    e.play.play();
                } else {
                    self.failures = 0;
                    self.load(true)?;
                }
            }
            Command::Pause => {
                self.desired_playing = false;
                if let Some(e) = &self.engine {
                    e.play.pause();
                }
                self.status = Playback::Paused;
            }
            Command::Stop => {
                self.cancel();
                self.desired_playing = false;
                self.status = Playback::Stopped;
                self.saved.position_ms = 0;
            }
            Command::Next => {
                self.desired_playing = true;
                self.failures = 0;
                self.next(false)?;
            }
            Command::Previous => {
                if self.saved.position_ms > 3000 && self.engine.is_some() {
                    self.apply(Command::Seek { position_ms: 0 })?;
                } else if self.saved.queue.previous().is_some() {
                    self.desired_playing = true;
                    self.failures = 0;
                    self.load(false)?;
                }
            }
            Command::Seek { position_ms } => {
                let pos = if self.duration_ms > 0 {
                    position_ms.min(self.duration_ms)
                } else {
                    position_ms
                };
                if let Some(e) = &self.engine {
                    e.play.seek(ClockTime::from_mseconds(pos));
                } else {
                    self.pending_seek = Some(pos);
                }
                self.saved.position_ms = pos;
            }
            Command::Volume { value } => {
                anyhow::ensure!(
                    value.is_finite() && (0.0..=1.0).contains(&value),
                    "音量必须在 0 到 1 之间"
                );
                self.saved.settings.volume = value;
                self.shared_settings.write().unwrap().volume = value;
                if let Some(e) = &self.engine {
                    e.play.set_volume(value);
                }
            }
            Command::Mode { repeat, shuffle } => {
                let changed = self.saved.queue.shuffled != shuffle;
                self.saved.queue.mode(repeat, shuffle);
                if changed {
                    self.queue_changed();
                }
            }
            Command::Continuous { mode } => {
                if mode == Continuous::None {
                    self.saved.continuous = mode;
                    // Discard any pending refill and restore the current track if necessary.
                    if self.engine.is_none() && self.task.is_some() {
                        self.cancel();
                        if self.saved.queue.current().is_some() {
                            self.load(true)?;
                        } else {
                            self.status = Playback::Stopped;
                            self.desired_playing = false;
                        }
                    }
                    return Ok(false);
                }
                self.saved.continuous = mode;
                self.desired_playing = true;
                self.failures = 0;
                if self.saved.continuous == Continuous::Fm {
                    self.saved.queue.replace(Vec::new(), 0);
                    self.queue_changed();
                }
                if self.saved.continuous != Continuous::None {
                    self.refill();
                }
            }
            Command::Settings { settings } => {
                anyhow::ensure!(
                    ["system", "light", "dark"].contains(&settings.theme.as_str()),
                    "无效的主题"
                );
                anyhow::ensure!(
                    ncm_api::SongQuality::from_level(&settings.quality).is_some(),
                    "无效的音质"
                );
                anyhow::ensure!(
                    settings.volume.is_finite() && (0.0..=1.0).contains(&settings.volume),
                    "无效的音量"
                );
                self.api.set_proxy(&settings.proxy)?;
                if let Some(e) = &self.engine {
                    e.play.set_volume(settings.volume);
                }
                *self.shared_settings.write().unwrap() = settings.clone();
                self.saved.settings = settings;
            }
            Command::Shutdown => {
                self.cancel();
                return Ok(true);
            }
        }
        Ok(false)
    }
    async fn save(&mut self) -> Result<()> {
        let paths = self.paths.clone();
        let saved = self.saved.clone();
        tokio::task::spawn_blocking(move || paths.save(&saved)).await??;
        self.last_save = Instant::now();
        Ok(())
    }
    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Message>) {
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                Some(message) = rx.recv() => {
                    match message {
                        Message::Command(command, reply) => {
                            let result = self.apply(command);
                            let quit = matches!(result, Ok(true));
                            self.publish();
                            let result = match result { Ok(_) => self.save().await, Err(e) => Err(e) };
                            let _ = reply.send(result);
                            if quit { break; }
                        }
                        Message::Resolved(generation, result) if generation == self.generation => {
                            self.task.take();
                            let cache = self.saved.queue.current().filter(|t| t.source.is_none() && t.duration_ms > 30000)
                                .map(|t| (self.api.music_cache_path(t.id, &self.saved.settings.quality), t.duration_ms));
                            match result.and_then(|uri| Engine::new(&uri, self.saved.settings.volume, generation, self.tx.clone(), cache)) {
                                Ok(engine) => {
                                    if self.desired_playing { engine.play.play(); } else { engine.play.pause(); }
                                    self.engine = Some(engine);
                                }
                                Err(error) => self.fail(error),
                            }
                            self.publish();
                        }
                        Message::Refilled(generation, result) if generation == self.generation => {
                            self.task.take();
                            match result {
                                Ok(tracks) => {
                                    // A recommendation batch replaces the exhausted one. This
                                    // bounds memory and permits songs to reappear in later batches.
                                    if !tracks.is_empty() {
                                        self.saved.queue.replace(tracks, 0); self.queue_changed();
                                        if let Err(e) = self.load(false) { self.fail(e); }
                                    } else { self.cancel(); self.error = Some("没有更多推荐歌曲".into()); self.status = Playback::Error; self.desired_playing = false; }
                                }
                                Err(e) => { self.cancel(); self.error = Some(e.to_string()); self.status = Playback::Error; self.desired_playing = false; }
                            }
                            self.publish();
                        }
                        Message::Engine(generation, event) if generation == self.generation => {
                            match event {
                                EngineEvent::End => { self.failures = 0; if let Err(e) = self.next(true) { self.fail(e); } }
                                EngineEvent::Error(error) => self.fail(anyhow::anyhow!(error)),
                                EngineEvent::Seeked(position) => self.saved.position_ms = position,
                                EngineEvent::State(state) => {
                                    self.status = match state { PlayState::Playing => Playback::Playing, PlayState::Paused => Playback::Paused,
                                        PlayState::Buffering => Playback::Loading, _ => self.status };
                                    if matches!(state, PlayState::Playing | PlayState::Paused) {
                                        if let Some(pos) = self.pending_seek.take() { if let Some(e) = &self.engine { e.play.seek(ClockTime::from_mseconds(pos)); } }
                                    }
                                }
                            }
                            self.publish();
                        }
                        _ => {} // Late URL results and old pipeline events are discarded.
                    }
                }
                _ = tick.tick() => {
                    if let Some(e) = &self.engine {
                        let position = e.play.position().map(|t| t.mseconds()).unwrap_or(self.saved.position_ms);
                        let duration = e.play.duration().map(|t| t.mseconds()).unwrap_or(self.duration_ms);
                        if position != self.saved.position_ms || duration != self.duration_ms {
                            self.saved.position_ms = position; self.duration_ms = duration; self.publish();
                        }
                    }
                    if self.last_save.elapsed() > Duration::from_secs(5) && self.status == Playback::Playing {
                        if let Err(e) = self.save().await { log::warn!("保存播放进度失败: {e}"); }
                    }
                }
            }
        }
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Track {
    pub id: u64,
    pub name: String,
    pub artist: String,
    pub album: String,
    pub album_id: u64,
    pub cover: String,
    pub duration_ms: u64,
    pub playable: bool,
    /// Only local files carry a source; remote URLs are resolved by the core.
    pub source: Option<String>,
}

impl From<ncm_api::SongInfo> for Track {
    fn from(song: ncm_api::SongInfo) -> Self {
        Self {
            id: song.id,
            name: song.name,
            artist: song.singer,
            album: song.album,
            album_id: song.album_id,
            cover: song.pic_url,
            duration_ms: song.duration,
            playable: song.copyright.playable(),
            source: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: u64,
    pub name: String,
    pub subtitle: String,
    pub cover: String,
    pub kind: String,
}

impl Card {
    pub fn from_list(value: ncm_api::SongList, kind: &str) -> Self {
        Self {
            id: value.id,
            name: value.name,
            subtitle: value.author,
            cover: value.cover_img_url,
            kind: kind.into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Page {
    pub title: String,
    pub subtitle: String,
    pub cover: String,
    pub tracks: Vec<Track>,
    pub cards: Vec<Card>,
    pub more: bool,
    pub subscribed: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Playback {
    #[default]
    Stopped,
    Loading,
    Playing,
    Paused,
    Error,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Continuous {
    #[default]
    None,
    Fm,
    Heartbeat {
        playlist_id: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub quality: String,
    pub volume: f64,
    pub mute_start: bool,
    pub proxy: String,
    pub cache_days: u32,
    pub try_unavailable: bool,
    pub desktop_lyrics: bool,
    pub reduced_motion: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            quality: "standard".into(),
            volume: 0.8,
            mute_start: false,
            proxy: String::new(),
            cache_days: 0,
            try_unavailable: false,
            desktop_lyrics: false,
            reduced_motion: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub revision: u64,
    pub queue_revision: u64,
    pub queue: std::sync::Arc<Vec<Track>>,
    pub current_id: Option<u64>,
    pub playback: Playback,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub volume: f64,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub continuous: Continuous,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    Replace { tracks: Vec<Track>, index: usize },
    Append { tracks: Vec<Track> },
    Select { id: u64 },
    Remove { id: u64 },
    Clear,
    Play,
    Pause,
    Toggle,
    Stop,
    Next,
    Previous,
    Seek { position_ms: u64 },
    Volume { value: f64 },
    Mode { repeat: Repeat, shuffle: bool },
    Continuous { mode: Continuous },
    Settings { settings: Settings },
    Shutdown,
}

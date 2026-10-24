use crate::{queue::Queue, Continuous, Repeat, Settings, Track};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub legacy_config: PathBuf,
    pub legacy_data: PathBuf,
    pub isolated: bool,
}

impl Paths {
    pub fn discover() -> Result<Self> {
        if let Some(root) = std::env::var_os("NCM_DATA_DIR") {
            return Self::isolated(root.into());
        }
        let config = dirs::config_dir().context("找不到配置目录")?;
        let data = dirs::data_local_dir().context("找不到数据目录")?;
        let cache = dirs::cache_dir().context("找不到缓存目录")?;
        let paths = Self {
            config: config.join("netease-cloud-music-desktop"),
            data: data.join("netease-cloud-music-desktop"),
            cache: cache.join("netease-cloud-music-gtk4"),
            legacy_config: config.join("netease-cloud-music-gtk4"),
            legacy_data: data.join("netease-cloud-music-gtk4"),
            isolated: false,
        };
        paths.init()?;
        Ok(paths)
    }
    pub fn isolated(root: PathBuf) -> Result<Self> {
        let paths = Self {
            config: root.join("config"),
            data: root.join("data"),
            cache: root.join("cache"),
            legacy_config: root.join("legacy-config"),
            legacy_data: root.join("legacy-data"),
            isolated: true,
        };
        paths.init()?;
        Ok(paths)
    }
    fn init(&self) -> Result<()> {
        for p in [&self.config, &self.data, &self.cache] {
            fs::create_dir_all(p)?;
        }
        Ok(())
    }
    pub fn load(&self) -> Result<Saved> {
        let file = self.config.join("state.json");
        if file.exists() {
            let mut saved: Saved =
                serde_json::from_slice(&fs::read(&file)?).context("播放状态文件损坏")?;
            anyhow::ensure!(saved.version == 1, "不支持的播放状态版本 {}", saved.version);
            saved.queue.normalize();
            return Ok(saved);
        }
        let mut saved = Saved::default();
        let legacy = self.legacy_config.join("playlist.json");
        if legacy.exists() {
            let old: serde_json::Value = serde_json::from_slice(&fs::read(&legacy)?)?;
            let tracks = old["list"]
                .as_array()
                .map(|songs| {
                    songs
                        .iter()
                        .filter_map(|song| {
                            Some(Track {
                                id: song["id"].as_u64()?,
                                name: song["name"].as_str().unwrap_or_default().into(),
                                artist: song["singer"].as_str().unwrap_or_default().into(),
                                album: song["album"].as_str().unwrap_or_default().into(),
                                album_id: song["album_id"].as_u64().unwrap_or(0),
                                cover: song["pic_url"].as_str().unwrap_or_default().into(),
                                duration_ms: song["duration"].as_u64().unwrap_or(0),
                                playable: true,
                                source: None,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            saved
                .queue
                .replace(tracks, old["position"].as_u64().unwrap_or(0) as usize);
            if let Some(id) = old["current_song_id"].as_u64() {
                saved.queue.select(id);
            }
            let mode = old["loops"].as_str().unwrap_or("none");
            saved.queue.mode(
                match mode {
                    "one" => Repeat::One,
                    "loop" | "shuffle" => Repeat::All,
                    _ => Repeat::Off,
                },
                mode == "shuffle",
            );
            saved.position_ms = old["play_position"].as_u64().unwrap_or(0) / 1000;
        }
        // Read the old schema only when installed; never change the GTK configuration.
        if !self.isolated {
            saved.settings = legacy_settings();
        }
        self.save(&saved)?;
        Ok(saved)
    }
    pub fn save(&self, state: &Saved) -> Result<()> {
        atomic_write(
            &self.config.join("state.json"),
            &serde_json::to_vec_pretty(state)?,
        )
    }
    pub fn cookie_path(&self) -> PathBuf {
        self.data.join("cookies.json")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub version: u32,
    pub queue: Queue,
    pub position_ms: u64,
    pub settings: Settings,
    pub continuous: Continuous,
}
impl Default for Saved {
    fn default() -> Self {
        Self {
            version: 1,
            queue: Queue::default(),
            position_ms: 0,
            settings: Settings::default(),
            continuous: Continuous::None,
        }
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temp = path.with_extension(format!("{}.{sequence}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> Result<()> {
        let mut file = options.open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

#[cfg(not(target_os = "linux"))]
fn legacy_settings() -> Settings {
    Settings::default()
}

#[cfg(target_os = "linux")]
fn legacy_settings() -> Settings {
    fn get(key: &str) -> Option<String> {
        let out = std::process::Command::new("gsettings")
            .args(["get", "com.gitee.gmg137.NeteaseCloudMusicGtk4", key])
            .output()
            .ok()?;
        out.status.success().then(|| {
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .trim_matches('\'')
                .to_owned()
        })
    }
    let mut s = Settings::default();
    if let Some(v) = get("style-variant") {
        s.theme = v;
    }
    if let Some(v) = get("proxy-address") {
        s.proxy = v;
    }
    if let Some(v) = get("volume").and_then(|v| v.parse::<f64>().ok()) {
        s.volume = v.clamp(0.0, 1.0);
    }
    s.mute_start = get("mute-start").as_deref() == Some("true");
    s.desktop_lyrics = get("desktop-lyrics").as_deref() == Some("true");
    s.try_unavailable = get("not-ignore-grey").as_deref() == Some("true");
    let index = get("music-rate")
        .and_then(|v| v.split_whitespace().last()?.parse::<usize>().ok())
        .unwrap_or(0);
    s.quality = [
        "standard", "higher", "exhigh", "lossless", "hires", "jyeffect", "sky", "jymaster",
    ]
    .get(index)
    .unwrap_or(&"standard")
    .to_string();
    s.cache_days =
        match get("cache-clear").and_then(|v| v.split_whitespace().last()?.parse::<u32>().ok()) {
            Some(1) => 1,
            Some(2) => 7,
            Some(3) => 30,
            _ => 0,
        };
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_migration_is_idempotent_and_clears_remote_urls() {
        let temp = tempfile::tempdir().unwrap();
        let p = Paths::isolated(temp.path().into()).unwrap();
        fs::create_dir_all(&p.legacy_config).unwrap();
        fs::write(p.legacy_config.join("playlist.json"), r#"{"list":[{"id":7,"name":"old","song_url":"expired"}],"current_song_id":7,"position":99,"loops":"shuffle","play_position":12500000}"#).unwrap();
        let first = p.load().unwrap();
        let second = p.load().unwrap();
        assert_eq!(first.queue.current, Some(7));
        assert_eq!(second.position_ms, 12500);
        assert!(second.queue.shuffled);
        assert!(second.queue.current().unwrap().source.is_none());
        assert!(p.legacy_config.join("playlist.json").exists());
    }
    #[test]
    fn corrupt_state_is_reported_not_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let p = Paths::isolated(temp.path().into()).unwrap();
        fs::write(p.config.join("state.json"), b"broken").unwrap();
        assert!(p.load().is_err());
        assert_eq!(fs::read(p.config.join("state.json")).unwrap(), b"broken");
    }
    #[test]
    fn concurrent_writes_are_complete_and_leave_no_temporary_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        std::thread::scope(|scope| {
            for i in 0..12 {
                let path = &path;
                scope.spawn(move || atomic_write(path, &vec![i; 10000]).unwrap());
            }
        });
        let bytes = fs::read(path).unwrap();
        assert_eq!(bytes.len(), 10000);
        assert!(bytes.iter().all(|value| *value == bytes[0]));
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
}

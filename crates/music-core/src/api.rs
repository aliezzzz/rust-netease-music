// Session compatibility adapted from src/ncmapi.rs, Copyright (C) 2022 gmg137.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    lyrics::{self, LyricLine},
    persistence::atomic_write,
    Card, Page, Paths, Track,
};
use anyhow::{Context, Result};
use ncm_api::{CookieBuilder, CookieJar, MusicApi};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    sync::{Arc, RwLock},
};

#[derive(Clone)]
pub struct Api {
    client: Arc<RwLock<MusicApi>>,
    proxy: Arc<RwLock<String>>,
    pub paths: Paths,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_session_roundtrip_keeps_scoped_credentials_without_duplicates() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths::isolated(temp.path().into()).unwrap();
        fs::create_dir_all(&paths.legacy_data).unwrap();
        let mut store = cookie_store::CookieStore::default();
        let url = "https://music.163.com/weapi/".parse().unwrap();
        for cookie in [
            "MUSIC_U=test-token; Domain=music.163.com; Path=/; Max-Age=3600",
            "__csrf=test-csrf; Domain=music.163.com; Path=/weapi/; Max-Age=3600",
        ] {
            store.parse(cookie, &url).unwrap();
        }
        let mut bytes = Vec::new();
        cookie_store::serde::json::save(&store, &mut bytes).unwrap();
        fs::write(paths.legacy_data.join("cookies.json"), &bytes).unwrap();
        Api::new(paths.clone(), "").unwrap().save_session().unwrap();
        let restored = Api::new(paths.clone(), "").unwrap();
        let client = restored.client();
        let jar = client.cookie_jar().unwrap();
        let cookies: Vec<_> = jar
            .get_for_uri(&"https://music.163.com/weapi/".parse().unwrap())
            .into_iter()
            .collect();
        assert_eq!(cookies.iter().filter(|c| c.name() == "MUSIC_U").count(), 1);
        assert_eq!(
            cookies
                .iter()
                .filter(|c| c.name() == "__csrf" && c.value() == "test-csrf")
                .count(),
            1
        );
        assert!(!jar
            .get_for_uri(&"https://music.163.com/".parse().unwrap())
            .into_iter()
            .any(|c| c.name() == "__csrf"));
        // An empty new session is authoritative, even if the legacy login remains.
        atomic_write(&paths.cookie_path(), b"[]").unwrap();
        assert!(!Api::new(paths.clone(), "")
            .unwrap()
            .client()
            .cookie_jar()
            .unwrap()
            .get_for_uri(&"https://music.163.com/".parse().unwrap())
            .into_iter()
            .any(|c| c.name() == "MUSIC_U"));
        assert_eq!(
            fs::read(paths.legacy_data.join("cookies.json")).unwrap(),
            bytes
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub kind: String,
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub offset: u16,
    #[serde(default = "page_size")]
    pub limit: u16,
}
fn page_size() -> u16 {
    50
}

pub fn qr_svg(url: &str) -> Result<String> {
    Ok(qrcode::QrCode::new(url)?
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(220, 220)
        .build())
}

// The public search response often contains an artwork ID but omits picUrl.
// This is NetEase's public CDN image naming scheme, also used by its web UI.
fn artwork_url(id: u64) -> String {
    use base64::Engine;
    let key = b"3go8&$8*3*3h0k(2)2";
    let bytes: Vec<_> = id
        .to_string()
        .bytes()
        .enumerate()
        .map(|(i, byte)| byte ^ key[i % key.len()])
        .collect();
    let hash = base64::engine::general_purpose::URL_SAFE.encode(md5::compute(bytes).0);
    format!("https://p3.music.126.net/{hash}/{id}.jpg")
}

impl Api {
    pub fn new(paths: Paths, proxy: &str) -> Result<Self> {
        if !paths.cookie_path().exists() && paths.legacy_data.join("cookies.json").exists() {
            atomic_write(
                &paths.cookie_path(),
                &fs::read(paths.legacy_data.join("cookies.json"))?,
            )?;
        }
        let jar = CookieJar::default();
        if paths.cookie_path().exists() {
            let file = fs::File::open(paths.cookie_path())?;
            match cookie_store::serde::json::load(std::io::BufReader::new(file)) {
                Ok(store) => {
                    for cookie in store.iter_unexpired() {
                        let value = CookieBuilder::new(cookie.name(), cookie.value())
                            .domain("music.163.com")
                            .path(cookie.path().unwrap_or("/"))
                            .build();
                        if let Ok(c) = value {
                            let _ = jar.set(c, &"https://music.163.com/".parse()?);
                        }
                    }
                }
                Err(error) => log::warn!("无法读取旧登录会话: {error}"),
            }
        }
        let mut client = MusicApi::from_cookie_jar(jar, 12);
        if !proxy.is_empty() {
            client.set_proxy(proxy)?;
        }
        Ok(Self {
            client: Arc::new(RwLock::new(client)),
            proxy: Arc::new(RwLock::new(proxy.to_owned())),
            paths,
        })
    }
    pub fn client(&self) -> MusicApi {
        self.client.read().unwrap().clone()
    }
    pub fn set_proxy(&self, proxy: &str) -> Result<()> {
        let old = self.client();
        let mut client = old
            .cookie_jar()
            .map(|j| MusicApi::from_cookie_jar(j.clone(), 12))
            .unwrap_or_default();
        if !proxy.is_empty() {
            client.set_proxy(proxy)?;
        }
        *self.client.write().unwrap() = client;
        *self.proxy.write().unwrap() = proxy.to_owned();
        Ok(())
    }
    pub fn save_session(&self) -> Result<()> {
        let mut store = cookie_store::CookieStore::default();
        let mut seen = std::collections::HashSet::new();
        if let Some(jar) = self.client().cookie_jar() {
            for path in ["/", "/weapi/", "/eapi/", "/api/"] {
                let address = format!("https://music.163.com{path}");
                let uri = address.parse()?;
                let url = address.parse()?;
                for cookie in jar.get_for_uri(&uri) {
                    // isahc exposes matching cookies, not their private path metadata.
                    // Keep the broadest matching path and avoid four duplicate auth cookies.
                    if !seen.insert((cookie.name().to_owned(), cookie.value().to_owned())) {
                        continue;
                    }
                    let value = format!(
                        "{}={}; Path={path}; Domain=music.163.com; Max-Age=31536000",
                        cookie.name(),
                        cookie.value()
                    );
                    let _ = store.parse(&value, &url);
                }
            }
        }
        let mut bytes = Vec::new();
        cookie_store::serde::json::save(&store, &mut bytes)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        atomic_write(&self.paths.cookie_path(), &bytes)
    }
    pub async fn logout(&self) -> Result<()> {
        self.client().logout().await;
        let proxy = self.proxy.read().unwrap().clone();
        *self.client.write().unwrap() = MusicApi::new(12);
        self.set_proxy(&proxy)?;
        if self.paths.cookie_path().exists() {
            fs::remove_file(self.paths.cookie_path())?;
        }
        // A marker prevents re-importing the old account on the next startup.
        self.save_session()
    }
    pub async fn user_id(&self) -> Result<u64> {
        let user = self
            .client()
            .login_status()
            .await
            .context("请先登录网易云音乐")?;
        anyhow::ensure!(user.uid > 0, "请先登录网易云音乐");
        Ok(user.uid)
    }
    pub async fn favorite_id(&self) -> Result<u64> {
        let uid = self.user_id().await?;
        self.client()
            .user_song_list(uid, 0, 100)
            .await?
            .into_iter()
            .find(|p| p.special_type == 5)
            .map(|p| p.id)
            .context("未找到喜欢的音乐歌单")
    }
    pub async fn query(&self, q: Query) -> Result<Page> {
        tokio::time::timeout(std::time::Duration::from_secs(30), self.query_inner(q))
            .await
            .context("请求超时，请稍后重试")?
    }
    async fn query_inner(&self, q: Query) -> Result<Page> {
        let c = self.client();
        let limit = q.limit.clamp(1, 100);
        let mut page = Page::default();
        match q.kind.as_str() {
            "discover" | "playlists" => {
                page.title = "发现音乐".into();
                page.cards = c
                    .top_song_list("全部", "hot", q.offset, limit)
                    .await?
                    .into_iter()
                    .map(|p| Card::from_list(p, "playlist"))
                    .collect();
            }
            "albums" => {
                page.title = "新碟上架".into();
                page.cards = c
                    .new_albums("ALL", q.offset, limit)
                    .await?
                    .into_iter()
                    .map(|p| Card::from_list(p, "album"))
                    .collect();
            }
            "charts" => {
                page.title = "排行榜".into();
                page.cards = c
                    .toplist()
                    .await?
                    .into_iter()
                    .map(|p| Card {
                        id: p.id,
                        name: p.name,
                        subtitle: p.update,
                        cover: p.cover,
                        kind: "playlist".into(),
                    })
                    .collect();
            }
            "playlist" | "favorites" => {
                let id = if q.kind == "favorites" {
                    self.favorite_id().await?
                } else {
                    q.id
                };
                let p = c.song_list_detail(id).await?;
                page.title = p.name;
                page.subtitle = p.description;
                page.cover = p.cover_img_url;
                page.tracks = p.songs.into_iter().map(Into::into).collect();
                page.subscribed = c
                    .songlist_detail_dynamic(id)
                    .await
                    .ok()
                    .map(|s| s.subscribed);
            }
            "album" => {
                let p = c.album(q.id).await?;
                page.title = p.name;
                page.subtitle = p.description;
                page.cover = p.pic_url;
                page.tracks = p.songs.into_iter().map(Into::into).collect();
                page.subscribed = c.album_detail_dynamic(q.id).await.ok().map(|s| s.is_sub);
            }
            "artist" => {
                page.title = q.text;
                page.tracks = c
                    .singer_songs(q.id)
                    .await?
                    .into_iter()
                    .map(Into::into)
                    .collect();
            }
            "radio" => {
                page.title = q.text;
                page.tracks = c
                    .radio_program(q.id, q.offset, limit)
                    .await?
                    .into_iter()
                    .map(Into::into)
                    .collect();
            }
            "daily" => {
                page.title = "每日推荐".into();
                page.tracks = c
                    .recommend_songs()
                    .await?
                    .into_iter()
                    .map(Into::into)
                    .collect();
            }
            "cloud" => {
                page.title = "音乐云盘".into();
                page.tracks = c
                    .user_cloud_disk()
                    .await?
                    .into_iter()
                    .map(Into::into)
                    .collect();
            }
            "library" | "saved_playlists" => {
                page.title = "我的歌单".into();
                page.cards = c
                    .user_song_list(self.user_id().await?, q.offset, limit)
                    .await?
                    .into_iter()
                    .map(|p| Card::from_list(p, "playlist"))
                    .collect();
            }
            "recommended" => {
                page.title = "为你推荐的歌单".into();
                page.cards = c
                    .recommend_resource()
                    .await?
                    .into_iter()
                    .map(|p| Card::from_list(p, "playlist"))
                    .collect();
            }
            "saved_albums" => {
                page.title = "收藏的专辑".into();
                page.cards = c
                    .album_sublist(q.offset, limit)
                    .await?
                    .into_iter()
                    .map(|p| Card::from_list(p, "album"))
                    .collect();
            }
            "radios" => {
                page.title = "订阅电台".into();
                page.cards = c
                    .user_radio_sublist(q.offset, limit)
                    .await?
                    .into_iter()
                    .map(|p| Card::from_list(p, "radio"))
                    .collect();
            }
            "search_song" | "search_lyrics" => {
                page.title = format!("“{}”的搜索结果", q.text);
                let raw = c
                    .search(
                        q.text,
                        if q.kind == "search_song" { 1 } else { 1006 },
                        q.offset,
                        limit,
                    )
                    .await?;
                let metadata: serde_json::Value = serde_json::from_str(&raw)?;
                let songs = ncm_api::to_song_info(raw, ncm_api::Parse::Search)?;
                page.tracks = songs.into_iter().map(Into::into).collect();
                if let Some(songs) = metadata["result"]["songs"].as_array() {
                    for (track, song) in page.tracks.iter_mut().zip(songs) {
                        if track.cover.is_empty() {
                            if let Some(id) = song["album"]["picId"].as_u64().filter(|id| *id > 0) {
                                track.cover = artwork_url(id);
                            }
                        }
                    }
                }
            }
            "search_album" | "search_playlist" => {
                let lists = if q.kind == "search_album" {
                    c.search_album(q.text, q.offset, limit).await?
                } else {
                    c.search_songlist(q.text, q.offset, limit).await?
                };
                page.cards = lists
                    .into_iter()
                    .map(|p| Card::from_list(p, q.kind.trim_start_matches("search_")))
                    .collect();
            }
            "search_artist" => {
                page.cards = c
                    .search_singer(q.text, q.offset, limit)
                    .await?
                    .into_iter()
                    .map(|p| Card {
                        id: p.id,
                        name: p.name,
                        subtitle: "歌手".into(),
                        cover: p.pic_url,
                        kind: "artist".into(),
                    })
                    .collect();
            }
            _ => anyhow::bail!("未知页面类型: {}", q.kind),
        }
        page.more = (page.cards.len().max(page.tracks.len()) == limit as usize)
            && !matches!(
                q.kind.as_str(),
                "playlist"
                    | "album"
                    | "favorites"
                    | "charts"
                    | "daily"
                    | "cloud"
                    | "artist"
                    | "recommended"
            );
        Ok(page)
    }
    pub async fn lyrics(&self, id: u64) -> Result<Vec<LyricLine>> {
        let path = self.paths.cache.join(format!("lyrics-{id}.json"));
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(lines) = serde_json::from_slice(&bytes) {
                return Ok(lines);
            }
        }
        let raw = self.client().song_lyric(id).await?;
        let lines = lyrics::merge(&raw.lyric, &raw.tlyric);
        atomic_write(&path, &serde_json::to_vec(&lines)?)?;
        Ok(lines)
    }
    pub async fn resolve(&self, track: &Track, quality: &str) -> Result<String> {
        if let Some(source) = &track.source {
            anyhow::ensure!(source.starts_with("file://"), "本地音源必须使用 file URL");
            return Ok(source.clone());
        }
        let cached = self.music_cache_path(track.id, quality);
        if cached.is_file() && fs::metadata(&cached)?.len() > 0 {
            return url::Url::from_file_path(cached)
                .map(String::from)
                .map_err(|_| anyhow::anyhow!("缓存路径无效"));
        }
        let urls = self
            .client()
            .songs_url_v1(
                &[track.id],
                ncm_api::SongQuality::from_level(quality).unwrap_or_default(),
            )
            .await?;
        let source = urls
            .into_iter()
            .find(|s| s.id == track.id && !s.url.is_empty())
            .context("此歌曲暂时无法播放，请检查登录状态或音质设置")?;
        Ok(source.url)
    }

    pub fn music_cache_path(&self, id: u64, quality: &str) -> std::path::PathBuf {
        let rate = match quality {
            "higher" => 192000,
            "exhigh" => 320000,
            "lossless" => 999000,
            "hires" => 1900000,
            "jyeffect" => 804505,
            "sky" => 2695684,
            "jymaster" => 4532511,
            _ => 128000,
        };
        self.paths.cache.join(format!("music_{id}_{rate}"))
    }

    pub async fn export_desktop_lyrics(&self, track: &Track) -> Result<()> {
        let lines = self.lyrics(track.id).await?;
        let folder = if self.paths.isolated {
            self.paths.data.join("lyrics")
        } else {
            dirs::home_dir().context("找不到用户目录")?.join(".lyrics")
        };
        let sanitize = |s: &str| s.replace(['/', '\\', '\0'], "＿");
        let name = format!(
            "{}-{}-{}.lrc",
            sanitize(&track.name),
            sanitize(&track.artist),
            sanitize(&track.album)
        );
        let mut text = String::new();
        for line in lines {
            let time = line.time_ms;
            text.push_str(&format!(
                "[{:02}:{:02}.{:03}]{}\n",
                time / 60000,
                time / 1000 % 60,
                time % 1000,
                line.text
            ));
        }
        tokio::task::spawn_blocking(move || {
            fs::create_dir_all(&folder)?;
            atomic_write(&folder.join(name), text.as_bytes())
        })
        .await??;
        Ok(())
    }

    pub async fn cleanup_cache(&self, days: u32) -> Result<()> {
        if days == 0 {
            return Ok(());
        }
        let dir = self.paths.cache.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let age = std::time::Duration::from_secs(days as u64 * 86400);
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if !(name.starts_with("music_")
                    || name.starts_with("lyrics-")
                    || name.ends_with(".jpg"))
                {
                    continue;
                }
                let metadata = entry.metadata()?;
                if metadata.is_file() && metadata.modified()?.elapsed().unwrap_or_default() > age {
                    fs::remove_file(entry.path())?;
                }
            }
            Ok(())
        })
        .await?
    }
}

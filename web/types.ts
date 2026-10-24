export interface Track {
  id: number; name: string; artist: string; album: string; album_id: number;
  cover: string; duration_ms: number; playable: boolean; source: string | null;
}
export interface Card { id: number; name: string; subtitle: string; cover: string; kind: string }
export interface Page { title: string; subtitle: string; cover: string; tracks: Track[]; cards: Card[]; more: boolean; subscribed?: boolean | null }
export type Repeat = 'off' | 'all' | 'one';
export type Continuous = { kind: 'none' | 'fm' } | { kind: 'heartbeat'; playlist_id: number };
export interface Snapshot {
  revision: number; queue_revision: number; queue: Track[]; current_id: number | null;
  playback: 'stopped' | 'loading' | 'playing' | 'paused' | 'error';
  position_ms: number; duration_ms: number; volume: number;
  repeat: Repeat; shuffle: boolean; continuous: Continuous; error: string | null;
}
export interface Settings {
  theme: 'system' | 'light' | 'dark'; quality: string; volume: number; mute_start: boolean;
  proxy: string; cache_days: number; try_unavailable: boolean; desktop_lyrics: boolean; reduced_motion: boolean;
}
export interface Lyric { time_ms: number; text: string; translation: string }
export interface User { uid: number; nickname: string; avatar_url: string; code: number; msg: string }
export interface Query { kind: string; id?: number; text?: string; offset?: number; limit?: number }

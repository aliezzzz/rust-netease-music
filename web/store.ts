import { computed, reactive, shallowRef } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Snapshot, Settings, Track, User, Query, Page } from './types';
import { StateSync, type Update } from './sync';

export const initial: Snapshot = {
  revision: 0, queue_revision: 0, queue: [], current_id: null, playback: 'stopped',
  position_ms: 0, duration_ms: 0, volume: .8, repeat: 'off', shuffle: false, continuous: { kind: 'none' }, error: null,
};
export const playback = shallowRef<Snapshot>(initial);
export const preferences = reactive<Settings>({ theme: 'system', quality: 'standard', volume: .8,
  mute_start: false, proxy: '', cache_days: 0, try_unavailable: false, desktop_lyrics: false, reduced_motion: false });
export const ui = reactive({ queue: false, lyrics: false, login: false, message: '', connected: false, accountRevision: 0 });
export const user = shallowRef<User | null>(null);
export const likes = shallowRef(new Set<number>());
export const current = computed(() => playback.value.queue.find(t => t.id === playback.value.current_id));
let toastTimer: ReturnType<typeof setTimeout>;
export function notify(message: unknown) {
  ui.message = String(message); clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { ui.message = ''; }, 5000);
}
export async function call<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(name, args);
}
export async function command(value: Record<string, unknown>) {
  try { await call('player_command', { command: value }); } catch (e) { notify(e); }
}
export const query = (query: Query) => call<Page>('browse', { query });
export const play = (tracks: Track[], index = 0) => command({ type: 'replace', tracks, index });
export function formatTime(ms: number) {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}
let accountRequest = 0;
export async function refreshUser() {
  const token = ++accountRequest;
  const previousId = user.value?.uid;
  try {
    const value = await call<User>('account', { operation: 'status' });
    if (token !== accountRequest) return;
    user.value = value.uid ? value : null;
    if (user.value) {
      try { const ids = await call<number[]>('account', { operation: 'likes' }); if (token === accountRequest) likes.value = new Set(ids); } catch { if (token === accountRequest) likes.value = new Set(); }
    } else likes.value = new Set();
  } catch { if (token === accountRequest) { user.value = null; likes.value = new Set(); } }
  if (token === accountRequest && previousId !== user.value?.uid) ui.accountRevision++;
}
export async function like(track: Track) {
  if (!user.value) { ui.login = true; return; }
  const liked = !likes.value.has(track.id);
  try {
    await call('favorite', { kind: 'song', id: track.id, liked });
    const values = new Set(likes.value); liked ? values.add(track.id) : values.delete(track.id); likes.value = values;
  } catch (e) { notify(e); }
}
export async function initialize() {
  const sync = new StateSync(initial, state => {
    if (state.error && state.error !== playback.value.error) notify(state.error);
    playback.value = state;
    preferences.volume = state.volume;
  });
  let unlisten = () => {};
  try {
    unlisten = await listen<Update>('player-state', event => sync.event(event.payload));
    const [snapshot, settings] = await Promise.all([call<Snapshot>('player_snapshot'), call<Settings>('settings')]);
    sync.snapshot(snapshot); Object.assign(preferences, settings); ui.connected = true;
    void refreshUser();
    return unlisten;
  } catch (e) {
    unlisten();
    notify(`无法连接播放核心：${e}`);
    return () => {};
  }
}

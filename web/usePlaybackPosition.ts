import { onUnmounted, ref, watch, type Ref } from 'vue';
import { playback } from './store';
import { displayPosition } from './lyrics';

/** Line-timed lyrics need a display clock, not a whole Vue tree update at 144 Hz. */
export function usePlaybackPosition(enabled: Readonly<Ref<boolean>>) {
  const position = ref(playback.value.position_ms);
  let receivedAt = performance.now();
  let timer: ReturnType<typeof setTimeout> | undefined;
  function update() {
    if (!enabled.value || document.hidden) return;
    const state = playback.value;
    position.value = displayPosition(state.position_ms, receivedAt, performance.now(), state.playback === 'playing', state.duration_ms);
    if (state.playback === 'playing') timer = setTimeout(update, 50);
  }
  function restart() { clearTimeout(timer); update(); }
  // Backend snapshots/seek remain authoritative; transition suspension never
  // resets the time anchor, so reopening catches up instead of losing progress.
  watch(playback, () => { receivedAt = performance.now(); restart(); }, { immediate: true, flush: 'sync' });
  watch(enabled, restart);
  document.addEventListener('visibilitychange', restart);
  onUnmounted(() => { clearTimeout(timer); document.removeEventListener('visibilitychange', restart); });
  return position;
}

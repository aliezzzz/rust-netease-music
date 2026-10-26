<script setup lang="ts">
import { ref, watch, computed, onMounted, onBeforeUnmount, onUnmounted, nextTick } from 'vue';
import { ChevronDown, Mic2, Heart, ListMusic, X } from 'lucide-vue-next';
import { current, playback, ui, call, command, preferences, likes, like } from '../store';
import type { Lyric } from '../types';
import { normalizeLyrics } from '../lyrics';
import { usePlaybackPosition } from '../usePlaybackPosition';
import Cover from './Cover.vue';
import QualityPicker from './QualityPicker.vue';
import TransportControls from './TransportControls.vue';
import VolumeControl from './VolumeControl.vue';
import QueuePanel from './QueuePanel.vue';
import AlbumBackground from './AlbumBackground.vue';
import LyricStage from './LyricStage.vue';
import WindowControls from './WindowControls.vue';
const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ closed: [] }>();
const lines = ref<Lyric[]>([]);
const status = ref('');
const dialog = ref<HTMLDialogElement>();
const closeButton = ref<HTMLButtonElement>();
const queueButton = ref<HTMLButtonElement>();
const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
const systemReduced = ref(motionQuery.matches);
const hidden = ref(document.hidden);
const reduced = computed(() => preferences.reduced_motion || systemReduced.value);
const playing = computed(() => playback.value.playback === 'playing');
const phase = ref<'opening' | 'open' | 'closing'>('opening');
const suspended = computed(() => phase.value !== 'open');
const position = usePlaybackPosition(computed(() => !suspended.value));
const contentReady = ref(false);
let mounted = false;
let disposed = false;
let transitionId = 0;
let sheetAnimation: Animation | undefined;

function cancelTransition() {
  transitionId++;
  sheetAnimation?.cancel();
  sheetAnimation = undefined;
}
function finishTransition(open: boolean) {
  phase.value = open ? 'open' : 'closing';
  if (open) contentReady.value = true;
  if (!open) {
    dialog.value?.close();
    emit('closed');
  }
}
function transition(open: boolean, fromStart = false) {
  const element = dialog.value;
  if (!element || disposed) return;
  // Read the visual position before cancelling: reversing a half-open sheet
  // must continue from that point, not jump to either endpoint.
  const style = getComputedStyle(element);
  const from = fromStart
    ? { transform: 'translate3d(0, 85vh, 0)', opacity: 0 }
    : { transform: style.transform, opacity: style.opacity };
  cancelTransition();
  const id = transitionId;
  phase.value = open ? 'opening' : 'closing';
  if (reduced.value || document.hidden) { finishTransition(open); return; }
  const to = open
    ? { transform: 'translate3d(0, 0, 0)', opacity: 1 }
    : { transform: 'translate3d(0, 85vh, 0)', opacity: 0 };
  try {
    sheetAnimation = element.animate([from, to], {
      duration: open ? 440 : 320,
      easing: open ? 'cubic-bezier(.16, 1, .3, 1)' : 'cubic-bezier(.4, 0, .8, .4)',
      fill: 'both',
    });
    void sheetAnimation.finished.then(() => {
      if (disposed || id !== transitionId) return;
      // Close the native dialog before dropping its held final frame.
      finishTransition(open);
      sheetAnimation?.cancel();
      sheetAnimation = undefined;
    }).catch(() => { /* Reversal/unmount intentionally cancels the old animation. */ });
  } catch {
    // Older WebKit without WAAPI must still allow the modal to close.
    finishTransition(open);
  }
}
watch(() => props.open, open => { if (mounted) transition(open); });
watch(reduced, reduce => {
  if (reduce && sheetAnimation) { cancelTransition(); finishTransition(props.open); }
});

let request = 0;
watch(() => [current.value?.id, contentReady.value] as const, async ([id, ready]) => {
  const token = ++request;
  lines.value = [];
  if (!ready) { status.value = '正在加载歌词…'; return; }
  if (!id) { status.value = '选择一首歌曲，跟着旋律一起唱。'; return; }
  status.value = '正在加载歌词…';
  try {
    const result = await call<Lyric[]>('lyrics', { id });
    if (token === request) { lines.value = normalizeLyrics(result); status.value = lines.value.length ? '' : '此歌曲暂无歌词，静静听一会儿。'; }
  } catch { if (token === request) status.value = '暂时无法获取歌词，音乐仍在继续。'; }
}, { immediate: true });
function close() { ui.lyrics = false; }
function escape(event: KeyboardEvent) {
  event.preventDefault(); event.stopPropagation();
  if (ui.queue) ui.queue = false;
  else close();
}
function updateMotion() { systemReduced.value = motionQuery.matches; }
function updateVisibility() {
  hidden.value = document.hidden;
  if (hidden.value && sheetAnimation) { cancelTransition(); finishTransition(props.open); }
}
watch(() => ui.queue, async open => {
  await nextTick();
  if (disposed || !props.open) return;
  if (open) dialog.value?.querySelector<HTMLButtonElement>('[aria-label="关闭队列"]')?.focus();
  else queueButton.value?.focus();
});
onMounted(() => {
  mounted = true;
  motionQuery.addEventListener('change', updateMotion);
  document.addEventListener('visibilitychange', updateVisibility);
  // Async component loading can finish after an immediate Escape/Ctrl+F.
  if (!props.open) { finishTransition(false); return; }
  dialog.value?.showModal();
  closeButton.value?.focus();
  transition(true, true);
});
onBeforeUnmount(() => {
  disposed = true;
  cancelTransition();
  dialog.value?.close();
});
onUnmounted(() => {
  request++;
  motionQuery.removeEventListener('change', updateMotion);
  document.removeEventListener('visibilitychange', updateVisibility);
});
</script>
<template>
  <dialog ref="dialog" class="lyrics-view" :class="{ 'is-playing': playing, 'reduced-motion': reduced }" :data-phase="phase" aria-label="正在播放" @cancel.prevent="close" @keydown.esc="escape">
    <AlbumBackground :cover="current?.cover" :playing="playing" :reduced="reduced" :hidden="hidden" :suspended="suspended" />
    <header class="lyrics-header" data-tauri-drag-region>
      <button ref="closeButton" class="icon-button lyrics-close" aria-label="关闭歌词" @click="close"><ChevronDown /></button>
      <span class="lyrics-header-label" data-tauri-drag-region="deep">正在播放 <span>NOW PLAYING</span></span>
      <button ref="queueButton" class="icon-button" :class="{ active: ui.queue }" aria-label="播放队列" :aria-pressed="ui.queue" @click="ui.queue = !ui.queue"><ListMusic /></button>
      <WindowControls />
    </header>
    <div class="lyrics-layout">
      <section class="lyrics-song" aria-label="歌曲与播放控制">
        <div class="album-sleeve"><Cover :src="current?.cover" :name="current?.name" :size="800" /></div>
        <div class="lyrics-metadata">
          <div><h2 :title="current?.name">{{ current?.name ?? '等待音乐响起' }}</h2><p :title="current?.artist">{{ current?.artist ?? '让音乐，陪你一会儿' }}</p><span :title="current?.album">{{ current?.album }}</span></div>
          <button class="icon-button" :class="{ active: current && likes.has(current.id) }" :disabled="!current" :aria-pressed="!!current && likes.has(current.id)" aria-label="喜欢当前歌曲" @click="current && like(current)"><Heart :fill="current && likes.has(current.id) ? 'currentColor' : 'none'" /></button>
        </div>
        <TransportControls :position="position" />
        <div class="lyrics-extras"><VolumeControl /><QualityPicker /></div>
      </section>
      <section class="lyrics-pane" aria-label="歌词">
        <div v-if="status" class="empty-state" role="status"><Mic2 /><p>{{ status }}</p></div>
        <LyricStage v-else :lines="lines" :time="position" :reduced="reduced" :hidden="hidden" :suspended="suspended" @seek="command({ type: 'seek', position_ms: $event })" />
      </section>
    </div>
    <div v-if="ui.queue" class="lyrics-queue-overlay" @click.self="ui.queue = false"><QueuePanel /></div>
    <div v-if="ui.message" class="toast" role="status">{{ ui.message }}<button class="icon-button" aria-label="关闭消息" @click="ui.message = ''"><X /></button></div>
  </dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import type { Lyric } from '../types';
import { activeLyricIndex } from '../lyrics';
import { advanceLyricSpring, lyricStagger, shouldAnimateLyricChange, type LyricSpring } from '../lyricMotion';
const props = defineProps<{ lines: Lyric[]; time: number; reduced: boolean; hidden: boolean; suspended: boolean }>();
const emit = defineEmits<{ seek: [time: number] }>();
const list = ref<HTMLElement>();
const simple = ref(false);
const following = ref(true);
const moving = ref(false);
const still = computed(() => props.reduced || simple.value);
const active = computed(() => activeLyricIndex(props.lines, props.time));
interface RowMotion extends LyricSpring { element: HTMLElement; delay: number }
const motions = new Map<number, RowMotion>();
let rows: HTMLElement[] = [];
let geometry: { top: number; height: number }[] = [];
let viewportWidth = 0, viewportHeight = 0, scrollHeight = 0;
let measureFrame = 0;
let frame = 0;
function updateRange(scroll: number, previous = scroll) {
  const top = Math.min(scroll, previous) - viewportHeight * .6;
  const bottom = Math.max(scroll, previous) + viewportHeight * 1.6;
  // Writes only; geometry comes from the resize/data measurement pass.
  rows.forEach((row, index) => {
    const item = geometry[index];
    if (!item) return;
    const hidden = item.top + item.height < top || item.top > bottom;
    // Keep culling separate from Vue's active/past class binding.
    if (hidden === (row.dataset.offscreen === 'true')) return;
    row.dataset.offscreen = String(hidden);
    if (hidden) row.style.height = `${item.height}px`;
    else row.style.removeProperty('height');
  });
}
function scrolled() {
  if (!motions.size && list.value) updateRange(list.value.scrollTop);
}
function scheduleMeasure() {
  cancelAnimationFrame(measureFrame);
  measureFrame = requestAnimationFrame(() => { if (!disposed) measure(); });
}
let observer: ResizeObserver | undefined;
let disposed = false;
function stop() {
  cancelAnimationFrame(frame);
  for (const row of motions.values()) { row.element.style.removeProperty('--lyric-y'); row.element.style.removeProperty('will-change'); }
  motions.clear();
  moving.value = false;
}
function browse() {
  // Preserve the active line's on-screen position before handing scroll back to
  // the user, rather than snapping to the spring's final target mid-gesture.
  const offset = motions.get(Math.max(0, active.value))?.position ?? 0;
  if (list.value) list.value.scrollTop -= offset;
  following.value = false;
  stop();
}
function follow(instant = false) {
  const container = list.value;
  const target = geometry[Math.max(0, active.value)];
  if (disposed || !following.value || props.hidden || props.suspended || !container || !target) return;
  const end = Math.max(0, Math.min(scrollHeight - viewportHeight,
    target.top - viewportHeight * .44 + target.height / 2));
  if (instant || still.value) { stop(); container.scrollTop = end; updateRange(container.scrollTop); return; }

  // FLIP-style positioning: move the native scroll to its final anchor, then
  // compensate each visible row with its own spring. This keeps real buttons,
  // variable-height text and ordinary touch/wheel scrolling intact.
  const start = container.scrollTop;
  const previous = new Map(motions);
  cancelAnimationFrame(frame);
  container.scrollTop = end;
  const finalScroll = container.scrollTop;
  const delta = finalScroll - start;
  updateRange(finalScroll, start);
  motions.clear();
  rows.forEach((element, index) => {
    const old = previous.get(index);
    const position = (old?.position ?? 0) + delta;
    const item = geometry[index];
    const top = item.top - finalScroll;
    const visibleAtEitherEnd = Math.max(top, top + position) + item.height > -100
      && Math.min(top, top + position) < viewportHeight + 100;
    if (visibleAtEitherEnd && (Math.abs(position) > .08 || Math.abs(old?.velocity ?? 0) > .8)) {
      motions.set(index, { element, position, velocity: old?.velocity ?? 0, delay: old ? 0 : lyricStagger(index, active.value) });
      element.style.setProperty('--lyric-y', `${position}px`);
      element.style.willChange = 'transform';
    } else { element.style.removeProperty('--lyric-y'); element.style.removeProperty('will-change'); }
  });
  moving.value = motions.size > 0;
  let lastTime = performance.now();
  function animate(now: number) {
    const elapsed = Math.max(0, now - lastTime);
    lastTime = now;
    for (const [index, row] of motions) {
      const step = Math.max(0, elapsed - row.delay);
      row.delay = Math.max(0, row.delay - elapsed);
      if (step === 0) continue;
      Object.assign(row, advanceLyricSpring(row, step));
      if (row.position === 0 && row.velocity === 0) {
        row.element.style.removeProperty('--lyric-y'); row.element.style.removeProperty('will-change');
        motions.delete(index);
      } else row.element.style.setProperty('--lyric-y', `${row.position.toFixed(3)}px`);
    }
    if (motions.size) frame = requestAnimationFrame(animate);
    else { moving.value = false; if (list.value) updateRange(list.value.scrollTop); }
  }
  if (motions.size) frame = requestAnimationFrame(animate);
}
async function resume() { following.value = true; await nextTick(); follow(true); }
function seek(index: number) {
  const line = props.lines[index];
  if (!line) return;
  stop();
  following.value = true;
  emit('seek', line.time_ms);
  void nextTick(() => follow(true));
}
function keyboard(event: KeyboardEvent) {
  if (event.target !== list.value) return;
  if (['ArrowUp', 'ArrowDown', 'Enter'].includes(event.key)) {
    event.preventDefault();
    seek(Math.max(0, Math.min(props.lines.length - 1, active.value + (event.key === 'ArrowUp' ? -1 : event.key === 'ArrowDown' ? 1 : 0))));
  } else if (['PageUp', 'PageDown', 'Home', 'End'].includes(event.key)) browse();
}
function measure() {
  const container = list.value;
  if (!container || disposed) return;
  stop();
  rows = Array.from(container.querySelectorAll<HTMLElement>('[data-line]'));
  // Unhide in one write pass, read in one pass, then cull. Never interleave
  // offset reads with per-row transform/filter writes in a natural cut.
  for (const row of rows) { delete row.dataset.offscreen; row.style.removeProperty('height'); }
  const origin = container.getBoundingClientRect().top - container.scrollTop;
  geometry = rows.map(row => {
    // Preserve fractional line-box heights: rounding every hidden placeholder
    // would accumulate into a visible anchor error on a long song.
    const rect = row.getBoundingClientRect();
    return { top: rect.top - origin, height: rect.height };
  });
  viewportWidth = container.clientWidth;
  viewportHeight = container.clientHeight;
  scrollHeight = container.scrollHeight;
  follow(true);
  updateRange(container.scrollTop);
}
watch(() => props.time, (time, previous) => {
  const before = activeLyricIndex(props.lines, previous);
  if (active.value !== before) follow(!shouldAnimateLyricChange(before, active.value, time - previous));
  // Small backward corrections from the backend clock are not user seeks;
  // restarting on each correction would cut every spring short.
  else if (Math.abs(time - previous) >= 250) follow(true);
}, { flush: 'post' });
watch(() => props.lines, async () => { stop(); following.value = true; await nextTick(); if (!disposed) measure(); });
watch(still, () => { stop(); follow(true); });
watch(() => [props.hidden, props.suspended], ([hidden, suspended]) => { if (hidden || suspended) stop(); else follow(true); });
onMounted(() => {
  observer = new ResizeObserver(() => {
    if (list.value && (list.value.clientWidth !== viewportWidth || list.value.clientHeight !== viewportHeight)) scheduleMeasure();
  });
  measure();
  if (list.value) observer.observe(list.value);
  document.fonts?.addEventListener('loadingdone', scheduleMeasure);
});
onUnmounted(() => {
  disposed = true; stop(); cancelAnimationFrame(measureFrame); observer?.disconnect();
  document.fonts?.removeEventListener('loadingdone', scheduleMeasure);
});
</script>
<template>
  <div class="lyrics-content" :class="{ 'simple-lyrics': still, 'browsing-lyrics': !following, 'lyrics-moving': moving }">
    <div ref="list" class="lyric-lines" tabindex="0" aria-label="同步歌词，使用上下方向键切换歌词，回车播放当前句"
      @keydown="keyboard" @scroll.passive="scrolled" @wheel.passive="browse" @touchstart.passive="browse">
      <button v-for="(line, i) in lines" :key="`${line.time_ms}-${i}`" :data-line="i"
        :class="{ active: i === active, past: i < active, distant: Math.abs(i - active) > 1 }"
        :aria-current="i === active ? 'true' : undefined" @click="seek(i)">
        <span class="lyric-copy"><span>{{ line.text }}</span><small v-if="line.translation">{{ line.translation }}</small></span>
      </button>
    </div>
    <div class="lyrics-reading-tools">
      <button v-if="!following" class="soft-button return-lyric" @click="resume">返回当前歌词</button>
      <button v-if="!reduced" class="lyric-mode-button" :aria-pressed="simple" @click="simple = !simple">{{ simple ? '动态歌词' : '简洁歌词' }}</button>
    </div>
  </div>
</template>

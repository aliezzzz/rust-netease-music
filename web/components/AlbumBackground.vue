<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { BACKGROUND_FPS, backgroundSize, cachedTexture, coverSource, createCoverTexture } from '../albumTexture';
const props = defineProps<{ cover?: string; playing: boolean; reduced: boolean; hidden: boolean; suspended: boolean }>();
const canvas = ref<HTMLCanvasElement>();
const ready = ref(false);
const source = computed(() => coverSource(props.cover));
const running = computed(() => props.playing && !props.reduced && !props.hidden && !props.suspended);
let context: CanvasRenderingContext2D | null = null;
let texture: HTMLCanvasElement | undefined;
let observer: ResizeObserver | undefined;
let frame = 0, timer: ReturnType<typeof setTimeout> | undefined;
let lastFrame = 0, phase = 0;
let disposed = false;
let releaseImage = () => {};
let loadVersion = 0;
function stop() { cancelAnimationFrame(frame); clearTimeout(timer); frame = 0; timer = undefined; lastFrame = 0; }
function draw() {
  const surface = canvas.value;
  if (!context || !surface || !texture || props.suspended || props.hidden) return;
  const w = surface.width, h = surface.height;
  context.globalAlpha = 1;
  context.drawImage(texture, 0, 0, w, h);
  // Two small preblurred texture samples blended inside ONE capped canvas.
  context.save();
  context.globalAlpha = .48;
  context.translate(w * (.5 + Math.sin(phase * .13) * .08), h * .5);
  context.rotate(phase * .035);
  context.drawImage(texture, -w * .75, -h * .75, w * 1.5, h * 1.5);
  context.restore();
  ready.value = true;
}
function tick(now: number) {
  if (disposed || !running.value || !texture) return;
  if (lastFrame) phase += Math.min(100, now - lastFrame) / 1000;
  lastFrame = now;
  draw();
  // Do not wake on every high-refresh display frame just to skip rendering.
  timer = setTimeout(() => { frame = requestAnimationFrame(tick); }, 1000 / BACKGROUND_FPS);
}
function sync() {
  stop();
  if (props.suspended || props.hidden) return;
  draw();
  if (running.value && texture) frame = requestAnimationFrame(tick);
}
function load() {
  const src = source.value;
  if (!src || props.suspended || props.hidden || texture) return;
  const cached = cachedTexture(src);
  if (cached) { texture = cached; sync(); return; }
  releaseImage();
  const version = ++loadVersion;
  const image = new Image();
  const timeout = setTimeout(() => { image.onload = image.onerror = null; }, 8000);
  releaseImage = () => { clearTimeout(timeout); image.onload = image.onerror = null; };
  image.crossOrigin = 'anonymous';
  image.onload = () => {
    if (disposed || version !== loadVersion) return;
    releaseImage();
    // Don't decode/process a completed request in the middle of the sheet exit.
    if (props.suspended || props.hidden) return;
    try { texture = createCoverTexture(image, src); sync(); }
    catch { ready.value = false; } // CORS / unavailable canvas: static gradient.
  };
  image.onerror = releaseImage;
  image.src = src;
}
watch(source, () => {
  loadVersion++; releaseImage(); stop(); texture = undefined; ready.value = false;
  load();
});
watch(() => [props.suspended, props.hidden], () => { load(); sync(); });
watch(running, sync);
onMounted(() => {
  context = canvas.value?.getContext('2d', { alpha: false }) ?? null;
  if (!context || !canvas.value) return;
  context.imageSmoothingEnabled = true;
  observer = new ResizeObserver(entries => {
    const rect = entries[0].contentRect;
    const size = backgroundSize(rect.width, rect.height);
    const surface = canvas.value;
    if (!surface || (surface.width === size.width && surface.height === size.height)) return;
    surface.width = size.width; surface.height = size.height;
    draw();
  });
  observer.observe(canvas.value);
  load(); sync();
});
onUnmounted(() => { disposed = true; loadVersion++; stop(); releaseImage(); observer?.disconnect(); });
</script>
<template>
  <div class="album-background" :data-motion="running && ready ? 'flowing' : 'static'" aria-hidden="true">
    <canvas ref="canvas" class="album-background-canvas" :class="{ ready }" width="1" height="1"></canvas>
    <div class="album-background-shade"></div>
  </div>
</template>

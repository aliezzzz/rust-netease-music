<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue';
import { Play, Heart, Plus, MoreHorizontal, AudioLines } from 'lucide-vue-next';
import type { Track } from '../types';
import { play, playback, formatTime, command, like, likes, notify } from '../store';
import Cover from './Cover.vue';
const props = defineProps<{ tracks: Track[] }>();
const root = ref<HTMLElement>();
const start = ref(0); const count = ref(30); const menu = ref<number | null>(null);
const rowHeight = 64;
const visible = computed(() => props.tracks.slice(start.value, start.value + count.value));
let scrollParent: HTMLElement | null = null;
let observer: ResizeObserver | undefined;
function measure() {
  if (!root.value || !scrollParent) return;
  const relative = scrollParent.getBoundingClientRect().top - root.value.getBoundingClientRect().top;
  start.value = Math.max(0, Math.floor(relative / rowHeight) - 5);
  count.value = Math.ceil(scrollParent.clientHeight / rowHeight) + 12;
}
onMounted(() => {
  scrollParent = root.value?.closest('.content-scroll') ?? null;
  scrollParent?.addEventListener('scroll', measure, { passive: true });
  observer = new ResizeObserver(measure); if (scrollParent) observer.observe(scrollParent); measure();
});
onUnmounted(() => { scrollParent?.removeEventListener('scroll', measure); observer?.disconnect(); });
async function share(track: Track) {
  try { await navigator.clipboard.writeText(`${track.name} - ${track.artist} https://music.163.com/#/song?id=${track.id}`); notify('已复制歌曲链接'); } catch { notify('无法访问剪贴板'); }
  menu.value = null;
}
</script>
<template>
  <div class="track-list-header"><span>#</span><span>歌曲</span><span class="album-column">专辑</span><span>时长</span><span></span></div>
  <div ref="root" class="track-list" role="list" :aria-label="`${tracks.length} 首歌曲`" :style="{ height: `${tracks.length * rowHeight}px` }">
    <div v-for="(track, i) in visible" :key="track.id" class="track-row" :class="{ current: playback.current_id === track.id, unavailable: !track.playable }" :style="{ transform: `translateY(${(start + i) * rowHeight}px)` }" role="listitem" :aria-posinset="start + i + 1" :aria-setsize="tracks.length" @dblclick="play(tracks, start + i)">
      <button class="row-number icon-button" :aria-label="`播放 ${track.name}`" @click="play(tracks, start + i)"><AudioLines v-if="playback.current_id === track.id && playback.playback === 'playing'" /><span v-else>{{ start + i + 1 }}</span><Play class="hover-play" /></button>
      <div class="row-title"><Cover :src="track.cover" :size="100" /><div><button class="text-button song-title" :title="track.name" @click="play(tracks, start + i)">{{ track.name }}</button><span :title="track.artist">{{ track.artist }}</span></div></div>
      <RouterLink class="album-column" :to="`/album/${track.album_id}`" :title="track.album">{{ track.album }}</RouterLink>
      <span class="duration">{{ formatTime(track.duration_ms) }}</span>
      <div class="row-actions">
        <button class="icon-button" :class="{ active: likes.has(track.id) }" :aria-label="`喜欢 ${track.name}`" @click="like(track)"><Heart :fill="likes.has(track.id) ? 'currentColor' : 'none'" /></button>
        <button class="icon-button" :aria-label="`${track.name} 的更多操作`" @click="menu = menu === track.id ? null : track.id"><MoreHorizontal /></button>
        <div v-if="menu === track.id" class="context-menu" @keydown.esc="menu = null">
          <button @click="command({ type: 'append', tracks: [track] }); menu = null"><Plus />添加到播放队列</button>
          <button @click="share(track)">复制歌曲链接</button>
          <button @click="menu = null">关闭菜单</button>
        </div>
      </div>
    </div>
  </div>
</template>

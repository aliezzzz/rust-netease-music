<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { X, ListMusic } from 'lucide-vue-next';
import { playback, ui, command } from '../store';
import Cover from './Cover.vue';
const scroll = ref<HTMLElement>(); const top = ref(0); const height = ref(600);
const rowHeight = 56;
const start = computed(() => Math.max(0, Math.floor(top.value / rowHeight) - 4));
const visible = computed(() => playback.value.queue.slice(start.value, start.value + Math.ceil(height.value / rowHeight) + 8));
let observer: ResizeObserver | undefined;
onMounted(() => { observer = new ResizeObserver(() => { height.value = scroll.value?.clientHeight ?? 600; }); if (scroll.value) observer.observe(scroll.value); });
onUnmounted(() => observer?.disconnect());
watch(() => playback.value.queue_revision, async () => { await nextTick(); top.value = scroll.value?.scrollTop ?? 0; });
</script>
<template>
  <aside class="queue-panel" aria-label="播放队列">
    <header><h2>播放队列 <small>{{ playback.queue.length }}</small></h2><button class="icon-button" aria-label="关闭队列" @click="ui.queue = false"><X /></button></header>
    <div class="queue-actions"><span>接下来播放</span><button class="text-button" @click="command({ type: 'clear' })">清空</button></div>
    <div v-if="!playback.queue.length" class="empty-state"><ListMusic /><h3>队列还是空的</h3><p>播放歌曲后，会在这里显示。</p></div>
    <div ref="scroll" class="queue-items" @scroll.passive="top = ($event.target as HTMLElement).scrollTop">
      <div :style="{ height: `${playback.queue.length * rowHeight}px`, position: 'relative' }">
        <div v-for="(track, i) in visible" :key="track.id" class="queue-item virtual-queue-row" :class="{ current: track.id === playback.current_id }" :style="{ transform: `translateY(${(start + i) * rowHeight}px)` }">
          <button class="queue-track" @click="command({ type: 'select', id: track.id })"><Cover :src="track.cover" :size="100" /><span><strong>{{ track.name }}</strong><small>{{ track.artist }}</small></span></button>
          <button class="icon-button" :aria-label="`移除 ${track.name}`" @click="command({ type: 'remove', id: track.id })"><X /></button>
        </div>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { Play, Pause, SkipBack, SkipForward, Shuffle, Repeat, Repeat1, LoaderCircle } from 'lucide-vue-next';
import { playback, current, command, formatTime } from '../store';
const props = defineProps<{ position?: number }>();
const drag = ref<number | null>(null);
const progress = computed(() => drag.value ?? props.position ?? playback.value.position_ms);
watch(() => current.value?.id, () => { drag.value = null; });
function repeat() {
  const next = { off: 'all', all: 'one', one: 'off' }[playback.value.repeat];
  void command({ type: 'mode', repeat: next, shuffle: playback.value.shuffle });
}
function seek() {
  if (drag.value !== null) void command({ type: 'seek', position_ms: drag.value });
  drag.value = null;
}
</script>
<template>
  <div class="transport">
    <div class="transport-buttons">
      <button class="icon-button secondary" :class="{ active: playback.shuffle }" :aria-pressed="playback.shuffle" aria-label="随机播放" @click="command({ type: 'mode', repeat: playback.repeat, shuffle: !playback.shuffle })"><Shuffle /></button>
      <button class="icon-button" aria-label="上一首" :disabled="!current" @click="command({ type: 'previous' })"><SkipBack fill="currentColor" /></button>
      <button class="play-button" :aria-label="playback.playback === 'playing' ? '暂停' : '播放'" :disabled="!current && playback.playback !== 'loading'" @click="command({ type: 'toggle' })">
        <LoaderCircle v-if="playback.playback === 'loading'" class="spin" /><Pause v-else-if="playback.playback === 'playing'" fill="currentColor" /><Play v-else fill="currentColor" />
      </button>
      <button class="icon-button" aria-label="下一首" :disabled="!current" @click="command({ type: 'next' })"><SkipForward fill="currentColor" /></button>
      <button class="icon-button secondary" :class="{ active: playback.repeat !== 'off' }" :aria-label="`循环模式：${playback.repeat}`" @click="repeat"><Repeat1 v-if="playback.repeat === 'one'" /><Repeat v-else /></button>
    </div>
    <div class="timeline"><span>{{ formatTime(progress) }}</span><input aria-label="播放进度" type="range" min="0" :max="Math.max(1, playback.duration_ms)" :value="progress" :disabled="!current" @input="drag = Number(($event.target as HTMLInputElement).value)" @change="seek" @blur="drag = null" /><span>{{ formatTime(playback.duration_ms) }}</span></div>
  </div>
</template>

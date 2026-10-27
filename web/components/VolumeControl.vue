<script setup lang="ts">
import { ref } from 'vue';
import { Volume2, VolumeX } from 'lucide-vue-next';
import { playback, command } from '../store';
const previousVolume = ref(.8);
const draftVolume = ref<number | null>(null);
function setVolume(event: Event) {
  const value = Number((event.target as HTMLInputElement).value);
  draftVolume.value = null;
  void command({ type: 'volume', value });
}
function mute() {
  if (playback.value.volume > 0) previousVolume.value = playback.value.volume;
  void command({ type: 'volume', value: playback.value.volume ? 0 : previousVolume.value });
}
</script>
<template>
  <div class="volume-control">
    <button class="icon-button volume-button" :aria-label="playback.volume ? '静音' : '取消静音'" @click="mute"><Volume2 v-if="playback.volume" /><VolumeX v-else /></button>
    <input class="volume-slider" aria-label="音量" type="range" min="0" max="1" step="0.01" :value="draftVolume ?? playback.volume" :style="{ '--volume-fill': `${(draftVolume ?? playback.volume) * 100}%` }" @input="draftVolume = Number(($event.target as HTMLInputElement).value)" @change="setVolume" @blur="draftVolume = null" />
  </div>
</template>

<style scoped>
input.volume-slider {
  --volume-color: var(--accent);
  --volume-track: #8882;
  appearance: none;
  -webkit-appearance: none;
  height: 24px;
  margin: 0;
  padding: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
  cursor: pointer;
}
input.volume-slider::-webkit-slider-runnable-track {
  height: 3px;
  border-radius: 999px;
  background: linear-gradient(to right, var(--volume-color) var(--volume-fill), var(--volume-track) var(--volume-fill));
}
input.volume-slider::-webkit-slider-thumb {
  appearance: none;
  -webkit-appearance: none;
  width: 9px;
  height: 9px;
  margin-top: -3px;
  border: 0;
  border-radius: 50%;
  background: var(--volume-color);
}
input.volume-slider::-moz-range-track {
  height: 3px;
  border-radius: 999px;
  background: var(--volume-track);
}
input.volume-slider::-moz-range-progress {
  height: 3px;
  border-radius: 999px;
  background: var(--volume-color);
}
input.volume-slider::-moz-range-thumb {
  width: 9px;
  height: 9px;
  border: 0;
  border-radius: 50%;
  background: var(--volume-color);
}
input.volume-slider:focus-visible {
  outline: 2px solid var(--volume-color);
  outline-offset: 3px;
  border-radius: 4px;
}
:global(.lyrics-extras) input.volume-slider {
  --volume-color: #ffffffb3;
  --volume-track: #ffffff26;
}
</style>

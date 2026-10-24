<script setup lang="ts">
import { ref } from 'vue';
import { Volume2, VolumeX } from 'lucide-vue-next';
import { playback, command } from '../store';
const previousVolume = ref(.8);
function mute() {
  if (playback.value.volume > 0) previousVolume.value = playback.value.volume;
  void command({ type: 'volume', value: playback.value.volume ? 0 : previousVolume.value });
}
</script>
<template>
  <div class="volume-control">
    <button class="icon-button volume-button" :aria-label="playback.volume ? '静音' : '取消静音'" @click="mute"><Volume2 v-if="playback.volume" /><VolumeX v-else /></button>
    <input class="volume-slider" aria-label="音量" type="range" min="0" max="1" step="0.01" :value="playback.volume" @change="command({ type: 'volume', value: Number(($event.target as HTMLInputElement).value) })" />
  </div>
</template>

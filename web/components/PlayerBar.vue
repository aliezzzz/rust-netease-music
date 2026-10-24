<script setup lang="ts">
import { ListMusic, Mic2, Heart } from 'lucide-vue-next';
import { current, ui, like, likes } from '../store';
import Cover from './Cover.vue';
import QualityPicker from './QualityPicker.vue';
import TransportControls from './TransportControls.vue';
import VolumeControl from './VolumeControl.vue';
</script>
<template>
  <footer class="player-bar" aria-label="播放器">
    <div class="now-playing">
      <button class="art-button" :disabled="!current" title="打开歌词" @click="ui.lyrics = !ui.lyrics"><Cover :src="current?.cover" :name="current?.name" :size="120" /></button>
      <div class="track-info"><strong :title="current?.name">{{ current?.name ?? '让音乐，陪你一会儿' }}</strong><span :title="current?.artist">{{ current?.artist ?? '选择喜欢的歌曲开始播放' }}</span></div>
      <button class="icon-button like-button" :class="{ active: current && likes.has(current.id) }" :disabled="!current" aria-label="喜欢当前歌曲" @click="current && like(current)"><Heart :fill="current && likes.has(current.id) ? 'currentColor' : 'none'" /></button>
    </div>
    <TransportControls />
    <div class="player-tools">
      <QualityPicker />
      <button class="icon-button" :class="{ active: ui.lyrics }" aria-label="歌词" :aria-pressed="ui.lyrics" @click="ui.lyrics = !ui.lyrics"><Mic2 /></button>
      <button class="icon-button" :class="{ active: ui.queue }" aria-label="播放队列" :aria-pressed="ui.queue" @click="ui.queue = !ui.queue"><ListMusic /></button>
      <VolumeControl />
    </div>
  </footer>
</template>

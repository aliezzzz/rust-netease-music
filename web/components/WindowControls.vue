<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Minus, Square, Copy, X } from 'lucide-vue-next';
import { notify } from '../store';

const native = isTauri();
const maximized = ref(false);
let disposed = false;
let unlisten: (() => void) | undefined;
async function refresh() {
  const value = await getCurrentWindow().isMaximized();
  if (!disposed) maximized.value = value;
}
async function act(action: 'minimize' | 'toggleMaximize' | 'close') {
  try {
    // close(), not destroy(): preserve App.vue's state-saving close handler.
    await getCurrentWindow()[action]();
    if (action === 'toggleMaximize') await refresh();
  } catch (error) { notify(error); }
}
onMounted(async () => {
  if (!native) return;
  try {
    const stop = await getCurrentWindow().onResized(() => { void refresh().catch(notify); });
    if (disposed) { stop(); return; }
    unlisten = stop;
    await refresh();
  } catch (error) { notify(error); }
});
onUnmounted(() => { disposed = true; unlisten?.(); });
</script>
<template>
  <div v-if="native" class="window-controls" role="group" aria-label="窗口控制" data-tauri-drag-region="false">
    <button class="window-button" aria-label="最小化窗口" title="最小化" @click="act('minimize')"><Minus /></button>
    <button class="window-button" :aria-label="maximized ? '还原窗口' : '最大化窗口'" :title="maximized ? '还原' : '最大化'" @click="act('toggleMaximize')"><Copy v-if="maximized" /><Square v-else /></button>
    <button class="window-button window-close" aria-label="关闭窗口" title="关闭窗口，保留后台播放" @click="act('close')"><X /></button>
  </div>
</template>

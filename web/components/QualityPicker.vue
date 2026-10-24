<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue';
import { Check, ChevronUp, AudioLines, LoaderCircle } from 'lucide-vue-next';
import { preferences, call, notify } from '../store';
import { qualities } from '../quality';
const open = ref(false); const busy = ref(false); const root = ref<HTMLElement>(); const trigger = ref<HTMLButtonElement>();
const label = computed(() => qualities.find(([value]) => value === preferences.quality)?.[1] ?? '标准');
const descriptions: Record<string, string> = { standard: '轻盈流畅', higher: '更丰富的细节', exhigh: '清晰饱满', lossless: '保留声音细节', hires: '高解析聆听', jyeffect: '空间环绕体验', sky: '沉浸聆听', jymaster: '母带级体验' };
function close(focus = false) { open.value = false; if (focus) trigger.value?.focus(); }
function outside(event: PointerEvent) { if (!root.value?.contains(event.target as Node)) close(); }
async function toggle() {
  if (open.value) { close(); return; }
  open.value = true;
  await nextTick();
  root.value?.querySelector<HTMLButtonElement>('[aria-checked="true"]')?.focus();
}
function navigate(event: KeyboardEvent) {
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
  event.preventDefault();
  const items = Array.from(root.value?.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]:not(:disabled)') ?? []);
  const index = items.indexOf(document.activeElement as HTMLButtonElement);
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : (index + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
  items[next]?.focus();
}
async function select(quality: string) {
  if (busy.value) return;
  if (quality === preferences.quality) { close(true); return; }
  busy.value = true;
  try {
    await call('save_settings', { settings: { ...preferences, quality } });
    preferences.quality = quality;
    notify('首选音质已保存，下次加载歌曲时生效；实际音质取决于歌曲与会员权益');
    close(true);
  } catch (error) { notify(error); }
  finally { busy.value = false; }
}
onMounted(() => document.addEventListener('pointerdown', outside));
onUnmounted(() => document.removeEventListener('pointerdown', outside));
</script>
<template>
  <div ref="root" class="quality-picker" @keydown.esc.stop.prevent="close(true)" @focusout="event => { if (!root?.contains(event.relatedTarget as Node)) close(); }">
    <button ref="trigger" class="quality-trigger" :aria-expanded="open" aria-haspopup="menu" aria-label="切换首选音质" @click="toggle"><span>{{ label }}</span><ChevronUp :class="{ flipped: open }" /></button>
    <Transition name="quality-pop">
      <div v-if="open" class="quality-popover">
        <header class="quality-heading"><div><AudioLines /><strong>聆听品质</strong></div><span>首选音质</span></header>
        <div class="quality-options" role="menu" aria-label="首选音质" :aria-busy="busy" @keydown="navigate">
          <template v-for="([value, name], index) in qualities" :key="value">
            <div v-if="index === 3" class="quality-divider" role="presentation"><span>高品质体验</span></div>
            <button class="quality-option" role="menuitemradio" :aria-label="name" :aria-checked="preferences.quality === value" :disabled="busy" @click="select(value)">
              <span class="quality-option-copy"><span class="quality-option-name">{{ name }}<span v-if="value === 'hires'" class="quality-badge">Hi-Res</span><span v-if="value === 'jymaster'" class="quality-badge">MASTER</span></span><small>{{ descriptions[value] }}</small></span>
              <Check v-if="preferences.quality === value" class="quality-check" /><span v-else class="quality-check-space"></span>
            </button>
          </template>
        </div>
        <footer class="quality-note"><LoaderCircle v-if="busy" class="spin" /><span>{{ busy ? '正在保存偏好…' : '下次加载歌曲时生效' }}<small>实际音质取决于音源及会员权益</small></span></footer>
      </div>
    </Transition>
  </div>
</template>

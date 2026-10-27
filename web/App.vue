<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, watch, ref, nextTick } from 'vue';
import { useRouter, useRoute } from 'vue-router';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Search, Compass, ChartNoAxesCombined, Heart, CalendarDays, Radio, Cloud, Library, Disc3, Settings2, ChevronLeft, ChevronRight, UserRound, Music2, X, Trash2, ListMusic } from 'lucide-vue-next';
import { initialize, ui, user, preferences, playback, command, call, notify } from './store';
import PlayerBar from './components/PlayerBar.vue';
import logo from './assets/logo.png';
import WindowControls from './components/WindowControls.vue';
import Cover from './components/Cover.vue';
const Lyrics = defineAsyncComponent(() => import('./components/Lyrics.vue'));
import Login from './components/Login.vue';
import QueuePanel from './components/QueuePanel.vue';
const router = useRouter(); const route = useRoute();
const search = ref(''); const input = ref<HTMLInputElement>(); const scroll = ref<HTMLElement>();
const scrollPositions: Record<string, number> = {};
// Keep the modal and its focus boundary alive until its exit transition finishes.
const lyricsMounted = ref(ui.lyrics);
const lyricsBlocking = computed(() => ui.lyrics || lyricsMounted.value);
let lyricTrigger: HTMLElement | null = null;
let searchAfterLyrics = false;
async function lyricsClosed() {
  if (ui.lyrics) return;
  lyricsMounted.value = false;
  await nextTick();
  if (ui.lyrics) return;
  if (searchAfterLyrics) { searchAfterLyrics = false; input.value?.focus(); }
  else if (document.activeElement === document.body && lyricTrigger?.isConnected) lyricTrigger.focus();
}
let dispose = () => {}; let unclose = () => {}; let saveTimer: ReturnType<typeof setTimeout>; let restored = false;
const links = [
  { path: '/discover', label: '发现音乐', icon: Compass }, { path: '/charts', label: '排行榜', icon: ChartNoAxesCombined },
  { path: '/albums', label: '新碟上架', icon: Disc3 },
];
const library = [
  { path: '/favorites', label: '喜欢的音乐', icon: Heart }, { path: '/daily', label: '每日推荐', icon: CalendarDays },
  { path: '/recommended', label: '推荐歌单', icon: Disc3 },
  { path: '/fm', label: '私人 FM', icon: Radio }, { path: '/cloud', label: '音乐云盘', icon: Cloud },
  { path: '/library', label: '我的歌单', icon: Library }, { path: '/saved_albums', label: '收藏的专辑', icon: Disc3 },
  { path: '/radios', label: '订阅电台', icon: Radio },
];
function doSearch() { if (search.value.trim()) void router.push({ path: '/search_song', query: { q: search.value.trim() } }); }
function saveUi() {
  if (!ui.connected || !restored) return;
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    void flushUi().catch(notify);
  }, 200);
}
async function flushUi() {
  clearTimeout(saveTimer);
  if (ui.connected && restored) await call('ui_state', { value: { route: route.fullPath, queue: ui.queue, lyrics: ui.lyrics, scroll: scrollPositions } });
}
async function quit() { try { await flushUi(); } finally { await call('quit'); } }
function recordScroll() { if (scroll.value) { scrollPositions[route.fullPath] = scroll.value.scrollTop; saveUi(); } }
function restoreScroll() { if (scroll.value) scroll.value.scrollTop = scrollPositions[route.fullPath] ?? 0; }
function shortcut(event: KeyboardEvent) {
  const editing = (event.target as HTMLElement)?.closest('input, textarea, [contenteditable]');
  if ((event.ctrlKey && ['f', 'k'].includes(event.key)) || (!editing && event.key === '/')) {
    event.preventDefault();
    if (lyricsBlocking.value) { searchAfterLyrics = true; ui.lyrics = false; }
    else input.value?.focus();
  }
  else if (event.ctrlKey && event.key === 'q') { event.preventDefault(); void quit(); }
  else if (!editing && event.ctrlKey && event.key === 'b') { event.preventDefault(); void command({ type: 'previous' }); }
  else if (!editing && event.ctrlKey && event.key === 'n') { event.preventDefault(); void command({ type: 'next' }); }
  else if (event.key === 'Escape') { ui.login = false; ui.lyrics = false; ui.queue = false; }
  else if (!editing && !(event.target as HTMLElement)?.closest('button, a, select') && event.code === 'Space') { event.preventDefault(); void command({ type: 'toggle' }); }
}
watch(() => [preferences.theme, preferences.reduced_motion], () => {
  document.documentElement.dataset.theme = preferences.theme;
  document.documentElement.dataset.motion = preferences.reduced_motion ? 'reduce' : 'auto';
}, { immediate: true });
watch(() => [ui.queue, ui.lyrics, route.fullPath], saveUi);
watch(() => route.fullPath, () => { ui.lyrics = false; });
watch(() => ui.lyrics, open => {
  if (open) {
    if (!lyricsMounted.value) lyricTrigger = document.activeElement as HTMLElement | null;
    searchAfterLyrics = false;
    lyricsMounted.value = true;
  }
}, { flush: 'sync' });
onMounted(async () => {
  document.addEventListener('keydown', shortcut);
  dispose = await initialize();
  if (ui.connected) {
    try {
      const saved = await call<{ route?: string; queue?: boolean; lyrics?: boolean; scroll?: Record<string, number> }>('ui_state');
      Object.assign(scrollPositions, saved.scroll ?? {});
      ui.queue = saved.queue ?? false;
      if (saved.route?.startsWith('/')) await router.replace(saved.route);
      ui.lyrics = saved.lyrics ?? false;
    } catch (e) { notify(e); }
    restored = true;
    unclose = await getCurrentWindow().onCloseRequested(async event => {
      event.preventDefault();
      try { await flushUi(); } catch (e) { notify(e); }
      finally { await getCurrentWindow().destroy(); }
    });
  }
});
onUnmounted(() => { dispose(); unclose(); document.removeEventListener('keydown', shortcut); clearTimeout(saveTimer); });
</script>
<template>
  <div class="app-shell">
    <aside class="sidebar" :inert="lyricsBlocking" :aria-hidden="lyricsBlocking || undefined">
      <RouterLink class="brand" to="/discover" @click="ui.lyrics = false"><img class="brand-mark" :src="logo" alt="" /><span>云音乐<small>让喜欢，随时发生</small></span></RouterLink>
      <nav aria-label="主要导航"><RouterLink v-for="item in links" :key="item.path" :to="item.path" @click="ui.lyrics = false"><component :is="item.icon" /><span>{{ item.label }}</span></RouterLink></nav>
      <div class="nav-label">我的音乐</div>
      <nav class="library-navigation" aria-label="资料库"><RouterLink v-for="item in library" :key="item.path" :to="item.path" @click="ui.lyrics = false"><component :is="item.icon" /><span>{{ item.label }}</span></RouterLink></nav>
      <div class="sidebar-bottom"><RouterLink to="/settings" @click="ui.lyrics = false"><Settings2 />设置</RouterLink><span class="connection"><i :class="{ online: ui.connected }"></i>{{ ui.connected ? '音乐，始终在线' : '等待播放核心' }}</span></div>
    </aside>
    <div class="workspace" :inert="lyricsBlocking" :aria-hidden="lyricsBlocking || undefined">
      <header class="topbar" data-tauri-drag-region>
        <div class="history"><button class="icon-button" aria-label="后退" @click="router.back()"><ChevronLeft /></button><button class="icon-button" aria-label="前进" @click="router.forward()"><ChevronRight /></button></div>
        <form class="search-box" role="search" @submit.prevent="doSearch"><Search /><input ref="input" v-model="search" aria-label="搜索音乐" placeholder="搜索歌曲、歌手、专辑" /><kbd>Ctrl K</kbd></form>
        <button class="account-button" @click="user ? router.push('/settings') : ui.login = true"><Cover v-if="user" :src="user.avatar_url" :size="64" round /><UserRound v-else /><span>{{ user?.nickname ?? '登录' }}</span></button>
        <WindowControls />
      </header>
      <div class="body-panels">
        <main ref="scroll" class="content-scroll" @scroll.passive="recordScroll">
          <RouterView v-slot="{ Component }"><Transition name="page" mode="out-in"><component :is="Component" :key="route.fullPath" @ready="restoreScroll" /></Transition></RouterView>
        </main>
        <QueuePanel v-if="ui.queue && !lyricsBlocking" />
      </div>
    </div>
    <PlayerBar :inert="lyricsBlocking" :aria-hidden="lyricsBlocking || undefined" />
    <Lyrics v-if="lyricsMounted" :open="ui.lyrics" @closed="lyricsClosed" />
    <Login v-if="ui.login" />
    <div v-if="ui.message && !lyricsBlocking" class="toast" role="status">{{ ui.message }}<button class="icon-button" aria-label="关闭消息" @click="ui.message = ''"><X /></button></div>
  </div>
</template>

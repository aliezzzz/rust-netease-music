<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { Play, Shuffle, ArrowUpRight, RefreshCw, Music2, Heart, Radio, Sparkles, Plus } from 'lucide-vue-next';
import { query, play, command, call, notify, user, ui, current, playback } from '../store';
import type { Page, Card } from '../types';
import Cover from '../components/Cover.vue';
import TrackList from '../components/TrackList.vue';
const emit = defineEmits<{ ready: [] }>();
const route = useRoute(); const router = useRouter();
const page = ref<Page>({ title: '', subtitle: '', cover: '', cards: [], tracks: [], more: false });
const albums = ref<Card[]>([]); const loading = ref(true); const failure = ref(''); const offset = ref(0);
let generation = 0;
const kind = computed(() => String(route.params.kind));
const detail = computed(() => ['playlist', 'album', 'artist', 'radio', 'favorites', 'daily', 'cloud'].includes(kind.value));
const searching = computed(() => kind.value.startsWith('search_'));
const tabs = [{ kind: 'song', label: '歌曲' }, { kind: 'artist', label: '歌手' }, { kind: 'album', label: '专辑' }, { kind: 'playlist', label: '歌单' }, { kind: 'lyrics', label: '歌词' }];
const titles: Record<string,string> = { discover: '发现音乐', albums: '新碟上架', charts: '排行榜', favorites: '喜欢的音乐', daily: '每日推荐', cloud: '音乐云盘', library: '我的歌单', saved_albums: '收藏的专辑', radios: '订阅电台' };
async function load(more = false) {
  const token = ++generation;
  loading.value = true; failure.value = '';
  try {
    if (kind.value === 'fm') return;
    if (!more) offset.value = 0;
    const value = await query({ kind: kind.value, id: Number(route.params.id) || 0, text: String(route.query.q ?? route.query.name ?? ''), offset: offset.value, limit: kind.value === 'discover' ? 12 : 50 });
    if (token !== generation) return;
    page.value = more ? { ...value, tracks: [...page.value.tracks, ...value.tracks], cards: [...page.value.cards, ...value.cards] } : value;
    offset.value += kind.value === 'discover' ? 12 : 50;
    if (kind.value === 'discover' && !more) {
      query({ kind: 'albums', limit: 6 }).then(p => { if (token === generation) albums.value = p.cards; }).catch(() => {});
    }
  } catch (e) { if (token === generation) failure.value = String(e); }
  finally { if (token === generation) { loading.value = false; await nextTick(); emit('ready'); } }
}
function open(card: Card) { void router.push({ path: `/${card.kind}/${card.id}`, query: { name: card.name } }); }
async function fm() {
  if (!user.value) { ui.login = true; return; }
  await command({ type: 'continuous', mode: { kind: 'fm' } });
}
async function heartbeat() {
  try {
    if (playback.value.continuous.kind === 'heartbeat') { await command({ type: 'continuous', mode: { kind: 'none' } }); return; }
    const id = await call<number>('account', { operation: 'favorite_id' });
    if (!current.value && page.value.tracks.length) await play(page.value.tracks);
    await command({ type: 'continuous', mode: { kind: 'heartbeat', playlist_id: id } });
  } catch (e) { notify(e); }
}
async function trashFm() {
  if (!current.value) return;
  try { await call('favorite', { kind: 'fm_trash', id: current.value.id, liked: false }); await command({ type: 'next' }); } catch (e) { notify(e); }
}
async function saveCollection() {
  if (!user.value) { ui.login = true; return; }
  const liked = !page.value.subscribed;
  try { await call('favorite', { kind: kind.value, id: Number(route.params.id), liked }); page.value.subscribed = liked; notify(liked ? '已收藏' : '已取消收藏'); } catch (e) { notify(e); }
}
onMounted(() => load());
onUnmounted(() => { generation++; });
watch(() => ui.accountRevision, () => { if (['favorites', 'daily', 'cloud', 'library', 'saved_albums', 'radios', 'recommended', 'playlist', 'album'].includes(kind.value)) void load(); });
</script>
<template>
  <section class="page">
    <template v-if="kind === 'fm'">
      <div class="page-heading"><span class="eyebrow">为你而来</span><h1>私人 FM</h1><p>放下选择，让下一首带来惊喜。</p></div>
      <div class="fm-stage"><div class="fm-art"><Cover :src="current?.cover" :name="current?.name" /></div><div><Radio class="fm-symbol" /><h2>{{ playback.continuous.kind === 'fm' ? current?.name ?? '正在寻找你的音乐' : '总有一首，懂你此刻' }}</h2><p>{{ playback.continuous.kind === 'fm' ? current?.artist : '根据你的音乐喜好，持续发现新的旋律。' }}</p><button class="primary-button" @click="playback.continuous.kind === 'fm' ? command({ type: 'next' }) : fm()"><Play fill="currentColor" />{{ playback.continuous.kind === 'fm' ? '下一首' : '开始私人 FM' }}</button><button v-if="playback.continuous.kind === 'fm' && current" class="soft-button" @click="trashFm">不喜欢这首</button></div></div>
    </template>
    <template v-else>
      <template v-if="kind === 'discover'">
        <div class="page-heading"><span class="eyebrow">为每一种心情</span><h1>发现音乐<span class="accent-dot">.</span></h1><p>熟悉的旋律，新的心动。</p></div>
        <div class="discovery-features">
          <button class="feature-card daily-feature" @click="router.push('/daily')"><div><span class="eyebrow">专属于你的每日灵感</span><h2>今天，听点喜欢的。</h2><p>每日推荐 · 每天都有新发现</p></div><span class="feature-play"><Play fill="currentColor" /></span><span class="feature-decoration">{{ new Date().getDate() }}</span></button>
          <button class="feature-card fm-feature" @click="router.push('/fm')"><Radio /><span><small>无需选择</small><h2>私人 FM</h2><p>下一首，也许正合心意</p></span><ArrowUpRight /></button>
        </div>
      </template>
      <div v-else-if="detail && !loading && !failure" class="detail-heading">
        <Cover :src="page.cover || page.tracks[0]?.cover" :name="page.title" />
        <div><span class="eyebrow">{{ kind === 'album' ? '专辑' : kind === 'artist' ? '歌手' : '音乐精选' }}</span><h1>{{ page.title || titles[kind] }}</h1><p class="description">{{ page.subtitle }}</p><span class="muted">{{ page.tracks.length }} 首歌曲</span><div class="detail-actions"><button class="primary-button" :disabled="!page.tracks.length" @click="play(page.tracks)"><Play fill="currentColor" />播放全部</button><button class="soft-button" :disabled="!page.tracks.length" @click="command({ type: 'append', tracks: page.tracks }); notify('已添加到队列')"><Plus />添加到队列</button><button v-if="['playlist','album'].includes(kind)" class="icon-button" :class="{ active: page.subscribed }" :aria-label="page.subscribed ? '取消收藏' : '收藏'" :aria-pressed="!!page.subscribed" @click="saveCollection"><Heart :fill="page.subscribed ? 'currentColor' : 'none'" /></button><button v-if="kind === 'favorites'" class="soft-button" :class="{ active: playback.continuous.kind === 'heartbeat' }" @click="heartbeat"><Sparkles />{{ playback.continuous.kind === 'heartbeat' ? '退出心动模式' : '心动模式' }}</button></div></div>
      </div>
      <div v-else-if="kind !== 'discover'" class="page-heading"><span class="eyebrow">{{ searching ? '寻找你的下一首喜欢' : '你的音乐世界' }}</span><h1>{{ searching ? `“${route.query.q ?? ''}”` : page.title || titles[kind] || route.query.name || '音乐' }}</h1></div>
      <nav v-if="searching" class="search-tabs" aria-label="搜索分类"><RouterLink v-for="tab in tabs" :key="tab.kind" :to="{ path: `/search_${tab.kind}`, query: { q: route.query.q } }">{{ tab.label }}</RouterLink></nav>
      <div v-if="failure" class="empty-state error-state" role="alert"><Music2 /><h2>暂时没有连上音乐</h2><p>{{ failure }}</p><button class="soft-button" @click="load()"><RefreshCw />重试</button><button v-if="!user" class="text-button" @click="ui.login = true">登录账户</button></div>
      <div v-else-if="loading && !page.cards.length && !page.tracks.length" class="card-grid" aria-label="正在加载"><div v-for="i in 12" :key="i" class="skeleton-card"><div></div><span></span><small></small></div></div>
      <template v-else>
        <div v-if="kind === 'discover'" class="section-heading"><h2>值得一听</h2><RouterLink to="/playlists">查看全部 <ArrowUpRight /></RouterLink></div>
        <div v-if="page.cards.length" class="card-grid"><article v-for="card in page.cards" :key="`${card.kind}-${card.id}`" class="music-card"><button class="card-art" :aria-label="`打开 ${card.name}`" @click="open(card)"><Cover :src="card.cover" :name="card.name" :round="card.kind === 'artist'" /><span class="card-open"><ArrowUpRight /></span></button><button class="text-button card-title" :title="card.name" @click="open(card)">{{ card.name }}</button><p :title="card.subtitle">{{ card.subtitle || (card.kind === 'album' ? '专辑' : '精选歌单') }}</p></article></div>
        <TrackList v-if="page.tracks.length" :tracks="page.tracks" />
        <div v-if="!page.cards.length && !page.tracks.length" class="empty-state"><Music2 /><h2>这里还很安静</h2><p>{{ searching ? '换个关键词，再找找看。' : '发现喜欢的音乐，让它们留在这里。' }}</p></div>
        <button v-if="page.more" class="soft-button load-more" :disabled="loading" @click="load(true)">{{ loading ? '加载中…' : '加载更多' }}</button>
        <template v-if="kind === 'discover' && albums.length"><div class="section-heading"><h2>新鲜上架</h2><RouterLink to="/albums">查看全部 <ArrowUpRight /></RouterLink></div><div class="card-grid"><article v-for="card in albums" :key="card.id" class="music-card"><button class="card-art" @click="open(card)"><Cover :src="card.cover" :name="card.name" /></button><button class="text-button card-title" @click="open(card)">{{ card.name }}</button><p>{{ card.subtitle }}</p></article></div></template>
      </template>
    </template>
  </section>
</template>

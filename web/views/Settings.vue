<script setup lang="ts">
import { reactive, ref, onMounted, watch } from 'vue';
import { preferences, user, ui, call, notify, refreshUser } from '../store';
import { Save, Trash2, LogOut } from 'lucide-vue-next';
import { qualities } from '../quality';
const draft = reactive({ ...preferences }); const cacheSize = ref(0); const busy = ref(false);
watch(() => preferences.volume, value => { draft.volume = value; });
watch(() => [preferences.theme, preferences.quality, preferences.mute_start, preferences.proxy, preferences.cache_days, preferences.try_unavailable, preferences.desktop_lyrics, preferences.reduced_motion], () => Object.assign(draft, preferences));
onMounted(async () => { try { cacheSize.value = await call<number>('cache', { clear: false }); } catch (e) { notify(e); } });
async function save() {
  busy.value = true;
  try { await call('save_settings', { settings: draft }); Object.assign(preferences, draft); notify('设置已保存'); } catch (e) { notify(e); }
  finally { busy.value = false; }
}
async function clearCache() { try { cacheSize.value = await call<number>('cache', { clear: true }); notify('缓存已清理'); } catch (e) { notify(e); } }
async function logout() { try { await call('account', { operation: 'logout' }); await refreshUser(); notify('已退出登录'); } catch (e) { notify(e); } }
</script>
<template>
  <section class="page settings-page"><div class="page-heading"><span class="eyebrow">按你的喜好</span><h1>设置</h1><p>让每次聆听，都恰到好处。</p></div>
    <section class="settings-section"><h2>账户</h2><div class="setting-row"><div><strong>{{ user?.nickname ?? '尚未登录' }}</strong><p>{{ user ? '已连接网易云音乐账户' : '登录后同步喜欢、歌单和每日推荐' }}</p></div><button v-if="user" class="soft-button" @click="logout"><LogOut />退出登录</button><button v-else class="primary-button" @click="ui.login = true">登录</button></div></section>
    <form @submit.prevent="save">
      <section class="settings-section"><h2>外观</h2><label class="setting-row"><span><strong>主题</strong><small>选择界面的明暗风格</small></span><select v-model="draft.theme"><option value="system">跟随系统</option><option value="light">浅色</option><option value="dark">深色</option></select></label><label class="setting-row"><span><strong>减少动态效果</strong><small>减少过渡动画与歌词滚动效果</small></span><input v-model="draft.reduced_motion" type="checkbox" /></label></section>
      <section class="settings-section"><h2>播放</h2><label class="setting-row"><span><strong>音质</strong><small>实际可用音质取决于歌曲和账户权限</small></span><select v-model="draft.quality"><option v-for="q in qualities" :key="q[0]" :value="q[0]">{{ q[1] }}</option></select></label><label class="setting-row"><span><strong>启动时静音</strong></span><input v-model="draft.mute_start" type="checkbox" /></label><label class="setting-row"><span><strong>尝试播放灰色歌曲</strong></span><input v-model="draft.try_unavailable" type="checkbox" /></label><label class="setting-row"><span><strong>桌面歌词兼容</strong><small>为外部桌面歌词程序提供歌词文件与媒体信息</small></span><input v-model="draft.desktop_lyrics" type="checkbox" /></label></section>
      <section class="settings-section"><h2>网络与缓存</h2><label class="setting-row"><span><strong>代理地址</strong><small>支持 HTTP、HTTPS、SOCKS5</small></span><input v-model="draft.proxy" class="text-input" placeholder="http://127.0.0.1:7890" /></label><label class="setting-row"><span><strong>自动清理缓存</strong></span><select v-model.number="draft.cache_days"><option :value="0">不自动清理</option><option :value="1">保留一天</option><option :value="7">保留一周</option><option :value="30">保留一月</option></select></label><div class="setting-row"><div><strong>音乐与歌词缓存</strong><p>{{ (cacheSize / 1024 / 1024).toFixed(1) }} MB</p></div><button type="button" class="soft-button" @click="clearCache"><Trash2 />清理缓存</button></div></section>
      <button class="primary-button" :disabled="busy"><Save />{{ busy ? '保存中…' : '保存设置' }}</button>
    </form>
    <section class="settings-section shortcuts"><h2>键盘操作</h2><p><kbd>Space</kbd> 播放 / 暂停 <kbd>Ctrl F</kbd> 搜索 <kbd>Esc</kbd> 关闭面板 <kbd>Ctrl Q</kbd> 退出应用</p><p class="muted">关闭主窗口后音乐继续播放。通过托盘或再次启动应用恢复窗口。</p></section>
  </section>
</template>

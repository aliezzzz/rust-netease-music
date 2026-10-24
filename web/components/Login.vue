<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { X, RefreshCw, QrCode, Smartphone } from 'lucide-vue-next';
import { ui, call, notify, refreshUser } from '../store';
import type { User } from '../types';
const dialog = ref<HTMLDialogElement>(); const mode = ref('qr'); const qr = ref(''); const message = ref('正在生成二维码…');
const country = ref('86'); const phone = ref(''); const captcha = ref(''); const busy = ref(false);
let timer: ReturnType<typeof setTimeout>; let disposed = false; let generation = 0;
async function createQr() {
  const token = ++generation; clearTimeout(timer); qr.value = ''; message.value = '正在生成二维码…';
  try { const result = await call<{ key: string; svg: string }>('account', { operation: 'qr' });
    if (disposed || token !== generation) return;
    qr.value = `data:image/svg+xml,${encodeURIComponent(result.svg)}`; message.value = '使用网易云音乐 App 扫码登录';
    timer = setTimeout(() => checkQr(result.key, token), 1800);
  } catch (e) { if (!disposed) message.value = String(e); }
}
async function checkQr(key: string, token: number) {
  if (disposed || token !== generation) return;
  try {
    const result = await call<{ code: number; msg: string }>('account', { operation: 'qr_check', args: { key } });
    if (disposed || token !== generation) return;
    if (result.code === 803) { await refreshUser(); ui.login = false; notify('登录成功'); return; }
    if (result.code === 800) { message.value = '二维码已过期，请刷新'; return; }
    message.value = result.code === 802 ? '已扫码，请在手机上确认' : '使用网易云音乐 App 扫码登录';
    timer = setTimeout(() => checkQr(key, token), 1800);
  } catch (e) { message.value = String(e); }
}
async function sendCaptcha() {
  if (!phone.value.trim()) { message.value = '请输入手机号'; return; }
  busy.value = true;
  try { await call('account', { operation: 'captcha', args: { country: country.value, phone: phone.value } }); message.value = '验证码已发送'; }
  catch (e) { message.value = String(e); } finally { busy.value = false; }
}
async function login() {
  busy.value = true;
  try {
    const result = await call<User>('account', { operation: 'login', args: { country: country.value, phone: phone.value, captcha: captcha.value } });
    if (result.code !== 200) throw new Error(result.msg || '登录失败');
    await refreshUser(); ui.login = false; notify('登录成功');
  } catch (e) { message.value = String(e); } finally { busy.value = false; }
}
function switchMode() { mode.value = mode.value === 'qr' ? 'phone' : 'qr'; clearTimeout(timer); generation++; if (mode.value === 'qr') void createQr(); else message.value = '使用手机号与验证码登录'; }
onMounted(() => { dialog.value?.showModal(); void createQr(); });
onUnmounted(() => { disposed = true; clearTimeout(timer); generation++; });
</script>
<template>
  <dialog ref="dialog" class="login-dialog" aria-labelledby="login-title" @cancel.prevent="ui.login = false">
    <button class="icon-button dialog-close" aria-label="关闭登录" @click="ui.login = false"><X /></button><h2 id="login-title">让喜欢，与你同步。</h2><p class="muted">登录网易云音乐</p>
    <div v-if="mode === 'qr'" class="qr-login"><div class="qr-image"><img v-if="qr" :src="qr" alt="登录二维码" /><QrCode v-else /></div><button class="text-button" @click="createQr"><RefreshCw />刷新二维码</button></div>
    <form v-else class="phone-login" @submit.prevent="login"><label>国家 / 地区代码<input v-model="country" class="text-input" aria-label="国家代码" /></label><label>手机号<input v-model="phone" class="text-input" type="tel" autocomplete="tel" required /></label><label>验证码<input v-model="captcha" class="text-input" autocomplete="one-time-code" required /></label><button type="button" class="soft-button" :disabled="busy" @click="sendCaptcha">发送验证码</button><button class="primary-button" :disabled="busy">{{ busy ? '处理中…' : '登录' }}</button></form>
    <p class="login-message" role="status">{{ message }}</p><button class="text-button" @click="switchMode"><Smartphone v-if="mode === 'qr'" /><QrCode v-else />{{ mode === 'qr' ? '使用验证码登录' : '使用扫码登录' }}</button>
  </dialog>
</template>

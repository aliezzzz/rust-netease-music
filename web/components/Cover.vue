<script setup lang="ts">
import { ref, watch } from 'vue';
import { Music2 } from 'lucide-vue-next';
const props = defineProps<{ src?: string; name?: string; size?: number; round?: boolean }>();
const failed = ref(false);
watch(() => props.src, () => { failed.value = false; });
function imageUrl() {
  if (!props.src) return '';
  try {
    const url = new URL(props.src);
    if (!['https:', 'http:', 'data:'].includes(url.protocol)) return '';
    if (url.hostname.endsWith('.music.126.net')) url.searchParams.set('param', `${props.size ?? 400}y${props.size ?? 400}`);
    return url.toString();
  } catch { return ''; }
}
</script>
<template>
  <div class="cover" :class="{ round }">
    <img v-if="src && !failed && imageUrl()" :src="imageUrl()" :alt="name ?? ''" loading="lazy" decoding="async" @error="failed = true" />
    <Music2 v-else aria-hidden="true" />
  </div>
</template>

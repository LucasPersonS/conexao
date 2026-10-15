<script setup lang="ts">
import { computed } from 'vue'
import { pingQuality } from '../lib/format'

const props = defineProps<{ ms: number | null; online: boolean }>()
const quality = computed(() => (props.online ? pingQuality(props.ms) : 'none'))
const bars = computed(() => ({ good: 3, ok: 2, bad: 1, none: 0 })[quality.value])
const title = computed(() =>
  !props.online ? 'Servidor offline' : props.ms == null ? 'Latência desconhecida' : `Latência do servidor: ${props.ms} ms`,
)
</script>

<template>
  <span class="ping" :class="quality" :title="title">
    <i v-for="n in 3" :key="n" :class="{ on: n <= bars }" :style="{ height: `${4 + n * 3}px` }" />
  </span>
</template>

<style scoped>
.ping {
  display: inline-flex;
  align-items: flex-end;
  gap: 2px;
  height: 13px;
}

i {
  width: 3px;
  border-radius: 2px;
  background: currentColor;
  opacity: 0.22;
}

i.on {
  opacity: 1;
}

.good {
  color: var(--lime);
}

.ok {
  color: var(--warn);
}

.bad {
  color: var(--bad);
}

.none {
  color: var(--text-faint);
}
</style>

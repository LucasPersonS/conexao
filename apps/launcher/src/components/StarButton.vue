<script setup lang="ts">
/** Estrela pública do servidor: o jogador marca os que curtiu e o total aparece para todos. */
import { computed } from 'vue'
import { formatNumber } from '../lib/format'
import { stars, toggleStar } from '../lib/stars'
import Icon from './Icon.vue'

const props = defineProps<{ slug: string; count?: number; light?: boolean; big?: boolean }>()
const on = computed(() => stars.mine.has(props.slug))
const total = computed(() => props.count ?? 0)
const label = computed(() => `${formatNumber(total.value)} ${total.value === 1 ? 'estrela' : 'estrelas'}`)
</script>

<template>
  <button
    class="star"
    :class="{ on, light, big }"
    :title="on ? `Tirar estrela (${label})` : `Dar estrela (${label})`"
    :aria-label="on ? 'Tirar estrela' : 'Dar estrela'"
    :aria-pressed="on"
    @click.stop="toggleStar(slug)"
    @keydown.enter.stop
  >
    <Icon name="star" :size="big ? 18 : 16" />
    <span class="count">{{ big ? label : formatNumber(total) }}</span>
  </button>
</template>

<style scoped>
.star {
  height: 36px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 12px 0 10px;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--text-dim);
  background: var(--bg-soft);
  border: 1px solid var(--line);
  transition: color 0.15s, background 0.15s, border-color 0.15s, transform 0.1s;
}

.star.light {
  color: #fff;
  background: rgba(var(--shade), 0.5);
  border-color: rgba(255, 255, 255, 0.18);
  backdrop-filter: blur(6px);
}

.star:hover {
  transform: scale(1.04);
  border-color: var(--lime-line);
}

.star.on {
  color: var(--lime);
  border-color: var(--lime-line);
}

.star.on :deep(svg) {
  fill: currentColor;
}

.star.on :deep(svg) {
  animation: pop 0.25s ease-out;
}

.star.big {
  height: 40px;
  padding: 0 14px 0 12px;
  font-size: 14px;
}

@keyframes pop {
  50% {
    transform: scale(1.3);
  }
}

@media (prefers-reduced-motion: reduce) {
  .star.on :deep(svg) {
    animation: none;
  }
}
</style>

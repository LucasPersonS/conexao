<script setup lang="ts">
import { computed } from 'vue'
import { isFavorite, toggleFavorite } from '../lib/prefs'
import Icon from './Icon.vue'

const props = defineProps<{ slug: string; light?: boolean }>()
const on = computed(() => isFavorite(props.slug))
</script>

<template>
  <button
    class="fav"
    :class="{ on, light }"
    :title="on ? 'Remover dos favoritos' : 'Adicionar aos favoritos'"
    :aria-pressed="on"
    @click.stop="toggleFavorite(slug)"
  >
    <Icon name="heart" :size="18" />
  </button>
</template>

<style scoped>
.fav {
  width: 36px;
  height: 36px;
  display: grid;
  place-items: center;
  border-radius: 10px;
  color: var(--text-dim);
  background: var(--bg-soft);
  border: 1px solid var(--line);
  transition: color 0.15s, background 0.15s, transform 0.1s;
}

.fav.light {
  color: #fff;
  background: rgba(var(--shade), 0.5);
  border-color: rgba(255, 255, 255, 0.18);
  backdrop-filter: blur(6px);
}

.fav:hover {
  transform: scale(1.06);
}

.fav.on {
  color: var(--lime);
  border-color: var(--lime-line);
}

.fav.on :deep(svg) {
  fill: currentColor;
}
</style>

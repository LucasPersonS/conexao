<script setup lang="ts">
/**
 * Banner de patrocinador com rotação. `wide` = faixa 8:1 (home);
 * `side` = 2:1 (lateral do servidor e tela de carregamento), usando a imagem
 * lateral quando o patrocinador tiver uma.
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { resting } from '../lib/activity'
import type { Sponsor, SponsorPlacement } from '../lib/api'
import { loadSponsors, markViewed, openSponsor, weightedOrder } from '../lib/sponsors'

const props = withDefaults(defineProps<{ placement: SponsorPlacement; variant?: 'wide' | 'side'; interval?: number }>(), {
  variant: 'wide',
  interval: 8000,
})

const list = ref<Sponsor[]>([])
const index = ref(0)
const hovering = ref(false)
let timer: number | undefined

const current = computed(() => list.value[index.value] ?? null)
const image = (s: Sponsor) => (props.variant === 'side' && s.image_side_url) || s.image_url
const host = (s: Sponsor) => {
  try {
    return new URL(s.link_url).hostname.replace(/^www\./, '')
  } catch {
    return ''
  }
}

function tick() {
  window.clearTimeout(timer)
  if (list.value.length < 2) return
  timer = window.setTimeout(() => {
    // Sem rodar com o jogo aberto, janela escondida ou mouse em cima.
    if (!resting.value && !hovering.value) index.value = (index.value + 1) % list.value.length
    tick()
  }, props.interval)
}

onMounted(async () => {
  list.value = weightedOrder(await loadSponsors(props.placement))
  tick()
})
onBeforeUnmount(() => window.clearTimeout(timer))

// Conta como visto só o que apareceu de verdade e com a janela visível.
watch(
  [current, resting],
  ([s, rest]) => {
    if (s && !rest) markViewed(props.placement, s.id)
  },
  { immediate: true },
)

function go(i: number) {
  index.value = i
  tick()
}
</script>

<template>
  <aside
    v-if="current"
    class="sponsor"
    :class="variant"
    aria-label="Patrocinador"
    @mouseenter="hovering = true"
    @mouseleave="hovering = false"
  >
    <button
      class="art"
      :title="`${current.name} — abre ${host(current)} no navegador`"
      @click="openSponsor(placement, current)"
    >
      <Transition name="fade" mode="out-in">
        <img :key="current.id" :src="image(current)" :alt="current.alt_text ?? current.name" loading="lazy" draggable="false" />
      </Transition>
    </button>
    <div class="meta">
      <span class="tag">Patrocinado</span>
      <span class="name">{{ current.name }}</span>
      <span v-if="list.length > 1" class="dots" role="tablist" aria-label="Trocar patrocinador">
        <button
          v-for="(s, i) in list"
          :key="s.id"
          class="dot-btn"
          :class="{ on: i === index }"
          role="tab"
          :aria-selected="i === index"
          :aria-label="s.name"
          @click="go(i)"
        />
      </span>
    </div>
  </aside>
</template>

<style scoped>
.sponsor {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.art {
  display: block;
  width: 100%;
  padding: 0;
  overflow: hidden;
  border-radius: var(--radius);
  border: 1px solid var(--line);
  background: var(--bg-card);
  transition: border-color 0.15s, box-shadow 0.15s;
}

.art:hover {
  border-color: var(--lime-line);
  box-shadow: var(--lime-glow);
}

.wide .art {
  aspect-ratio: 8 / 1;
}

.side .art {
  aspect-ratio: 2 / 1;
}

img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.meta {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 18px;
  font-size: 12px;
  color: var(--text-faint);
}

.tag {
  padding: 1px 7px;
  border-radius: 999px;
  border: 1px solid var(--line-strong);
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dots {
  margin-left: auto;
  display: flex;
  gap: 5px;
}

.dot-btn {
  width: 7px;
  height: 7px;
  padding: 0;
  border-radius: 50%;
  background: var(--line-strong);
}

.dot-btn.on {
  background: var(--lime);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.35s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>

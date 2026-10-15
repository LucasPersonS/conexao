<script setup lang="ts">
import { resting } from '../lib/activity'
/** Vitrine da home: um servidor grande por vez + faixa de miniaturas (estilo FiveM). */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { ServerSummary } from '../lib/api'
import { bgImage, formatNumber } from '../lib/format'
import FavoriteButton from './FavoriteButton.vue'
import Icon from './Icon.vue'
import PingBars from './PingBars.vue'
import ServerIcon from './ServerIcon.vue'

const props = defineProps<{ servers: ServerSummary[] }>()
const emit = defineEmits<{ open: [slug: string] }>()

const index = ref(0)
const paused = ref(false)
const current = computed(() => props.servers[index.value] ?? props.servers[0])
let timer: number | undefined

watch(
  () => props.servers.length,
  (n) => {
    if (index.value >= n) index.value = 0
  },
)

onMounted(() => {
  timer = window.setInterval(() => {
    if (!paused.value && !resting.value && props.servers.length > 1) index.value = (index.value + 1) % props.servers.length
  }, 8000)
})
onBeforeUnmount(() => window.clearInterval(timer))
</script>

<template>
  <section v-if="current" class="spotlight" @mouseenter="paused = true" @mouseleave="paused = false">
    <Transition name="fade" mode="out-in">
      <div
        :key="current.slug"
        class="stage"
        :style="bgImage(current.banner_url)"
      >
        <div class="shade" />
        <div class="content">
          <ServerIcon :name="current.name" :url="current.icon_url" :size="104" class="icon" />
          <div class="info">
            <span v-if="current.featured" class="badge"><Icon name="star" :size="12" /> Destaque</span>
            <h2>{{ current.name }}</h2>
            <p class="desc">{{ current.description || 'Servidor parceiro do Conexão' }}</p>
            <div class="meta">
              <span class="players">
                <span class="dot" :class="{ on: current.status.online }" />
                <template v-if="current.status.online">
                  <strong>{{ formatNumber(current.status.players_online) }}</strong> /
                  {{ formatNumber(current.status.players_max) }} jogando
                </template>
                <template v-else>Offline</template>
              </span>
              <PingBars :ms="current.status.latency_ms" :online="current.status.online" />
              <span v-for="tag in current.tags.slice(0, 3)" :key="tag" class="tag">{{ tag }}</span>
            </div>
            <div class="actions">
              <button class="btn btn-primary connect" @click="emit('open', current.slug)">
                <Icon name="play" :size="14" /> Conectar
              </button>
              <FavoriteButton :slug="current.slug" light />
            </div>
          </div>
        </div>
      </div>
    </Transition>

    <div v-if="servers.length > 1" class="strip">
      <button
        v-for="(server, i) in servers"
        :key="server.slug"
        class="thumb"
        :class="{ active: i === index }"
        :title="server.name"
        @click="index = i"
        @dblclick="emit('open', server.slug)"
      >
        <ServerIcon :name="server.name" :url="server.icon_url" :size="34" />
        <span v-if="i === index" class="thumb-name">{{ server.name }}</span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.spotlight {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.stage {
  position: relative;
  min-height: 260px;
  border-radius: 18px;
  overflow: hidden;
  background-color: #0b1113;
  background-image:
    radial-gradient(circle at 78% 18%, rgba(183, 254, 0, 0.28), transparent 55%),
    linear-gradient(rgba(183, 254, 0, 0.07) 1px, transparent 1px),
    linear-gradient(90deg, rgba(183, 254, 0, 0.07) 1px, transparent 1px),
    linear-gradient(135deg, #15241a, #0b1113);
  /* Com banner, só o primeiro valor vale (cover/center). */
  background-size: cover, 32px 32px, 32px 32px, cover;
  background-position: center, 0 0, 0 0, center;
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
}

.shade {
  position: absolute;
  inset: 0;
  background:
    linear-gradient(90deg, rgba(var(--shade), 0.95) 0%, rgba(var(--shade), 0.72) 45%, rgba(var(--shade), 0.1) 100%),
    linear-gradient(0deg, rgba(var(--shade), 0.5), transparent 40%);
}

.content {
  position: relative;
  display: flex;
  gap: 24px;
  padding: 28px 32px;
  color: #fff;
}

.icon {
  border: 3px solid var(--lime);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4), 0 0 26px rgba(183, 254, 0, 0.3);
}

.info {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
  max-width: 560px;
}

.badge {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 10px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 700;
  background: var(--lime);
  color: var(--ink);
  box-shadow: 0 0 14px rgba(183, 254, 0, 0.45);
}

.badge :deep(svg) {
  fill: currentColor;
}

h2 {
  margin: 0;
  font-size: 34px;
  line-height: 1.05;
  text-shadow: 0 2px 18px rgba(0, 0, 0, 0.5);
}

.desc {
  margin: 0;
  font-size: 14px;
  line-height: 1.5;
  color: rgba(255, 255, 255, 0.85);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.9);
}

.players {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.tag {
  padding: 2px 10px;
  border-radius: 999px;
  font-size: 12px;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.16);
}

.actions {
  display: flex;
  gap: 10px;
  margin-top: 6px;
}

.connect {
  height: 44px;
  padding: 0 28px;
  font-size: 15px;
  box-shadow: 0 0 24px rgba(183, 254, 0, 0.3);
}

.strip {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  padding: 2px 2px 6px;
}

.thumb {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px;
  border-radius: 12px;
  background: var(--bg-card);
  border: 1px solid var(--line);
  flex: none;
  transition: border-color 0.15s, background 0.15s;
}

.thumb:hover {
  background: var(--bg-hover);
}

.thumb.active {
  border-color: var(--lime);
  box-shadow: 0 0 0 3px var(--lime-soft), 0 0 18px rgba(183, 254, 0, 0.15);
  padding-right: 14px;
}

.thumb-name {
  font-size: 13px;
  font-weight: 600;
  max-width: 220px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>

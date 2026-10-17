<script setup lang="ts">
import type { ServerSummary } from '../lib/api'
import { bgImage, formatNumber } from '../lib/format'
import FavoriteButton from './FavoriteButton.vue'
import PingBars from './PingBars.vue'
import ServerIcon from './ServerIcon.vue'
import StarButton from './StarButton.vue'

const props = defineProps<{ server: ServerSummary }>()
defineEmits<{ open: [] }>()

// Até 2 estilos cabem no rodapé; com mais, mostra o primeiro e "+N".
const shownTags = () => (props.server.tags.length <= 2 ? props.server.tags : props.server.tags.slice(0, 1))
const extraTags = () => props.server.tags.length - shownTags().length
</script>

<template>
  <article class="card" :class="{ offline: !server.status.online }" tabindex="0" @click="$emit('open')" @keydown.enter="$emit('open')">
    <div class="banner" :style="bgImage(server.banner_url)">
      <span v-if="server.featured" class="badge featured">Destaque</span>
      <span v-if="!server.status.online" class="badge off">Offline</span>
      <div class="actions">
        <StarButton :slug="server.slug" :count="server.stars" light />
        <FavoriteButton class="fav" :slug="server.slug" light />
      </div>
      <ServerIcon class="icon" :name="server.name" :url="server.icon_url" :size="56" />
    </div>
    <div class="body">
      <h3 :title="server.name">{{ server.name }}</h3>
      <p class="desc">{{ server.description || 'Servidor parceiro do Conexão' }}</p>
      <div class="foot">
        <span class="players" :title="`${server.status.players_online} de ${server.status.players_max} jogadores`">
          <span class="dot" :class="{ on: server.status.online }" />
          <template v-if="server.status.online">
            <strong>{{ formatNumber(server.status.players_online) }}</strong>
            <span class="max">/{{ formatNumber(server.status.players_max) }}</span>
          </template>
          <span v-else class="max">Offline</span>
        </span>
        <PingBars :ms="server.status.latency_ms" :online="server.status.online" />
        <span class="tags">
          <span v-for="tag in shownTags()" :key="tag" class="chip">{{ tag }}</span>
          <span v-if="extraTags()" class="chip more">+{{ extraTags() }}</span>
        </span>
      </div>
    </div>
  </article>
</template>

<style scoped>
.card {
  display: flex;
  flex-direction: column;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
  overflow: hidden;
  cursor: pointer;
  transition: transform 0.15s, box-shadow 0.15s, border-color 0.15s;
}

.card:hover,
.card:focus-visible {
  transform: translateY(-3px);
  border-color: var(--lime-line);
  box-shadow: var(--shadow-pop), 0 0 0 1px rgba(183, 254, 0, 0.25), 0 0 30px rgba(183, 254, 0, 0.08);
  outline: none;
}

.banner {
  position: relative;
  height: 112px;
  background-color: #0b1113;
  background-image:
    radial-gradient(circle at 78% 18%, rgba(183, 254, 0, 0.28), transparent 55%),
    linear-gradient(rgba(183, 254, 0, 0.07) 1px, transparent 1px),
    linear-gradient(90deg, rgba(183, 254, 0, 0.07) 1px, transparent 1px),
    linear-gradient(135deg, #15241a, #0b1113);
  /* Com banner, só o primeiro valor vale (cover/center). */
  background-size: cover, 22px 22px, 22px 22px, cover;
  background-position: center, 0 0, 0 0, center;
}

.banner::after {
  content: '';
  position: absolute;
  inset: 0;
  background: linear-gradient(to bottom, rgba(var(--shade), 0.35), transparent 45%, rgba(var(--shade), 0.55));
}

.offline .banner {
  filter: grayscale(0.9) brightness(0.7);
}

.badge {
  position: absolute;
  top: 10px;
  left: 10px;
  z-index: 1;
  padding: 3px 9px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.featured {
  background: var(--lime);
  color: var(--ink);
  box-shadow: 0 0 14px rgba(183, 254, 0, 0.45);
}

.off {
  background: rgba(var(--shade), 0.8);
  border: 1px solid var(--line-strong);
  color: var(--text-dim);
}

.featured + .off {
  left: 86px;
}

.actions {
  position: absolute;
  top: 8px;
  right: 8px;
  z-index: 1;
  display: flex;
  gap: 6px;
}

.actions .fav,
.actions .star {
  height: 32px;
}

.actions .fav {
  width: 32px;
}

.icon {
  position: absolute;
  left: 14px;
  bottom: -24px;
  z-index: 1;
  border: 3px solid var(--bg-card);
  box-shadow: var(--shadow);
}

.body {
  padding: 30px 14px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
}

h3 {
  margin: 0;
  font-size: 17px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.desc {
  margin: 0;
  font-size: 13px;
  line-height: 1.45;
  color: var(--text-dim);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  min-height: 38px;
}

.foot {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
  min-width: 0;
}

.players {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}

.max {
  color: var(--text-faint);
}

.tags {
  flex: 1;
  min-width: 0;
  display: flex;
  justify-content: flex-end;
  gap: 4px;
  overflow: hidden;
}

.chip {
  height: 22px;
  font-size: 11px;
  white-space: nowrap;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  display: inline-block;
  line-height: 20px;
}

.chip.more {
  flex: none;
}
</style>

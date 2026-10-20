<script setup lang="ts">
import { computed } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import centralLogo from '../assets/central.webp'
import Icon from '../components/Icon.vue'
import PlayButton from '../components/PlayButton.vue'
import SponsorBanner from '../components/SponsorBanner.vue'
import ServerCard from '../components/ServerCard.vue'
import ServerIcon from '../components/ServerIcon.vue'
import Spotlight from '../components/Spotlight.vue'
import UpcomingEvents from '../components/UpcomingEvents.vue'
import type { ServerSummary } from '../lib/api'
import { bgImage, formatNumber, timeAgo } from '../lib/format'
import { prefs } from '../lib/prefs'
import { allTags, CENTRAL_PAGE, centralServers, findServer, go, openServer, sortServers, store, totalPlayers } from '../lib/store'

const recent = computed(() =>
  prefs.recent
    .map((r) => ({ server: findServer(r.slug), at: r.at }))
    .filter((r): r is { server: ServerSummary; at: number } => r.server !== null),
)
const last = computed(() => recent.value[0]?.server ?? null)
const favorites = computed(() => store.servers.filter((s) => prefs.favorites.includes(s.slug)))

/** Destaques pagos primeiro; completa com os mais cheios para a vitrine nunca ficar vazia. */
const spotlight = computed(() => {
  const featured = store.servers.filter((s) => s.featured)
  const rest = sortServers(
    store.servers.filter((s) => !s.featured && s.status.online),
    'players',
  )
  return [...featured, ...rest].slice(0, 8)
})
const popular = computed(() => sortServers(store.servers, 'players').slice(0, 4))
const central = computed(() => sortServers(centralServers.value, 'players'))
const centralPlayers = computed(() => central.value.reduce((n, s) => n + (s.status.online ? s.status.players_online : 0), 0))
</script>

<template>
  <div class="home">
    <div class="top">
      <section
        v-if="last"
        class="quick continue"
        :style="bgImage(last.banner_url)"
      >
        <div class="shade" />
        <div class="quick-body">
          <span class="eyebrow">Continuar jogando</span>
          <div class="quick-server" @click="openServer(last.slug)">
            <ServerIcon :name="last.name" :url="last.icon_url" :size="56" class="quick-icon" />
            <div>
              <h2>{{ last.name }}</h2>
              <span class="quick-meta">
                <span class="dot" :class="{ on: last.status.online }" />
                {{ last.status.online ? `${formatNumber(last.status.players_online)} jogando agora` : 'Offline' }}
              </span>
            </div>
          </div>
          <div class="quick-actions">
            <PlayButton :server="last" compact class="quick-play" />
            <button class="btn ghost" @click="openServer(last.slug)">Ver servidor</button>
          </div>
        </div>
      </section>
      <button v-else class="quick explore" @click="go('browse')">
        <span class="play-circle"><Icon name="play" :size="26" /></span>
        <span>
          <strong>Jogar</strong>
          <small>Explore {{ store.servers.length }} servidores parceiros · {{ formatNumber(totalPlayers) }} jogando agora</small>
        </span>
        <Icon name="arrow" :size="22" class="go" />
      </button>

      <div class="side">
        <button class="mini" @click="go('browse', { favorites: true })">
          <span class="mini-head"><Icon name="heart" /> Favoritos <b>{{ favorites.length }}</b></span>
          <span v-if="favorites.length" class="faces">
            <ServerIcon v-for="s in favorites.slice(0, 5)" :key="s.slug" :name="s.name" :url="s.icon_url" :size="30" />
          </span>
          <small v-else>Toque no coração de um servidor para guardá-lo aqui.</small>
        </button>
        <div class="mini history">
          <span class="mini-head"><Icon name="clock" /> Histórico</span>
          <button v-for="r in recent.slice(0, 3)" :key="r.server.slug" class="hist" @click="openServer(r.server.slug)">
            <ServerIcon :name="r.server.name" :url="r.server.icon_url" :size="26" />
            <span class="hist-name">{{ r.server.name }}</span>
            <small>{{ timeAgo(r.at) }}</small>
          </button>
          <small v-if="!recent.length">Os servidores em que você jogar aparecem aqui.</small>
        </div>
      </div>
    </div>

    <template v-if="spotlight.length">
      <div class="section-head">
        <h3>Em destaque</h3>
      </div>
      <Spotlight :servers="spotlight" @open="openServer" />
    </template>

    <!-- Parceria Central SMP's: comunidade parceira oficial, com os SMPs do programa. -->
    <section class="central-strip">
      <div class="central-brand">
        <img :src="centralLogo" alt="Central SMP's" class="central-logo" />
        <div>
          <span class="central-eyebrow">Comunidade parceira oficial</span>
          <h3>SMPs da Central</h3>
          <p>
            A Central SMP's é onde os SMPs se divulgam e conversam entre si.
            <template v-if="central.length">
              {{ central.length }} {{ central.length === 1 ? 'servidor' : 'servidores' }} do programa no Conexão ·
              {{ formatNumber(centralPlayers) }} jogando agora
            </template>
            <template v-else>Os primeiros SMPs do programa chegam em breve.</template>
          </p>
        </div>
      </div>
      <div v-if="central.length" class="central-servers">
        <button v-for="s in central.slice(0, 5)" :key="s.slug" class="central-server" :title="s.name" @click="openServer(s.slug)">
          <ServerIcon :name="s.name" :url="s.icon_url" :size="40" />
          <span class="central-name">{{ s.name }}</span>
          <small><span class="dot" :class="{ on: s.status.online }" />{{ s.status.online ? formatNumber(s.status.players_online) : 'off' }}</small>
        </button>
      </div>
      <div class="central-actions">
        <button v-if="central.length" class="btn central-btn" @click="go('browse', { central: true })">Ver SMPs da Central</button>
        <button class="btn ghost" @click="openUrl(CENTRAL_PAGE)">Conhecer a parceria</button>
      </div>
    </section>

    <SponsorBanner placement="home" variant="wide" class="sponsor-strip" />

    <UpcomingEvents />

    <div class="section-head">
      <h3>Mais jogados agora</h3>
      <button class="see-all" @click="go('browse')">Ver todos os servidores <Icon name="arrow" :size="15" /></button>
    </div>
    <p v-if="store.loading && !store.servers.length" class="empty">Carregando servidores…</p>
    <p v-else-if="!store.servers.length" class="empty">Nenhum servidor parceiro publicado ainda.</p>
    <div class="grid">
      <ServerCard v-for="server in popular" :key="server.slug" :server="server" @open="openServer(server.slug)" />
    </div>

    <template v-if="allTags.length">
      <div class="section-head">
        <h3>Explorar por estilo</h3>
      </div>
      <div class="categories">
        <button v-for="tag in allTags.slice(0, 10)" :key="tag" class="category" @click="go('browse', { tag })">
          <Icon name="tag" :size="16" />
          {{ tag }}
          <small>{{ store.servers.filter((s) => s.tags.includes(tag)).length }}</small>
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.central-strip {
  position: relative;
  display: grid;
  grid-template-columns: minmax(260px, 1.1fr) minmax(0, 1.6fr) auto;
  align-items: center;
  gap: 18px;
  padding: 16px 18px;
  border-radius: var(--radius);
  border: 1px solid rgba(124, 192, 247, 0.4);
  background:
    linear-gradient(rgba(124, 192, 247, 0.06) 1px, transparent 1px),
    linear-gradient(90deg, rgba(124, 192, 247, 0.06) 1px, transparent 1px),
    radial-gradient(520px 200px at 0% 50%, rgba(59, 154, 232, 0.24), transparent 70%),
    var(--bg-card);
  background-size: 22px 22px, 22px 22px, auto, auto;
  overflow: hidden;
}

.central-brand {
  display: flex;
  align-items: center;
  gap: 14px;
  min-width: 0;
}

.central-logo {
  flex: none;
  width: 64px;
  height: 64px;
  border-radius: 18px;
  background: #fff;
  border: 2px solid #3b9ae8;
  box-shadow: 0 0 0 4px rgba(59, 154, 232, 0.18), 0 10px 26px rgba(59, 154, 232, 0.3);
}

.central-eyebrow {
  font-family: var(--font-pixel);
  font-size: 10px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: #7cc0f7;
}

.central-brand h3 {
  margin: 2px 0;
  font-size: 20px;
}

.central-brand p {
  margin: 0;
  font-size: 13px;
  line-height: 1.4;
  color: var(--text-dim);
}

.central-servers {
  display: flex;
  gap: 8px;
  min-width: 0;
  overflow: hidden;
}

.central-server {
  flex: 1 1 0;
  min-width: 82px;
  max-width: 120px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 10px 6px 8px;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: rgba(var(--shade), 0.5);
  color: var(--text);
  cursor: pointer;
  transition: border-color 0.15s, transform 0.15s;
}

.central-server:hover {
  border-color: #7cc0f7;
  transform: translateY(-2px);
}

.central-name {
  max-width: 100%;
  font-size: 12px;
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.central-server small {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--text-dim);
}

.central-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.central-btn {
  background: #3b9ae8;
  border-color: #7cc0f7;
  color: #fff;
}

.central-btn:hover {
  background: #4fa8f0;
}

@media (max-width: 1100px) {
  .central-strip {
    grid-template-columns: 1fr;
  }

  .central-actions {
    flex-direction: row;
  }
}

.sponsor-strip {
  margin-top: 8px;
}

.home {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.top {
  display: grid;
  grid-template-columns: minmax(0, 2fr) minmax(240px, 1fr);
  gap: 16px;
}

.quick {
  position: relative;
  min-height: 190px;
  border-radius: 18px;
  overflow: hidden;
  text-align: left;
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
}

.continue {
  background-color: #0b1113;
  background-image:
    radial-gradient(circle at 80% 15%, rgba(183, 254, 0, 0.3), transparent 55%),
    linear-gradient(rgba(183, 254, 0, 0.07) 1px, transparent 1px),
    linear-gradient(90deg, rgba(183, 254, 0, 0.07) 1px, transparent 1px),
    linear-gradient(135deg, #15241a, #0b1113);
  background-size: cover, 32px 32px, 32px 32px, cover;
  background-position: center, 0 0, 0 0, center;
  color: #fff;
}

.continue .shade {
  position: absolute;
  inset: 0;
  background: linear-gradient(90deg, rgba(var(--shade), 0.94), rgba(var(--shade), 0.5));
}

.quick-body {
  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 14px;
  padding: 22px 24px;
}

.quick .eyebrow {
  font-family: var(--font-pixel);
  font-size: 11px;
  letter-spacing: 0.08em;
  color: var(--lime);
}

.quick-server {
  display: flex;
  align-items: center;
  gap: 14px;
  cursor: pointer;
}

.quick-icon {
  border: 2px solid var(--lime);
  box-shadow: 0 0 18px rgba(183, 254, 0, 0.35);
}

.quick-server h2 {
  margin: 0 0 4px;
  font-size: 26px;
}

.quick-meta {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.85);
}

.quick-actions {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}

.quick-play {
  width: 280px;
}

.ghost {
  height: 44px;
  background: rgba(255, 255, 255, 0.06);
  color: #fff;
  border: 1px solid rgba(255, 255, 255, 0.22);
  backdrop-filter: blur(6px);
}

.ghost:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.12);
  border-color: var(--lime-line);
}

.explore {
  display: flex;
  align-items: center;
  gap: 20px;
  padding: 28px;
  background:
    radial-gradient(circle at 15% 50%, rgba(183, 254, 0, 0.16), transparent 45%),
    linear-gradient(120deg, var(--bg-card), var(--bg-soft));
  transition: border-color 0.15s, transform 0.15s, box-shadow 0.15s;
}

.explore:hover {
  border-color: var(--lime-line);
  transform: translateY(-2px);
  box-shadow: var(--shadow-pop), 0 0 30px rgba(183, 254, 0, 0.1);
}

.play-circle {
  width: 64px;
  height: 64px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  background: var(--lime);
  color: var(--ink);
  box-shadow: 0 0 0 6px var(--lime-soft), 0 0 30px rgba(183, 254, 0, 0.5);
  flex: none;
}

.explore strong {
  display: block;
  font-family: var(--font-display);
  font-size: 36px;
  font-weight: 700;
  color: var(--lime);
}

.explore small {
  color: var(--text-dim);
  font-size: 14px;
}

.explore .go {
  margin-left: auto;
  color: var(--lime);
}

.side {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.mini {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 16px;
  text-align: left;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
}

button.mini:hover {
  border-color: var(--lime-line);
}

.history {
  flex: 1;
}

.mini-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: var(--font-display);
  font-weight: 600;
  font-size: 15px;
}

.mini-head :deep(svg) {
  color: var(--lime);
}

.mini-head b {
  margin-left: auto;
  font-family: 'Inter', sans-serif;
  font-size: 12px;
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--lime-soft);
  color: var(--lime);
}

.mini small {
  color: var(--text-faint);
  font-size: 12px;
  line-height: 1.4;
}

.faces {
  display: flex;
  gap: 6px;
}

.hist {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px;
  margin: -2px -4px;
  border-radius: 8px;
  text-align: left;
}

.hist:hover {
  background: var(--bg-hover);
}

.hist-name {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 10px;
}

h3 {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0;
  font-size: 19px;
}

/* Blocos de pixel do logo antes de cada seção */
h3::before {
  content: '';
  width: 8px;
  height: 8px;
  background: var(--lime);
  box-shadow: 10px -6px 0 -1px rgba(183, 254, 0, 0.55), 0 0 10px rgba(183, 254, 0, 0.6);
  margin-right: 8px;
}

.see-all {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--lime);
}

.see-all:hover {
  color: var(--lime-hi);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  gap: 16px;
}

.empty {
  color: var(--text-dim);
  margin: 0;
}

.categories {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.category {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 40px;
  padding: 0 16px;
  border-radius: 12px;
  font-weight: 600;
  font-size: 14px;
  background: var(--bg-card);
  border: 1px solid var(--line);
  color: var(--text);
}

.category:hover {
  border-color: var(--lime-line);
  background: var(--bg-hover);
}

.category :deep(svg) {
  color: var(--lime);
}

.category small {
  color: var(--text-faint);
  font-weight: 500;
}

@media (max-width: 980px) {
  .top {
    grid-template-columns: 1fr;
  }
}
</style>

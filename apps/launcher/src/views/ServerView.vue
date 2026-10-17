<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import FavoriteButton from '../components/FavoriteButton.vue'
import Icon from '../components/Icon.vue'
import PingBars from '../components/PingBars.vue'
import PlayButton from '../components/PlayButton.vue'
import ServerIcon from '../components/ServerIcon.vue'
import ServerComments from '../components/ServerComments.vue'
import ServerNews from '../components/ServerNews.vue'
import StarButton from '../components/StarButton.vue'
import { isFull, slots, toggleSlotWatch } from '../lib/slots'
import { api, errorMessage, LOADER_NAMES, type InstallState, type ServerSummary } from '../lib/api'
import { bgImage, formatBytes, formatNumber } from '../lib/format'
import { go, store } from '../lib/store'

const props = defineProps<{ server: ServerSummary }>()

const install = ref<InstallState | null>(null)
const folderError = ref<string | null>(null)
const copied = ref(false)

watch(
  () => props.server.slug,
  async (slug) => {
    install.value = null
    try {
      install.value = await api.installState(slug)
    } catch {
      install.value = null
    }
  },
  { immediate: true },
)

const size = computed(() => formatBytes(props.server.pack_size))
const backLabel = computed(() => ({ home: 'Início', browse: 'Servidores', settings: 'Configurações', profile: 'Meu perfil', server: 'Voltar' })[store.back])

async function openFolder() {
  folderError.value = null
  try {
    await api.openGameFolder(props.server.slug)
  } catch (e) {
    folderError.value = errorMessage(e)
  }
}

async function copyAddress() {
  try {
    await navigator.clipboard.writeText(props.server.address)
    copied.value = true
    window.setTimeout(() => (copied.value = false), 1500)
  } catch {
    // área de transferência indisponível
  }
}
</script>

<template>
  <div class="server">
    <button class="back" @click="store.view = store.back === 'server' ? 'home' : store.back">
      <Icon name="back" :size="16" /> {{ backLabel }}
    </button>

    <header class="hero" :style="bgImage(server.banner_url)">
      <div class="shade" />
      <div class="hero-body">
        <ServerIcon :name="server.name" :url="server.icon_url" :size="96" class="icon" />
        <div class="title">
          <span v-if="server.featured" class="badge"><Icon name="star" :size="12" /> Destaque</span>
          <h1>{{ server.name }}</h1>
          <div class="meta">
            <span class="players">
              <span class="dot" :class="{ on: server.status.online }" />
              <template v-if="server.status.online">
                <strong>{{ formatNumber(server.status.players_online) }}</strong> /
                {{ formatNumber(server.status.players_max) }} jogadores
              </template>
              <template v-else>Offline no momento</template>
            </span>
            <PingBars :ms="server.status.latency_ms" :online="server.status.online" />
          </div>
        </div>
        <div class="hero-actions">
          <StarButton :slug="server.slug" :count="server.stars" light big />
          <FavoriteButton :slug="server.slug" light class="fav" />
        </div>
      </div>
    </header>

    <div class="columns">
      <div class="main">
        <section class="card">
          <h3>Sobre o servidor</h3>
          <p class="description">{{ server.description || 'Este servidor ainda não escreveu uma descrição.' }}</p>
          <div v-if="server.tags.length" class="tags">
            <button v-for="tag in server.tags" :key="tag" class="chip tag" @click="go('browse', { tag })">{{ tag }}</button>
          </div>
        </section>

        <ServerNews :server="server" />

        <section class="card how">
          <h3>Como entrar</h3>
          <ol>
            <li :class="{ done: !server.whitelist_required }">
              <b>{{ server.whitelist_required ? 'Peça whitelist' : 'Entrada aberta' }}</b>
              <span>{{
                server.whitelist_required
                  ? 'Clique em Pedir whitelist e aguarde a equipe aprovar. Acompanhe pelo Discord do servidor.'
                  : server.allow_offline
                    ? 'Qualquer jogador pode entrar, com ou sem Minecraft original.'
                    : 'Qualquer jogador com Minecraft original pode entrar.'
              }}</span>
            </li>
            <li v-if="server.allow_offline" class="done">
              <b>Aceita contas não originais</b>
              <span>Quem entrou com o Discord também joga. Entre sempre pelo Conexão: o servidor só libera quem vem pelo launcher.</span>
            </li>
            <li>
              <b>Instale o modpack</b>
              <span>O Conexão baixa sozinho o Java, o {{ LOADER_NAMES[server.loader.kind] }} e os mods{{ size ? ` (${size})` : '' }}.</span>
            </li>
            <li>
              <b>Jogue</b>
              <span>O Minecraft abre já conectando no servidor. Nas próximas vezes, só as novidades são baixadas.</span>
            </li>
          </ol>
        </section>

        <ServerComments :server="server" />
      </div>

      <aside class="side">
        <section class="card play-card">
          <PlayButton :server="server" />
          <div v-if="isFull(server) || slots.watching.has(server.slug)" class="full">
            <span>{{ slots.watching.has(server.slug) ? 'Esperando vaga… avisamos aqui e no Windows.' : 'Servidor lotado agora.' }}</span>
            <button class="btn" :class="{ 'btn-primary': !slots.watching.has(server.slug) }" @click="toggleSlotWatch(server)">
              <Icon name="bell" :size="16" /> {{ slots.watching.has(server.slug) ? 'Cancelar aviso' : 'Avise-me quando abrir vaga' }}
            </button>
          </div>
          <button v-if="server.discord_url" class="btn discord" @click="openUrl(server.discord_url)">
            Entrar no Discord
          </button>
        </section>

        <details class="card tech">
          <summary>Detalhes técnicos <Icon name="chevron" :size="16" /></summary>
          <dl>
            <dt>Minecraft</dt>
            <dd>{{ server.minecraft }}</dd>
            <dt>Loader</dt>
            <dd>{{ LOADER_NAMES[server.loader.kind] }} {{ server.loader.version }}</dd>
            <dt>Modpack</dt>
            <dd>{{ server.current_version ?? 'não publicado' }}</dd>
            <dt>Instalado</dt>
            <dd>{{ install?.installed ? `versão ${install.version}` : 'não' }}</dd>
            <template v-if="size">
              <dt>Tamanho</dt>
              <dd>{{ size }}</dd>
            </template>
            <dt>Endereço</dt>
            <dd>
              <button class="addr" title="Copiar" @click="copyAddress">{{ copied ? 'Copiado!' : server.address }}</button>
            </dd>
          </dl>
          <button v-if="install?.installed" class="btn folder" @click="openFolder">
            <Icon name="folder" :size="16" /> Abrir pasta do jogo
          </button>
          <p v-if="folderError" class="err">{{ folderError }}</p>
        </details>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.full {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
  border-radius: 12px;
  background: var(--warn-bg);
  border: 1px solid rgba(255, 201, 74, 0.3);
  color: var(--warn);
  font-size: 13px;
  font-weight: 600;
}

.full .btn {
  justify-content: center;
}

.server {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.back {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 0 12px 0 8px;
  border-radius: 10px;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-dim);
}

.back:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.hero {
  position: relative;
  border-radius: 18px;
  overflow: hidden;
  min-height: 210px;
  display: flex;
  align-items: flex-end;
  background-color: #0b1113;
  background-image:
    radial-gradient(circle at 80% 15%, rgba(183, 254, 0, 0.3), transparent 55%),
    linear-gradient(rgba(183, 254, 0, 0.07) 1px, transparent 1px),
    linear-gradient(90deg, rgba(183, 254, 0, 0.07) 1px, transparent 1px),
    linear-gradient(135deg, #15241a, #0b1113);
  background-size: cover, 32px 32px, 32px 32px, cover;
  background-position: center, 0 0, 0 0, center;
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
}

.shade {
  position: absolute;
  inset: 0;
  background: linear-gradient(to top, rgba(var(--shade), 0.96) 0%, rgba(var(--shade), 0.4) 70%, rgba(var(--shade), 0.1));
}

.hero-body {
  position: relative;
  width: 100%;
  display: flex;
  align-items: flex-end;
  gap: 20px;
  padding: 24px 28px;
  color: #fff;
}

.hero-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.hero-actions .fav {
  width: 40px;
  height: 40px;
}

.icon {
  border: 3px solid var(--lime);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4), 0 0 24px rgba(183, 254, 0, 0.3);
}

.title {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
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

h1 {
  margin: 0;
  font-size: 38px;
  line-height: 1.05;
  text-shadow: 0 2px 18px rgba(0, 0, 0, 0.5);
}

.meta {
  display: flex;
  align-items: center;
  gap: 14px;
  font-size: 14px;
  color: rgba(255, 255, 255, 0.9);
}

.players {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.columns {
  display: grid;
  grid-template-columns: minmax(0, 1.7fr) minmax(300px, 1fr);
  gap: 16px;
  align-items: start;
}

.main,
.side {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.side {
  position: sticky;
  top: 0;
}

.card {
  padding: 18px 20px;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
}

h3 {
  margin: 0 0 10px;
  font-size: 18px;
}

.description {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-dim);
  white-space: pre-line;
  user-select: text;
}

.tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 14px;
}

.tag:hover {
  border-color: var(--lime-line);
  color: var(--lime);
}

.how ol {
  margin: 0;
  padding: 0;
  list-style: none;
  counter-reset: step;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.how li {
  counter-increment: step;
  display: grid;
  grid-template-columns: 28px 1fr;
  column-gap: 12px;
  font-size: 14px;
}

.how li::before {
  content: counter(step);
  grid-row: span 2;
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border-radius: 8px;
  background: var(--lime-soft);
  border: 1px solid var(--lime-line);
  color: var(--lime);
  font-family: var(--font-display);
  font-weight: 700;
  font-size: 14px;
}

.how li.done::before {
  background: var(--lime);
  color: var(--ink);
}

.how li span {
  color: var(--text-dim);
  font-size: 13px;
  line-height: 1.45;
}

.play-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  border-color: var(--lime-line);
  background:
    radial-gradient(circle at 50% 0%, rgba(183, 254, 0, 0.1), transparent 60%),
    var(--bg-card);
}

.discord {
  background: #5865f2;
  border-color: #5865f2;
  color: #fff;
}

.discord:hover:not(:disabled) {
  background: #4752c4;
  border-color: #4752c4;
}

.tech summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  font-weight: 700;
  font-size: 14px;
  list-style: none;
}

.tech summary::-webkit-details-marker {
  display: none;
}

.tech[open] summary :deep(svg) {
  transform: rotate(180deg);
}

dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 8px 16px;
  margin: 14px 0 0;
  font-size: 13px;
}

dt {
  color: var(--text-faint);
}

dd {
  margin: 0;
  text-align: right;
  font-weight: 600;
}

.addr {
  font-weight: 600;
  color: var(--lime);
}

.folder {
  width: 100%;
  margin-top: 14px;
}

.err {
  margin: 8px 0 0;
  font-size: 13px;
  color: var(--bad);
}

@media (max-width: 900px) {
  .columns {
    grid-template-columns: 1fr;
  }
}
</style>

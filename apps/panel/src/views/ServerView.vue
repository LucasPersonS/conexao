<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import ApprovalBadge from '../components/ApprovalBadge.vue'
import InviteCard from '../components/InviteCard.vue'
import FeaturedCard from '../components/FeaturedCard.vue'
import PostsTab from '../components/PostsTab.vue'
import CommentsTab from '../components/CommentsTab.vue'
import ServerForm, { type ServerFormValue } from '../components/ServerForm.vue'
import VersionsTab from '../components/VersionsTab.vue'
import WhitelistTab from '../components/WhitelistTab.vue'
import TeamTab from '../components/TeamTab.vue'
import CompanionTab from '../components/CompanionTab.vue'
import HistoryTab from '../components/HistoryTab.vue'
import ProTab from '../components/ProTab.vue'
import SupportTab from '../components/SupportTab.vue'
import { api, LOADER_NAMES, loadSession, message, session, type Server } from '../lib/api'

const props = defineProps<{ slug: string }>()
const route = useRoute()
const router = useRouter()

const server = ref<Server | null>(null)
const error = ref<string | null>(null)
const saved = ref(false)
const busy = ref(false)

const role = computed(() => {
  if (session.user?.is_staff) return 'owner'
  return session.memberships.find((m) => m.slug === props.slug)?.role ?? 'moderator'
})
const isAdmin = computed(() => role.value === 'owner' || role.value === 'admin')

const tabs = computed(() =>
  [
    { id: 'geral', label: 'Visão geral', show: true },
    { id: 'modpack', label: 'Modpack', show: isAdmin.value },
    { id: 'whitelist', label: 'Whitelist', show: true },
    { id: 'novidades', label: 'Eventos e notícias', show: true },
    { id: 'comentarios', label: 'Comentários', show: true },
    { id: 'equipe', label: 'Equipe', show: true },
    { id: 'servidor', label: 'Servidor', show: isAdmin.value },
    { id: 'pro', label: 'PRO e apoios', show: isAdmin.value },
    { id: 'historico', label: 'Histórico', show: isAdmin.value },
  ].filter((t) => t.show),
)
const tab = computed(() => {
  const wanted = String(route.query.tab ?? (isAdmin.value ? 'geral' : 'whitelist'))
  return tabs.value.some((t) => t.id === wanted) ? wanted : 'geral'
})

async function load() {
  try {
    server.value = await api.server(props.slug)
  } catch (e) {
    error.value = message(e)
  }
}
onMounted(load)

function setStars(count: number) {
  if (server.value) server.value.stars = count
}

const formValue = computed<ServerFormValue | null>(() =>
  server.value
    ? {
        slug: server.value.slug,
        name: server.value.name,
        description: server.value.description,
        address: server.value.address,
        minecraft: server.value.minecraft,
        loader: server.value.loader.kind,
        loader_version: server.value.loader.version,
        tags: server.value.tags.join(', '),
        icon_url: server.value.icon_url ?? '',
        banner_url: server.value.banner_url ?? '',
        discord_url: server.value.discord_url ?? '',
        whitelist_required: server.value.whitelist_required,
        allow_offline: server.value.allow_offline ?? false,
      }
    : null,
)

const orNull = (value: string) => value.trim() || null

async function save(value: ServerFormValue) {
  busy.value = true
  error.value = null
  saved.value = false
  try {
    server.value = await api.updateServer(props.slug, {
      name: value.name.trim(),
      description: value.description,
      address: value.address.trim(),
      tags: value.tags.split(',').map((t) => t.trim()).filter(Boolean),
      icon_url: orNull(value.icon_url),
      banner_url: orNull(value.banner_url),
      discord_url: orNull(value.discord_url),
      whitelist_required: value.whitelist_required,
      // Só quando muda: ligar exige o Companion conectado (a API confere na hora).
      ...(value.allow_offline !== (server.value?.allow_offline ?? false) ? { allow_offline: value.allow_offline } : {}),
    })
    await loadSession()
    saved.value = true
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <p v-if="error && !server" class="error">{{ error }}</p>
  <template v-if="server">
    <header class="hero" :style="server.banner_url ? { backgroundImage: `url(${server.banner_url})` } : undefined">
      <div class="shade" />
      <div class="info">
        <div class="title">
          <h1>{{ server.name }}</h1>
          <ApprovalBadge :approval="server.approval" />
          <span v-if="server.featured" class="badge brand">Destaque</span>
        </div>
        <p>
          {{ server.address }} · {{ LOADER_NAMES[server.loader.kind] }} {{ server.loader.version }} · Minecraft
          {{ server.minecraft }}
        </p>
      </div>
      <div class="kpis">
        <div>
          <b>{{ server.status.online ? `${server.status.players_online}/${server.status.players_max}` : 'Offline' }}</b>
          <span>jogadores</span>
        </div>
        <div>
          <b>{{ server.current_version ?? '—' }}</b>
          <span>modpack estável</span>
        </div>
        <div>
          <b>★ {{ (server.stars ?? 0).toLocaleString('pt-BR') }}</b>
          <span>{{ server.stars === 1 ? 'estrela' : 'estrelas' }}</span>
        </div>
      </div>
    </header>

    <p v-if="server.approval === 'pending'" class="notice">
      Seu servidor está em análise pela equipe Conexão. Enquanto isso, publique o modpack e instale o Conexão Companion: ele
      aparece na home do launcher assim que for aprovado.
    </p>

    <nav class="tabs">
      <button
        v-for="t in tabs"
        :key="t.id"
        :class="{ active: tab === t.id }"
        @click="router.replace({ query: { tab: t.id } })"
      >
        {{ t.label }}
      </button>
    </nav>

    <InviteCard v-if="tab === 'geral'" :server="server" />
    <FeaturedCard v-if="tab === 'geral' && isAdmin" :server="server" @changed="load" />
    <section v-if="tab === 'geral'" class="card">
      <ServerForm v-if="formValue && isAdmin" :key="server.slug" :initial="formValue" :creating="false" :busy="busy" @submit="save" @image="load">
        <template #extra>
          <span v-if="saved" class="ok">Alterações salvas.</span>
          <span v-if="error" class="error">{{ error }}</span>
        </template>
      </ServerForm>
      <p v-else class="muted">{{ server.description || 'Sem descrição.' }}</p>
    </section>
    <VersionsTab v-else-if="tab === 'modpack'" :server="server" @changed="load" />
    <WhitelistTab v-else-if="tab === 'whitelist'" :server="server" />
    <PostsTab v-else-if="tab === 'novidades'" :server="server" />
    <CommentsTab v-else-if="tab === 'comentarios'" :server="server" @stars="setStars" />
    <TeamTab v-else-if="tab === 'equipe'" :server="server" :role="role" />
    <CompanionTab v-else-if="tab === 'servidor'" :server="server" :role="role" />
    <template v-else-if="tab === 'pro'">
      <ProTab :server="server" @changed="load" />
      <SupportTab :server="server" @changed="load" />
    </template>
    <HistoryTab v-else-if="tab === 'historico'" :server="server" />
  </template>
</template>

<style scoped>
.hero {
  position: relative;
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: 20px;
  min-height: 150px;
  padding: 24px;
  border-radius: 18px;
  overflow: hidden;
  color: white;
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
  background: linear-gradient(to top, rgba(6, 10, 11, 0.85), transparent 70%);
}

.info,
.kpis {
  position: relative;
}

.title {
  display: flex;
  align-items: center;
  gap: 10px;
}

.info p {
  margin: 6px 0 0;
  opacity: 0.9;
}

.kpis {
  display: flex;
  gap: 12px;
}

.kpis div {
  display: flex;
  flex-direction: column;
  padding: 10px 14px;
  border-radius: 12px;
  background: rgba(10, 15, 17, 0.6);
  backdrop-filter: blur(4px);
  min-width: 110px;
}

.kpis b {
  font-family: var(--font-display);
  font-size: 20px;
  color: var(--lime);
}

.kpis span {
  font-size: 12px;
  opacity: 0.85;
}

.notice {
  margin: 16px 0 0;
  padding: 12px 16px;
  border-radius: 12px;
  background: var(--warn-bg);
  border: 1px solid var(--warn-line);
  color: var(--warn);
  line-height: 1.5;
  font-size: 14px;
}

.tabs {
  display: flex;
  gap: 4px;
  margin: 20px 0 16px;
  border-bottom: 1px solid var(--line);
}

.tabs button {
  border: none;
  background: none;
  padding: 10px 14px;
  font-weight: 600;
  color: var(--text-dim);
  border-bottom: 3px solid transparent;
  margin-bottom: -1px;
}

.tabs button.active {
  color: var(--text);
  border-bottom-color: var(--brand);
}
</style>

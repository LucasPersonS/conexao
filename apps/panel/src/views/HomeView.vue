<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { api, LOADER_NAMES, message, session, type Server } from '../lib/api'
import ApprovalBadge from '../components/ApprovalBadge.vue'

const servers = ref<Server[]>([])
const error = ref<string | null>(null)

onMounted(async () => {
  try {
    servers.value = await Promise.all(session.memberships.map((m) => api.server(m.slug)))
  } catch (e) {
    error.value = message(e)
  }
})
</script>

<template>
  <header class="head">
    <div>
      <h1>Olá, {{ session.user?.minecraft_name }}</h1>
      <p class="muted">Seus servidores no Conexão.</p>
    </div>
    <div class="head-actions">
      <RouterLink to="/link" class="btn">Vincular pelo código do console</RouterLink>
      <RouterLink to="/novo" class="btn btn-primary">Cadastrar servidor</RouterLink>
    </div>
  </header>

  <p v-if="error" class="error">{{ error }}</p>

  <div v-if="!session.memberships.length" class="card empty">
    <h2>Nenhum servidor ainda</h2>
    <p class="muted">
      Cadastre seu servidor. Ele aparece na home do launcher depois que a equipe Conexão aprovar — é gratuito para
      parceiros.
    </p>
    <RouterLink to="/novo" class="btn btn-primary">Cadastrar servidor</RouterLink>
  </div>

  <div class="grid">
    <RouterLink v-for="s in servers" :key="s.slug" :to="`/s/${s.slug}`" class="card server">
      <div class="top">
        <strong>{{ s.name }}</strong>
        <ApprovalBadge :approval="s.approval" />
      </div>
      <p class="muted addr">{{ s.address }}</p>
      <div class="meta">
        <span class="badge">{{ LOADER_NAMES[s.loader.kind] }} {{ s.loader.version }}</span>
        <span class="badge">MC {{ s.minecraft }}</span>
        <span v-if="s.featured" class="badge brand">Destaque</span>
      </div>
      <div class="stats">
        <span><b>{{ s.status.online ? s.status.players_online : '—' }}</b> online</span>
        <span>Modpack <b>{{ s.current_version ?? 'não publicado' }}</b></span>
      </div>
    </RouterLink>
  </div>
</template>

<style scoped>
.head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 24px;
}

.head .muted {
  margin: 4px 0 0;
}

.btn {
  text-decoration: none;
}

.head-actions {
  display: flex;
  gap: 10px;
}

.empty p {
  max-width: 520px;
  line-height: 1.5;
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 16px;
}

.server {
  text-decoration: none;
  color: var(--text);
  transition: border-color 0.15s, transform 0.15s;
}

.server:hover {
  border-color: var(--brand);
  transform: translateY(-1px);
}

.top {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  font-size: 17px;
}

.addr {
  margin: 4px 0 12px;
  font-size: 13px;
}

.meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 14px;
}

.stats {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
  color: var(--text-dim);
  padding-top: 12px;
  border-top: 1px solid var(--line);
}
</style>

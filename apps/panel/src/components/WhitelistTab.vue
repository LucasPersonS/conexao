<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import ProfileCard from '@profile/ProfileCard.vue'
import { api, formatDate, message, type Server, type WhitelistEntry, type WhitelistStatus } from '../lib/api'

const props = defineProps<{ server: Server }>()

const FILTERS: { id: WhitelistStatus; label: string }[] = [
  { id: 'pending', label: 'Pedidos' },
  { id: 'approved', label: 'Liberados' },
  { id: 'rejected', label: 'Recusados' },
  { id: 'banned', label: 'Banidos' },
]

const filter = ref<WhitelistStatus>('pending')
const entries = ref<WhitelistEntry[]>([])
const error = ref<string | null>(null)
const busy = ref(false)
const nick = ref('')
const added = ref<string | null>(null)
const opened = ref<WhitelistEntry | null>(null)

async function load() {
  error.value = null
  try {
    entries.value = await api.whitelist(props.server.slug, filter.value)
  } catch (e) {
    error.value = message(e)
  }
}
onMounted(load)
watch(filter, load)

async function set(name: string, status: WhitelistStatus) {
  opened.value = null
  if (status === 'banned' && !window.confirm(`Banir ${name}? Ele sai da whitelist e é expulso se estiver online.`)) return
  busy.value = true
  error.value = null
  try {
    await api.setWhitelist(props.server.slug, name, status)
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

async function add() {
  const name = nick.value.trim()
  if (!name) return
  await set(name, 'approved')
  if (!error.value) {
    added.value = name
    nick.value = ''
  }
}
</script>

<template>
  <div class="stack">
    <section class="card">
      <h2>Liberar jogador</h2>
      <form class="add" @submit.prevent="add">
        <input v-model="nick" class="input" placeholder="Nick do Minecraft" maxlength="16" />
        <button class="btn btn-primary" :disabled="busy || !nick.trim()">Liberar</button>
      </form>
      <p v-if="added" class="ok small">{{ added }} foi liberado. O agente aplica no servidor na hora.</p>
      <p v-if="!server.whitelist_required" class="muted small">
        Este servidor está com entrada aberta (whitelist desligada na Visão geral). Banimentos continuam valendo.
      </p>
    </section>

    <section class="card">
      <div class="filters">
        <button v-for="f in FILTERS" :key="f.id" class="btn btn-sm" :class="{ on: filter === f.id }" @click="filter = f.id">
          {{ f.label }}
        </button>
      </div>
      <p v-if="error" class="error">{{ error }}</p>
      <p v-if="!entries.length" class="muted">Nenhum jogador nesta lista.</p>
      <div v-else class="requests">
        <article v-for="e in entries" :key="e.minecraft_uuid" class="request">
          <button v-if="e.profile" class="profile" title="Ver perfil completo" @click="opened = e">
            <ProfileCard :profile="e.profile" compact />
          </button>
          <div v-else class="player">
            <img :src="`https://mc-heads.net/avatar/${e.minecraft_uuid}/40`" alt="" />
            <div>
              <b>{{ e.minecraft_name }}</b>
              <div class="muted small-inline">Ainda não montou o perfil</div>
            </div>
          </div>
          <div class="meta">
            <p v-if="e.note" class="note">“{{ e.note }}”</p>
            <p class="muted small-inline">
              {{ formatDate(e.updated_at) }}<span v-if="e.decided_by_name"> · decidido por {{ e.decided_by_name }}</span>
            </p>
            <div class="actions">
              <button v-if="e.status !== 'approved'" class="btn btn-sm btn-primary" :disabled="busy" @click="set(e.minecraft_name, 'approved')">
                Aprovar
              </button>
              <button v-if="e.status === 'pending'" class="btn btn-sm" :disabled="busy" @click="set(e.minecraft_name, 'rejected')">
                Recusar
              </button>
              <button v-if="e.status !== 'banned'" class="btn btn-sm btn-danger" :disabled="busy" @click="set(e.minecraft_name, 'banned')">
                Banir
              </button>
            </div>
          </div>
        </article>
      </div>
    </section>

    <div v-if="opened?.profile" class="overlay" @click.self="opened = null">
      <div class="modal">
        <ProfileCard :profile="opened.profile" />
        <p v-if="opened.note" class="note">Mensagem do pedido: “{{ opened.note }}”</p>
        <div class="actions">
          <button class="btn" @click="opened = null">Fechar</button>
          <button v-if="opened.status !== 'approved'" class="btn btn-primary" :disabled="busy" @click="set(opened.minecraft_name, 'approved')">
            Aprovar
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.stack {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.add {
  display: flex;
  gap: 8px;
  max-width: 420px;
}

.add .input {
  flex: 1;
}

.small {
  font-size: 13px;
  margin: 10px 0 0;
}

.filters {
  display: flex;
  gap: 6px;
  margin-bottom: 14px;
}

.filters .on {
  background: var(--brand);
  color: var(--ink);
}

.requests {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.request {
  display: grid;
  grid-template-columns: minmax(260px, 340px) 1fr;
  gap: 16px;
  align-items: center;
  padding: 12px;
  border: 1px solid var(--line);
  border-radius: 16px;
}

.profile {
  all: unset;
  cursor: pointer;
  display: block;
  border-radius: 14px;
  transition: transform 0.15s;
}

.profile:hover {
  transform: translateY(-2px);
}

.profile:focus-visible {
  outline: 2px solid var(--brand);
  outline-offset: 3px;
}

.player {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px;
}

.player img {
  border-radius: 6px;
}

.meta {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.note {
  margin: 0;
  font-style: italic;
  line-height: 1.5;
}

.small-inline {
  font-size: 13px;
  margin: 0;
}

.actions {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.overlay {
  position: fixed;
  inset: 0;
  background: rgba(6, 10, 11, 0.75);
  display: grid;
  place-items: center;
  z-index: 50;
  padding: 16px;
}

.modal {
  width: min(420px, 100%);
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: var(--card);
  padding: 16px;
  border-radius: 20px;
}

.modal .actions {
  justify-content: flex-end;
}

@media (max-width: 720px) {
  .request {
    grid-template-columns: 1fr;
  }
}
</style>

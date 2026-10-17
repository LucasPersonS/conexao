<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { api, formatDate, message, type WaitlistEntry, type WaitlistList, type WaitlistStatus } from '../lib/api'

const status = ref<WaitlistStatus>('pending')
const list = ref<WaitlistList | null>(null)
const error = ref<string | null>(null)
const notice = ref<string | null>(null)
const busy = ref(false)
const search = ref('')
const batch = ref(25)
const manualNick = ref('')

const tabs: { id: WaitlistStatus; label: string; count: () => number }[] = [
  { id: 'pending', label: 'Na fila', count: () => list.value?.pending ?? 0 },
  { id: 'approved', label: 'Liberados', count: () => list.value?.approved ?? 0 },
  { id: 'rejected', label: 'Recusados', count: () => list.value?.rejected ?? 0 },
]

async function load() {
  error.value = null
  try {
    list.value = await api.waitlist(status.value)
  } catch (e) {
    error.value = message(e)
  }
}

async function run(action: () => Promise<unknown>, done?: (result: unknown) => string | null) {
  busy.value = true
  error.value = null
  notice.value = null
  try {
    const result = await action()
    notice.value = done ? done(result) : null
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

const decide = (entry: WaitlistEntry, next: WaitlistStatus) => run(() => api.decideWaitlist(entry.id, next))

function reject(entry: WaitlistEntry) {
  const note = window.prompt(`Recusar ${entry.minecraft_name}? Motivo (só a equipe vê, opcional):`, '')
  if (note === null) return
  run(() => api.decideWaitlist(entry.id, 'rejected', note.trim() || undefined))
}

function approveNext() {
  const count = Math.max(1, Math.min(500, Math.floor(batch.value || 0)))
  if (!window.confirm(`Liberar as próximas ${count} pessoas da fila?`)) return
  run(
    () => api.approveNextWaitlist(count),
    (r) => `${(r as { approved: number }).approved} pessoas liberadas. Avise no Discord!`,
  )
}

function manual() {
  const nick = manualNick.value.trim()
  if (!nick) return
  run(
    () => api.manualWaitlist(nick),
    (r) => {
      const result = r as { nick: string; existing: boolean }
      manualNick.value = ''
      return result.existing
        ? `${result.nick} já tinha conta no Conexão e foi liberado.`
        : `${result.nick} (conta original) foi liberado: já pode entrar no launcher.`
    },
  )
}

const avatar = (e: WaitlistEntry) =>
  e.discord_id && e.discord_avatar ? `https://cdn.discordapp.com/avatars/${e.discord_id}/${e.discord_avatar}.png?size=64` : null

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase()
  const entries = list.value?.entries ?? []
  if (!q) return entries
  return entries.filter((e) => [e.minecraft_name, e.discord_name, e.discord_id ?? ''].some((v) => v.toLowerCase().includes(q)))
})

watch(status, load)
onMounted(load)
</script>

<template>
  <h1>Lista de espera</h1>
  <p class="muted lead">
    Quem entra no site com o Discord cai aqui. Só usa o launcher quem você liberar (a equipe Conexão, quem já tinha conta e a
    equipe dos servidores parceiros passam direto).
  </p>
  <p v-if="list && !list.enabled" class="warn-box">
    A lista está <b>desligada</b> na API (<code>CONEXAO_WAITLIST</code>): qualquer pessoa consegue baixar e entrar no launcher.
  </p>
  <p v-if="error" class="error">{{ error }}</p>
  <p v-if="notice" class="ok-box">{{ notice }}</p>

  <section class="card tools">
    <div class="tool">
      <h2>Liberar uma leva</h2>
      <p class="muted small">Libera os primeiros da fila, por ordem de chegada.</p>
      <div class="row">
        <input v-model.number="batch" class="input num" type="number" min="1" max="500" />
        <button class="btn btn-primary" :disabled="busy || !list?.pending" @click="approveNext">Liberar os próximos</button>
      </div>
    </div>
    <div class="tool">
      <h2>Liberar pelo nick</h2>
      <p class="muted small">Para quem não passou pelo site: conta que já existe no Conexão ou conta original.</p>
      <form class="row" @submit.prevent="manual">
        <input v-model="manualNick" class="input" maxlength="16" placeholder="Nick do Minecraft" />
        <button class="btn" type="submit" :disabled="busy || !manualNick.trim()">Liberar</button>
      </form>
    </div>
  </section>

  <nav class="tabs">
    <button v-for="t in tabs" :key="t.id" :class="{ active: status === t.id }" @click="status = t.id">
      {{ t.label }} <span class="count">{{ t.count() }}</span>
    </button>
    <input v-model="search" class="input search" placeholder="Buscar nick ou Discord" />
  </nav>

  <section class="card">
    <p v-if="!list" class="muted">Carregando…</p>
    <p v-else-if="!filtered.length" class="muted">Ninguém aqui.</p>
    <table v-else>
      <thead>
        <tr>
          <th v-if="status === 'pending'">#</th>
          <th>Discord</th>
          <th>Nick</th>
          <th>{{ status === 'pending' ? 'Entrou em' : 'Decidido' }}</th>
          <th />
        </tr>
      </thead>
      <tbody>
        <tr v-for="(e, i) in filtered" :key="e.id">
          <td v-if="status === 'pending'" class="pos">{{ search ? '·' : i + 1 }}</td>
          <td>
            <div class="who">
              <img v-if="avatar(e)" :src="avatar(e)!" alt="" class="avatar" />
              <span v-else class="avatar empty" />
              <div>
                <b>{{ e.discord_name || '—' }}</b>
                <div class="muted small">{{ e.discord_id ?? 'sem Discord (liberado pelo painel)' }}</div>
              </div>
            </div>
          </td>
          <td>
            <b>{{ e.minecraft_name }}</b>
            <div>
              <span :class="['badge', e.account_type === 'microsoft' ? 'good' : 'warn']">
                {{ e.account_type === 'microsoft' ? 'Original' : 'Não original' }}
              </span>
            </div>
          </td>
          <td class="muted small">
            <template v-if="status === 'pending'">{{ formatDate(e.created_at) }}</template>
            <template v-else>
              {{ formatDate(e.decided_at) }}<span v-if="e.decided_by_name"> · {{ e.decided_by_name }}</span>
              <div v-if="e.note">{{ e.note }}</div>
            </template>
          </td>
          <td class="actions">
            <button v-if="e.status !== 'approved'" class="btn btn-sm btn-primary" :disabled="busy" @click="decide(e, 'approved')">Liberar</button>
            <button v-if="e.status !== 'rejected'" class="btn btn-sm btn-danger" :disabled="busy" @click="reject(e)">
              {{ e.status === 'approved' ? 'Tirar acesso' : 'Recusar' }}
            </button>
            <button v-if="e.status !== 'pending'" class="btn btn-sm" :disabled="busy" @click="decide(e, 'pending')">Voltar à fila</button>
          </td>
        </tr>
      </tbody>
    </table>
    <p v-if="list && list.entries.length >= 1000" class="muted small">Mostrando os primeiros 1000.</p>
  </section>
</template>

<style scoped>
.lead {
  margin: 6px 0 20px;
  max-width: 760px;
}
.card {
  margin-bottom: 16px;
}
.small {
  font-size: 13px;
}
.tools {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 24px;
}
.tool h2 {
  margin: 0 0 4px;
  font-size: 17px;
}
.row {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}
.row .input {
  flex: 1;
  min-width: 0;
}
.row .input.num {
  flex: 0 0 90px;
}
.tabs {
  display: flex;
  align-items: center;
  gap: 4px;
  margin: 20px 0 16px;
  border-bottom: 1px solid var(--line);
  flex-wrap: wrap;
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
.count {
  font-size: 12px;
  opacity: 0.7;
}
.search {
  margin-left: auto;
  width: 240px;
  max-width: 100%;
  margin-bottom: 6px;
}
.who {
  display: flex;
  align-items: center;
  gap: 10px;
}
.avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  object-fit: cover;
  flex: none;
}
.avatar.empty {
  background: var(--line);
}
.pos {
  font-weight: 700;
  color: var(--text-dim);
  width: 40px;
}
.actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
  align-items: center;
  flex-wrap: wrap;
}
.warn-box,
.ok-box {
  padding: 10px 14px;
  border-radius: 10px;
  margin-bottom: 16px;
}
.warn-box {
  background: rgba(255, 201, 74, 0.1);
  border: 1px solid rgba(255, 201, 74, 0.35);
  color: #ffc94a;
}
.ok-box {
  background: rgba(183, 254, 0, 0.08);
  border: 1px solid rgba(183, 254, 0, 0.35);
}
</style>

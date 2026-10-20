<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import ApprovalBadge from '../components/ApprovalBadge.vue'
import { api, formatDate, LOADER_NAMES, message, type ModerationQueue, type Server } from '../lib/api'
import type { PublicProfile } from '@profile/theme'
import ProfileCard from '@profile/ProfileCard.vue'

const servers = ref<Server[]>([])
const error = ref<string | null>(null)
const busy = ref(false)
const days = ref<Record<string, number>>({})
const queue = ref<ModerationQueue>({ reports: [], backgrounds: [], banned: [] })
const viewing = ref<PublicProfile | null>(null)

async function load() {
  try {
    ;[servers.value, queue.value] = await Promise.all([api.staffServers(), api.moderation()])
  } catch (e) {
    error.value = message(e)
  }
}

async function run(action: () => Promise<unknown>) {
  busy.value = true
  error.value = null
  try {
    await action()
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

/** "Tudo certo" só tira da lista; "Remover" apaga o fundo e bloqueia o arquivo. */
const background = (name: string, keep: boolean) => run(() => api.reviewBackground(name, keep))
const allSeen = () => run(() => api.backgroundsSeen(queue.value.backgrounds.map((b) => b.minecraft_name)))

function askBan(name: string): string | null {
  const reason = window.prompt(
    `Bloquear o perfil de ${name}?\n\nO perfil volta ao padrão e ele não pode mais personalizar (continua jogando normalmente).\nMotivo (aparece para o jogador):`,
    'Conteúdo impróprio no perfil',
  )
  return reason === null ? null : reason.trim()
}

function ban(name: string) {
  const reason = askBan(name)
  if (reason !== null) run(() => api.banProfile(name, true, reason))
}

const unban = (name: string) => run(() => api.banProfile(name, false))

function report(id: string, action: 'dismiss' | 'clear' | 'ban', name: string) {
  if (action === 'clear' && !window.confirm(`Limpar o perfil de ${name}? Nome, pronomes, bio, links e fundo são apagados.`)) return
  if (action === 'ban') {
    const reason = askBan(name)
    if (reason !== null) run(() => api.resolveReport(id, 'ban', reason))
    return
  }
  run(() => api.resolveReport(id, action))
}

async function view(name: string) {
  try {
    viewing.value = await api.profile(name)
  } catch (e) {
    error.value = message(e)
  }
}
onMounted(load)

const pending = computed(() => servers.value.filter((s) => s.approval === 'pending'))
const others = computed(() => servers.value.filter((s) => s.approval !== 'pending'))

async function approval(slug: string, value: string) {
  busy.value = true
  try {
    await api.setApproval(slug, value)
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

async function central(slug: string, enable: boolean) {
  busy.value = true
  try {
    await api.setCentral(slug, enable)
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

async function feature(slug: string, enable: boolean) {
  busy.value = true
  try {
    const until = enable ? new Date(Date.now() + (days.value[slug] ?? 30) * 86_400_000).toISOString() : null
    await api.setFeatured(slug, until)
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <h1>Equipe Conexão</h1>
  <p class="muted lead">Aprove servidores parceiros, modere perfis e acompanhe o destaque na home do launcher.</p>
  <p v-if="error" class="error">{{ error }}</p>

  <section class="card">
    <h2>Aguardando aprovação ({{ pending.length }})</h2>
    <p v-if="!pending.length" class="muted">Nenhum pedido pendente.</p>
    <table v-else>
      <tbody>
        <tr v-for="s in pending" :key="s.slug">
          <td>
            <b>{{ s.name }}</b>
            <div class="muted small">{{ s.address }} · {{ LOADER_NAMES[s.loader.kind] }} · MC {{ s.minecraft }}</div>
            <div class="muted small desc">{{ s.description }}</div>
          </td>
          <td class="actions">
            <button class="btn btn-sm btn-primary" :disabled="busy" @click="approval(s.slug, 'approved')">Aprovar</button>
            <button class="btn btn-sm btn-danger" :disabled="busy" @click="approval(s.slug, 'rejected')">Recusar</button>
          </td>
        </tr>
      </tbody>
    </table>
  </section>

  <section class="card">
    <h2>Moderação de perfis ({{ queue.backgrounds.length + queue.reports.length }})</h2>
    <p v-if="!queue.backgrounds.length && !queue.reports.length" class="muted">Nada para revisar.</p>

    <template v-if="queue.backgrounds.length">
      <div class="subhead">
        <h3>Fundos novos ({{ queue.backgrounds.length }})</h3>
        <button class="btn btn-sm" :disabled="busy" @click="allSeen">Marcar todos como vistos</button>
      </div>
      <p class="muted small">
        Já aparecem nos perfis. Passe o olho e remova o que for impróprio (conteúdo adulto, ofensivo ou foto de pessoa real);
        a imagem removida não pode ser enviada de novo. Em caso grave, bloqueie o perfil.
      </p>
      <div class="backgrounds">
        <figure v-for="b in queue.backgrounds" :key="b.minecraft_uuid">
          <a :href="b.background_url" target="_blank" rel="noopener"><img :src="b.background_url" alt="" /></a>
          <figcaption>
            <b>{{ b.minecraft_name }}</b>
            <span class="muted small">{{ formatDate(b.updated_at) }}</span>
          </figcaption>
          <div class="actions">
            <button class="btn btn-sm" :disabled="busy" @click="background(b.minecraft_name, true)">Tudo certo</button>
            <button class="btn btn-sm btn-danger" :disabled="busy" @click="background(b.minecraft_name, false)">Remover</button>
            <button class="btn btn-sm btn-danger" :disabled="busy" @click="ban(b.minecraft_name)">Bloquear perfil</button>
          </div>
        </figure>
      </div>
    </template>

    <template v-if="queue.reports.length">
      <h3>Denúncias</h3>
      <table>
        <tbody>
          <tr v-for="r in queue.reports" :key="r.id">
            <td>
              <button class="link" @click="view(r.target_name)"><b>{{ r.target_name }}</b></button>
              <span v-if="r.open_count > 1" class="badge warn">{{ r.open_count }} denúncias</span>
              <div class="muted small">“{{ r.reason }}” · por {{ r.reporter_name }} · {{ formatDate(r.created_at) }}</div>
            </td>
            <td class="actions">
              <button class="btn btn-sm" :disabled="busy" @click="report(r.id, 'dismiss', r.target_name)">Está tudo certo</button>
              <button class="btn btn-sm btn-danger" :disabled="busy" @click="report(r.id, 'clear', r.target_name)">Limpar perfil</button>
              <button class="btn btn-sm btn-danger" :disabled="busy" @click="report(r.id, 'ban', r.target_name)">Bloquear perfil</button>
            </td>
          </tr>
        </tbody>
      </table>
    </template>

    <details v-if="queue.banned.length" class="banned">
      <summary>Perfis bloqueados ({{ queue.banned.length }})</summary>
      <table>
        <tbody>
          <tr v-for="b in queue.banned" :key="b.minecraft_uuid">
            <td>
              <b>{{ b.minecraft_name }}</b>
              <div class="muted small">{{ b.profile_ban_reason || 'Sem motivo informado' }} · {{ formatDate(b.profile_banned_at) }}</div>
            </td>
            <td class="actions">
              <button class="btn btn-sm" :disabled="busy" @click="unban(b.minecraft_name)">Desbloquear</button>
            </td>
          </tr>
        </tbody>
      </table>
    </details>
  </section>

  <div v-if="viewing" class="overlay" @click.self="viewing = null">
    <div class="modal">
      <ProfileCard :profile="viewing" />
      <button class="btn" @click="viewing = null">Fechar</button>
    </div>
  </div>

  <section class="card">
    <h2>Todos os servidores</h2>
    <table>
      <thead>
        <tr>
          <th>Servidor</th>
          <th>Situação</th>
          <th>Jogadores</th>
          <th>Destaque</th>
          <th title="Programa SMP da Central (parceria Central SMP's)">Central</th>
          <th />
        </tr>
      </thead>
      <tbody>
        <tr v-for="s in others" :key="s.slug">
          <td><RouterLink :to="`/s/${s.slug}`">{{ s.name }}</RouterLink></td>
          <td><ApprovalBadge :approval="s.approval" /></td>
          <td>{{ s.status.online ? s.status.players_online : '—' }}</td>
          <td>
            <span v-if="s.featured" class="badge brand">Até {{ formatDate(s.featured_until ?? null) }}</span>
            <span v-else class="muted">—</span>
          </td>
          <td>
            <label class="central-toggle" :title="s.central_partner ? 'Tirar do programa SMP da Central' : 'Colocar no programa SMP da Central'">
              <input type="checkbox" :checked="!!s.central_partner" :disabled="busy" @change="central(s.slug, !s.central_partner)" />
              <span>SMP da Central</span>
            </label>
          </td>
          <td class="actions">
            <template v-if="s.approval === 'approved'">
              <template v-if="!s.featured">
                <input v-model.number="days[s.slug]" class="input days" type="number" min="1" placeholder="30" />
                <span class="muted small">dias</span>
                <button class="btn btn-sm btn-primary" :disabled="busy" @click="feature(s.slug, true)">Destacar</button>
              </template>
              <button v-else class="btn btn-sm" :disabled="busy" @click="feature(s.slug, false)">Tirar destaque</button>
              <button class="btn btn-sm btn-danger" :disabled="busy" @click="approval(s.slug, 'suspended')">Suspender</button>
            </template>
            <button v-else class="btn btn-sm" :disabled="busy" @click="approval(s.slug, 'approved')">Reativar</button>
          </td>
        </tr>
      </tbody>
    </table>
    <p class="muted small">Atualizado em {{ formatDate(new Date().toISOString()) }}</p>
  </section>
</template>

<style scoped>
.central-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  white-space: nowrap;
  color: var(--text-dim);
}
.central-toggle input {
  accent-color: #3b9ae8;
}
.lead {
  margin: 6px 0 20px;
}

.card {
  margin-bottom: 16px;
}

.small {
  font-size: 13px;
}

.desc {
  max-width: 560px;
  margin-top: 4px;
}

.actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
  align-items: center;
}

h3 {
  margin: 18px 0 4px;
  font-size: 15px;
}

.subhead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.banned {
  margin-top: 18px;
}

.banned summary {
  cursor: pointer;
  font-weight: 600;
  color: var(--text-dim);
  margin-bottom: 8px;
}

.backgrounds {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
  margin-top: 10px;
}

.backgrounds figure {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
  border: 1px solid var(--line);
  border-radius: 14px;
}

.backgrounds img {
  width: 100%;
  aspect-ratio: 16 / 10;
  object-fit: cover;
  border-radius: 10px;
  display: block;
}

.backgrounds figcaption {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
}

.backgrounds .actions {
  justify-content: flex-start;
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
  border: 1px solid var(--line);
  padding: 16px;
  border-radius: 20px;
}

.days {
  width: 70px;
  height: 30px;
}
</style>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import ApprovalBadge from '../components/ApprovalBadge.vue'
import {
  api,
  API_URL,
  formatBytes,
  formatDate,
  message,
  type ServerStorage,
  type ServerUsage,
  type StorageOverview,
} from '../lib/api'

const apiUrl = API_URL
const data = ref<StorageOverview | null>(null)
const error = ref<string | null>(null)
const notice = ref<string | null>(null)
const busy = ref(false)
const filter = ref('')
const sort = ref<keyof ServerUsage>('hosted_bytes')
const open = ref<string | null>(null)
const detail = ref<ServerStorage | null>(null)

async function load() {
  try {
    data.value = await api.storage()
  } catch (e) {
    error.value = message(e)
  }
}

async function run(action: () => Promise<string | void>) {
  busy.value = true
  error.value = null
  notice.value = null
  try {
    notice.value = (await action()) ?? null
    await load()
    if (open.value) detail.value = await api.storageServer(open.value)
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

onMounted(load)

const diskUsed = computed(() => (data.value ? data.value.disk.total - data.value.disk.free : 0))
const diskPercent = computed(() => data.value?.disk.used_percent ?? 0)
const diskTone = computed(() => {
  const alert = data.value?.disk.alert_percent ?? 90
  return diskPercent.value >= alert ? 'bad' : diskPercent.value >= alert - 5 ? 'warn' : 'good'
})

const servers = computed(() => {
  const term = filter.value.trim().toLowerCase()
  const list = (data.value?.servers ?? []).filter(
    (s) => !term || s.name.toLowerCase().includes(term) || s.slug.includes(term),
  )
  const key = sort.value
  return [...list].sort((a, b) => Number(b[key] ?? 0) - Number(a[key] ?? 0))
})

const maxHosted = computed(() => Math.max(1, ...(data.value?.servers ?? []).map((s) => s.hosted_bytes)))

function quotaOf(s: ServerUsage): number {
  return s.featured ? (data.value?.featured_quota_bytes ?? 0) : (data.value?.quota_bytes ?? 0)
}

async function toggle(slug: string) {
  if (open.value === slug) {
    open.value = null
    detail.value = null
    return
  }
  open.value = slug
  detail.value = null
  try {
    detail.value = await api.storageServer(slug)
  } catch (e) {
    error.value = message(e)
  }
}

function deleteVersion(slug: string, id: string, version: string) {
  if (!window.confirm(`Apagar a versão ${version} de ${slug}?\n\nOs arquivos que só ela usa saem do disco na hora. Não dá para desfazer.`)) return
  run(async () => {
    const r = await api.deleteVersion(slug, id)
    return `Versão ${r.deleted} apagada · ${formatBytes(r.bytes_freed)} liberados (${r.files_freed} arquivo(s)).`
  })
}

function blockFile(file: ServerStorage['files'][number]) {
  const others = file.other_servers.length ? `\nTambém é usado por: ${file.other_servers.join(', ')}.` : ''
  const reason = window.prompt(
    `Derrubar ${file.path} (${formatBytes(file.size)})?\n\nO arquivo sai do disco na hora e não pode ser enviado de novo. ` +
      `As versões que o usam (${file.versions.join(', ')}) deixam de instalar até o servidor publicar sem ele.${others}\n\nMotivo:`,
    'Arquivo não permitido',
  )
  if (reason === null) return
  run(async () => {
    const r = await api.blockFile(file.sha1, reason.trim())
    return `Arquivo derrubado · ${formatBytes(r.freed)} liberados. Versões afetadas: ${r.affected_versions.join(', ') || 'nenhuma'}.`
  })
}

function unblock(sha1: string, path: string) {
  if (!window.confirm(`Liberar ${path || sha1} para ser enviado de novo?`)) return
  run(async () => {
    await api.unblockFile(sha1)
    return 'Arquivo liberado.'
  })
}

function cleanup() {
  run(async () => {
    const r = await api.runCleanup()
    return `Limpeza concluída · ${formatBytes(r.bytes)} liberados (${r.files} arquivo(s), ${r.versions} versão(ões) de servidores recusados, ${r.temporaries} temporário(s)).`
  })
}
</script>

<template>
  <h1>Armazenamento</h1>
  <p class="muted lead">
    Quanto cada servidor ocupa no disco da VPS. Só contam arquivos hospedados no Conexão; mods do Modrinth e do CurseForge são
    baixados direto de lá.
  </p>
  <p v-if="error" class="error">{{ error }}</p>
  <p v-if="notice" class="notice">{{ notice }}</p>

  <template v-if="data">
    <section class="stats">
      <div class="card stat">
        <span class="label">Disco da VPS</span>
        <strong>{{ diskPercent.toFixed(0) }}%</strong>
        <div class="meter" :class="diskTone">
          <span :style="{ width: `${Math.min(100, diskPercent)}%` }" />
          <i :style="{ left: `${data.disk.alert_percent}%` }" title="Trava de envio" />
        </div>
        <span class="muted small">{{ formatBytes(diskUsed) }} de {{ formatBytes(data.disk.total) }} · livres {{ formatBytes(data.disk.free) }}</span>
      </div>
      <div class="card stat">
        <span class="label">Arquivos hospedados</span>
        <strong>{{ formatBytes(data.stored.bytes) }}</strong>
        <span class="muted small">{{ data.stored.files.toLocaleString('pt-BR') }} arquivo(s)</span>
      </div>
      <div class="card stat">
        <span class="label">Sem uso</span>
        <strong>{{ formatBytes(data.unused.bytes) }}</strong>
        <span class="muted small">{{ data.unused.files }} arquivo(s) que nenhuma versão usa · saem na limpeza</span>
      </div>
      <div class="card stat">
        <span class="label">Limpeza</span>
        <button class="btn btn-sm" :disabled="busy" @click="cleanup">Rodar limpeza agora</button>
        <span class="muted small">Roda sozinha todo dia. Mantém o que foi usado nas últimas 48 h.</span>
      </div>
    </section>

    <section class="card">
      <div class="subhead">
        <h2>Servidores</h2>
        <div class="tools">
          <input v-model="filter" class="input" placeholder="Buscar servidor" />
          <select v-model="sort" class="input">
            <option value="hosted_bytes">Maior espaço total</option>
            <option value="exclusive_bytes">Maior espaço exclusivo</option>
            <option value="current_bytes">Maior versão ativa</option>
            <option value="uploaded_24h">Mais enviado em 24 h</option>
            <option value="versions">Mais versões</option>
          </select>
        </div>
      </div>
      <p class="muted small">
        <b>Total</b>: todas as versões, cada arquivo contado uma vez. <b>Exclusivo</b>: o que volta para o disco se as versões
        do servidor forem apagadas. <b>Ativa</b>: versão do canal stable, comparada com a cota.
      </p>
      <p v-if="!servers.length" class="muted">Nenhum servidor.</p>
      <table v-else>
        <thead>
          <tr>
            <th>Servidor</th>
            <th class="num">Total</th>
            <th class="num">Exclusivo</th>
            <th class="num">Ativa / cota</th>
            <th class="num">24 h</th>
            <th class="num">Versões</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <template v-for="s in servers" :key="s.slug">
            <tr :class="{ opened: open === s.slug }">
              <td>
                <RouterLink :to="`/s/${s.slug}`"><b>{{ s.name }}</b></RouterLink>
                <div class="row">
                  <ApprovalBadge :approval="s.approval" />
                  <span v-if="s.featured" class="badge brand">Destaque</span>
                  <span class="muted small">{{ s.files }} arquivo(s) · última versão {{ formatDate(s.last_published_at) }}</span>
                </div>
                <div class="bar"><span :style="{ width: `${(100 * s.hosted_bytes) / maxHosted}%` }" /></div>
              </td>
              <td class="num"><b>{{ formatBytes(s.hosted_bytes) }}</b></td>
              <td class="num">{{ formatBytes(s.exclusive_bytes) }}</td>
              <td class="num" :class="{ over: s.current_bytes > quotaOf(s) }">
                {{ formatBytes(s.current_bytes) }} <span class="muted">/ {{ formatBytes(quotaOf(s)) }}</span>
              </td>
              <td class="num">{{ formatBytes(s.uploaded_24h) }}</td>
              <td class="num">{{ s.versions }}</td>
              <td class="actions">
                <button class="btn btn-sm" :disabled="!s.versions" @click="toggle(s.slug)">
                  {{ open === s.slug ? 'Fechar' : 'Analisar' }}
                </button>
              </td>
            </tr>
            <tr v-if="open === s.slug" class="detail">
              <td colspan="7">
                <p v-if="!detail" class="muted">Carregando…</p>
                <template v-else>
                  <h3>Versões</h3>
                  <table>
                    <tbody>
                      <tr v-for="v in detail.versions" :key="v.id">
                        <td>
                          <b>{{ v.version }}</b>
                          <span v-for="c in v.channels" :key="c" class="badge good">{{ c }}</span>
                          <div class="muted small">{{ formatDate(v.published_at) }} · {{ v.files }} arquivo(s) hospedado(s)</div>
                        </td>
                        <td class="num">{{ formatBytes(v.hosted_bytes) }}</td>
                        <td class="actions">
                          <button
                            class="btn btn-sm btn-danger"
                            :disabled="busy || v.channels.length > 0"
                            :title="v.channels.length ? 'Versão ativa: não pode ser apagada' : ''"
                            @click="deleteVersion(s.slug, v.id, v.version)"
                          >
                            Apagar versão
                          </button>
                        </td>
                      </tr>
                    </tbody>
                  </table>

                  <h3>Maiores arquivos</h3>
                  <p v-if="!detail.files.length" class="muted small">Nenhum arquivo hospedado.</p>
                  <table v-else>
                    <tbody>
                      <tr v-for="f in detail.files" :key="f.sha1">
                        <td>
                          <code>{{ f.path }}</code>
                          <div class="muted small">
                            versões {{ f.versions.join(', ') }} · enviado {{ formatDate(f.created_at) }}
                            <template v-if="f.other_servers.length"> · também em {{ f.other_servers.join(', ') }}</template>
                          </div>
                        </td>
                        <td class="num">{{ formatBytes(f.size) }}</td>
                        <td class="actions">
                          <a class="btn btn-sm" :href="`${apiUrl}/files/${f.sha1}`" rel="noopener">Baixar</a>
                          <button class="btn btn-sm btn-danger" :disabled="busy" @click="blockFile(f)">Derrubar</button>
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </template>
              </td>
            </tr>
          </template>
        </tbody>
      </table>
    </section>

    <section v-if="data.blocked.length" class="card">
      <h2>Arquivos derrubados ({{ data.blocked.length }})</h2>
      <p class="muted small">Não podem ser enviados de novo. Liberar não restaura o arquivo: o servidor precisa enviar outra vez.</p>
      <table>
        <tbody>
          <tr v-for="b in data.blocked" :key="b.sha1">
            <td>
              <code>{{ b.path || b.sha1 }}</code>
              <div class="muted small">
                {{ b.reason || 'Sem motivo' }} · {{ formatBytes(b.size) }} · {{ b.blocked_by ?? 'equipe' }} · {{ formatDate(b.created_at) }}
              </div>
            </td>
            <td class="actions">
              <button class="btn btn-sm" :disabled="busy" @click="unblock(b.sha1, b.path)">Liberar</button>
            </td>
          </tr>
        </tbody>
      </table>
    </section>
  </template>
  <p v-else-if="!error" class="muted">Carregando…</p>
</template>

<style scoped>
.lead {
  margin: 6px 0 20px;
  max-width: 760px;
}

.notice {
  padding: 10px 14px;
  border-radius: 10px;
  background: var(--good-bg);
  border: 1px solid var(--good-line);
  color: var(--good);
}

.stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
  gap: 12px;
  margin-bottom: 16px;
}

.stat {
  display: flex;
  flex-direction: column;
  gap: 6px;
  align-items: flex-start;
}

.stat strong {
  font-family: var(--font-display);
  font-size: 26px;
}

.label {
  font-family: var(--font-pixel);
  font-size: 11px;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--lime);
}

.meter {
  position: relative;
  width: 100%;
  height: 8px;
  border-radius: 99px;
  background: var(--bg-input);
  border: 1px solid var(--line);
}

.meter span {
  display: block;
  height: 100%;
  border-radius: 99px;
  background: var(--good);
}

.meter.warn span {
  background: var(--warn);
}

.meter.bad span {
  background: var(--bad);
}

.meter i {
  position: absolute;
  top: -3px;
  bottom: -3px;
  width: 2px;
  background: var(--text-dim);
}

.card {
  margin-bottom: 16px;
}

.small {
  font-size: 13px;
}

.subhead {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.tools {
  display: flex;
  gap: 8px;
}

.tools .input {
  height: 34px;
}

.row {
  display: flex;
  gap: 6px;
  align-items: center;
  flex-wrap: wrap;
  margin-top: 4px;
}

.bar {
  height: 4px;
  margin-top: 8px;
  border-radius: 99px;
  background: var(--bg-input);
  max-width: 320px;
}

.bar span {
  display: block;
  height: 100%;
  border-radius: 99px;
  background: var(--lime);
}

.num {
  text-align: right;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.over {
  color: var(--bad);
  font-weight: 700;
}

tr.opened > td {
  border-bottom-color: transparent;
}

tr.detail > td {
  background: var(--bg-soft);
  padding: 4px 16px 16px;
}

h3 {
  margin: 14px 0 6px;
  font-size: 15px;
}

code {
  font-size: 13px;
  word-break: break-all;
}

.badge + .badge,
b + .badge {
  margin-left: 6px;
}
</style>

<script setup lang="ts">
/** Equipe Conexão: banners de patrocinadores no launcher (home, servidor e carregamento). */
import { computed, onMounted, ref } from 'vue'
import {
  api,
  formatDate,
  message,
  uploadSponsorImage,
  type SponsorDay,
  type SponsorInput,
  type SponsorPlacement,
  type StaffSponsor,
} from '../lib/api'

const PLACES: { id: SponsorPlacement; label: string; hint: string }[] = [
  { id: 'home', label: 'Home', hint: 'Faixa larga na tela inicial' },
  { id: 'server', label: 'Página do servidor', hint: 'Lateral de todos os servidores' },
  { id: 'loading', label: 'Carregamento do jogo', hint: 'Enquanto baixa e abre o Minecraft' },
]
const PLACE_LABEL = Object.fromEntries(PLACES.map((p) => [p.id, p.label])) as Record<SponsorPlacement, string>

const list = ref<StaffSponsor[]>([])
const error = ref<string | null>(null)
const notice = ref<string | null>(null)
const busy = ref(false)
const uploading = ref<'wide' | 'side' | null>(null)
const editing = ref<string | 'new' | null>(null)
const confirmDelete = ref<string | null>(null)
const report = ref<{ id: string; days: SponsorDay[] } | null>(null)

type Form = Omit<SponsorInput, 'starts_at' | 'ends_at'> & { starts_at: string; ends_at: string }
const blank = (): Form => ({
  name: '',
  image_url: '',
  image_side_url: null,
  link_url: 'https://',
  alt_text: '',
  placements: ['home'],
  weight: 1,
  starts_at: '',
  ends_at: '',
  active: true,
})
const form = ref<Form>(blank())

async function load() {
  try {
    list.value = await api.sponsors()
  } catch (e) {
    error.value = message(e)
  }
}
onMounted(load)

/** ISO → valor do <input type="datetime-local"> no fuso do navegador. */
function toLocal(iso: string | null): string {
  if (!iso) return ''
  const d = new Date(iso)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`
}
const fromLocal = (value: string) => (value ? new Date(value).toISOString() : null)

function startNew() {
  form.value = blank()
  editing.value = 'new'
  notice.value = null
}

function startEdit(s: StaffSponsor) {
  form.value = {
    name: s.name,
    image_url: s.image_url,
    image_side_url: s.image_side_url,
    link_url: s.link_url,
    alt_text: s.alt_text ?? '',
    placements: [...s.placements],
    weight: s.weight,
    starts_at: toLocal(s.starts_at),
    ends_at: toLocal(s.ends_at),
    active: s.active,
  }
  editing.value = s.id
  notice.value = null
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

function togglePlace(id: SponsorPlacement) {
  const p = form.value.placements
  form.value.placements = p.includes(id) ? p.filter((x) => x !== id) : [...p, id]
}

async function pick(event: Event, kind: 'wide' | 'side') {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  error.value = null
  if (!['image/png', 'image/jpeg', 'image/webp'].includes(file.type)) {
    error.value = 'Use PNG, JPEG ou WebP.'
    return
  }
  if (file.size > 5 * 1024 * 1024) {
    error.value = 'Imagem de até 5 MB.'
    return
  }
  uploading.value = kind
  try {
    const url = await uploadSponsorImage(file)
    if (kind === 'wide') form.value.image_url = url
    else form.value.image_side_url = url
  } catch (e) {
    error.value = message(e)
  } finally {
    uploading.value = null
  }
}

const canSave = computed(
  () => form.value.name.trim() && form.value.image_url && form.value.link_url.startsWith('https://') && form.value.placements.length,
)

function payload(f: Form, active = f.active): SponsorInput {
  return {
    ...f,
    alt_text: f.alt_text?.trim() || null,
    image_side_url: f.image_side_url || null,
    starts_at: fromLocal(f.starts_at),
    ends_at: fromLocal(f.ends_at),
    active,
  }
}

async function save() {
  if (!canSave.value || busy.value) return
  busy.value = true
  error.value = null
  try {
    if (editing.value === 'new') await api.createSponsor(payload(form.value))
    else if (editing.value) await api.updateSponsor(editing.value, payload(form.value))
    notice.value = 'Patrocinador salvo. O launcher atualiza a lista em até 5 minutos.'
    editing.value = null
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

async function toggleActive(s: StaffSponsor) {
  busy.value = true
  try {
    await api.updateSponsor(s.id, {
      name: s.name,
      image_url: s.image_url,
      image_side_url: s.image_side_url,
      link_url: s.link_url,
      alt_text: s.alt_text,
      placements: s.placements,
      weight: s.weight,
      starts_at: s.starts_at,
      ends_at: s.ends_at,
      active: !s.active,
    })
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

async function remove(s: StaffSponsor) {
  if (confirmDelete.value !== s.id) {
    confirmDelete.value = s.id
    return
  }
  busy.value = true
  try {
    await api.deleteSponsor(s.id)
    confirmDelete.value = null
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

async function openReport(s: StaffSponsor) {
  if (report.value?.id === s.id) {
    report.value = null
    return
  }
  try {
    report.value = { id: s.id, days: (await api.sponsorStats(s.id)).days }
  } catch (e) {
    error.value = message(e)
  }
}

/** Planilha para mandar ao patrocinador. */
function downloadCsv(s: StaffSponsor) {
  if (!report.value) return
  const rows = [['dia', 'posicao', 'visualizacoes', 'cliques'], ...report.value.days.map((d) => [d.day, PLACE_LABEL[d.placement], d.views, d.clicks])]
  const csv = rows.map((r) => r.join(';')).join('\n')
  const link = document.createElement('a')
  link.href = URL.createObjectURL(new Blob(['﻿' + csv], { type: 'text/csv;charset=utf-8' }))
  link.download = `patrocinio-${s.name.toLowerCase().replace(/[^a-z0-9]+/g, '-')}.csv`
  link.click()
  URL.revokeObjectURL(link.href)
}

function status(s: StaffSponsor): { label: string; tone: string } {
  if (!s.active) return { label: 'Pausado', tone: 'warn' }
  if (s.live) return { label: 'No ar', tone: 'good' }
  if (new Date(s.starts_at) > new Date()) return { label: 'Agendado', tone: '' }
  return { label: 'Encerrado', tone: 'bad' }
}
const ctr = (s: StaffSponsor) => (s.views_30d ? `${((s.clicks_30d / s.views_30d) * 100).toLocaleString('pt-BR', { maximumFractionDigits: 1 })}%` : '—')
const host = (url: string) => {
  try {
    return new URL(url).hostname.replace(/^www\./, '')
  } catch {
    return url
  }
}
const reportTotals = computed(() =>
  (report.value?.days ?? []).reduce(
    (acc, d) => {
      acc[d.placement] = acc[d.placement] ?? { views: 0, clicks: 0 }
      acc[d.placement]!.views += d.views
      acc[d.placement]!.clicks += d.clicks
      return acc
    },
    {} as Partial<Record<SponsorPlacement, { views: number; clicks: number }>>,
  ),
)
</script>

<template>
  <header class="head">
    <div>
      <h1>Patrocinadores</h1>
      <p class="muted">Banners clicáveis no launcher. Visualizações e cliques contam uma vez por jogador por dia.</p>
    </div>
    <button v-if="!editing" class="btn btn-primary" @click="startNew">Novo patrocinador</button>
  </header>

  <p v-if="error" class="error">{{ error }}</p>
  <p v-if="notice" class="notice">{{ notice }}</p>

  <section v-if="editing" class="card form">
    <h2>{{ editing === 'new' ? 'Novo patrocinador' : 'Editar patrocinador' }}</h2>
    <div class="grid-2">
      <label class="field">
        Nome
        <input v-model="form.name" class="input" maxlength="80" placeholder="Ex.: HostCraft" />
      </label>
      <label class="field">
        Link ao clicar
        <input v-model.trim="form.link_url" class="input" maxlength="500" placeholder="https://site.com/?utm_source=conexao" />
      </label>
    </div>
    <label class="field">
      <span>Descrição curta <span class="muted small">(acessibilidade e quando a imagem não carrega)</span></span>
      <input v-model="form.alt_text" class="input" maxlength="140" placeholder="Ex.: Hospedagem de servidores com 20% off" />
    </label>

    <div class="images">
      <div class="img-field">
        <strong>Banner largo <span class="muted small">8:1 · 1600×200 · home e carregamento</span></strong>
        <div class="preview wide" :style="form.image_url ? { backgroundImage: `url(&quot;${form.image_url}&quot;)` } : undefined">
          <span v-if="!form.image_url">Sem imagem</span>
        </div>
        <label class="btn btn-sm file">
          {{ uploading === 'wide' ? 'Enviando…' : form.image_url ? 'Trocar' : 'Enviar imagem' }}
          <input type="file" accept="image/png,image/jpeg,image/webp" :disabled="!!uploading" @change="pick($event, 'wide')" />
        </label>
      </div>
      <div class="img-field">
        <strong>Banner lateral <span class="muted small">2:1 · 800×400 · opcional</span></strong>
        <div class="preview side" :style="form.image_side_url ? { backgroundImage: `url(&quot;${form.image_side_url}&quot;)` } : undefined">
          <span v-if="!form.image_side_url">Usa o banner largo</span>
        </div>
        <div class="row">
          <label class="btn btn-sm file">
            {{ uploading === 'side' ? 'Enviando…' : form.image_side_url ? 'Trocar' : 'Enviar imagem' }}
            <input type="file" accept="image/png,image/jpeg,image/webp" :disabled="!!uploading" @change="pick($event, 'side')" />
          </label>
          <button v-if="form.image_side_url" class="btn btn-sm" @click="form.image_side_url = null">Remover</button>
        </div>
      </div>
    </div>

    <fieldset class="places">
      <legend>Onde aparece</legend>
      <label v-for="p in PLACES" :key="p.id" class="place" :class="{ on: form.placements.includes(p.id) }">
        <input type="checkbox" :checked="form.placements.includes(p.id)" @change="togglePlace(p.id)" />
        <span><b>{{ p.label }}</b><small class="muted">{{ p.hint }}</small></span>
      </label>
    </fieldset>

    <div class="grid-3">
      <label class="field">
        Peso na rotação (1 a 10)
        <input v-model.number="form.weight" class="input" type="number" min="1" max="10" />
      </label>
      <label class="field">
        <span>Início <span class="muted small">(vazio = agora)</span></span>
        <input v-model="form.starts_at" class="input" type="datetime-local" />
      </label>
      <label class="field">
        <span>Fim <span class="muted small">(vazio = sem fim)</span></span>
        <input v-model="form.ends_at" class="input" type="datetime-local" />
      </label>
    </div>
    <label class="check"><input v-model="form.active" type="checkbox" /> Ativo</label>

    <div class="actions">
      <button class="btn" @click="editing = null">Cancelar</button>
      <button class="btn btn-primary" :disabled="!canSave || busy" @click="save">{{ busy ? 'Salvando…' : 'Salvar' }}</button>
    </div>
  </section>

  <section class="card">
    <p v-if="!list.length" class="muted">Nenhum patrocinador ainda.</p>
    <div v-for="s in list" :key="s.id" class="item">
      <div class="thumb" :style="{ backgroundImage: `url(&quot;${s.image_url}&quot;)` }" />
      <div class="info">
        <div class="title">
          <strong>{{ s.name }}</strong>
          <span class="badge" :class="status(s).tone">{{ status(s).label }}</span>
          <span class="muted small">peso {{ s.weight }}</span>
        </div>
        <span class="muted small">
          {{ host(s.link_url) }} · {{ s.placements.map((p) => PLACE_LABEL[p]).join(', ') }} · {{ formatDate(s.starts_at) }} →
          {{ s.ends_at ? formatDate(s.ends_at) : 'sem fim' }}
        </span>
      </div>
      <div class="nums">
        <div><b>{{ s.views_30d.toLocaleString('pt-BR') }}</b><small>visualizações</small></div>
        <div><b>{{ s.clicks_30d.toLocaleString('pt-BR') }}</b><small>cliques</small></div>
        <div><b>{{ ctr(s) }}</b><small>CTR 30 dias</small></div>
      </div>
      <div class="item-actions">
        <button class="btn btn-sm" @click="openReport(s)">{{ report?.id === s.id ? 'Fechar' : 'Relatório' }}</button>
        <button class="btn btn-sm" :disabled="busy" @click="startEdit(s)">Editar</button>
        <button class="btn btn-sm" :disabled="busy" @click="toggleActive(s)">{{ s.active ? 'Pausar' : 'Ativar' }}</button>
        <button class="btn btn-sm btn-danger" :disabled="busy" @click="remove(s)">{{ confirmDelete === s.id ? 'Confirmar' : 'Excluir' }}</button>
      </div>
      <div v-if="report?.id === s.id" class="report">
        <div class="totals">
          <div v-for="p in s.placements" :key="p">
            <small>{{ PLACE_LABEL[p] }}</small>
            <b>{{ (reportTotals[p]?.views ?? 0).toLocaleString('pt-BR') }} vis. · {{ (reportTotals[p]?.clicks ?? 0).toLocaleString('pt-BR') }} cliques</b>
          </div>
          <button class="btn btn-sm" :disabled="!report.days.length" @click="downloadCsv(s)">Baixar planilha (CSV)</button>
        </div>
        <div class="table-wrap">
          <table v-if="report.days.length">
            <thead><tr><th>Dia</th><th>Posição</th><th>Visualizações</th><th>Cliques</th></tr></thead>
            <tbody>
              <tr v-for="d in report.days" :key="d.day + d.placement">
                <td>{{ new Date(d.day + 'T12:00:00').toLocaleDateString('pt-BR') }}</td>
                <td>{{ PLACE_LABEL[d.placement] }}</td>
                <td>{{ d.views.toLocaleString('pt-BR') }}</td>
                <td>{{ d.clicks.toLocaleString('pt-BR') }}</td>
              </tr>
            </tbody>
          </table>
          <p v-else class="muted small">Sem visualizações nos últimos 30 dias.</p>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.head {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: 16px;
  margin-bottom: 16px;
  flex-wrap: wrap;
}

h1,
h2 {
  margin: 0 0 4px;
}

.head p {
  margin: 0;
}

.small {
  font-size: 12px;
}

.notice {
  color: var(--good);
}

.form {
  display: flex;
  flex-direction: column;
  gap: 14px;
  margin-bottom: 16px;
}

.images {
  display: grid;
  grid-template-columns: 2fr 1fr;
  gap: 14px;
}

.img-field {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.preview {
  display: grid;
  place-items: center;
  border-radius: 12px;
  border: 1px dashed var(--line-strong);
  background: var(--bg-soft) center / cover no-repeat;
  color: var(--text-faint);
  font-size: 13px;
}

.preview.wide {
  aspect-ratio: 8 / 1;
}

.preview.side {
  aspect-ratio: 2 / 1;
}

.file {
  position: relative;
  align-self: flex-start;
  overflow: hidden;
}

.file input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}

.row {
  display: flex;
  gap: 8px;
}

.places {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 10px;
  margin: 0;
  padding: 0;
  border: 0;
}

.places legend {
  margin-bottom: 6px;
  font-weight: 600;
}

.place {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  padding: 10px 12px;
  border-radius: 12px;
  border: 1px solid var(--line);
  cursor: pointer;
}

.place.on {
  border-color: var(--lime);
  background: var(--lime-soft);
}

.place span {
  display: flex;
  flex-direction: column;
}

.grid-3 {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
}

.check {
  display: flex;
  gap: 8px;
  align-items: center;
}

.actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.item {
  display: grid;
  grid-template-columns: 200px 1fr auto auto;
  gap: 14px;
  align-items: center;
  padding: 14px 0;
  border-bottom: 1px solid var(--line);
}

.item:last-child {
  border-bottom: 0;
}

.thumb {
  aspect-ratio: 8 / 1;
  border-radius: 8px;
  background: var(--bg-soft) center / cover no-repeat;
  border: 1px solid var(--line);
}

.info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.title {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

.nums {
  display: flex;
  gap: 16px;
}

.nums div {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}

.nums small {
  color: var(--text-dim);
  font-size: 11px;
}

.item-actions {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.report {
  grid-column: 1 / -1;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  border-radius: 12px;
  background: var(--bg-soft);
}

.totals {
  display: flex;
  gap: 18px;
  align-items: center;
  flex-wrap: wrap;
}

.totals div {
  display: flex;
  flex-direction: column;
}

.totals small {
  color: var(--text-dim);
}

.totals .btn {
  margin-left: auto;
}

.table-wrap {
  overflow-x: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 14px;
}

th,
td {
  text-align: left;
  padding: 6px;
  border-bottom: 1px solid var(--line);
}

@media (max-width: 1000px) {
  .item {
    grid-template-columns: 1fr;
  }

  .nums div {
    align-items: flex-start;
  }

  .item-actions {
    justify-content: flex-start;
  }

  .images,
  .grid-3 {
    grid-template-columns: 1fr;
  }
}
</style>

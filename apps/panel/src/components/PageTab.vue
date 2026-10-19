<script setup lang="ts">
/**
 * Página personalizada do servidor (PRO): fundo em gradiente, cor de destaque e blocos
 * (texto, imagem/GIF, galeria e vídeo do YouTube) que aparecem no launcher, abaixo do
 * "Sobre o servidor". Quando o PRO vence, a página fica guardada aqui e volta ao renovar.
 */
import { computed, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import {
  api,
  formatDate,
  message,
  session,
  uploadPageMedia,
  youtubeId,
  type PageBlock,
  type PageData,
  type PageState,
  type PageTheme,
  type Server,
} from '../lib/api'

const props = defineProps<{ server: Server }>()
const emit = defineEmits<{ changed: [] }>()
const router = useRouter()

const MAX_BLOCKS = 20
const MAX_GALLERY = 12
const MAX_IMAGE = 5 * 1024 * 1024
const MAX_GIF = 8 * 1024 * 1024
const TYPES = ['image/png', 'image/jpeg', 'image/webp', 'image/gif']

const PRESETS: { name: string; colors: string[]; angle: number }[] = [
  { name: 'Conexão', colors: ['#0a0f11', '#1d3410', '#4d7a00'], angle: 160 },
  { name: 'Medieval', colors: ['#1a1208', '#4a2e12', '#8a5a1c'], angle: 170 },
  { name: 'Noite', colors: ['#070b1c', '#1b2560', '#4b3c9e'], angle: 160 },
  { name: 'Inferno', colors: ['#140606', '#5a1010', '#c2410c'], angle: 150 },
  { name: 'Oceano', colors: ['#03141c', '#0b4d63', '#1592a8'], angle: 165 },
  { name: 'Neon', colors: ['#0b0614', '#3d0a52', '#d1006f'], angle: 135 },
  { name: 'Floresta', colors: ['#06120a', '#14432a', '#2f7d4a'], angle: 175 },
  { name: 'Gelo', colors: ['#0b1620', '#2c4c66', '#8fb8d6'], angle: 160 },
]

type EditBlock = PageBlock & { key: number }

const state = ref<PageState | null>(null)
const loadError = ref<string | null>(null)
const error = ref<string | null>(null)
const saved = ref(false)
const saving = ref(false)
const dirty = ref(false)
const uploading = ref<number | null>(null)
let nextKey = 1

const form = reactive({
  useTheme: false,
  theme: { colors: ['#0a0f11', '#1d3410', '#4d7a00'], angle: 160, style: 'linear', accent: null } as PageTheme,
  useAccent: false,
  accent: '#b7fe00',
  blocks: [] as EditBlock[],
})

const active = computed(() => !!state.value?.active)
// A equipe Conexão edita mesmo sem PRO (a API libera); os demais só com o PRO ativo.
const canEdit = computed(() => !!state.value?.can_edit && (active.value || !!session.user?.is_staff))
const usage = computed(() => state.value?.usage)

function fill(page: PageData) {
  form.useTheme = !!page.theme
  if (page.theme) form.theme = { ...page.theme, colors: [...page.theme.colors], style: page.theme.style ?? 'linear' }
  form.useAccent = !!page.theme?.accent
  if (page.theme?.accent) form.accent = page.theme.accent
  form.blocks = (page.blocks ?? []).map((b) => ({ ...(JSON.parse(JSON.stringify(b)) as PageBlock), key: nextKey++ }) as EditBlock)
  dirty.value = false
}

async function load() {
  loadError.value = null
  try {
    state.value = await api.page(props.server.slug)
    fill(state.value.page)
  } catch (e) {
    loadError.value = message(e)
  }
}
onMounted(load)

function touch() {
  dirty.value = true
  saved.value = false
}

// ------------------------------------------------------------------ tema

const gradient = computed(() => {
  if (!form.useTheme) return null
  const c = form.theme.colors.join(', ')
  return form.theme.style === 'radial' ? `radial-gradient(circle at 50% 0%, ${c})` : `linear-gradient(${form.theme.angle}deg, ${c})`
})

function usePreset(p: (typeof PRESETS)[number]) {
  form.useTheme = true
  form.theme = { ...form.theme, colors: [...p.colors], angle: p.angle, style: 'linear' }
  touch()
}

function addColor() {
  if (form.theme.colors.length < 4) form.theme.colors.push(form.theme.colors[form.theme.colors.length - 1])
  touch()
}

function removeColor(i: number) {
  if (form.theme.colors.length > 2) form.theme.colors.splice(i, 1)
  touch()
}

// ---------------------------------------------------------------- blocos

function addBlock(type: PageBlock['type']) {
  if (form.blocks.length >= MAX_BLOCKS) return
  const base = { key: nextKey++ }
  const block: EditBlock =
    type === 'text'
      ? { ...base, type, title: '', body: '' }
      : type === 'image'
        ? { ...base, type, url: '', caption: '' }
        : type === 'gallery'
          ? { ...base, type, urls: [], caption: '' }
          : { ...base, type, youtube: '', caption: '' }
  form.blocks.push(block)
  touch()
}

function move(i: number, delta: number) {
  const j = i + delta
  if (j < 0 || j >= form.blocks.length) return
  const [b] = form.blocks.splice(i, 1)
  form.blocks.splice(j, 0, b)
  touch()
}

function remove(i: number) {
  form.blocks.splice(i, 1)
  touch()
}

function checkFile(file: File): string | null {
  if (!TYPES.includes(file.type)) return 'Use PNG, JPEG, WebP ou GIF.'
  const max = file.type === 'image/gif' ? MAX_GIF : MAX_IMAGE
  if (file.size > max) return `${file.type === 'image/gif' ? 'GIF' : 'Imagem'} com no máximo ${max / 1024 / 1024} MB.`
  return null
}

async function pick(event: Event, block: EditBlock) {
  const input = event.target as HTMLInputElement
  const files = Array.from(input.files ?? [])
  input.value = ''
  if (!files.length) return
  error.value = null
  uploading.value = block.key
  try {
    for (const file of files) {
      const problem = checkFile(file)
      if (problem) throw new Error(`${file.name}: ${problem}`)
      if (block.type === 'gallery' && block.urls.length >= MAX_GALLERY) break
      const { url } = await uploadPageMedia(props.server.slug, file)
      if (block.type === 'image') block.url = url
      else if (block.type === 'gallery') block.urls.push(url)
      touch()
    }
  } catch (e) {
    error.value = message(e)
  } finally {
    uploading.value = null
    if (state.value) {
      // Atualiza a contagem de envios do dia sem perder o que está sendo editado.
      api.page(props.server.slug).then((s) => state.value && (state.value.usage = s.usage)).catch(() => {})
    }
  }
}

const LABELS: Record<PageBlock['type'], string> = { text: 'Texto', image: 'Imagem ou GIF', gallery: 'Galeria', video: 'Vídeo do YouTube' }

// ------------------------------------------------------------------ salvar

function problems(): string | null {
  for (const [i, b] of form.blocks.entries()) {
    const n = `Bloco ${i + 1} (${LABELS[b.type]})`
    if (b.type === 'text' && !b.title.trim() && !b.body.trim()) return `${n}: escreva um título ou um texto.`
    if (b.type === 'image' && !b.url) return `${n}: envie a imagem.`
    if (b.type === 'gallery' && b.urls.length < 2) return `${n}: a galeria precisa de pelo menos 2 imagens.`
    if (b.type === 'video' && !youtubeId(b.youtube)) return `${n}: cole um link de vídeo do YouTube.`
  }
  return null
}

function payload(): PageData {
  const page: PageData = {}
  if (form.useTheme) {
    page.theme = { colors: [...form.theme.colors], angle: form.theme.angle, style: form.theme.style, accent: form.useAccent ? form.accent : null }
  }
  page.blocks = form.blocks.map(({ key: _key, ...block }) => block as PageBlock)
  return page
}

async function save() {
  error.value = problems()
  if (error.value) return
  saving.value = true
  saved.value = false
  try {
    state.value = await api.savePage(props.server.slug, payload())
    fill(state.value.page)
    saved.value = true
    emit('changed')
  } catch (e) {
    error.value = message(e)
  } finally {
    saving.value = false
  }
}

const previewAccent = computed(() => (form.useTheme && form.useAccent ? form.accent : '#b7fe00'))
const mb = (bytes: number) => `${(bytes / 1024 / 1024).toLocaleString('pt-BR', { maximumFractionDigits: 1 })} MB`
</script>

<template>
  <p v-if="loadError" class="error">{{ loadError }}</p>
  <template v-else-if="state">
    <section class="card intro">
      <div>
        <h2>
          Página do servidor
          <span v-if="active" class="badge good">Visível no launcher</span>
          <span v-else-if="state.page.blocks?.length || state.page.theme" class="badge warn">Escondida: PRO inativo</span>
          <span v-else class="badge brand">PRO</span>
        </h2>
        <p class="muted">
          Mostre a personalidade do servidor: fundo em gradiente, imagens, GIFs, galerias e vídeos do YouTube. Aparece na
          página do servidor no launcher, logo abaixo de "Sobre o servidor".
        </p>
      </div>
      <div v-if="!active" class="upsell">
        <p>
          {{
            state.page.blocks?.length || state.page.theme
              ? 'O PRO venceu: sua página está guardada e volta assim que você renovar.'
              : 'Disponível no plano PRO.'
          }}
        </p>
        <button class="btn btn-primary btn-sm" @click="router.replace({ query: { tab: 'pro' } })">Ver planos PRO</button>
      </div>
    </section>

    <div class="layout">
      <fieldset class="editor" :disabled="!canEdit">
        <section class="card">
          <h3>Fundo e cores</h3>
          <label class="check">
            <input v-model="form.useTheme" type="checkbox" @change="touch" />
            Usar fundo personalizado na página
          </label>
          <template v-if="form.useTheme">
            <div class="presets">
              <button
                v-for="p in PRESETS"
                :key="p.name"
                type="button"
                class="preset"
                :title="p.name"
                :style="{ background: `linear-gradient(${p.angle}deg, ${p.colors.join(', ')})` }"
                @click="usePreset(p)"
              >
                <span>{{ p.name }}</span>
              </button>
            </div>
            <div class="colors">
              <div v-for="(_, i) in form.theme.colors" :key="i" class="color">
                <input v-model="form.theme.colors[i]" type="color" @input="touch" />
                <button v-if="form.theme.colors.length > 2" type="button" class="x" title="Remover cor" @click="removeColor(i)">×</button>
              </div>
              <button v-if="form.theme.colors.length < 4" type="button" class="btn btn-sm" @click="addColor">+ cor</button>
              <div class="seg">
                <button type="button" :class="{ on: form.theme.style === 'linear' }" @click="(form.theme.style = 'linear'), touch()">Linear</button>
                <button type="button" :class="{ on: form.theme.style === 'radial' }" @click="(form.theme.style = 'radial'), touch()">Radial</button>
              </div>
            </div>
            <label v-if="form.theme.style === 'linear'" class="field angle">
              Ângulo: {{ form.theme.angle }}°
              <input v-model.number="form.theme.angle" type="range" min="0" max="360" @input="touch" />
            </label>
            <label class="check">
              <input v-model="form.useAccent" type="checkbox" @change="touch" />
              Cor de destaque (títulos, ícone e botão de play)
              <input v-if="form.useAccent" v-model="form.accent" type="color" class="accent" @input="touch" />
            </label>
            <p class="hint muted">O launcher escurece um pouco o fundo para o texto continuar legível.</p>
          </template>
        </section>

        <section class="card">
          <h3>Blocos <span class="muted count">{{ form.blocks.length }}/{{ MAX_BLOCKS }}</span></h3>
          <p v-if="!form.blocks.length" class="muted empty">Nenhum bloco ainda. Comece com um texto de boas-vindas ou um vídeo do servidor.</p>

          <article v-for="(block, i) in form.blocks" :key="block.key" class="block">
            <header>
              <strong>{{ i + 1 }}. {{ LABELS[block.type] }}</strong>
              <div class="tools">
                <button type="button" class="btn btn-sm" title="Subir" :disabled="i === 0" @click="move(i, -1)">↑</button>
                <button type="button" class="btn btn-sm" title="Descer" :disabled="i === form.blocks.length - 1" @click="move(i, 1)">↓</button>
                <button type="button" class="btn btn-sm btn-danger" @click="remove(i)">Remover</button>
              </div>
            </header>

            <template v-if="block.type === 'text'">
              <input v-model="block.title" class="input" maxlength="80" placeholder="Título (opcional)" @input="touch" />
              <textarea v-model="block.body" class="input" maxlength="3000" placeholder="Conte a história, as regras, os eventos…" @input="touch" />
            </template>

            <template v-else-if="block.type === 'image'">
              <div class="media-row">
                <div class="thumb big" :style="block.url ? { backgroundImage: `url(&quot;${block.url}&quot;)` } : undefined">
                  <span v-if="!block.url">sem imagem</span>
                </div>
                <label class="btn btn-sm file">
                  {{ uploading === block.key ? 'Enviando…' : block.url ? 'Trocar' : 'Enviar imagem ou GIF' }}
                  <input type="file" :accept="TYPES.join(',')" :disabled="uploading !== null" @change="pick($event, block)" />
                </label>
              </div>
              <input v-model="block.caption" class="input" maxlength="140" placeholder="Legenda (opcional)" @input="touch" />
            </template>

            <template v-else-if="block.type === 'gallery'">
              <div class="gallery">
                <div v-for="(url, j) in block.urls" :key="url + j" class="thumb" :style="{ backgroundImage: `url(&quot;${url}&quot;)` }">
                  <button type="button" class="x" title="Tirar da galeria" @click="block.urls.splice(j, 1), touch()">×</button>
                </div>
                <label v-if="block.urls.length < MAX_GALLERY" class="thumb add file">
                  {{ uploading === block.key ? 'Enviando…' : '+ imagens' }}
                  <input type="file" multiple :accept="TYPES.join(',')" :disabled="uploading !== null" @change="pick($event, block)" />
                </label>
              </div>
              <input v-model="block.caption" class="input" maxlength="140" placeholder="Legenda (opcional)" @input="touch" />
            </template>

            <template v-else-if="block.type === 'video'">
              <div class="media-row">
                <div
                  class="thumb big"
                  :style="youtubeId(block.youtube) ? { backgroundImage: `url(https://i.ytimg.com/vi/${youtubeId(block.youtube)}/hqdefault.jpg)` } : undefined"
                >
                  <span v-if="!youtubeId(block.youtube)">{{ block.youtube ? 'link inválido' : 'sem vídeo' }}</span>
                </div>
                <input v-model="block.youtube" class="input grow" placeholder="https://www.youtube.com/watch?v=…" @input="touch" />
              </div>
              <input v-model="block.caption" class="input" maxlength="140" placeholder="Legenda (opcional)" @input="touch" />
            </template>
          </article>

          <div class="add">
            <button type="button" class="btn btn-sm" :disabled="form.blocks.length >= MAX_BLOCKS" @click="addBlock('text')">+ Texto</button>
            <button type="button" class="btn btn-sm" :disabled="form.blocks.length >= MAX_BLOCKS" @click="addBlock('image')">+ Imagem ou GIF</button>
            <button type="button" class="btn btn-sm" :disabled="form.blocks.length >= MAX_BLOCKS" @click="addBlock('gallery')">+ Galeria</button>
            <button type="button" class="btn btn-sm" :disabled="form.blocks.length >= MAX_BLOCKS" @click="addBlock('video')">+ Vídeo do YouTube</button>
          </div>
          <p class="hint muted">
            Imagens: PNG, JPEG ou WebP até 5 MB. GIF animado até 8 MB (no máximo 1600×1600).
            <template v-if="usage">
              Envios nas últimas 24 h: {{ usage.uploads_24h }}/{{ usage.max_uploads_24h }} ({{ mb(usage.bytes_24h) }} de {{ mb(usage.max_bytes_24h) }}).
            </template>
          </p>
        </section>

        <div class="actions">
          <span v-if="state.updated_at && !dirty" class="muted small">Salvo em {{ formatDate(state.updated_at) }}</span>
          <span v-if="saved" class="ok">Página salva.</span>
          <span v-if="error" class="error">{{ error }}</span>
          <button type="button" class="btn btn-primary" :disabled="saving || !dirty" @click="save">
            {{ saving ? 'Salvando…' : 'Salvar página' }}
          </button>
        </div>
      </fieldset>

      <aside class="preview-wrap">
        <span class="label">Prévia no launcher</span>
        <div class="preview" :style="{ '--accent': previewAccent, ...(gradient ? { '--bg': gradient } : {}) }" :class="{ themed: !!gradient }">
          <div class="p-hero" :style="server.banner_url ? { backgroundImage: `url(&quot;${server.banner_url}&quot;)` } : undefined">
            <div class="p-icon" :style="server.icon_url ? { backgroundImage: `url(&quot;${server.icon_url}&quot;)` } : undefined" />
            <b>{{ server.name }}</b>
          </div>
          <div class="p-card">
            <i>Sobre o servidor</i>
            <span class="line" /><span class="line short" />
          </div>
          <template v-for="block in form.blocks" :key="block.key">
            <div v-if="block.type === 'text'" class="p-card">
              <i v-if="block.title" class="accent">{{ block.title }}</i>
              <p v-if="block.body">{{ block.body }}</p>
            </div>
            <div v-else-if="block.type === 'image'" class="p-img" :style="block.url ? { backgroundImage: `url(&quot;${block.url}&quot;)` } : undefined" />
            <div v-else-if="block.type === 'gallery'" class="p-card p-gallery">
              <span v-for="(url, j) in block.urls" :key="j" :style="{ backgroundImage: `url(&quot;${url}&quot;)` }" />
            </div>
            <div
              v-else-if="block.type === 'video'"
              class="p-img p-video"
              :style="youtubeId(block.youtube) ? { backgroundImage: `url(https://i.ytimg.com/vi/${youtubeId(block.youtube)}/hqdefault.jpg)` } : undefined"
            >
              <span class="play">▶</span>
            </div>
          </template>
        </div>
      </aside>
    </div>
  </template>
</template>

<style scoped>
.intro {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 20px;
  margin-bottom: 16px;
}

.intro p {
  margin: 0;
}

h2 {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0 0 4px;
}

h3 {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin: 0 0 12px;
}

.count {
  font-size: 13px;
  font-weight: 400;
}

.upsell {
  flex: none;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 8px;
  max-width: 280px;
  text-align: right;
  font-size: 13px;
  color: var(--warn);
}

.layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 320px;
  gap: 16px;
  align-items: start;
}

.editor {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin: 0;
  padding: 0;
  border: 0;
  min-width: 0;
}

.editor input[type='checkbox'],
.editor input[type='range'] {
  accent-color: var(--lime);
}

.editor:disabled {
  opacity: 0.6;
}

.check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 14px;
  margin-bottom: 12px;
}

.presets {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
  gap: 8px;
  margin-bottom: 12px;
}

.preset {
  height: 48px;
  border-radius: 10px;
  border: 1px solid var(--line);
  display: flex;
  align-items: flex-end;
  padding: 6px 8px;
  color: #fff;
  font-size: 12px;
  font-weight: 700;
  text-shadow: 0 1px 4px rgba(0, 0, 0, 0.7);
  cursor: pointer;
}

.preset:hover {
  border-color: var(--lime);
}

.colors {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  margin-bottom: 12px;
}

.color {
  position: relative;
}

input[type='color'] {
  width: 44px;
  height: 36px;
  padding: 2px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--bg-input);
  cursor: pointer;
}

.color .x,
.thumb .x {
  position: absolute;
  top: -6px;
  right: -6px;
  width: 18px;
  height: 18px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  border: none;
  background: var(--bad);
  color: #fff;
  font-size: 12px;
  line-height: 1;
  cursor: pointer;
}

.seg {
  display: inline-flex;
  margin-left: auto;
  border: 1px solid var(--line);
  border-radius: 10px;
  overflow: hidden;
}

.seg button {
  border: none;
  background: none;
  padding: 7px 12px;
  font-weight: 600;
  color: var(--text-dim);
  cursor: pointer;
}

.seg button.on {
  background: var(--lime);
  color: var(--ink);
}

.angle {
  margin-bottom: 12px;
}

.accent {
  margin-left: 6px;
}

.hint {
  margin: 0;
  font-size: 12px;
}

.small {
  font-size: 12px;
}

.empty {
  margin: 0 0 12px;
}

.block {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  margin-bottom: 10px;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: var(--bg-soft);
}

.block header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}

.tools {
  display: flex;
  gap: 6px;
}

.media-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.grow {
  flex: 1;
}

.thumb {
  position: relative;
  flex: none;
  width: 96px;
  height: 60px;
  display: grid;
  place-items: center;
  border-radius: 10px;
  border: 1px dashed var(--line);
  background: var(--bg-input) center / cover;
  color: var(--text-faint);
  font-size: 12px;
}

.thumb.big {
  width: 160px;
  height: 90px;
}

.gallery {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.file {
  position: relative;
  overflow: hidden;
  cursor: pointer;
}

.file input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}

.thumb.add:hover {
  border-color: var(--lime);
  color: var(--lime);
}

.add {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 4px 0 10px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 12px;
}

.preview-wrap {
  position: sticky;
  top: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.label {
  font-family: var(--font-pixel);
  font-size: 11px;
  color: var(--lime);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.preview {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 640px;
  overflow-y: auto;
  padding: 10px;
  border-radius: 14px;
  border: 1px solid var(--line);
  background-color: #0a0f11;
}

.preview.themed {
  background-image: linear-gradient(rgba(6, 10, 11, 0.15), rgba(6, 10, 11, 0.6)), var(--bg);
}

.p-hero {
  display: flex;
  align-items: flex-end;
  gap: 8px;
  min-height: 70px;
  padding: 8px;
  border-radius: 10px;
  background: linear-gradient(135deg, #15241a, #0b1113) center / cover;
  color: #fff;
  font-size: 13px;
  text-shadow: 0 1px 6px rgba(0, 0, 0, 0.8);
}

.p-icon {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  border: 2px solid var(--accent);
  background: #1c2a20 center / cover;
}

.p-card {
  padding: 8px 10px;
  border-radius: 10px;
  background: rgba(17, 24, 27, 0.92);
  border: 1px solid rgba(255, 255, 255, 0.06);
  font-size: 11px;
  color: #a3b1ad;
}

.p-card i {
  display: block;
  font-style: normal;
  font-weight: 700;
  color: #f2f6ef;
  margin-bottom: 4px;
}

.p-card i.accent {
  color: var(--accent);
}

.p-card p {
  margin: 0;
  white-space: pre-line;
  display: -webkit-box;
  -webkit-line-clamp: 4;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.line {
  display: block;
  height: 5px;
  margin-top: 4px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.12);
}

.line.short {
  width: 60%;
}

.p-img {
  position: relative;
  aspect-ratio: 16 / 9;
  border-radius: 10px;
  background: #11181b center / cover;
  border: 1px solid rgba(255, 255, 255, 0.06);
}

.p-gallery {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 4px;
  padding: 6px;
}

.p-gallery span {
  aspect-ratio: 16 / 10;
  border-radius: 5px;
  background: #1c2a20 center / cover;
}

.play {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 30px;
  height: 30px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  background: var(--accent);
  color: #0a0f11;
  font-size: 12px;
}

@media (max-width: 1100px) {
  .layout {
    grid-template-columns: 1fr;
  }

  .preview-wrap {
    position: static;
  }
}
</style>

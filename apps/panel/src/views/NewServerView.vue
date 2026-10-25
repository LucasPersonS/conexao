<script setup lang="ts">
/**
 * Cadastro "drop-in": o parceiro digita o endereço e arrasta o modpack.
 * Versões, loader, MOTD e vagas são detectados; nada técnico para digitar.
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import DropZone from '../components/DropZone.vue'
import ImagePicker from '../components/ImagePicker.vue'
import VersionSelect from '../components/VersionSelect.vue'
import {
  api,
  formatBytes,
  LOADER_NAMES,
  loadSession,
  message,
  dataUrlToBlob,
  sha1All,
  uploadImage,
  type DetectedServer,
  type LinkView,
  type Loader,
  type ResolvedFile,
} from '../lib/api'
import { detectPack, inferFromJars, inferFromModrinth, type DetectedPack, type InputFile } from '../lib/packdetect'
import { pendingPack } from '../lib/pending'

const router = useRouter()
const route = useRoute()

// ------------------------------------------------------------ formulário
const address = ref('')
const name = ref('')
const slug = ref('')
const slugTouched = ref(false)
const description = ref('')
const tags = ref('')
const discord = ref('')
const whitelist = ref(true)
const minecraft = ref('1.21.1')
const loader = ref<Loader>('neoforge')
const loaderVersion = ref('')

const busy = ref(false)
const error = ref<string | null>(null)
const iconFile = ref<File | null>(null)
const bannerFile = ref<File | null>(null)

function slugify(value: string) {
  return value
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48)
}
watch(name, (value) => {
  if (!slugTouched.value) slug.value = slugify(value)
})

// ----------------------------------------------------- vínculo do Companion
const linkCode = typeof route.query.link === 'string' ? route.query.link : null
const link = ref<LinkView | null>(null)

onMounted(async () => {
  if (!linkCode) return
  try {
    link.value = await api.companionLink(linkCode)
    const info = link.value.info
    if (info.minecraft) minecraft.value = info.minecraft
    if (info.loader) loader.value = info.loader
    if (info.loader_version) loaderVersion.value = info.loader_version
    if (info.motd) {
      description.value = info.motd
      name.value ||= info.motd.split('\n')[0].slice(0, 64)
    }
    whitelist.value = info.whitelist || whitelist.value
  } catch (e) {
    error.value = `Código de vínculo inválido ou expirado: ${message(e)}`
  }
})

// --------------------------------------------------------- detectar endereço
const detecting = ref(false)
const detected = ref<DetectedServer | null>(null)

async function detectAddress() {
  if (!address.value.trim()) return
  detecting.value = true
  error.value = null
  try {
    const result = await api.detectServer(address.value.trim())
    detected.value = result
    if (result.online) {
      if (result.motd) {
        description.value ||= result.motd
        name.value ||= result.motd.split('\n')[0].slice(0, 64)
      }
      if (!pack.value && !link.value) {
        if (result.minecraft) minecraft.value = result.minecraft
        if (result.loader) loader.value = result.loader
      }
    }
  } catch (e) {
    error.value = message(e)
  } finally {
    detecting.value = false
  }
}

// ----------------------------------------------------------- modpack drop-in
const pack = ref<DetectedPack | null>(null)
const analyzing = ref('')
const modrinthCount = ref(0)
const ownBytes = ref(0)
const detectedVersions = ref<{ minecraft?: string | null; loader?: Loader | null; loaderVersion?: string | null }>({})

async function onFiles(files: InputFile[]) {
  error.value = null
  analyzing.value = 'Lendo o modpack…'
  pack.value = null
  try {
    const result = await detectPack(files)
    if (!result.files.length && !result.external.length) {
      throw new Error('Não encontramos mods/, config/… nesta seleção. Escolha a pasta do modpack ou da instância.')
    }
    const known = (await api.metaMinecraft()).map((v) => v.id)

    // Hash dos mods para reconhecer no Modrinth (o mesmo hash é reaproveitado na publicação).
    const jars = result.files.filter((f) => f.path.startsWith('mods/'))
    await sha1All(result.files, (done, total) => {
      analyzing.value = `Identificando arquivos (${done}/${total})`
    })
    const hashes = [...new Set(result.files.map((f) => f.sha1 as string))]
    const resolved: Record<string, ResolvedFile> = hashes.length ? (await api.resolveFiles(hashes)).resolved : {}
    modrinthCount.value = result.files.filter((f) => resolved[f.sha1 as string]).length + result.external.length
    ownBytes.value = result.files.filter((f) => !resolved[f.sha1 as string]).reduce((s, f) => s + f.file.size, 0)

    if (!result.minecraft || !result.loader) {
      const viaModrinth = inferFromModrinth(Object.values(resolved), known)
      result.minecraft ??= viaModrinth.minecraft
      result.loader ??= viaModrinth.loader
      if (viaModrinth.minecraft && !result.evidence) result.evidence = `${viaModrinth.votes} mods reconhecidos no Modrinth`
    }
    if (!result.minecraft || !result.loader) {
      analyzing.value = 'Lendo os metadados dos mods…'
      const viaJars = await inferFromJars(jars, known, (done, total) => {
        analyzing.value = `Lendo os metadados dos mods (${done}/${total})`
      })
      result.minecraft ??= viaJars.minecraft
      result.loader ??= viaJars.loader
      if (viaJars.minecraft && !result.evidence) result.evidence = `metadados de ${viaJars.scanned} mods`
    }
    pack.value = result
    detectedVersions.value = { minecraft: result.minecraft, loader: result.loader, loaderVersion: result.loaderVersion }
    if (result.minecraft) minecraft.value = result.minecraft
    if (result.loader) loader.value = result.loader
    if (result.loaderVersion) loaderVersion.value = result.loaderVersion
  } catch (e) {
    error.value = message(e)
  } finally {
    analyzing.value = ''
  }
}

const packSummary = computed(() => {
  const p = pack.value
  if (!p) return ''
  const mods = p.files.filter((f) => f.path.startsWith('mods/')).length + p.external.filter((f) => f.path.startsWith('mods/')).length
  const configs = p.files.filter((f) => /^(config|defaultconfigs)\//.test(f.path)).length
  return `${mods} mods · ${configs} configs`
})

const mismatch = computed(() => {
  const d = detected.value
  if (!d?.online || !pack.value?.minecraft) return null
  if (d.minecraft && d.minecraft !== pack.value.minecraft) {
    return `O servidor respondeu Minecraft ${d.minecraft}, mas o modpack é para ${pack.value.minecraft}. Confira antes de publicar.`
  }
  return null
})

// -------------------------------------------------------------------- envio
const canSubmit = computed(() => name.value.trim() && slug.value.trim() && address.value.trim() && loaderVersion.value)

async function submit() {
  busy.value = true
  error.value = null
  try {
    const created = await api.createServer({
      slug: slug.value.trim(),
      name: name.value.trim(),
      description: description.value,
      address: address.value.trim(),
      minecraft: minecraft.value,
      loader: { kind: loader.value, version: loaderVersion.value },
      tags: tags.value.split(',').map((t) => t.trim()).filter(Boolean),
      icon_url: null,
      banner_url: null,
      discord_url: discord.value.trim() || null,
      whitelist_required: whitelist.value,
    })
    if (linkCode && link.value) await api.confirmLink(linkCode, created.slug)
    // Ícone: o escolhido, ou o do próprio servidor (server-icon.png detectado no ping).
    const icon = iconFile.value ?? (detected.value?.favicon ? dataUrlToBlob(detected.value.favicon) : null)
    const uploads: Promise<unknown>[] = []
    if (icon) uploads.push(uploadImage(created.slug, 'icon', icon))
    if (bannerFile.value) uploads.push(uploadImage(created.slug, 'banner', bannerFile.value))
    await Promise.allSettled(uploads)
    await loadSession()
    if (pack.value) {
      pendingPack.value = { slug: created.slug, pack: pack.value }
      router.push(`/s/${created.slug}?tab=modpack`)
    } else {
      router.push(`/s/${created.slug}${link.value ? '?tab=servidor' : ''}`)
    }
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <h1>Cadastrar servidor</h1>
  <p class="muted lead">
    Gratuito para parceiros. Digite o endereço e arraste o modpack: o resto o Conexão descobre sozinho. O servidor aparece
    no launcher depois da aprovação da equipe Conexão.
  </p>

  <div v-if="link" class="card linked">
    <strong>Servidor detectado pelo Conexão Companion</strong>
    <span class="muted">
      {{ link.info.loader ? LOADER_NAMES[link.info.loader] : '' }} {{ link.info.loader_version }} · Minecraft
      {{ link.info.minecraft }} · {{ link.info.max_players }} vagas. Ao cadastrar, ele fica vinculado e a whitelist e os
      mods passam a sincronizar sozinhos.
    </span>
  </div>

  <section class="card step">
    <h2><span class="num">1</span> Onde os jogadores entram?</h2>
    <div class="row">
      <input
        v-model="address"
        class="input grow"
        placeholder="play.seuservidor.com"
        @keydown.enter.prevent="detectAddress"
        @blur="!detected && detectAddress()"
      />
      <button class="btn" :disabled="detecting || !address.trim()" @click="detectAddress">
        {{ detecting ? 'Verificando…' : 'Verificar servidor' }}
      </button>
    </div>
    <div v-if="detected" class="detected" :class="{ off: !detected.online }">
      <img v-if="detected.favicon" :src="detected.favicon" alt="" class="favicon" />
      <div>
        <template v-if="detected.online">
          <strong class="ok">Servidor online</strong>
          <span class="muted">
            {{ detected.version_name }} · {{ detected.players_online }}/{{ detected.players_max }} jogadores ·
            {{ detected.latency_ms }} ms
            <template v-if="detected.mods.length"> · {{ detected.mods.length }} mods anunciados</template>
          </span>
          <span v-if="detected.motd" class="motd">{{ detected.motd }}</span>
        </template>
        <template v-else>
          <strong class="warn-text">Não conseguimos falar com o servidor agora</strong>
          <span class="muted">{{ detected.error }} Você pode cadastrar mesmo assim.</span>
        </template>
      </div>
    </div>
  </section>

  <section class="card step">
    <h2><span class="num">2</span> Modpack dos jogadores <span class="muted opt">(pode deixar para depois)</span></h2>
    <DropZone v-if="!pack && !analyzing" @files="onFiles" />
    <div v-else-if="analyzing" class="analyzing">
      <div class="spinner" />
      <span>{{ analyzing }}</span>
    </div>
    <div v-else-if="pack" class="pack">
      <div class="pack-head">
        <div>
          <strong>{{ pack.sourceLabel }}</strong>
          <span class="muted">{{ packSummary }}</span>
        </div>
        <button class="link" @click="pack = null">trocar</button>
      </div>
      <div class="facts">
        <span class="badge good">{{ modrinthCount }} arquivos baixados direto do Modrinth</span>
        <span class="badge">{{ formatBytes(ownBytes) }} de arquivos próprios (configs, mods privados)</span>
      </div>
      <p v-if="pack.evidence" class="muted small">
        Versões detectadas por <b>{{ pack.evidence }}</b>.
      </p>
      <p v-else class="warn-text small">Não foi possível detectar as versões. Escolha abaixo.</p>
      <p v-for="note in pack.notes" :key="note" class="muted small">{{ note }}</p>
      <p v-if="mismatch" class="warn-text small">{{ mismatch }}</p>
    </div>
  </section>

  <section class="card step">
    <h2><span class="num">3</span> Versões</h2>
    <VersionSelect
      v-model:minecraft="minecraft"
      v-model:loader="loader"
      v-model:loader-version="loaderVersion"
      :detected="detectedVersions"
    />
    <p class="muted small">Minecraft 1.18.2 ou mais novo. O launcher instala Java, loader e mods para os jogadores.</p>
  </section>

  <section class="card step">
    <h2><span class="num">4</span> Como o servidor aparece no launcher</h2>
    <div class="grid-2">
      <label class="field">
        Nome
        <input v-model="name" class="input" maxlength="64" placeholder="Paravexia RP" />
      </label>
      <label class="field">
        Endereço da página (URL)
        <input
          v-model="slug"
          class="input"
          pattern="[a-z0-9][a-z0-9-]{1,47}"
          placeholder="paravexia"
          @input="slugTouched = true"
        />
      </label>
    </div>
    <label class="field">
      Descrição
      <textarea v-model="description" class="input" maxlength="2000" placeholder="Tema do RP, regras, eventos…" />
    </label>
    <div class="grid-2">
      <label class="field">
        Estilos (separados por vírgula)
        <input v-model="tags" class="input" placeholder="Medieval, RP médio, Economia" />
      </label>
      <label class="field">
        Discord
        <input v-model="discord" class="input" type="url" placeholder="https://discord.gg/…" />
      </label>
    </div>
    <div class="grid-2">
      <ImagePicker kind="icon" :current="detected?.favicon ?? null" @pick="iconFile = $event" />
      <ImagePicker kind="banner" :current="null" @pick="bannerFile = $event" />
    </div>
    <label class="check">
      <input v-model="whitelist" type="checkbox" />
      Exigir whitelist (jogadores pedem acesso e a equipe aprova pelo painel)
    </label>
  </section>

  <div class="submit">
    <p v-if="error" class="error">{{ error }}</p>
    <button class="btn btn-primary big" :disabled="busy || !canSubmit" @click="submit">
      {{ busy ? 'Cadastrando…' : pack ? 'Cadastrar e revisar o modpack' : 'Cadastrar servidor' }}
    </button>
  </div>
</template>

<style scoped>
.lead {
  margin: 6px 0 20px;
  max-width: 720px;
  line-height: 1.5;
}

.step {
  display: flex;
  flex-direction: column;
  gap: 14px;
  margin-bottom: 16px;
}

h2 {
  margin: 0;
  font-size: 17px;
  display: flex;
  align-items: center;
  gap: 10px;
}

.num {
  width: 26px;
  height: 26px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  background: var(--brand-light);
  color: var(--brand-strong);
  font-size: 13px;
  font-weight: 800;
}

.opt {
  font-size: 13px;
  font-weight: 500;
}

.row {
  display: flex;
  gap: 10px;
}

.grow {
  flex: 1;
}

.detected {
  display: flex;
  gap: 14px;
  align-items: center;
  padding: 12px 14px;
  border-radius: 12px;
  background: var(--good-bg);
  border: 1px solid var(--good-line);
}

.detected.off {
  background: var(--warn-bg);
  border-color: var(--warn-line);
}

.detected div {
  display: flex;
  flex-direction: column;
  gap: 3px;
  font-size: 13px;
}

.favicon {
  width: 48px;
  height: 48px;
  border-radius: 10px;
  image-rendering: pixelated;
}

.motd {
  white-space: pre-line;
  color: var(--text);
}

.warn-text {
  color: var(--warn);
}

.analyzing {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 20px;
  color: var(--text-dim);
}

.spinner {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 3px solid var(--line);
  border-top-color: var(--brand);
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.pack {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.pack-head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
}

.pack-head div {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.facts {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.small {
  margin: 0;
  font-size: 13px;
}

.check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 14px;
}

.linked {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 16px;
  background: var(--bg-soft);
}

.submit {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 8px;
  margin: 4px 0 40px;
}

.big {
  height: 46px;
  padding: 0 24px;
  font-size: 15px;
}
</style>

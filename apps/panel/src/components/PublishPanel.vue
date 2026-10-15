<script setup lang="ts">
/**
 * Publicar uma versão do modpack: arrastar a pasta (ou .mrpack/.zip) ou usar
 * os mods do próprio servidor (Companion). O que está no Modrinth vira link;
 * só configs e arquivos próprios são enviados ao Conexão.
 */
import { computed, onMounted, ref, watch } from 'vue'
import DropZone from './DropZone.vue'
import VersionSelect from './VersionSelect.vue'
import {
  ApiError,
  LOADER_NAMES,
  api,
  formatBytes,
  message,
  sha1Hex,
  uploadFile,
  type Loader,
  type PlanFileInfo,
  type PlanRequestFile,
  type PlanResponse,
  type Server,
  type SnapshotView,
} from '../lib/api'
import { detectPack, type DetectedPack, type InputFile } from '../lib/packdetect'
import { pendingPack } from '../lib/pending'

const props = defineProps<{ server: Server; suggestedVersion: string }>()
const emit = defineEmits<{ published: [version: string] }>()

const CONFIG_GROUPS = ['config', 'defaultconfigs']

const pack = ref<DetectedPack | null>(null)
const mode = ref<'pack' | 'server'>('pack')
const version = ref(props.suggestedVersion)
const changeVersions = ref(false)
const minecraft = ref(props.server.minecraft)
const loader = ref<Loader>(props.server.loader.kind)
const loaderVersion = ref(props.server.loader.version)
const channel = ref<'stable' | 'beta'>('stable')

const step = ref<'pick' | 'working' | 'review' | 'done'>('pick')
const progressText = ref('')
const progress = ref(0)
const error = ref<string | null>(null)
const plan = ref<PlanResponse | null>(null)
const managed = ref<string[]>([])
const excluded = ref<string[]>([])
const includeServerOnly = ref<string[]>([])
const publishing = ref(false)
const snapshot = ref<SnapshotView | null>(null)

watch(
  () => props.suggestedVersion,
  (next) => {
    if (step.value === 'pick') version.value = next
  },
)

onMounted(async () => {
  api.snapshot(props.server.slug).then((s) => (snapshot.value = s)).catch(() => {})
  // Modpack arrastado no cadastro: segue direto para a análise.
  const pending = pendingPack.value
  if (pending && pending.slug === props.server.slug) {
    pendingPack.value = null
    usePack(pending.pack)
    if (!plan.value) await analyze()
  }
})

function usePack(detected: DetectedPack) {
  pack.value = detected
  mode.value = 'pack'
  plan.value = null
  error.value = null
  step.value = 'pick'
  const differs =
    (detected.minecraft && detected.minecraft !== props.server.minecraft) ||
    (detected.loader && detected.loader !== props.server.loader.kind) ||
    (detected.loaderVersion && detected.loaderVersion !== props.server.loader.version)
  if (detected.minecraft) minecraft.value = detected.minecraft
  if (detected.loader) loader.value = detected.loader
  if (detected.loaderVersion) loaderVersion.value = detected.loaderVersion
  changeVersions.value = !!differs
}

async function onFiles(files: InputFile[]) {
  error.value = null
  try {
    const detected = await detectPack(files)
    if (!detected.files.length && !detected.external.length) {
      throw new Error('Nenhuma pasta reconhecida (mods, config, defaultconfigs, resourcepacks, shaderpacks, kubejs).')
    }
    usePack(detected)
  } catch (e) {
    error.value = message(e)
  }
}

const totalBytes = computed(
  () =>
    (pack.value?.files.reduce((sum, p) => sum + p.file.size, 0) ?? 0) +
    (pack.value?.external.reduce((sum, p) => sum + p.size, 0) ?? 0),
)
const fileCount = computed(() => (pack.value?.files.length ?? 0) + (pack.value?.external.length ?? 0))

function planBody() {
  const files: PlanRequestFile[] = [
    ...(pack.value?.files ?? []).map((p) => ({ path: p.path, sha1: p.sha1 as string })),
    ...(pack.value?.external ?? []).map((e) => ({ path: e.path, sha1: e.sha1, url: e.url, sha512: e.sha512, size: e.size })),
  ]
  return {
    version: version.value.trim(),
    ...(changeVersions.value ? { minecraft: minecraft.value, loader: { kind: loader.value, version: loaderVersion.value } } : {}),
    files,
    managed: managed.value,
    excluded: excluded.value,
    include_server_only: includeServerOnly.value,
  }
}

async function analyze() {
  if (!pack.value) return
  error.value = null
  step.value = 'working'
  mode.value = 'pack'
  try {
    const local = pack.value.files
    // 1. Hash no navegador
    let done = 0
    for (const p of local) {
      if (!p.sha1) p.sha1 = await sha1Hex(p.file)
      done++
      progress.value = done / Math.max(local.length, 1)
      progressText.value = `Identificando arquivos (${done}/${local.length})`
    }
    // 2. Modrinth reconhece o que puder; envia só o resto
    const unique = [...new Set(local.map((p) => p.sha1 as string))]
    progressText.value = 'Procurando os mods no Modrinth…'
    const { missing } = unique.length ? await api.resolveFiles(unique) : { missing: [] as string[] }
    const toSend = missing.map((sha) => local.find((p) => p.sha1 === sha)).filter((p) => !!p)
    const sendBytes = toSend.reduce((sum, p) => sum + p.file.size, 0)
    let sent = 0
    let sentBytes = 0
    progress.value = 0
    const queue = [...toSend]
    const worker = async () => {
      for (let item = queue.shift(); item; item = queue.shift()) {
        await uploadFile(item.sha1 as string, item.file)
        sent++
        sentBytes += item.file.size
        progress.value = sendBytes ? sentBytes / sendBytes : 1
        progressText.value = `Enviando arquivos próprios (${sent}/${toSend.length} · ${formatBytes(sentBytes)} de ${formatBytes(sendBytes)})`
      }
    }
    progressText.value = toSend.length ? 'Enviando arquivos próprios…' : 'Nada para enviar: tudo já estava no Modrinth ou no Conexão'
    await Promise.all([worker(), worker(), worker()])
    await runPlan()
  } catch (e) {
    error.value = message(e)
    step.value = 'pick'
  }
}

async function runPlan() {
  progressText.value = 'Analisando o modpack…'
  progress.value = 1
  try {
    plan.value =
      mode.value === 'server'
        ? await api.planFromServer(props.server.slug, {
            version: version.value.trim(),
            excluded: excluded.value,
            include_server_only: includeServerOnly.value,
          })
        : await api.plan(props.server.slug, planBody())
    step.value = 'review'
  } catch (e) {
    if (mode.value === 'pack' && e instanceof ApiError && e.status === 409) {
      error.value = 'Alguns arquivos não chegaram. Tente de novo.'
    } else {
      error.value = message(e)
    }
    step.value = plan.value ? 'review' : 'pick'
  }
}

/** Usa os mods instalados no servidor (o Companion envia os que faltam). */
async function fromServer() {
  error.value = null
  mode.value = 'server'
  step.value = 'working'
  progress.value = 0
  progressText.value = 'Lendo os mods do servidor…'
  const started = Date.now()
  for (;;) {
    try {
      plan.value = await api.planFromServer(props.server.slug, {
        version: version.value.trim(),
        excluded: excluded.value,
        include_server_only: includeServerOnly.value,
      })
      step.value = 'review'
      return
    } catch (e) {
      const details = e instanceof ApiError ? (e.details as { waiting?: string[]; requested?: boolean } | null) : null
      if (e instanceof ApiError && e.status === 409 && details?.waiting && details.requested && Date.now() - started < 120_000) {
        progressText.value = `${e.message} (${details.waiting.length} restante(s))`
        progress.value = Math.min(0.95, (Date.now() - started) / 60_000)
        await new Promise((r) => setTimeout(r, 2500))
        continue
      }
      error.value = message(e)
      step.value = 'pick'
      return
    }
  }
}

function toggle(which: 'managed' | 'excluded' | 'server', path: string) {
  const list = which === 'managed' ? managed : which === 'excluded' ? excluded : includeServerOnly
  list.value = list.value.includes(path) ? list.value.filter((p) => p !== path) : [...list.value, path]
  runPlan()
}

const infoByPath = computed(() => new Map((plan.value?.files ?? []).map((f) => [f.path, f])))
const visibleChanges = computed(() => plan.value?.changes.filter((c) => c.kind !== 'unchanged') ?? [])
const count = (kind: string) => plan.value?.changes.filter((c) => c.kind === kind).length ?? 0
const configFiles = computed(() =>
  (plan.value?.manifest.files ?? []).filter((f) => CONFIG_GROUPS.includes(f.path.split('/')[0])),
)
const sources = computed(() => {
  const files = plan.value?.files ?? []
  const by = (s: PlanFileInfo['source']) => files.filter((f) => f.source === s)
  return { modrinth: by('modrinth').length, external: by('external').length, hosted: by('hosted').length }
})
const quotaPercent = computed(() =>
  plan.value && plan.value.quota_bytes ? Math.min(100, (plan.value.hosted_bytes / plan.value.quota_bytes) * 100) : 0,
)

async function publish() {
  if (!plan.value) return
  publishing.value = true
  error.value = null
  try {
    await api.publish(props.server.slug, plan.value.manifest, channel.value)
    step.value = 'done'
    emit('published', plan.value.manifest.version)
  } catch (e) {
    error.value = message(e)
  } finally {
    publishing.value = false
  }
}

function reset() {
  pack.value = null
  plan.value = null
  managed.value = []
  excluded.value = []
  includeServerOnly.value = []
  step.value = 'pick'
  mode.value = 'pack'
  version.value = props.suggestedVersion
}

const KIND = { added: '+', changed: '~', removed: '−', unchanged: '' } as const
const serverReady = computed(() => !!snapshot.value?.connected && (snapshot.value?.mods.length ?? 0) > 0)
</script>

<template>
  <section class="card">
    <h2>Publicar nova versão</h2>

    <template v-if="step === 'done'">
      <p class="ok big">Versão {{ plan?.manifest.version }} publicada no canal {{ channel === 'stable' ? 'estável' : 'beta' }}.</p>
      <p class="muted">Os jogadores recebem a atualização na próxima vez que clicarem em Jogar.</p>
      <button class="btn" @click="reset">Publicar outra versão</button>
    </template>

    <template v-else>
      <div v-if="step === 'pick'" class="sources-pick">
        <div v-if="serverReady" class="from-server" :class="{ highlight: snapshot?.out_of_sync }">
          <div>
            <strong>{{ snapshot?.out_of_sync ? 'Os mods do servidor mudaram' : 'Usar os mods do servidor' }}</strong>
            <span class="muted">
              O Conexão Companion vê {{ snapshot?.mods.length }} mods no servidor. Os do Modrinth viram link; os outros
              o servidor envia sozinho. Configs continuam como na versão atual.
            </span>
          </div>
          <button class="btn btn-primary" :disabled="!version.trim()" @click="fromServer">Montar a partir do servidor</button>
        </div>

        <DropZone v-if="!pack" compact @files="onFiles" />
        <div v-else class="picked">
          <div>
            <strong>{{ pack.sourceLabel }}</strong>
            <span class="muted">
              {{ fileCount }} arquivos · {{ formatBytes(totalBytes) }}
              <template v-if="pack.ignored"> · {{ pack.ignored }} ignorados (pastas não gerenciadas)</template>
            </span>
            <span v-if="pack.evidence" class="muted small">
              Detectado: {{ pack.loader ? LOADER_NAMES[pack.loader] : '?' }} {{ pack.loaderVersion ?? '' }} · Minecraft
              {{ pack.minecraft ?? '?' }} ({{ pack.evidence }})
            </span>
            <span v-for="note in pack.notes" :key="note" class="muted small">{{ note }}</span>
          </div>
          <button class="link" @click="pack = null">trocar</button>
        </div>
      </div>

      <div v-if="(pack || serverReady) && step === 'pick'" class="options">
        <label class="field">
          Nova versão
          <input v-model="version" class="input" placeholder="1.2.0" />
        </label>
        <label class="field">
          Canal
          <select v-model="channel" class="input">
            <option value="stable">Estável (todos os jogadores)</option>
            <option value="beta">Beta (teste)</option>
          </select>
        </label>
        <label v-if="pack" class="check">
          <input v-model="changeVersions" type="checkbox" />
          Mudar Minecraft ou loader
        </label>
      </div>
      <VersionSelect
        v-if="pack && changeVersions && step === 'pick'"
        v-model:minecraft="minecraft"
        v-model:loader="loader"
        v-model:loader-version="loaderVersion"
        :detected="{ minecraft: pack.minecraft, loader: pack.loader, loaderVersion: pack.loaderVersion }"
        class="versions"
      />

      <div v-if="step === 'working'" class="working">
        <div class="bar"><div :style="{ width: `${Math.round(progress * 100)}%` }" /></div>
        <span class="muted">{{ progressText }}</span>
      </div>

      <button v-if="pack && step === 'pick'" class="btn btn-primary" :disabled="!version.trim()" @click="analyze">
        Analisar modpack
      </button>

      <template v-if="step === 'review' && plan">
        <div class="summary">
          <strong>{{ plan.previous_version ? `${plan.previous_version} → ` : '' }}{{ plan.manifest.version }}</strong>
          <span class="muted">
            {{ LOADER_NAMES[plan.manifest.loader.kind] }} {{ plan.manifest.loader.version }} · Minecraft
            {{ plan.manifest.minecraft }}
          </span>
          <span class="badge good">{{ count('added') }} novos</span>
          <span class="badge warn">{{ count('changed') }} alterados</span>
          <span class="badge bad">{{ count('removed') }} removidos</span>
          <span class="badge">{{ count('unchanged') }} sem mudança</span>
        </div>

        <div class="storage">
          <div class="storage-row">
            <span><b>{{ sources.modrinth }}</b> baixados do Modrinth</span>
            <span v-if="sources.external"><b>{{ sources.external }}</b> links do pacote</span>
            <span><b>{{ sources.hosted }}</b> hospedados no Conexão</span>
            <span class="muted quota">{{ formatBytes(plan.hosted_bytes) }} de {{ formatBytes(plan.quota_bytes) }}</span>
          </div>
          <div class="bar thin"><div :style="{ width: `${quotaPercent}%` }" /></div>
        </div>

        <ul v-if="plan.warnings.length" class="warnings">
          <li v-for="(w, i) in plan.warnings" :key="i">{{ w }}</li>
        </ul>

        <details v-if="plan.server_only.length || includeServerOnly.length" class="configs">
          <summary>Mods só do servidor ({{ plan.server_only.length }}) — ficaram de fora do pacote dos jogadores</summary>
          <label v-for="f in plan.server_only" :key="f.path" class="config">
            <input type="checkbox" :checked="false" @change="toggle('server', f.path)" />
            <span>{{ f.title ?? f.path }}</span>
            <span class="muted">incluir mesmo assim</span>
          </label>
          <label v-for="p in includeServerOnly" :key="p" class="config">
            <input type="checkbox" checked @change="toggle('server', p)" />
            <span>{{ infoByPath.get(p)?.title ?? p }}</span>
            <span class="muted">incluído por você</span>
          </label>
        </details>

        <div v-if="visibleChanges.length" class="changes">
          <div v-for="c in visibleChanges" :key="c.path" class="change" :class="c.kind">
            <span class="kind">{{ KIND[c.kind] }}</span>
            <img v-if="infoByPath.get(c.path)?.icon_url" :src="infoByPath.get(c.path)?.icon_url ?? ''" alt="" class="icon" />
            <span class="path">{{ infoByPath.get(c.path)?.title ?? c.path }}</span>
            <span v-if="infoByPath.get(c.path)?.title || c.name !== c.path" class="muted name">{{ c.path }}</span>
            <span v-if="infoByPath.get(c.path)?.side === 'client'" class="badge">só cliente</span>
            <span v-if="infoByPath.get(c.path)?.source === 'hosted'" class="badge">hospedado</span>
            <button v-if="c.kind !== 'removed'" class="link right" @click="toggle('excluded', c.path)">deixar de fora</button>
          </div>
        </div>
        <p v-else class="muted">Nenhuma mudança em relação à versão estável.</p>

        <div v-if="excluded.length" class="excluded muted">
          Fora desta versão:
          <button v-for="p in excluded" :key="p" class="badge" @click="toggle('excluded', p)">{{ p }} ✕</button>
        </div>

        <details v-if="configFiles.length" class="configs">
          <summary>Configs ({{ configFiles.length }}) — por padrão só são instaladas se o jogador ainda não tiver o arquivo</summary>
          <label v-for="f in configFiles" :key="f.path" class="config">
            <input type="checkbox" :checked="f.policy === 'managed'" @change="toggle('managed', f.path)" />
            <span>{{ f.path }}</span>
            <span class="muted">{{ f.policy === 'managed' ? 'sempre sobrescreve' : 'preserva a do jogador' }}</span>
          </label>
        </details>

        <div class="actions">
          <button class="btn" :disabled="publishing" @click="reset">Cancelar</button>
          <button class="btn btn-primary" :disabled="publishing" @click="publish">
            {{ publishing ? 'Publicando…' : `Publicar ${plan.manifest.version} (${channel === 'stable' ? 'estável' : 'beta'})` }}
          </button>
        </div>
      </template>
    </template>

    <p v-if="error" class="error">{{ error }}</p>
  </section>
</template>

<style scoped>
h2 {
  margin-top: 0;
}

.muted {
  line-height: 1.5;
}

.small {
  font-size: 12px;
}

.big {
  font-size: 17px;
  font-weight: 700;
}

.sources-pick {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 14px;
}

.from-server {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 16px;
  border-radius: 14px;
  border: 1px solid var(--line);
  background: var(--bg-soft);
}

.from-server.highlight {
  border-color: var(--brand);
  background: var(--lime-soft);
}

.from-server div,
.picked div {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.picked {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  padding: 14px 16px;
  border-radius: 14px;
  border: 1px solid var(--line);
}

.options {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: 14px;
  align-items: end;
  margin-bottom: 14px;
}

.versions {
  margin-bottom: 14px;
}

.check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 14px;
  height: 38px;
}

.working {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 8px 0 14px;
}

.bar {
  height: 8px;
  border-radius: 999px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  overflow: hidden;
}

.bar.thin {
  height: 6px;
}

.bar div {
  height: 100%;
  background: var(--brand);
  transition: width 0.2s;
}

.summary {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin: 6px 0 14px;
}

.summary strong {
  font-size: 18px;
}

.storage {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 14px;
  font-size: 13px;
}

.storage-row {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
}

.quota {
  margin-left: auto;
}

.warnings {
  margin: 0 0 14px;
  padding: 12px 16px 12px 32px;
  border-radius: 12px;
  background: var(--warn-bg);
  border: 1px solid var(--warn-line);
  color: var(--warn);
  font-size: 13px;
  line-height: 1.5;
}

.changes {
  max-height: 340px;
  overflow-y: auto;
  border: 1px solid var(--line);
  border-radius: 12px;
  margin-bottom: 14px;
}

.change {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 12px;
  font-size: 13px;
  border-top: 1px solid var(--line);
}

.change:first-child {
  border-top: none;
}

.icon {
  width: 20px;
  height: 20px;
  border-radius: 5px;
}

.kind {
  width: 14px;
  font-weight: 800;
  text-align: center;
}

.added .kind {
  color: var(--good);
}

.changed .kind {
  color: var(--warn);
}

.removed .kind {
  color: var(--bad);
}

.removed .path {
  text-decoration: line-through;
  color: var(--text-dim);
}

.path {
  font-weight: 600;
}

.name {
  font-size: 12px;
  font-family: 'Cascadia Code', Consolas, monospace;
}

.right {
  margin-left: auto;
  color: var(--text-faint);
  font-size: 12px;
  font-weight: 500;
  text-decoration: underline;
}

.excluded {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  margin-bottom: 14px;
  font-size: 13px;
}

.excluded .badge {
  cursor: pointer;
}

.configs {
  margin-bottom: 16px;
  font-size: 14px;
}

.configs summary {
  cursor: pointer;
  font-weight: 600;
  color: var(--text-dim);
  margin-bottom: 8px;
}

.config {
  display: flex;
  gap: 10px;
  align-items: center;
  padding: 4px 0;
  font-size: 13px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>

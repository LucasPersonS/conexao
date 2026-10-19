<script setup lang="ts">
/** Aba "Servidor": Conexão Companion (recomendado) e o agente separado (alternativa). */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import AgentTab from './AgentTab.vue'
import {
  api,
  companionVersionOf,
  compareVersions,
  formatBytes,
  formatDate,
  LOADER_NAMES,
  message,
  pickCompanionBuild,
  type CompanionBuild,
  type CompanionBuilds,
  type Loader,
  type Server,
  type SnapshotView,
} from '../lib/api'

const props = defineProps<{ server: Server; role: string }>()
const router = useRouter()
const snapshot = ref<SnapshotView | null>(null)
const catalog = ref<CompanionBuilds | null>(null)
const error = ref<string | null>(null)
const code = ref('')
let timer: number | undefined

async function load() {
  try {
    snapshot.value = await api.snapshot(props.server.slug)
    error.value = null
  } catch (e) {
    error.value = message(e)
  }
}
async function loadBuilds() {
  try {
    catalog.value = await api.companionBuilds()
  } catch {
    catalog.value = { version: null, builds: [] }
  }
}
onMounted(() => {
  load()
  loadBuilds()
  timer = window.setInterval(load, 10_000)
})
onBeforeUnmount(() => window.clearInterval(timer))

const linked = computed(() => !!snapshot.value?.agent_version?.startsWith('companion'))
const unknownMods = computed(() => snapshot.value?.mods.filter((m) => m.source === 'unknown') ?? [])
const serverOnly = computed(() => snapshot.value?.mods.filter((m) => m.server_only) ?? [])

/** Versão e loader do servidor: o que o Companion informou ou, antes disso, o cadastro no painel. */
const target = computed<{ minecraft: string; loader: Loader }>(() => ({
  minecraft: snapshot.value?.info?.minecraft || props.server.minecraft,
  loader: snapshot.value?.info?.loader ?? props.server.loader.kind,
}))
const build = computed(() => pickCompanionBuild(catalog.value?.builds ?? [], target.value.minecraft, target.value.loader))

/** Todos os JARs, agrupados por versão do Minecraft (mais nova primeiro). */
const byVersion = computed(() => {
  const groups = new Map<string, CompanionBuild[]>()
  for (const b of catalog.value?.builds ?? []) {
    groups.set(b.minecraft, [...(groups.get(b.minecraft) ?? []), b])
  }
  const order: Loader[] = ['fabric', 'forge', 'neoforge']
  return [...groups.entries()]
    .sort((a, b) => compareVersions(b[0], a[0]))
    .map(([minecraft, builds]) => ({
      minecraft,
      label: builds.find((b) => b.loader === 'fabric' && b.supports.length > 1)
        ? `${builds.find((b) => b.loader === 'fabric')!.supports[0]} – ${minecraft}`
        : minecraft,
      builds: builds.sort((a, b) => order.indexOf(a.loader) - order.indexOf(b.loader)),
    }))
})

const installed = computed(() => companionVersionOf(snapshot.value?.agent_version))
const updateAvailable = computed(
  () => !!installed.value && !!catalog.value?.version && compareVersions(installed.value, catalog.value.version) < 0,
)

function openLink() {
  const value = code.value.trim().toUpperCase()
  if (value) router.push(`/link/${value}`)
}
</script>

<template>
  <div class="stack">
    <section class="card">
      <div class="head">
        <div>
          <h2>Conexão Companion</h2>
          <p class="muted">
            Um mod que fica só no servidor. Com ele a whitelist do painel vale na hora (sem RCON), e o Conexão sabe quais
            mods o servidor tem para montar o modpack dos jogadores com um clique.
          </p>
        </div>
        <span v-if="snapshot?.connected && linked" class="badge good">Conectado</span>
        <span v-else-if="linked" class="badge warn">Desconectado</span>
        <span v-else class="badge">Não instalado</span>
      </div>

      <template v-if="linked && snapshot">
        <div class="facts">
          <div>
            <span class="label">Servidor</span>
            <b>{{ snapshot.info?.loader ? LOADER_NAMES[snapshot.info.loader] : '—' }} {{ snapshot.info?.loader_version }}</b>
            <span class="muted">Minecraft {{ snapshot.info?.minecraft }}</span>
          </div>
          <div>
            <span class="label">Mods no servidor</span>
            <b>{{ snapshot.mods.length }}</b>
            <span class="muted">{{ unknownMods.length }} fora do Modrinth · {{ serverOnly.length }} só do servidor</span>
          </div>
          <div>
            <span class="label">Última conexão</span>
            <b>{{ formatDate(snapshot.last_seen_at) }}</b>
            <span class="muted">{{ snapshot.agent_version }}</span>
          </div>
        </div>
        <div v-if="updateAvailable" class="update">
          <div>
            <strong>Companion {{ catalog?.version }} disponível</strong>
            <span class="muted">O servidor está com a {{ installed }}. Troque o JAR na pasta mods/ e reinicie.</span>
          </div>
          <a v-if="build" :href="build.url" class="btn btn-sm" download>Baixar {{ LOADER_NAMES[build.loader] }} {{ target.minecraft }}</a>
        </div>
        <div v-if="snapshot.out_of_sync" class="sync">
          <div>
            <strong>Os mods do servidor mudaram desde a última versão publicada.</strong>
            <span class="muted">
              <template v-if="snapshot.only_in_pack.length">{{ snapshot.only_in_pack.length }} saíram do servidor. </template>
              Publique uma nova versão para os jogadores receberem.
            </span>
          </div>
          <button class="btn btn-primary" @click="router.replace({ query: { tab: 'modpack' } })">Atualizar modpack</button>
        </div>
        <p v-else-if="snapshot.mods.length" class="ok small">O modpack dos jogadores está igual aos mods do servidor.</p>

        <details v-if="snapshot.mods.length" class="mods">
          <summary>Ver mods do servidor</summary>
          <div v-for="m in snapshot.mods" :key="m.path" class="mod">
            <img v-if="m.icon_url" :src="m.icon_url" alt="" />
            <span v-else class="noicon" />
            <span class="name">{{ m.title ?? m.path.replace('mods/', '') }}</span>
            <span v-if="m.source === 'unknown'" class="badge">fora do Modrinth · {{ formatBytes(m.size) }}</span>
            <span v-if="m.server_only" class="badge">só servidor</span>
            <span v-if="!m.in_pack && !m.server_only" class="badge warn">não publicado</span>
          </div>
        </details>
      </template>

      <template v-else>
        <ol class="steps">
          <li>
            <b>Baixe o mod</b> e coloque na pasta <code>mods/</code> do servidor.
            <div v-if="build" class="dl">
              <a :href="build.url" class="btn btn-primary" download>
                Baixar para {{ LOADER_NAMES[build.loader] }} {{ target.minecraft }}
              </a>
              <span class="muted small">{{ build.file }} · {{ formatBytes(build.size) }} · Java {{ build.java }}+</span>
            </div>
            <p v-else-if="catalog && catalog.builds.length" class="missing small">
              Ainda não há Companion para {{ LOADER_NAMES[target.loader] }} {{ target.minecraft }}. Escolha a versão do
              seu servidor na lista abaixo.
            </p>
            <p v-else-if="catalog" class="missing small">O Companion ainda não foi publicado neste Conexão.</p>
          </li>
          <li><b>Reinicie o servidor.</b> O console mostra um link e um código, por exemplo <code>K7Q2-9XMA</code>.</li>
          <li>
            <b>Abra o link</b> ou digite o código aqui:
            <span class="code-row">
              <input v-model="code" class="input" placeholder="K7Q2-9XMA" maxlength="9" @keydown.enter="openLink" />
              <button class="btn btn-sm" :disabled="!code.trim()" @click="openLink">Vincular</button>
            </span>
          </li>
        </ol>
        <p class="muted small">
          Os jogadores não precisam do mod. Funciona em Fabric, Forge e NeoForge, do Minecraft 1.18.2 ao mais novo.
        </p>
      </template>
      <details v-if="byVersion.length" class="versions" :open="!linked && !build">
        <summary>Outra versão ou loader</summary>
        <p class="muted small">Use o JAR da versão exata do Minecraft e do loader do servidor.</p>
        <div v-for="g in byVersion" :key="g.minecraft" class="vrow">
          <span class="mc">{{ g.label }}</span>
          <span class="chips">
            <a
              v-for="b in g.builds"
              :key="b.file"
              :href="b.url"
              class="chip"
              :class="{ current: b === build }"
              :title="`${b.file} · Java ${b.java}+`"
              download
            >{{ LOADER_NAMES[b.loader] }}</a>
          </span>
        </div>
      </details>
      <p v-if="error" class="error">{{ error }}</p>
    </section>

    <details class="card alt">
      <summary>Alternativa: agente separado (para VPS, sem instalar mod)</summary>
      <AgentTab :server="server" :role="role" />
    </details>
  </div>
</template>

<style scoped>
.stack {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
}

h2 {
  margin: 0 0 6px;
}

.head p {
  margin: 0 0 14px;
  max-width: 640px;
  line-height: 1.5;
}

.facts {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
  margin-bottom: 14px;
}

.facts div {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 12px;
  border-radius: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  font-size: 13px;
}

.label {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-faint);
}

.sync {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  padding: 14px 16px;
  border-radius: 14px;
  background: var(--lime-soft);
  border: 1px solid var(--brand);
  margin-bottom: 12px;
}

.sync div {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.small {
  font-size: 13px;
}

.mods summary,
.alt summary {
  cursor: pointer;
  font-weight: 600;
  color: var(--text-dim);
}

.mods {
  margin-top: 8px;
}

.mod {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 0;
  border-bottom: 1px solid var(--line);
  font-size: 13px;
}

.mod img,
.noicon {
  width: 22px;
  height: 22px;
  border-radius: 6px;
  background: var(--bg-soft);
}

.name {
  flex: 1;
}

.steps {
  margin: 0 0 12px;
  padding-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  line-height: 1.5;
}

.dl {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 8px;
}

.dl .btn {
  text-decoration: none;
}

.missing {
  margin: 6px 0 0;
  color: var(--warn, #d9a400);
}

.update {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  border-radius: 14px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  margin-bottom: 12px;
}

.update div {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.update .btn {
  text-decoration: none;
  white-space: nowrap;
}

.versions {
  margin-top: 14px;
  border-top: 1px solid var(--line);
  padding-top: 12px;
}

.versions summary {
  cursor: pointer;
  font-weight: 600;
  color: var(--text-dim);
}

.vrow {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 0;
  border-bottom: 1px solid var(--line);
  font-size: 13px;
}

.vrow .mc {
  width: 130px;
  flex-shrink: 0;
  font-weight: 600;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chip {
  padding: 3px 10px;
  border-radius: 999px;
  border: 1px solid var(--line);
  color: var(--text-dim);
  text-decoration: none;
  font-size: 12px;
}

.chip:hover {
  border-color: var(--brand);
  color: var(--text);
}

.chip.current {
  background: var(--brand);
  border-color: var(--brand);
  color: #000;
}

.code-row {
  display: inline-flex;
  gap: 8px;
  margin-left: 8px;
  vertical-align: middle;
}

.code-row .input {
  width: 130px;
  text-transform: uppercase;
}

.alt :deep(.card) {
  box-shadow: none;
  border: none;
  padding: 0;
  margin-top: 14px;
}
</style>

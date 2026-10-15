<script setup lang="ts">
/** Aba "Servidor": Conexão Companion (recomendado) e o agente separado (alternativa). */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import AgentTab from './AgentTab.vue'
import { COMPANION_DOWNLOAD, api, formatBytes, formatDate, LOADER_NAMES, message, type Server, type SnapshotView } from '../lib/api'

const props = defineProps<{ server: Server; role: string }>()
const router = useRouter()
const snapshot = ref<SnapshotView | null>(null)
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
onMounted(() => {
  load()
  timer = window.setInterval(load, 10_000)
})
onBeforeUnmount(() => window.clearInterval(timer))

const linked = computed(() => !!snapshot.value?.agent_version?.startsWith('companion'))
const unknownMods = computed(() => snapshot.value?.mods.filter((m) => m.source === 'unknown') ?? [])
const serverOnly = computed(() => snapshot.value?.mods.filter((m) => m.server_only) ?? [])

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
            <b>Baixe o mod</b> e coloque na pasta <code>mods/</code> do servidor (NeoForge 1.21.1).
            <a :href="COMPANION_DOWNLOAD" class="btn btn-sm btn-primary dl">Baixar conexao-companion.jar</a>
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
        <p class="muted small">Os jogadores não precisam do mod. Forge, Fabric e outras versões chegam em breve.</p>
      </template>
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
  margin-left: 10px;
  text-decoration: none;
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

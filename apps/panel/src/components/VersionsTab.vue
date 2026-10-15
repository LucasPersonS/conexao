<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { API_URL, api, formatDate, message, type Server, type VersionItem } from '../lib/api'
import PublishPanel from './PublishPanel.vue'

const props = defineProps<{ server: Server }>()
const emit = defineEmits<{ changed: [] }>()

const versions = ref<VersionItem[]>([])
const error = ref<string | null>(null)
const busy = ref<string | null>(null)

async function load() {
  try {
    versions.value = await api.versions(props.server.slug)
  } catch (e) {
    error.value = message(e)
  }
}
onMounted(load)

async function point(version: string, channel: 'stable' | 'beta') {
  const label = channel === 'stable' ? 'estável' : 'beta'
  if (!window.confirm(`Apontar o canal ${label} para a versão ${version}? Os jogadores recebem essa versão ao clicar em Jogar.`)) return
  busy.value = `${version}:${channel}`
  error.value = null
  try {
    await api.setChannel(props.server.slug, channel, version)
    await load()
    emit('changed')
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = null
  }
}

async function onPublished() {
  await load()
  emit('changed')
}

const nextVersion = computed(() => {
  const latest = versions.value[0]?.version
  const match = latest?.match(/^(\d+)\.(\d+)\.(\d+)$/)
  return match ? `${match[1]}.${Number(match[2]) + 1}.0` : '1.0.0'
})
const command = computed(
  () =>
    `$env:CONEXAO_API_URL = "${API_URL}"\n$env:CONEXAO_TOKEN = "<token de publicação copiado no launcher>"\nconexao-publish --server ${props.server.slug} --dir "C:\\caminho\\do\\modpack" --version ${nextVersion.value}`,
)
</script>

<template>
  <div class="stack">
    <section class="card">
      <h2>Versões publicadas</h2>
      <p v-if="error" class="error">{{ error }}</p>
      <p v-if="!versions.length" class="muted">Nenhuma versão publicada ainda. Use o publicador abaixo.</p>
      <table v-else>
        <thead>
          <tr>
            <th>Versão</th>
            <th>Canal</th>
            <th>Arquivos</th>
            <th>Publicada em</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <tr v-for="v in versions" :key="v.id">
            <td><b>{{ v.version }}</b></td>
            <td>
              <span v-if="v.channels.includes('stable')" class="badge good">Estável</span>
              <span v-if="v.channels.includes('beta')" class="badge warn">Beta</span>
              <span v-if="!v.channels.length" class="muted">—</span>
            </td>
            <td>{{ v.file_count }}</td>
            <td class="muted">{{ formatDate(v.published_at) }}</td>
            <td class="actions">
              <button
                v-if="!v.channels.includes('stable')"
                class="btn btn-sm"
                :disabled="busy !== null"
                @click="point(v.version, 'stable')"
              >
                Tornar estável
              </button>
              <button
                v-if="!v.channels.includes('beta')"
                class="btn btn-sm"
                :disabled="busy !== null"
                @click="point(v.version, 'beta')"
              >
                Beta
              </button>
            </td>
          </tr>
        </tbody>
      </table>
      <p class="muted small">Rollback: clique em <b>Tornar estável</b> numa versão anterior. Nada é apagado.</p>
    </section>

    <PublishPanel :server="server" :suggested-version="nextVersion" @published="onPublished" />

    <details class="card cli">
      <summary>Alternativa: publicar pela linha de comando (<code>conexao-publish</code>)</summary>
      <p class="muted">
        Útil para modpacks muito grandes ou para automatizar. Rode na pasta do modpack; ele mostra o que mudou e envia
        só os arquivos novos. Use <code>--managed config/arquivo.toml</code> para forçar uma config.
      </p>
      <pre>{{ command }}</pre>
    </details>
  </div>
</template>

<style scoped>
.stack {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
}

.badge + .badge {
  margin-left: 4px;
}

.cli summary {
  cursor: pointer;
  font-weight: 600;
  color: var(--text-dim);
}

.cli[open] summary {
  margin-bottom: 12px;
}

.small {
  font-size: 13px;
  margin: 12px 0 0;
}

.muted {
  line-height: 1.5;
}
</style>

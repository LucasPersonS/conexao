<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { API_URL, api, formatDate, message, type AgentStatus, type Server } from '../lib/api'

const props = defineProps<{ server: Server; role: string }>()

const status = ref<AgentStatus | null>(null)
const token = ref<string | null>(null)
const error = ref<string | null>(null)
const busy = ref(false)

async function load() {
  try {
    status.value = await api.agent(props.server.slug)
  } catch (e) {
    error.value = message(e)
  }
}
onMounted(load)

async function generate() {
  if (status.value?.configured && !window.confirm('Gerar um novo token desconecta o agente atual até você atualizar o conexao-agent.toml. Continuar?')) return
  busy.value = true
  error.value = null
  try {
    token.value = (await api.agentToken(props.server.slug)).token
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

const wsUrl = computed(() => API_URL.replace(/^http/, 'ws') + '/v1/agent/connect')
const config = computed(
  () => `api_url = "${wsUrl.value}"
token = "${token.value ?? 'cagt_…'}"
server_dir = "/caminho/do/servidor"

[rcon]
address = "127.0.0.1:25575"
password = "a mesma senha do server.properties"`,
)
</script>

<template>
  <div class="stack">
    <section class="card">
      <h2>Status</h2>
      <div v-if="status" class="status">
        <span class="badge" :class="status.connected ? 'good' : status.configured ? 'warn' : ''">
          {{ status.connected ? 'Conectado' : status.configured ? 'Desconectado' : 'Não configurado' }}
        </span>
        <span v-if="status.agent_version" class="muted">versão {{ status.agent_version }}</span>
        <span class="muted">último contato: {{ formatDate(status.last_seen_at) }}</span>
      </div>
      <p class="muted">
        O agente roda na mesma máquina do servidor Minecraft. Quando alguém é aprovado ou banido, ele regrava o
        <code>whitelist.json</code> e executa <code>whitelist reload</code> pelo RCON — sem plugin, em NeoForge, Forge,
        Fabric ou Paper.
      </p>
      <p v-if="error" class="error">{{ error }}</p>
      <button v-if="role === 'owner'" class="btn btn-primary" :disabled="busy" @click="generate">
        {{ status?.configured ? 'Gerar novo token' : 'Gerar token do agente' }}
      </button>
      <p v-else class="muted small">Só o dono do servidor pode gerar o token do agente.</p>
    </section>

    <section class="card">
      <h2>Configuração (conexao-agent.toml)</h2>
      <p v-if="token" class="warn-box">Copie agora: o token não será mostrado de novo.</p>
      <pre>{{ config }}</pre>
      <p class="muted small">
        No <code>server.properties</code>: <code>white-list=true</code>, <code>enforce-whitelist=true</code>,
        <code>enable-rcon=true</code>. Depois rode <code>conexao-agent --config conexao-agent.toml</code>.
      </p>
    </section>
  </div>
</template>

<style scoped>
.stack {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.status {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-bottom: 10px;
  font-size: 14px;
}

.muted {
  line-height: 1.5;
}

.small {
  font-size: 13px;
}

.warn-box {
  padding: 10px 14px;
  border-radius: 10px;
  background: var(--warn-bg);
  border: 1px solid var(--warn-line);
  color: var(--warn);
  font-weight: 600;
  margin: 0 0 10px;
}
</style>

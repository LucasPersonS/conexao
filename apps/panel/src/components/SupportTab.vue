<script setup lang="ts">
/** Apoios dos jogadores: conta Mercado Pago que recebe (só o dono mexe) e apoios recebidos. */
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api, formatDate, goToMercadoPago, message, money, type Server, type SupportAdmin } from '../lib/api'

const props = defineProps<{ server: Server }>()
const emit = defineEmits<{ changed: [] }>()
const route = useRoute()
const router = useRouter()

const data = ref<SupportAdmin | null>(null)
const busy = ref(false)
const error = ref<string | null>(null)
const confirmDisconnect = ref(false)
const justConnected = computed(() => route.query.mp === 'ok')

async function load() {
  error.value = null
  try {
    data.value = await api.supportAdmin(props.server.slug)
  } catch (e) {
    error.value = message(e)
  }
}
onMounted(load)

async function connect() {
  busy.value = true
  error.value = null
  try {
    const { url } = await api.payoutConnect(props.server.slug)
    goToMercadoPago(url)
  } catch (e) {
    error.value = message(e)
    busy.value = false
  }
}

async function toggle() {
  if (!data.value?.account) return
  busy.value = true
  try {
    await api.payoutSettings(props.server.slug, !data.value.account.support_enabled)
    await load()
    emit('changed')
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

async function disconnect() {
  if (!confirmDisconnect.value) {
    confirmDisconnect.value = true
    return
  }
  busy.value = true
  try {
    await api.payoutDisconnect(props.server.slug)
    confirmDisconnect.value = false
    await load()
    emit('changed')
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

function dismiss() {
  const { mp: _, ...rest } = route.query
  router.replace({ query: rest })
}
</script>

<template>
  <section class="card support">
    <div>
      <h2>Apoio dos jogadores</h2>
      <p class="muted">
        Jogadores apoiam o servidor pelo launcher com Pix ou cartão. O dinheiro cai direto na conta Mercado Pago do dono;
        o Conexão fica com {{ data?.fee_percent ?? 15 }}% de cada apoio para manter a plataforma, e a taxa do Mercado
        Pago também sai da parte do servidor. O Conexão nunca guarda o seu dinheiro.
      </p>
    </div>

    <div v-if="justConnected" class="result approved">
      <strong>Conta conectada!</strong> O botão "Apoiar" já aparece na página do servidor no launcher.
      <button class="link" @click="dismiss">fechar</button>
    </div>

    <template v-if="data">
      <p v-if="!data.available" class="muted">O apoio a servidores ainda não foi ativado no Conexão.</p>
      <p v-else-if="server.approval !== 'approved'" class="muted">Disponível depois que a equipe Conexão aprovar o servidor.</p>

      <div v-else-if="!data.account" class="connect">
        <p>Nenhuma conta conectada.</p>
        <button v-if="data.can_manage" class="btn btn-primary" :disabled="busy" @click="connect">
          {{ busy ? 'Abrindo Mercado Pago…' : 'Conectar conta Mercado Pago' }}
        </button>
        <p v-else class="muted">Só o dono do servidor pode conectar a conta que recebe os apoios.</p>
      </div>

      <div v-else class="account">
        <div>
          <span class="badge" :class="data.account.support_enabled ? 'good' : 'warn'">
            {{ data.account.support_enabled ? 'Recebendo apoios' : 'Apoios pausados' }}
          </span>
          <span v-if="!data.account.live_mode" class="badge warn">Conta de teste</span>
          <p class="muted small">Conta {{ data.account.mp_user }} · conectada em {{ formatDate(data.account.connected_at) }}</p>
        </div>
        <div v-if="data.can_manage" class="actions">
          <button class="btn btn-sm" :disabled="busy" @click="toggle">
            {{ data.account.support_enabled ? 'Pausar apoios' : 'Voltar a receber' }}
          </button>
          <button class="btn btn-sm" :disabled="busy" @click="connect">Trocar conta</button>
          <button class="btn btn-sm btn-danger" :disabled="busy" @click="disconnect">
            {{ confirmDisconnect ? 'Confirmar desconexão' : 'Desconectar' }}
          </button>
        </div>
      </div>

      <div v-if="data.account || data.recent.length" class="totals">
        <div><small>Apoios (30 dias)</small><strong>{{ data.last_30_days.count }}</strong></div>
        <div><small>Total pago</small><strong>{{ money(data.last_30_days.gross_cents) }}</strong></div>
        <div><small>Taxa Conexão</small><strong>{{ money(data.last_30_days.fee_cents) }}</strong></div>
        <div class="net"><small>Para o servidor*</small><strong>{{ money(data.last_30_days.net_cents) }}</strong></div>
      </div>
      <p v-if="data.account" class="muted small">* antes da taxa do Mercado Pago. O valor exato aparece no extrato do Mercado Pago.</p>

      <div v-if="data.recent.length" class="table-wrap">
      <table>
        <thead>
          <tr><th>Quando</th><th>Quem</th><th>Valor</th><th>Para o servidor</th><th>Mensagem</th></tr>
        </thead>
        <tbody>
          <tr v-for="r in data.recent" :key="r.id">
            <td>{{ formatDate(r.paid_at) }}</td>
            <td>{{ r.supporter ?? 'Anônimo' }}</td>
            <td>{{ money(r.amount_cents) }} <span v-if="r.status === 'refunded'" class="badge bad">Estornado</span></td>
            <td>{{ money(r.amount_cents - r.fee_cents) }}</td>
            <td class="msg">{{ r.message ?? '—' }}</td>
          </tr>
        </tbody>
      </table>
      </div>
    </template>
    <p v-if="error" class="error">{{ error }}</p>
  </section>
</template>

<style scoped>
.support {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

h2 {
  margin: 0 0 4px;
}

p {
  margin: 0;
  line-height: 1.5;
}

.small {
  font-size: 12px;
  margin-top: 4px;
}

.connect,
.account {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px;
  padding: 14px 16px;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: var(--bg-soft);
}

.actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.totals {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 10px;
}

.totals div {
  display: flex;
  flex-direction: column;
  padding: 12px 14px;
  border-radius: 12px;
  border: 1px solid var(--line);
}

.totals small {
  color: var(--text-dim);
}

.totals strong {
  font-size: 20px;
}

.totals .net {
  border-color: var(--brand);
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
  padding: 8px 6px;
  border-bottom: 1px solid var(--line);
}

th {
  color: var(--text-dim);
  font-weight: 600;
}

.msg {
  max-width: 280px;
  overflow-wrap: anywhere;
  color: var(--text-dim);
}

.result {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px 10px;
  padding: 12px 14px;
  border-radius: 12px;
  background: var(--good-bg);
  border: 1px solid var(--good-line);
}

.result .link {
  margin-left: auto;
}
</style>

<script setup lang="ts">
/** Plano PRO do servidor: compra (dinheiro ou Moedas) e estatísticas. */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api, formatDate, goToMercadoPago, message, money, type PlanOption, type Server, type ServerStats, type ShopOrder } from '../lib/api'

const props = defineProps<{ server: Server }>()
const emit = defineEmits<{ changed: [] }>()
const route = useRoute()
const router = useRouter()

const plans = ref<PlanOption[]>([])
const moneyEnabled = ref(false)
const balance = ref(0)
const stats = ref<ServerStats | null>(null)
const busy = ref<string | null>(null)
const confirming = ref<string | null>(null)
const error = ref<string | null>(null)
const checking = ref<ShopOrder | null>(null)
let timer: number | undefined

const isPro = computed(() => !!props.server.pro_until)

async function load() {
  error.value = null
  try {
    const [catalog, wallet] = await Promise.all([api.shopCatalog(), api.wallet()])
    plans.value = catalog.pro
    moneyEnabled.value = catalog.money_enabled
    balance.value = wallet.balance
  } catch (e) {
    error.value = message(e)
  }
  if (isPro.value) {
    try {
      stats.value = await api.stats(props.server.slug)
    } catch {
      stats.value = null
    }
  }
}

/** Volta do Mercado Pago com ?pedido=<id>: confere até confirmar. */
async function poll(id: string, tries = 0) {
  try {
    checking.value = await api.order(id)
    if (checking.value.status === 'pending' && tries < 40) {
      timer = window.setTimeout(() => poll(id, tries + 1), 3000)
      return
    }
    if (checking.value.status === 'approved') emit('changed')
  } catch (e) {
    error.value = message(e)
  }
}

onMounted(load)
watch(() => props.server.pro_until, load)
watch(
  () => route.query.pedido,
  (id) => {
    window.clearTimeout(timer)
    if (typeof id === 'string' && /^[0-9a-f-]{36}$/i.test(id)) poll(id)
  },
  { immediate: true },
)
onBeforeUnmount(() => window.clearTimeout(timer))

async function buyMoney(plan: PlanOption) {
  busy.value = plan.sku
  error.value = null
  try {
    const { checkout_url } = await api.proCheckout(props.server.slug, plan.sku)
    goToMercadoPago(checkout_url)
  } catch (e) {
    error.value = message(e)
    busy.value = null
  }
}

async function buyCoins(plan: PlanOption) {
  if (confirming.value !== plan.sku) {
    confirming.value = plan.sku
    return
  }
  confirming.value = null
  busy.value = plan.sku + ':coins'
  error.value = null
  try {
    const result = await api.spendCoins(props.server.slug, plan.sku, crypto.randomUUID())
    balance.value = result.balance
    emit('changed')
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = null
  }
}

function dismiss() {
  checking.value = null
  const { pedido: _, ...rest } = route.query
  router.replace({ query: rest })
}

/** Barras dos últimos 30 dias. */
const SERIES = [
  { key: 'whitelist', label: 'Pedidos de whitelist' },
  { key: 'stars', label: 'Estrelas' },
  { key: 'comments', label: 'Comentários' },
  { key: 'supports', label: 'Apoios' },
] as const
const totals = computed(() =>
  Object.fromEntries(SERIES.map((s) => [s.key, stats.value?.days.reduce((sum, d) => sum + d[s.key], 0) ?? 0])),
)
const maxOf = (key: (typeof SERIES)[number]['key']) => Math.max(1, ...(stats.value?.days.map((d) => d[key]) ?? [1]))
</script>

<template>
  <section class="card pro">
    <div class="head">
      <div>
        <h2>Conexão PRO <span v-if="isPro" class="badge good">Ativo até {{ formatDate(server.pro_until ?? null) }}</span></h2>
        <p class="muted">Ferramentas para crescer o servidor. Pague com Pix, cartão, boleto ou Moedas Conexão.</p>
      </div>
    </div>

    <ul class="perks">
      <li><b>Mais espaço</b> para configs e arquivos do modpack hospedados no Conexão.</li>
      <li><b>Selo PRO</b> no card e na página do servidor no launcher.</li>
      <li><b>Página personalizada</b>: fundo em gradiente, imagens, GIFs, galerias e vídeos do YouTube na página do servidor.</li>
      <li><b>Estatísticas</b> de 30 dias: pedidos de whitelist, estrelas, comentários e apoios.</li>
    </ul>

    <div v-if="checking" class="result" :class="checking.status">
      <template v-if="checking.status === 'approved'"><strong>Pagamento confirmado!</strong> PRO ativo até {{ formatDate(checking.granted_until) }}.</template>
      <template v-else-if="checking.status === 'pending'"><strong>Aguardando a confirmação do pagamento…</strong> Pode fechar esta página: o PRO liga sozinho.</template>
      <template v-else><strong>O pagamento não foi concluído.</strong> Nenhum valor foi cobrado.</template>
      <button class="link" @click="dismiss">fechar</button>
    </div>

    <p v-if="server.approval !== 'approved'" class="muted">O PRO pode ser comprado depois que a equipe Conexão aprovar o servidor.</p>
    <div v-else class="plans">
      <div v-for="p in plans" :key="p.sku" class="plan">
        <strong>{{ p.days }} dias</strong>
        <span class="price">{{ money(p.price_cents) }}</span>
        <button class="btn btn-primary btn-sm" :disabled="!moneyEnabled || busy !== null" @click="buyMoney(p)">
          {{ busy === p.sku ? 'Abrindo pagamento…' : isPro ? 'Estender com Pix/cartão' : 'Assinar com Pix/cartão' }}
        </button>
        <button
          class="btn btn-sm"
          :class="{ confirm: confirming === p.sku }"
          :disabled="balance < p.coins || busy !== null"
          :title="balance < p.coins ? `Você tem ${balance} moedas` : ''"
          @click="buyCoins(p)"
        >
          {{ confirming === p.sku ? `Confirmar ${p.coins.toLocaleString('pt-BR')} moedas` : `${p.coins.toLocaleString('pt-BR')} moedas` }}
        </button>
      </div>
    </div>
    <p class="muted small">Suas Moedas: {{ balance.toLocaleString('pt-BR') }}. Comprar de novo soma os dias ao plano atual.</p>
    <p v-if="error" class="error">{{ error }}</p>
  </section>

  <section v-if="isPro && stats" class="card stats">
    <h2>Últimos 30 dias</h2>
    <p class="muted">
      Whitelist: {{ stats.whitelist_approved }} aprovados de {{ stats.whitelist_total }} pedidos no total.
    </p>
    <div class="charts">
      <figure v-for="s in SERIES" :key="s.key" class="chart">
        <figcaption>{{ s.label }} <b>{{ totals[s.key] }}</b></figcaption>
        <div class="bars" role="img" :aria-label="`${s.label}: ${totals[s.key]} nos últimos 30 dias`">
          <span
            v-for="d in stats.days"
            :key="d.day"
            :style="{ height: `${Math.max(2, (d[s.key] / maxOf(s.key)) * 100)}%` }"
            :class="{ zero: d[s.key] === 0 }"
            :title="`${new Date(d.day + 'T12:00:00').toLocaleDateString('pt-BR')}: ${d[s.key]}`"
          />
        </div>
      </figure>
    </div>
  </section>
</template>

<style scoped>
.pro {
  display: flex;
  flex-direction: column;
  gap: 14px;
  margin-bottom: 16px;
}

h2 {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0 0 4px;
}

.head p,
.small {
  margin: 0;
}

.small {
  font-size: 12px;
}

.perks {
  margin: 0;
  padding-left: 18px;
  display: grid;
  gap: 4px;
  color: var(--text-dim);
}

.plans {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 12px;
}

.plan {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 16px;
  border-radius: 14px;
  border: 1px solid var(--line);
  background: var(--bg-soft);
}

.price {
  font-size: 22px;
  font-weight: 800;
}

.confirm {
  border-color: var(--lime);
  box-shadow: var(--lime-glow);
}

.result {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px 10px;
  padding: 12px 14px;
  border-radius: 12px;
  font-size: 14px;
  background: var(--warn-bg);
  border: 1px solid var(--warn-line);
}

.result.approved {
  background: var(--good-bg);
  border-color: var(--good-line);
}

.result.rejected,
.result.cancelled,
.result.refunded {
  background: var(--bad-bg);
  border-color: var(--bad-line);
}

.result .link {
  margin-left: auto;
}

.stats {
  margin-bottom: 16px;
}

.charts {
  margin-top: 12px;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 16px;
}

.chart {
  margin: 0;
}

figcaption {
  display: flex;
  justify-content: space-between;
  margin-bottom: 6px;
  color: var(--text-dim);
  font-size: 13px;
}

figcaption b {
  color: var(--text);
}

.bars {
  height: 72px;
  display: flex;
  align-items: flex-end;
  gap: 2px;
  padding: 6px;
  border-radius: 10px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
}

.bars span {
  flex: 1;
  border-radius: 2px 2px 0 0;
  background: var(--lime);
}

.bars span.zero {
  background: var(--line);
}
</style>

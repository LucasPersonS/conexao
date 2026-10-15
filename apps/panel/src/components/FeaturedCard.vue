<script setup lang="ts">
/** Comprar destaque na home do launcher (Mercado Pago: Pix, cartão ou boleto). */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api, formatDate, message, type FeaturedPurchase, type Server } from '../lib/api'

const props = defineProps<{ server: Server }>()
const emit = defineEmits<{ changed: [] }>()
const route = useRoute()
const router = useRouter()

const plans = ref<{ days: number; price_cents: number }[]>([])
const enabled = ref(false)
const purchases = ref<FeaturedPurchase[]>([])
const busy = ref<number | null>(null)
const error = ref<string | null>(null)
const checking = ref<FeaturedPurchase | null>(null)
let timer: number | undefined

/** Nome e e-mail de quem paga (opcionais): preenchem o checkout e ajudam a aprovar cartão. */
const PAYER_KEY = 'conexao.payer'
function readPayer(): { name: string; email: string } {
  try {
    const saved = JSON.parse(localStorage.getItem(PAYER_KEY) ?? '{}')
    return { name: String(saved.name ?? ''), email: String(saved.email ?? '') }
  } catch {
    return { name: '', email: '' }
  }
}
const payer = ref(readPayer())

const money = (cents: number) => (cents / 100).toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' })
const perDay = (p: { days: number; price_cents: number }) => money(Math.round(p.price_cents / p.days))
const best = computed(() => [...plans.value].sort((a, b) => a.price_cents / a.days - b.price_cents / b.days)[0]?.days)

async function load() {
  try {
    const result = await api.featuredPlans()
    plans.value = result.plans
    enabled.value = result.enabled
    purchases.value = await api.featuredPurchases(props.server.slug)
  } catch (e) {
    error.value = message(e)
  }
}

/** Volta do Mercado Pago com ?compra=<id>: confere até o pagamento ser confirmado. */
async function poll(id: string, tries = 0) {
  try {
    checking.value = await api.featuredPurchase(props.server.slug, id)
    if (checking.value.status === 'pending' && tries < 40) {
      timer = window.setTimeout(() => poll(id, tries + 1), 3000)
      return
    }
    if (checking.value.status === 'approved') emit('changed')
    await load()
  } catch (e) {
    error.value = message(e)
  }
}

onMounted(load)
watch(
  () => route.query.compra,
  (id) => {
    window.clearTimeout(timer)
    if (typeof id === 'string') poll(id)
  },
  { immediate: true },
)
onBeforeUnmount(() => window.clearTimeout(timer))

async function buy(days: number) {
  busy.value = days
  error.value = null
  try {
    const filled = { name: payer.value.name.trim(), email: payer.value.email.trim() }
    try {
      localStorage.setItem(PAYER_KEY, JSON.stringify(filled))
    } catch {
      /* sem armazenamento: tudo bem */
    }
    const { checkout_url } = await api.featuredCheckout(
      props.server.slug,
      days,
      filled.name || filled.email ? filled : undefined,
    )
    window.location.href = checkout_url
  } catch (e) {
    error.value = message(e)
    busy.value = null
  }
}

function dismiss() {
  checking.value = null
  const { compra: _, ...rest } = route.query
  router.replace({ query: rest })
}

const STATUS: Record<FeaturedPurchase['status'], string> = {
  pending: 'Aguardando pagamento',
  approved: 'Pago',
  rejected: 'Recusado',
  cancelled: 'Cancelado',
  refunded: 'Estornado',
}
</script>

<template>
  <section class="card featured">
    <div class="head">
      <div>
        <h2>Destaque na home do launcher</h2>
        <p class="muted">
          Servidores em destaque aparecem na vitrine do topo, com banner grande, e primeiro na lista. Pague com Pix, cartão
          ou boleto; o destaque liga sozinho assim que o pagamento é confirmado.
        </p>
      </div>
      <span v-if="server.featured_until" class="badge brand">Ativo até {{ formatDate(server.featured_until) }}</span>
    </div>

    <div v-if="checking" class="result" :class="checking.status">
      <template v-if="checking.status === 'approved'">
        <strong>Pagamento confirmado!</strong> Destaque de {{ checking.days }} dias ativo até
        {{ formatDate(checking.featured_until) }}.
      </template>
      <template v-else-if="checking.status === 'pending'">
        <strong>Aguardando a confirmação do pagamento…</strong> Pix costuma confirmar em segundos; boleto pode levar até 3
        dias úteis. Pode fechar esta página: o destaque liga sozinho.
      </template>
      <template v-else><strong>O pagamento não foi concluído.</strong> Nenhum valor foi cobrado; tente de novo.</template>
      <button class="link" @click="dismiss">fechar</button>
    </div>

    <p v-if="server.approval !== 'approved'" class="muted">O destaque pode ser comprado depois que a equipe Conexão aprovar o servidor.</p>
    <p v-else-if="!enabled" class="muted">Os pagamentos ainda não foram ativados no Conexão.</p>
    <div v-else class="payer">
      <p class="payer-title">Dados para o recibo <span class="muted">(opcional, já vêm preenchidos no Mercado Pago)</span></p>
      <div class="grid-2">
        <label class="field">
          Nome de quem paga
          <input v-model="payer.name" class="input" maxlength="80" autocomplete="name" placeholder="Nome e sobrenome" />
        </label>
        <label class="field">
          E-mail para o recibo
          <input v-model="payer.email" class="input" type="email" maxlength="254" autocomplete="email" placeholder="voce@exemplo.com" />
        </label>
      </div>
    </div>
    <div v-if="enabled && server.approval === 'approved'" class="plans">
      <div v-for="p in plans" :key="p.days" class="plan" :class="{ best: p.days === best }">
        <span v-if="p.days === best" class="tag">Melhor preço</span>
        <strong>{{ p.days }} dias</strong>
        <span class="price">{{ money(p.price_cents) }}</span>
        <span class="muted small">{{ perDay(p) }} por dia</span>
        <button class="btn btn-primary btn-sm" :disabled="busy !== null" @click="buy(p.days)">
          {{ busy === p.days ? 'Abrindo pagamento…' : server.featured_until ? 'Estender' : 'Destacar' }}
        </button>
      </div>
    </div>
    <p v-if="server.featured_until && enabled" class="muted small">Comprar de novo soma os dias ao destaque atual.</p>
    <p v-if="error" class="error">{{ error }}</p>

    <details v-if="purchases.length" class="history">
      <summary>Histórico de compras ({{ purchases.length }})</summary>
      <table>
        <tbody>
          <tr v-for="p in purchases" :key="p.id">
            <td>{{ formatDate(p.created_at) }}</td>
            <td>{{ p.days }} dias</td>
            <td>{{ money(p.amount_cents) }}</td>
            <td><span class="badge" :class="{ good: p.status === 'approved', warn: p.status === 'pending', bad: ['rejected', 'refunded'].includes(p.status) }">{{ STATUS[p.status] }}</span></td>
            <td class="muted">{{ p.buyer_name }}</td>
          </tr>
        </tbody>
      </table>
    </details>
  </section>
</template>

<style scoped>
.featured {
  display: flex;
  flex-direction: column;
  gap: 14px;
  margin-bottom: 16px;
}

.head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
}

h2 {
  margin: 0 0 4px;
}

.head p {
  margin: 0;
  line-height: 1.5;
  max-width: 680px;
}

.payer {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.payer-title {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
}

.payer-title .muted {
  font-weight: 400;
  font-size: 13px;
}

.plans {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
}

.plan {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 16px;
  border-radius: 14px;
  border: 1px solid var(--line);
  background: var(--bg-soft);
}

.plan.best {
  border-color: var(--brand);
  background: var(--lime-soft);
}

.plan .btn {
  margin-top: 8px;
}

.price {
  font-size: 22px;
  font-weight: 800;
}

.tag {
  position: absolute;
  top: -10px;
  right: 12px;
  padding: 2px 10px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 800;
  background: var(--brand);
  color: var(--ink);
}

.small {
  font-size: 12px;
  margin: 0;
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
.result.cancelled {
  background: var(--bad-bg);
  border-color: var(--bad-line);
}

.result .link {
  margin-left: auto;
}

.history summary {
  cursor: pointer;
  font-weight: 600;
  color: var(--text-dim);
  margin-bottom: 8px;
}
</style>

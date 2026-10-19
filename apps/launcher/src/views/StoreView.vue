<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import Icon from '../components/Icon.vue'
import { errorMessage, shop, type CoinPack, type LedgerEntry, type PlanOption } from '../lib/api'
import { formatCoins, formatDate, formatMoney } from '../lib/format'
import { showToast } from '../lib/store'
import { checkNow, dismissWaiting, isPremium, loadWallet, newKey, payWithMoney, wallet } from '../lib/wallet'

const busy = ref<string | null>(null)
const error = ref<string | null>(null)
/** Plano esperando o segundo clique para gastar moedas. */
const confirming = ref<string | null>(null)

onMounted(loadWallet)

const balance = computed(() => wallet.data?.balance ?? 0)
const catalog = computed(() => wallet.catalog)

/** Bônus de um pacote em relação ao valor de face (1 moeda = R$ 0,01). */
function bonus(pack: CoinPack) {
  return Math.round((pack.coins / pack.price_cents - 1) * 100)
}

function perMonth(plan: PlanOption) {
  return formatMoney(Math.round((plan.price_cents / plan.days) * 30))
}

async function buyWithMoney(sku: string, label: string) {
  error.value = null
  busy.value = sku
  try {
    await payWithMoney(() => shop.checkout(sku), label)
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = null
  }
}

async function buyWithCoins(plan: PlanOption) {
  if (confirming.value !== plan.sku) {
    confirming.value = plan.sku
    return
  }
  confirming.value = null
  error.value = null
  busy.value = plan.sku + ':coins'
  try {
    await shop.spend(plan.sku, newKey())
    await loadWallet()
    showToast(`Conexão+ ativado por ${plan.days} dias!`)
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = null
  }
}

const KIND_LABEL: Record<LedgerEntry['kind'], string> = {
  purchase: 'Compra',
  spend: 'Gasto',
  refund: 'Estorno',
  chargeback: 'Contestação',
  grant: 'Presente da equipe',
  revoke: 'Ajuste da equipe',
}

const STATUS_TEXT = {
  pending: 'Aguardando a confirmação do Mercado Pago…',
  approved: 'Pagamento confirmado! Já está na sua conta.',
  rejected: 'O pagamento foi recusado. Tente outro meio de pagamento.',
  cancelled: 'O pagamento foi cancelado.',
  refunded: 'O pagamento foi estornado.',
} as const
</script>

<template>
  <div class="store grid-bg">
    <header class="hero">
      <div>
        <span class="eyebrow">Loja Conexão</span>
        <h1>Apoie o Conexão e destaque seu perfil</h1>
        <p class="sub">Pix, cartão ou boleto pelo Mercado Pago. Ou use suas Moedas.</p>
      </div>
      <div class="balance" aria-live="polite">
        <Icon name="coin" :size="30" />
        <div>
          <strong>{{ formatCoins(balance) }}</strong>
          <small>Moedas</small>
        </div>
        <span v-if="isPremium()" class="plus-on">Conexão+ até {{ formatDate(wallet.data?.premium_until) }}</span>
      </div>
    </header>

    <div v-if="wallet.waiting" class="waiting" :class="wallet.waiting.status">
      <Icon :name="wallet.waiting.status === 'approved' ? 'check' : wallet.waiting.status === 'pending' ? 'clock' : 'x'" :size="20" />
      <div>
        <strong>{{ wallet.waiting.label }}</strong>
        <span>{{ STATUS_TEXT[wallet.waiting.status] }}</span>
      </div>
      <button v-if="wallet.waiting.status === 'pending'" class="btn" @click="checkNow">Já paguei</button>
      <button class="btn" @click="dismissWaiting">Fechar</button>
    </div>

    <p v-if="error" class="err" role="alert">{{ error }}</p>
    <p v-if="wallet.error && !wallet.data" class="err">{{ wallet.error }}</p>

    <section v-if="catalog" class="card plus">
      <div class="plus-head">
        <div>
          <span class="tag">Conexão+</span>
          <h2>Seu perfil com tudo liberado</h2>
          <ul>
            <li><Icon name="check" :size="16" /> 7 molduras animadas (Neon, Chamas, Órbita…)</li>
            <li><Icon name="check" :size="16" /> 5 fontes exclusivas para o seu nome</li>
            <li><Icon name="check" :size="16" /> Efeitos de nome: Reflexo, Arco-íris, Glitch e Onda</li>
            <li><Icon name="check" :size="16" /> Selo Conexão+ no seu perfil</li>
          </ul>
        </div>
      </div>
      <div class="plans">
        <article v-for="plan in catalog.plus" :key="plan.sku" class="plan">
          <span class="days">{{ plan.days }} dias</span>
          <strong class="price">{{ formatMoney(plan.price_cents) }}</strong>
          <small v-if="plan.days > 30">{{ perMonth(plan) }} por mês</small>
          <small v-else>&nbsp;</small>
          <button
            class="btn btn-primary"
            :disabled="!catalog.money_enabled || busy !== null"
            @click="buyWithMoney(plan.sku, `Conexão+ por ${plan.days} dias`)"
          >
            {{ busy === plan.sku ? 'Abrindo…' : 'Pagar com Pix ou cartão' }}
          </button>
          <button
            class="btn coins-btn"
            :class="{ confirm: confirming === plan.sku }"
            :disabled="balance < plan.coins || busy !== null"
            :title="balance < plan.coins ? `Faltam ${formatCoins(plan.coins - balance)} moedas` : ''"
            @click="buyWithCoins(plan)"
          >
            <Icon name="coin" :size="16" />
            {{ confirming === plan.sku ? `Confirmar ${formatCoins(plan.coins)} moedas` : `${formatCoins(plan.coins)} moedas` }}
          </button>
        </article>
      </div>
    </section>

    <section v-if="catalog" class="card">
      <h2><Icon name="coin" :size="20" /> Moedas Conexão</h2>
      <p class="hint">
        Use em Conexão+, no PRO e no destaque do seu servidor. Moedas ficam na sua conta: não podem ser sacadas nem
        transferidas para outra pessoa.
      </p>
      <div class="packs">
        <button
          v-for="pack in catalog.coin_packs"
          :key="pack.sku"
          class="pack"
          :disabled="!catalog.money_enabled || busy !== null"
          @click="buyWithMoney(pack.sku, `${formatCoins(pack.coins)} Moedas`)"
        >
          <span v-if="bonus(pack) > 0" class="bonus">+{{ bonus(pack) }}%</span>
          <Icon name="coin" :size="26" />
          <strong>{{ formatCoins(pack.coins) }}</strong>
          <span>{{ busy === pack.sku ? 'Abrindo…' : formatMoney(pack.price_cents) }}</span>
        </button>
      </div>
      <p v-if="!catalog.money_enabled" class="hint">Pagamentos em dinheiro ainda não estão ativos.</p>
    </section>

    <section v-if="wallet.data?.history.length" class="card">
      <h2>Histórico de Moedas</h2>
      <ul class="history">
        <li v-for="h in wallet.data.history" :key="h.id">
          <div>
            <strong>{{ h.description }}</strong>
            <small>{{ KIND_LABEL[h.kind] }} · {{ formatDate(h.created_at) }}</small>
          </div>
          <span class="delta" :class="{ neg: h.delta < 0 }">{{ h.delta > 0 ? '+' : '' }}{{ formatCoins(h.delta) }}</span>
        </li>
      </ul>
    </section>

    <p class="legal">
      Pagamentos processados pelo Mercado Pago. O Conexão não guarda dados do seu cartão. Estornos e contestações
      desfazem o que foi entregue pela compra.
    </p>
  </div>
</template>

<style scoped>
.store {
  min-height: 100%;
  padding: 28px 32px 40px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.hero {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: 20px;
  flex-wrap: wrap;
}

h1 {
  margin: 6px 0 4px;
  font-family: var(--font-display);
  font-size: 30px;
}

.sub,
.hint,
.legal {
  margin: 0;
  color: var(--text-dim);
  font-size: 14px;
}

.legal {
  font-size: 12px;
  color: var(--text-faint);
}

.balance {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 18px;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--lime-line);
  color: var(--lime);
  flex-wrap: wrap;
}

.balance strong {
  display: block;
  font-family: var(--font-display);
  font-size: 26px;
  color: var(--text);
  line-height: 1;
}

.balance small {
  color: var(--text-dim);
}

.plus-on {
  padding: 4px 10px;
  border-radius: 999px;
  background: var(--lime);
  color: var(--ink);
  font-size: 12px;
  font-weight: 700;
}

.card {
  padding: 20px 22px;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
}

h2 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 8px;
  font-family: var(--font-display);
  font-size: 21px;
}

.plus {
  border-color: var(--lime-line);
  background:
    radial-gradient(600px 220px at 100% 0, rgba(183, 254, 0, 0.1), transparent 70%),
    var(--bg-card);
}

.tag {
  display: inline-block;
  margin-bottom: 6px;
  padding: 3px 10px;
  border-radius: 999px;
  background: var(--lime);
  color: var(--ink);
  font-size: 12px;
  font-weight: 700;
}

.plus ul {
  margin: 8px 0 16px;
  padding: 0;
  list-style: none;
  display: grid;
  gap: 6px;
  color: var(--text-dim);
}

.plus li {
  display: flex;
  align-items: center;
  gap: 8px;
}

.plus li :deep(svg) {
  color: var(--lime);
}

.plans {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
}

.plan {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
  border-radius: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
}

.days {
  color: var(--text-dim);
  font-weight: 600;
}

.price {
  font-family: var(--font-display);
  font-size: 26px;
}

.plan small {
  color: var(--text-faint);
  margin-top: -6px;
}

.coins-btn {
  color: var(--lime);
}

.coins-btn.confirm {
  border-color: var(--lime);
  box-shadow: var(--lime-glow);
}

.packs {
  margin-top: 14px;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 12px;
}

.pack {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 18px 12px;
  border-radius: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  color: var(--lime);
  transition: border-color 0.15s, transform 0.1s;
}

.pack:hover:not(:disabled) {
  border-color: var(--lime-line);
  transform: translateY(-1px);
}

.pack strong {
  font-family: var(--font-display);
  font-size: 22px;
  color: var(--text);
}

.pack span {
  color: var(--text-dim);
  font-weight: 600;
}

.bonus {
  position: absolute;
  top: 8px;
  right: 8px;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--lime);
  color: var(--ink) !important;
  font-size: 11px;
}

.history {
  margin: 0;
  padding: 0;
  list-style: none;
}

.history li {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 10px 0;
  border-bottom: 1px solid var(--line);
}

.history li:last-child {
  border-bottom: 0;
}

.history small {
  display: block;
  color: var(--text-faint);
}

.delta {
  font-weight: 700;
  color: var(--good);
}

.delta.neg {
  color: var(--text-dim);
}

.waiting {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 18px;
  border-radius: var(--radius);
  background: var(--warn-bg);
  border: 1px solid rgba(255, 201, 74, 0.35);
  color: var(--warn);
}

.waiting.approved {
  background: var(--good-bg);
  border-color: rgba(95, 227, 138, 0.35);
  color: var(--good);
}

.waiting.rejected,
.waiting.cancelled,
.waiting.refunded {
  background: var(--bad-bg);
  border-color: var(--bad-line);
  color: var(--bad);
}

.waiting div {
  flex: 1;
  display: flex;
  flex-direction: column;
  color: var(--text);
}

.waiting span {
  color: var(--text-dim);
  font-size: 13px;
}

.err {
  margin: 0;
  padding: 10px 14px;
  border-radius: 10px;
  background: var(--bad-bg);
  border: 1px solid var(--bad-line);
  color: var(--bad);
}
</style>

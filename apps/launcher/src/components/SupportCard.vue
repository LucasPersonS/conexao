<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import Icon from './Icon.vue'
import { errorMessage, shop, type ServerSummary, type SupportWall } from '../lib/api'
import { formatMoney, timeAgo } from '../lib/format'
import { loadWallet, payWithMoney, wallet } from '../lib/wallet'

const props = defineProps<{ server: ServerSummary }>()

const open = ref(false)
const wall = ref<SupportWall | null>(null)
const preset = ref<number | null>(1000)
const custom = ref('')
const message = ref('')
const anonymous = ref(false)
const busy = ref(false)
const error = ref<string | null>(null)

const PRESETS = [500, 1000, 2000, 5000]

watch(
  () => props.server.slug,
  async (slug) => {
    wall.value = null
    open.value = false
    try {
      wall.value = await shop.supporters(slug)
    } catch {
      wall.value = null
    }
    if (!wallet.catalog) loadWallet()
  },
  { immediate: true },
)

const limits = computed(() => wallet.catalog?.support ?? { min_cents: 500, max_cents: 50000, fee_percent: 15 })

/** Valor escolhido em centavos (personalizado aceita "12,50"). */
const amount = computed(() => {
  if (preset.value) return preset.value
  const value = Number(custom.value.replace(/\./g, '').replace(',', '.'))
  return Number.isFinite(value) ? Math.round(value * 100) : 0
})

const valid = computed(() => amount.value >= limits.value.min_cents && amount.value <= limits.value.max_cents)
const toServer = computed(() => amount.value - Math.round((amount.value * limits.value.fee_percent) / 100))

async function support() {
  if (!valid.value || busy.value) return
  error.value = null
  busy.value = true
  try {
    await payWithMoney(
      () => shop.support(props.server.slug, amount.value, message.value, anonymous.value),
      `Apoio de ${formatMoney(amount.value)} para ${props.server.name}`,
    )
    open.value = false
    message.value = ''
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <section class="card support">
    <div class="head">
      <Icon name="heart" :size="20" />
      <div>
        <h3>Apoie {{ server.name }}</h3>
        <small v-if="wall?.supporters_30d">{{ wall.supporters_30d }} {{ wall.supporters_30d === 1 ? 'apoiador' : 'apoiadores' }} nos últimos 30 dias</small>
        <small v-else>Ajude a manter o servidor no ar.</small>
      </div>
    </div>

    <button v-if="!open" class="btn btn-primary" @click="open = true"><Icon name="gift" :size="16" /> Apoiar com Pix ou cartão</button>

    <form v-else class="form" @submit.prevent="support">
      <div class="presets" role="radiogroup" aria-label="Valor do apoio">
        <button
          v-for="p in PRESETS"
          :key="p"
          type="button"
          class="preset"
          :class="{ on: preset === p }"
          role="radio"
          :aria-checked="preset === p"
          @click="preset = p"
        >
          {{ formatMoney(p) }}
        </button>
        <label class="preset other" :class="{ on: preset === null }">
          R$
          <input v-model="custom" inputmode="decimal" placeholder="outro" maxlength="8" @focus="preset = null" />
        </label>
      </div>
      <textarea v-model="message" maxlength="140" rows="2" placeholder="Mensagem para o mural (opcional, sem links)" />
      <label class="check"><input v-model="anonymous" type="checkbox" /> Apoiar sem mostrar meu nome</label>
      <p v-if="valid" class="split">
        {{ server.name }} recebe <b>{{ formatMoney(toServer) }}</b> direto na conta do Mercado Pago. O Conexão fica com
        {{ limits.fee_percent }}% para manter a plataforma (a taxa do Mercado Pago sai da parte do servidor).
      </p>
      <p v-else class="split warn">
        O apoio vai de {{ formatMoney(limits.min_cents) }} a {{ formatMoney(limits.max_cents) }}.
      </p>
      <p v-if="error" class="err" role="alert">{{ error }}</p>
      <div class="actions">
        <button type="button" class="btn" @click="open = false">Cancelar</button>
        <button type="submit" class="btn btn-primary" :disabled="!valid || busy">
          {{ busy ? 'Abrindo…' : `Apoiar com ${formatMoney(amount)}` }}
        </button>
      </div>
    </form>

    <ul v-if="wall?.recent.length" class="wall">
      <li v-for="(s, i) in wall.recent.slice(0, 5)" :key="i">
        <img v-if="s.supporter_uuid" :src="`https://mc-heads.net/avatar/${s.supporter_uuid}/24`" alt="" />
        <span v-else class="anon"><Icon name="heart" :size="12" /></span>
        <div>
          <strong>{{ s.supporter ?? 'Apoiador anônimo' }}</strong>
          <small v-if="s.paid_at"> · {{ timeAgo(new Date(s.paid_at).getTime()) }}</small>
          <p v-if="s.message">{{ s.message }}</p>
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.card {
  padding: 18px 20px;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.support {
  border-color: var(--lime-line);
}

.head {
  display: flex;
  gap: 10px;
  align-items: center;
  color: var(--lime);
}

h3 {
  margin: 0;
  font-size: 17px;
  color: var(--text);
}

.head small {
  color: var(--text-dim);
}

.form {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.presets {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
}

.preset {
  height: 36px;
  border-radius: 10px;
  border: 1px solid var(--line);
  background: var(--bg-soft);
  font-weight: 600;
  font-size: 13px;
}

.preset.on {
  border-color: var(--lime);
  color: var(--lime);
  box-shadow: 0 0 0 1px var(--lime-line);
}

.other {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 8px;
  color: var(--text-dim);
}

.other input {
  width: 100%;
  min-width: 0;
  border: 0;
  background: none;
  outline: none;
}

textarea {
  resize: none;
  padding: 8px 10px;
  border-radius: 10px;
  border: 1px solid var(--line);
  background: var(--bg-input);
}

.check {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: 13px;
  color: var(--text-dim);
}

.split {
  margin: 0;
  font-size: 12px;
  color: var(--text-dim);
  line-height: 1.45;
}

.split.warn,
.err {
  color: var(--bad);
}

.err {
  margin: 0;
  font-size: 13px;
}

.actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.wall {
  margin: 0;
  padding: 10px 0 0;
  list-style: none;
  border-top: 1px solid var(--line);
  display: grid;
  gap: 10px;
}

.wall li {
  display: flex;
  gap: 8px;
  font-size: 13px;
}

.wall img,
.anon {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  flex: none;
}

.anon {
  display: grid;
  place-items: center;
  background: var(--lime-soft);
  color: var(--lime);
}

.wall small {
  color: var(--text-faint);
}

.wall p {
  margin: 2px 0 0;
  color: var(--text-dim);
  overflow-wrap: anywhere;
}
</style>

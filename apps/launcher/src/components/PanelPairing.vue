<script setup lang="ts">
import { computed, ref } from 'vue'
import { api, errorMessage, type PairingInfo } from '../lib/api'
import Icon from './Icon.vue'

const emit = defineEmits<{ close: [] }>()
const code = ref('')
const busy = ref(false)
const done = ref(false)
const error = ref<string | null>(null)
/** Passo 2: quem pediu o código, para conferir antes de liberar. */
const info = ref<PairingInfo | null>(null)

const minutes = computed(() => {
  const secs = info.value?.age_secs ?? 0
  return secs < 60 ? 'agora há pouco' : `há ${Math.floor(secs / 60)} min`
})

async function check() {
  busy.value = true
  error.value = null
  try {
    info.value = await api.pairingInfo(code.value.trim())
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = false
  }
}

async function approve() {
  busy.value = true
  error.value = null
  try {
    await api.approvePanel(code.value.trim())
    done.value = true
    window.setTimeout(() => emit('close'), 1800)
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    busy.value = false
  }
}

function back() {
  info.value = null
  error.value = null
}
</script>

<template>
  <div class="backdrop" @click.self="emit('close')">
    <form v-if="!info" class="dialog" @submit.prevent="check">
      <h3>Conectar painel web</h3>
      <p>Abra o painel do Conexão no navegador, clique em <strong>Entrar com o launcher</strong> e digite aqui o código exibido.</p>
      <p class="warning">
        <Icon name="shield" :size="16" />
        <span>
          Só use um código que <strong>você mesmo</strong> gerou. Nunca digite um código que alguém te mandou: ele daria
          a essa pessoa acesso ao seu painel.
        </span>
      </p>
      <input v-model="code" class="code" placeholder="ABCD-EFGH" maxlength="9" autofocus :disabled="busy" />
      <p v-if="error" class="error">{{ error }}</p>
      <div class="actions">
        <button type="button" class="btn" @click="emit('close')">Fechar</button>
        <button type="submit" class="btn btn-primary" :disabled="busy || code.trim().length < 8">
          {{ busy ? 'Conferindo…' : 'Continuar' }}
        </button>
      </div>
    </form>

    <form v-else class="dialog" @submit.prevent="approve">
      <h3>Este pedido é seu?</h3>
      <p>Confira se foi você que abriu o painel agora, neste computador ou no seu celular.</p>
      <dl class="origin">
        <dt>Navegador</dt>
        <dd>{{ info.browser }}</dd>
        <dt>Endereço IP</dt>
        <dd>{{ info.ip }}</dd>
        <dt>Pedido</dt>
        <dd>{{ minutes }}</dd>
      </dl>
      <p class="warning">
        <Icon name="shield" :size="16" />
        <span>Se não reconhece, clique em <strong>Não fui eu</strong>. Ninguém da equipe Conexão pede esse código.</span>
      </p>
      <p v-if="done" class="ok">Painel conectado. Pode voltar ao navegador.</p>
      <p v-if="error" class="error">{{ error }}</p>
      <div class="actions">
        <button type="button" class="btn" :disabled="busy || done" @click="back">Não fui eu</button>
        <button type="submit" class="btn btn-primary" :disabled="busy || done">
          {{ busy ? 'Conectando…' : 'Sim, conectar' }}
        </button>
      </div>
    </form>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  display: grid;
  place-items: center;
  background: rgba(var(--shade), 0.7);
  backdrop-filter: blur(4px);
  z-index: 10;
}

.dialog {
  width: 420px;
  padding: 24px;
  background: var(--bg-raised);
  border: 1px solid var(--line);
  border-radius: 18px;
  box-shadow: var(--shadow-pop);
}

h3 {
  margin: 0 0 8px;
  font-size: 20px;
}

p {
  margin: 0 0 16px;
  color: var(--text-dim);
  line-height: 1.5;
  font-size: 14px;
}

.warning {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  padding: 10px 12px;
  border-radius: 12px;
  background: var(--warn-bg);
  border: 1px solid rgba(255, 201, 74, 0.3);
  color: var(--warn);
  font-size: 13px;
}

.warning :deep(svg) {
  flex: none;
  margin-top: 2px;
}

.code {
  width: 100%;
  height: 52px;
  text-align: center;
  font-size: 24px;
  font-weight: 700;
  letter-spacing: 0.2em;
  text-transform: uppercase;
  border: 1px solid var(--line);
  border-radius: 12px;
  background: var(--bg-input);
  color: var(--lime);
  font-family: var(--font-pixel);
  margin-bottom: 16px;
  outline: none;
}

.code:focus {
  border-color: var(--lime);
  box-shadow: 0 0 0 3px var(--lime-soft);
}

.origin {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 8px 16px;
  margin: 0 0 16px;
  padding: 12px 14px;
  border-radius: 12px;
  background: var(--bg-input);
  border: 1px solid var(--line);
  font-size: 14px;
}

.origin dt {
  color: var(--text-faint);
}

.origin dd {
  margin: 0;
  font-weight: 600;
  text-align: right;
  user-select: text;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.ok {
  color: var(--good);
  font-weight: 600;
}

.error {
  color: var(--bad);
}
</style>

<script setup lang="ts">
/** Perfil de outro jogador (aberto pelos comentários, apoiadores etc.), por cima da tela atual. */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import ProfileCard from '@profile/ProfileCard.vue'
import type { PublicProfile } from '@profile/theme'
import { errorMessage, profiles } from '../lib/api'
import { go, showToast, store } from '../lib/store'
import Icon from './Icon.vue'

const props = defineProps<{ name: string }>()
const emit = defineEmits<{ close: [] }>()

const profile = ref<PublicProfile | null>(null)
const error = ref<string | null>(null)
const reporting = ref(false)
const reason = ref('')
const sending = ref(false)
const dialog = ref<HTMLElement | null>(null)

const isMe = computed(() => !!store.me && store.me.name.toLowerCase() === props.name.toLowerCase())

async function load(name: string) {
  profile.value = null
  error.value = null
  reporting.value = false
  try {
    profile.value = await profiles.get(name)
  } catch (e) {
    error.value = errorMessage(e)
  }
}
watch(() => props.name, load, { immediate: true })

function onKey(event: KeyboardEvent) {
  if (event.key === 'Escape') emit('close')
}
onMounted(() => {
  document.addEventListener('keydown', onKey)
  dialog.value?.focus()
})
onBeforeUnmount(() => document.removeEventListener('keydown', onKey))

async function copyDiscord(value: string) {
  try {
    await navigator.clipboard.writeText(value)
    showToast(`Discord "${value}" copiado.`)
  } catch {
    /* sem área de transferência */
  }
}

function editMine() {
  emit('close')
  go('profile')
}

async function report() {
  const text = reason.value.trim()
  if (!text || sending.value) return
  sending.value = true
  try {
    await profiles.report(props.name, text)
    showToast('Denúncia enviada. A equipe Conexão vai analisar.')
    reporting.value = false
    reason.value = ''
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    sending.value = false
  }
}
</script>

<template>
  <div class="backdrop" @click.self="emit('close')">
    <div ref="dialog" class="dialog" role="dialog" aria-modal="true" :aria-label="`Perfil de ${name}`" tabindex="-1">
      <button class="close" title="Fechar (Esc)" @click="emit('close')"><Icon name="x" :size="18" /></button>

      <div v-if="profile" class="card-wrap">
        <ProfileCard :profile="profile" @discord="copyDiscord" />
      </div>
      <div v-else-if="error" class="state err">
        <Icon name="x" :size="22" />
        <p>{{ error }}</p>
      </div>
      <div v-else class="state">
        <span class="spinner" />
        <p>Carregando perfil…</p>
      </div>

      <div v-if="profile" class="actions">
        <button v-if="isMe" class="btn btn-primary" @click="editMine"><Icon name="star" :size="16" /> Editar meu perfil</button>
        <template v-else>
          <button v-if="!reporting" class="btn report" @click="reporting = true"><Icon name="shield" :size="16" /> Denunciar perfil</button>
          <form v-else class="report-form" @submit.prevent="report">
            <input v-model="reason" maxlength="300" placeholder="O que há de errado neste perfil?" autofocus />
            <button type="button" class="btn" @click="reporting = false">Cancelar</button>
            <button class="btn btn-primary" :disabled="!reason.trim() || sending">{{ sending ? 'Enviando…' : 'Enviar' }}</button>
          </form>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgba(var(--shade), 0.72);
  backdrop-filter: blur(4px);
  z-index: 30;
  animation: fade 0.15s ease-out;
}

.dialog {
  position: relative;
  width: fit-content;
  min-width: min(360px, 100%);
  max-width: min(460px, 100%);
  max-height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 14px;
  outline: none;
  animation: pop 0.18s ease-out;
}

.close {
  position: absolute;
  top: 10px;
  right: 10px;
  z-index: 2;
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  border-radius: 10px;
  background: rgba(var(--shade), 0.6);
  color: var(--white);
}

.close:hover {
  background: rgba(var(--shade), 0.9);
}

.state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 48px 24px;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--line);
  color: var(--text-dim);
  text-align: center;
}

.state p {
  margin: 0;
}

.state.err {
  color: var(--bad);
}

.spinner {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  border: 3px solid var(--line);
  border-top-color: var(--lime);
  animation: spin 0.8s linear infinite;
}

.actions {
  display: flex;
  justify-content: flex-end;
}

.report {
  color: var(--text-dim);
}

.report-form {
  display: flex;
  gap: 8px;
  width: 100%;
}

.report-form input {
  flex: 1;
  min-width: 0;
  height: 40px;
  padding: 0 12px;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: var(--bg-input);
}

@keyframes fade {
  from {
    opacity: 0;
  }
}

@keyframes pop {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.98);
  }
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>

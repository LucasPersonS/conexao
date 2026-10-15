<script setup lang="ts">
/**
 * Aviso de atualização do launcher: um card pequeno no canto inferior direito.
 * Confere ao abrir, a cada 6 horas e quando o launcher sai do repouso. "Depois" esconde
 * até aparecer uma versão mais nova. Com o jogo aberto, espera ele fechar.
 */
import { listen } from '@tauri-apps/api/event'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import logo from '../assets/logo.png'
import { activity, onWake, resting } from '../lib/activity'
import { api, errorMessage, type UpdateInfo, type UpdateProgress } from '../lib/api'
import Icon from './Icon.vue'

const CHECK_EVERY = 6 * 60 * 60 * 1000

const update = ref<UpdateInfo | null>(null)
const dismissed = ref<string | null>(null)
const showNotes = ref(false)
const busy = ref(false)
const progress = ref<UpdateProgress | null>(null)
const error = ref<string | null>(null)

const visible = computed(() => update.value !== null && (busy.value || dismissed.value !== update.value.version))
const playing = computed(() => activity.playing !== null)
const percent = computed(() => {
  const p = progress.value
  if (!p) return 0
  if (p.installing) return 100
  return p.total ? Math.min(100, Math.round((100 * p.downloaded) / p.total)) : 0
})

let lastCheck = 0
async function check() {
  if (busy.value || resting.value) return
  lastCheck = Date.now()
  try {
    update.value = await api.checkUpdate()
  } catch {
    /* sem internet: tenta de novo mais tarde */
  }
}

async function install() {
  if (playing.value) return
  busy.value = true
  error.value = null
  progress.value = { downloaded: 0, total: null, installing: false }
  try {
    // No Windows o launcher fecha aqui, roda o instalador e abre de novo sozinho.
    await api.installUpdate()
  } catch (e) {
    error.value = errorMessage(e)
    busy.value = false
    progress.value = null
  }
}

function later() {
  if (update.value) dismissed.value = update.value.version
  showNotes.value = false
}

let timer: number | undefined
let stopProgress: (() => void) | undefined
let stopWake: (() => void) | undefined
onMounted(async () => {
  check()
  timer = window.setInterval(check, CHECK_EVERY)
  stopWake = onWake(() => Date.now() - lastCheck > CHECK_EVERY && check())
  stopProgress = await listen<UpdateProgress>('update-progress', (e) => (progress.value = e.payload)).catch(() => undefined)
})
onUnmounted(() => {
  window.clearInterval(timer)
  stopProgress?.()
  stopWake?.()
})
</script>

<template>
  <Transition name="update">
    <aside v-if="visible && update" class="update-card" role="status" aria-live="polite">
      <div class="head">
        <div class="mark">
          <img :src="logo" alt="" />
          <span class="ping" />
        </div>
        <div class="text">
          <strong>{{ busy ? (progress?.installing ? 'Instalando a atualização…' : 'Baixando a atualização…') : 'Atualização disponível' }}</strong>
          <span class="versions">
            {{ update.current }} <Icon name="arrow" :size="12" /> <b>{{ update.version }}</b>
          </span>
        </div>
        <button v-if="!busy" class="close" aria-label="Lembrar depois" title="Lembrar depois" @click="later">
          <Icon name="x" :size="14" />
        </button>
      </div>

      <template v-if="busy">
        <div class="bar" :class="{ indeterminate: !progress?.total || progress?.installing }">
          <span :style="{ width: `${percent}%` }" />
        </div>
        <p class="hint">
          {{ progress?.installing ? 'O Conexão vai fechar e abrir de novo sozinho.' : `${percent}% · não feche o launcher` }}
        </p>
      </template>
      <template v-else>
        <p v-if="update.notes && showNotes" class="notes">{{ update.notes }}</p>
        <p v-if="error" class="error">{{ error }}</p>
        <p v-else-if="playing" class="hint">Feche o jogo para atualizar. O aviso continua aqui.</p>
        <div class="actions">
          <button class="btn btn-primary go" :disabled="playing" @click="install">
            <Icon name="download" :size="15" />
            Atualizar e reiniciar
          </button>
          <button v-if="update.notes" class="btn ghost" @click="showNotes = !showNotes">
            {{ showNotes ? 'Esconder' : 'Novidades' }}
          </button>
          <button v-else class="btn ghost" @click="later">Depois</button>
        </div>
      </template>
    </aside>
  </Transition>
</template>

<style scoped>
.update-card {
  position: fixed;
  right: 18px;
  bottom: 18px;
  z-index: 95;
  width: 320px;
  padding: 14px;
  border-radius: 16px;
  background:
    linear-gradient(135deg, rgba(183, 254, 0, 0.12), rgba(183, 254, 0, 0) 55%),
    rgba(14, 21, 24, 0.96);
  border: 1px solid var(--lime-line);
  box-shadow: var(--shadow-pop), 0 0 40px rgba(183, 254, 0, 0.12);
  backdrop-filter: blur(10px);
}

/* Fio de luz no topo, como a barra de título */
.update-card::before {
  content: '';
  position: absolute;
  left: 16px;
  right: 16px;
  top: -1px;
  height: 1px;
  background: linear-gradient(90deg, transparent, var(--lime), transparent);
}

.head {
  display: flex;
  align-items: center;
  gap: 12px;
}

.mark {
  position: relative;
  flex: none;
  width: 38px;
  height: 38px;
  border-radius: 11px;
  display: grid;
  place-items: center;
  background: rgba(183, 254, 0, 0.1);
  border: 1px solid var(--lime-line);
}

.mark img {
  width: 26px;
  height: 26px;
  filter: drop-shadow(0 0 6px rgba(183, 254, 0, 0.5));
}

.ping {
  position: absolute;
  top: -3px;
  right: -3px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--lime);
  box-shadow: 0 0 0 3px var(--bg-soft);
  animation: ping 1.8s ease-in-out infinite;
}

@keyframes ping {
  50% {
    box-shadow: 0 0 0 3px var(--bg-soft), 0 0 12px var(--lime);
  }
}

.text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.text strong {
  font-family: var(--font-display);
  font-size: 15px;
}

.versions {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  color: var(--text-faint);
}

.versions b {
  color: var(--lime);
}

.close {
  align-self: flex-start;
  width: 26px;
  height: 26px;
  border-radius: 8px;
  display: grid;
  place-items: center;
  color: var(--text-faint);
}

.close:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.notes {
  margin: 12px 0 0;
  max-height: 120px;
  overflow: auto;
  white-space: pre-line;
  font-size: 13px;
  line-height: 1.5;
  color: var(--text-dim);
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--bg-input);
  border: 1px solid var(--line);
}

.hint,
.error {
  margin: 10px 0 0;
  font-size: 12px;
  color: var(--text-dim);
}

.error {
  color: var(--bad);
}

.actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.actions .btn {
  height: 34px;
  font-size: 13px;
}

.go {
  flex: 1;
  gap: 7px;
}

.ghost {
  padding: 0 12px;
}

.bar {
  position: relative;
  height: 6px;
  margin-top: 14px;
  border-radius: 99px;
  overflow: hidden;
  background: var(--bg-input);
  border: 1px solid var(--line);
}

.bar span {
  display: block;
  height: 100%;
  border-radius: 99px;
  background: linear-gradient(90deg, var(--lime-yellow), var(--lime), var(--lime-deep));
  box-shadow: 0 0 10px rgba(183, 254, 0, 0.5);
  transition: width 0.2s ease-out;
}

.bar.indeterminate span {
  width: 35% !important;
  animation: slide 1.1s ease-in-out infinite;
}

@keyframes slide {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(300%);
  }
}

.update-enter-active,
.update-leave-active {
  transition: opacity 0.25s, transform 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
}

.update-enter-from,
.update-leave-to {
  opacity: 0;
  transform: translateY(16px) scale(0.98);
}
</style>

<script setup lang="ts">
/**
 * Botão "Jogar" inteligente: mostra sempre o próximo passo do jogador
 * (pedir whitelist, aguardar, instalar, atualizar, jogar) e o progresso
 * da instalação em etapas.
 */
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import { api, errorMessage, type AccessResponse, type InstallState, type PlayProgress, type ServerSummary } from '../lib/api'
import { formatBytes, friendlyError } from '../lib/format'
import { markPlayed } from '../lib/prefs'
import { activity, resting } from '../lib/activity'
import Icon from './Icon.vue'
import SponsorBanner from './SponsorBanner.vue'

const props = defineProps<{ server: ServerSummary; compact?: boolean }>()

const access = ref<AccessResponse | null>(null)
const accessError = ref<string | null>(null)
const install = ref<InstallState | null>(null)
const progress = ref<PlayProgress>({ stage: 'idle', message: '', current: 0, total: 0, step: 0 })
const applying = ref(false)
let timer: number | undefined
let alive = true

const active = computed(() => ['checking', 'installing', 'launching'].includes(progress.value.stage))

const STEPS = ['Verificando acesso', 'Minecraft e Java', 'Loader', 'Mods e arquivos', 'Abrindo o jogo']

async function refresh() {
  accessError.value = null
  const slug = props.server.slug
  const [a, i] = await Promise.allSettled([api.access(slug), api.installState(slug)])
  if (slug !== props.server.slug) return
  if (a.status === 'fulfilled') access.value = a.value
  else accessError.value = errorMessage(a.reason)
  if (i.status === 'fulfilled') install.value = i.value
}

async function poll() {
  if (!alive) return
  try {
    const before = progress.value.stage
    progress.value = await api.progress(props.server.slug)
    // Terminou de jogar ou de instalar: atualiza o estado da instalação.
    if (before !== progress.value.stage && ['idle', 'error'].includes(progress.value.stage)) {
      install.value = await api.installState(props.server.slug)
    }
  } catch {
    // ignora: tenta de novo no próximo ciclo
  }
  window.clearTimeout(timer)
  // Rápido só durante a instalação; parado, quase nada (o evento "game" acorda o botão).
  timer = window.setTimeout(poll, active.value ? 400 : resting.value ? 15_000 : 4000)
}

watch(() => [activity.playing, resting.value], () => alive && poll())

watch(
  () => props.server.slug,
  () => {
    access.value = null
    install.value = null
    progress.value = { stage: 'idle', message: '', current: 0, total: 0, step: 0 }
    refresh()
    poll()
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  alive = false
  window.clearTimeout(timer)
})

const percent = computed(() =>
  progress.value.total > 0 ? Math.min(100, Math.round((progress.value.current / progress.value.total) * 100)) : null,
)
const size = computed(() => formatBytes(props.server.pack_size))

type Action = 'play' | 'apply' | 'none'
interface Primary {
  label: string
  action: Action
  tone?: 'warn' | 'bad'
  hint?: string
}

const primary = computed<Primary>(() => {
  const p = progress.value
  if (active.value) return { label: p.message || 'Preparando…', action: 'none' }
  if (p.stage === 'running') return { label: 'Em jogo', action: 'none', hint: 'O Minecraft está aberto.' }
  if (!props.server.current_version)
    return { label: 'Em breve', action: 'none', hint: 'Este servidor ainda não publicou o modpack.' }
  const a = access.value
  if (!a) return { label: accessError.value ? 'Indisponível' : 'Verificando…', action: 'none' }
  if (!a.can_play) {
    if (a.reason) return { label: 'Só contas originais', action: 'none', tone: 'warn', hint: a.reason }
    switch (a.status) {
      case 'pending':
        return {
          label: 'Aguardando aprovação',
          action: 'none',
          tone: 'warn',
          hint: 'Seu pedido de whitelist foi enviado. A equipe do servidor vai analisar.',
        }
      case 'rejected':
        return { label: 'Pedido recusado', action: 'none', tone: 'bad', hint: 'Fale com a equipe do servidor no Discord.' }
      case 'banned':
        return { label: 'Acesso bloqueado', action: 'none', tone: 'bad', hint: 'Você foi banido deste servidor.' }
      default:
        return { label: 'Pedir whitelist', action: 'apply', hint: 'Este servidor só libera a entrada de quem está na whitelist.' }
    }
  }
  const i = install.value
  if (i && !i.installed) return { label: size.value ? `Instalar e jogar · ${size.value}` : 'Instalar e jogar', action: 'play', hint: 'Na primeira vez o Conexão baixa o Java, o loader e os mods.' }
  if (i?.installed && i.version !== props.server.current_version)
    return { label: 'Atualizar e jogar', action: 'play', hint: `Nova versão do modpack: ${props.server.current_version}` }
  return { label: 'Jogar', action: 'play' }
})

const showSteps = computed(() => active.value || (progress.value.stage === 'error' && progress.value.step > 0))
const error = computed(() => (progress.value.stage === 'error' ? progress.value.message : null))

async function onPrimary() {
  if (primary.value.action === 'apply') {
    applying.value = true
    try {
      access.value = await api.apply(props.server.slug)
    } catch (e) {
      accessError.value = errorMessage(e)
    } finally {
      applying.value = false
    }
  } else if (primary.value.action === 'play') {
    markPlayed(props.server.slug)
    progress.value = { stage: 'checking', message: 'Iniciando', current: 0, total: 0, step: 0 }
    try {
      await api.play(props.server.slug, props.server.address)
    } catch (e) {
      progress.value = { ...progress.value, stage: 'error', message: errorMessage(e) }
    }
    poll()
  }
}
</script>

<template>
  <div class="play-box" :class="{ compact }">
    <button
      class="btn btn-primary main"
      :class="[primary.tone, { working: active || progress.stage === 'running' }]"
      :disabled="primary.action === 'none' || applying"
      @click.stop="onPrimary"
    >
      <Icon v-if="primary.action === 'play'" name="play" :size="16" />
      {{ applying ? 'Enviando pedido…' : primary.label }}
      <span v-if="active && percent !== null" class="pct">{{ percent }}%</span>
    </button>

    <div v-if="active" class="progress">
      <div class="bar" :class="{ indeterminate: percent === null }" :style="{ width: `${percent ?? 20}%` }" />
    </div>

    <template v-if="!compact">
      <p v-if="primary.hint && !active && !error" class="hint" :class="primary.tone">{{ primary.hint }}</p>
      <p v-if="accessError && !access" class="hint bad">{{ accessError }}</p>

      <ol v-if="showSteps" class="steps">
        <li
          v-for="(label, index) in STEPS"
          :key="label"
          :class="{
            done: index < progress.step,
            current: index === progress.step,
            failed: index === progress.step && !!error,
          }"
        >
          <span class="mark">
            <Icon v-if="index < progress.step" name="check" :size="12" />
            <Icon v-else-if="index === progress.step && error" name="x" :size="12" />
          </span>
          {{ label }}
        </li>
      </ol>

      <!-- Enquanto baixa e abre o jogo: banner de patrocinador (troca a cada 10 s). -->
      <SponsorBanner v-if="active" placement="loading" variant="side" :interval="10000" class="loading-sponsor" />

      <div v-if="error" class="error">
        <p>{{ friendlyError(error) }}</p>
        <div class="error-actions">
          <button class="btn small" @click="onPrimary">Tentar de novo</button>
          <button v-if="server.discord_url" class="btn small" @click="openUrl(server.discord_url)">Pedir ajuda no Discord</button>
        </div>
        <details>
          <summary>Ver detalhes</summary>
          <code>{{ error }}</code>
        </details>
      </div>
    </template>
    <p v-else-if="error" class="hint bad">{{ friendlyError(error) }}</p>
  </div>
</template>

<style scoped>
.play-box {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.main {
  position: relative;
  height: 52px;
  font-family: var(--font-display);
  font-size: 18px;
  font-weight: 700;
  letter-spacing: 0.01em;
  box-shadow: 0 0 0 1px rgba(183, 254, 0, 0.5), 0 8px 28px rgba(183, 254, 0, 0.28);
}

.compact .main {
  height: 44px;
  font-size: 16px;
}

.main:disabled {
  box-shadow: none;
}

.main:disabled:not(.working):not(.warn):not(.bad) {
  background: var(--bg-hover);
  border-color: var(--line-strong);
  color: var(--text-dim);
}

.main.working:disabled {
  opacity: 1;
  background: var(--lime-soft);
  border-color: var(--lime-line);
  color: var(--lime);
}

.main.warn:disabled {
  opacity: 1;
  background: var(--warn-bg);
  border-color: rgba(255, 201, 74, 0.35);
  color: var(--warn);
}

.main.bad:disabled {
  opacity: 1;
  background: var(--bad-bg);
  border-color: var(--bad-line);
  color: var(--bad);
}

.pct {
  font-weight: 600;
  opacity: 0.75;
}

.progress {
  height: 6px;
  border-radius: 999px;
  background: var(--bg-hover);
  overflow: hidden;
}

.bar {
  height: 100%;
  background: linear-gradient(90deg, var(--lime-deep), var(--lime), var(--lime-yellow));
  box-shadow: 0 0 10px rgba(183, 254, 0, 0.7);
  transition: width 0.3s;
}

.bar.indeterminate {
  animation: slide 1.2s ease-in-out infinite;
}

@keyframes slide {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(500%);
  }
}

.hint {
  margin: 0;
  font-size: 13px;
  line-height: 1.45;
  color: var(--text-dim);
}

.hint.warn {
  color: var(--warn);
}

.hint.bad {
  color: var(--bad);
}

.loading-sponsor {
  margin-top: 6px;
}

.steps {
  list-style: none;
  margin: 4px 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: 13px;
  color: var(--text-faint);
}

.steps li {
  display: flex;
  align-items: center;
  gap: 10px;
}

.mark {
  width: 18px;
  height: 18px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  border: 2px solid var(--line-strong);
  color: var(--ink);
  flex: none;
}

.done {
  color: var(--text-dim);
}

.done .mark {
  background: var(--lime);
  border-color: var(--lime);
}

.current {
  color: var(--text);
  font-weight: 600;
}

.current .mark {
  border-color: var(--lime);
  border-top-color: transparent;
  animation: spin 0.9s linear infinite;
}

.failed {
  color: var(--bad);
}

.failed .mark {
  background: var(--bad);
  border-color: var(--bad);
  color: #fff;
  animation: none;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.error {
  padding: 12px;
  border-radius: 12px;
  background: var(--bad-bg);
  border: 1px solid var(--bad-line);
  font-size: 13px;
}

.error p {
  margin: 0 0 10px;
  color: #ffb3b3;
  line-height: 1.45;
}

.error-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.small {
  height: 32px;
  padding: 0 12px;
  font-size: 13px;
}

details {
  margin-top: 10px;
  color: var(--text-faint);
}

summary {
  cursor: pointer;
}

code {
  display: block;
  margin-top: 6px;
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-all;
  user-select: text;
}
</style>

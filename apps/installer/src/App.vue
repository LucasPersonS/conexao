<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import banner from './assets/banner.jpg'
import logo from './assets/logo.png'
import wordmark from './assets/wordmark.png'
import { close, install, launch, minimize, releaseInfo, type Progress, type ReleaseInfo } from './lib/installer'

type State = 'loading' | 'ready' | 'installing' | 'done' | 'error'

const state = ref<State>('loading')
const release = ref<ReleaseInfo | null>(null)
const error = ref('')
const progress = ref<Progress | null>(null)
const shortcut = ref(true)
const openAfter = ref(true)
const opening = ref(false)
/** Progresso exibido (0–1): o download vai até 80%; o resto é conferir e instalar. */
const shown = ref(0)

const STEPS = [
  { key: 'download', label: 'Baixar' },
  { key: 'verify', label: 'Conferir' },
  { key: 'install', label: 'Instalar' },
  { key: 'done', label: 'Pronto' },
] as const

function mb(bytes: number): string {
  return `${(bytes / 1048576).toLocaleString('pt-BR', { minimumFractionDigits: 1, maximumFractionDigits: 1 })} MB`
}

async function load() {
  state.value = 'loading'
  error.value = ''
  try {
    release.value = await releaseInfo()
    state.value = 'ready'
  } catch (e) {
    error.value = String(e)
    state.value = 'error'
  }
}

let creep: number | undefined
function onProgress(p: Progress) {
  progress.value = p
  window.clearInterval(creep)
  if (p.stage === 'download') {
    shown.value = p.total ? 0.8 * (p.downloaded / p.total) : Math.min(0.75, shown.value + 0.01)
  } else if (p.stage === 'verify') {
    shown.value = 0.84
  } else if (p.stage === 'install') {
    // O NSIS não informa o progresso: avança devagar até 97% enquanto ele trabalha.
    shown.value = 0.86
    creep = window.setInterval(() => (shown.value += (0.97 - shown.value) * 0.06), 200)
  } else {
    shown.value = 0.99
  }
}

async function start() {
  state.value = 'installing'
  shown.value = 0
  progress.value = { stage: 'download', downloaded: 0, total: release.value?.size ?? null, bytes_per_sec: 0 }
  try {
    await install(shortcut.value, onProgress)
    window.clearInterval(creep)
    shown.value = 1
    state.value = 'done'
    if (openAfter.value) {
      opening.value = true
      window.setTimeout(open, 1400)
    }
  } catch (e) {
    window.clearInterval(creep)
    error.value = String(e)
    state.value = 'error'
  }
}

async function open() {
  opening.value = true
  try {
    await launch()
  } catch (e) {
    opening.value = false
    error.value = String(e)
    state.value = 'error'
  }
}

function retry() {
  if (release.value) start()
  else load()
}

const stepIndex = computed(() => {
  if (state.value === 'done') return 3
  switch (progress.value?.stage) {
    case 'verify':
      return 1
    case 'install':
    case 'shortcut':
      return 2
    default:
      return 0
  }
})

const title = computed(() => {
  switch (progress.value?.stage) {
    case 'verify':
      return 'Conferindo a assinatura'
    case 'install':
      return 'Instalando o Conexão'
    case 'shortcut':
      return 'Criando o atalho'
    default:
      return 'Baixando o Conexão'
  }
})

const detail = computed(() => {
  const p = progress.value
  if (p?.stage === 'download') {
    const total = p.total ?? release.value?.size ?? null
    const parts = [total ? `${mb(p.downloaded)} de ${mb(total)}` : mb(p.downloaded)]
    if (p.bytes_per_sec > 0) {
      parts.push(`${mb(p.bytes_per_sec)}/s`)
      if (total) {
        const left = Math.max(0, Math.ceil((total - p.downloaded) / p.bytes_per_sec))
        parts.push(left > 0 ? `faltam ${left} s` : 'quase lá')
      }
    }
    return parts.join(' · ')
  }
  if (p?.stage === 'verify') return 'Garantindo que o arquivo veio do Conexão e não foi alterado.'
  if (p?.stage === 'install') return 'Só leva alguns segundos, sem pedir administrador.'
  return 'Finalizando…'
})

const percent = computed(() => Math.round(shown.value * 100))
// Anel de progresso: circunferência do círculo de raio 76.
const CIRC = 2 * Math.PI * 76
const dash = computed(() => (state.value === 'error' ? `${CIRC} ${CIRC}` : `${CIRC * shown.value} ${CIRC}`))

onMounted(load)
</script>

<template>
  <div class="window" :class="state">
    <!-- Fundo: banner escurecido, luzes em gradiente, grade em perspectiva e blocos de pixel -->
    <div class="bg" aria-hidden="true">
      <div class="photo" :style="{ backgroundImage: `url(${banner})` }" />
      <div class="blob b1" />
      <div class="blob b2" />
      <div class="blob b3" />
      <div class="floor"><div class="grid" /></div>
      <span v-for="n in 9" :key="n" class="pixel" :class="`p${n}`" />
      <div class="vignette" />
    </div>

    <header class="titlebar" data-tauri-drag-region>
      <div class="brand" data-tauri-drag-region>
        <img :src="logo" alt="" data-tauri-drag-region />
        <span data-tauri-drag-region>Instalador</span>
      </div>
      <div class="controls">
        <button aria-label="Minimizar" @click="minimize">
          <svg viewBox="0 0 12 12" width="12" height="12"><path d="M2 6h8" stroke="currentColor" stroke-width="1.4" /></svg>
        </button>
        <button class="x" aria-label="Fechar" @click="close">
          <svg viewBox="0 0 12 12" width="12" height="12">
            <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" stroke-width="1.4" />
          </svg>
        </button>
      </div>
    </header>

    <main class="stage">
      <!-- Carregando -->
      <section v-if="state === 'loading'" key="loading" class="view">
        <div class="emblem">
          <span class="halo" />
          <img :src="logo" alt="" class="logo float" />
        </div>
        <p class="muted pulse">Buscando a versão mais nova…</p>
      </section>

      <!-- Boas-vindas -->
      <section v-else-if="state === 'ready'" key="ready" class="view">
        <div class="emblem">
          <span class="halo" />
          <img :src="logo" alt="" class="logo float" />
        </div>
        <img :src="wordmark" alt="Conexão" class="wordmark" />
        <p class="lead">Os servidores de Minecraft Roleplay num só lugar.</p>

        <div class="cta">
          <span class="corner tl" /><span class="corner tr" /><span class="corner bl" /><span class="corner br" />
          <button v-if="release?.installed" class="primary" @click="open">
            <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><path d="M8 5v14l11-7z" fill="currentColor" /></svg>
            Abrir o Conexão
          </button>
          <button v-else class="primary" @click="start">
            <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
              <path
                d="M12 3v11m0 0-4.5-4.5M12 14l4.5-4.5M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2"
                fill="none"
                stroke="currentColor"
                stroke-width="2.2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
            Instalar o Conexão
          </button>
        </div>
        <button v-if="release?.installed" class="secondary" @click="start">Atualizar / reinstalar</button>

        <div class="options">
          <label class="chip" :class="{ on: shortcut }">
            <input v-model="shortcut" type="checkbox" />
            <span class="box" />
            Atalho na área de trabalho
          </label>
          <label class="chip" :class="{ on: openAfter }">
            <input v-model="openAfter" type="checkbox" />
            <span class="box" />
            Abrir ao terminar
          </label>
        </div>

        <p class="meta">
          <span v-if="release">Versão {{ release.version }}</span>
          <span v-if="release?.size">{{ mb(release.size) }}</span>
          <span>Windows 10 e 11</span>
          <span>Sem pedir administrador</span>
        </p>
      </section>

      <!-- Instalando / pronto / erro -->
      <section v-else key="progress" class="view">
        <div class="ring-wrap">
          <svg class="ring" viewBox="0 0 180 180" aria-hidden="true">
            <defs>
              <linearGradient id="ring-grad" x1="0" y1="0" x2="1" y2="1">
                <stop offset="0%" stop-color="#e4fa00" />
                <stop offset="50%" stop-color="#b7fe00" />
                <stop offset="100%" stop-color="#5fe38a" />
              </linearGradient>
            </defs>
            <circle class="track" cx="90" cy="90" r="76" />
            <circle class="bar" :class="{ bad: state === 'error' }" cx="90" cy="90" r="76" :stroke-dasharray="dash" />
          </svg>
          <img :src="logo" alt="" class="logo inner" :class="{ float: state === 'installing' }" />
          <span v-if="state === 'done'" class="badge ok">
            <svg viewBox="0 0 24 24" width="22" height="22">
              <path d="M5 12.5l4.5 4.5L19 7.5" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </span>
          <span v-else-if="state === 'error'" class="badge err">!</span>
        </div>

        <template v-if="state === 'installing'">
          <p class="percent">{{ percent }}<small>%</small></p>
          <h1>{{ title }}</h1>
          <p class="muted detail">{{ detail }}</p>
        </template>
        <template v-else-if="state === 'done'">
          <h1 class="big">Tudo pronto!</h1>
          <p class="muted detail">
            {{ opening ? 'Abrindo o Conexão…' : 'Entre com a sua conta Microsoft e escolha um servidor para jogar.' }}
          </p>
          <div v-if="!opening" class="cta small">
            <button class="primary" @click="open">Abrir o Conexão</button>
          </div>
        </template>
        <template v-else>
          <h1 class="big">Algo deu errado</h1>
          <p class="muted detail error">{{ error }}</p>
          <div class="cta small">
            <button class="primary" @click="retry">Tentar de novo</button>
          </div>
        </template>

        <ol class="steps">
          <li
            v-for="(step, i) in STEPS"
            :key="step.key"
            :class="{
              done: i < stepIndex || state === 'done',
              active: i === stepIndex && state === 'installing',
              bad: state === 'error' && i === stepIndex,
            }"
          >
            <span class="dot" />{{ step.label }}
          </li>
        </ol>
      </section>
    </main>
  </div>
</template>

<style scoped>
.window {
  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--line);
}

/* ---------- Fundo ---------- */
.bg {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
}

.photo {
  position: absolute;
  inset: -20px;
  background: center / cover no-repeat;
  opacity: 0.16;
  filter: blur(3px) saturate(1.1);
}

.blob {
  position: absolute;
  border-radius: 50%;
  filter: blur(70px);
  opacity: 0.55;
  mix-blend-mode: screen;
}

.b1 {
  width: 380px;
  height: 380px;
  left: -120px;
  top: -140px;
  background: radial-gradient(circle, rgba(183, 254, 0, 0.55), transparent 65%);
  animation: drift1 14s ease-in-out infinite alternate;
}

.b2 {
  width: 420px;
  height: 420px;
  right: -160px;
  top: 40px;
  background: radial-gradient(circle, rgba(228, 250, 0, 0.35), transparent 65%);
  animation: drift2 18s ease-in-out infinite alternate;
}

.b3 {
  width: 360px;
  height: 360px;
  left: 30%;
  bottom: -220px;
  background: radial-gradient(circle, rgba(95, 227, 138, 0.35), transparent 65%);
  animation: drift3 16s ease-in-out infinite alternate;
}

@keyframes drift1 {
  to {
    transform: translate(120px, 80px) scale(1.15);
  }
}

@keyframes drift2 {
  to {
    transform: translate(-140px, 60px) scale(0.9);
  }
}

@keyframes drift3 {
  to {
    transform: translate(-90px, -60px) scale(1.2);
  }
}

/* Grade em perspectiva, como o chão dos banners */
.floor {
  position: absolute;
  left: -40%;
  right: -40%;
  bottom: -10px;
  height: 52%;
  perspective: 320px;
  mask-image: linear-gradient(to top, #000 10%, transparent 95%);
}

.grid {
  position: absolute;
  inset: 0 0 -60% 0;
  transform: rotateX(62deg);
  transform-origin: bottom;
  background-image:
    linear-gradient(rgba(183, 254, 0, 0.32) 1px, transparent 1px),
    linear-gradient(90deg, rgba(183, 254, 0, 0.32) 1px, transparent 1px);
  background-size: 48px 48px;
  animation: run 3.5s linear infinite;
}

@keyframes run {
  to {
    background-position: 0 48px;
  }
}

.pixel {
  position: absolute;
  width: 8px;
  height: 8px;
  background: var(--lime);
  box-shadow: 0 0 12px rgba(183, 254, 0, 0.7);
  opacity: 0;
  animation: rise 9s linear infinite;
}

.p1 { left: 6%; bottom: 18%; animation-delay: 0s; }
.p2 { left: 12%; bottom: 40%; width: 5px; height: 5px; animation-delay: -3s; }
.p3 { left: 20%; bottom: 10%; width: 11px; height: 11px; animation-delay: -6s; }
.p4 { right: 8%; bottom: 30%; animation-delay: -1.5s; }
.p5 { right: 15%; bottom: 12%; width: 5px; height: 5px; animation-delay: -4.5s; }
.p6 { right: 22%; bottom: 46%; width: 10px; height: 10px; animation-delay: -7.5s; }
.p7 { left: 30%; bottom: 60%; width: 4px; height: 4px; animation-delay: -2s; }
.p8 { right: 32%; bottom: 66%; width: 6px; height: 6px; animation-delay: -5s; }
.p9 { right: 4%; bottom: 70%; width: 7px; height: 7px; animation-delay: -8s; }

@keyframes rise {
  0% {
    transform: translateY(0);
    opacity: 0;
  }
  15% {
    opacity: 0.75;
  }
  85% {
    opacity: 0.6;
  }
  100% {
    transform: translateY(-150px);
    opacity: 0;
  }
}

.vignette {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse 70% 60% at 50% 45%, transparent 0%, rgba(10, 15, 17, 0.55) 60%, rgba(10, 15, 17, 0.92) 100%),
    linear-gradient(180deg, rgba(10, 15, 17, 0.6), transparent 30%);
}

/* ---------- Barra de título ---------- */
.titlebar {
  position: relative;
  z-index: 2;
  height: 38px;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-left: 12px;
  background: rgba(7, 11, 12, 0.72);
  border-bottom: 1px solid var(--line);
  backdrop-filter: blur(8px);
}

.titlebar::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 1px;
  background: linear-gradient(90deg, var(--lime) 0%, rgba(183, 254, 0, 0.35) 25%, transparent 65%);
}

.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: var(--font-pixel);
  font-size: 11px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--lime);
}

.brand img {
  width: 22px;
  height: 22px;
  filter: drop-shadow(0 0 6px rgba(183, 254, 0, 0.5));
}

.controls {
  display: flex;
  height: 100%;
}

.controls button {
  width: 46px;
  height: 100%;
  display: grid;
  place-items: center;
  color: var(--text-dim);
  transition: background 0.12s, color 0.12s;
}

.controls button:hover {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text);
}

.controls .x:hover {
  background: #e81123;
  color: #fff;
}

/* ---------- Conteúdo ---------- */
.stage {
  position: relative;
  z-index: 1;
  flex: 1;
  display: grid;
  place-items: center;
  padding: 10px 24px 18px;
}

.view {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  animation: enter 0.45s cubic-bezier(0.2, 0.8, 0.2, 1);
}

@keyframes enter {
  from {
    opacity: 0;
    transform: translateY(10px) scale(0.98);
  }
}

.emblem {
  position: relative;
  width: 104px;
  height: 104px;
  display: grid;
  place-items: center;
}

.halo {
  position: absolute;
  inset: -18px;
  border-radius: 50%;
  background: conic-gradient(
    from 0deg,
    transparent,
    rgba(183, 254, 0, 0.5),
    transparent 40%,
    rgba(228, 250, 0, 0.35),
    transparent 75%
  );
  filter: blur(14px);
  animation: spin 7s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.logo {
  position: relative;
  width: 96px;
  height: 96px;
  filter: drop-shadow(0 0 22px rgba(183, 254, 0, 0.5));
}

.float {
  animation: float 3.6s ease-in-out infinite;
}

@keyframes float {
  50% {
    transform: translateY(-6px);
  }
}

.wordmark {
  width: 236px;
  margin: 12px 0 6px;
  filter: drop-shadow(0 4px 18px rgba(0, 0, 0, 0.6));
}

.lead {
  margin: 0 0 20px;
  color: var(--text-dim);
  font-size: 15px;
}

.muted {
  color: var(--text-dim);
}

.pulse {
  margin-top: 22px;
  animation: pulse 1.4s ease-in-out infinite;
}

@keyframes pulse {
  50% {
    opacity: 0.45;
  }
}

/* Botão principal com cantoneiras, como o "Junte-se agora" dos banners */
.cta {
  position: relative;
  padding: 7px;
}

.corner {
  position: absolute;
  width: 14px;
  height: 14px;
  border: 0 solid var(--lime);
  opacity: 0.9;
}

.tl { top: 0; left: 0; border-top-width: 2px; border-left-width: 2px; }
.tr { top: 0; right: 0; border-top-width: 2px; border-right-width: 2px; }
.bl { bottom: 0; left: 0; border-bottom-width: 2px; border-left-width: 2px; }
.br { bottom: 0; right: 0; border-bottom-width: 2px; border-right-width: 2px; }

.primary {
  position: relative;
  overflow: hidden;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  width: 310px;
  height: 56px;
  border-radius: 12px;
  background: var(--gradient);
  color: var(--ink);
  font-family: var(--font-display);
  font-weight: 700;
  font-size: 19px;
  letter-spacing: 0.01em;
  box-shadow:
    0 0 0 1px rgba(183, 254, 0, 0.6),
    0 10px 36px rgba(183, 254, 0, 0.35),
    inset 0 1px 0 rgba(255, 255, 255, 0.5);
  transition: transform 0.12s, box-shadow 0.2s, filter 0.2s;
}

.small .primary {
  width: 240px;
  height: 48px;
  font-size: 17px;
}

/* Brilho que atravessa o botão */
.primary::after {
  content: '';
  position: absolute;
  top: 0;
  bottom: 0;
  width: 60px;
  left: -80px;
  background: linear-gradient(100deg, transparent, rgba(255, 255, 255, 0.55), transparent);
  transform: skewX(-20deg);
  animation: shine 3.2s ease-in-out infinite;
}

@keyframes shine {
  0%,
  55% {
    left: -80px;
  }
  100% {
    left: 120%;
  }
}

.primary:hover {
  filter: brightness(1.06);
  box-shadow:
    0 0 0 1px rgba(183, 254, 0, 0.8),
    0 12px 44px rgba(183, 254, 0, 0.5),
    inset 0 1px 0 rgba(255, 255, 255, 0.5);
}

.primary:active {
  transform: translateY(1px) scale(0.99);
}

.secondary {
  margin-top: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-dim);
  text-decoration: underline;
  text-underline-offset: 3px;
}

.secondary:hover {
  color: var(--text);
}

.options {
  display: flex;
  gap: 10px;
  margin-top: 16px;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 34px;
  padding: 0 14px 0 10px;
  border-radius: 999px;
  border: 1px solid var(--line-strong);
  background: rgba(14, 21, 24, 0.75);
  color: var(--text-dim);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: border-color 0.15s, color 0.15s, background 0.15s;
}

.chip input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}

.chip:has(input:focus-visible) {
  outline: 2px solid var(--lime);
  outline-offset: 2px;
}

.chip.on {
  border-color: var(--lime-line);
  color: var(--text);
  background: rgba(183, 254, 0, 0.08);
}

.box {
  width: 18px;
  height: 18px;
  border-radius: 6px;
  border: 1.5px solid var(--line-strong);
  display: grid;
  place-items: center;
  transition: background 0.15s, border-color 0.15s;
}

.chip.on .box {
  background: var(--gradient);
  border-color: transparent;
}

.chip.on .box::after {
  content: '';
  width: 8px;
  height: 4px;
  border: 2px solid var(--ink);
  border-top: 0;
  border-right: 0;
  transform: translateY(-1px) rotate(-45deg);
}

.meta {
  display: flex;
  gap: 8px;
  margin: 16px 0 0;
  font-size: 12px;
  color: var(--text-faint);
}

.meta span + span::before {
  content: '·';
  margin-right: 8px;
}

/* ---------- Progresso ---------- */
.ring-wrap {
  position: relative;
  width: 176px;
  height: 176px;
  display: grid;
  place-items: center;
}

.ring {
  position: absolute;
  inset: 0;
  transform: rotate(-90deg);
  filter: drop-shadow(0 0 14px rgba(183, 254, 0, 0.35));
}

.track {
  fill: rgba(14, 21, 24, 0.7);
  stroke: rgba(183, 254, 0, 0.1);
  stroke-width: 9;
}

.bar {
  fill: none;
  stroke: url(#ring-grad);
  stroke-width: 9;
  stroke-linecap: round;
  transition: stroke-dasharray 0.25s ease-out, stroke 0.3s;
}

.bar.bad {
  stroke: var(--bad);
}

.inner {
  width: 84px;
  height: 84px;
}

.badge {
  position: absolute;
  right: 18px;
  bottom: 18px;
  width: 38px;
  height: 38px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  font-family: var(--font-display);
  font-weight: 700;
  font-size: 22px;
  border: 3px solid var(--bg);
  animation: pop 0.4s cubic-bezier(0.3, 1.6, 0.5, 1);
}

.badge.ok {
  background: var(--gradient);
  color: var(--ink);
  box-shadow: 0 0 20px rgba(183, 254, 0, 0.6);
}

.badge.err {
  background: var(--bad);
  color: #fff;
}

@keyframes pop {
  from {
    transform: scale(0);
  }
}

.percent {
  margin: 14px 0 0;
  font-family: var(--font-display);
  font-weight: 700;
  font-size: 34px;
  line-height: 1;
  background: var(--gradient);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}

.percent small {
  font-size: 18px;
  margin-left: 2px;
}

h1 {
  margin: 8px 0 6px;
  font-family: var(--font-display);
  font-weight: 700;
  font-size: 20px;
}

h1.big {
  margin-top: 18px;
  font-size: 28px;
}

.detail {
  margin: 0;
  min-height: 20px;
  font-size: 14px;
  max-width: 460px;
}

.detail.error {
  color: #ffb0b0;
}

.view .cta.small {
  margin-top: 16px;
}

.steps {
  display: flex;
  gap: 6px;
  list-style: none;
  margin: 22px 0 0;
  padding: 0;
}

.steps li {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  height: 28px;
  padding: 0 12px;
  border-radius: 999px;
  font-family: var(--font-pixel);
  font-size: 10px;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--text-faint);
  background: rgba(14, 21, 24, 0.7);
  border: 1px solid var(--line);
}

.steps .dot {
  width: 7px;
  height: 7px;
  border-radius: 2px;
  background: var(--line-strong);
}

.steps li.active {
  color: var(--lime);
  border-color: var(--lime-line);
}

.steps li.active .dot {
  background: var(--lime);
  box-shadow: 0 0 8px var(--lime);
  animation: pulse 1s ease-in-out infinite;
}

.steps li.done {
  color: var(--text-dim);
}

.steps li.done .dot {
  background: var(--lime);
}

.steps li.bad {
  color: var(--bad);
  border-color: rgba(255, 100, 100, 0.35);
}

.steps li.bad .dot {
  background: var(--bad);
}
</style>

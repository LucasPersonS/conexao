<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import { api, errorMessage, type GameSettings, type JavaInfo, type LauncherPrefs, type SettingsView } from '../lib/api'
import { prefs } from '../lib/prefs'

const DEFAULT_GAME: Omit<GameSettings, 'memory_mb'> = {
  fullscreen: false,
  resolution_width: 854,
  resolution_height: 480,
  jvm_args: '',
  env_vars: '',
  pre_launch: '',
  wrapper: '',
  post_exit: '',
  max_downloads: 10,
}
const DEFAULT_LAUNCHER: LauncherPrefs = {
  memory_auto: true,
  jvm_optimized: true,
  minimize_on_play: true,
  discord_presence: true,
  notifications: true,
  java_path: null,
}
const RESOLUTIONS: [number, number][] = [
  [854, 480],
  [1280, 720],
  [1600, 900],
  [1920, 1080],
]

const data = ref<SettingsView | null>(null)
const game = ref<GameSettings>({ memory_mb: 4096, ...DEFAULT_GAME })
const launcher = ref<LauncherPrefs>({ ...DEFAULT_LAUNCHER })
const snapshot = ref('')
const saving = ref(false)
const saved = ref(false)
const error = ref<string | null>(null)
const version = ref('')
const showFlags = ref(false)

const snap = () => JSON.stringify({ g: game.value, l: launcher.value })

onMounted(async () => {
  getVersion().then((v) => (version.value = v)).catch(() => {})
  try {
    const view = await api.getSettings()
    data.value = view
    game.value = {
      memory_mb: view.memory_mb,
      fullscreen: view.fullscreen,
      resolution_width: view.resolution_width,
      resolution_height: view.resolution_height,
      jvm_args: view.jvm_args,
      env_vars: view.env_vars,
      pre_launch: view.pre_launch,
      wrapper: view.wrapper,
      post_exit: view.post_exit,
      max_downloads: view.max_downloads,
    }
    launcher.value = { ...DEFAULT_LAUNCHER, ...view.launcher }
    javaMode.value = launcher.value.java_path ? 'custom' : 'auto'
    snapshot.value = snap()
  } catch (e) {
    error.value = errorMessage(e)
  }
})

const dirty = computed(() => !!data.value && snap() !== snapshot.value)

/* ---------- Memória ---------- */
const systemMb = computed(() => data.value?.system_memory_mb ?? 8192)
/** Deixa ~2 GB para o Windows; no máximo 32 GB. */
const maxMemory = computed(() => Math.max(2048, Math.min(32768, Math.floor((systemMb.value - 2048) / 256) * 256)))
/** Teto do modo automático neste computador (mesma conta do launcher). */
const autoCap = computed(() => {
  const s = systemMb.value
  return Math.max(Math.min(s / 2, 2048), Math.min(8192, s - 3072, Math.floor(s * 0.6)))
})
const gb = (mb: number) => (mb / 1024).toLocaleString('pt-BR', { maximumFractionDigits: 2 })
const presets = computed(() => [2048, 3072, 4096, 6144, 8192, 12288].filter((mb) => mb <= maxMemory.value))
const memoryGb = computed({
  get: () => +(game.value.memory_mb / 1024).toFixed(2),
  set: (v: number) => {
    if (!Number.isFinite(v)) return
    const mb = Math.round((v * 1024) / 256) * 256
    game.value.memory_mb = Math.min(maxMemory.value, Math.max(1024, mb))
  },
})
const memoryWarning = computed(() => {
  const mb = game.value.memory_mb
  if (mb > systemMb.value * 0.75) return 'Muito perto da RAM total: o Windows pode ficar lento ou o jogo fechar sozinho.'
  if (mb > 12288) return 'Acima de 12 GB o coletor de memória costuma travar mais, não menos.'
  if (mb < 2048) return 'Com menos de 2 GB a maioria dos modpacks nem abre.'
  return null
})
const sliderFill = computed(() => {
  const pct = ((game.value.memory_mb - 1024) / (maxMemory.value - 1024)) * 100
  return `${Math.max(0, Math.min(100, pct))}%`
})

/* ---------- Java ---------- */
const javaMode = ref<'auto' | 'custom'>('auto')
const javas = ref<JavaInfo[] | null>(null)
const javaLoading = ref(false)
const javaTest = ref<{ ok: boolean; text: string } | null>(null)

function setJavaMode(mode: 'auto' | 'custom') {
  javaMode.value = mode
  javaTest.value = null
  if (mode === 'auto') launcher.value.java_path = null
  else {
    launcher.value.java_path = launcher.value.java_path ?? ''
    if (!javas.value) findJava()
  }
}

async function findJava() {
  javaLoading.value = true
  try {
    javas.value = await api.listJava()
  } catch (e) {
    javaTest.value = { ok: false, text: errorMessage(e) }
  } finally {
    javaLoading.value = false
  }
}

async function testJava() {
  const path = launcher.value.java_path?.trim()
  if (!path) return
  javaTest.value = { ok: true, text: 'Testando…' }
  try {
    const j = await api.testJava(path)
    javaTest.value = { ok: true, text: `Funciona: Java ${j.version} (${j.architecture})` }
  } catch (e) {
    javaTest.value = { ok: false, text: errorMessage(e) }
  }
}

function pickJava(j: JavaInfo) {
  launcher.value.java_path = j.path
  javaTest.value = { ok: true, text: `Java ${j.version} (${j.architecture})` }
}

/* ---------- Argumentos ---------- */
const finalArgs = computed(() => {
  const mem = launcher.value.memory_auto ? '-Xmx<automático>' : `-Xmx${game.value.memory_mb}M`
  const custom = game.value.jvm_args.trim()
  const otherGc = /-XX:\+Use(?!G1GC)\w*GC/.test(custom)
  const flags = launcher.value.jvm_optimized && !otherGc ? (data.value?.optimized_flags ?? []) : []
  return [mem, ...flags, custom].filter(Boolean).join(' ')
})

/* ---------- Ações ---------- */
function restoreDefaults() {
  game.value = { ...DEFAULT_GAME, memory_mb: data.value?.recommended_memory_mb ?? 4096 }
  launcher.value = { ...DEFAULT_LAUNCHER }
  javaMode.value = 'auto'
  javaTest.value = null
}

function discard() {
  const { g, l } = JSON.parse(snapshot.value) as { g: GameSettings; l: LauncherPrefs }
  game.value = g
  launcher.value = l
  javaMode.value = l.java_path ? 'custom' : 'auto'
  javaTest.value = null
  error.value = null
}

async function save() {
  saving.value = true
  error.value = null
  try {
    if (javaMode.value === 'custom' && !launcher.value.java_path?.trim()) {
      launcher.value.java_path = null
      javaMode.value = 'auto'
    }
    await api.setSettings(game.value, launcher.value)
    snapshot.value = snap()
    saved.value = true
    window.setTimeout(() => (saved.value = false), 2500)
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    saving.value = false
  }
}

async function openFolder(kind: 'data' | 'logs') {
  try {
    await api.openLauncherFolder(kind)
  } catch (e) {
    error.value = errorMessage(e)
  }
}
</script>

<template>
  <div class="settings">
    <header class="head">
      <h2>Configurações</h2>
      <button class="btn btn-ghost" :disabled="!data" @click="restoreDefaults">Restaurar padrões</button>
    </header>

    <p v-if="error && !data" class="banner err">{{ error }}</p>

    <!-- MEMÓRIA -->
    <section class="card">
      <h3>Memória (RAM)</h3>
      <div class="row">
        <div class="label">
          <strong>Como escolher a memória</strong>
          <span v-if="launcher.memory_auto">
            O Conexão escolhe pela quantidade de mods de cada servidor, deixando espaço para o Windows e o Discord.
            <template v-if="data">Neste computador, até {{ gb(autoCap) }} GB.</template>
          </span>
          <span v-else>Você define quanto de RAM o Minecraft pode usar, igual para todos os servidores.</span>
        </div>
        <div class="seg" role="radiogroup">
          <button :class="{ on: launcher.memory_auto }" role="radio" :aria-checked="launcher.memory_auto" :disabled="!data" @click="launcher.memory_auto = true">Automática</button>
          <button :class="{ on: !launcher.memory_auto }" role="radio" :aria-checked="!launcher.memory_auto" :disabled="!data" @click="launcher.memory_auto = false">Manual</button>
        </div>
      </div>

      <div class="row column" :class="{ muted: launcher.memory_auto }">
        <div class="mem-top">
          <div class="label">
            <strong>Memória máxima</strong>
            <span v-if="data">
              Seu computador tem {{ gb(systemMb) }} GB. Recomendado: {{ gb(data.recommended_memory_mb) }} GB.
              <template v-if="launcher.memory_auto"> Mude para “Manual” para usar este valor.</template>
            </span>
          </div>
          <div class="mem-value">
            <input v-model.lazy.number="memoryGb" type="number" min="1" :max="maxMemory / 1024" step="0.25" :disabled="!data || launcher.memory_auto" aria-label="Memória em GB" />
            <span>GB</span>
          </div>
        </div>
        <input
          v-model.number="game.memory_mb"
          class="slider"
          type="range"
          min="1024"
          :max="maxMemory"
          step="256"
          :style="{ '--fill': sliderFill }"
          :disabled="!data || launcher.memory_auto"
          aria-label="Memória máxima"
        />
        <div class="scale">
          <span>1 GB</span>
          <span>{{ gb(maxMemory) }} GB</span>
        </div>
        <div class="presets">
          <button
            v-for="mb in presets"
            :key="mb"
            class="preset"
            :class="{ on: game.memory_mb === mb }"
            :disabled="!data || launcher.memory_auto"
            @click="game.memory_mb = mb"
          >
            {{ gb(mb) }} GB
          </button>
        </div>
        <p v-if="!launcher.memory_auto && memoryWarning" class="hint warn">{{ memoryWarning }}</p>
      </div>
    </section>

    <!-- JAVA -->
    <section class="card">
      <h3>Java</h3>
      <div class="row">
        <div class="label">
          <strong>Versão do Java</strong>
          <span v-if="javaMode === 'auto'">O Conexão baixa e usa o Java certo para cada versão do Minecraft.</span>
          <span v-else>Use um Java instalado por você. Se for a versão errada para o modpack, o jogo não abre.</span>
        </div>
        <div class="seg" role="radiogroup">
          <button :class="{ on: javaMode === 'auto' }" :disabled="!data" @click="setJavaMode('auto')">Automático</button>
          <button :class="{ on: javaMode === 'custom' }" :disabled="!data" @click="setJavaMode('custom')">Escolher</button>
        </div>
      </div>
      <div v-if="javaMode === 'custom'" class="row column">
        <div class="inline">
          <input
            v-model="launcher.java_path"
            class="field mono grow"
            placeholder="C:\Program Files\Java\jdk-21\bin\javaw.exe"
            spellcheck="false"
            @input="javaTest = null"
          />
          <button class="btn" :disabled="!launcher.java_path?.trim()" @click="testJava">Testar</button>
        </div>
        <p v-if="javaTest" class="hint" :class="javaTest.ok ? 'ok' : 'err'">{{ javaTest.text }}</p>
        <div class="java-list">
          <div class="java-head">
            <span>Encontrados neste computador</span>
            <button class="link" :disabled="javaLoading" @click="findJava">{{ javaLoading ? 'Procurando…' : 'Procurar de novo' }}</button>
          </div>
          <p v-if="javas && !javas.length" class="hint">Nenhum Java encontrado. Cole o caminho do <code>javaw.exe</code> acima.</p>
          <button
            v-for="j in javas ?? []"
            :key="j.path"
            class="java-item"
            :class="{ on: launcher.java_path === j.path }"
            @click="pickJava(j)"
          >
            <strong>Java {{ j.major }}</strong>
            <span class="mono">{{ j.path }}</span>
            <small>{{ j.version }} · {{ j.architecture }}</small>
          </button>
        </div>
      </div>

      <div class="row">
        <div class="label">
          <strong>Otimizações do Java</strong>
          <span>
            Coletor de memória ajustado para modpacks: menos travadas ao andar pelo mapa e carregar texturas.
            <button class="link" @click="showFlags = !showFlags">{{ showFlags ? 'Esconder flags' : 'Ver flags' }}</button>
          </span>
          <code v-if="showFlags && data" class="flags">{{ data.optimized_flags.join(' ') }}</code>
        </div>
        <button class="switch" :class="{ on: launcher.jvm_optimized }" role="switch" :aria-checked="launcher.jvm_optimized" :disabled="!data" @click="launcher.jvm_optimized = !launcher.jvm_optimized">
          <span />
        </button>
      </div>

      <div class="row column">
        <div class="label">
          <strong>Argumentos personalizados da JVM</strong>
          <span>
            Separados por espaço; use aspas para valores com espaço. Vêm depois das otimizações, então prevalecem sobre
            elas. A memória máxima (<code>-Xmx</code>) é definida acima.
          </span>
        </div>
        <textarea
          v-model="game.jvm_args"
          class="field mono"
          rows="3"
          spellcheck="false"
          placeholder="-Xms2G -XX:+UseZGC -Dfile.encoding=UTF-8"
          :disabled="!data"
        />
        <div class="preview">
          <span>Como o Java vai abrir</span>
          <code>java {{ finalArgs }} …</code>
        </div>
      </div>
    </section>

    <!-- JANELA -->
    <section class="card">
      <h3>Janela do jogo</h3>
      <div class="row">
        <div class="label">
          <strong>Abrir em tela cheia</strong>
          <span>O Minecraft já abre ocupando a tela inteira.</span>
        </div>
        <button class="switch" :class="{ on: game.fullscreen }" role="switch" :aria-checked="game.fullscreen" :disabled="!data" @click="game.fullscreen = !game.fullscreen">
          <span />
        </button>
      </div>
      <div class="row" :class="{ muted: game.fullscreen }">
        <div class="label">
          <strong>Tamanho da janela</strong>
          <span>Largura × altura quando o jogo não está em tela cheia.</span>
          <div class="presets">
            <button
              v-for="[w, h] in RESOLUTIONS"
              :key="w"
              class="preset"
              :class="{ on: game.resolution_width === w && game.resolution_height === h }"
              :disabled="!data || game.fullscreen"
              @click="game.resolution_width = w; game.resolution_height = h"
            >
              {{ w }}×{{ h }}
            </button>
          </div>
        </div>
        <div class="res">
          <input v-model.number="game.resolution_width" class="field num" type="number" min="320" max="7680" :disabled="!data || game.fullscreen" aria-label="Largura" />
          <span>×</span>
          <input v-model.number="game.resolution_height" class="field num" type="number" min="240" max="4320" :disabled="!data || game.fullscreen" aria-label="Altura" />
        </div>
      </div>
    </section>

    <!-- LAUNCHER -->
    <section class="card">
      <h3>Launcher</h3>
      <div class="row">
        <div class="label">
          <strong>Minimizar o Conexão ao jogar</strong>
          <span>O launcher sai do caminho e pausa o que roda em segundo plano; volta quando o jogo fecha.</span>
        </div>
        <button class="switch" :class="{ on: launcher.minimize_on_play }" role="switch" :aria-checked="launcher.minimize_on_play" :disabled="!data" @click="launcher.minimize_on_play = !launcher.minimize_on_play">
          <span />
        </button>
      </div>
      <div v-if="data?.discord_available" class="row">
        <div class="label">
          <strong>Mostrar no Discord onde estou jogando</strong>
          <span>Seus amigos veem o servidor e um botão “Jogar também” que abre o Conexão direto nele.</span>
        </div>
        <button class="switch" :class="{ on: launcher.discord_presence }" role="switch" :aria-checked="launcher.discord_presence" @click="launcher.discord_presence = !launcher.discord_presence">
          <span />
        </button>
      </div>
      <div class="row">
        <div class="label">
          <strong>Notificações</strong>
          <span>Avisa quando abrir vaga num servidor cheio e 10 minutos antes dos eventos que você marcou.</span>
        </div>
        <button class="switch" :class="{ on: launcher.notifications }" role="switch" :aria-checked="launcher.notifications" :disabled="!data" @click="launcher.notifications = !launcher.notifications">
          <span />
        </button>
      </div>
      <div class="row">
        <div class="label">
          <strong>Downloads simultâneos</strong>
          <span>Arquivos baixados ao mesmo tempo ao instalar um modpack. Diminua se a internet cair durante a instalação.</span>
        </div>
        <input v-model.number="game.max_downloads" class="field num" type="number" min="1" max="50" :disabled="!data" aria-label="Downloads simultâneos" />
      </div>
    </section>

    <!-- AVANÇADO -->
    <details class="card advanced">
      <summary><h3>Avançado</h3><span>Variáveis de ambiente e comandos de inicialização</span></summary>
      <div class="row column">
        <div class="label">
          <strong>Variáveis de ambiente</strong>
          <span>Uma por linha, no formato <code>NOME=valor</code>. Linhas com <code>#</code> são ignoradas.</span>
        </div>
        <textarea v-model="game.env_vars" class="field mono" rows="3" spellcheck="false" placeholder="__GL_THREADED_OPTIMIZATIONS=0" :disabled="!data" />
      </div>
      <div class="row column">
        <div class="label">
          <strong>Antes de abrir o jogo</strong>
          <span>Comando rodado antes do Minecraft iniciar.</span>
        </div>
        <input v-model="game.pre_launch" class="field mono" spellcheck="false" :disabled="!data" />
      </div>
      <div class="row column">
        <div class="label">
          <strong>Wrapper</strong>
          <span>Programa que “embrulha” o Java, por exemplo <code>prime-run</code> ou <code>gamemoderun</code> no Linux.</span>
        </div>
        <input v-model="game.wrapper" class="field mono" spellcheck="false" :disabled="!data" />
      </div>
      <div class="row column">
        <div class="label">
          <strong>Depois que o jogo fechar</strong>
          <span>Comando rodado quando o Minecraft é encerrado.</span>
        </div>
        <input v-model="game.post_exit" class="field mono" spellcheck="false" :disabled="!data" />
      </div>
    </details>

    <!-- PASTAS E SOBRE -->
    <section class="card">
      <h3>Pastas e suporte</h3>
      <div class="row">
        <div class="label">
          <strong>Pastas do Conexão</strong>
          <span>Instâncias, Java baixado e logs. Útil para mandar um log quando algo dá errado.</span>
        </div>
        <div class="inline">
          <button class="btn" @click="openFolder('data')">Dados</button>
          <button class="btn" @click="openFolder('logs')">Logs</button>
        </div>
      </div>
      <div class="row">
        <div class="label">
          <strong>Tela de boas-vindas</strong>
          <span>Mostra de novo o passo a passo de como jogar.</span>
        </div>
        <button class="btn" @click="prefs.onboarded = false">Ver de novo</button>
      </div>
      <div class="row">
        <div class="label">
          <strong>Versão</strong>
          <span>Conexão {{ version || '…' }}</span>
        </div>
      </div>
    </section>

    <div class="savebar" :class="{ show: dirty || saved || (error && data) }">
      <span v-if="error && data" class="err">{{ error }}</span>
      <span v-else-if="saved && !dirty" class="ok">Salvo. Vale a partir da próxima vez que o jogo abrir.</span>
      <span v-else>Você tem alterações não salvas.</span>
      <div class="inline">
        <button v-if="dirty" class="btn" :disabled="saving" @click="discard">Descartar</button>
        <button class="btn btn-primary" :disabled="!dirty || saving" @click="save">{{ saving ? 'Salvando…' : 'Salvar' }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings {
  max-width: 780px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding-bottom: 88px;
}

.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

h2 {
  margin: 0;
  font-size: 30px;
}

.card {
  padding: 6px 20px;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
}

h3 {
  margin: 16px 0 4px;
  font-family: var(--font-pixel);
  font-size: 11px;
  font-weight: 400;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--lime);
}

.row {
  display: flex;
  align-items: center;
  gap: 24px;
  padding: 16px 0;
  border-bottom: 1px solid var(--line);
}

.row.column {
  flex-direction: column;
  align-items: stretch;
  gap: 10px;
}

.row:last-child {
  border-bottom: none;
}

.row.muted .label strong,
.row.muted .mem-value,
.row.muted .scale {
  opacity: 0.55;
}

.label {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.label span {
  font-size: 13px;
  line-height: 1.45;
  color: var(--text-dim);
}

code,
.mono {
  font-family: ui-monospace, 'Cascadia Code', Consolas, monospace;
  font-size: 12.5px;
}

.label code {
  padding: 1px 5px;
  border-radius: 5px;
  background: var(--bg-input);
  color: var(--text);
}

/* Botões */
.btn-ghost {
  background: transparent;
}

.link {
  padding: 0;
  margin-left: 4px;
  background: none;
  border: none;
  color: var(--lime);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.link:hover:not(:disabled) {
  text-decoration: underline;
}

.seg {
  display: inline-flex;
  padding: 3px;
  border-radius: 12px;
  background: var(--bg-input);
  border: 1px solid var(--line);
  flex-shrink: 0;
}

.seg button {
  height: 32px;
  padding: 0 14px;
  border-radius: 9px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-dim);
  background: transparent;
  transition: background 0.15s, color 0.15s;
}

.seg button.on {
  background: var(--lime);
  color: var(--ink);
}

.presets {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 4px;
}

.preset {
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 600;
  background: var(--bg-input);
  border: 1px solid var(--line);
  color: var(--text-dim);
}

.preset:hover:not(:disabled) {
  border-color: var(--line-strong);
  color: var(--text);
}

.preset.on {
  border-color: var(--lime-line);
  background: var(--lime-soft);
  color: var(--lime);
}

.preset:disabled {
  opacity: 0.45;
}

/* Campos */
.field {
  width: 100%;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--bg-input);
  border: 1px solid var(--line);
  color: var(--text);
  outline: none;
  transition: border-color 0.15s, box-shadow 0.15s;
}

textarea.field {
  resize: vertical;
  line-height: 1.5;
}

.field:focus {
  border-color: var(--lime-line);
  box-shadow: 0 0 0 3px var(--lime-soft);
}

.field:disabled {
  opacity: 0.5;
}

.field.num {
  width: 90px;
  text-align: center;
}

.inline {
  display: flex;
  align-items: center;
  gap: 8px;
}

.grow {
  flex: 1;
}

/* Memória */
.mem-top {
  display: flex;
  align-items: center;
  gap: 24px;
}

.mem-value {
  display: flex;
  align-items: baseline;
  gap: 6px;
}

.mem-value input {
  width: 92px;
  padding: 4px 6px;
  border-radius: 10px;
  background: var(--bg-input);
  border: 1px solid var(--line);
  font-family: var(--font-display);
  font-size: 24px;
  font-weight: 600;
  color: var(--lime);
  text-align: right;
  outline: none;
}

.mem-value input:focus {
  border-color: var(--lime-line);
}

.mem-value span {
  font-family: var(--font-display);
  font-size: 18px;
  color: var(--lime);
}

.slider {
  width: 100%;
  height: 6px;
  appearance: none;
  border-radius: 999px;
  background: linear-gradient(to right, var(--lime) var(--fill), var(--line-strong) var(--fill));
  outline: none;
}

.slider::-webkit-slider-thumb {
  appearance: none;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: var(--white);
  border: 3px solid var(--lime);
  box-shadow: 0 0 12px rgba(183, 254, 0, 0.4);
  cursor: pointer;
}

.slider:disabled {
  opacity: 0.4;
}

.slider:disabled::-webkit-slider-thumb {
  cursor: default;
}

.scale {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--text-faint);
}

.hint {
  margin: 0;
  font-size: 13px;
  color: var(--text-dim);
}

.hint.warn {
  padding: 8px 12px;
  border-radius: 10px;
  background: var(--warn-bg);
  color: var(--warn);
}

.hint.ok,
.ok {
  color: var(--good);
}

.hint.err,
.err {
  color: var(--bad);
}

/* Java */
.java-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.java-head {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--text-faint);
}

.java-item {
  display: grid;
  grid-template-columns: 70px 1fr auto;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 10px;
  text-align: left;
  background: var(--bg-input);
  border: 1px solid var(--line);
  color: var(--text);
}

.java-item span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-dim);
}

.java-item small {
  color: var(--text-faint);
}

.java-item:hover {
  border-color: var(--line-strong);
}

.java-item.on {
  border-color: var(--lime-line);
  background: var(--lime-soft);
}

.flags {
  display: block;
  margin-top: 6px;
  padding: 8px 10px;
  border-radius: 8px;
  background: var(--bg-input);
  color: var(--text-dim);
  word-break: break-all;
}

.preview {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--text-faint);
}

.preview code {
  padding: 8px 10px;
  border-radius: 8px;
  background: var(--bg-input);
  color: var(--text-dim);
  word-break: break-all;
}

/* Janela */
.res {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-dim);
}

/* Avançado */
.advanced summary {
  display: flex;
  align-items: baseline;
  gap: 12px;
  padding-bottom: 12px;
  cursor: pointer;
  list-style: none;
}

.advanced summary::-webkit-details-marker {
  display: none;
}

.advanced summary h3 {
  margin-bottom: 0;
}

.advanced summary h3::before {
  content: '▸ ';
}

.advanced[open] summary h3::before {
  content: '▾ ';
}

.advanced summary span {
  font-size: 13px;
  color: var(--text-faint);
}

/* Switch */
.switch {
  width: 46px;
  height: 26px;
  padding: 3px;
  flex-shrink: 0;
  border-radius: 999px;
  background: var(--line-strong);
  transition: background 0.15s, box-shadow 0.15s;
}

.switch span {
  display: block;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--white);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
  transition: transform 0.15s, background 0.15s;
}

.switch.on {
  background: var(--lime);
  box-shadow: 0 0 14px rgba(183, 254, 0, 0.35);
}

.switch.on span {
  background: var(--ink);
  transform: translateX(20px);
}

/* Barra de salvar */
.savebar {
  position: sticky;
  bottom: 16px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 12px 12px 18px;
  border-radius: var(--radius);
  background: var(--bg-raised);
  border: 1px solid var(--lime-line);
  box-shadow: var(--shadow-pop);
  font-size: 13px;
  color: var(--text-dim);
  opacity: 0;
  transform: translateY(12px);
  pointer-events: none;
  transition: opacity 0.18s, transform 0.18s;
}

.savebar.show {
  opacity: 1;
  transform: none;
  pointer-events: auto;
}

.banner {
  margin: 0;
  padding: 12px 16px;
  border-radius: 12px;
  background: rgba(255, 100, 100, 0.1);
}
</style>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import { api, errorMessage, type LauncherPrefs, type SettingsView } from '../lib/api'
import { prefs } from '../lib/prefs'

const data = ref<SettingsView | null>(null)
const memory = ref(4096)
const fullscreen = ref(false)
const launcher = ref<LauncherPrefs>({
  memory_auto: true,
  jvm_optimized: true,
  minimize_on_play: true,
  discord_presence: true,
  notifications: true,
})
const saving = ref(false)
const saved = ref(false)
const error = ref<string | null>(null)
const version = ref('')

onMounted(async () => {
  getVersion().then((v) => (version.value = v)).catch(() => {})
  try {
    data.value = await api.getSettings()
    memory.value = data.value.memory_mb
    fullscreen.value = data.value.fullscreen
    launcher.value = { ...data.value.launcher }
  } catch (e) {
    error.value = errorMessage(e)
  }
})

/** Deixa ~2 GB para o Windows; no máximo 16 GB (mais que isso só piora o GC). */
const maxMemory = computed(() => {
  const system = data.value?.system_memory_mb ?? 8192
  return Math.max(2048, Math.min(16384, Math.floor((system - 2048) / 512) * 512))
})
const gb = (mb: number) => (mb / 1024).toLocaleString('pt-BR', { maximumFractionDigits: 1 })
const KEYS = ['memory_auto', 'jvm_optimized', 'minimize_on_play', 'discord_presence', 'notifications'] as const
const dirty = computed(
  () =>
    !!data.value &&
    (memory.value !== data.value.memory_mb ||
      fullscreen.value !== data.value.fullscreen ||
      KEYS.some((k) => launcher.value[k] !== data.value?.launcher[k])),
)
/** Teto do modo automático neste computador (mesma conta do launcher). */
const autoCap = computed(() => {
  const system = data.value?.system_memory_mb ?? 8192
  return Math.max(Math.min(system / 2, 2048), Math.min(8192, system - 3072, Math.floor(system * 0.6)))
})
const toggle = (key: (typeof KEYS)[number]) => (launcher.value[key] = !launcher.value[key])

async function save() {
  saving.value = true
  error.value = null
  try {
    await api.setSettings({ memory_mb: memory.value, fullscreen: fullscreen.value }, launcher.value)
    if (data.value) {
      data.value = { ...data.value, memory_mb: memory.value, fullscreen: fullscreen.value, launcher: { ...launcher.value } }
    }
    saved.value = true
    window.setTimeout(() => (saved.value = false), 2000)
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="settings">
    <h2>Configurações</h2>

    <section class="card">
      <h3>Jogo e desempenho</h3>
      <div class="row">
        <div class="label">
          <strong>Memória automática</strong>
          <span>
            O Conexão escolhe a memória pelo tamanho do modpack de cada servidor, deixando espaço para o Windows e o
            Discord.<template v-if="data"> Neste computador, até {{ gb(autoCap) }} GB.</template>
          </span>
        </div>
        <button class="switch" :class="{ on: launcher.memory_auto }" role="switch" :aria-checked="launcher.memory_auto" :disabled="!data" @click="toggle('memory_auto')">
          <span />
        </button>
      </div>
      <div v-if="!launcher.memory_auto" class="row">
        <div class="label">
          <strong>Memória do Minecraft</strong>
          <span>
            Quanto da RAM o jogo pode usar. Mais do que o modpack precisa não deixa o jogo mais rápido.
            <template v-if="data">
              Seu computador tem {{ gb(data.system_memory_mb) }} GB; recomendado: {{ gb(data.recommended_memory_mb) }} GB.
            </template>
          </span>
        </div>
        <div class="memory">
          <strong>{{ gb(memory) }} GB</strong>
          <input v-model.number="memory" type="range" min="2048" :max="maxMemory" step="512" :disabled="!data" />
        </div>
      </div>
      <div class="row">
        <div class="label">
          <strong>Otimizações do Java</strong>
          <span>Coletor de memória ajustado para modpacks: menos travadas ao andar pelo mapa e carregar texturas.</span>
        </div>
        <button class="switch" :class="{ on: launcher.jvm_optimized }" role="switch" :aria-checked="launcher.jvm_optimized" :disabled="!data" @click="toggle('jvm_optimized')">
          <span />
        </button>
      </div>
      <div class="row">
        <div class="label">
          <strong>Minimizar o Conexão ao jogar</strong>
          <span>O launcher sai do caminho e pausa tudo que roda em segundo plano enquanto o jogo está aberto.</span>
        </div>
        <button class="switch" :class="{ on: launcher.minimize_on_play }" role="switch" :aria-checked="launcher.minimize_on_play" :disabled="!data" @click="toggle('minimize_on_play')">
          <span />
        </button>
      </div>
      <div class="row">
        <div class="label">
          <strong>Abrir em tela cheia</strong>
          <span>O Minecraft já abre ocupando a tela inteira.</span>
        </div>
        <button class="switch" :class="{ on: fullscreen }" role="switch" :aria-checked="fullscreen" :disabled="!data" @click="fullscreen = !fullscreen">
          <span />
        </button>
      </div>
    </section>

    <section class="card">
      <h3>Discord e avisos</h3>
      <div v-if="data?.discord_available" class="row">
        <div class="label">
          <strong>Mostrar no Discord onde estou jogando</strong>
          <span>Seus amigos veem o servidor e um botão "Jogar também" que abre o Conexão direto nele.</span>
        </div>
        <button class="switch" :class="{ on: launcher.discord_presence }" role="switch" :aria-checked="launcher.discord_presence" @click="toggle('discord_presence')">
          <span />
        </button>
      </div>
      <div class="row">
        <div class="label">
          <strong>Notificações</strong>
          <span>Avisa quando abrir vaga num servidor cheio e 10 minutos antes dos eventos que você marcou.</span>
        </div>
        <button class="switch" :class="{ on: launcher.notifications }" role="switch" :aria-checked="launcher.notifications" :disabled="!data" @click="toggle('notifications')">
          <span />
        </button>
      </div>
      <div class="foot">
        <span v-if="error" class="err">{{ error }}</span>
        <span v-else-if="saved" class="ok">Salvo. Vale a partir da próxima vez que o jogo abrir.</span>
        <button class="btn btn-primary" :disabled="!dirty || saving" @click="save">
          {{ saving ? 'Salvando…' : 'Salvar' }}
        </button>
      </div>
    </section>

    <section class="card">
      <h3>Launcher</h3>
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
  </div>
</template>

<style scoped>
.settings {
  max-width: 760px;
  display: flex;
  flex-direction: column;
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

.row:last-child {
  border-bottom: none;
}

.label {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.label span {
  font-size: 13px;
  line-height: 1.45;
  color: var(--text-dim);
}

.memory {
  width: 260px;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 6px;
}

.memory strong {
  font-family: var(--font-display);
  font-size: 22px;
  color: var(--lime);
}

.memory input {
  width: 100%;
  accent-color: var(--lime);
}

.switch {
  width: 46px;
  height: 26px;
  padding: 3px;
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
}

.switch.on span {
  transform: translateX(20px);
}

.foot {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 14px;
  padding: 4px 0 16px;
  font-size: 13px;
}

.err {
  color: var(--bad);
}

.ok {
  color: var(--good);
}
</style>

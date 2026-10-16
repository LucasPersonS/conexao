<script setup lang="ts">
/** Vínculo do Conexão Companion: o link mostrado no console do servidor abre aqui. */
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { api, LOADER_NAMES, loadSession, message, session, type LinkView } from '../lib/api'

const props = defineProps<{ code?: string }>()
const router = useRouter()
const typed = ref(props.code ?? '')
const link = ref<LinkView | null>(null)
const error = ref<string | null>(null)
const busy = ref(false)
const target = ref('')

const owned = computed(() => session.memberships.filter((m) => m.role === 'owner' || session.user?.is_staff))

async function load(code: string) {
  error.value = null
  link.value = null
  try {
    link.value = await api.companionLink(code.trim().toUpperCase())
    target.value = link.value.linked_slug ?? owned.value[0]?.slug ?? ''
  } catch (e) {
    error.value = (e as { status?: number }).status === 404 ? 'Código não encontrado ou expirado. Reinicie o servidor para gerar outro.' : message(e)
  }
}
onMounted(() => props.code && load(props.code))
watch(
  () => props.code,
  (code) => code && load(code),
)

async function confirm() {
  if (!link.value || !target.value) return
  busy.value = true
  error.value = null
  try {
    await api.confirmLink(link.value.code, target.value)
    await loadSession()
    router.push(`/s/${target.value}?tab=servidor`)
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <h1>Vincular servidor</h1>
  <p class="muted lead">Confirme o servidor que apareceu no console com o Conexão Companion.</p>

  <section v-if="!props.code" class="card ask">
    <label class="field">
      Código mostrado no console
      <input v-model="typed" class="input code" placeholder="K7Q2-9XMA" maxlength="9" @keydown.enter="router.push(`/link/${typed.trim().toUpperCase()}`)" />
    </label>
    <button class="btn btn-primary" :disabled="!typed.trim()" @click="router.push(`/link/${typed.trim().toUpperCase()}`)">Continuar</button>
  </section>

  <section v-if="link" class="card">
    <div class="server">
      <div>
        <span class="label">Código</span>
        <b class="mono">{{ link.code }}</b>
      </div>
      <div>
        <span class="label">Servidor</span>
        <b>{{ link.info.loader ? LOADER_NAMES[link.info.loader] : '—' }} {{ link.info.loader_version }}</b>
        <span class="muted">Minecraft {{ link.info.minecraft }} · {{ link.info.max_players }} vagas</span>
      </div>
      <div>
        <span class="label">MOTD</span>
        <b>{{ link.info.motd || '—' }}</b>
      </div>
    </div>
    <p v-if="!link.info.online_mode" class="warn-text">
      Atenção: o servidor está com <code>online-mode=false</code>. Depois de vincular, ligue
      <b>Aceitar contas não originais</b> nas configurações do servidor: sem isso, qualquer um entra com o nick de
      outra pessoa. Se o servidor é só para contas originais, ative o <code>online-mode</code>.
    </p>

    <p v-if="link.linked_slug" class="ok">Este servidor já está vinculado a <b>{{ link.linked_slug }}</b>.</p>

    <template v-else>
      <div v-if="owned.length" class="choose">
        <label class="field">
          Vincular a qual servidor do painel?
          <select v-model="target" class="input">
            <option v-for="m in owned" :key="m.slug" :value="m.slug">{{ m.name }} ({{ m.slug }})</option>
          </select>
        </label>
        <button class="btn btn-primary" :disabled="busy || !target" @click="confirm">
          {{ busy ? 'Vinculando…' : 'Vincular' }}
        </button>
      </div>
      <div class="new">
        <span class="muted">{{ owned.length ? 'Ou é um servidor novo?' : 'Ainda não cadastrou este servidor?' }}</span>
        <RouterLink :to="`/novo?link=${link.code}`" class="btn" :class="{ 'btn-primary': !owned.length }">
          Cadastrar com os dados detectados
        </RouterLink>
      </div>
    </template>
  </section>

  <p v-if="error" class="error">{{ error }}</p>
</template>

<style scoped>
.lead {
  margin: 6px 0 20px;
}

.ask {
  display: flex;
  align-items: flex-end;
  gap: 12px;
  margin-bottom: 16px;
}

.code {
  width: 180px;
  text-transform: uppercase;
  font-size: 16px;
  letter-spacing: 0.08em;
}

.server {
  display: grid;
  grid-template-columns: 0.7fr 1fr 1.3fr;
  gap: 12px;
  margin-bottom: 16px;
}

.server div {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 12px;
  border-radius: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  font-size: 13px;
}

.label {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-faint);
}

.mono {
  font-family: 'Cascadia Code', Consolas, monospace;
  font-size: 16px;
}

.warn-text {
  color: var(--warn);
  font-size: 13px;
}

.choose {
  display: flex;
  align-items: flex-end;
  gap: 12px;
  margin-bottom: 16px;
}

.choose .field {
  flex: 1;
  max-width: 420px;
}

.new {
  display: flex;
  align-items: center;
  gap: 12px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
}

.btn {
  text-decoration: none;
}
</style>

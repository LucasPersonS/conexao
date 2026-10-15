<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import banner from '../assets/banner.jpg'
import logo from '../assets/logo.png'
import wordmark from '../assets/wordmark.png'
import { api, loadSession, message, setToken } from '../lib/api'

const router = useRouter()
const route = useRoute()
const next = typeof route.query.next === 'string' && route.query.next.startsWith('/') ? route.query.next : '/'
const code = ref<string | null>(null)
const error = ref<string | null>(null)
const expired = ref(false)
let timer: number | undefined

async function start() {
  error.value = null
  expired.value = false
  window.clearTimeout(timer)
  try {
    const pair = await api.pairStart()
    code.value = pair.code
    poll(pair.poll_token)
  } catch (e) {
    error.value = message(e)
  }
}

async function poll(pollToken: string) {
  try {
    const result = await api.pairPoll(pollToken)
    if (result.status === 'approved' && result.token) {
      setToken(result.token)
      await loadSession()
      router.push(next)
      return
    }
    if (result.status === 'expired') {
      expired.value = true
      code.value = null
      return
    }
  } catch (e) {
    if ((e as { status?: number }).status === 410) {
      expired.value = true
      code.value = null
      return
    }
    error.value = message(e)
  }
  timer = window.setTimeout(() => poll(pollToken), 2000)
}

onBeforeUnmount(() => window.clearTimeout(timer))
</script>

<template>
  <section class="login" :style="{ backgroundImage: `url(${banner})` }">
    <div class="veil" />
    <div class="card box">
      <img :src="logo" alt="" class="logo" />
      <img :src="wordmark" alt="Conexão" class="wordmark" />
      <h1>Painel dos servidores</h1>
      <p class="muted">Gerencie seu servidor: modpack, whitelist, equipe e agente.</p>

      <template v-if="!code">
        <button class="btn btn-primary wide" @click="start">Entrar com o launcher</button>
        <p v-if="expired" class="muted small">O código expirou. Gere outro.</p>
      </template>
      <template v-else>
        <ol class="steps">
          <li>Abra o launcher Conexão já logado na sua conta.</li>
          <li>No menu da sua conta (canto superior direito), clique em <strong>Conectar painel web</strong>.</li>
          <li>Digite o código abaixo.</li>
        </ol>
        <div class="code">{{ code }}</div>
        <p class="muted small waiting">Aguardando confirmação no launcher…</p>
      </template>
      <p v-if="error" class="error">{{ error }}</p>
    </div>
  </section>
</template>

<style scoped>
.login {
  position: relative;
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 24px;
  background: var(--bg) center / cover no-repeat;
}

.veil {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse at center, rgba(6, 10, 11, 0.7) 0%, rgba(6, 10, 11, 0.9) 70%),
    linear-gradient(180deg, rgba(6, 10, 11, 0.2), rgba(6, 10, 11, 0.75));
}

.box {
  position: relative;
  width: 440px;
  text-align: center;
  padding: 36px 40px;
  border-radius: 22px;
  background: rgba(14, 21, 24, 0.88);
  backdrop-filter: blur(10px);
  box-shadow: var(--shadow-pop), 0 0 60px rgba(183, 254, 0, 0.08);
}

.logo {
  width: 96px;
  height: 96px;
  filter: drop-shadow(0 0 22px rgba(183, 254, 0, 0.45));
}

.wordmark {
  display: block;
  width: 210px;
  margin: 12px auto 4px;
}

h1 {
  margin: 8px 0 6px;
  font-size: 20px;
  color: var(--text-dim);
  font-weight: 600;
}

.muted {
  margin: 0 0 22px;
  line-height: 1.5;
}

.wide {
  width: 100%;
  height: 46px;
}

.steps {
  text-align: left;
  color: var(--text-dim);
  line-height: 1.6;
  padding-left: 20px;
  margin: 0 0 18px;
  font-size: 14px;
}

.code {
  font-size: 34px;
  font-weight: 800;
  letter-spacing: 0.18em;
  padding: 16px;
  border-radius: 14px;
  background: var(--bg-input);
  border: 2px dashed var(--lime);
  color: var(--lime);
  font-family: var(--font-pixel);
  font-weight: 400;
  user-select: all;
}

.small {
  font-size: 13px;
}

.waiting {
  margin: 14px 0 0;
}
</style>

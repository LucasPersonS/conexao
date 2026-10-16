<script setup lang="ts">
import { ref, watch } from 'vue'
import banner from '../assets/banner.jpg'
import logo from '../assets/logo.png'
import wordmark from '../assets/wordmark.png'
import Icon from './Icon.vue'

/** Primeiro acesso pelo Discord: falta escolher o nick. */
export interface Signup {
  discord_name: string
  suggested_nick: string | null
}

const props = defineProps<{ busy: boolean; error: string | null; signup: Signup | null }>()
const emit = defineEmits<{ login: []; discord: []; register: [nick: string]; cancelSignup: [] }>()

const nick = ref('')
const NICK = /^[A-Za-z0-9_]{3,16}$/

watch(
  () => props.signup,
  (signup) => (nick.value = signup?.suggested_nick ?? ''),
  { immediate: true },
)

function submit() {
  if (NICK.test(nick.value.trim())) emit('register', nick.value.trim())
}
</script>

<template>
  <section class="gate" :style="{ backgroundImage: `url(${banner})` }">
    <div class="veil" />
    <div class="card">
      <span class="corner tl" /><span class="corner tr" /><span class="corner bl" /><span class="corner br" />
      <img :src="logo" alt="" class="logo" />
      <img :src="wordmark" alt="Conexão" class="wordmark" />

      <form v-if="signup" class="signup" @submit.prevent="submit">
        <p>
          Olá, <b>{{ signup.discord_name }}</b>! Escolha o nick que você vai usar nos servidores. Ele não pode ser
          trocado depois.
        </p>
        <input
          v-model="nick"
          class="nick"
          maxlength="16"
          placeholder="SeuNick"
          autocomplete="off"
          spellcheck="false"
          autofocus
          :disabled="busy"
        />
        <small class="rule" :class="{ bad: nick && !NICK.test(nick.trim()) }">3 a 16 caracteres: letras, números e _</small>
        <button class="btn btn-primary wide" type="submit" :disabled="busy || !NICK.test(nick.trim())">
          {{ busy ? 'Criando conta…' : 'Criar conta e entrar' }}
        </button>
        <button class="link" type="button" :disabled="busy" @click="emit('cancelSignup')">Voltar</button>
      </form>

      <template v-else>
        <p>Os servidores de Minecraft Roleplay num só lugar. Entre com a conta Microsoft que tem o Minecraft Java.</p>
        <button class="btn btn-primary wide" :disabled="busy" @click="emit('login')">
          <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
            <rect x="1" y="1" width="6.5" height="6.5" fill="currentColor" />
            <rect x="8.5" y="1" width="6.5" height="6.5" fill="currentColor" />
            <rect x="1" y="8.5" width="6.5" height="6.5" fill="currentColor" />
            <rect x="8.5" y="8.5" width="6.5" height="6.5" fill="currentColor" />
          </svg>
          {{ busy ? 'Aguardando login…' : 'Entrar com Microsoft' }}
        </button>
        <div class="or"><span>ou</span></div>
        <button class="btn wide discord" :disabled="busy" @click="emit('discord')">
          <Icon name="users" :size="18" />
          Jogar sem conta original
        </button>
        <small class="note">Entra com o Discord. Funciona só nos servidores que aceitam contas não originais.</small>
      </template>

      <p v-if="error" class="error">{{ error }}</p>
    </div>
  </section>
</template>

<style scoped>
.gate {
  position: relative;
  height: 100%;
  display: grid;
  place-items: center;
  background: var(--bg) center / cover no-repeat;
}

.veil {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse at center, rgba(var(--shade), 0.7) 0%, rgba(var(--shade), 0.9) 70%),
    linear-gradient(180deg, rgba(var(--shade), 0.2), rgba(var(--shade), 0.75));
}

.card {
  position: relative;
  width: 440px;
  padding: 34px 40px 34px;
  text-align: center;
  background: rgba(14, 21, 24, 0.86);
  border: 1px solid var(--line);
  border-radius: 22px;
  box-shadow: var(--shadow-pop), 0 0 60px rgba(183, 254, 0, 0.08);
  backdrop-filter: blur(10px);
}

/* Cantoneiras do botão "Junte-se agora" dos banners */
.corner {
  position: absolute;
  width: 18px;
  height: 18px;
  border: 0 solid var(--lime);
}

.tl {
  top: -1px;
  left: -1px;
  border-top-width: 2px;
  border-left-width: 2px;
  border-top-left-radius: 22px;
}

.tr {
  top: -1px;
  right: -1px;
  border-top-width: 2px;
  border-right-width: 2px;
  border-top-right-radius: 22px;
}

.bl {
  bottom: -1px;
  left: -1px;
  border-bottom-width: 2px;
  border-left-width: 2px;
  border-bottom-left-radius: 22px;
}

.br {
  bottom: -1px;
  right: -1px;
  border-bottom-width: 2px;
  border-right-width: 2px;
  border-bottom-right-radius: 22px;
}

.logo {
  width: 104px;
  height: 104px;
  filter: drop-shadow(0 0 22px rgba(183, 254, 0, 0.45));
}

.wordmark {
  display: block;
  width: 230px;
  margin: 14px auto 14px;
}

p {
  margin: 0 0 24px;
  color: var(--text-dim);
  line-height: 1.5;
}

.wide {
  width: 100%;
  height: 48px;
  font-size: 15px;
}

.error {
  margin: 16px 0 0;
  color: var(--bad);
}

.or {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 16px 0;
  color: var(--text-faint);
  font-size: 12px;
}

.or::before,
.or::after {
  content: '';
  flex: 1;
  height: 1px;
  background: var(--line);
}

.discord:hover:not(:disabled) {
  border-color: #5865f2;
  background: rgba(88, 101, 242, 0.16);
}

.note,
.rule {
  display: block;
  margin-top: 10px;
  color: var(--text-faint);
  font-size: 12px;
  line-height: 1.4;
}

.rule.bad {
  color: var(--bad);
}

.signup p {
  margin-bottom: 18px;
}

.nick {
  width: 100%;
  height: 46px;
  padding: 0 14px;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: var(--bg-input);
  font-size: 16px;
  text-align: center;
  outline: none;
}

.nick:focus {
  border-color: var(--lime);
  box-shadow: 0 0 0 3px var(--lime-soft);
}

.signup .rule {
  margin: 8px 0 18px;
}

.link {
  margin-top: 12px;
  color: var(--text-dim);
  font-size: 13px;
  text-decoration: underline;
}
</style>

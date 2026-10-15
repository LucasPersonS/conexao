<script setup lang="ts">
import { ref } from 'vue'
import logo from '../assets/logo.png'
import { prefs } from '../lib/prefs'
import Icon from './Icon.vue'

const STEPS = [
  {
    icon: 'compass',
    title: 'Escolha um servidor',
    text: 'Na tela inicial ficam os destaques e os mais jogados. Em Servidores você busca por nome ou filtra por estilo.',
  },
  {
    icon: 'shield',
    title: 'Peça whitelist, se precisar',
    text: 'Alguns servidores aprovam cada jogador. O botão mostra se você já pode entrar ou se o pedido está em análise.',
  },
  {
    icon: 'play',
    title: 'Clique em Jogar',
    text: 'O Conexão instala Java, loader e mods sozinho e abre o Minecraft já conectado. Nas próximas vezes só baixa o que mudou.',
  },
]
const step = ref(0)
function finish() {
  prefs.onboarded = true
}
</script>

<template>
  <div class="backdrop">
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="onb-title">
      <img :src="logo" alt="" class="logo" />
      <h2 id="onb-title">Bem-vindo ao Conexão</h2>
      <p class="sub">Três passos para jogar nos servidores de roleplay:</p>

      <div class="steps">
        <div v-for="(s, i) in STEPS" :key="s.title" class="step" :class="{ on: i === step, done: i < step }" @click="step = i">
          <span class="num"><Icon :name="s.icon" :size="20" /></span>
          <div>
            <strong>{{ i + 1 }}. {{ s.title }}</strong>
            <p>{{ s.text }}</p>
          </div>
        </div>
      </div>

      <div class="actions">
        <button class="btn" @click="finish">Pular</button>
        <button v-if="step < STEPS.length - 1" class="btn btn-primary" @click="step++">Próximo</button>
        <button v-else class="btn btn-primary" @click="finish">Começar a jogar</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: grid;
  place-items: center;
  background: rgba(var(--shade), 0.72);
  backdrop-filter: blur(6px);
}

.dialog {
  position: relative;
  width: min(520px, calc(100% - 32px));
  padding: 28px;
  border-radius: 20px;
  background: var(--bg-raised);
  border: 1px solid var(--line);
  box-shadow: var(--shadow-pop), 0 0 50px rgba(183, 254, 0, 0.08);
  text-align: center;
  overflow: hidden;
}

.dialog::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 3px;
  background: linear-gradient(90deg, transparent, var(--lime), transparent);
}

.logo {
  width: 72px;
  height: 72px;
  filter: drop-shadow(0 0 16px rgba(183, 254, 0, 0.45));
}

h2 {
  margin: 8px 0 4px;
  font-size: 24px;
}

.sub {
  margin: 0 0 18px;
  color: var(--text-dim);
}

.steps {
  display: flex;
  flex-direction: column;
  gap: 8px;
  text-align: left;
}

.step {
  display: flex;
  gap: 14px;
  padding: 12px 14px;
  border-radius: 14px;
  border: 1px solid transparent;
  cursor: pointer;
  opacity: 0.5;
  transition: opacity 0.2s, background 0.2s, border-color 0.2s;
}

.step.on {
  opacity: 1;
  background: var(--bg-soft);
  border-color: var(--lime-line);
}

.step.done {
  opacity: 0.8;
}

.num {
  width: 40px;
  height: 40px;
  display: grid;
  place-items: center;
  flex: none;
  border-radius: 12px;
  background: var(--lime-soft);
  color: var(--lime);
}

.step.on .num {
  background: var(--lime);
  color: var(--ink);
  box-shadow: 0 0 18px rgba(183, 254, 0, 0.35);
}

.step p {
  margin: 4px 0 0;
  font-size: 13px;
  line-height: 1.45;
  color: var(--text-dim);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 20px;
}
</style>

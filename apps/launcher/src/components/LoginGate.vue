<script setup lang="ts">
import banner from '../assets/banner.jpg'
import logo from '../assets/logo.png'
import wordmark from '../assets/wordmark.png'

defineProps<{ busy: boolean; error: string | null }>()
defineEmits<{ login: [] }>()
</script>

<template>
  <section class="gate" :style="{ backgroundImage: `url(${banner})` }">
    <div class="veil" />
    <div class="card">
      <span class="corner tl" /><span class="corner tr" /><span class="corner bl" /><span class="corner br" />
      <img :src="logo" alt="" class="logo" />
      <img :src="wordmark" alt="Conexão" class="wordmark" />
      <p>Os servidores de Minecraft Roleplay num só lugar. Entre com a conta Microsoft que tem o Minecraft Java.</p>
      <button class="btn btn-primary wide" :disabled="busy" @click="$emit('login')">
        <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
          <rect x="1" y="1" width="6.5" height="6.5" fill="currentColor" />
          <rect x="8.5" y="1" width="6.5" height="6.5" fill="currentColor" />
          <rect x="1" y="8.5" width="6.5" height="6.5" fill="currentColor" />
          <rect x="8.5" y="8.5" width="6.5" height="6.5" fill="currentColor" />
        </svg>
        {{ busy ? 'Aguardando login…' : 'Entrar com Microsoft' }}
      </button>
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
</style>

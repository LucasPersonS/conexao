<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
import logo from '../assets/logo.png'
import wordmark from '../assets/wordmark.png'

const win = getCurrentWindow()
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <img :src="logo" alt="" class="logo" data-tauri-drag-region />
      <img :src="wordmark" alt="Conexão" class="wordmark" data-tauri-drag-region />
    </div>
    <div class="controls">
      <button aria-label="Minimizar" @click="win.minimize()">
        <svg viewBox="0 0 12 12" width="12" height="12"><path d="M2 6h8" stroke="currentColor" stroke-width="1.4" /></svg>
      </button>
      <button aria-label="Maximizar" @click="win.toggleMaximize()">
        <svg viewBox="0 0 12 12" width="12" height="12"><rect x="2" y="2" width="8" height="8" fill="none" stroke="currentColor" stroke-width="1.4" /></svg>
      </button>
      <button class="close" aria-label="Fechar" @click="win.close()">
        <svg viewBox="0 0 12 12" width="12" height="12"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7" stroke="currentColor" stroke-width="1.4" /></svg>
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  height: 42px;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-left: 12px;
  background: #070b0c;
  border-bottom: 1px solid var(--line);
  position: relative;
}

/* Fio de luz lima, como o cabo do logo */
.titlebar::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 1px;
  background: linear-gradient(90deg, var(--lime) 0%, rgba(183, 254, 0, 0.35) 22%, transparent 60%);
}

.brand {
  display: flex;
  align-items: center;
  gap: 8px;
}

.logo {
  width: 26px;
  height: 26px;
  filter: drop-shadow(0 0 6px rgba(183, 254, 0, 0.45));
}

.wordmark {
  height: 19px;
  width: auto;
}

.controls {
  display: flex;
  height: 100%;
}

.controls button {
  width: 46px;
  height: 100%;
  color: var(--text-dim);
  display: grid;
  place-items: center;
}

.controls button:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.controls .close:hover {
  background: #e5484d;
  color: white;
}
</style>

<script setup lang="ts">
/** Área para arrastar a pasta do modpack, um .mrpack ou um .zip do CurseForge. */
import { ref } from 'vue'
import { filesFromDrop, filesFromInput, type InputFile } from '../lib/packdetect'

defineProps<{ disabled?: boolean; compact?: boolean }>()
const emit = defineEmits<{ files: [files: InputFile[]] }>()
const over = ref(false)
const reading = ref(false)

async function onDrop(event: DragEvent) {
  over.value = false
  if (!event.dataTransfer) return
  reading.value = true
  try {
    const files = await filesFromDrop(event.dataTransfer)
    if (files.length) emit('files', files)
  } finally {
    reading.value = false
  }
}

function onInput(event: Event) {
  const input = event.target as HTMLInputElement
  const files = filesFromInput(input.files)
  input.value = ''
  if (files.length) emit('files', files)
}
</script>

<template>
  <div
    class="drop"
    :class="{ over, compact, disabled }"
    @dragover.prevent="over = true"
    @dragleave.prevent="over = false"
    @drop.prevent="onDrop"
  >
    <svg viewBox="0 0 24 24" width="34" height="34" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
      <path d="M12 16V4M7 9l5-5 5 5M5 20h14" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
    <strong>{{ reading ? 'Lendo arquivos…' : 'Arraste aqui a pasta do modpack' }}</strong>
    <span class="muted">ou um <code>.mrpack</code> / <code>.zip</code> exportado. Versões e loader são detectados sozinhos.</span>
    <div class="buttons">
      <label class="btn btn-primary btn-sm file">
        Escolher pasta
        <input type="file" webkitdirectory multiple :disabled="disabled" @change="onInput" />
      </label>
      <label class="btn btn-sm file">
        Escolher .mrpack / .zip
        <input type="file" accept=".mrpack,.zip" :disabled="disabled" @change="onInput" />
      </label>
    </div>
  </div>
</template>

<style scoped>
.drop {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 28px 20px;
  border: 2px dashed var(--line);
  border-radius: 16px;
  background: var(--bg-soft);
  text-align: center;
  color: var(--brand-strong);
  transition: border-color 0.15s, background 0.15s;
}

.drop.compact {
  padding: 18px;
}

.drop.over {
  border-color: var(--brand);
  background: var(--lime-soft);
}

.drop.disabled {
  opacity: 0.6;
  pointer-events: none;
}

strong {
  color: var(--text);
  font-size: 15px;
}

.muted {
  font-size: 13px;
}

.buttons {
  display: flex;
  gap: 8px;
  margin-top: 6px;
}

.file {
  position: relative;
  overflow: hidden;
  cursor: pointer;
}

.file input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}
</style>

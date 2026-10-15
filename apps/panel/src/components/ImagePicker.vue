<script setup lang="ts">
/** Escolher ícone ou banner com pré-visualização. Emite o arquivo; quem usa decide quando enviar. */
import { computed, onBeforeUnmount, ref, watch } from 'vue'

const props = defineProps<{ kind: 'icon' | 'banner'; current: string | null; busy?: boolean }>()
const emit = defineEmits<{ pick: [file: File] }>()
const error = ref<string | null>(null)
const local = ref<string | null>(null)

const limit = computed(() => (props.kind === 'icon' ? 1 : 5) * 1024 * 1024)
const preview = computed(() => local.value ?? props.current)

watch(
  () => props.current,
  () => {
    if (local.value) URL.revokeObjectURL(local.value)
    local.value = null
  },
)
onBeforeUnmount(() => local.value && URL.revokeObjectURL(local.value))

function onInput(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  error.value = null
  if (!['image/png', 'image/jpeg', 'image/webp'].includes(file.type)) {
    error.value = 'Use PNG, JPEG ou WebP.'
    return
  }
  if (file.size > limit.value) {
    error.value = `Máximo de ${limit.value / 1024 / 1024} MB.`
    return
  }
  if (local.value) URL.revokeObjectURL(local.value)
  local.value = URL.createObjectURL(file)
  emit('pick', file)
}
</script>

<template>
  <div class="picker" :class="kind">
    <div class="preview" :style="preview ? { backgroundImage: `url(&quot;${preview}&quot;)` } : undefined">
      <span v-if="!preview">{{ kind === 'icon' ? 'Ícone' : 'Banner' }}</span>
    </div>
    <div class="side">
      <strong>{{ kind === 'icon' ? 'Ícone' : 'Banner' }}</strong>
      <span class="muted">{{ kind === 'icon' ? 'Quadrado, 256×256 ou maior' : 'Paisagem, 1600×500 ou maior' }}</span>
      <label class="btn btn-sm file">
        {{ busy ? 'Enviando…' : preview ? 'Trocar imagem' : 'Escolher imagem' }}
        <input type="file" accept="image/png,image/jpeg,image/webp" :disabled="busy" @change="onInput" />
      </label>
      <span v-if="error" class="error">{{ error }}</span>
    </div>
  </div>
</template>

<style scoped>
.picker {
  display: flex;
  gap: 14px;
  align-items: center;
}

.preview {
  flex: none;
  display: grid;
  place-items: center;
  border-radius: 14px;
  border: 1px dashed var(--line);
  background: var(--bg-soft) center / cover;
  color: var(--text-faint);
  font-size: 12px;
}

.icon .preview {
  width: 72px;
  height: 72px;
}

.banner .preview {
  width: 200px;
  height: 72px;
}

.side {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 13px;
}

.file {
  position: relative;
  overflow: hidden;
  align-self: flex-start;
  cursor: pointer;
}

.file input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}

.error {
  font-size: 12px;
}
</style>

<script setup lang="ts">
/** Seletores de Minecraft, loader e versão do loader (listas oficiais, nada para digitar). */
import { computed, onMounted, ref, watch } from 'vue'
import { api, LOADER_NAMES, message, type Loader, type LoaderOption, type MinecraftOption } from '../lib/api'

const minecraft = defineModel<string>('minecraft', { required: true })
const loader = defineModel<Loader>('loader', { required: true })
const loaderVersion = defineModel<string>('loaderVersion', { required: true })
const props = defineProps<{ disabled?: boolean; detected?: { minecraft?: string | null; loader?: Loader | null; loaderVersion?: string | null } }>()

const versions = ref<MinecraftOption[]>([])
const loaderVersions = ref<LoaderOption[]>([])
const showAll = ref(false)
const error = ref<string | null>(null)
const loadingLoaders = ref(false)

onMounted(async () => {
  try {
    versions.value = await api.metaMinecraft()
    if (!versions.value.some((v) => v.id === minecraft.value) && versions.value.length) {
      minecraft.value = versions.value.find((v) => v.id === '1.21.1')?.id ?? versions.value[0].id
    }
  } catch (e) {
    error.value = `Não foi possível carregar as versões: ${message(e)}`
  }
})

const current = computed(() => versions.value.find((v) => v.id === minecraft.value))
const availableLoaders = computed<Loader[]>(() => current.value?.loaders ?? ['neoforge', 'forge', 'fabric'])

watch(availableLoaders, (list) => {
  if (list.length && !list.includes(loader.value)) loader.value = list[0]
})

async function loadLoaderVersions() {
  if (!minecraft.value || !loader.value) return
  loadingLoaders.value = true
  try {
    loaderVersions.value = await api.metaLoaders(loader.value, minecraft.value)
    const ids = loaderVersions.value.map((v) => v.id)
    if (!ids.includes(loaderVersion.value)) {
      loaderVersion.value = loaderVersions.value.find((v) => v.recommended)?.id ?? ids[0] ?? ''
    }
  } catch {
    loaderVersions.value = []
  } finally {
    loadingLoaders.value = false
  }
}
watch([minecraft, loader], loadLoaderVersions, { immediate: true })

const shownLoaderVersions = computed(() => {
  const list = showAll.value ? loaderVersions.value : loaderVersions.value.slice(0, 25)
  // Garante que a versão escolhida (ex.: detectada) aparece mesmo fora das 25 mais novas.
  if (loaderVersion.value && !list.some((v) => v.id === loaderVersion.value)) {
    const chosen = loaderVersions.value.find((v) => v.id === loaderVersion.value)
    if (chosen) return [...list, chosen]
  }
  return list
})

const matchesDetected = (field: 'minecraft' | 'loader' | 'loaderVersion', value: string) =>
  !!props.detected?.[field] && props.detected[field] === value
</script>

<template>
  <div class="versions">
    <label class="field">
      Minecraft
      <select v-model="minecraft" class="input" :disabled="disabled || !versions.length">
        <option v-if="!versions.length" :value="minecraft">{{ minecraft || 'Carregando…' }}</option>
        <option v-for="v in versions" :key="v.id" :value="v.id">
          {{ v.id }}{{ matchesDetected('minecraft', v.id) ? ' (detectado)' : '' }}
        </option>
      </select>
    </label>
    <label class="field">
      Loader
      <select v-model="loader" class="input" :disabled="disabled">
        <option v-for="l in availableLoaders" :key="l" :value="l">
          {{ LOADER_NAMES[l] }}{{ matchesDetected('loader', l) ? ' (detectado)' : '' }}
        </option>
      </select>
    </label>
    <label class="field">
      Versão do {{ LOADER_NAMES[loader] }}
      <select v-model="loaderVersion" class="input" :disabled="disabled || loadingLoaders || !loaderVersions.length">
        <option v-if="!loaderVersions.length" :value="loaderVersion">{{ loadingLoaders ? 'Carregando…' : loaderVersion || '—' }}</option>
        <option v-for="v in shownLoaderVersions" :key="v.id" :value="v.id">
          {{ v.id }}{{ v.recommended ? ' — recomendada' : '' }}{{ matchesDetected('loaderVersion', v.id) ? ' (detectada)' : '' }}
        </option>
      </select>
      <button v-if="loaderVersions.length > 25" type="button" class="link more" @click="showAll = !showAll">
        {{ showAll ? 'mostrar só as mais novas' : `ver todas (${loaderVersions.length})` }}
      </button>
    </label>
    <p v-if="error" class="error span">{{ error }}</p>
  </div>
</template>

<style scoped>
.versions {
  display: grid;
  grid-template-columns: 1fr 1fr 1.3fr;
  gap: 14px;
}

.more {
  align-self: flex-start;
  font-size: 12px;
  font-weight: 500;
}

.span {
  grid-column: 1 / -1;
  margin: 0;
}
</style>

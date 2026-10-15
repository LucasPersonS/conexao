<script setup lang="ts">
import { reactive, ref } from 'vue'
import ImagePicker from './ImagePicker.vue'
import { message, uploadImage, type Loader } from '../lib/api'

export interface ServerFormValue {
  slug: string
  name: string
  description: string
  address: string
  minecraft: string
  loader: Loader
  loader_version: string
  tags: string
  icon_url: string
  banner_url: string
  discord_url: string
  whitelist_required: boolean
}

const props = defineProps<{ initial: ServerFormValue; creating: boolean; busy: boolean }>()
const emit = defineEmits<{ submit: [value: ServerFormValue]; image: [kind: 'icon' | 'banner', url: string] }>()
const form = reactive({ ...props.initial })
const uploading = ref<'icon' | 'banner' | null>(null)
const imageError = ref<string | null>(null)

async function upload(kind: 'icon' | 'banner', file: File) {
  uploading.value = kind
  imageError.value = null
  try {
    const url = await uploadImage(form.slug, kind, file)
    if (kind === 'icon') form.icon_url = url
    else form.banner_url = url
    emit('image', kind, url)
  } catch (e) {
    imageError.value = message(e)
  } finally {
    uploading.value = null
  }
}
</script>

<template>
  <form class="form" @submit.prevent="emit('submit', { ...form })">
    <div class="grid-2">
      <label class="field">
        Nome do servidor
        <input v-model="form.name" class="input" required maxlength="64" placeholder="Paravexia" />
      </label>
      <label class="field">
        Identificador (URL)
        <input
          v-model="form.slug"
          class="input"
          required
          :disabled="!creating"
          pattern="[a-z0-9][a-z0-9-]{1,47}"
          placeholder="paravexia"
          title="Letras minúsculas, números e hífen"
        />
      </label>
    </div>

    <label class="field">
      Descrição
      <textarea v-model="form.description" class="input" maxlength="2000" placeholder="Conte o tema do RP, regras principais, horário de eventos…" />
    </label>

    <div class="grid-2">
      <label class="field">
        Endereço para entrar
        <input v-model="form.address" class="input" required placeholder="jogar.seuservidor.com:25565" />
      </label>
      <label class="field">
        Tags (separadas por vírgula)
        <input v-model="form.tags" class="input" placeholder="rp médio, medieval" />
      </label>
    </div>

    <div v-if="creating" class="grid-3">
      <label class="field">
        Minecraft
        <input v-model="form.minecraft" class="input" required placeholder="1.21.1" />
      </label>
      <label class="field">
        Loader
        <select v-model="form.loader" class="input">
          <option value="neoforge">NeoForge</option>
          <option value="forge">Forge</option>
          <option value="fabric">Fabric</option>
        </select>
      </label>
      <label class="field">
        Versão do loader
        <input v-model="form.loader_version" class="input" required placeholder="21.1.256" />
      </label>
    </div>
    <p v-if="creating" class="hint muted">Minecraft 1.18.2 ou mais novo. Depois, versão e loader seguem o modpack publicado.</p>

    <div v-if="!creating" class="grid-2">
      <ImagePicker kind="icon" :current="form.icon_url || null" :busy="uploading === 'icon'" @pick="upload('icon', $event)" />
      <ImagePicker kind="banner" :current="form.banner_url || null" :busy="uploading === 'banner'" @pick="upload('banner', $event)" />
    </div>
    <p v-if="imageError" class="error">{{ imageError }}</p>
    <p v-if="!creating" class="hint muted">As imagens são salvas na hora e aparecem no launcher e no link de convite.</p>

    <div class="grid-2">
      <label class="field">
        Discord (URL https)
        <input v-model="form.discord_url" class="input" type="url" placeholder="https://discord.gg/…" />
      </label>
      <label class="check">
        <input v-model="form.whitelist_required" type="checkbox" />
        Exigir whitelist para jogar
      </label>
    </div>

    <div class="actions">
      <slot name="extra" />
      <button class="btn btn-primary" :disabled="busy">{{ busy ? 'Salvando…' : creating ? 'Cadastrar servidor' : 'Salvar alterações' }}</button>
    </div>
  </form>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.grid-3 {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: 14px;
}

.hint {
  margin: -6px 0 0;
  font-size: 12px;
}

.check {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 14px;
  padding-top: 22px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 12px;
}
</style>

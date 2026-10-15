<script setup lang="ts">
/** Editor do perfil com pré-visualização ao vivo. */
import { computed, onMounted, reactive, ref } from 'vue'
import ProfileCard from '@profile/ProfileCard.vue'
import { catalog, DEFAULT_THEME, fontFamily, LINK_LABELS, type ProfileLinks, type ProfileTheme, type PublicProfile } from '@profile/theme'
import Icon from '../components/Icon.vue'
import { errorMessage, profiles, type OwnProfile } from '../lib/api'
import { showToast } from '../lib/store'

const loaded = ref<OwnProfile | null>(null)
const error = ref<string | null>(null)
const saving = ref(false)
const uploading = ref(false)
const dirty = ref(false)

const form = reactive({
  display_name: '',
  pronouns: '',
  bio: '',
  links: {} as ProfileLinks,
  discord_visibility: 'everyone' as OwnProfile['discord_visibility'],
  theme: structuredClone(DEFAULT_THEME) as ProfileTheme,
})

function fill(p: OwnProfile) {
  loaded.value = p
  form.display_name = p.display_name ?? ''
  form.pronouns = p.pronouns ?? ''
  form.bio = p.bio ?? ''
  form.links = { ...p.links }
  form.discord_visibility = p.discord_visibility
  form.theme = structuredClone({ ...DEFAULT_THEME, ...p.theme })
  dirty.value = false
}

onMounted(async () => {
  try {
    fill(await profiles.mine())
  } catch (e) {
    error.value = errorMessage(e)
  }
})

const touch = () => (dirty.value = true)

/** Perfil exibido na pré-visualização, com o que ainda não foi salvo. */
const preview = computed<PublicProfile | null>(() =>
  loaded.value
    ? {
        ...loaded.value,
        display_name: form.display_name || null,
        pronouns: form.pronouns || null,
        bio: form.bio || null,
        links: Object.fromEntries(Object.entries(form.links).filter(([, v]) => v)),
        theme: form.theme,
        background_url: loaded.value.background_status === 'rejected' ? null : loaded.value.pending_background_url,
      }
    : null,
)

const locked = (tier: string) => tier === 'premium' && !!loaded.value?.premium_enforced && !loaded.value?.premium

// ------------------------------------------------------------------ fundo
function usePreset(colors: string[], angle: number) {
  form.theme.background = { kind: 'gradient', colors: [...colors], angle }
  touch()
}
function addColor() {
  if (form.theme.background.colors.length < 4) form.theme.background.colors.push('#ffffff')
  touch()
}
function removeColor(index: number) {
  if (form.theme.background.colors.length > 1) form.theme.background.colors.splice(index, 1)
  touch()
}
async function onBackground(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  if (!['image/png', 'image/jpeg', 'image/webp'].includes(file.type) || file.size > 3 * 1024 * 1024) {
    error.value = 'Use PNG, JPEG ou WebP de até 3 MB.'
    return
  }
  uploading.value = true
  error.value = null
  try {
    const updated = await profiles.uploadBackground(file)
    if (loaded.value) {
      loaded.value.pending_background_url = updated.pending_background_url
      loaded.value.background_status = updated.background_status
    }
    form.theme.background.kind = 'image'
    touch()
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    uploading.value = false
  }
}

const ACCENTS = ['#b7fe00', '#f2f6ef', '#47e891', '#2bd2ff', '#a259ff', '#ff6fa5', '#ffc371', '#ff5f6d']
const PRONOUNS = ['ele/dele', 'ela/dela', 'elu/delu', 'qualquer pronome']

// ----------------------------------------------------------------- salvar
async function save() {
  saving.value = true
  error.value = null
  try {
    fill(
      await profiles.save({
        display_name: form.display_name.trim() || null,
        pronouns: form.pronouns.trim() || null,
        bio: form.bio.trim() || null,
        links: form.links,
        theme: form.theme,
        discord_visibility: form.discord_visibility,
      }),
    )
    showToast('Perfil salvo!')
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    saving.value = false
  }
}

async function copyLink() {
  if (!loaded.value) return
  try {
    await navigator.clipboard.writeText(loaded.value.public_url)
    showToast('Link do perfil copiado. Cole no Discord!')
  } catch {
    showToast(loaded.value.public_url)
  }
}

async function copyDiscord(value: string) {
  try {
    await navigator.clipboard.writeText(value)
    showToast(`Discord "${value}" copiado.`)
  } catch {
    /* sem área de transferência */
  }
}

const bioLeft = computed(() => 300 - form.bio.length)
</script>

<template>
  <div class="profile-page">
    <div class="head">
      <div>
        <h2>Meu perfil</h2>
        <p class="muted">Aparece para a equipe dos servidores quando você pede whitelist e no seu link público.</p>
      </div>
      <div class="head-actions">
        <button class="btn" :disabled="!loaded" @click="copyLink"><Icon name="link" :size="16" /> Copiar link do perfil</button>
        <button class="btn btn-primary" :disabled="saving || !dirty || loaded?.banned" @click="save">{{ saving ? 'Salvando…' : 'Salvar' }}</button>
      </div>
    </div>
    <p v-if="error" class="error">{{ error }}</p>
    <div v-if="loaded?.banned" class="banned">
      <strong>Seu perfil foi bloqueado pela equipe Conexão.</strong>
      <span v-if="loaded.ban_reason">Motivo: {{ loaded.ban_reason }}.</span>
      Você continua jogando normalmente, mas não pode mais personalizar o perfil. Se achar que foi engano, fale com a
      equipe no Discord do Conexão.
    </div>

    <div v-if="loaded && preview" class="layout">
      <div class="editor" :class="{ locked: loaded.banned }" :inert="loaded.banned || undefined">
        <section class="card">
          <h3>Sobre você</h3>
          <div class="row2">
            <label class="field">
              Nome de exibição
              <input v-model="form.display_name" class="input" maxlength="32" :placeholder="loaded.minecraft_name" @input="touch" />
            </label>
            <label class="field">
              Pronomes
              <input v-model="form.pronouns" class="input" maxlength="24" placeholder="ela/dela" @input="touch" />
            </label>
          </div>
          <div class="chips">
            <button v-for="p in PRONOUNS" :key="p" class="chip" :class="{ on: form.pronouns === p }" @click="form.pronouns = p; touch()">{{ p }}</button>
          </div>
          <label class="field">
            <span class="label-row">Bio <span class="count" :class="{ warn: bioLeft < 20 }">{{ bioLeft }} restantes</span></span>
            <textarea v-model="form.bio" class="input area" maxlength="300" rows="4" placeholder="Seus fandoms, personagens, o que você curte no RP…" @input="touch" />
          </label>
        </section>

        <section class="card">
          <h3>Fundo</h3>
          <div class="tabs">
            <button :class="{ on: form.theme.background.kind === 'gradient' }" @click="form.theme.background.kind = 'gradient'; touch()">Gradiente</button>
            <button :class="{ on: form.theme.background.kind === 'image' }" @click="form.theme.background.kind = 'image'; touch()">Imagem</button>
          </div>
          <template v-if="form.theme.background.kind === 'gradient'">
            <div class="presets">
              <button
                v-for="g in catalog.gradients"
                :key="g.id"
                class="preset"
                :title="g.name"
                :style="{ background: `linear-gradient(${g.angle}deg, ${g.colors.join(', ')})` }"
                @click="usePreset(g.colors, g.angle)"
              />
            </div>
            <div class="stops">
              <label v-for="(c, i) in form.theme.background.colors" :key="i" class="stop">
                <input v-model="form.theme.background.colors[i]" type="color" @input="touch" />
                <button v-if="form.theme.background.colors.length > 1" class="x" title="Remover cor" @click.prevent="removeColor(i)">×</button>
              </label>
              <button v-if="form.theme.background.colors.length < 4" class="add" @click="addColor">+ cor</button>
              <label class="angle">
                Ângulo {{ form.theme.background.angle }}°
                <input v-model.number="form.theme.background.angle" type="range" min="0" max="360" @input="touch" />
              </label>
            </div>
          </template>
          <div v-else class="bg-image">
            <label class="btn btn-sm file">
              {{ uploading ? 'Enviando…' : loaded.pending_background_url ? 'Trocar imagem' : 'Enviar imagem' }}
              <input type="file" accept="image/png,image/jpeg,image/webp" :disabled="uploading" @change="onBackground" />
            </label>
            <span v-if="loaded.background_status === 'rejected'" class="status rejected">Removida pela moderação — escolha outra imagem</span>
            <p class="muted small">
              PNG, JPEG ou WebP até 3 MB. Aparece na hora para todo mundo. Imagens impróprias são removidas pela equipe Conexão
              e podem bloquear a personalização do seu perfil.
            </p>
          </div>
          <div class="row2">
            <div class="field">
              Cartão
              <div class="tabs small">
                <button :class="{ on: form.theme.panel === 'claro' }" @click="form.theme.panel = 'claro'; touch()">Claro</button>
                <button :class="{ on: form.theme.panel === 'escuro' }" @click="form.theme.panel = 'escuro'; touch()">Escuro</button>
              </div>
            </div>
            <div class="field">
              Cor de destaque
              <div class="accents">
                <button v-for="a in ACCENTS" :key="a" class="swatch" :class="{ on: form.theme.accent === a }" :style="{ background: a }" @click="form.theme.accent = a; touch()" />
                <input v-model="form.theme.accent" type="color" class="swatch-input" @input="touch" />
              </div>
            </div>
          </div>
        </section>

        <section class="card">
          <h3>Moldura</h3>
          <div class="grid frames">
            <button
              v-for="f in catalog.frames"
              :key="f.id"
              class="option"
              :class="{ on: form.theme.frame === f.id, locked: locked(f.tier) }"
              :disabled="locked(f.tier)"
              @click="form.theme.frame = f.id; touch()"
            >
              <span class="frame-preview" :style="{ '--cp-accent': form.theme.accent }">
                <span class="cp-avatar" :class="`cp-frame-${f.id}`">
                  <img class="cp-head" :src="`https://mc-heads.net/avatar/${loaded.minecraft_uuid}/64`" alt="" />
                  <i v-for="n in 6" :key="n" />
                </span>
              </span>
              <span class="label">{{ f.name }}</span>
              <span v-if="f.tier === 'premium'" class="tag">Conexão+</span>
            </button>
          </div>
        </section>

        <section class="card">
          <h3>Nome</h3>
          <div class="grid fonts">
            <button
              v-for="f in catalog.fonts"
              :key="f.id"
              class="option font"
              :class="{ on: form.theme.font === f.id, locked: locked(f.tier) }"
              :disabled="locked(f.tier)"
              :style="{ fontFamily: fontFamily(f.id) }"
              @click="form.theme.font = f.id; touch()"
            >
              {{ form.display_name || loaded.minecraft_name }}
              <span class="label">{{ f.name }}</span>
              <span v-if="f.tier === 'premium'" class="tag">Conexão+</span>
            </button>
          </div>
          <div class="effects">
            <button
              v-for="e in catalog.name_effects"
              :key="e.id"
              class="chip"
              :class="{ on: form.theme.name_effect === e.id }"
              :disabled="locked(e.tier)"
              @click="form.theme.name_effect = e.id; touch()"
            >
              {{ e.name }}<span v-if="e.tier === 'premium'" class="plus">+</span>
            </button>
          </div>
          <div v-if="['gradiente', 'brilho'].includes(form.theme.name_effect)" class="name-colors">
            Cores do nome
            <input v-model="form.theme.name_colors[0]" type="color" @input="touch" />
            <input v-model="form.theme.name_colors[1]" type="color" @input="touch" />
          </div>
        </section>

        <section class="card">
          <h3>Redes</h3>
          <div class="links">
            <label v-for="(label, key) in LINK_LABELS" :key="key" class="field">
              {{ label }}
              <input
                v-model="form.links[key]"
                class="input"
                :placeholder="key === 'discord' ? 'seu.usuario ou https://discord.gg/…' : `https://${key === 'x' ? 'x.com' : key === 'bluesky' ? 'bsky.app' : key + (key === 'twitch' ? '.tv' : '.com')}/…`"
                @input="touch"
              />
            </label>
          </div>
          <label class="field visibility">
            Quem vê seu Discord
            <select v-model="form.discord_visibility" class="input" @change="touch">
              <option value="everyone">Todo mundo</option>
              <option value="staff">Só a equipe dos servidores em que eu pedi whitelist</option>
              <option value="nobody">Ninguém</option>
            </select>
          </label>
        </section>
      </div>

      <aside class="preview">
        <ProfileCard :profile="preview" @discord="copyDiscord" />
        <p class="muted small center">Pré-visualização ao vivo. {{ dirty ? 'Não esqueça de salvar.' : '' }}</p>
      </aside>
    </div>
    <p v-else-if="!error" class="muted">Carregando perfil…</p>
  </div>
</template>

<style scoped>
.profile-page {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.head {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: 16px;
}

h2 {
  margin: 0 0 2px;
  font-size: 30px;
}

.head p {
  margin: 0;
}

.head-actions {
  display: flex;
  gap: 8px;
}

.layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 380px;
  gap: 20px;
  align-items: start;
}

.editor {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 18px;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
}

h3 {
  margin: 0;
  font-size: 17px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-dim);
}

.input {
  height: 38px;
  padding: 0 12px;
  border-radius: 10px;
  border: 1px solid var(--line);
  background: var(--bg-input);
  color: var(--text);
  outline: none;
  font-weight: 400;
}

.input:focus {
  border-color: var(--lime);
  background: var(--bg-input);
  box-shadow: 0 0 0 3px var(--lime-soft);
}

.area {
  height: auto;
  padding: 10px 12px;
  resize: vertical;
  font-family: inherit;
  line-height: 1.45;
}

.label-row {
  display: flex;
  justify-content: space-between;
}

.count {
  font-weight: 500;
  color: var(--text-faint);
}

.count.warn {
  color: var(--warn);
}

.row2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.chips,
.effects {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.chip {
  height: 30px;
  padding: 0 12px;
  border-radius: 999px;
  font-size: 13px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  color: var(--text-dim);
}

.chip:hover:not(:disabled) {
  border-color: var(--lime-line);
}

.chip.on {
  background: var(--lime-soft);
  border-color: var(--lime);
  color: var(--lime);
  font-weight: 600;
}

.plus {
  margin-left: 3px;
  color: var(--lime);
  font-weight: 800;
}

.tabs {
  display: inline-flex;
  align-self: flex-start;
  padding: 3px;
  border-radius: 10px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
}

.tabs button {
  height: 30px;
  padding: 0 14px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-dim);
}

.tabs button.on {
  background: var(--lime);
  color: var(--ink);
}

.presets {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.preset {
  width: 44px;
  height: 30px;
  border-radius: 8px;
  border: 2px solid var(--bg-card);
  box-shadow: 0 0 0 1px var(--line-strong);
}

.preset:hover {
  box-shadow: 0 0 0 2px var(--lime);
}

.stops {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.stop {
  position: relative;
}

input[type='color'] {
  width: 40px;
  height: 32px;
  padding: 0;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: none;
  cursor: pointer;
}

.x {
  position: absolute;
  top: -6px;
  right: -6px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--bad);
  color: #fff;
  font-size: 11px;
  line-height: 16px;
}

.add {
  height: 32px;
  padding: 0 10px;
  border-radius: 8px;
  border: 1px dashed var(--line-strong);
  font-size: 13px;
  color: var(--text-dim);
}

.add:hover {
  border-color: var(--lime);
  color: var(--lime);
}

.angle {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-width: 160px;
  font-size: 12px;
  color: var(--text-dim);
}

.angle input {
  accent-color: var(--lime);
}

.bg-image {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.file {
  position: relative;
  overflow: hidden;
}

.file input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}

.status {
  font-size: 12px;
  font-weight: 600;
  padding: 3px 10px;
  border-radius: 999px;
}

.banned {
  margin-bottom: 16px;
  padding: 12px 14px;
  border-radius: 12px;
  background: var(--bad-bg);
  border: 1px solid var(--bad-line);
  color: var(--text);
  line-height: 1.5;
  font-size: 14px;
}

.editor.locked {
  opacity: 0.45;
  filter: grayscale(0.6);
}

.status.pending {
  background: var(--warn-bg);
  color: var(--warn);
}

.status.approved {
  background: var(--good-bg);
  color: var(--good);
}

.status.rejected {
  background: var(--bad-bg);
  color: var(--bad);
}

.accents {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.swatch {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 2px solid var(--bg-card);
  box-shadow: 0 0 0 1px var(--line-strong);
}

.swatch.on {
  box-shadow: 0 0 0 2px var(--lime);
}

.swatch-input {
  width: 28px !important;
  height: 28px !important;
  border-radius: 50% !important;
}

.grid {
  display: grid;
  gap: 8px;
}

.frames {
  grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
}

.fonts {
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
}

.option {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 12px 6px 8px;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: var(--bg-soft);
  overflow: hidden;
}

.option:hover:not(:disabled) {
  border-color: var(--lime-line);
}

.option.on {
  border-color: var(--lime);
  background: var(--lime-soft);
  box-shadow: 0 0 0 2px rgba(183, 254, 0, 0.25);
}

.option.locked {
  opacity: 0.5;
}

.option.font {
  font-size: 17px;
  padding: 14px 8px 8px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.label {
  font-family: var(--font, inherit);
  font-size: 12px;
  color: var(--text-dim);
  font-weight: 600;
}

.option.font .label {
  font-family: 'Inter', sans-serif;
}

.tag {
  font-family: 'Inter', sans-serif;
  position: absolute;
  top: 4px;
  right: 4px;
  padding: 1px 5px;
  border-radius: 6px;
  font-size: 9px;
  font-weight: 800;
  color: var(--ink);
  background: linear-gradient(90deg, var(--lime), var(--lime-yellow));
}

.frame-preview {
  display: block;
  width: 56px;
  height: 56px;
  margin: 6px 0 2px;
}

.frame-preview .cp-avatar {
  position: relative;
  left: auto;
  bottom: auto;
  width: 56px;
  height: 56px;
}

.frame-preview .cp-head {
  border-radius: 14px;
}

.name-colors {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--text-dim);
  font-weight: 600;
}

.links {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px 12px;
}

.preview {
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}

.small {
  font-size: 12px;
  margin: 0;
}

.center {
  text-align: center;
}

.error {
  color: var(--bad);
  margin: 0;
}

@media (max-width: 1000px) {
  .layout {
    grid-template-columns: 1fr;
  }

  .preview {
    position: static;
    order: -1;
  }
}
</style>

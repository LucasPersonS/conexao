<script setup lang="ts">
/**
 * Card de perfil do Conexão. A página pública (/u/<nick>) gera o mesmo HTML no
 * servidor (apps/api/src/routes/profiles.rs) com o mesmo profile.css.
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { cardStyle, LINK_LABELS, type ProfileLinks, type PublicProfile } from './theme'
import './profile.css'

const props = defineProps<{ profile: PublicProfile; compact?: boolean }>()
const emit = defineEmits<{ discord: [value: string] }>()

const root = ref<HTMLElement | null>(null)
const paused = ref(false)
let observer: IntersectionObserver | undefined

// Animações param quando o card sai da tela (não pesa no launcher).
onMounted(() => {
  if (!root.value || typeof IntersectionObserver === 'undefined') return
  observer = new IntersectionObserver(([entry]) => (paused.value = !entry.isIntersecting))
  observer.observe(root.value)
})
onBeforeUnmount(() => observer?.disconnect())

const name = computed(() => props.profile.display_name || props.profile.minecraft_name)
const letters = computed(() => Array.from(name.value))
const style = computed(() => cardStyle(props.profile.theme, props.profile.background_url))
const links = computed(() =>
  (Object.keys(LINK_LABELS) as (keyof ProfileLinks)[])
    .map((key) => ({ key, value: props.profile.links[key] }))
    .filter((l): l is { key: keyof ProfileLinks; value: string } => !!l.value),
)
const isUrl = (value: string) => /^https:\/\//.test(value)
</script>

<template>
  <article
    ref="root"
    class="cp-card"
    :class="{ 'cp-dark': profile.theme.panel === 'escuro', 'cp-paused': paused, 'cp-compact': compact }"
    :style="style"
  >
    <div class="cp-bg" />
    <div class="cp-top">
      <div class="cp-avatar" :class="`cp-frame-${profile.theme.frame}`">
        <img class="cp-head" :src="`https://mc-heads.net/avatar/${profile.minecraft_uuid}/128`" alt="" />
        <i v-for="n in 6" :key="n" />
      </div>
    </div>
    <div class="cp-body">
      <h2 class="cp-name" :class="`cp-name-${profile.theme.name_effect}`" :data-text="name">
        <template v-if="profile.theme.name_effect === 'onda'">
          <span v-for="(c, i) in letters" :key="i" :style="{ '--i': i }">{{ c }}</span>
        </template>
        <template v-else>{{ name }}</template>
      </h2>
      <div class="cp-sub">
        <span>@{{ profile.minecraft_name }}</span>
        <span v-if="profile.pronouns">· {{ profile.pronouns }}</span>
      </div>
      <div v-if="profile.badges.length" class="cp-badges">
        <span v-for="b in profile.badges" :key="b.id" class="cp-badge">{{ b.label }}</span>
      </div>
      <p v-if="profile.bio && !compact" class="cp-bio">{{ profile.bio }}</p>
      <div v-if="links.length && !compact" class="cp-links">
        <template v-for="l in links" :key="l.key">
          <a v-if="isUrl(l.value)" class="cp-link" :class="`cp-link-${l.key}`" :href="l.value" target="_blank" rel="noopener noreferrer">
            {{ LINK_LABELS[l.key] }}
          </a>
          <button v-else type="button" class="cp-link" :class="`cp-link-${l.key}`" :title="`Copiar ${l.value}`" @click="emit('discord', l.value)">
            {{ LINK_LABELS[l.key] }} · {{ l.value }}
          </button>
        </template>
      </div>
    </div>
  </article>
</template>

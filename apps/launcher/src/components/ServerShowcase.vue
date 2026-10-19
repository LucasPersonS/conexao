<script setup lang="ts">
/**
 * Vitrine do servidor (PRO): blocos de texto, imagens/GIFs, galerias e vídeos do YouTube
 * montados pela equipe no painel. Tudo vem como dado estruturado da API; nada de HTML livre.
 */
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import Icon from './Icon.vue'
import type { PageBlock } from '../lib/api'
import { youtubeEmbed, youtubeThumb, youtubeWatch } from '../lib/page'

defineProps<{ blocks: PageBlock[] }>()

/** Vídeos já abertos (o player só carrega depois do clique). */
const playing = ref(new Set<number>())
const lightbox = ref<{ urls: string[]; index: number } | null>(null)

function play(index: number) {
  playing.value = new Set(playing.value).add(index)
}

function open(urls: string[], index: number) {
  lightbox.value = { urls, index }
}

function step(delta: number) {
  const box = lightbox.value
  if (box) box.index = (box.index + delta + box.urls.length) % box.urls.length
}

function onKey(event: KeyboardEvent) {
  if (!lightbox.value) return
  if (event.key === 'Escape') lightbox.value = null
  else if (event.key === 'ArrowRight') step(1)
  else if (event.key === 'ArrowLeft') step(-1)
}

onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <div class="showcase">
    <template v-for="(block, index) in blocks" :key="index">
      <section v-if="block.type === 'text'" class="card text">
        <h3 v-if="block.title">{{ block.title }}</h3>
        <p v-if="block.body">{{ block.body }}</p>
      </section>

      <figure v-else-if="block.type === 'image'" class="media">
        <button class="shot" title="Ampliar" @click="open([block.url], 0)">
          <img :src="block.url" :alt="block.caption || 'Imagem do servidor'" loading="lazy" draggable="false" />
        </button>
        <figcaption v-if="block.caption">{{ block.caption }}</figcaption>
      </figure>

      <figure v-else-if="block.type === 'gallery'" class="media card gallery-card">
        <div class="gallery" :class="{ few: block.urls.length <= 3 }">
          <button v-for="(url, i) in block.urls" :key="url + i" class="thumb" @click="open(block.urls, i)">
            <img :src="url" :alt="`Imagem ${i + 1} da galeria`" loading="lazy" draggable="false" />
          </button>
        </div>
        <figcaption v-if="block.caption">{{ block.caption }}</figcaption>
      </figure>

      <figure v-else-if="block.type === 'video'" class="media">
        <div class="video">
          <iframe
            v-if="playing.has(index)"
            :src="youtubeEmbed(block.youtube)"
            title="Vídeo do YouTube"
            allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
            allowfullscreen
            referrerpolicy="strict-origin-when-cross-origin"
          />
          <button v-else class="poster" title="Assistir" @click="play(index)">
            <img :src="youtubeThumb(block.youtube)" alt="" loading="lazy" draggable="false" />
            <span class="play"><Icon name="play" :size="28" /></span>
          </button>
        </div>
        <figcaption>
          <span>{{ block.caption }}</span>
          <button class="yt" @click="openUrl(youtubeWatch(block.youtube))">
            Abrir no YouTube <Icon name="external" :size="13" />
          </button>
        </figcaption>
      </figure>
    </template>

    <Teleport to="body">
      <div v-if="lightbox" class="lightbox" @click.self="lightbox = null">
        <img :src="lightbox.urls[lightbox.index]" alt="" draggable="false" />
        <button class="close" title="Fechar (Esc)" @click="lightbox = null"><Icon name="x" :size="20" /></button>
        <template v-if="lightbox.urls.length > 1">
          <button class="nav prev" title="Anterior" @click="step(-1)"><Icon name="back" :size="22" /></button>
          <button class="nav next" title="Próxima" @click="step(1)"><Icon name="back" :size="22" /></button>
          <span class="count">{{ lightbox.index + 1 }} / {{ lightbox.urls.length }}</span>
        </template>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.showcase {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.card {
  padding: 18px 20px;
  border-radius: var(--radius);
  background: var(--bg-card);
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
}

.text h3 {
  margin: 0 0 8px;
  font-size: 18px;
  color: var(--page-accent, var(--text));
}

.text p {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-dim);
  white-space: pre-line;
  user-select: text;
}

.media {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

figcaption {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 0 4px;
  font-size: 13px;
  color: var(--text-dim);
}

.shot,
.poster,
.thumb {
  display: block;
  width: 100%;
  padding: 0;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  overflow: hidden;
  background: var(--bg-card);
  box-shadow: var(--shadow);
  cursor: zoom-in;
}

.shot img {
  display: block;
  width: 100%;
  max-height: 560px;
  object-fit: cover;
}

.gallery-card {
  padding: 12px;
}

.gallery {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 8px;
}

.gallery.few {
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
}

.thumb {
  aspect-ratio: 16 / 10;
  border-radius: 10px;
  box-shadow: none;
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.2s ease;
}

.thumb:hover img {
  transform: scale(1.04);
}

.video {
  position: relative;
  aspect-ratio: 16 / 9;
  border-radius: var(--radius);
  overflow: hidden;
  border: 1px solid var(--line);
  background: #000;
  box-shadow: var(--shadow);
}

.video iframe,
.poster {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  border: 0;
  border-radius: 0;
  cursor: pointer;
}

.poster img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  filter: brightness(0.8);
  transition: filter 0.2s ease;
}

.poster:hover img {
  filter: brightness(1);
}

.play {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 68px;
  height: 68px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  background: var(--page-accent, var(--lime));
  color: var(--ink);
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.45);
}

.yt {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-faint);
  white-space: nowrap;
}

.yt:hover {
  color: var(--text);
}

.lightbox {
  position: fixed;
  inset: 0;
  z-index: 200;
  display: grid;
  place-items: center;
  padding: 48px 72px;
  background: rgba(6, 10, 11, 0.92);
}

.lightbox img {
  max-width: 100%;
  max-height: 100%;
  border-radius: 12px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
}

.lightbox button {
  position: absolute;
  width: 44px;
  height: 44px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  color: #fff;
  background: rgba(255, 255, 255, 0.1);
}

.lightbox button:hover {
  background: rgba(255, 255, 255, 0.2);
}

.close {
  top: 16px;
  right: 16px;
}

.nav {
  top: 50%;
  transform: translateY(-50%);
}

.prev {
  left: 16px;
}

.next {
  right: 16px;
}

.next :deep(svg) {
  transform: rotate(180deg);
}

.count {
  position: absolute;
  bottom: 16px;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.75);
}
</style>

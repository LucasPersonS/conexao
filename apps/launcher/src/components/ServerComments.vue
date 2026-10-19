<script setup lang="ts">
/**
 * Comentários dos jogadores na página do servidor. Comenta quem tem whitelist aprovada
 * no servidor ou conta cadastrada pelo Discord; a equipe do servidor apaga pelo painel
 * (ou aqui mesmo) e o autor pode apagar o próprio comentário.
 */
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { community, errorMessage, type CommentPage, type ServerComment, type ServerSummary } from '../lib/api'
import { formatNumber, timeAgo } from '../lib/format'
import { setStarCount } from '../lib/stars'
import { openProfile, store } from '../lib/store'
import Icon from './Icon.vue'

const MAX = 500

const props = defineProps<{ server: ServerSummary }>()

const page = ref<CommentPage | null>(null)
const loadError = ref<string | null>(null)
const text = ref('')
const sending = ref(false)
const sendError = ref<string | null>(null)
const confirming = ref<string | null>(null)
const focused = ref(false)
const box = ref<HTMLTextAreaElement | null>(null)
/** Cabeças de skin que não carregaram (mostra a inicial no lugar). */
const broken = reactive(new Set<string>())
const removing = ref<string | null>(null)

async function load(slug: string) {
  loadError.value = null
  try {
    const fresh = await community.comments(slug)
    if (slug !== props.server.slug) return
    page.value = fresh
    setStarCount(slug, fresh.stars)
  } catch (e) {
    if (slug === props.server.slug) loadError.value = errorMessage(e)
  }
}

watch(
  () => props.server.slug,
  (slug) => {
    page.value = null
    text.value = ''
    sendError.value = null
    confirming.value = null
    load(slug)
  },
  { immediate: true },
)

const length = computed(() => text.value.trim().length)
const remaining = computed(() => MAX - length.value)
/** Anel de caracteres: 0 a 1. */
const fill = computed(() => Math.min(1, length.value / MAX))
const RING = 2 * Math.PI * 9

/** O campo cresce com o texto (até ~8 linhas). */
function grow() {
  const el = box.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, 220)}px`
}
const canSend = computed(() => !sending.value && length.value >= 2 && length.value <= MAX)

async function send() {
  if (!canSend.value || !page.value) return
  sending.value = true
  sendError.value = null
  try {
    const comment = await community.comment(props.server.slug, text.value)
    page.value.comments.unshift(comment)
    page.value.total += 1
    text.value = ''
    await nextTick()
    grow()
  } catch (e) {
    sendError.value = errorMessage(e)
  } finally {
    sending.value = false
  }
}

async function remove(comment: ServerComment) {
  if (confirming.value !== comment.id) {
    confirming.value = comment.id
    return
  }
  removing.value = comment.id
  try {
    await community.deleteComment(props.server.slug, comment.id)
    if (page.value) {
      page.value.comments = page.value.comments.filter((c) => c.id !== comment.id)
      page.value.total = Math.max(0, page.value.total - 1)
    }
  } catch (e) {
    sendError.value = errorMessage(e)
  } finally {
    removing.value = null
    confirming.value = null
  }
}

const ROLE_LABEL: Record<string, string> = { owner: 'Dono', admin: 'Admin', moderator: 'Moderador' }
const initial = (name: string) => name.charAt(0).toUpperCase()
/** Cabeça da skin (2x para telas nítidas). */
const head = (uuid: string) => `https://mc-heads.net/avatar/${uuid}/72`
const when = (iso: string) => timeAgo(new Date(iso).getTime())
const fullDate = (iso: string) => new Date(iso).toLocaleString('pt-BR', { dateStyle: 'short', timeStyle: 'short' })
</script>

<template>
  <section class="card comments">
    <h3>
      <Icon name="message" :size="18" /> Comentários
      <span v-if="page?.total" class="count">{{ formatNumber(page.total) }}</span>
    </h3>

    <p v-if="loadError" class="err">
      Não foi possível carregar os comentários. <button class="link" @click="load(server.slug)">Tentar de novo</button>
    </p>
    <p v-else-if="!page" class="muted">Carregando…</p>

    <template v-else>
      <form
        v-if="page.viewer.can_comment"
        class="composer"
        :class="{ focused, filled: length > 0, over: remaining < 0 }"
        @submit.prevent="send"
      >
        <img v-if="store.me" class="me" :src="head(store.me.uuid)" :alt="store.me.name" draggable="false" />
        <div class="field">
          <textarea
            ref="box"
            v-model="text"
            :maxlength="MAX"
            rows="1"
            aria-label="Escreva um comentário"
            placeholder="Conte o que achou do servidor: a comunidade, a staff, o RP…"
            @input="grow"
            @focus="focused = true"
            @blur="focused = false"
            @keydown.ctrl.enter.prevent="send"
          />
          <div class="tools">
            <span class="hint"><kbd>Ctrl</kbd> + <kbd>Enter</kbd> para enviar · sem links</span>
            <span
              class="ring"
              :class="{ warn: remaining <= 50, bad: remaining < 0 }"
              :title="`${length}/${MAX} caracteres`"
              aria-hidden="true"
            >
              <svg viewBox="0 0 22 22" width="22" height="22">
                <circle cx="11" cy="11" r="9" class="track" />
                <circle cx="11" cy="11" r="9" class="bar" :stroke-dasharray="RING" :stroke-dashoffset="RING * (1 - fill)" />
              </svg>
              <span v-if="remaining <= 50" class="left">{{ remaining }}</span>
            </span>
            <button class="send" :disabled="!canSend" :title="sending ? 'Enviando…' : 'Comentar'">
              <span>{{ sending ? 'Enviando…' : 'Comentar' }}</span>
              <Icon name="arrow" :size="16" />
            </button>
          </div>
        </div>
      </form>
      <p v-else class="locked"><Icon name="lock" :size="16" /> {{ page.viewer.reason }}</p>
      <p v-if="sendError" class="err">{{ sendError }}</p>

      <p v-if="!page.comments.length" class="muted empty">
        {{ page.viewer.can_comment ? 'Ninguém comentou ainda. Seja o primeiro!' : 'Ninguém comentou ainda.' }}
      </p>
      <ul v-else class="list">
        <li v-for="c in page.comments" :key="c.id" class="comment" :class="{ mine: c.mine, team: c.author_role }">
          <button class="avatar" :title="`Ver perfil de ${c.author_name}`" @click="openProfile(c.author_name)">
            <img
              v-if="!broken.has(c.author_uuid)"
              :src="head(c.author_uuid)"
              :alt="c.author_name"
              loading="lazy"
              draggable="false"
              @error="broken.add(c.author_uuid)"
            />
            <span v-else aria-hidden="true">{{ initial(c.author_name) }}</span>
          </button>
          <div class="content">
            <header>
              <button class="author" :title="`Ver perfil de ${c.author_name}`" @click="openProfile(c.author_name)">{{ c.author_name }}</button>
              <span v-if="c.author_role" class="role">{{ ROLE_LABEL[c.author_role] }}</span>
              <span v-if="c.mine" class="you">você</span>
              <time class="muted" :datetime="c.created_at" :title="fullDate(c.created_at)">{{ when(c.created_at) }}</time>
              <button
                v-if="c.can_delete"
                class="del"
                :class="{ confirm: confirming === c.id }"
                :disabled="removing === c.id"
                :title="c.mine ? 'Apagar meu comentário' : 'Apagar comentário (equipe do servidor)'"
                @click="remove(c)"
                @blur="confirming === c.id && (confirming = null)"
              >
                <Icon name="trash" :size="14" />
                <span v-if="confirming === c.id">Apagar?</span>
              </button>
            </header>
            <p>{{ c.body }}</p>
          </div>
        </li>
      </ul>
      <p v-if="page.has_more" class="muted more">
        Mostrando os {{ page.comments.length }} comentários mais recentes de {{ formatNumber(page.total) }}.
      </p>
    </template>
  </section>
</template>

<style scoped>
.comments {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

h3 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 18px;
}

h3 :deep(svg) {
  color: var(--lime);
}

.count {
  font-size: 12px;
  font-weight: 800;
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--lime-soft);
  border: 1px solid var(--lime-line);
  color: var(--lime);
}

/* Campo de comentário */
.composer {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  padding: 12px;
  border-radius: 16px;
  background:
    linear-gradient(var(--bg-soft), var(--bg-soft)) padding-box,
    linear-gradient(135deg, var(--line-strong), var(--line)) border-box;
  border: 1px solid transparent;
  transition: box-shadow 0.18s, background 0.18s;
}

.composer.focused {
  background:
    linear-gradient(var(--bg-input), var(--bg-input)) padding-box,
    linear-gradient(135deg, var(--lime), rgba(183, 254, 0, 0.25) 60%, var(--line)) border-box;
  box-shadow: 0 0 0 4px var(--lime-soft), 0 12px 30px rgba(0, 0, 0, 0.35);
}

.composer.over {
  background:
    linear-gradient(var(--bg-input), var(--bg-input)) padding-box,
    linear-gradient(135deg, var(--bad), var(--line)) border-box;
}

.me {
  flex: none;
  width: 38px;
  height: 38px;
  border-radius: 10px;
  image-rendering: pixelated;
  box-shadow: 0 0 0 2px var(--bg-soft), 0 0 0 3px var(--line-strong);
  transition: box-shadow 0.18s;
}

.composer.focused .me {
  box-shadow: 0 0 0 2px var(--bg-input), 0 0 0 3px var(--lime);
}

.field {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

textarea {
  width: 100%;
  min-height: 38px;
  max-height: 220px;
  resize: none;
  padding: 8px 2px;
  border: 0;
  outline: none;
  background: transparent;
  font: inherit;
  font-size: 14px;
  line-height: 1.55;
  overflow-y: auto;
}

textarea:focus-visible {
  outline: none;
}

.tools {
  display: flex;
  align-items: center;
  gap: 10px;
  padding-top: 8px;
  border-top: 1px dashed var(--line);
  opacity: 0.75;
  transition: opacity 0.18s;
}

.composer.focused .tools,
.composer.filled .tools {
  opacity: 1;
}

.hint {
  flex: 1;
  font-size: 12px;
  color: var(--text-faint);
}

kbd {
  padding: 0 5px;
  border-radius: 5px;
  border: 1px solid var(--line-strong);
  border-bottom-width: 2px;
  font: inherit;
  font-size: 11px;
  color: var(--text-dim);
}

.ring {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  font-weight: 700;
  color: var(--text-dim);
}

.ring svg {
  transform: rotate(-90deg);
}

.ring circle {
  fill: none;
  stroke-width: 2.5;
}

.ring .track {
  stroke: var(--line);
}

.ring .bar {
  stroke: var(--lime);
  stroke-linecap: round;
  transition: stroke-dashoffset 0.15s, stroke 0.15s;
}

.ring.warn .bar {
  stroke: var(--warn);
}

.ring.warn .left {
  color: var(--warn);
}

.ring.bad .bar {
  stroke: var(--bad);
}

.ring.bad .left {
  color: var(--bad);
}

.send {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 34px;
  padding: 0 14px;
  border-radius: 10px;
  background: var(--lime);
  color: var(--ink);
  font-weight: 700;
  font-size: 13px;
  transition: background 0.15s, box-shadow 0.15s, transform 0.1s;
}

.send:hover:not(:disabled) {
  background: var(--lime-hi);
  box-shadow: var(--lime-glow);
}

.send:active:not(:disabled) {
  transform: translateY(1px);
}

.send:disabled {
  opacity: 1;
  background: var(--lime-soft);
  color: rgba(183, 254, 0, 0.55);
}

.locked {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  padding: 10px 12px;
  border-radius: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  font-size: 13px;
  line-height: 1.45;
  color: var(--text-dim);
}

.locked :deep(svg) {
  flex: none;
  color: var(--text-faint);
}

.list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
}

.comment {
  display: grid;
  grid-template-columns: 34px 1fr;
  gap: 12px;
  padding: 12px 0;
  border-top: 1px solid var(--line);
}

.avatar {
  width: 34px;
  height: 34px;
  padding: 0;
  display: grid;
  place-items: center;
  overflow: hidden;
  border-radius: 10px;
  background: var(--bg-hover);
  border: 1px solid var(--line);
  font-family: var(--font-display);
  font-weight: 700;
  color: var(--text-dim);
  transition: transform 0.12s, box-shadow 0.15s;
}

.avatar img {
  width: 100%;
  height: 100%;
  image-rendering: pixelated;
}

.avatar:hover {
  transform: translateY(-1px);
  box-shadow: 0 0 0 2px var(--lime-line);
}

.team .avatar {
  border-color: var(--lime-line);
  box-shadow: 0 0 0 1px var(--lime-line);
}

.author {
  padding: 0;
  font-weight: 700;
  color: var(--text);
}

.author:hover {
  color: var(--lime);
  text-decoration: underline;
  text-underline-offset: 3px;
}

.content {
  min-width: 0;
}

.content header {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  font-size: 14px;
}

.role,
.you {
  font-size: 11px;
  font-weight: 800;
  padding: 1px 8px;
  border-radius: 999px;
}

.role {
  background: var(--lime);
  color: var(--ink);
}

.you {
  background: var(--bg-hover);
  border: 1px solid var(--line);
  color: var(--text-dim);
}

time {
  font-size: 12px;
}

.del {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 26px;
  padding: 0 8px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 700;
  color: var(--text-faint);
}

.del:hover,
.del.confirm {
  color: var(--bad);
  background: var(--bad-bg);
}

.content p {
  margin: 4px 0 0;
  font-size: 14px;
  line-height: 1.55;
  color: var(--text-dim);
  white-space: pre-line;
  overflow-wrap: anywhere;
  user-select: text;
}

.muted {
  color: var(--text-faint);
}

.empty,
.more {
  margin: 0;
  font-size: 13px;
}

.err {
  margin: 0;
  font-size: 13px;
  color: var(--bad);
}

.link {
  font-weight: 700;
  color: var(--lime);
}
</style>

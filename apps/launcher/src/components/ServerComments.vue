<script setup lang="ts">
/**
 * Comentários dos jogadores na página do servidor. Comenta quem tem whitelist aprovada
 * no servidor ou conta cadastrada pelo Discord; a equipe do servidor apaga pelo painel
 * (ou aqui mesmo) e o autor pode apagar o próprio comentário.
 */
import { computed, ref, watch } from 'vue'
import { community, errorMessage, type CommentPage, type ServerComment, type ServerSummary } from '../lib/api'
import { formatNumber, timeAgo } from '../lib/format'
import { setStarCount } from '../lib/stars'
import Icon from './Icon.vue'

const MAX = 500

const props = defineProps<{ server: ServerSummary }>()

const page = ref<CommentPage | null>(null)
const loadError = ref<string | null>(null)
const text = ref('')
const sending = ref(false)
const sendError = ref<string | null>(null)
const confirming = ref<string | null>(null)
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
      <form v-if="page.viewer.can_comment" class="form" @submit.prevent="send">
        <textarea
          v-model="text"
          :maxlength="MAX"
          rows="3"
          placeholder="Conte o que achou do servidor: a comunidade, a staff, o RP…"
          @keydown.ctrl.enter.prevent="send"
        />
        <div class="form-foot">
          <span class="hint" :class="{ over: length > MAX }">{{ length }}/{{ MAX }} · sem links</span>
          <button class="btn btn-primary" :disabled="!canSend">{{ sending ? 'Enviando…' : 'Comentar' }}</button>
        </div>
      </form>
      <p v-else class="locked"><Icon name="lock" :size="16" /> {{ page.viewer.reason }}</p>
      <p v-if="sendError" class="err">{{ sendError }}</p>

      <p v-if="!page.comments.length" class="muted empty">
        {{ page.viewer.can_comment ? 'Ninguém comentou ainda. Seja o primeiro!' : 'Ninguém comentou ainda.' }}
      </p>
      <ul v-else class="list">
        <li v-for="c in page.comments" :key="c.id" class="comment" :class="{ mine: c.mine, team: c.author_role }">
          <span class="avatar" aria-hidden="true">{{ initial(c.author_name) }}</span>
          <div class="content">
            <header>
              <b>{{ c.author_name }}</b>
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

.form {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

textarea {
  width: 100%;
  resize: vertical;
  min-height: 76px;
  font: inherit;
  font-size: 14px;
  line-height: 1.5;
  padding: 10px 12px;
}

.form-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.hint {
  font-size: 12px;
  color: var(--text-faint);
}

.hint.over {
  color: var(--bad);
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
  display: grid;
  place-items: center;
  border-radius: 10px;
  background: var(--bg-hover);
  border: 1px solid var(--line);
  font-family: var(--font-display);
  font-weight: 700;
  color: var(--text-dim);
}

.team .avatar {
  background: var(--lime-soft);
  border-color: var(--lime-line);
  color: var(--lime);
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

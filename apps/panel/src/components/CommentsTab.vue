<script setup lang="ts">
/**
 * Comentários dos jogadores (feitos na página do servidor no launcher). A equipe do
 * servidor apaga o que for ofensivo, spam ou fora das regras; cada remoção fica no Histórico.
 */
import { computed, onMounted, ref } from 'vue'
import { api, formatDate, message, type CommentPage, type Server, type ServerComment } from '../lib/api'

const props = defineProps<{ server: Server }>()
const emit = defineEmits<{ stars: [count: number] }>()

const page = ref<CommentPage | null>(null)
const comments = ref<ServerComment[]>([])
const error = ref<string | null>(null)
const loading = ref(false)
const busy = ref<string | null>(null)
const filter = ref('')

async function load(more = false) {
  loading.value = true
  error.value = null
  try {
    const before = more ? comments.value[comments.value.length - 1]?.created_at : undefined
    const data = await api.comments(props.server.slug, before)
    page.value = data
    comments.value = more ? [...comments.value, ...data.comments] : data.comments
    emit('stars', data.stars)
  } catch (e) {
    error.value = message(e)
  } finally {
    loading.value = false
  }
}
onMounted(() => load())

const shown = computed(() => {
  const q = filter.value.trim().toLowerCase()
  if (!q) return comments.value
  return comments.value.filter((c) => c.author_name.toLowerCase().includes(q) || c.body.toLowerCase().includes(q))
})

async function remove(comment: ServerComment) {
  const preview = comment.body.length > 80 ? `${comment.body.slice(0, 80)}…` : comment.body
  if (!window.confirm(`Apagar o comentário de ${comment.author_name}?\n\n"${preview}"\n\nO jogador não é avisado; a remoção fica no Histórico.`)) return
  busy.value = comment.id
  error.value = null
  try {
    await api.deleteComment(props.server.slug, comment.id)
    comments.value = comments.value.filter((c) => c.id !== comment.id)
    if (page.value) page.value.total = Math.max(0, page.value.total - 1)
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = null
  }
}

const ROLE_LABEL: Record<string, string> = { owner: 'Dono', admin: 'Admin', moderator: 'Moderador' }
</script>

<template>
  <div class="stack">
    <section class="card">
      <div class="head">
        <div>
          <h2>Comentários dos jogadores</h2>
          <p class="muted lead">
            Aparecem na página do servidor no launcher. Podem comentar jogadores com whitelist aprovada aqui ou com conta
            cadastrada pelo Discord (até 3 comentários por dia, sem links). Apague o que for ofensivo, spam ou contra as
            regras: o jogador não é avisado e a remoção fica registrada no Histórico.
          </p>
        </div>
        <div v-if="page" class="kpis">
          <div>
            <b>{{ page.total.toLocaleString('pt-BR') }}</b>
            <span>{{ page.total === 1 ? 'comentário' : 'comentários' }}</span>
          </div>
          <div>
            <b>★ {{ page.stars.toLocaleString('pt-BR') }}</b>
            <span>{{ page.stars === 1 ? 'estrela' : 'estrelas' }}</span>
          </div>
        </div>
      </div>

      <div class="tools">
        <input v-model="filter" class="input" placeholder="Filtrar por nick ou texto" />
        <button class="btn" :disabled="loading" @click="load()">Atualizar</button>
      </div>
      <p v-if="error" class="error">{{ error }}</p>
      <p v-if="page && !comments.length" class="muted">Nenhum comentário ainda.</p>
      <p v-else-if="page && !shown.length" class="muted">Nenhum comentário carregado combina com o filtro.</p>

      <ul class="list">
        <li v-for="c in shown" :key="c.id">
          <div class="content">
            <div class="who">
              <b>{{ c.author_name }}</b>
              <span v-if="c.author_role" class="badge brand">{{ ROLE_LABEL[c.author_role] }}</span>
              <span v-if="c.author_offline" class="badge" title="Conta não original, cadastrada pelo Discord">Discord</span>
              <span class="muted small">{{ formatDate(c.created_at) }}</span>
            </div>
            <p class="body">{{ c.body }}</p>
          </div>
          <div class="item-actions">
            <button class="btn btn-sm btn-danger" :disabled="busy === c.id || !c.can_delete" @click="remove(c)">
              {{ busy === c.id ? 'Apagando…' : 'Apagar' }}
            </button>
          </div>
        </li>
      </ul>
      <button v-if="page?.has_more" class="btn more" :disabled="loading" @click="load(true)">
        {{ loading ? 'Carregando…' : 'Carregar mais antigos' }}
      </button>
    </section>
  </div>
</template>

<style scoped>
.stack {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.head {
  display: flex;
  justify-content: space-between;
  gap: 20px;
  align-items: flex-start;
}

.lead {
  margin: 4px 0 14px;
  line-height: 1.5;
  max-width: 680px;
}

.kpis {
  display: flex;
  gap: 10px;
}

.kpis div {
  display: flex;
  flex-direction: column;
  padding: 10px 14px;
  border-radius: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  min-width: 110px;
}

.kpis b {
  font-family: var(--font-display);
  font-size: 20px;
  color: var(--lime);
}

.kpis span {
  font-size: 12px;
  opacity: 0.85;
}

.tools {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}

.tools .input {
  flex: 1;
  max-width: 360px;
}

.list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.list li {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  padding: 12px 0;
  border-top: 1px solid var(--line);
}

.content {
  min-width: 0;
}

.who {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.small {
  font-size: 12px;
}

.body {
  margin: 6px 0 0;
  white-space: pre-line;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.item-actions {
  display: flex;
  align-items: flex-start;
}

.more {
  margin-top: 8px;
}

@media (max-width: 760px) {
  .head {
    flex-direction: column;
  }
}
</style>

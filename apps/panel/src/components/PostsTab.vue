<script setup lang="ts">
/** Notícias e eventos: aparecem na página do servidor no launcher (eventos também na home). */
import { computed, onMounted, reactive, ref } from 'vue'
import { api, formatDate, message, type PostInput, type Server, type ServerPost } from '../lib/api'

const props = defineProps<{ server: Server }>()

const news = ref<ServerPost[]>([])
const events = ref<ServerPost[]>([])
const error = ref<string | null>(null)
const busy = ref(false)
const editing = ref<string | null>(null)
const form = reactive({ kind: 'event' as PostInput['kind'], title: '', body: '', when: '' })

async function load() {
  try {
    const data = await api.posts(props.server.slug)
    news.value = data.news
    events.value = data.events
  } catch (e) {
    error.value = message(e)
  }
}
onMounted(load)

/** `datetime-local` trabalha no fuso do navegador; a API recebe UTC. */
const toLocalInput = (iso: string) => {
  const d = new Date(iso)
  return new Date(d.getTime() - d.getTimezoneOffset() * 60_000).toISOString().slice(0, 16)
}
const minWhen = computed(() => toLocalInput(new Date().toISOString()))

function reset() {
  editing.value = null
  Object.assign(form, { kind: 'event', title: '', body: '', when: '' })
}

function edit(post: ServerPost) {
  editing.value = post.id
  Object.assign(form, { kind: post.kind, title: post.title, body: post.body, when: post.starts_at ? toLocalInput(post.starts_at) : '' })
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

async function submit() {
  busy.value = true
  error.value = null
  const input: PostInput = {
    kind: form.kind,
    title: form.title.trim(),
    body: form.body.trim(),
    starts_at: form.kind === 'event' && form.when ? new Date(form.when).toISOString() : null,
  }
  try {
    if (editing.value) await api.updatePost(props.server.slug, editing.value, input)
    else await api.createPost(props.server.slug, input)
    reset()
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

async function remove(post: ServerPost) {
  if (!window.confirm(`Apagar "${post.title}"?`)) return
  busy.value = true
  try {
    await api.deletePost(props.server.slug, post.id)
    if (editing.value === post.id) reset()
    await load()
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}

const whenText = (iso: string) =>
  new Date(iso).toLocaleString('pt-BR', { weekday: 'short', day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' })
</script>

<template>
  <div class="stack">
    <section class="card">
      <h2>{{ editing ? 'Editar publicação' : 'Nova publicação' }}</h2>
      <p class="muted lead">
        Aparece na página do servidor no launcher. Eventos também entram em "Próximos eventos" na home, e os jogadores
        podem pedir um aviso 10 minutos antes.
      </p>
      <form class="form" @submit.prevent="submit">
        <div class="kinds">
          <button type="button" class="btn btn-sm" :class="{ on: form.kind === 'event' }" @click="form.kind = 'event'">Evento</button>
          <button type="button" class="btn btn-sm" :class="{ on: form.kind === 'news' }" @click="form.kind = 'news'">Notícia</button>
        </div>
        <div class="row">
          <label class="field grow">
            Título
            <input v-model="form.title" class="input" maxlength="80" :placeholder="form.kind === 'event' ? 'Coroação do novo rei' : 'Atualização 1.3: novas profissões'" required />
          </label>
          <label v-if="form.kind === 'event'" class="field">
            Data e hora
            <input v-model="form.when" class="input" type="datetime-local" :min="minWhen" required />
          </label>
        </div>
        <label class="field">
          Texto <span class="muted small">(opcional, até 1000 caracteres)</span>
          <textarea v-model="form.body" class="input area" maxlength="1000" rows="4" placeholder="Onde vai ser, o que levar, quem pode participar…" />
        </label>
        <div class="actions">
          <button v-if="editing" type="button" class="btn" @click="reset">Cancelar</button>
          <button class="btn btn-primary" :disabled="busy || form.title.trim().length < 3">{{ editing ? 'Salvar' : 'Publicar' }}</button>
        </div>
        <p v-if="error" class="error">{{ error }}</p>
      </form>
    </section>

    <section class="card">
      <h2>Próximos eventos</h2>
      <p v-if="!events.length" class="muted">Nenhum evento marcado.</p>
      <ul class="list">
        <li v-for="e in events" :key="e.id">
          <div>
            <span class="badge brand">{{ whenText(e.starts_at!) }}</span>
            <b>{{ e.title }}</b>
            <p v-if="e.body" class="muted body">{{ e.body }}</p>
          </div>
          <div class="item-actions">
            <button class="btn btn-sm" @click="edit(e)">Editar</button>
            <button class="btn btn-sm btn-danger" :disabled="busy" @click="remove(e)">Apagar</button>
          </div>
        </li>
      </ul>
    </section>

    <section class="card">
      <h2>Notícias</h2>
      <p v-if="!news.length" class="muted">Nenhuma notícia publicada.</p>
      <ul class="list">
        <li v-for="n in news" :key="n.id">
          <div>
            <b>{{ n.title }}</b>
            <span class="muted small"> · {{ formatDate(n.created_at) }}<template v-if="n.author_name"> · {{ n.author_name }}</template></span>
            <p v-if="n.body" class="muted body">{{ n.body }}</p>
          </div>
          <div class="item-actions">
            <button class="btn btn-sm" @click="edit(n)">Editar</button>
            <button class="btn btn-sm btn-danger" :disabled="busy" @click="remove(n)">Apagar</button>
          </div>
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.stack {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.lead {
  margin: 4px 0 14px;
  line-height: 1.5;
}

.form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.kinds {
  display: flex;
  gap: 6px;
}

.kinds .on {
  background: var(--brand);
  color: var(--ink);
}

.row {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
}

.grow {
  flex: 1;
  min-width: 260px;
}

.area {
  resize: vertical;
  font: inherit;
  padding: 10px 12px;
  height: auto;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.small {
  font-size: 12px;
  font-weight: 400;
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

.list li:first-child {
  border-top: none;
}

.list .badge {
  margin-right: 8px;
}

.body {
  margin: 6px 0 0;
  white-space: pre-line;
  line-height: 1.5;
}

.item-actions {
  display: flex;
  gap: 6px;
  align-items: flex-start;
}
</style>

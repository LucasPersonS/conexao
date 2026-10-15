<script setup lang="ts">
/** Eventos e notícias publicados pela equipe do servidor (painel → aqui). */
import { computed, ref, watch } from 'vue'
import { errorMessage, posts, type ServerPost, type ServerSummary } from '../lib/api'
import { formatWhen, startsIn, timeAgo } from '../lib/format'
import { loadReminders, reminders, toggleReminder } from '../lib/reminders'
import { prefs } from '../lib/prefs'
import Icon from './Icon.vue'

const props = defineProps<{ server: ServerSummary }>()

// Cache curto por servidor: voltar à página não refaz a requisição.
const cache = new Map<string, { at: number; data: { news: ServerPost[]; events: ServerPost[] } }>()
const data = ref<{ news: ServerPost[]; events: ServerPost[] } | null>(null)
const error = ref<string | null>(null)
const expanded = ref(false)

watch(
  () => props.server.slug,
  async (slug) => {
    error.value = null
    expanded.value = false
    const hit = cache.get(slug)
    if (hit && Date.now() - hit.at < 120_000) {
      data.value = hit.data
    } else {
      data.value = null
      try {
        const fresh = await posts.of(slug)
        cache.set(slug, { at: Date.now(), data: fresh })
        if (slug === props.server.slug) data.value = fresh
      } catch (e) {
        error.value = errorMessage(e)
      }
    }
    loadReminders()
  },
  { immediate: true },
)

/** Notícias novas desde a última visita ficam marcadas. */
const lastSeen = computed(() => prefs.newsSeen?.[props.server.slug] ?? 0)
watch(data, (value) => {
  const newest = value?.news[0]?.created_at
  if (!newest) return
  window.setTimeout(() => {
    prefs.newsSeen = { ...(prefs.newsSeen ?? {}), [props.server.slug]: new Date(newest).getTime() }
  }, 1500)
})
const isNew = (p: ServerPost) => lastSeen.value > 0 && new Date(p.created_at).getTime() > lastSeen.value

const news = computed(() => (expanded.value ? data.value?.news : data.value?.news.slice(0, 3)) ?? [])
const empty = computed(() => !!data.value && !data.value.news.length && !data.value.events.length)
</script>

<template>
  <section v-if="data && !empty" class="card news">
    <template v-if="data.events.length">
      <h3><Icon name="calendar" :size="18" /> Próximos eventos</h3>
      <ul class="events">
        <li v-for="e in data.events" :key="e.id" class="event">
          <div class="when">
            <strong>{{ formatWhen(e.starts_at!) }}</strong>
            <span v-if="startsIn(e.starts_at!)" class="soon">{{ startsIn(e.starts_at!) }}</span>
          </div>
          <div class="what">
            <b>{{ e.title }}</b>
            <p v-if="e.body">{{ e.body }}</p>
          </div>
          <button
            class="remind"
            :class="{ on: reminders.ids.has(e.id) }"
            :title="reminders.ids.has(e.id) ? 'Não lembrar mais' : 'Avisar 10 minutos antes'"
            @click="toggleReminder({ id: e.id, title: e.title, starts_at: e.starts_at! }, server.name)"
          >
            <Icon name="bell" :size="16" /> {{ reminders.ids.has(e.id) ? 'Vou lembrar' : 'Lembrar' }}
          </button>
        </li>
      </ul>
    </template>

    <template v-if="data.news.length">
      <h3><Icon name="megaphone" :size="18" /> Novidades</h3>
      <article v-for="n in news" :key="n.id" class="post">
        <header>
          <b>{{ n.title }}</b>
          <span v-if="isNew(n)" class="new">Novo</span>
          <span class="muted">{{ timeAgo(new Date(n.created_at).getTime()) }}<template v-if="n.author_name"> · {{ n.author_name }}</template></span>
        </header>
        <p v-if="n.body">{{ n.body }}</p>
      </article>
      <button v-if="data.news.length > 3" class="more" @click="expanded = !expanded">
        {{ expanded ? 'Mostrar menos' : `Ver todas (${data.news.length})` }}
      </button>
    </template>
  </section>
  <p v-else-if="error" class="err">{{ error }}</p>
</template>

<style scoped>
.news {
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

.events {
  list-style: none;
  margin: 0 0 6px;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.event {
  display: grid;
  grid-template-columns: 130px 1fr auto;
  gap: 14px;
  align-items: center;
  padding: 12px 14px;
  border-radius: 14px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  border-left: 3px solid var(--lime);
}

.when {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 13px;
}

.soon {
  color: var(--lime);
  font-weight: 700;
  font-size: 12px;
}

.what b {
  font-size: 15px;
}

.what p,
.post p {
  margin: 4px 0 0;
  font-size: 14px;
  line-height: 1.5;
  color: var(--text-dim);
  white-space: pre-line;
}

.remind {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 0 12px;
  border-radius: 999px;
  font-size: 13px;
  font-weight: 700;
  border: 1px solid var(--line);
  background: var(--bg-hover);
  color: var(--text);
}

.remind:hover {
  border-color: var(--lime-line);
}

.remind.on {
  background: var(--lime);
  border-color: var(--lime);
  color: var(--ink);
}

.post {
  padding-top: 10px;
  border-top: 1px solid var(--line);
}

.post header {
  display: flex;
  align-items: baseline;
  gap: 8px;
  flex-wrap: wrap;
}

.post header .muted {
  margin-left: auto;
  font-size: 12px;
}

.new {
  font-size: 11px;
  font-weight: 800;
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--lime);
  color: var(--ink);
}

.more {
  align-self: flex-start;
  font-size: 13px;
  font-weight: 700;
  color: var(--lime);
}

.err {
  font-size: 13px;
  color: var(--text-dim);
}
</style>

<script lang="ts">
import type { UpcomingEvent } from '../lib/api'

// Uma requisição a cada 5 minutos, no máximo, entre idas e vindas da home.
let cache: { at: number; data: UpcomingEvent[] } | null = null
</script>

<script setup lang="ts">
/** Faixa "Próximos eventos" da home: seus servidores (favoritos e recentes) primeiro. */
import { computed, onMounted, ref } from 'vue'
import { posts } from '../lib/api'
import { formatWhen, startsIn } from '../lib/format'
import { prefs } from '../lib/prefs'
import { loadReminders, reminders, toggleReminder } from '../lib/reminders'
import { openServer } from '../lib/store'
import Icon from './Icon.vue'
import ServerIcon from './ServerIcon.vue'

const events = ref<UpcomingEvent[]>([])

onMounted(async () => {
  loadReminders()
  if (cache && Date.now() - cache.at < 300_000) {
    events.value = cache.data
    return
  }
  try {
    const data = await posts.upcoming()
    cache = { at: Date.now(), data }
    events.value = data
  } catch {
    // sem eventos: a faixa simplesmente não aparece
  }
})

const shown = computed(() => {
  const mine = new Set([...prefs.favorites, ...prefs.recent.map((r) => r.slug)])
  return [...events.value]
    .sort((a, b) => Number(mine.has(b.slug)) - Number(mine.has(a.slug)) || a.starts_at.localeCompare(b.starts_at))
    .slice(0, 4)
})
</script>

<template>
  <template v-if="shown.length">
    <div class="section-head">
      <h3>Próximos eventos</h3>
    </div>
    <div class="events">
      <article v-for="e in shown" :key="e.id" class="event" @click="openServer(e.slug)">
        <header>
          <ServerIcon :name="e.server_name" :url="e.icon_url" :size="32" />
          <span class="server">{{ e.server_name }}</span>
        </header>
        <b class="title">{{ e.title }}</b>
        <span class="when"><Icon name="calendar" :size="14" /> {{ formatWhen(e.starts_at) }}<em v-if="startsIn(e.starts_at)"> · {{ startsIn(e.starts_at) }}</em></span>
        <button
          class="remind"
          :class="{ on: reminders.ids.has(e.id) }"
          @click.stop="toggleReminder({ id: e.id, title: e.title, starts_at: e.starts_at }, e.server_name)"
        >
          <Icon name="bell" :size="14" /> {{ reminders.ids.has(e.id) ? 'Vou lembrar' : 'Lembrar' }}
        </button>
      </article>
    </div>
  </template>
</template>

<style scoped>
.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 10px;
}

h3 {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 0;
  font-size: 19px;
}

h3::before {
  content: '';
  width: 8px;
  height: 8px;
  background: var(--lime);
  box-shadow: 10px -6px 0 -1px rgba(183, 254, 0, 0.55), 0 0 10px rgba(183, 254, 0, 0.6);
  margin-right: 8px;
}

.events {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
}

.event {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 14px;
  border-radius: 16px;
  background: var(--bg-card);
  border: 1px solid var(--line);
  border-top: 2px solid var(--lime);
  cursor: pointer;
  transition: transform 0.15s, box-shadow 0.15s, border-color 0.15s;
}

.event:hover {
  transform: translateY(-2px);
  border-color: var(--lime-line);
  border-top-color: var(--lime);
  box-shadow: var(--shadow-pop);
}

header {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.server {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-dim);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.title {
  font-family: var(--font-display);
  font-size: 17px;
  font-weight: 600;
  line-height: 1.25;
}

.when {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: var(--text-dim);
}

.when em {
  font-style: normal;
  font-weight: 700;
  color: var(--brand-strong);
}

.remind {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 10px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 700;
  border: 1px solid var(--line);
  background: var(--bg-soft);
}

.remind:hover {
  border-color: var(--lime-line);
}

.remind.on {
  background: var(--lime);
  border-color: var(--lime);
  color: var(--ink);
}
</style>

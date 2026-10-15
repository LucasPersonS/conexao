<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import Icon from '../components/Icon.vue'
import SelectMenu from '../components/SelectMenu.vue'
import ServerCard from '../components/ServerCard.vue'
import { formatNumber } from '../lib/format'
import { prefs } from '../lib/prefs'
import { allTags, loadServers, openServer, sortServers, store, totalPlayers, type SortMode } from '../lib/store'

const search = ref('')
const tag = ref<string | null>(store.browseTag)
const sort = ref<SortMode>('players')
const onlyFavorites = ref(store.browseFavorites)
const hideOffline = ref(false)

watch(
  () => [store.browseTag, store.browseFavorites] as const,
  ([t, f]) => {
    tag.value = t
    onlyFavorites.value = f
  },
)

const tagOptions = computed(() => [
  { value: null, label: 'Todos os estilos', count: store.servers.length },
  ...allTags.value.map((t) => ({ value: t, label: t, count: store.servers.filter((s) => s.tags.includes(t)).length })),
])
const sortOptions: { value: SortMode; label: string }[] = [
  { value: 'players', label: 'Mais jogadores' },
  { value: 'ping', label: 'Menor latência' },
  { value: 'name', label: 'Nome (A–Z)' },
]

const results = computed(() => {
  const q = search.value.trim().toLowerCase()
  const list = store.servers.filter(
    (s) =>
      (!q || s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q) || s.tags.some((t) => t.toLowerCase().includes(q))) &&
      (!tag.value || s.tags.includes(tag.value)) &&
      (!onlyFavorites.value || prefs.favorites.includes(s.slug)) &&
      (!hideOffline.value || s.status.online),
  )
  return sortServers(list, sort.value)
})

const filtering = computed(() => !!search.value || !!tag.value || onlyFavorites.value || hideOffline.value)
function clear() {
  search.value = ''
  tag.value = null
  onlyFavorites.value = false
  hideOffline.value = false
}
</script>

<template>
  <div class="browse">
    <div class="head">
      <div>
        <h2>Servidores</h2>
        <p>{{ store.servers.length }} servidores parceiros · {{ formatNumber(totalPlayers) }} jogando agora</p>
      </div>
    </div>

    <div class="toolbar">
      <label class="search">
        <Icon name="search" :size="17" />
        <input v-model="search" placeholder="Buscar por nome, estilo ou descrição" />
        <button v-if="search" class="clear-x" title="Limpar" @click="search = ''"><Icon name="x" :size="14" /></button>
      </label>
      <SelectMenu v-model="tag" :options="tagOptions" icon="tag" :searchable="allTags.length > 8" />
      <SelectMenu v-model="sort" :options="sortOptions" icon="users" />
      <button class="toggle" :class="{ on: onlyFavorites }" :aria-pressed="onlyFavorites" @click="onlyFavorites = !onlyFavorites">
        <Icon name="heart" :size="16" /> Favoritos
      </button>
      <button class="toggle" :class="{ on: hideOffline }" :aria-pressed="hideOffline" @click="hideOffline = !hideOffline">
        Só online
      </button>
    </div>

    <p v-if="store.error" class="error">
      {{ store.error }} <button class="link" @click="loadServers">Tentar de novo</button>
    </p>
    <p v-else-if="store.loading && !store.servers.length" class="empty">Carregando servidores…</p>
    <div v-else-if="!results.length" class="empty-box">
      <strong>Nenhum servidor encontrado</strong>
      <span v-if="onlyFavorites && !prefs.favorites.length">Você ainda não favoritou nenhum servidor.</span>
      <span v-else>Tente outra busca ou outro estilo.</span>
      <button v-if="filtering" class="btn" @click="clear">Limpar filtros</button>
    </div>

    <div class="grid">
      <ServerCard v-for="server in results" :key="server.slug" :server="server" @open="openServer(server.slug)" />
    </div>
  </div>
</template>

<style scoped>
.browse {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

h2 {
  margin: 0 0 2px;
  font-size: 30px;
}

.head p {
  margin: 0;
  color: var(--text-dim);
  font-size: 14px;
}

.toolbar {
  position: sticky;
  top: -24px;
  z-index: 10;
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  padding: 10px 0;
  background: linear-gradient(var(--bg) 80%, transparent);
}

.search {
  flex: 1;
  min-width: 240px;
  display: flex;
  align-items: center;
  gap: 8px;
  height: 42px;
  padding: 0 12px;
  border-radius: 12px;
  background: var(--bg-card);
  border: 1px solid var(--line);
  color: var(--text-faint);
}

.search:focus-within {
  border-color: var(--lime);
  box-shadow: 0 0 0 3px var(--lime-soft);
  color: var(--lime);
}

.search input {
  flex: 1;
  border: none;
  outline: none;
  background: none;
  font-size: 14px;
}

.clear-x {
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--bg-hover);
}

.toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 42px;
  padding: 0 14px;
  border-radius: 12px;
  background: var(--bg-card);
  border: 1px solid var(--line);
  font-size: 14px;
  font-weight: 600;
  color: var(--text-dim);
}

.toggle:hover {
  border-color: var(--lime-line);
}

.toggle.on {
  background: var(--lime-soft);
  border-color: var(--lime);
  color: var(--lime);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 16px;
}

.empty {
  color: var(--text-dim);
  margin: 0;
}

.empty-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 48px 16px;
  border: 1px dashed var(--line);
  border-radius: var(--radius);
  color: var(--text-dim);
}

.empty-box strong {
  color: var(--text);
  font-size: 16px;
}

.error {
  color: var(--bad);
  margin: 0;
}

.link {
  color: var(--lime);
  font-weight: 600;
}
</style>

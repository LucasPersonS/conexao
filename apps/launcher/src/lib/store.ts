import { computed, reactive } from 'vue'
import { api, errorMessage, type ServerSummary } from './api'

export type View = 'home' | 'browse' | 'server' | 'settings' | 'profile'
export type SortMode = 'players' | 'name' | 'ping'

export const store = reactive({
  servers: [] as ServerSummary[],
  loading: false,
  error: null as string | null,
  view: 'home' as View,
  back: 'home' as View,
  slug: null as string | null,
  /** Abre a lista já filtrada (ex.: "Favoritos" na home). */
  browseFavorites: false,
  browseTag: null as string | null,
  /** Aviso rápido no rodapé (ex.: convite para servidor que não está na lista). */
  toast: null as string | null,
})

let toastTimer: number | undefined
export function showToast(text: string) {
  store.toast = text
  window.clearTimeout(toastTimer)
  toastTimer = window.setTimeout(() => (store.toast = null), 5000)
}

/** Abre a página do servidor de um convite (`conexao://servidor/<slug>`). */
export function openInvite(slug: string) {
  if (findServer(slug)) openServer(slug)
  else showToast('O servidor do convite ainda não está disponível no Conexão (pode estar em análise).')
}

export async function loadServers() {
  store.loading = true
  store.error = null
  try {
    store.servers = await api.servers()
  } catch (error) {
    store.error = errorMessage(error)
  } finally {
    store.loading = false
  }
}

export function go(view: Exclude<View, 'server'>, options: { favorites?: boolean; tag?: string | null } = {}) {
  store.browseFavorites = options.favorites ?? false
  store.browseTag = options.tag ?? null
  store.view = view
}

export function openServer(slug: string) {
  if (store.view !== 'server') store.back = store.view
  store.slug = slug
  store.view = 'server'
}

export function findServer(slug: string | null): ServerSummary | null {
  return store.servers.find((s) => s.slug === slug) ?? null
}

/** Online primeiro; depois pela ordem escolhida. */
export function sortServers(list: ServerSummary[], mode: SortMode): ServerSummary[] {
  return [...list].sort((a, b) => {
    if (a.status.online !== b.status.online) return a.status.online ? -1 : 1
    switch (mode) {
      case 'name':
        return a.name.localeCompare(b.name, 'pt-BR')
      case 'ping':
        return (a.status.latency_ms ?? 9999) - (b.status.latency_ms ?? 9999)
      default:
        return b.status.players_online - a.status.players_online || Number(b.featured) - Number(a.featured)
    }
  })
}

export const totalPlayers = computed(() => store.servers.reduce((sum, s) => sum + s.status.players_online, 0))
export const allTags = computed(() =>
  [...new Set(store.servers.flatMap((s) => s.tags))].sort((a, b) => a.localeCompare(b, 'pt-BR')),
)

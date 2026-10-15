import { reactive, watch } from 'vue'

/** Preferências locais do jogador (favoritos, histórico, boas-vindas). */
interface Prefs {
  favorites: string[]
  recent: { slug: string; at: number }[]
  onboarded: boolean
  /** Data da notícia mais nova já vista, por servidor (marca "Novo"). */
  newsSeen?: Record<string, number>
}

const KEY = 'conexao.prefs.v1'
/** Chave usada quando o launcher se chamava CENTRAL (lida uma vez, na migração). */
const LEGACY_KEY = 'central.prefs.v1'

function load(): Prefs {
  const base: Prefs = { favorites: [], recent: [], onboarded: false }
  try {
    const raw = localStorage.getItem(KEY) ?? localStorage.getItem(LEGACY_KEY)
    if (raw) return { ...base, ...JSON.parse(raw) }
  } catch {
    // armazenamento indisponível: segue com o padrão
  }
  return base
}

export const prefs = reactive<Prefs>(load())

watch(
  prefs,
  (value) => {
    try {
      localStorage.setItem(KEY, JSON.stringify(value))
    } catch {
      // sem armazenamento, as preferências valem só nesta sessão
    }
  },
  { deep: true },
)

export function isFavorite(slug: string): boolean {
  return prefs.favorites.includes(slug)
}

export function toggleFavorite(slug: string) {
  prefs.favorites = isFavorite(slug) ? prefs.favorites.filter((s) => s !== slug) : [...prefs.favorites, slug]
}

export function markPlayed(slug: string) {
  prefs.recent = [{ slug, at: Date.now() }, ...prefs.recent.filter((r) => r.slug !== slug)].slice(0, 10)
}

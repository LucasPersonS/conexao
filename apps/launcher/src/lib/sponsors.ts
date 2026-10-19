import { openUrl } from '@tauri-apps/plugin-opener'
import { sponsorsApi, type Sponsor, type SponsorPlacement } from './api'

/** Lista por posição, guardada por 5 minutos (evita uma requisição a cada troca de tela). */
const cache = new Map<SponsorPlacement, { at: number; list: Sponsor[] }>()
const TTL = 5 * 60_000

export async function loadSponsors(placement: SponsorPlacement): Promise<Sponsor[]> {
  const hit = cache.get(placement)
  if (hit && Date.now() - hit.at < TTL) return hit.list
  try {
    const list = await sponsorsApi.list(placement)
    cache.set(placement, { at: Date.now(), list })
    return list
  } catch {
    return hit?.list ?? []
  }
}

/** Ordem sorteada pelo peso (peso 3 aparece primeiro ~3x mais vezes que peso 1). */
export function weightedOrder(list: Sponsor[]): Sponsor[] {
  return list
    .map((s) => ({ s, key: Math.random() ** (1 / Math.max(1, s.weight)) }))
    .sort((a, b) => b.key - a.key)
    .map((x) => x.s)
}

/** Visualizações juntas e enviadas de uma vez (o servidor conta uma por pessoa por dia). */
const seen = new Set<string>()
const pending = new Map<SponsorPlacement, Set<string>>()
let flushTimer: number | undefined

export function markViewed(placement: SponsorPlacement, id: string) {
  const key = `${placement}:${id}`
  if (seen.has(key)) return
  seen.add(key)
  if (!pending.has(placement)) pending.set(placement, new Set())
  pending.get(placement)!.add(id)
  window.clearTimeout(flushTimer)
  flushTimer = window.setTimeout(flush, 3000)
}

function flush() {
  for (const [placement, ids] of pending) {
    if (ids.size) sponsorsApi.events(placement, [...ids]).catch(() => {})
  }
  pending.clear()
}

/** Só abre links https comuns (defesa extra: o servidor já valida ao cadastrar). */
export function safeLink(url: string): boolean {
  try {
    const parsed = new URL(url)
    return parsed.protocol === 'https:' && !parsed.username && !parsed.password
  } catch {
    return false
  }
}

export async function openSponsor(placement: SponsorPlacement, sponsor: Sponsor) {
  sponsorsApi.events(placement, [], sponsor.id).catch(() => {})
  if (safeLink(sponsor.link_url)) await openUrl(sponsor.link_url)
}

import { reactive } from 'vue'
import { community, errorMessage } from './api'
import { showToast, store } from './store'

/** Servidores em que o jogador deu estrela (vem da API, vale em qualquer computador). */
export const stars = reactive({
  mine: new Set<string>(),
  busy: new Set<string>(),
})

export async function loadStars() {
  try {
    stars.mine = new Set(await community.myStars())
  } catch {
    // sem a lista, os botões só começam desmarcados
  }
}

export function clearStars() {
  stars.mine = new Set()
}

/** Atualiza o total no card e na página do servidor sem recarregar a lista. */
export function setStarCount(slug: string, count: number) {
  const server = store.servers.find((s) => s.slug === slug)
  if (server) server.stars = count
}

export async function toggleStar(slug: string) {
  if (stars.busy.has(slug)) return
  const on = !stars.mine.has(slug)
  const server = store.servers.find((s) => s.slug === slug)
  const before = server?.stars ?? 0
  stars.busy.add(slug)
  // Resposta imediata; a API confirma o total logo depois.
  if (on) stars.mine.add(slug)
  else stars.mine.delete(slug)
  setStarCount(slug, Math.max(0, before + (on ? 1 : -1)))
  try {
    const result = await community.star(slug, on)
    if (result.starred) stars.mine.add(slug)
    else stars.mine.delete(slug)
    setStarCount(slug, result.stars)
  } catch (error) {
    if (on) stars.mine.delete(slug)
    else stars.mine.add(slug)
    setStarCount(slug, before)
    showToast(`Não foi possível ${on ? 'dar' : 'tirar'} a estrela: ${errorMessage(error)}`)
  } finally {
    stars.busy.delete(slug)
  }
}

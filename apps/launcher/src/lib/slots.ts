/** "Avise-me quando abrir vaga": o Rust pinga só aquele servidor e avisa. */
import { reactive } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { api, errorMessage, type ServerSummary } from './api'
import { showToast } from './store'

export const slots = reactive({ watching: new Set<string>() })

let started = false
function start() {
  if (started) return
  started = true
  api.watchedSlots().then((list) => (slots.watching = new Set(list))).catch(() => {})
  listen<{ slug: string; open: boolean }>('slot', (event) => {
    const next = new Set(slots.watching)
    next.delete(event.payload.slug)
    slots.watching = next
  }).catch(() => {})
}

export async function toggleSlotWatch(server: ServerSummary) {
  start()
  const next = new Set(slots.watching)
  try {
    if (next.has(server.slug)) {
      await api.unwatchSlot(server.slug)
      next.delete(server.slug)
    } else {
      await api.watchSlot(server.slug, server.name, server.address)
      next.add(server.slug)
      showToast('Pode fazer outra coisa: avisamos quando abrir uma vaga.')
    }
    slots.watching = next
  } catch (e) {
    showToast(errorMessage(e))
  }
}

export const isFull = (server: ServerSummary) =>
  server.status.online && server.status.players_max > 0 && server.status.players_online >= server.status.players_max

/** Eventos marcados com "Lembrar" (o Rust guarda e avisa 10 min antes). */
import { reactive } from 'vue'
import { api, errorMessage } from './api'
import { showToast } from './store'

export const reminders = reactive({ ids: new Set<string>(), loaded: false })

export async function loadReminders() {
  if (reminders.loaded) return
  reminders.loaded = true
  try {
    reminders.ids = new Set(await api.reminders())
  } catch {
    reminders.loaded = false
  }
}

export async function toggleReminder(event: { id: string; title: string; starts_at: string }, serverName: string) {
  const enabled = !reminders.ids.has(event.id)
  try {
    const ids = await api.setReminder(
      { id: event.id, title: event.title, server_name: serverName, starts_at: event.starts_at },
      enabled,
    )
    reminders.ids = new Set(ids)
    if (enabled) showToast('Combinado! Você recebe um aviso 10 minutos antes do evento.')
  } catch (e) {
    showToast(errorMessage(e))
  }
}

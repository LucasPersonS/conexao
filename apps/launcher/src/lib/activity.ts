/**
 * Modo repouso: com o jogo aberto ou a janela minimizada/escondida, o launcher para
 * o que roda em segundo plano (atualização da lista, carrossel, animações) para não
 * disputar CPU e GPU com o Minecraft. Volta ao normal sozinho.
 */
import { computed, reactive, watch } from 'vue'
import { listen } from '@tauri-apps/api/event'

export const activity = reactive({
  hidden: typeof document !== 'undefined' && document.hidden,
  /** Slug do servidor em jogo (o jogo está aberto). */
  playing: null as string | null,
})

export const resting = computed(() => activity.hidden || activity.playing !== null)

let started = false
export function initActivity() {
  if (started) return
  started = true
  document.addEventListener('visibilitychange', () => (activity.hidden = document.hidden))
  listen<{ slug: string; running: boolean }>('game', (event) => {
    activity.playing = event.payload.running ? event.payload.slug : null
  }).catch(() => {})
  watch(resting, (value) => document.documentElement.classList.toggle('resting', value), { immediate: true })
}

/** Chama `fn` sempre que o launcher sai do repouso. */
export function onWake(fn: () => void) {
  return watch(resting, (value, before) => {
    if (before && !value) fn()
  })
}

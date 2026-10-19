import { ref } from 'vue'
import type { PageTheme } from './api'

/** Fundo da área de conteúdo enquanto a página de um servidor PRO está aberta (App.vue aplica). */
export const pageBackground = ref<string | null>(null)

const HEX = /^#[0-9a-f]{6}$/i

/** Gradiente CSS a partir do tema validado pela API (confere de novo: só cores #rrggbb e número). */
export function gradientCss(theme: PageTheme | null | undefined): string | null {
  if (!theme) return null
  const colors = theme.colors.filter((c) => HEX.test(c))
  if (colors.length < 2) return null
  if (theme.style === 'radial') return `radial-gradient(circle at 50% 0%, ${colors.join(', ')})`
  const angle = Math.max(0, Math.min(360, Math.round(Number(theme.angle) || 0)))
  return `linear-gradient(${angle}deg, ${colors.join(', ')})`
}

export function accentColor(theme: PageTheme | null | undefined): string | null {
  return theme?.accent && HEX.test(theme.accent) ? theme.accent : null
}

export const youtubeThumb = (id: string) => `https://i.ytimg.com/vi/${encodeURIComponent(id)}/hqdefault.jpg`
export const youtubeEmbed = (id: string) =>
  `https://www.youtube-nocookie.com/embed/${encodeURIComponent(id)}?autoplay=1&rel=0&modestbranding=1`
export const youtubeWatch = (id: string) => `https://www.youtube.com/watch?v=${encodeURIComponent(id)}`

/** Link do formulário com `{nick}` trocado pelo nick do jogador (para formulários pré-preenchidos). */
export function formLink(url: string, nick: string | null | undefined): string {
  return url.replace(/\{nick\}/g, encodeURIComponent(nick ?? ''))
}

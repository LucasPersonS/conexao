import { ref } from 'vue'
import type { PageTheme } from './api'

/** Fundo da área de conteúdo enquanto a página de um servidor PRO está aberta (App.vue aplica). */
export const pageBackground = ref<string | null>(null)
/** Cor das linhas da grade por cima do gradiente (acompanha as cores escolhidas). */
export const pageGrid = ref<string | null>(null)

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

type Rgb = [number, number, number]

function hexRgb(hex: string): Rgb {
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as Rgb
}

/** Luminância relativa (WCAG), para achar a cor mais clara do gradiente. */
function luminance([r, g, b]: Rgb): number {
  const lin = (c: number) => {
    const v = c / 255
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

const mix = (a: Rgb, b: Rgb, t: number): Rgb => a.map((v, i) => Math.round(v + (b[i] - v) * t)) as Rgb

/**
 * Cor das linhas em quadrados por cima do gradiente: parte da cor mais viva do fundo (ou
 * da cor de destaque, quando tem), clareia um pouco e fica bem transparente. Assim a grade
 * do Conexão continua lá, mas no tom do servidor (verde num tema verde, roxo num roxo…).
 */
export function gridColor(theme: PageTheme | null | undefined): string | null {
  if (!theme) return null
  const colors = theme.colors.filter((c) => HEX.test(c)).map(hexRgb)
  if (colors.length < 2) return null
  // A mais clara costuma ser a cor "do servidor"; as escuras são só profundidade.
  let base = colors.reduce((best, c) => (luminance(c) > luminance(best) ? c : best))
  const accent = accentColor(theme)
  if (accent) base = mix(base, hexRgb(accent), 0.6)
  const light = luminance(base)
  // Fundo muito claro: puxa para o escuro, senão a linha some; escuro: clareia.
  const tone = light > 0.45 ? mix(base, [10, 15, 17], 0.55) : mix(base, [255, 255, 255], 0.35)
  const alpha = light > 0.45 ? 0.16 : 0.11
  return `rgba(${tone[0]}, ${tone[1]}, ${tone[2]}, ${alpha})`
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

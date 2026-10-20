/**
 * Cor das linhas em quadrados por cima do gradiente da página do servidor. Mesma conta do
 * launcher (apps/launcher/src/lib/page.ts → gridColor): a prévia do painel fica igual ao launcher.
 */
import type { PageTheme } from './api'

type Rgb = [number, number, number]
const HEX = /^#[0-9a-f]{6}$/i

function hexRgb(hex: string): Rgb {
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as Rgb
}

function luminance([r, g, b]: Rgb): number {
  const lin = (c: number) => {
    const v = c / 255
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

const mix = (a: Rgb, b: Rgb, t: number): Rgb => a.map((v, i) => Math.round(v + (b[i] - v) * t)) as Rgb

export function gridColor(theme: PageTheme | null | undefined): string | null {
  if (!theme) return null
  const colors = theme.colors.filter((c) => HEX.test(c)).map(hexRgb)
  if (colors.length < 2) return null
  let base = colors.reduce((best, c) => (luminance(c) > luminance(best) ? c : best))
  if (theme.accent && HEX.test(theme.accent)) base = mix(base, hexRgb(theme.accent), 0.6)
  const light = luminance(base)
  const tone = light > 0.45 ? mix(base, [10, 15, 17], 0.55) : mix(base, [255, 255, 255], 0.35)
  const alpha = light > 0.45 ? 0.16 : 0.11
  return `rgba(${tone[0]}, ${tone[1]}, ${tone[2]}, ${alpha})`
}

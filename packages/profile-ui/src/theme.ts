import catalogJson from './catalog.json'

export type Tier = 'free' | 'premium'

export interface CatalogItem {
  id: string
  name: string
  tier: Tier
  family?: string
  animated?: boolean
}

export interface GradientPreset {
  id: string
  name: string
  colors: string[]
  angle: number
}

export const catalog = catalogJson as {
  fonts: CatalogItem[]
  frames: CatalogItem[]
  name_effects: CatalogItem[]
  gradients: GradientPreset[]
}

export interface ProfileTheme {
  background: { kind: 'gradient' | 'image'; colors: string[]; angle: number }
  panel: 'claro' | 'escuro'
  accent: string
  frame: string
  font: string
  name_effect: string
  name_colors: string[]
}

export interface ProfileLinks {
  discord?: string | null
  youtube?: string | null
  twitch?: string | null
  tiktok?: string | null
  instagram?: string | null
  x?: string | null
  bluesky?: string | null
}

export interface PublicProfile {
  minecraft_uuid: string
  minecraft_name: string
  display_name: string | null
  pronouns: string | null
  bio: string | null
  links: ProfileLinks
  theme: ProfileTheme
  /** Fundo enviado e aprovado pela equipe (nulo enquanto em revisão). */
  background_url: string | null
  badges: { id: string; label: string }[]
}

export const DEFAULT_THEME: ProfileTheme = {
  background: { kind: 'gradient', colors: ['#0a0f11', '#2a4a10', '#b7fe00'], angle: 160 },
  panel: 'escuro',
  accent: '#b7fe00',
  frame: 'simples',
  font: 'padrao',
  name_effect: 'nenhum',
  name_colors: ['#b7fe00', '#f2f6ef'],
}

export function fontFamily(id: string): string {
  const family = catalog.fonts.find((f) => f.id === id)?.family ?? 'Inter'
  return `'${family}', 'Inter', system-ui, sans-serif`
}

/** Variáveis CSS do card a partir do tema (cores já validadas pela API). */
export function cardStyle(theme: ProfileTheme, backgroundUrl: string | null): Record<string, string> {
  const colors = theme.background.colors.length >= 2 ? theme.background.colors : [...theme.background.colors, theme.background.colors[0] ?? '#b7fe00']
  const bg =
    theme.background.kind === 'image' && backgroundUrl
      ? `url("${backgroundUrl.replace(/"/g, '%22')}")`
      : `linear-gradient(${theme.background.angle}deg, ${colors.join(', ')})`
  return {
    '--cp-accent': theme.accent,
    '--cp-bg': bg,
    '--cp-font': fontFamily(theme.font),
    '--cp-name-a': theme.name_colors[0] ?? theme.accent,
    '--cp-name-b': theme.name_colors[1] ?? theme.accent,
  }
}

export const LINK_LABELS: Record<keyof ProfileLinks, string> = {
  discord: 'Discord',
  youtube: 'YouTube',
  twitch: 'Twitch',
  tiktok: 'TikTok',
  instagram: 'Instagram',
  x: 'X',
  bluesky: 'Bluesky',
}

export function isPremium(kind: 'fonts' | 'frames' | 'name_effects', id: string): boolean {
  return catalog[kind].find((i) => i.id === id)?.tier === 'premium'
}

/**
 * `invoke` falso da prévia: responde aos comandos do launcher com os dados de ../data.ts.
 * Comandos que a prévia não precisa respondem com um valor vazio e seguro.
 */
import * as data from '../data'

type Args = Record<string, unknown> | undefined

const wait = (ms = 60) => new Promise((resolve) => setTimeout(resolve, ms))

function apiJson(method: string, path: string): unknown {
  const url = path.split('?')[0]
  const parts = url.split('/').filter(Boolean) // ['v1', ...]
  if (method === 'GET' && url === '/v1/me/profile') {
    return {
      ...data.myProfile,
      discord_visibility: 'everyone',
      pending_background_url: null,
      background_status: null,
      banned: false,
      ban_reason: null,
      premium: true,
      premium_enforced: false,
      public_url: `https://perfil.conexaosmps.com.br/u/${data.ME.name}`,
    }
  }
  if (url === '/v1/profile/catalog') return { premium_enforced: false }
  if (parts[1] === 'profiles' && parts[2]) {
    const name = decodeURIComponent(parts[2])
    return data.profiles[name] ?? data.myProfile
  }
  if (url === '/v1/me/stars') return ['reino-valdoria', 'vila-serena']
  if (url === '/v1/events') return data.upcoming
  if (url.startsWith('/v1/sponsors/')) return []
  if (url === '/v1/me/wallet') return { balance: 2450, premium_until: new Date(Date.now() + 40 * 86_400_000).toISOString(), history: [] }
  if (url === '/v1/me/orders') return []
  if (url === '/v1/shop/catalog') {
    return {
      money_enabled: true,
      support_enabled: true,
      coin_packs: [
        { sku: 'moedas-500', coins: 500, price_cents: 500 },
        { sku: 'moedas-1050', coins: 1050, price_cents: 1000 },
        { sku: 'moedas-2200', coins: 2200, price_cents: 2000 },
        { sku: 'moedas-5750', coins: 5750, price_cents: 5000 },
      ],
      plus: [
        { sku: 'plus-30', days: 30, price_cents: 990, coins: 990 },
        { sku: 'plus-90', days: 90, price_cents: 2690, coins: 2690 },
      ],
      pro: [{ sku: 'pro-30', days: 30, price_cents: 2990, coins: 2990 }],
      featured: [{ sku: 'destaque-7', days: 7, price_cents: 2990, coins: 2990 }],
      support: { min_cents: 500, max_cents: 50000, fee_percent: 15 },
    }
  }
  if (parts[1] === 'servers' && parts[2]) {
    const slug = parts[2]
    const tail = parts[3]
    if (tail === 'page') return data.pages[slug] ?? { active: false, page: {} }
    if (tail === 'posts') return data.news(slug)
    if (tail === 'comments') return data.comments(slug)
    if (tail === 'supporters') {
      return {
        supporters_30d: 37,
        recent: [
          { supporter: 'Aurora_Mel', supporter_uuid: 'Aurora_Mel', message: 'Pelo torneio de sexta!', paid_at: new Date().toISOString() },
          { supporter: 'NeonByte', supporter_uuid: 'NeonByte', message: 'Servidor top', paid_at: new Date().toISOString() },
          { supporter: null, supporter_uuid: null, message: 'Valeu, equipe!', paid_at: new Date().toISOString() },
        ],
      }
    }
    if (tail === 'star') return { stars: 1, starred: true }
  }
  return null
}

export async function invoke<T>(cmd: string, args?: Args): Promise<T> {
  await wait()
  const a = args ?? {}
  const result: unknown = (() => {
    switch (cmd) {
      case 'initialize':
        return null
      case 'take_invite':
        return null
      case 'current_account':
        return data.ME
      case 'conexao_session':
        return {}
      case 'list_servers':
        return data.servers
      case 'my_roles':
        return { is_staff: true, servers: [{ slug: 'reino-valdoria', name: 'Reino de Valdoria', role: 'admin' }] }
      case 'server_access':
        return { server: a.slug, whitelist_required: true, status: 'approved', can_play: true, allow_offline: false, reason: null }
      case 'install_state':
        return { installed: a.slug === 'reino-valdoria', version: a.slug === 'reino-valdoria' ? '2.4.1' : null }
      case 'play_progress':
        return { stage: 'idle', message: '', current: 0, total: 0, step: 0 }
      case 'watched_slots':
      case 'reminders':
        return []
      case 'check_update':
        return null
      case 'api_json':
        return apiJson(String(a.method), String(a.path))
      default:
        console.info('[prévia] comando sem resposta:', cmd, a)
        return null
    }
  })()
  return result as T
}

export function convertFileSrc(path: string) {
  return path
}

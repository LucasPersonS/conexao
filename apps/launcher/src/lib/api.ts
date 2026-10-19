import { invoke } from '@tauri-apps/api/core'
import type { ProfileLinks, ProfileTheme, PublicProfile } from '@profile/theme'

export type Loader = 'fabric' | 'forge' | 'neoforge'

export interface ServerStatus {
  online: boolean
  players_online: number
  players_max: number
  latency_ms: number | null
  checked_at: string | null
}

export interface ServerSummary {
  id: string
  slug: string
  name: string
  description: string
  address: string
  minecraft: string
  loader: { kind: Loader; version: string }
  tags: string[]
  featured: boolean
  icon_url: string | null
  banner_url: string | null
  discord_url: string | null
  whitelist_required: boolean
  status: ServerStatus
  current_version: string | null
  pack_size: number | null
  /** Aceita contas não originais (login pelo Discord). */
  allow_offline?: boolean
  /** Estrelas dadas pelos jogadores. */
  stars?: number
  /** Plano PRO ativo. */
  pro?: boolean
  /** Aceita apoio dos jogadores (Mercado Pago conectado). */
  supportable?: boolean
  /** Google Forms para pedir whitelist (pode ter `{nick}`). */
  whitelist_form_url?: string | null
  /** Página personalizada ativa (PRO). */
  has_page?: boolean
}

export type WhitelistStatus = 'pending' | 'approved' | 'rejected' | 'banned'

export interface AccessResponse {
  server: string
  whitelist_required: boolean
  status: WhitelistStatus | null
  can_play: boolean
  allow_offline?: boolean
  /** Motivo para mostrar quando não pode jogar por outra razão que a whitelist. */
  reason?: string | null
}

export type AccountKind = 'microsoft' | 'offline'

export interface Account {
  uuid: string
  name: string
  /** microsoft = conta original; offline = conta não original (Discord). */
  kind: AccountKind
}

export type DiscordLogin =
  | { status: 'cancelled' }
  | { status: 'ok'; account: Account }
  | { status: 'needs_nick'; signup_token: string; discord_name: string; suggested_nick: string | null }

export type Stage = 'idle' | 'checking' | 'installing' | 'launching' | 'running' | 'error'

export interface PlayProgress {
  stage: Stage
  message: string
  current: number
  total: number
  /** 0 acesso, 1 Minecraft e Java, 2 loader, 3 mods, 4 abrindo, 5 em jogo */
  step: number
}

export interface InstallState {
  installed: boolean
  version: string | null
}

/** Quem pediu o código do painel (mostrado antes de aprovar). */
export interface PairingInfo {
  ip: string
  browser: string
  age_secs: number
}

export interface MyRoles {
  is_staff: boolean
  servers: { slug: string; name: string; role: string }[]
}

export interface GameSettings {
  memory_mb: number
  fullscreen: boolean
  resolution_width: number
  resolution_height: number
  jvm_args: string
  env_vars: string
  pre_launch: string
  wrapper: string
  post_exit: string
  max_downloads: number
}

export interface LauncherPrefs {
  memory_auto: boolean
  jvm_optimized: boolean
  minimize_on_play: boolean
  discord_presence: boolean
  notifications: boolean
  java_path: string | null
}

export interface SettingsView extends GameSettings {
  system_memory_mb: number
  recommended_memory_mb: number
  launcher: LauncherPrefs
  optimized_flags: string[]
  discord_available: boolean
}

export interface JavaInfo {
  path: string
  version: string
  major: number
  architecture: string
}

export interface ServerPost {
  id: string
  kind: 'news' | 'event'
  title: string
  body: string
  starts_at: string | null
  created_at: string
  author_name: string | null
}

export interface UpcomingEvent {
  id: string
  title: string
  body: string
  starts_at: string
  slug: string
  server_name: string
  icon_url: string | null
}

export interface Reminder {
  id: string
  title: string
  server_name: string
  starts_at: string
}

export const LOADER_NAMES: Record<Loader, string> = {
  fabric: 'Fabric',
  forge: 'Forge',
  neoforge: 'NeoForge',
}

export function errorMessage(error: unknown): string {
  if (error && typeof error === 'object' && 'message' in error) return String((error as { message: unknown }).message)
  return String(error)
}

export interface OwnProfile extends PublicProfile {
  discord_visibility: 'everyone' | 'staff' | 'nobody'
  pending_background_url: string | null
  background_status: 'pending' | 'approved' | 'rejected' | null
  banned: boolean
  ban_reason: string | null
  premium: boolean
  premium_enforced: boolean
  public_url: string
}

export interface ProfileInput {
  display_name: string | null
  pronouns: string | null
  bio: string | null
  links: ProfileLinks
  theme: ProfileTheme
  discord_visibility: OwnProfile['discord_visibility']
}

const json = <T>(method: string, path: string, body?: unknown) => invoke<T>('api_json', { method, path, body: body ?? null })

export const profiles = {
  catalog: () => json<{ premium_enforced: boolean }>('GET', '/v1/profile/catalog'),
  mine: () => json<OwnProfile>('GET', '/v1/me/profile'),
  save: (input: ProfileInput) => json<OwnProfile>('PUT', '/v1/me/profile', input),
  get: (name: string) => json<PublicProfile>('GET', `/v1/profiles/${encodeURIComponent(name)}`),
  report: (name: string, reason: string) => json<null>('POST', `/v1/profiles/${encodeURIComponent(name)}/report`, { reason }),
  uploadBackground: async (file: File) =>
    invoke<OwnProfile>('upload_profile_background', { bytes: Array.from(new Uint8Array(await file.arrayBuffer())) }),
}

export interface ServerComment {
  id: string
  body: string
  created_at: string
  author_name: string
  author_uuid: string
  /** Papel do autor na equipe do servidor. */
  author_role: 'owner' | 'admin' | 'moderator' | null
  author_offline: boolean
  mine: boolean
  can_delete: boolean
}

export interface CommentPage {
  total: number
  stars: number
  comments: ServerComment[]
  has_more: boolean
  viewer: { can_comment: boolean; reason: string | null; can_moderate: boolean }
}

export interface StarState {
  stars: number
  starred: boolean
}

const serverPath = (slug: string) => `/v1/servers/${encodeURIComponent(slug)}`

export const community = {
  myStars: () => json<string[]>('GET', '/v1/me/stars'),
  star: (slug: string, on: boolean) => json<StarState>(on ? 'PUT' : 'DELETE', `${serverPath(slug)}/star`),
  comments: (slug: string) => json<CommentPage>('GET', `${serverPath(slug)}/comments`),
  comment: (slug: string, body: string) => json<ServerComment>('POST', `${serverPath(slug)}/comments`, { body }),
  deleteComment: (slug: string, id: string) => json<null>('DELETE', `${serverPath(slug)}/comments/${id}`),
}

// ------------------------------------------------------- página personalizada (PRO)

export interface PageTheme {
  colors: string[]
  angle: number
  style: 'linear' | 'radial'
  accent?: string | null
}

export type PageBlock =
  | { type: 'text'; title: string; body: string }
  | { type: 'image'; url: string; caption: string }
  | { type: 'gallery'; urls: string[]; caption: string }
  | { type: 'video'; youtube: string; caption: string }

export interface ServerPage {
  active: boolean
  page: { theme?: PageTheme | null; blocks?: PageBlock[] }
}

export const pages = {
  get: (slug: string) => json<ServerPage>('GET', `${serverPath(slug)}/page`),
}

// ------------------------------------------------------------------ loja e apoio

export interface PlanOption {
  sku: string
  days: number
  price_cents: number
  /** Preço em Moedas (1 moeda = 1 centavo). */
  coins: number
}

export interface CoinPack {
  sku: string
  coins: number
  price_cents: number
}

export interface ShopCatalog {
  money_enabled: boolean
  support_enabled: boolean
  coin_packs: CoinPack[]
  plus: PlanOption[]
  pro: PlanOption[]
  featured: PlanOption[]
  support: { min_cents: number; max_cents: number; fee_percent: number }
}

export interface LedgerEntry {
  id: string
  delta: number
  balance_after: number
  kind: 'purchase' | 'spend' | 'refund' | 'chargeback' | 'grant' | 'revoke'
  description: string
  created_at: string
}

export interface Wallet {
  balance: number
  premium_until: string | null
  history: LedgerEntry[]
}

export type OrderStatus = 'pending' | 'approved' | 'rejected' | 'cancelled' | 'refunded'

export interface Order {
  id: string
  kind: 'coins' | 'plus' | 'pro' | 'featured' | 'support'
  sku: string
  pay_with: 'money' | 'coins'
  amount_cents: number
  coins: number
  days: number | null
  status: OrderStatus
  server_slug: string | null
  server_name: string | null
  created_at: string
  paid_at: string | null
  granted_until: string | null
}

export interface Checkout {
  order_id: string
  checkout_url: string
  fee_cents?: number
}

export interface SupportWall {
  supporters_30d: number
  recent: { supporter: string | null; supporter_uuid: string | null; message: string | null; paid_at: string | null }[]
}

export const shop = {
  catalog: () => json<ShopCatalog>('GET', '/v1/shop/catalog'),
  wallet: () => json<Wallet>('GET', '/v1/me/wallet'),
  orders: () => json<Order[]>('GET', '/v1/me/orders'),
  order: (id: string) => json<Order>('GET', `/v1/me/orders/${id}`),
  /** Pagar com dinheiro: devolve o link do Mercado Pago. */
  checkout: (sku: string, server?: string) => json<Checkout>('POST', '/v1/shop/checkout', { sku, server: server ?? null }),
  /** Pagar com Moedas. `key` é único por clique (o servidor não cobra duas vezes). */
  spend: (sku: string, key: string, server?: string) =>
    json<{ order: Order; balance: number }>('POST', '/v1/shop/spend', { sku, key, server: server ?? null }),
  supporters: (slug: string) => json<SupportWall>('GET', `${serverPath(slug)}/supporters`),
  support: (slug: string, amountCents: number, message: string, anonymous: boolean) =>
    json<Checkout>('POST', `${serverPath(slug)}/support/checkout`, {
      amount_cents: amountCents,
      message: message.trim() || null,
      anonymous,
    }),
}

// ------------------------------------------------------------------ patrocinadores

export type SponsorPlacement = 'home' | 'server' | 'loading'

export interface Sponsor {
  id: string
  name: string
  image_url: string
  image_side_url: string | null
  link_url: string
  alt_text: string | null
  weight: number
}

export const sponsorsApi = {
  list: (placement: SponsorPlacement) => json<Sponsor[]>('GET', `/v1/sponsors/${placement}`),
  events: (placement: SponsorPlacement, views: string[], click?: string) =>
    json<null>('POST', '/v1/sponsors/events', { placement, views, click: click ?? null }),
}

export const posts = {
  of: (slug: string) => json<{ news: ServerPost[]; events: ServerPost[] }>('GET', `/v1/servers/${encodeURIComponent(slug)}/posts`),
  upcoming: () => json<UpcomingEvent[]>('GET', '/v1/events'),
}

export const api = {
  initialize: () => invoke<void>('initialize'),
  takeInvite: () => invoke<string | null>('take_invite'),
  currentAccount: () => invoke<Account | null>('current_account'),
  login: () => invoke<Account | null>('login_microsoft'),
  loginDiscord: () => invoke<DiscordLogin>('login_discord'),
  registerOffline: (signupToken: string, nick: string) => invoke<Account>('register_offline', { signupToken, nick }),
  logout: () => invoke<void>('logout'),
  session: () => invoke<unknown>('conexao_session'),
  servers: () => invoke<ServerSummary[]>('list_servers'),
  access: (slug: string) => invoke<AccessResponse>('server_access', { slug }),
  apply: (slug: string) => invoke<AccessResponse>('apply_whitelist', { slug }),
  play: (slug: string, address: string) => invoke<void>('play', { slug, address }),
  publishToken: () => invoke<string>('publish_token'),
  approvePanel: (code: string) => invoke<void>('approve_panel', { code }),
  pairingInfo: (code: string) => invoke<PairingInfo>('pairing_info', { code }),
  checkUpdate: () => invoke<UpdateInfo | null>('check_update'),
  installUpdate: () => invoke<void>('install_update'),
  progress: (slug: string) => invoke<PlayProgress>('play_progress', { slug }),
  installState: (slug: string) => invoke<InstallState>('install_state', { slug }),
  myRoles: () => invoke<MyRoles>('my_roles'),
  getSettings: () => invoke<SettingsView>('get_settings'),
  setSettings: (game: GameSettings, launcher: LauncherPrefs) => invoke<void>('set_settings', { game, launcher }),
  watchSlot: (slug: string, name: string, address: string) => invoke<void>('watch_slot', { slug, name, address }),
  unwatchSlot: (slug: string) => invoke<void>('unwatch_slot', { slug }),
  watchedSlots: () => invoke<string[]>('watched_slots'),
  setReminder: (reminder: Reminder, enabled: boolean) => invoke<string[]>('set_reminder', { reminder, enabled }),
  reminders: () => invoke<string[]>('reminders'),
  openGameFolder: (slug: string) => invoke<void>('open_game_folder', { slug }),
  openLauncherFolder: (kind: 'data' | 'logs') => invoke<void>('open_launcher_folder', { kind }),
  listJava: () => invoke<JavaInfo[]>('list_java'),
  testJava: (path: string) => invoke<JavaInfo>('test_java', { path }),
}

export interface UpdateInfo {
  version: string
  /** Versão instalada agora. */
  current: string
  notes: string | null
}

export interface UpdateProgress {
  downloaded: number
  total: number | null
  /** Download terminou; o instalador está rodando (o launcher fecha e reabre). */
  installing: boolean
}

import { reactive } from 'vue'
import type { PublicProfile } from '@profile/theme'

export const API_URL: string = (import.meta.env.VITE_CONEXAO_API_URL ?? 'http://localhost:8080').replace(/\/$/, '')

export type Loader = 'fabric' | 'forge' | 'neoforge'
export type WhitelistStatus = 'pending' | 'approved' | 'rejected' | 'banned'
export type Role = 'owner' | 'admin' | 'moderator'

export const LOADER_NAMES: Record<Loader, string> = { fabric: 'Fabric', forge: 'Forge', neoforge: 'NeoForge' }

export interface User {
  id: string
  minecraft_uuid: string
  minecraft_name: string
  is_staff: boolean
}

export interface Membership {
  server_id: string
  slug: string
  name: string
  role: Role
}

export interface Server {
  id: string
  slug: string
  name: string
  description: string
  address: string
  minecraft: string
  loader: { kind: Loader; version: string }
  tags: string[]
  featured: boolean
  featured_until?: string | null
  icon_url: string | null
  banner_url: string | null
  discord_url: string | null
  whitelist_required: boolean
  /** Aceita contas não originais (login pelo Discord + passe de entrada do Companion). */
  allow_offline?: boolean
  status: { online: boolean; players_online: number; players_max: number; latency_ms: number | null; checked_at: string | null }
  current_version: string | null
  approval: 'pending' | 'approved' | 'rejected' | 'suspended'
  /** Estrelas dadas pelos jogadores no launcher. */
  stars?: number
  /** Plano PRO ativo e até quando. */
  pro?: boolean
  pro_until?: string | null
  /** Aceita apoio dos jogadores. */
  supportable?: boolean
  /** Google Forms para pedir whitelist (pode ter `{nick}`). */
  whitelist_form_url?: string | null
  /** Página personalizada visível no launcher (PRO ativo e página preenchida). */
  has_page?: boolean
  /** Programa "SMP da Central" (parceria Central SMP's). */
  central_partner?: boolean
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

export interface PageData {
  theme?: PageTheme | null
  blocks?: PageBlock[]
}

export interface PageState {
  active: boolean
  pro_until: string | null
  can_edit: boolean
  page: PageData
  updated_at: string | null
  usage?: { uploads_24h: number; bytes_24h: number; max_uploads_24h: number; max_bytes_24h: number }
}

/** Imagem ou GIF guardado no armazenamento da página do servidor. */
export interface PageMediaItem {
  id: string
  url: string
  kind: 'image' | 'gif'
  bytes: number
  created_at: string
  uploaded_by_name: string | null
  /** Está na página salva: tire do bloco e salve antes de apagar. */
  in_use: boolean
}

export interface PlanOption {
  sku: string
  days: number
  price_cents: number
  coins: number
}

export interface ShopCatalog {
  money_enabled: boolean
  support_enabled: boolean
  pro: PlanOption[]
  featured: PlanOption[]
  support: { min_cents: number; max_cents: number; fee_percent: number }
}

export interface ShopOrder {
  id: string
  kind: string
  sku: string
  pay_with: 'money' | 'coins'
  amount_cents: number
  coins: number
  days: number | null
  status: 'pending' | 'approved' | 'rejected' | 'cancelled' | 'refunded'
  granted_until: string | null
}

export interface SupportAdmin {
  available: boolean
  can_manage: boolean
  fee_percent: number
  account: { mp_user: string; support_enabled: boolean; live_mode: boolean; connected_at: string } | null
  last_30_days: { count: number; gross_cents: number; fee_cents: number; net_cents: number }
  recent: {
    id: string
    supporter: string | null
    amount_cents: number
    fee_cents: number
    message: string | null
    status: 'approved' | 'refunded'
    paid_at: string | null
  }[]
}

export interface ServerStats {
  days: { day: string; whitelist: number; stars: number; comments: number; supports: number; support_net_cents: number }[]
  whitelist_total: number
  whitelist_approved: number
}

export interface VersionItem {
  id: string
  version: string
  channels: ('stable' | 'beta')[]
  file_count: number
  manifest_sha256: string
  published_at: string
}

export interface WhitelistEntry {
  profile: PublicProfile | null
  minecraft_uuid: string
  minecraft_name: string
  status: WhitelistStatus
  note: string | null
  decided_by_name: string | null
  updated_at: string
}

export interface Member {
  user_id: string
  minecraft_uuid: string
  minecraft_name: string
  role: Role
}

export interface AuditEntry {
  action: string
  details: Record<string, unknown>
  actor_name: string | null
  created_at: string
}

export interface PackFile {
  path: string
  sha1: string
  sha512: string
  size: number
  policy: 'managed' | 'initial'
  side: 'both' | 'client' | 'server'
  url?: string | null
}

export interface PackManifest {
  format: number
  server: string
  name: string
  version: string
  minecraft: string
  loader: { kind: Loader; version: string }
  files: PackFile[]
  created_at: string
}

export interface PlanFileInfo {
  path: string
  source: 'modrinth' | 'external' | 'hosted'
  size: number
  title: string | null
  icon_url: string | null
  project_slug: string | null
  side: 'both' | 'client' | 'server'
}

export interface PlanResponse {
  manifest: PackManifest
  previous_version: string | null
  changes: { kind: 'added' | 'changed' | 'unchanged' | 'removed'; path: string; name: string }[]
  warnings: string[]
  files: PlanFileInfo[]
  server_only: PlanFileInfo[]
  hosted_bytes: number
  quota_bytes: number
}

export interface PlanRequestFile {
  path: string
  sha1: string
  url?: string
  sha512?: string
  size?: number
}

export interface PlanRequest {
  version: string
  minecraft?: string
  loader?: { kind: Loader; version: string }
  files: PlanRequestFile[]
  managed: string[]
  excluded: string[]
  include_server_only?: string[]
}

export interface MinecraftOption {
  id: string
  loaders: Loader[]
}

export interface LoaderOption {
  id: string
  stable: boolean
  recommended: boolean
}

export interface DetectedServer {
  online: boolean
  address: string
  version_name: string | null
  minecraft: string | null
  loader: Loader | null
  motd: string | null
  favicon: string | null
  players_online: number
  players_max: number
  latency_ms: number | null
  mods: { id: string; version: string | null }[]
  error: string | null
}

export interface ResolvedFile {
  sha1: string
  sha512: string
  size: number
  url: string
  title: string
  icon_url: string | null
  project_slug: string
  version_number: string
  loaders: string[]
  game_versions: string[]
  client_side: string
  server_side: string
}

export interface CompanionInfo {
  minecraft: string
  loader: Loader | null
  loader_version: string | null
  motd: string
  max_players: number
  port: number
  online_mode: boolean
  whitelist: boolean
  enforce_secure_profile?: boolean
}

export interface LinkView {
  code: string
  info: CompanionInfo
  created_at: string
  linked_slug: string | null
}

export interface SnapshotMod {
  path: string
  sha1: string
  size: number
  title: string | null
  icon_url: string | null
  source: 'modrinth' | 'unknown'
  server_only: boolean
  client_only: boolean
  in_pack: boolean
}

export interface SnapshotView {
  connected: boolean
  last_seen_at: string | null
  agent_version: string | null
  reported_at: string | null
  info: CompanionInfo | null
  mods: SnapshotMod[]
  only_in_pack: string[]
  out_of_sync: boolean
}

export interface FeaturedPurchase {
  id: string
  days: number
  amount_cents: number
  status: 'pending' | 'approved' | 'rejected' | 'cancelled' | 'refunded'
  created_at: string
  paid_at: string | null
  featured_until: string | null
  buyer_name: string | null
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

export interface ServerComment {
  id: string
  body: string
  created_at: string
  author_name: string
  author_uuid: string
  author_role: Role | null
  /** Conta não original (cadastrada pelo Discord). */
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

export interface PostInput {
  kind: 'news' | 'event'
  title: string
  body: string
  starts_at: string | null
}

export interface ModerationQueue {
  reports: { id: string; target_name: string; reporter_name: string; reason: string; created_at: string; open_count: number }[]
  backgrounds: { minecraft_name: string; minecraft_uuid: string; background_url: string; updated_at: string }[]
  banned: { minecraft_name: string; minecraft_uuid: string; profile_banned_at: string; profile_ban_reason: string | null }[]
}

export interface AgentStatus {
  configured: boolean
  connected: boolean
  agent_version: string | null
  last_seen_at: string | null
}

const TOKEN_KEY = 'conexao.panel.token'
/** Sessão salva quando o painel se chamava CENTRAL. */
const LEGACY_TOKEN_KEY = 'central.panel.token'

function readToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY) ?? localStorage.getItem(LEGACY_TOKEN_KEY)
  } catch {
    return null
  }
}

export const session = reactive<{ token: string | null; user: User | null; memberships: Membership[] }>({
  token: readToken(),
  user: null,
  memberships: [],
})

export function setToken(token: string | null) {
  session.token = token
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token)
    else localStorage.removeItem(TOKEN_KEY)
    localStorage.removeItem(LEGACY_TOKEN_KEY)
  } catch {
    /* armazenamento indisponível: a sessão vale só nesta aba */
  }
}

export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public details: unknown = null,
  ) {
    super(message)
  }
}

export async function request<T>(method: string, path: string, body?: unknown, extraHeaders: Record<string, string> = {}): Promise<T> {
  const headers: Record<string, string> = { ...extraHeaders }
  if (body !== undefined) headers['Content-Type'] = 'application/json'
  if (session.token) headers.Authorization = `Bearer ${session.token}`
  let response: Response
  try {
    response = await fetch(`${API_URL}${path}`, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) })
  } catch {
    throw new ApiError('Não foi possível falar com a API do Conexão.', 0)
  }
  if (response.status === 401 && session.token) {
    setToken(null)
    session.user = null
  }
  const text = await response.text()
  const data = text ? JSON.parse(text) : null
  if (!response.ok && response.status !== 202) {
    throw new ApiError(data?.message ?? `Erro ${response.status}`, response.status, data?.details)
  }
  return data as T
}

export const api = {
  me: () => request<{ user: User; memberships: Membership[] }>('GET', '/v1/me'),
  pairStart: () => request<{ code: string; poll_token: string; expires_in: number }>('POST', '/v1/auth/pair'),
  pairPoll: (poll_token: string) =>
    request<{ status: string; token: string | null; user: User | null }>('POST', '/v1/auth/pair/poll', { poll_token }),
  server: (slug: string) => request<Server>('GET', `/v1/servers/${slug}`),
  createServer: (body: unknown) => request<Server>('POST', '/v1/servers', body),
  updateServer: (slug: string, body: unknown) => request<Server>('PATCH', `/v1/servers/${slug}`, body),
  page: (slug: string) => request<PageState>('GET', `/v1/servers/${slug}/page`),
  savePage: (slug: string, page: PageData) => request<PageState>('PUT', `/v1/servers/${slug}/page`, page),
  /** Confere a página (com `pending:N` no lugar das imagens novas) antes de enviar os arquivos. */
  checkPage: (slug: string, page: PageData, uploads: { bytes: number; gif: boolean }[]) =>
    request<{ ok: boolean }>('POST', `/v1/servers/${slug}/page/check`, { page, uploads }),
  pageMedia: (slug: string) => request<{ items: PageMediaItem[]; total_bytes: number }>('GET', `/v1/servers/${slug}/page/media`),
  deletePageMedia: (slug: string, id: string) =>
    request<{ ok: boolean; bytes: number; removed_from_disk: boolean }>('DELETE', `/v1/servers/${slug}/page/media/${id}`),
  versions: (slug: string) => request<VersionItem[]>('GET', `/v1/servers/${slug}/versions`),
  setChannel: (slug: string, channel: string, version: string) =>
    request<unknown>('PUT', `/v1/servers/${slug}/channels/${channel}`, { version }),
  whitelist: (slug: string, status?: WhitelistStatus) =>
    request<WhitelistEntry[]>('GET', `/v1/servers/${slug}/whitelist${status ? `?status=${status}` : ''}`),
  setWhitelist: (slug: string, minecraft_name: string, status: WhitelistStatus, note?: string) =>
    request<unknown>('POST', `/v1/servers/${slug}/whitelist`, { minecraft_name, status, note }),
  members: (slug: string) => request<Member[]>('GET', `/v1/servers/${slug}/members`),
  setMember: (slug: string, minecraft_name: string, role: string) =>
    request<Member[]>('PUT', `/v1/servers/${slug}/members`, { minecraft_name, role }),
  audit: (slug: string) => request<AuditEntry[]>('GET', `/v1/servers/${slug}/audit`),
  agent: (slug: string) => request<AgentStatus>('GET', `/v1/servers/${slug}/agent`),
  agentToken: (slug: string) => request<{ token: string }>('POST', `/v1/servers/${slug}/agent/token`),
  missingFiles: (sha1: string[]) => request<{ missing: string[] }>('POST', '/v1/files/missing', { sha1 }),
  plan: (slug: string, body: PlanRequest) => request<PlanResponse>('POST', `/v1/servers/${slug}/versions/plan`, body),
  planFromServer: (slug: string, body: { version: string; excluded?: string[]; include_server_only?: string[] }) =>
    request<PlanResponse>('POST', `/v1/servers/${slug}/versions/plan-from-server`, body),
  resolveFiles: (sha1: string[]) =>
    request<{ resolved: Record<string, ResolvedFile>; missing: string[] }>('POST', '/v1/files/resolve', { sha1 }),
  metaMinecraft: () => request<MinecraftOption[]>('GET', '/v1/meta/minecraft'),
  metaLoaders: (loader: Loader, minecraft: string) =>
    request<LoaderOption[]>('GET', `/v1/meta/loaders/${loader}/${encodeURIComponent(minecraft)}`),
  detectServer: (address: string) =>
    request<DetectedServer>('GET', `/v1/detect/server?address=${encodeURIComponent(address)}`),
  snapshot: (slug: string) => request<SnapshotView>('GET', `/v1/servers/${slug}/snapshot`),
  companionBuilds: () => request<CompanionBuilds>('GET', '/v1/companion/builds'),
  companionLink: (code: string) => request<LinkView>('GET', `/v1/companion/links/${encodeURIComponent(code)}`),
  confirmLink: (code: string, slug: string) =>
    request<{ ok: boolean; slug: string }>('POST', `/v1/companion/links/${encodeURIComponent(code)}`, { slug }),
  publish: (slug: string, manifest: PackManifest, channel: 'stable' | 'beta' | null) =>
    request<{ id: string; version: string }>('POST', `/v1/servers/${slug}/versions`, { manifest, channel }),
  featuredPlans: () => request<{ enabled: boolean; plans: { days: number; price_cents: number }[] }>('GET', '/v1/featured/plans'),
  featuredCheckout: (slug: string, days: number, payer?: { name: string; email: string }) =>
    request<{ purchase_id: string; checkout_url: string }>('POST', `/v1/servers/${slug}/featured/checkout`, { days, payer }),
  featuredPurchases: (slug: string) => request<FeaturedPurchase[]>('GET', `/v1/servers/${slug}/featured/purchases`),
  featuredPurchase: (slug: string, id: string) => request<FeaturedPurchase>('GET', `/v1/servers/${slug}/featured/purchases/${id}`),
  shopCatalog: () => request<ShopCatalog>('GET', '/v1/shop/catalog'),
  wallet: () => request<{ balance: number; premium_until: string | null }>('GET', '/v1/me/wallet'),
  order: (id: string) => request<ShopOrder>('GET', `/v1/me/orders/${encodeURIComponent(id)}`),
  /** PRO com dinheiro: devolve o link do Mercado Pago. */
  proCheckout: (slug: string, sku: string) =>
    request<{ order_id: string; checkout_url: string }>('POST', '/v1/shop/checkout', { sku, server: slug }),
  /** PRO ou destaque com Moedas. `key` é único por clique (não cobra duas vezes). */
  spendCoins: (slug: string, sku: string, key: string) =>
    request<{ order: ShopOrder; balance: number }>('POST', '/v1/shop/spend', { sku, server: slug, key }),
  supportAdmin: (slug: string) => request<SupportAdmin>('GET', `/v1/servers/${slug}/support`),
  payoutConnect: (slug: string) => request<{ url: string }>('POST', `/v1/servers/${slug}/payout/connect`),
  payoutDisconnect: (slug: string) => request<null>('DELETE', `/v1/servers/${slug}/payout`),
  payoutSettings: (slug: string, support_enabled: boolean) => request<null>('PUT', `/v1/servers/${slug}/payout`, { support_enabled }),
  stats: (slug: string) => request<ServerStats>('GET', `/v1/servers/${slug}/stats`),
  profile: (name: string) => request<PublicProfile>('GET', `/v1/profiles/${encodeURIComponent(name)}`),
  posts: (slug: string) => request<{ news: ServerPost[]; events: ServerPost[] }>('GET', `/v1/servers/${slug}/posts`),
  comments: (slug: string, before?: string) =>
    request<CommentPage>('GET', `/v1/servers/${slug}/comments?limit=50${before ? `&before=${encodeURIComponent(before)}` : ''}`),
  deleteComment: (slug: string, id: string) => request<null>('DELETE', `/v1/servers/${slug}/comments/${id}`),
  createPost: (slug: string, input: PostInput) => request<{ id: string }>('POST', `/v1/servers/${slug}/posts`, input),
  updatePost: (slug: string, id: string, input: PostInput) => request<null>('PUT', `/v1/servers/${slug}/posts/${id}`, input),
  deletePost: (slug: string, id: string) => request<null>('DELETE', `/v1/servers/${slug}/posts/${id}`),
  moderation: () => request<ModerationQueue>('GET', '/v1/staff/moderation'),
  reviewBackground: (name: string, approve: boolean) =>
    request<null>('POST', `/v1/staff/profiles/${encodeURIComponent(name)}/background`, { approve }),
  resolveReport: (id: string, action: 'dismiss' | 'clear' | 'ban', reason?: string) =>
    request<null>('POST', `/v1/staff/reports/${id}`, { action, reason }),
  banProfile: (name: string, ban: boolean, reason?: string) =>
    request<null>('POST', `/v1/staff/profiles/${encodeURIComponent(name)}/ban`, { ban, reason }),
  backgroundsSeen: (names: string[]) => request<{ seen: number }>('POST', '/v1/staff/backgrounds/seen', { names }),
  staffServers: () => request<Server[]>('GET', '/v1/staff/servers'),
  sponsors: () => request<StaffSponsor[]>('GET', '/v1/staff/sponsors'),
  createSponsor: (input: SponsorInput) => request<{ id: string }>('POST', '/v1/staff/sponsors', input),
  updateSponsor: (id: string, input: SponsorInput) => request<null>('PUT', `/v1/staff/sponsors/${id}`, input),
  deleteSponsor: (id: string) => request<null>('DELETE', `/v1/staff/sponsors/${id}`),
  sponsorStats: (id: string) => request<{ days: SponsorDay[] }>('GET', `/v1/staff/sponsors/${id}/stats`),
  waitlist: (status: WaitlistStatus, central = false) =>
    request<WaitlistList>('GET', `/v1/staff/waitlist?status=${status}${central ? '&central=true' : ''}`),
  decideWaitlist: (id: string, status: WaitlistStatus, note?: string) =>
    request<WaitlistEntry>('POST', `/v1/staff/waitlist/${id}`, { status, note }),
  approveNextWaitlist: (count: number, central = false) =>
    request<{ approved: number }>('POST', '/v1/staff/waitlist/next', { count, central }),
  manualWaitlist: (nick: string) => request<{ nick: string; existing: boolean }>('POST', '/v1/staff/waitlist/manual', { nick }),
  setApproval: (slug: string, approval: string) => request<unknown>('POST', `/v1/staff/servers/${slug}/approval`, { approval }),
  setFeatured: (slug: string, until: string | null) => request<unknown>('POST', `/v1/staff/servers/${slug}/featured`, { until }),
  setCentral: (slug: string, central: boolean) => request<unknown>('POST', `/v1/staff/servers/${slug}/central`, { central }),
  storage: () => request<StorageOverview>('GET', '/v1/staff/storage'),
  storageServer: (slug: string) => request<ServerStorage>('GET', `/v1/staff/storage/servers/${slug}`),
  deleteVersion: (slug: string, id: string) =>
    request<{ deleted: string; files_freed: number; bytes_freed: number }>('DELETE', `/v1/staff/storage/servers/${slug}/versions/${id}`),
  blockFile: (sha1: string, reason: string) =>
    request<{ freed: number; affected_versions: string[] }>('POST', `/v1/staff/storage/files/${sha1}`, { reason }),
  unblockFile: (sha1: string) => request<null>('DELETE', `/v1/staff/storage/files/${sha1}`),
  runCleanup: () => request<{ versions: number; files: number; bytes: number; temporaries: number }>('POST', '/v1/staff/storage/cleanup'),
}

export interface ServerUsage {
  slug: string
  name: string
  approval: Server['approval']
  featured: boolean
  versions: number
  files: number
  hosted_bytes: number
  exclusive_bytes: number
  current_bytes: number
  uploaded_24h: number
  last_published_at: string | null
}

export interface StorageOverview {
  disk: { total: number; free: number; used_percent: number | null; alert_percent: number }
  stored: { files: number; bytes: number }
  unused: { files: number; bytes: number }
  quota_bytes: number
  featured_quota_bytes: number
  servers: ServerUsage[]
  blocked: { sha1: string; size: number; path: string; reason: string; blocked_by: string | null; created_at: string }[]
}

export interface ServerStorage {
  versions: { id: string; version: string; published_at: string; channels: string[]; files: number; hosted_bytes: number }[]
  files: { sha1: string; path: string; size: number; created_at: string; versions: string[]; other_servers: string[] }[]
}

export async function loadSession() {
  if (!session.token) return
  const me = await api.me()
  session.user = me.user
  session.memberships = me.memberships
}

export type WaitlistStatus = 'pending' | 'approved' | 'rejected'

export interface WaitlistEntry {
  id: string
  discord_id: string | null
  discord_name: string
  discord_avatar: string | null
  minecraft_name: string
  minecraft_uuid: string | null
  account_type: 'microsoft' | 'offline'
  status: WaitlistStatus
  note: string
  created_at: string
  decided_at: string | null
  decided_by_name: string | null
  /** Link de origem (`?ref=central`). */
  ref?: string | null
  /** Membro verificado da Central SMP's (lote da parceria). */
  central_member?: boolean
}

export interface WaitlistList {
  enabled: boolean
  entries: WaitlistEntry[]
  pending: number
  approved: number
  rejected: number
  /** Aguardando e membros verificados da Central. */
  central_pending?: number
  /** Parceria com a Central configurada na API. */
  central_enabled?: boolean
}

export function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function formatDate(value: string | null): string {
  if (!value) return '—'
  return new Date(value).toLocaleString('pt-BR', { dateStyle: 'short', timeStyle: 'short' })
}

/**
 * Envia o conteúdo bruto de um arquivo (o SHA-1 é conferido pela API).
 *
 * Usa XMLHttpRequest porque o `fetch` não informa o progresso do envio: com ele a barra
 * só andava quando um arquivo inteiro terminava e parecia travada nos mods grandes.
 * `onProgress(bytes)` recebe quanto deste arquivo já foi enviado (volta a 0 se precisar
 * reenviar). Quedas de rede e erros do servidor são tentados de novo algumas vezes.
 */
export async function uploadFile(sha1: string, file: Blob, onProgress?: (sent: number) => void): Promise<void> {
  const attempts = 4
  for (let attempt = 1; ; attempt++) {
    try {
      await uploadOnce(sha1, file, onProgress)
      return
    } catch (e) {
      onProgress?.(0)
      const retry = e instanceof ApiError && (e.status === 0 || e.status === 429 || e.status >= 500)
      if (!retry || attempt >= attempts) throw e
      await new Promise((r) => setTimeout(r, 2000 * attempt))
    }
  }
}

function uploadOnce(sha1: string, file: Blob, onProgress?: (sent: number) => void): Promise<void> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest()
    xhr.open('PUT', `${API_URL}/v1/files/${sha1}`)
    xhr.setRequestHeader('Authorization', `Bearer ${session.token ?? ''}`)
    xhr.setRequestHeader('Content-Type', 'application/octet-stream')
    xhr.upload.onprogress = (event) => onProgress?.(event.loaded)
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        onProgress?.(file.size)
        resolve()
        return
      }
      let data: { message?: string } | null = null
      try {
        data = JSON.parse(xhr.responseText)
      } catch {
        // resposta sem JSON (ex.: proxy)
      }
      reject(new ApiError(data?.message ?? `Erro ${xhr.status} ao enviar arquivo`, xhr.status))
    }
    xhr.onerror = () => reject(new ApiError('Falha de rede ao enviar um arquivo.', 0))
    xhr.onabort = () => reject(new ApiError('Envio cancelado.', 0))
    xhr.send(file)
  })
}

export type SponsorPlacement = 'home' | 'server' | 'loading'

export interface SponsorInput {
  name: string
  image_url: string
  image_side_url: string | null
  link_url: string
  alt_text: string | null
  placements: SponsorPlacement[]
  weight: number
  starts_at: string | null
  ends_at: string | null
  active: boolean
}

export interface StaffSponsor extends SponsorInput {
  id: string
  starts_at: string
  live: boolean
  views_30d: number
  clicks_30d: number
  created_at: string
}

export interface SponsorDay {
  day: string
  placement: SponsorPlacement
  views: number
  clicks: number
}

/** Envia a imagem de um patrocinador (equipe). Devolve a URL limpa pela API. */
export async function uploadSponsorImage(image: Blob): Promise<string> {
  let response: Response
  try {
    response = await fetch(`${API_URL}/v1/staff/sponsors/image`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${session.token ?? ''}`, 'Content-Type': image.type || 'application/octet-stream' },
      body: image,
    })
  } catch {
    throw new ApiError('Falha de rede ao enviar a imagem.', 0)
  }
  const data = await response.json().catch(() => null)
  if (!response.ok) throw new ApiError(data?.message ?? `Erro ${response.status} ao enviar a imagem`, response.status)
  return data.url as string
}

/** Envia ícone ou banner do servidor (PNG, JPEG ou WebP). */
export async function uploadImage(slug: string, kind: 'icon' | 'banner', image: Blob): Promise<string> {
  let response: Response
  try {
    response = await fetch(`${API_URL}/v1/servers/${slug}/images/${kind}`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${session.token ?? ''}`, 'Content-Type': image.type || 'application/octet-stream' },
      body: image,
    })
  } catch {
    throw new ApiError('Falha de rede ao enviar a imagem.', 0)
  }
  const data = await response.json().catch(() => null)
  if (!response.ok) throw new ApiError(data?.message ?? `Erro ${response.status} ao enviar a imagem`, response.status)
  return data.url as string
}

/** Imagem (PNG, JPEG, WebP) ou GIF animado para os blocos da página personalizada. */
export async function uploadPageMedia(slug: string, file: Blob): Promise<{ url: string; kind: 'image' | 'gif'; bytes: number }> {
  let response: Response
  try {
    response = await fetch(`${API_URL}/v1/servers/${slug}/page/media`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${session.token ?? ''}`, 'Content-Type': file.type || 'application/octet-stream' },
      body: file,
    })
  } catch {
    throw new ApiError('Falha de rede ao enviar o arquivo.', 0)
  }
  const data = await response.json().catch(() => null)
  if (!response.ok) throw new ApiError(data?.message ?? `Erro ${response.status} ao enviar o arquivo`, response.status)
  return data
}

/** ID do vídeo a partir de um link do YouTube (a API confere de novo). */
export function youtubeId(input: string): string | null {
  const text = input.trim()
  if (/^[A-Za-z0-9_-]{11}$/.test(text)) return text
  try {
    const url = new URL(text)
    const host = url.hostname.replace(/^(www|m|music)\./, '')
    let id: string | null = null
    if (host === 'youtu.be') id = url.pathname.split('/')[1] ?? null
    else if (host === 'youtube.com' || host === 'youtube-nocookie.com') {
      const [, first, second] = url.pathname.split('/')
      id = first === 'watch' ? url.searchParams.get('v') : ['shorts', 'embed', 'live', 'v'].includes(first) ? (second ?? null) : null
    }
    return id && /^[A-Za-z0-9_-]{11}$/.test(id) ? id : null
  } catch {
    return null
  }
}

/** `data:image/png;base64,...` → Blob (ícone detectado no ping do servidor). */
export function dataUrlToBlob(dataUrl: string): Blob {
  const [head, body] = dataUrl.split(',')
  const mime = head.match(/data:([^;]+)/)?.[1] ?? 'image/png'
  const bytes = Uint8Array.from(atob(body), (c) => c.charCodeAt(0))
  return new Blob([bytes], { type: mime })
}

export async function sha1Hex(file: Blob): Promise<string> {
  if (!crypto?.subtle) throw new Error('Abra o painel por HTTPS para calcular os hashes dos arquivos.')
  const digest = await crypto.subtle.digest('SHA-1', await file.arrayBuffer())
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('')
}

/**
 * Calcula o SHA-1 de vários arquivos, alguns ao mesmo tempo (antes era um por vez).
 * `onProgress(prontos, total)` é chamado a cada arquivo concluído.
 */
export async function sha1All(
  files: { file: Blob; sha1?: string | null }[],
  onProgress?: (done: number, total: number) => void,
  parallel = 4,
): Promise<void> {
  let done = 0
  let next = 0
  const worker = async () => {
    while (next < files.length) {
      const item = files[next++]
      if (!item.sha1) item.sha1 = await sha1Hex(item.file)
      done++
      onProgress?.(done, files.length)
    }
  }
  await Promise.all(Array.from({ length: Math.min(parallel, files.length) }, worker))
}

/** "45 s", "3 min", "1 h 20 min". */
export function formatDuration(seconds: number): string {
  const s = Math.max(1, Math.round(seconds))
  if (s < 60) return `${s} s`
  const m = Math.round(s / 60)
  if (m < 60) return `${m} min`
  return `${Math.floor(m / 60)} h ${m % 60} min`
}

/** Um JAR do Conexão Companion (um por versão do Minecraft × loader). */
export interface CompanionBuild {
  /** Versão para a qual foi compilado. */
  minecraft: string
  /** Versões em que funciona (Fabric cobre faixas; Forge/NeoForge só a exata). */
  supports: string[]
  loader: Loader
  java: number
  file: string
  size: number
  sha1: string
  url: string
}

export interface CompanionBuilds {
  version: string | null
  builds: CompanionBuild[]
}

/** O JAR certo para o servidor: mesmo loader e, de preferência, a mesma versão do Minecraft. */
export function pickCompanionBuild(builds: CompanionBuild[], minecraft: string | null | undefined, loader: Loader | null | undefined): CompanionBuild | null {
  if (!minecraft || !loader) return null
  const same = builds.filter((b) => b.loader === loader)
  return same.find((b) => b.minecraft === minecraft) ?? same.find((b) => b.supports.includes(minecraft)) ?? null
}

/** Compara versões tipo 1.21.10 / 26.1.2 (negativo = a < b). */
export function compareVersions(a: string, b: string): number {
  const pa = a.split(/[.-]/).map((x) => parseInt(x, 10) || 0)
  const pb = b.split(/[.-]/).map((x) => parseInt(x, 10) || 0)
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (d) return d
  }
  return 0
}

/** `companion-neoforge-0.2.0` → `0.2.0` (versão do Companion conectado). */
export function companionVersionOf(agentVersion: string | null | undefined): string | null {
  const m = agentVersion?.match(/^companion-.*-(\d+\.\d+\.\d+)$/)
  return m ? m[1] : null
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(0)} KB`
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`
}

export const money = (cents: number) => (cents / 100).toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' })

/** Só segue links de pagamento/autorização do Mercado Pago. */
export function goToMercadoPago(url: string) {
  const parsed = new URL(url)
  if (parsed.protocol !== 'https:' || !/(^|\.)mercadopago\.com(\.br)?$|(^|\.)mercadolivre\.com(\.br)?$/.test(parsed.hostname)) {
    throw new Error('Link de pagamento inesperado. Fale com a equipe Conexão.')
  }
  window.location.href = url
}

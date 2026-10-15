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
  status: { online: boolean; players_online: number; players_max: number; latency_ms: number | null; checked_at: string | null }
  current_version: string | null
  approval: 'pending' | 'approved' | 'rejected' | 'suspended'
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
  profile: (name: string) => request<PublicProfile>('GET', `/v1/profiles/${encodeURIComponent(name)}`),
  posts: (slug: string) => request<{ news: ServerPost[]; events: ServerPost[] }>('GET', `/v1/servers/${slug}/posts`),
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
  setApproval: (slug: string, approval: string) => request<unknown>('POST', `/v1/staff/servers/${slug}/approval`, { approval }),
  setFeatured: (slug: string, until: string | null) => request<unknown>('POST', `/v1/staff/servers/${slug}/featured`, { until }),
}

export async function loadSession() {
  if (!session.token) return
  const me = await api.me()
  session.user = me.user
  session.memberships = me.memberships
}

export function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function formatDate(value: string | null): string {
  if (!value) return '—'
  return new Date(value).toLocaleString('pt-BR', { dateStyle: 'short', timeStyle: 'short' })
}

/** Envia o conteúdo bruto de um arquivo (o SHA-1 é conferido pela API). */
export async function uploadFile(sha1: string, file: Blob): Promise<void> {
  let response: Response
  try {
    response = await fetch(`${API_URL}/v1/files/${sha1}`, {
      method: 'PUT',
      headers: { Authorization: `Bearer ${session.token ?? ''}`, 'Content-Type': 'application/octet-stream' },
      body: file,
    })
  } catch {
    throw new ApiError('Falha de rede ao enviar um arquivo.', 0)
  }
  if (!response.ok) {
    const data = await response.json().catch(() => null)
    throw new ApiError(data?.message ?? `Erro ${response.status} ao enviar arquivo`, response.status)
  }
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

/** Download do Conexão Companion (publicado junto com o launcher). */
export const COMPANION_DOWNLOAD = `${API_URL}/launcher/companion/conexao-companion-neoforge-1.21.1.jar`

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(0)} KB`
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`
}

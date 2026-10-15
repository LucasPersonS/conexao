/**
 * Detecção automática do modpack que o parceiro arrasta para o painel.
 *
 * Aceita: pasta da instância (Prism/MultiMC, CurseForge App, Modrinth App,
 * .minecraft), pasta do servidor, `.mrpack` e `.zip` exportado pelo CurseForge.
 * Descobre Minecraft, loader e versão do loader sem perguntar nada; quando o
 * pacote não diz, deduz pelos metadados dos próprios mods.
 */
import { strFromU8, unzipSync, type Unzipped } from 'fflate'
import type { Loader } from './api'

export const GROUPS = ['mods', 'config', 'defaultconfigs', 'resourcepacks', 'shaderpacks', 'kubejs']

export interface InputFile {
  /** Caminho relativo ao que foi arrastado, com `/`. */
  path: string
  file: File
}

export interface PackFileEntry {
  path: string
  file: File
  sha1?: string
}

export interface ExternalEntry {
  path: string
  sha1: string
  sha512: string
  size: number
  url: string
}

export interface DetectedPack {
  source: 'mrpack' | 'curseforge' | 'prism' | 'curseforge-app' | 'server' | 'folder'
  sourceLabel: string
  files: PackFileEntry[]
  external: ExternalEntry[]
  minecraft: string | null
  loader: Loader | null
  loaderVersion: string | null
  /** De onde vieram as versões (mostrado ao parceiro). */
  evidence: string | null
  notes: string[]
  ignored: number
}

// ----------------------------------------------------------------- arrastar

interface FsEntry {
  isFile: boolean
  isDirectory: boolean
  name: string
  file?: (ok: (f: File) => void, err: (e: unknown) => void) => void
  createReader?: () => { readEntries: (ok: (entries: FsEntry[]) => void, err: (e: unknown) => void) => void }
}

async function walkEntry(entry: FsEntry, prefix: string, out: InputFile[]) {
  if (entry.isFile && entry.file) {
    const file = await new Promise<File>((ok, err) => entry.file?.(ok, err))
    out.push({ path: `${prefix}${entry.name}`, file })
  } else if (entry.isDirectory && entry.createReader) {
    const reader = entry.createReader()
    for (;;) {
      const batch = await new Promise<FsEntry[]>((ok, err) => reader.readEntries(ok, err))
      if (!batch.length) break
      for (const child of batch) await walkEntry(child, `${prefix}${entry.name}/`, out)
    }
  }
}

/** Lê pastas e arquivos soltos em uma área de "arrastar e soltar". */
export async function filesFromDrop(transfer: DataTransfer): Promise<InputFile[]> {
  const out: InputFile[] = []
  const entries: FsEntry[] = []
  for (const item of Array.from(transfer.items)) {
    const entry = item.webkitGetAsEntry?.() as unknown as FsEntry | null
    if (entry) entries.push(entry)
  }
  if (entries.length) {
    for (const entry of entries) await walkEntry(entry, '', out)
    return out
  }
  return Array.from(transfer.files).map((file) => ({ path: file.name, file }))
}

/** Arquivos de um `<input type=file webkitdirectory>` ou de arquivo único. */
export function filesFromInput(list: FileList | null): InputFile[] {
  return Array.from(list ?? []).map((file) => ({ path: file.webkitRelativePath || file.name, file }))
}

// ------------------------------------------------------------------ versões

/** Comparação de versões numéricas (1.21.1 < 1.21.10 < 26.1). */
export function compareVersions(a: string, b: string): number {
  const pa = a.split(/[.-]/).map((p) => (/^\d+$/.test(p) ? Number(p) : p))
  const pb = b.split(/[.-]/).map((p) => (/^\d+$/.test(p) ? Number(p) : p))
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const x = pa[i] ?? 0
    const y = pb[i] ?? 0
    if (typeof x === 'number' && typeof y === 'number') {
      if (x !== y) return x - y
    } else if (String(x) !== String(y)) {
      return String(x) < String(y) ? -1 : 1
    }
  }
  return 0
}

type Test = (version: string) => boolean

/** Intervalos do Maven usados por Forge/NeoForge: `[1.21.1,1.22)`, `[1.21]`, `1.21.1`. */
export function mavenRange(spec: string): Test {
  const trimmed = spec.trim()
  if (!trimmed || trimmed === '*') return () => true
  const intervals = [...trimmed.matchAll(/([[(])\s*([^,\])]*)\s*(?:,\s*([^\])]*))?\s*([\])])/g)]
  if (!intervals.length) return (v) => compareVersions(v, trimmed) >= 0
  return (v) =>
    intervals.some(([, open, low, high, close]) => {
      const hasComma = high !== undefined
      if (!hasComma) return compareVersions(v, low) === 0
      if (low && (open === '[' ? compareVersions(v, low) < 0 : compareVersions(v, low) <= 0)) return false
      if (high && (close === ']' ? compareVersions(v, high) > 0 : compareVersions(v, high) >= 0)) return false
      return true
    })
}

/** Intervalos do Fabric: `~1.21.1`, `1.21.x`, `>=1.21 <1.22`, `1.21.1`, `*`, arrays e `||`. */
export function fabricRange(spec: string | string[]): Test {
  const alternatives = (Array.isArray(spec) ? spec : String(spec).split('||')).map((s) => s.trim())
  const tests = alternatives.map((alt): Test => {
    if (!alt || alt === '*') return () => true
    const terms = alt.split(/\s+/)
    const checks = terms.map((term): Test => {
      const m = term.match(/^(>=|<=|>|<|=|~|\^)?(.+)$/)
      if (!m) return () => true
      const [, op = '', raw] = m
      if (/\.x$|\.\*$/.test(raw)) {
        const base = raw.replace(/\.(x|\*)$/, '')
        return (v) => v === base || v.startsWith(`${base}.`)
      }
      const version = raw.replace(/-.*$/, '')
      switch (op) {
        case '>=':
          return (v) => compareVersions(v, version) >= 0
        case '>':
          return (v) => compareVersions(v, version) > 0
        case '<=':
          return (v) => compareVersions(v, version) <= 0
        case '<':
          return (v) => compareVersions(v, version) < 0
        case '~': {
          const [major, minor] = version.split('.')
          return (v) => compareVersions(v, version) >= 0 && v.split('.').slice(0, 2).join('.') === `${major}.${minor}`
        }
        case '^': {
          const [major] = version.split('.')
          return (v) => compareVersions(v, version) >= 0 && v.split('.')[0] === major
        }
        default:
          return (v) => compareVersions(v, version) === 0
      }
    })
    return (v) => checks.every((check) => check(v))
  })
  return (v) => tests.some((test) => test(v))
}

/** NeoForge 21.1.77 → Minecraft 1.21.1; 20.4.x → 1.20.4; 26.1.0.19 → 26.1. */
export function minecraftFromNeoForge(version: string): string | null {
  const parts = version.split(/[.-]/)
  if (parts.length < 2) return null
  const [major, minor] = parts
  if (Number(major) >= 26) {
    const patch = parts[2] && parts[2] !== '0' ? `.${parts[2]}` : ''
    return `${major}.${minor}${patch}`
  }
  return minor === '0' ? `1.${major}` : `1.${major}.${minor}`
}

// --------------------------------------------------------------- metadados

interface JarInfo {
  loader: Loader | null
  minecraft: Test | null
  /** Limite inferior declarado para Minecraft (desempate). */
  minecraftLow: string | null
}

function tomlDependency(toml: string, modId: string): string | null {
  // Procura o bloco [[dependencies.x]] cujo modId é o pedido e devolve o versionRange.
  const blocks = toml.split(/\[\[dependencies\.[^\]]+\]\]/).slice(1)
  for (const block of blocks) {
    const id = block.match(/modId\s*=\s*"([^"]+)"/)?.[1]
    if (id === modId) return block.match(/versionRange\s*=\s*"([^"]*)"/)?.[1] ?? null
  }
  return null
}

function lowerBound(range: string): string | null {
  return range.match(/[[(]\s*([0-9][^,\])]*)/)?.[1] ?? (/^[0-9]/.test(range.trim()) ? range.trim() : null)
}

export function readJarInfo(bytes: Uint8Array): JarInfo | null {
  let files: Unzipped
  try {
    files = unzipSync(bytes, {
      filter: (f) => ['META-INF/neoforge.mods.toml', 'META-INF/mods.toml', 'fabric.mod.json', 'quilt.mod.json'].includes(f.name),
    })
  } catch {
    return null
  }
  const neo = files['META-INF/neoforge.mods.toml']
  const forge = files['META-INF/mods.toml']
  if (neo || forge) {
    const toml = strFromU8((neo ?? forge) as Uint8Array)
    const range = tomlDependency(toml, 'minecraft')
    const isNeo = !!neo || tomlDependency(toml, 'neoforge') !== null
    return {
      loader: isNeo ? 'neoforge' : 'forge',
      minecraft: range ? mavenRange(range) : null,
      minecraftLow: range ? lowerBound(range) : null,
    }
  }
  const fabric = files['fabric.mod.json'] ?? files['quilt.mod.json']
  if (fabric) {
    try {
      const json = JSON.parse(strFromU8(fabric))
      const spec = json.depends?.minecraft ?? json.quilt_loader?.depends?.find?.((d: { id: string }) => d.id === 'minecraft')?.versions
      return {
        loader: 'fabric',
        minecraft: spec ? fabricRange(spec) : null,
        minecraftLow: typeof spec === 'string' ? spec.replace(/^[^0-9]*/, '').split(/\s/)[0] || null : null,
      }
    } catch {
      return { loader: 'fabric', minecraft: null, minecraftLow: null }
    }
  }
  return null
}

/** Escolhe Minecraft e loader pela "votação" dos mods. */
export async function inferFromJars(
  jars: PackFileEntry[],
  knownVersions: string[],
  onProgress?: (done: number, total: number) => void,
): Promise<{ minecraft: string | null; loader: Loader | null; scanned: number }> {
  const sample = jars.slice(0, 400)
  const infos: JarInfo[] = []
  for (let i = 0; i < sample.length; i++) {
    const info = readJarInfo(new Uint8Array(await sample[i].file.arrayBuffer()))
    if (info) infos.push(info)
    onProgress?.(i + 1, sample.length)
  }
  const loaderVotes = new Map<Loader, number>()
  for (const info of infos) if (info.loader) loaderVotes.set(info.loader, (loaderVotes.get(info.loader) ?? 0) + 1)
  const loader = [...loaderVotes.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? null

  const withRange = infos.filter((i) => i.minecraft)
  let minecraft: string | null = null
  if (withRange.length && knownVersions.length) {
    let best = -1
    let bestLow = -1
    for (const version of knownVersions) {
      const score = withRange.filter((i) => i.minecraft?.(version)).length
      const lowScore = withRange.filter((i) => i.minecraftLow && compareVersions(i.minecraftLow, version) === 0).length
      if (score > best || (score === best && lowScore > bestLow)) {
        best = score
        bestLow = lowScore
        minecraft = version
      }
    }
  }
  return { minecraft, loader, scanned: infos.length }
}

// ------------------------------------------------------------------ pastas

function loaderFromComponent(uid: string): Loader | null {
  if (uid === 'net.neoforged') return 'neoforge'
  if (uid === 'net.minecraftforge') return 'forge'
  if (uid === 'net.fabricmc.fabric-loader' || uid === 'org.quiltmc.quilt-loader') return 'fabric'
  return null
}

/** `neoforge-21.1.77`, `forge-47.2.0`, `fabric-0.16.9` (CurseForge). */
function parseCurseForgeLoader(id: string): { loader: Loader | null; version: string | null } {
  const [name, ...rest] = id.split('-')
  const version = rest.join('-') || null
  const loader: Loader | null = name === 'neoforge' ? 'neoforge' : name === 'forge' ? 'forge' : name === 'fabric' || name === 'quilt' ? 'fabric' : null
  return { loader, version }
}

async function readJson(file: File): Promise<unknown> {
  try {
    return JSON.parse(await file.text())
  } catch {
    return null
  }
}

function emptyPack(source: DetectedPack['source'], sourceLabel: string): DetectedPack {
  return { source, sourceLabel, files: [], external: [], minecraft: null, loader: null, loaderVersion: null, evidence: null, notes: [], ignored: 0 }
}

async function detectFolder(inputs: InputFile[]): Promise<DetectedPack> {
  // Base = prefixo antes da primeira pasta gerenciada (mods/, config/…), o mais curto.
  let base: string[] | null = null
  for (const input of inputs) {
    const parts = input.path.split('/')
    const idx = parts.findIndex((p, i) => i < parts.length - 1 && p === 'mods')
    if (idx >= 0 && (!base || idx < base.length)) base = parts.slice(0, idx)
  }
  if (!base) {
    for (const input of inputs) {
      const parts = input.path.split('/')
      const idx = parts.findIndex((p, i) => i < parts.length - 1 && GROUPS.includes(p))
      if (idx >= 0 && (!base || idx < base.length)) base = parts.slice(0, idx)
    }
  }
  const prefix = base ? base.join('/') : ''
  const pack = emptyPack('folder', 'Pasta do modpack')
  for (const input of inputs) {
    const rel = prefix && input.path.startsWith(`${prefix}/`) ? input.path.slice(prefix.length + 1) : input.path
    const parts = rel.split('/')
    const name = parts[parts.length - 1]
    if (parts.length < 2 || !GROUPS.includes(parts[0]) || name.startsWith('.')) {
      pack.ignored++
      continue
    }
    // O próprio Companion só roda no servidor.
    if (parts[0] === 'mods' && (!name.toLowerCase().endsWith('.jar') || parts.length !== 2 || /^(conexao|central)-companion/.test(name.toLowerCase()))) {
      pack.ignored++
      continue
    }
    pack.files.push({ path: rel, file: input.file })
  }

  const byName = (name: string) =>
    inputs.filter((i) => i.path.split('/').pop() === name).sort((a, b) => a.path.length - b.path.length)[0]

  // Prism Launcher / MultiMC
  const mmc = byName('mmc-pack.json')
  if (mmc) {
    const json = (await readJson(mmc.file)) as { components?: { uid: string; version?: string }[] } | null
    for (const c of json?.components ?? []) {
      if (c.uid === 'net.minecraft') pack.minecraft = c.version ?? null
      const loader = loaderFromComponent(c.uid)
      if (loader) {
        pack.loader = loader
        pack.loaderVersion = c.version ?? null
      }
    }
    if (pack.minecraft) {
      pack.source = 'prism'
      pack.sourceLabel = 'Instância do Prism Launcher / MultiMC'
      pack.evidence = 'mmc-pack.json'
      return pack
    }
  }
  // CurseForge App
  const cfApp = byName('minecraftinstance.json')
  if (cfApp) {
    const json = (await readJson(cfApp.file)) as { gameVersion?: string; baseModLoader?: { name?: string; minecraftVersion?: string } } | null
    const parsed = parseCurseForgeLoader(json?.baseModLoader?.name ?? '')
    pack.minecraft = json?.baseModLoader?.minecraftVersion ?? json?.gameVersion ?? null
    pack.loader = parsed.loader
    pack.loaderVersion = parsed.version
    if (pack.minecraft) {
      pack.source = 'curseforge-app'
      pack.sourceLabel = 'Instância do CurseForge App'
      pack.evidence = 'minecraftinstance.json'
      return pack
    }
  }
  // Pasta de servidor: libraries/ diz a versão exata do loader.
  for (const input of inputs) {
    const neo = input.path.match(/libraries\/net\/neoforged\/neoforge\/([^/]+)\//)
    if (neo) {
      pack.loader = 'neoforge'
      pack.loaderVersion = neo[1]
      pack.minecraft = minecraftFromNeoForge(neo[1])
    }
    const forge = input.path.match(/libraries\/net\/minecraftforge\/forge\/([^/]+)-([^/]+)\//)
    if (forge && !pack.loader) {
      pack.loader = 'forge'
      pack.minecraft = forge[1]
      pack.loaderVersion = forge[2]
    }
    const fabric = input.path.match(/libraries\/net\/fabricmc\/fabric-loader\/([^/]+)\//)
    if (fabric && !pack.loader) {
      pack.loader = 'fabric'
      pack.loaderVersion = fabric[1]
    }
    const vanilla = input.path.match(/libraries\/net\/minecraft\/server\/([^/]+)\//) ?? input.path.match(/(?:^|\/)versions\/(\d[^/]*)\/\1\.jar$/)
    if (vanilla && pack.loader === 'fabric') pack.minecraft = vanilla[1]
    if (pack.loader && pack.minecraft) break
  }
  if (pack.loader) {
    pack.source = 'server'
    pack.sourceLabel = 'Pasta do servidor'
    pack.evidence = 'pasta libraries/ do servidor'
    pack.notes.push('Mods que só rodam no servidor ficam de fora do pacote dos jogadores automaticamente.')
  }
  return pack
}

// ---------------------------------------------------------------- arquivos

function fileFrom(bytes: Uint8Array, name: string): File {
  return new File([bytes as BlobPart], name)
}

function collectOverrides(unzipped: Unzipped, prefixes: string[], pack: DetectedPack) {
  for (const [name, bytes] of Object.entries(unzipped)) {
    if (name.endsWith('/')) continue
    const prefix = prefixes.find((p) => name.startsWith(p))
    if (!prefix) continue
    const rel = name.slice(prefix.length)
    const parts = rel.split('/')
    if (parts.length < 2 || !GROUPS.includes(parts[0])) {
      pack.ignored++
      continue
    }
    pack.files = pack.files.filter((f) => f.path !== rel) // client-overrides vence overrides
    pack.files.push({ path: rel, file: fileFrom(bytes, parts[parts.length - 1]) })
  }
}

async function detectArchive(input: InputFile): Promise<DetectedPack> {
  const bytes = new Uint8Array(await input.file.arrayBuffer())
  let unzipped: Unzipped
  try {
    unzipped = unzipSync(bytes)
  } catch {
    throw new Error('Não foi possível abrir o arquivo. Envie um .mrpack, um .zip do CurseForge ou a pasta do modpack.')
  }
  const index = unzipped['modrinth.index.json']
  if (index) {
    const json = JSON.parse(strFromU8(index)) as {
      dependencies: Record<string, string>
      files: { path: string; hashes: Record<string, string>; downloads: string[]; fileSize: number; env?: { client?: string } }[]
    }
    const pack = emptyPack('mrpack', 'Modpack do Modrinth (.mrpack)')
    const deps = json.dependencies ?? {}
    pack.minecraft = deps.minecraft ?? null
    for (const [key, loader] of [['neoforge', 'neoforge'], ['forge', 'forge'], ['fabric-loader', 'fabric'], ['quilt-loader', 'fabric']] as const) {
      if (deps[key]) {
        pack.loader = loader
        pack.loaderVersion = deps[key]
      }
    }
    pack.evidence = 'modrinth.index.json'
    for (const f of json.files ?? []) {
      if (f.env?.client === 'unsupported') continue
      if (!f.hashes?.sha1 || !f.hashes?.sha512 || !f.downloads?.length) continue
      pack.external.push({ path: f.path, sha1: f.hashes.sha1, sha512: f.hashes.sha512, size: f.fileSize, url: f.downloads[0] })
    }
    collectOverrides(unzipped, ['overrides/', 'client-overrides/'], pack)
    return pack
  }
  const manifest = unzipped['manifest.json']
  if (manifest) {
    const json = JSON.parse(strFromU8(manifest)) as {
      minecraft?: { version?: string; modLoaders?: { id: string; primary?: boolean }[] }
      files?: unknown[]
      overrides?: string
    }
    const pack = emptyPack('curseforge', 'Modpack exportado do CurseForge (.zip)')
    pack.minecraft = json.minecraft?.version ?? null
    const primary = json.minecraft?.modLoaders?.find((l) => l.primary) ?? json.minecraft?.modLoaders?.[0]
    if (primary) {
      const parsed = parseCurseForgeLoader(primary.id)
      pack.loader = parsed.loader
      pack.loaderVersion = parsed.version
    }
    pack.evidence = 'manifest.json do CurseForge'
    collectOverrides(unzipped, [`${json.overrides ?? 'overrides'}/`], pack)
    const listed = json.files?.length ?? 0
    if (listed) {
      pack.notes.push(
        `Este .zip só lista ${listed} mods do CurseForge (sem os arquivos). Arraste a pasta da instância (com a pasta mods/) para incluí-los.`,
      )
    }
    return pack
  }
  // .zip comum com mods/, config/… dentro
  const inputs: InputFile[] = Object.entries(unzipped)
    .filter(([name]) => !name.endsWith('/'))
    .map(([name, data]) => ({ path: `${input.file.name}/${name}`, file: fileFrom(data, name.split('/').pop() ?? name) }))
  return detectFolder(inputs)
}

export async function detectPack(inputs: InputFile[]): Promise<DetectedPack> {
  if (!inputs.length) throw new Error('Nada foi selecionado.')
  if (inputs.length === 1 && /\.(mrpack|zip)$/i.test(inputs[0].file.name)) return detectArchive(inputs[0])
  return detectFolder(inputs)
}

/** Com os mods reconhecidos no Modrinth, confirma Minecraft e loader por votação. */
export function inferFromModrinth(
  resolved: { game_versions: string[]; loaders: string[] }[],
  knownVersions: string[],
): { minecraft: string | null; loader: Loader | null; votes: number } {
  const mods = resolved.filter((r) => r.loaders.some((l) => ['neoforge', 'forge', 'fabric', 'quilt'].includes(l)))
  if (!mods.length) return { minecraft: null, loader: null, votes: 0 }
  let minecraft: string | null = null
  let best = 0
  for (const version of knownVersions) {
    const score = mods.filter((m) => m.game_versions.includes(version)).length
    if (score > best) {
      best = score
      minecraft = version
    }
  }
  const votes = new Map<Loader, number>()
  for (const m of mods) {
    for (const l of m.loaders) {
      const loader: Loader | null = l === 'neoforge' ? 'neoforge' : l === 'forge' ? 'forge' : l === 'fabric' || l === 'quilt' ? 'fabric' : null
      if (loader) votes.set(loader, (votes.get(loader) ?? 0) + 1)
    }
  }
  // Mods multiplataforma votam em vários loaders; o mais votado entre os exclusivos decide.
  const exclusive = new Map<Loader, number>()
  for (const m of mods) {
    const own = [...new Set(m.loaders.map((l) => (l === 'quilt' ? 'fabric' : l)).filter((l) => ['neoforge', 'forge', 'fabric'].includes(l)))]
    if (own.length === 1) exclusive.set(own[0] as Loader, (exclusive.get(own[0] as Loader) ?? 0) + 1)
  }
  const pick = (map: Map<Loader, number>) => [...map.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? null
  return { minecraft, loader: pick(exclusive) ?? pick(votes), votes: best }
}

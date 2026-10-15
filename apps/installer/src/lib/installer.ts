// Ponte com o Rust (src-tauri/src/main.rs). Fora do Tauri (npm run dev no navegador)
// usa uma simulação, para dar para ajustar o visual sem compilar nada.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

export interface ReleaseInfo {
  version: string
  notes: string
  size: number | null
  installed: boolean
}

export type Progress =
  | { stage: 'download'; downloaded: number; total: number | null; bytes_per_sec: number }
  | { stage: 'verify' }
  | { stage: 'install' }
  | { stage: 'shortcut' }

const inTauri = '__TAURI_INTERNALS__' in window
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

export async function releaseInfo(): Promise<ReleaseInfo> {
  if (inTauri) return invoke<ReleaseInfo>('release_info')
  await sleep(700)
  return { version: '0.1.0', notes: '', size: 9_477_596, installed: false }
}

export async function install(desktopShortcut: boolean, onProgress: (p: Progress) => void): Promise<void> {
  if (inTauri) {
    const stop = await listen<Progress>('progress', (e) => onProgress(e.payload))
    try {
      await invoke('install', { desktopShortcut })
    } finally {
      stop()
    }
    return
  }
  const total = 9_477_596
  for (let done = 0; done < total; done += 420_000) {
    onProgress({ stage: 'download', downloaded: done, total, bytes_per_sec: 3_400_000 })
    await sleep(110)
  }
  onProgress({ stage: 'download', downloaded: total, total, bytes_per_sec: 0 })
  onProgress({ stage: 'verify' })
  await sleep(600)
  onProgress({ stage: 'install' })
  await sleep(2200)
  onProgress({ stage: 'shortcut' })
  await sleep(300)
}

export async function launch(): Promise<void> {
  if (inTauri) await invoke('launch')
}

export function minimize() {
  if (inTauri) getCurrentWindow().minimize()
}

export function close() {
  if (inTauri) getCurrentWindow().close()
}

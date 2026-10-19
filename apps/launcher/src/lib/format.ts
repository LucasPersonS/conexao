/** "1,2 GB", "350 MB" */
export function formatBytes(bytes: number | null | undefined): string | null {
  if (!bytes) return null
  const mb = bytes / 1024 / 1024
  if (mb >= 1024) return `${(mb / 1024).toLocaleString('pt-BR', { maximumFractionDigits: 1 })} GB`
  return `${Math.max(1, Math.round(mb)).toLocaleString('pt-BR')} MB`
}

export function formatNumber(n: number): string {
  return n.toLocaleString('pt-BR')
}

export function timeAgo(at: number): string {
  const minutes = Math.round((Date.now() - at) / 60_000)
  if (minutes < 1) return 'agora há pouco'
  if (minutes < 60) return `há ${minutes} min`
  const hours = Math.round(minutes / 60)
  if (hours < 24) return `há ${hours} h`
  const days = Math.round(hours / 24)
  return days === 1 ? 'ontem' : `há ${days} dias`
}

export type PingQuality = 'good' | 'ok' | 'bad' | 'none'

export function pingQuality(ms: number | null | undefined): PingQuality {
  if (ms == null) return 'none'
  if (ms < 80) return 'good'
  if (ms < 160) return 'ok'
  return 'bad'
}

/**
 * Traduz erros técnicos (rede, Java, disco) em algo que o jogador entenda.
 * Mensagens que já vêm escritas para o jogador passam direto.
 */
export function friendlyError(raw: string): string {
  const text = raw.toLowerCase()
  if (/whitelist|banido|assinatura|entre com sua conta|verifique sua conex/.test(text)) return raw
  if (/no space|os error 112|espaço/.test(text))
    return 'Não há espaço livre suficiente no disco para instalar o modpack. Libere espaço e tente de novo.'
  if (/error fetching url|builder error|error sending request|dns|timed out|connection|conex|network|rede/.test(text))
    return 'Não foi possível baixar os arquivos do servidor. Verifique sua internet e tente de novo.'
  if (/java|jre/.test(text)) return 'Não foi possível preparar o Java para o jogo. Tente de novo em instantes.'
  if (/hash|sha1|corrupt/.test(text)) return 'Um arquivo do modpack veio corrompido. Tente de novo; se continuar, avise a equipe do servidor.'
  return 'Algo deu errado ao preparar o jogo. Tente de novo; se continuar, envie os detalhes à equipe do servidor.'
}

/** `background-image` seguro para qualquer URL (aspas e parênteses incluídos). */
export function bgImage(url: string | null | undefined): Record<string, string> | undefined {
  return url ? { backgroundImage: `url("${url.replace(/"/g, '%22')}")` } : undefined
}

const WEEKDAY = new Intl.DateTimeFormat('pt-BR', { weekday: 'short', day: '2-digit', month: '2-digit' })
const HOUR = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' })

/** "Acontecendo agora", "Hoje, 20:00", "Amanhã, 20:00", "sáb., 18/10, 20:00" */
export function formatWhen(iso: string, now = new Date()): string {
  const at = new Date(iso)
  if (at <= now) return 'Acontecendo agora'
  const day = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
  const diff = Math.round((day(at) - day(now)) / 86_400_000)
  const hour = HOUR.format(at)
  if (diff === 0) return `Hoje, ${hour}`
  if (diff === 1) return `Amanhã, ${hour}`
  return `${WEEKDAY.format(at)}, ${hour}`
}

/** "em 25 min", "em 3 h" (só para menos de um dia) */
export function startsIn(iso: string, now = Date.now()): string | null {
  const minutes = Math.round((new Date(iso).getTime() - now) / 60_000)
  if (minutes <= 0 || minutes >= 24 * 60) return null
  return minutes < 60 ? `em ${minutes} min` : `em ${Math.round(minutes / 60)} h`
}

/** 990 → "R$ 9,90" */
export function formatMoney(cents: number): string {
  return (cents / 100).toLocaleString('pt-BR', { style: 'currency', currency: 'BRL' })
}

/** 1050 → "1.050" */
export function formatCoins(coins: number): string {
  return coins.toLocaleString('pt-BR')
}

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return ''
  return new Date(iso).toLocaleDateString('pt-BR', { day: '2-digit', month: 'short', year: 'numeric' })
}

import { reactive } from 'vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import { errorMessage, shop, type Order, type ShopCatalog, type Wallet } from './api'

/** Carteira e catálogo compartilhados (barra do topo, loja e perfil). */
export const wallet = reactive({
  data: null as Wallet | null,
  catalog: null as ShopCatalog | null,
  loading: false,
  error: null as string | null,
  /** Pedido pago com dinheiro esperando a confirmação do Mercado Pago. */
  waiting: null as { id: string; label: string; status: Order['status'] } | null,
})

export async function loadWallet() {
  wallet.loading = true
  wallet.error = null
  try {
    const [data, catalog] = await Promise.all([shop.wallet(), wallet.catalog ? Promise.resolve(wallet.catalog) : shop.catalog()])
    wallet.data = data
    wallet.catalog = catalog
  } catch (error) {
    wallet.error = errorMessage(error)
  } finally {
    wallet.loading = false
  }
}

export function clearWallet() {
  wallet.data = null
  stopWatching()
  wallet.waiting = null
}

export const isPremium = () => !!wallet.data?.premium_until && new Date(wallet.data.premium_until) > new Date()

/** Chave única por clique: repetir o mesmo pedido não cobra duas vezes no servidor. */
export const newKey = () => crypto.randomUUID()

let timer: number | undefined
let deadline = 0

function stopWatching() {
  window.clearTimeout(timer)
  timer = undefined
}

/** Consulta o pedido até o Mercado Pago confirmar (Pix leva segundos; boleto, dias). */
async function poll() {
  const waiting = wallet.waiting
  if (!waiting) return
  try {
    const order = await shop.order(waiting.id)
    waiting.status = order.status
    if (order.status !== 'pending') {
      stopWatching()
      await loadWallet()
      return
    }
  } catch {
    // rede instável: tenta de novo no próximo ciclo
  }
  if (Date.now() < deadline) timer = window.setTimeout(poll, 5000)
}

/** Abre o pagamento no navegador e acompanha a confirmação. */
export async function payWithMoney(start: () => Promise<{ order_id: string; checkout_url: string }>, label: string) {
  const checkout = await start()
  // Só abre links do Mercado Pago (defesa extra contra uma resposta adulterada).
  const url = new URL(checkout.checkout_url)
  if (url.protocol !== 'https:' || !/(^|\.)mercadopago\.com(\.br)?$|(^|\.)mercadolivre\.com(\.br)?$/.test(url.hostname)) {
    throw new Error('Link de pagamento inesperado. Fale com a equipe Conexão.')
  }
  await openUrl(checkout.checkout_url)
  wallet.waiting = { id: checkout.order_id, label, status: 'pending' }
  deadline = Date.now() + 15 * 60_000
  stopWatching()
  timer = window.setTimeout(poll, 5000)
}

/** "Já paguei": consulta na hora. */
export function checkNow() {
  stopWatching()
  deadline = Math.max(deadline, Date.now() + 5 * 60_000)
  return poll()
}

export function dismissWaiting() {
  stopWatching()
  wallet.waiting = null
}

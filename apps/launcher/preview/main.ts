/**
 * Entrada da prévia: prepara as preferências locais (sem boas-vindas, com favoritos e
 * histórico), sobe o launcher de verdade e abre a cena pedida em `?cena=`.
 */
const hours = (h: number) => Date.now() - h * 3_600_000
try {
  localStorage.setItem(
    'conexao.prefs.v1',
    JSON.stringify({
      onboarded: true,
      favorites: ['reino-valdoria', 'vila-serena', 'nova-aurora', 'porto-neblina'],
      recent: [
        { slug: 'reino-valdoria', at: hours(2) },
        { slug: 'nova-aurora', at: hours(26) },
        { slug: 'vila-serena', at: hours(50) },
      ],
    }),
  )
} catch {
  // sem armazenamento: segue sem favoritos
}

const { createApp } = await import('vue')
const { default: App } = await import('../src/App.vue')
await import('../src/fonts')
await import('../src/style.css')
const { go, openServer, openProfile, store } = await import('../src/lib/store')

createApp(App).mount('#app')

const cena = new URLSearchParams(location.search).get('cena') ?? 'inicio'
const [kind, arg] = cena.split(':')
const ready = () => store.servers.length > 0
const start = Date.now()
const timer = window.setInterval(() => {
  if (!ready() && Date.now() - start < 8000) return
  window.clearInterval(timer)
  if (kind === 'servidores') go('browse')
  else if (kind === 'central') go('browse', { central: true })
  else if (kind === 'servidor' && arg) openServer(arg)
  else if (kind === 'perfil' && arg) openProfile(arg)
  else if (kind === 'perfil') go('profile')
  else if (kind === 'loja') go('store')
  document.documentElement.dataset.previewReady = '1'
}, 50)

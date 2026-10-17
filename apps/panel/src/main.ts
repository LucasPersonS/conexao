import { createApp } from 'vue'
import { createRouter, createWebHashHistory } from 'vue-router'
import App from './App.vue'
import { loadSession, session } from './lib/api'
import './style.css'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/login', component: () => import('./views/LoginView.vue'), meta: { public: true } },
    { path: '/', component: () => import('./views/HomeView.vue') },
    { path: '/novo', component: () => import('./views/NewServerView.vue') },
    { path: '/s/:slug', component: () => import('./views/ServerView.vue'), props: true },
    { path: '/equipe-conexao', component: () => import('./views/StaffView.vue') },
    { path: '/equipe-conexao/armazenamento', component: () => import('./views/StorageView.vue') },
    { path: '/equipe-conexao/lista-de-espera', component: () => import('./views/WaitlistView.vue') },
    { path: '/equipe-conexao', redirect: '/equipe-conexao' },
    { path: '/link/:code?', component: () => import('./views/LinkView.vue'), props: true },
    { path: '/vincular', redirect: '/link' },
  ],
})

router.beforeEach(async (to) => {
  if (to.meta.public) return true
  const login = { path: '/login', query: to.fullPath !== '/' ? { next: to.fullPath } : {} }
  if (!session.token) return login
  if (!session.user) {
    try {
      await loadSession()
    } catch {
      return login
    }
  }
  return true
})

createApp(App).use(router).mount('#app')

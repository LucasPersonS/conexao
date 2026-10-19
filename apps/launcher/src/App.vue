<script setup lang="ts">
import { initActivity, onWake, resting } from './lib/activity'
import { computed, onMounted, ref, watch } from 'vue'
import LoginGate from './components/LoginGate.vue'
import Onboarding from './components/Onboarding.vue'
import PanelPairing from './components/PanelPairing.vue'
import TitleBar from './components/TitleBar.vue'
import TopNav from './components/TopNav.vue'
import UpdateToast from './components/UpdateToast.vue'
import logo from './assets/logo.png'
import { api, errorMessage, type Account, type MyRoles } from './lib/api'
import { prefs } from './lib/prefs'
import { clearStars, loadStars } from './lib/stars'
import { clearWallet, loadWallet } from './lib/wallet'
import { listen } from '@tauri-apps/api/event'
import { findServer, go, loadServers, openInvite, store } from './lib/store'
import BrowseView from './views/BrowseView.vue'
import HomeView from './views/HomeView.vue'
import ServerView from './views/ServerView.vue'
import ProfileView from './views/ProfileView.vue'
import SettingsView from './views/SettingsView.vue'
import StoreView from './views/StoreView.vue'
import ProfileModal from './components/ProfileModal.vue'

const ready = ref(false)
const account = ref<Account | null>(null)
const roles = ref<MyRoles | null>(null)
const loginBusy = ref(false)
const loginError = ref<string | null>(null)
/** Cadastro pelo Discord em andamento (falta o nick). */
const signup = ref<{ token: string; discord_name: string; suggested_nick: string | null } | null>(null)
const pairing = ref(false)
const content = ref<HTMLElement | null>(null)

const current = computed(() => findServer(store.slug))

// Conta logada disponível para os componentes (ex.: avatar no campo de comentário).
watch(account, (a) => (store.me = a ? { uuid: a.uuid, name: a.name } : null), { immediate: true })

// Cada tela começa do topo.
watch(
  () => [store.view, store.slug],
  () => content.value?.scrollTo({ top: 0 }),
)

async function afterLogin() {
  try {
    await api.session()
  } catch (error) {
    store.error = errorMessage(error)
  }
  await loadServers()
  loadStars()
  loadWallet()
  // Aberto por um convite (conexao://servidor/<slug>)?
  api.takeInvite().then((slug) => slug && openInvite(slug)).catch(() => {})
  api.myRoles().then((r) => (roles.value = r)).catch(() => (roles.value = null))
}

async function login() {
  loginBusy.value = true
  loginError.value = null
  try {
    account.value = await api.login()
    if (account.value) await afterLogin()
  } catch (error) {
    loginError.value = errorMessage(error)
  } finally {
    loginBusy.value = false
  }
}

async function loginDiscord() {
  loginBusy.value = true
  loginError.value = null
  try {
    const result = await api.loginDiscord()
    if (result.status === 'ok') {
      account.value = result.account
      await afterLogin()
    } else if (result.status === 'needs_nick') {
      signup.value = {
        token: result.signup_token,
        discord_name: result.discord_name,
        suggested_nick: result.suggested_nick,
      }
    }
  } catch (error) {
    loginError.value = errorMessage(error)
  } finally {
    loginBusy.value = false
  }
}

async function registerOffline(nick: string) {
  if (!signup.value) return
  loginBusy.value = true
  loginError.value = null
  try {
    account.value = await api.registerOffline(signup.value.token, nick)
    signup.value = null
    await afterLogin()
  } catch (error) {
    loginError.value = errorMessage(error)
  } finally {
    loginBusy.value = false
  }
}

async function logout() {
  await api.logout()
  account.value = null
  roles.value = null
  clearStars()
  clearWallet()
  go('home')
}

async function copyToken() {
  try {
    await navigator.clipboard.writeText(await api.publishToken())
  } catch (error) {
    store.error = errorMessage(error)
  }
}

onMounted(async () => {
  initActivity()
  listen<string>('invite', (event) => {
    if (account.value && store.servers.length) {
      api.takeInvite().catch(() => {})
      openInvite(event.payload)
    }
  }).catch(() => {})
  try {
    await api.initialize()
    account.value = await api.currentAccount()
    if (account.value) await afterLogin()
  } catch (error) {
    loginError.value = errorMessage(error)
  } finally {
    ready.value = true
  }
  // Lista atualizada a cada minuto, só com o launcher em uso (em repouso não gasta nada).
  let lastLoad = Date.now()
  const refreshList = () => {
    if (!account.value || resting.value) return
    lastLoad = Date.now()
    loadServers()
  }
  window.setInterval(refreshList, 60_000)
  onWake(() => Date.now() - lastLoad > 60_000 && refreshList())
})
</script>

<template>
  <div class="shell">
    <TitleBar />
    <div v-if="!ready" class="splash"><img :src="logo" alt="" />Carregando…</div>
    <LoginGate
      v-else-if="!account"
      :busy="loginBusy"
      :error="loginError"
      :signup="signup"
      @login="login"
      @discord="loginDiscord"
      @register="registerOffline"
      @cancel-signup="(signup = null), (loginError = null)"
    />
    <template v-else>
      <TopNav :account="account" :roles="roles" @pair="pairing = true" @token="copyToken" @logout="logout" />
      <main ref="content" class="content grid-bg">
        <div class="page">
          <HomeView v-if="store.view === 'home'" />
          <BrowseView v-else-if="store.view === 'browse'" />
          <ServerView v-else-if="store.view === 'server' && current" :server="current" />
          <SettingsView v-else-if="store.view === 'settings'" />
          <ProfileView v-else-if="store.view === 'profile'" />
          <StoreView v-else-if="store.view === 'store'" />
          <div v-else class="missing">
            Este servidor não está mais na lista.
            <button class="btn" @click="go('browse')">Ver servidores</button>
          </div>
        </div>
      </main>
      <PanelPairing v-if="pairing" @close="pairing = false" />
      <Onboarding v-if="!prefs.onboarded" />
      <ProfileModal v-if="store.viewingProfile" :name="store.viewingProfile" @close="store.viewingProfile = null" />
      <Transition name="toast">
        <div v-if="store.toast" class="toast" role="status">{{ store.toast }}</div>
      </Transition>
    </template>
    <UpdateToast />
  </div>
</template>

<style scoped>
.shell {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.splash {
  flex: 1;
  display: grid;
  place-items: center;
  align-content: center;
  gap: 18px;
  color: var(--text-dim);
}

.splash img {
  width: 76px;
  height: 76px;
  filter: drop-shadow(0 0 18px rgba(183, 254, 0, 0.45));
  animation: pulse 1.6s ease-in-out infinite;
}

@keyframes pulse {
  50% {
    transform: scale(0.94);
    filter: drop-shadow(0 0 6px rgba(183, 254, 0, 0.25));
  }
}

.content {
  flex: 1;
  overflow-y: auto;
  background-color: var(--bg);
  background-attachment: local;
}

.page {
  max-width: 1240px;
  margin: 0 auto;
  padding: 24px 28px 40px;
}

.toast {
  position: fixed;
  left: 50%;
  bottom: 24px;
  transform: translateX(-50%);
  z-index: 90;
  max-width: 520px;
  padding: 12px 18px;
  border-radius: 12px;
  background: var(--bg-raised);
  border: 1px solid var(--lime-line);
  color: var(--text);
  font-size: 14px;
  box-shadow: var(--shadow-pop), 0 0 24px rgba(183, 254, 0, 0.12);
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.2s, transform 0.2s;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translate(-50%, 8px);
}

.missing {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 60px;
  color: var(--text-dim);
}
</style>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import type { Account, MyRoles } from '../lib/api'
import { formatNumber } from '../lib/format'
import { formatCoins } from '../lib/format'
import { go, store, totalPlayers } from '../lib/store'
import { isPremium, wallet } from '../lib/wallet'
import Icon from './Icon.vue'

const props = defineProps<{ account: Account; roles: MyRoles | null }>()
const emit = defineEmits<{ pair: []; token: []; logout: [] }>()

const open = ref(false)
const root = ref<HTMLElement | null>(null)
const tokenCopied = ref(false)

const isAdmin = () => !!props.roles && (props.roles.is_staff || props.roles.servers.length > 0)

function onDocClick(event: MouseEvent) {
  if (root.value && !root.value.contains(event.target as Node)) open.value = false
}
onMounted(() => document.addEventListener('mousedown', onDocClick))
onBeforeUnmount(() => document.removeEventListener('mousedown', onDocClick))

function act(fn: () => void) {
  open.value = false
  fn()
}

function copyToken() {
  emit('token')
  tokenCopied.value = true
  window.setTimeout(() => (tokenCopied.value = false), 2000)
}

const ROLE_NAMES: Record<string, string> = { owner: 'Dono', admin: 'Admin', moderator: 'Moderador' }
</script>

<template>
  <nav class="topnav">
    <div class="tabs">
      <button class="tab" :class="{ on: store.view === 'home' }" @click="go('home')">Início</button>
      <button class="tab" :class="{ on: store.view === 'browse' || store.view === 'server' }" @click="go('browse')">
        Servidores
      </button>
      <button class="tab" :class="{ on: store.view === 'store' }" @click="go('store')">Loja</button>
    </div>

    <div class="right">
      <span class="online" title="Jogadores em todos os servidores parceiros">
        <span class="dot on" />
        {{ formatNumber(totalPlayers) }} jogando · {{ store.servers.length }} servidores
      </span>
      <button class="coins" :class="{ plus: isPremium() }" title="Suas Moedas Conexão — abrir a loja" @click="go('store')">
        <Icon name="coin" :size="16" />
        {{ formatCoins(wallet.data?.balance ?? 0) }}
        <span v-if="isPremium()" class="plus-tag">+</span>
      </button>
      <button class="icon-btn" :class="{ on: store.view === 'settings' }" title="Configurações" @click="go('settings')">
        <Icon name="settings" :size="19" />
      </button>

      <div ref="root" class="account">
        <button class="me" :aria-expanded="open" @click="open = !open">
          <img :src="`https://mc-heads.net/avatar/${account.uuid}/32`" alt="" />
          <span>{{ account.name }}</span>
          <Icon name="chevron" :size="14" />
        </button>
        <div v-if="open" class="menu">
          <div class="who">
            <img :src="`https://mc-heads.net/avatar/${account.uuid}/48`" alt="" />
            <div>
              <strong>{{ account.name }}</strong>
              <small>{{ account.kind === 'offline' ? 'Conta não original (Discord)' : 'Conta Microsoft' }}</small>
            </div>
          </div>
          <button class="item highlight" @click="act(() => go('profile'))"><Icon name="star" :size="16" /> Meu perfil</button>
          <button class="item" @click="act(() => go('store'))"><Icon name="bag" :size="16" /> Loja e Moedas</button>
          <button class="item" @click="act(() => go('settings'))"><Icon name="settings" :size="16" /> Configurações</button>

          <template v-if="isAdmin()">
            <div class="group">
              Administração
              <span v-if="roles?.is_staff" class="pill">Equipe Conexão</span>
            </div>
            <div v-for="s in roles?.servers ?? []" :key="s.slug" class="managed">
              {{ s.name }} <small>{{ ROLE_NAMES[s.role] ?? s.role }}</small>
            </div>
            <button class="item" @click="act(() => emit('pair'))"><Icon name="link" :size="16" /> Conectar painel web</button>
            <button class="item" title="Para o conexao-publish: vale 2 horas e só serve para publicar modpack" @click="copyToken">
              <Icon name="key" :size="16" /> {{ tokenCopied ? 'Token copiado!' : 'Copiar token de publicação' }}
            </button>
          </template>

          <button class="item danger" @click="act(() => emit('logout'))"><Icon name="logout" :size="16" /> Sair</button>
        </div>
      </div>
    </div>
  </nav>
</template>

<style scoped>
.topnav {
  height: 58px;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 20px;
  background: rgba(10, 15, 17, 0.92);
  border-bottom: 1px solid var(--line);
  backdrop-filter: blur(10px);
  position: relative;
  z-index: 20;
}

.tabs {
  display: flex;
  gap: 4px;
  height: 100%;
}

.tab {
  position: relative;
  padding: 0 14px;
  font-family: var(--font-display);
  font-size: 16px;
  font-weight: 600;
  letter-spacing: 0.01em;
  color: var(--text-dim);
  transition: color 0.15s;
}

.tab:hover {
  color: var(--text);
}

.tab.on {
  color: var(--text);
}

.tab.on::after {
  content: '';
  position: absolute;
  left: 12px;
  right: 12px;
  bottom: -1px;
  height: 3px;
  border-radius: 3px 3px 0 0;
  background: var(--lime);
  box-shadow: 0 0 12px rgba(183, 254, 0, 0.7);
}

.right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.online {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  margin-right: 6px;
  padding: 0 12px;
  border-radius: 999px;
  background: var(--lime-soft);
  border: 1px solid rgba(183, 254, 0, 0.18);
  font-size: 13px;
  color: var(--text-dim);
}

.coins {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 0 12px;
  border-radius: 999px;
  border: 1px solid var(--lime-line);
  background: var(--bg-card);
  color: var(--lime);
  font-weight: 700;
  font-size: 13px;
}

.coins:hover {
  background: var(--bg-hover);
}

.plus-tag {
  display: grid;
  place-items: center;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--lime);
  color: var(--ink);
  font-size: 12px;
}

.icon-btn {
  width: 38px;
  height: 38px;
  display: grid;
  place-items: center;
  border-radius: 10px;
  color: var(--text-dim);
}

.icon-btn:hover,
.icon-btn.on {
  background: var(--bg-hover);
  color: var(--text);
}

.account {
  position: relative;
}

.me {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 40px;
  padding: 0 10px 0 4px;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: var(--bg-card);
  font-weight: 600;
  font-size: 14px;
  transition: border-color 0.15s, background 0.15s;
}

.me:hover,
.me[aria-expanded='true'] {
  background: var(--bg-hover);
  border-color: var(--lime-line);
}

.me img {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  image-rendering: pixelated;
}

.menu {
  position: absolute;
  right: 0;
  top: calc(100% + 8px);
  z-index: 40;
  width: 270px;
  padding: 8px;
  border-radius: 14px;
  background: var(--bg-raised);
  border: 1px solid var(--line);
  box-shadow: var(--shadow-pop);
}

.who {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 8px 12px;
  border-bottom: 1px solid var(--line);
  margin-bottom: 6px;
}

.who img {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  image-rendering: pixelated;
}

.who div {
  display: flex;
  flex-direction: column;
}

.who small {
  color: var(--text-faint);
}

.item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 10px;
  border-radius: 9px;
  font-size: 14px;
  text-align: left;
}

.item:hover {
  background: var(--bg-hover);
}

.danger {
  color: var(--bad);
}

.highlight {
  font-weight: 600;
  color: var(--lime);
}

.group {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin: 10px 10px 4px;
  padding-top: 10px;
  border-top: 1px solid var(--line);
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-faint);
}

.pill {
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--lime);
  color: var(--ink);
  text-transform: none;
  letter-spacing: 0;
}

.managed {
  display: flex;
  justify-content: space-between;
  padding: 4px 10px;
  font-size: 13px;
  color: var(--text-dim);
}

.managed small {
  color: var(--text-faint);
}

.item.danger {
  margin-top: 6px;
  border-top: 1px solid var(--line);
  border-radius: 0 0 9px 9px;
}

.item.danger:hover {
  background: var(--bad-bg);
}
</style>

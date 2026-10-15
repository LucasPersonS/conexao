<script setup lang="ts">
import { useRoute, useRouter } from 'vue-router'
import logo from './assets/logo.png'
import wordmark from './assets/wordmark.png'
import { session, setToken } from './lib/api'

const route = useRoute()
const router = useRouter()

function logout() {
  setToken(null)
  session.user = null
  session.memberships = []
  router.push('/login')
}

const ROLE: Record<string, string> = { owner: 'Dono', admin: 'Admin', moderator: 'Moderador' }
</script>

<template>
  <RouterView v-if="route.meta.public" />
  <div v-else class="layout">
    <aside class="sidebar">
      <RouterLink to="/" class="brand">
        <img :src="logo" alt="" class="mark" />
        <div>
          <img :src="wordmark" alt="Conexão" class="wordmark" />
          <span>Painel dos servidores</span>
        </div>
      </RouterLink>

      <nav>
        <span class="label">Meus servidores</span>
        <RouterLink v-for="m in session.memberships" :key="m.slug" :to="`/s/${m.slug}`" class="nav">
          <span class="name">{{ m.name }}</span>
          <span class="role">{{ ROLE[m.role] }}</span>
        </RouterLink>
        <p v-if="!session.memberships.length" class="empty">Nenhum servidor ainda.</p>
        <RouterLink to="/novo" class="nav add">+ Cadastrar servidor</RouterLink>

        <template v-if="session.user?.is_staff">
          <span class="label">Equipe Conexão</span>
          <RouterLink to="/equipe-conexao" class="nav">Aprovações e moderação</RouterLink>
          <RouterLink to="/equipe-conexao/armazenamento" class="nav">Armazenamento</RouterLink>
        </template>
      </nav>

      <div v-if="session.user" class="me">
        <img :src="`https://mc-heads.net/avatar/${session.user.minecraft_uuid}/32`" alt="" />
        <div>
          <strong>{{ session.user.minecraft_name }}</strong>
          <button class="link" @click="logout">Sair</button>
        </div>
      </div>
    </aside>
    <main class="content">
      <RouterView :key="route.fullPath.split('?')[0]" />
    </main>
  </div>
</template>

<style scoped>
.layout {
  display: flex;
  min-height: 100vh;
}

.sidebar {
  width: 260px;
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 20px;
  padding: 20px 14px;
  background:
    radial-gradient(300px 220px at 0 0, rgba(183, 254, 0, 0.1), transparent 70%),
    #070b0c;
  border-right: 1px solid var(--line);
  position: sticky;
  top: 0;
  height: 100vh;
}

.brand {
  display: flex;
  align-items: center;
  gap: 12px;
  text-decoration: none;
  color: var(--text);
  padding: 0 6px;
}

.brand .mark {
  width: 46px;
  height: 46px;
  filter: drop-shadow(0 0 10px rgba(183, 254, 0, 0.45));
}

.brand .wordmark {
  display: block;
  height: 26px;
  width: auto;
  margin-bottom: 3px;
}

.brand span {
  font-size: 12px;
  color: var(--text-dim);
}

nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  overflow-y: auto;
}

.label {
  margin: 12px 8px 4px;
  font-family: var(--font-pixel);
  font-size: 10px;
  font-weight: 400;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--lime);
}

.nav {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 9px 10px;
  border-radius: 10px;
  text-decoration: none;
  color: var(--text);
  font-weight: 600;
  font-size: 14px;
}

.nav:hover {
  background: var(--hover);
}

.nav.router-link-exact-active {
  background: var(--card);
  box-shadow: inset 3px 0 0 var(--lime), 0 0 0 1px var(--line);
}

.role {
  font-size: 11px;
  color: var(--text-faint);
  font-weight: 600;
}

.add {
  color: var(--lime);
}

.empty {
  margin: 4px 10px;
  font-size: 13px;
  color: var(--text-faint);
}

.me {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px;
  border-radius: 12px;
  background: var(--card);
  border: 1px solid var(--line);
}

.me img {
  width: 32px;
  height: 32px;
  border-radius: 6px;
}

.me strong {
  display: block;
  font-size: 13px;
}

.link {
  border: none;
  background: none;
  padding: 0;
  font-size: 12px;
  color: var(--text-faint);
  text-decoration: underline;
}

.content {
  flex: 1;
  min-width: 0;
  padding: 28px 36px 48px;
  max-width: 1100px;
}
</style>

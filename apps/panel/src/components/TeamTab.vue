<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { api, message, type Member, type Server } from '../lib/api'

const props = defineProps<{ server: Server; role: string }>()
const ROLE: Record<string, string> = { owner: 'Dono', admin: 'Admin', moderator: 'Moderador' }

const members = ref<Member[]>([])
const error = ref<string | null>(null)
const busy = ref(false)
const nick = ref('')
const newRole = ref<'admin' | 'moderator'>('moderator')

async function load() {
  try {
    members.value = await api.members(props.server.slug)
  } catch (e) {
    error.value = message(e)
  }
}
onMounted(load)

async function set(name: string, role: string) {
  busy.value = true
  error.value = null
  try {
    members.value = await api.setMember(props.server.slug, name, role)
    nick.value = ''
  } catch (e) {
    error.value = message(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <section class="card">
    <h2>Equipe do servidor</h2>
    <p class="muted">
      <b>Admin</b> publica modpack, edita o servidor e gerencia a whitelist. <b>Moderador</b> só cuida da whitelist.
    </p>
    <form v-if="role === 'owner'" class="add" @submit.prevent="set(nick.trim(), newRole)">
      <input v-model="nick" class="input" placeholder="Nick do Minecraft" maxlength="16" />
      <select v-model="newRole" class="input">
        <option value="moderator">Moderador</option>
        <option value="admin">Admin</option>
      </select>
      <button class="btn btn-primary" :disabled="busy || !nick.trim()">Adicionar</button>
    </form>
    <p v-if="error" class="error">{{ error }}</p>
    <table>
      <thead>
        <tr>
          <th>Membro</th>
          <th>Papel</th>
          <th />
        </tr>
      </thead>
      <tbody>
        <tr v-for="m in members" :key="m.user_id">
          <td class="player">
            <img :src="`https://mc-heads.net/avatar/${m.minecraft_uuid}/24`" alt="" />
            <b>{{ m.minecraft_name }}</b>
          </td>
          <td>{{ ROLE[m.role] }}</td>
          <td class="actions">
            <template v-if="role === 'owner' && m.role !== 'owner'">
              <button class="btn btn-sm" :disabled="busy" @click="set(m.minecraft_name, m.role === 'admin' ? 'moderator' : 'admin')">
                Tornar {{ m.role === 'admin' ? 'moderador' : 'admin' }}
              </button>
              <button class="btn btn-sm btn-danger" :disabled="busy" @click="set(m.minecraft_name, 'none')">Remover</button>
            </template>
          </td>
        </tr>
      </tbody>
    </table>
  </section>
</template>

<style scoped>
.muted {
  line-height: 1.5;
}

.add {
  display: flex;
  gap: 8px;
  margin: 14px 0;
  max-width: 560px;
}

.add .input:first-child {
  flex: 1;
}

.player {
  display: flex;
  align-items: center;
  gap: 8px;
}

.player img {
  border-radius: 4px;
}

.actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
}
</style>

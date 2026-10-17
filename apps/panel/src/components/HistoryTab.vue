<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { api, formatDate, message, type AuditEntry, type Server } from '../lib/api'

const props = defineProps<{ server: Server }>()
const entries = ref<AuditEntry[]>([])
const error = ref<string | null>(null)

onMounted(async () => {
  try {
    entries.value = await api.audit(props.server.slug)
  } catch (e) {
    error.value = message(e)
  }
})

const WHITELIST: Record<string, string> = { approved: 'liberou', rejected: 'recusou', banned: 'baniu', pending: 'pôs em análise' }

function describe(e: AuditEntry): string {
  const d = e.details as Record<string, string | number | null>
  switch (e.action) {
    case 'server.created':
      return 'cadastrou o servidor'
    case 'server.updated':
      return 'editou os dados do servidor'
    case 'server.approval':
      return `mudou a aprovação para ${d.approval}`
    case 'server.featured':
      return d.until ? `ativou o destaque até ${formatDate(String(d.until))}` : 'removeu o destaque'
    case 'version.published':
      return `publicou a versão ${d.version}${d.channel ? ` (${d.channel})` : ''} com ${d.files} arquivos`
    case 'channel.set':
      return `apontou o canal ${d.channel} para a versão ${d.version}`
    case 'whitelist.set':
      return `${WHITELIST[String(d.status)] ?? d.status} ${d.player}`
    case 'member.set':
      return d.role === 'none' ? `removeu ${d.member} da equipe` : `definiu ${d.member} como ${d.role}`
    case 'comment.deleted':
      return `apagou um comentário de ${d.author}: "${d.excerpt}${String(d.excerpt ?? '').length >= 120 ? '…' : ''}"`
    case 'agent.token_rotated':
      return 'gerou um novo token do agente'
    case 'agent.command_result':
      return `agente executou comando (${d.ok ? 'ok' : 'falhou'}): ${d.output ?? ''}`
    default:
      return e.action
  }
}
</script>

<template>
  <section class="card">
    <h2>Histórico</h2>
    <p v-if="error" class="error">{{ error }}</p>
    <p v-if="!entries.length" class="muted">Nada registrado ainda.</p>
    <table v-else>
      <tbody>
        <tr v-for="(e, i) in entries" :key="i">
          <td class="when muted">{{ formatDate(e.created_at) }}</td>
          <td><b>{{ e.actor_name ?? 'Sistema' }}</b> {{ describe(e) }}</td>
        </tr>
      </tbody>
    </table>
  </section>
</template>

<style scoped>
.when {
  width: 140px;
  white-space: nowrap;
}

tr:first-child td {
  border-top: none;
}
</style>

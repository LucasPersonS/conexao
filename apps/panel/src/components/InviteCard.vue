<script setup lang="ts">
/** Link de convite para o parceiro divulgar no Discord. */
import { computed, ref } from 'vue'
import { API_URL, type Server } from '../lib/api'

const props = defineProps<{ server: Server }>()
const copied = ref<string | null>(null)

const link = computed(() => `${API_URL}/convite/${props.server.slug}`)
const message = computed(
  () =>
    `🎮 Jogue no ${props.server.name} pelo Conexão!\nO launcher instala o modpack sozinho e entra no servidor com um clique:\n${link.value}`,
)

async function copy(text: string, which: string) {
  try {
    await navigator.clipboard.writeText(text)
    copied.value = which
    window.setTimeout(() => (copied.value = null), 2000)
  } catch {
    window.prompt('Copie o texto:', text)
  }
}
</script>

<template>
  <section class="card invite">
    <div>
      <h2>Convide seus jogadores</h2>
      <p class="muted">
        Cole este link no Discord: aparece um card com o banner do servidor. Quem já tem o Conexão abre direto na página
        do servidor; quem não tem baixa o launcher.
      </p>
      <p v-if="server.approval !== 'approved'" class="warn-text">
        O link funciona depois que a equipe Conexão aprovar o servidor.
      </p>
    </div>
    <div class="row">
      <code class="link-box">{{ link }}</code>
      <button class="btn btn-primary btn-sm" @click="copy(link, 'link')">{{ copied === 'link' ? 'Copiado!' : 'Copiar link' }}</button>
      <button class="btn btn-sm" @click="copy(message, 'msg')">{{ copied === 'msg' ? 'Copiada!' : 'Copiar mensagem pronta' }}</button>
      <a class="btn btn-sm" :href="link" target="_blank" rel="noopener">Ver página</a>
    </div>
  </section>
</template>

<style scoped>
.invite {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 16px;
}

h2 {
  margin: 0 0 4px;
}

p {
  margin: 0;
  line-height: 1.5;
}

.warn-text {
  color: var(--warn);
  font-size: 13px;
  margin-top: 6px;
}

.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.link-box {
  flex: 1;
  min-width: 260px;
  padding: 8px 12px;
  border-radius: 10px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  user-select: all;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.btn {
  text-decoration: none;
}
</style>

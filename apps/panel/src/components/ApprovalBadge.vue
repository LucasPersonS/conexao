<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ approval: string }>()
const view = computed(
  () =>
    ({
      approved: { text: 'Aprovado', tone: 'good' },
      pending: { text: 'Em análise', tone: 'warn' },
      rejected: { text: 'Recusado', tone: 'bad' },
      suspended: { text: 'Suspenso', tone: 'bad' },
    })[props.approval] ?? { text: props.approval, tone: '' },
)
</script>

<template>
  <span class="badge" :class="view.tone">{{ view.text }}</span>
</template>

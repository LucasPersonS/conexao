<script setup lang="ts" generic="T extends string | null">
/** Seletor suspenso (dropdown) com busca opcional, no visual do Conexão. */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import Icon from './Icon.vue'

const props = defineProps<{
  modelValue: T
  options: { value: T; label: string; count?: number }[]
  icon?: string
  searchable?: boolean
}>()
const emit = defineEmits<{ 'update:modelValue': [value: T] }>()

const open = ref(false)
const query = ref('')
const root = ref<HTMLElement | null>(null)

const selected = computed(() => props.options.find((o) => o.value === props.modelValue) ?? props.options[0])
const visible = computed(() => {
  const q = query.value.trim().toLowerCase()
  return q ? props.options.filter((o) => o.label.toLowerCase().includes(q)) : props.options
})

function pick(value: T) {
  emit('update:modelValue', value)
  open.value = false
  query.value = ''
}

function onDocClick(event: MouseEvent) {
  if (root.value && !root.value.contains(event.target as Node)) open.value = false
}
onMounted(() => document.addEventListener('mousedown', onDocClick))
onBeforeUnmount(() => document.removeEventListener('mousedown', onDocClick))
</script>

<template>
  <div ref="root" class="select" :class="{ open }">
    <button class="trigger" :aria-expanded="open" @click="open = !open" @keydown.esc="open = false">
      <Icon v-if="icon" :name="icon" :size="16" class="lead" />
      <span class="value">{{ selected?.label }}</span>
      <Icon name="chevron" :size="16" class="chev" />
    </button>
    <div v-if="open" class="menu" role="listbox">
      <input v-if="searchable" v-model="query" class="filter" placeholder="Filtrar…" autofocus @keydown.esc="open = false" />
      <button
        v-for="option in visible"
        :key="String(option.value)"
        class="option"
        :class="{ on: option.value === modelValue }"
        role="option"
        :aria-selected="option.value === modelValue"
        @click="pick(option.value)"
      >
        <span>{{ option.label }}</span>
        <small v-if="option.count !== undefined">{{ option.count }}</small>
        <Icon v-if="option.value === modelValue" name="check" :size="14" class="tick" />
      </button>
      <p v-if="!visible.length" class="none">Nada encontrado</p>
    </div>
  </div>
</template>

<style scoped>
.select {
  position: relative;
}

.trigger {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 42px;
  min-width: 190px;
  padding: 0 12px;
  border-radius: 12px;
  background: var(--bg-card);
  border: 1px solid var(--line);
  font-size: 14px;
  font-weight: 600;
  text-align: left;
}

.trigger:hover,
.open .trigger {
  border-color: var(--lime-line);
}

.lead {
  color: var(--brand-strong);
}

.value {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.chev {
  color: var(--text-faint);
  transition: transform 0.15s;
}

.open .chev {
  transform: rotate(180deg);
}

.menu {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 30;
  min-width: 100%;
  max-height: 320px;
  overflow-y: auto;
  padding: 6px;
  border-radius: 12px;
  background: var(--bg-raised);
  border: 1px solid var(--line);
  box-shadow: var(--shadow-pop);
}

.filter {
  width: 100%;
  height: 34px;
  margin-bottom: 4px;
  padding: 0 10px;
  border-radius: 8px;
  border: 1px solid var(--line);
  background: var(--bg-input);
  outline: none;
}

.option {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  font-size: 14px;
  text-align: left;
  white-space: nowrap;
}

.option:hover {
  background: var(--bg-hover);
}

.option.on {
  font-weight: 700;
  color: var(--brand-strong);
}

.option span {
  flex: 1;
}

.option small {
  color: var(--text-faint);
}

.tick {
  color: var(--brand-strong);
}

.none {
  margin: 8px 10px;
  font-size: 13px;
  color: var(--text-faint);
}
</style>

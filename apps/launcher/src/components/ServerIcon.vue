<script setup lang="ts">
defineProps<{ name: string; url: string | null; size?: number }>()
</script>

<template>
  <img v-if="url" :src="url" class="server-icon" :style="{ width: `${size ?? 48}px`, height: `${size ?? 48}px` }" alt="" />
  <div
    v-else
    class="server-icon placeholder"
    :style="{ width: `${size ?? 48}px`, height: `${size ?? 48}px`, fontSize: `${(size ?? 48) * 0.42}px` }"
  >
    {{ name.slice(0, 1).toUpperCase() }}
  </div>
</template>

<style scoped>
.server-icon {
  flex: none;
  border-radius: 22%;
  object-fit: cover;
  background: var(--bg-raised);
}

.placeholder {
  display: grid;
  place-items: center;
  font-family: var(--font-display);
  font-weight: 700;
  color: var(--lime);
  background: linear-gradient(135deg, #1c2a1a, #0e1518);
}
</style>

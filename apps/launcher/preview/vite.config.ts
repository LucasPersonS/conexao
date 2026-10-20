// Prévia do launcher no navegador: troca as APIs do Tauri por versões falsas (preview/tauri/*)
// que respondem com os dados de preview/data.ts. Uso: `npm run preview:mock`.
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url))

export default defineConfig({
  root: here('.'),
  plugins: [vue()],
  server: { port: 1430, fs: { allow: [here('..'), here('../../../packages')] } },
  resolve: {
    alias: [
      { find: '@profile', replacement: here('../../../packages/profile-ui/src') },
      { find: '@tauri-apps/api/core', replacement: here('./tauri/core.ts') },
      { find: '@tauri-apps/api/event', replacement: here('./tauri/event.ts') },
      { find: '@tauri-apps/api/window', replacement: here('./tauri/window.ts') },
      { find: '@tauri-apps/api/app', replacement: here('./tauri/app.ts') },
      { find: '@tauri-apps/plugin-opener', replacement: here('./tauri/opener.ts') },
    ],
    dedupe: ['vue'],
  },
})

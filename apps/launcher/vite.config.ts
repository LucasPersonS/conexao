import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Configuração recomendada pelo Tauri 2.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, fs: { allow: ['..', '../../packages'] } },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: { target: 'es2021', sourcemap: false },
  resolve: {
    // Card de perfil compartilhado com o painel e a página pública.
    alias: { '@profile': fileURLToPath(new URL('../../packages/profile-ui/src', import.meta.url)) },
    dedupe: ['vue'],
  },
})

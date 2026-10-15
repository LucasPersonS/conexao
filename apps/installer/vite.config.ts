import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Configuração recomendada pelo Tauri 2.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: { port: 1430, strictPort: true },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: { target: 'es2021', sourcemap: false },
})

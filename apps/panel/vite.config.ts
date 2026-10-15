import { fileURLToPath, URL } from 'node:url'
import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig(({ command, mode }) => {
  // Um painel de produção sem o endereço da API cairia em http://localhost:8080.
  if (command === 'build' && mode === 'production') {
    const url = loadEnv(mode, process.cwd(), 'VITE_').VITE_CONEXAO_API_URL ?? ''
    if (!url.startsWith('https://')) {
      throw new Error(
        'Defina VITE_CONEXAO_API_URL com o endereço https da API antes de gerar o painel ' +
          '(ex.: VITE_CONEXAO_API_URL=https://api.seudominio npm run build).',
      )
    }
  }
  return {
    plugins: [vue()],
    server: { port: 5174, fs: { allow: ['..', '../../packages'] } },
    resolve: {
      // Card de perfil compartilhado com o launcher e a página pública.
      alias: { '@profile': fileURLToPath(new URL('../../packages/profile-ui/src', import.meta.url)) },
      dedupe: ['vue'],
    },
  }
})

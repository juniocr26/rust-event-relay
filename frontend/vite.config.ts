import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', 'DEMO_')
  return {
    plugins: [vue(), tailwindcss()],
    server: { proxy: env.DEMO_API_TARGET ? { '/demo': { target: env.DEMO_API_TARGET } } : undefined },
    // Acceptance must not reuse a running development server's cache.
    cacheDir: mode === 'acceptance' ? 'node_modules/.vite-acceptance' : 'node_modules/.vite',
  }
})

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig(({ mode }) => ({
  plugins: [vue(), tailwindcss()],
  // Acceptance must not reuse or invalidate a running development server's cache.
  cacheDir: mode === 'acceptance' ? 'node_modules/.vite-acceptance' : 'node_modules/.vite',
}))

import { defineConfig } from '@playwright/test'
export default defineConfig({
  testDir: './tests',
  timeout: 30000,
  fullyParallel: true,
  use: { baseURL: 'http://127.0.0.1:5174', headless: true },
  webServer: {
    command: 'npm run dev -- --mode acceptance --host 127.0.0.1 --port 5174 --strictPort',
    url: 'http://127.0.0.1:5174',
    reuseExistingServer: false,
  },
})

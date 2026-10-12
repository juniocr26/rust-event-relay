import { defineConfig } from '@playwright/test'
export default defineConfig({
  testDir: './tests',
  timeout: 30000,
  fullyParallel: true,
  use: { baseURL: 'http://127.0.0.1:5173', headless: true },
  webServer: {
    command: 'npm run dev -- --host 0.0.0.0 --port 5173',
    url: 'http://127.0.0.1:5173',
    reuseExistingServer: true,
  },
})

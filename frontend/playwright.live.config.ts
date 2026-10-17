import { defineConfig } from '@playwright/test'
export default defineConfig({
  testDir: './tests', testMatch: ['live-delivery.spec.ts', 'experience.spec.ts', 'live-dependency.spec.ts', 'live-pending.spec.ts'],
  timeout: 30000, workers: 1, outputDir: '/tmp/relay-browser-results',
  reporter: 'list', use: { baseURL: 'http://localhost:5174', headless: true },
})

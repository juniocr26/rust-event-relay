import { defineConfig } from '@playwright/test'
export default defineConfig({
  testDir: './tests', testMatch: ['live-delivery.spec.ts', 'experience.spec.ts', 'live-dependency.spec.ts', 'live-pending.spec.ts', 'real-scenarios.spec.ts'],
  timeout: 30000, workers: 1, outputDir: '/tmp/relay-browser-results',
  reporter: 'list', use: { baseURL: 'http://localhost:5173', headless: true },
})

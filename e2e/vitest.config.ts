import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['spec/**/*.spec.ts'],
    environment: 'node',
    setupFiles: ['./lib/setup.ts'],
    testTimeout: 120000,
    hookTimeout: 120000,
    globals: true
  }
})

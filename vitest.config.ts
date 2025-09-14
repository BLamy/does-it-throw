import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    include: ['tests/**/*.{test,spec}.{js,ts}'],
    exclude: ['node_modules', 'dist', 'build', 'server/src/rust'],
    testTimeout: 10000,
    hookTimeout: 10000,
    setupFiles: ['tests/vitest-setup.ts'],
  },
})
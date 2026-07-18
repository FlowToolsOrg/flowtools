import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { playwright } from '@vitest/browser-playwright'
import { defineConfig } from 'vitest/config'

const uiSrcDir = fileURLToPath(
  new URL('../../packages/ui/src', import.meta.url)
)
const uiSrcIndex = fileURLToPath(
  new URL('../../packages/ui/src/index.ts', import.meta.url)
)

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: [
      { find: '@flowtools/ui', replacement: uiSrcIndex },
      { find: '@flowtools/ui/', replacement: `${uiSrcDir}/` },
    ],
  },
  test: {
    setupFiles: ['./src/test/browser-setup.ts'],
    browser: {
      enabled: true,
      provider: playwright(),
      // https://vitest.dev/config/browser/playwright
      instances: [{ browser: 'chromium' }],
    },
  },
})

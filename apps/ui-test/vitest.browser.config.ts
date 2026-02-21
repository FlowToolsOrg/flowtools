import { fileURLToPath, URL } from 'node:url'

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
  plugins: [
    react({
      babel: {
        plugins: [['babel-plugin-react-compiler']],
      },
    }),
  ],
  resolve: {
    alias: [
      { find: '@flow-tool/ui', replacement: uiSrcIndex },
      { find: '@flow-tool/ui/', replacement: `${uiSrcDir}/` },
    ],
  },
  test: {
    browser: {
      enabled: true,
      provider: playwright(),
      // https://vitest.dev/config/browser/playwright
      instances: [{ browser: 'chromium' }],
    },
  },
})

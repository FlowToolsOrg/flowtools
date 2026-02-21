import { playwright } from '@vitest/browser-playwright'
import { defineConfig, mergeConfig } from 'vitest/config'

import viteConfig from './vite.config'

export default defineConfig(() =>
  mergeConfig(
    viteConfig,
    defineConfig({
      test: {
        browser: {
          enabled: true,
          provider: playwright(),
          // https://vitest.dev/config/browser/playwright
          instances: [{ browser: 'chromium' }],
        },
        exclude: ['packages/template/*'],
        globals: true,
      },
    })
  )
)

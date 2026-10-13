import type { ViteUserConfig } from 'vitest/config'

import { createServer } from 'node:net'
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

async function browserApi() {
  const probe = createServer()
  try {
    await new Promise<void>((resolve, reject) => {
      probe.once('error', reject)
      probe.listen(0, '127.0.0.1', resolve)
    })
    const address = probe.address()
    if (!address || typeof address === 'string')
      throw new Error('Browser test loopback port unavailable')
    // Vitest replaces port 0 with 63315; use the OS-selected port explicitly.
    return { host: '127.0.0.1', port: address.port, strictPort: true }
  } finally {
    if (probe.listening)
      await new Promise<void>((resolve, reject) => {
        probe.close(error => (error ? reject(error) : resolve()))
      })
  }
}

export default defineConfig(
  async (): Promise<ViteUserConfig> => ({
    plugins: [react(), tailwindcss()],
    resolve: {
      alias: [
        { find: /^@flowtools\/ui$/, replacement: uiSrcIndex },
        { find: /^@flowtools\/ui\/(.*)$/, replacement: `${uiSrcDir}/$1` },
      ],
    },
    test: {
      // Bound Chromium page/transform concurrency on Windows; run every file.
      fileParallelism: false,
      setupFiles: ['./src/test/browser-setup.ts'],
      browser: {
        api: await browserApi(),
        enabled: true,
        headless: true,
        provider: playwright(),
        // https://vitest.dev/config/browser/playwright
        instances: [{ browser: 'chromium' }],
      },
    },
  })
)

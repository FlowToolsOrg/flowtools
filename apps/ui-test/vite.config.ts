import { fileURLToPath, URL } from 'node:url'

import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

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
  server: {
    port: 5174,
  },
})

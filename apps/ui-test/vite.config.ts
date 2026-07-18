import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const workspaceRoot = fileURLToPath(new URL('../..', import.meta.url))
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
      { find: /^@flowtools\/ui$/, replacement: uiSrcIndex },
      { find: /^@flowtools\/ui\/(.+)$/, replacement: `${uiSrcDir}/$1` },
    ],
    dedupe: ['react', 'react-dom'],
  },
  server: {
    port: 5174,
    fs: {
      allow: [workspaceRoot],
    },
  },
})

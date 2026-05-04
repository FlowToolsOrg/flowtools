import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const workspaceRoot = fileURLToPath(new URL('../..', import.meta.url))
const pluginsDir = fileURLToPath(new URL('../../plugins', import.meta.url))
const sdkEntry = fileURLToPath(
  new URL('../../packages/sdk/src/index.ts', import.meta.url)
)
const sdkDir = fileURLToPath(new URL('../../packages/sdk/src', import.meta.url))
const uiEntry = fileURLToPath(
  new URL('../../packages/ui/src/index.ts', import.meta.url)
)
const uiDir = fileURLToPath(new URL('../../packages/ui/src', import.meta.url))
const srcDir = fileURLToPath(new URL('./src', import.meta.url))

// https://vite.dev/config/
export default defineConfig({
  resolve: {
    alias: [
      { find: /^@\//, replacement: `${srcDir}/` },
      { find: /^@flowtools\/plugins\/(.+)$/, replacement: `${pluginsDir}/$1` },
      { find: /^@flowtools\/sdk$/, replacement: sdkEntry },
      { find: /^@flowtools\/sdk\/(.+)$/, replacement: `${sdkDir}/$1` },
      { find: /^@flowtools\/ui$/, replacement: uiEntry },
      { find: /^@flowtools\/ui\/(.+)$/, replacement: `${uiDir}/$1` },
    ],
    dedupe: ['react', 'react-dom'],
  },
  server: {
    fs: {
      allow: [workspaceRoot],
    },
  },
  plugins: [
    tanstackRouter({
      target: 'react',
      autoCodeSplitting: true,
    }),
    react({
      babel: {
        plugins: ['babel-plugin-react-compiler'],
      },
    }),
    tailwindcss(),
  ],
})

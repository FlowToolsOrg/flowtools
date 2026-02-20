import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const workspaceRoot = fileURLToPath(new URL('../..', import.meta.url))
const sdkEntry = fileURLToPath(
  new URL('../../packages/sdk/src/index.ts', import.meta.url)
)
const reactEntry = fileURLToPath(
  new URL('./node_modules/react', import.meta.url)
)
const reactDomEntry = fileURLToPath(
  new URL('./node_modules/react-dom', import.meta.url)
)

// https://vite.dev/config/
export default defineConfig({
  resolve: {
    alias: {
      '@flow-tool/sdk': sdkEntry,
      react: reactEntry,
      'react-dom': reactDomEntry,
    },
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
        plugins: [['babel-plugin-react-compiler']],
      },
    }),
    tailwindcss(),
  ],
})

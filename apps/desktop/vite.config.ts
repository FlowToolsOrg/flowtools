import { fileURLToPath, URL } from 'node:url'

import babel from '@rolldown/plugin-babel'
import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import { codeInspectorPlugin } from 'code-inspector-plugin'
import { defineConfig } from 'vite'

const host = process.env.TAURI_DEV_HOST
const devHost = host || '127.0.0.1'
const workspaceRoot = fileURLToPath(new URL('../..', import.meta.url))
const sdkEntry = fileURLToPath(
  new URL('../../packages/sdk/src/index.ts', import.meta.url)
)
const sdkDir = fileURLToPath(new URL('../../packages/sdk/src', import.meta.url))
const uiEntry = fileURLToPath(
  new URL('../../packages/ui/src/index.ts', import.meta.url)
)
const uiDir = fileURLToPath(new URL('../../packages/ui/src', import.meta.url))

// https://vite.dev/config/
export default defineConfig(async () => ({
  resolve: {
    alias: [
      { find: /^@flowtools\/sdk$/, replacement: sdkEntry },
      { find: /^@flowtools\/sdk\/(.+)$/, replacement: `${sdkDir}/$1` },
      { find: /^@flowtools\/ui$/, replacement: uiEntry },
      { find: /^@flowtools\/ui\/(.+)$/, replacement: `${uiDir}/$1` },
    ],
    dedupe: ['react', 'react-dom'],
  },
  plugins: [
    codeInspectorPlugin({
      bundler: 'vite',
    }),
    // tanstackRouter({
    //   target: 'react',
    //   autoCodeSplitting: true,
    // }),
    react(),
    babel({
      presets: [reactCompilerPreset()],
    }),
    tailwindcss(),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: devHost,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ['**/src-tauri/**'],
    },
    fs: {
      allow: [workspaceRoot],
    },
  },
}))

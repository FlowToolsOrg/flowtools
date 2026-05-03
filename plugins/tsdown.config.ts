/// <reference types="node" />

import { readdirSync } from 'node:fs'
import { dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

import { createPackageTsdownConfig } from '../configs/tsdown/create-config.ts'

const packageDir = dirname(fileURLToPath(import.meta.url))

const getEntries = () => {
  return readdirSync(packageDir).filter(file => file.startsWith('plugin-'))
}

export default createPackageTsdownConfig({
  packageDir,
  entry: getEntries().reduce((acc: Record<string, string>, name) => {
    acc[name] = `${name}/index.tsx`
    return acc
  }, {}),
  external: [
    'react/jsx-runtime',
    'react/jsx-dev-runtime',
    '@flow-tool/sdk',
    '@flow-tool/cli',
    '@flow-tool/ui',
  ],
})

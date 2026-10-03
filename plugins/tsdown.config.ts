/// <reference types="node" />

import { dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

import { createPackageTsdownConfig } from '../configs/tsdown/create-config.ts'

import { getPluginEntries } from './plugin-entries.ts'

const packageDir = dirname(fileURLToPath(import.meta.url))

export default createPackageTsdownConfig({
  packageDir,
  entry: getPluginEntries(packageDir),
  neverBundle: [
    'react/jsx-runtime',
    'react/jsx-dev-runtime',
    '@flowtools/sdk',
    '@flowtools/cli',
    '@flowtools/ui',
  ],
})

/// <reference types="node" />

import { dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

import { createPackageTsdownConfig } from '../../configs/tsdown/create-config.ts'

const packageDir = dirname(fileURLToPath(import.meta.url))

export default createPackageTsdownConfig({
  packageDir,
  entry: {
    index: 'src/index.ts',
    plugin: 'src/plugin/index.ts',
    icons: 'src/icons.ts',
  },
  neverBundle: ['react/jsx-runtime', 'react/jsx-dev-runtime'],
})

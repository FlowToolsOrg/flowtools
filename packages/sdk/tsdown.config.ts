/// <reference types="node" />

import { dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

import { createPackageTsdownConfig } from '../../configs/tsdown/create-config.ts'

const packageDir = dirname(fileURLToPath(import.meta.url))

export default createPackageTsdownConfig({
  packageDir,
  entry: {
    index: 'src/index.ts',
    utils: 'src/utils/index.ts',
    hooks: 'src/hooks/index.ts',
    types: 'src/types/index.ts',
    runtime: 'src/runtime/index.ts',
  },
  external: ['react/jsx-runtime', 'react/jsx-dev-runtime'],
})

/// <reference types="node" />

import { dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

import { createPackageTsdownConfig } from '../../configs/tsdown/create-config.ts'

const packageDir = dirname(fileURLToPath(import.meta.url))

export default createPackageTsdownConfig({
  packageDir,
  entry: {
    dependencies: 'src/dependencies/index.ts',
    data: 'src/data/index.ts',
    manifest: 'src/manifest/index.ts',
    'manifest/package': 'src/manifest/package.ts',
    'compat/catalog': 'src/compat/catalog.ts',
    development: 'src/development.ts',
    execution: 'src/execution/index.ts',
    definePlugin: 'src/compositions/definePlugin.ts',
    result: 'src/result/index.ts',
    runtime: 'src/runtime/index.ts',
    index: 'src/index.ts',
    utils: 'src/utils/index.ts',
    'utils/capability': 'src/utils/capability.ts',
    hooks: 'src/hooks/index.ts',
    types: 'src/types/index.ts',
  },
  neverBundle: ['react/jsx-runtime', 'react/jsx-dev-runtime'],
})

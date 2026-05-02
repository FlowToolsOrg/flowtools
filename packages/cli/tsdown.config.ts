/// <reference types="node" />

import { dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

import { createPackageTsdownConfig } from '../../configs/tsdown/create-config.ts'

const packageDir = dirname(fileURLToPath(import.meta.url))

export default createPackageTsdownConfig({
  packageDir,
  entry: {
    cli: 'src/cli.ts',
    index: 'src/index.ts',
  },
  platform: 'node',
})

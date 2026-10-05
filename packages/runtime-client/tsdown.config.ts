import { dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

import { createPackageTsdownConfig } from '../../configs/tsdown/create-config.ts'

export default createPackageTsdownConfig({
  packageDir: dirname(fileURLToPath(import.meta.url)),
  entry: { index: 'src/index.ts', node: 'src/node.ts' },
  neverBundle: ['node:net'],
})

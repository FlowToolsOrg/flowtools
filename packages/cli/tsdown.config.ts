/// <reference types="node" />

import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { createPackageTsdownConfig } from '../../configs/tsdown/create-config.ts'

const packageDir = dirname(fileURLToPath(import.meta.url))

const config = createPackageTsdownConfig({
  packageDir,
  entry: {
    cli: 'src/cli.ts',
    index: 'src/index.ts',
  },
  platform: 'node',
})

export default {
  ...config,
  define: {
    __FLOWTOOLS_NATIVE_BUILD_PATH__: JSON.stringify(
      resolve(
        process.env.CARGO_TARGET_DIR ?? resolve(packageDir, '../../target'),
        'debug/flowtools-runtime.exe'
      )
    ),
  },
}

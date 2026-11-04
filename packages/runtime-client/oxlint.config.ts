import { defineConfig } from 'oxlint'

import baseConfig from '../../configs/oxc/base-oxlint.config.ts'
export default defineConfig({
  extends: [baseConfig],
  // Exact AJV output is verified from the Rust schema before every build.
  ignorePatterns: ['src/wire-validators.cjs'],
})

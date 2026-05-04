import { defineConfig } from 'oxlint'

import baseOxlintConfig from './configs/oxc/base-oxlint.config.ts'

export default defineConfig({
  extends: [baseOxlintConfig],
})

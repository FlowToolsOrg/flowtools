import { z } from 'zod'

import { pluginMaturitySchema } from '../types/maturity'

import {
  manifestIdSchema,
  parsePluginManifest,
  type PluginManifestV1,
} from './schema'

const legacyMetadata = z.strictObject({
  id: manifestIdSchema,
  name: z.string().min(1),
  version: z.string().min(1),
  description: z.string().optional(),
  maturity: pluginMaturitySchema,
})

export type LegacyManifestDeclarations = Omit<
  PluginManifestV1,
  'id' | 'name' | 'version' | 'description' | 'maturity'
> & { description: string }

/** Only metadata migrates. All package, publisher, scope and operation fields are explicit. */
export function migrateLegacyManifest(
  value: unknown,
  declarations: LegacyManifestDeclarations
) {
  const meta = legacyMetadata.parse(value)
  return parsePluginManifest({
    ...declarations,
    ...meta,
    description: meta.description ?? declarations.description,
  })
}

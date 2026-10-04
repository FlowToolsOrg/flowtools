import { z } from 'zod'

/** Product maturity is not compatibility evidence or permission approval. */
export const pluginMaturitySchema = z
  .enum(['prototype', 'experimental', 'beta', 'production'])
  .default('prototype')
export type PluginMaturity = z.infer<typeof pluginMaturitySchema>

export const compatibilityEvidenceStatusSchema = z.enum([
  'indexed',
  'entry-resolved',
  'api-verified',
  'production-certified',
])
export type CompatibilityEvidenceStatus = z.infer<
  typeof compatibilityEvidenceStatusSchema
>

/** Missing legacy metadata has no validation evidence: never default to stable. */
export function resolvePluginMaturity(value: unknown): PluginMaturity {
  return pluginMaturitySchema.parse(value)
}

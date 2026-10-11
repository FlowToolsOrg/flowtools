import { z } from 'zod'

/** Build-owned export. Unknown or newly stacked custom rules must be mapped. */
export function exportCheckedSchema(
  schema: z.ZodType,
  refinements: ReadonlyMap<object, string>
) {
  return z.toJSONSchema(schema, {
    target: 'draft-7',
    io: 'input',
    cycles: 'ref',
    unrepresentable: 'any',
    override: ({ zodSchema, jsonSchema }) => {
      const definition = zodSchema._zod.def
      const refinement = refinements.get(zodSchema)
      const customChecks =
        definition.checks?.filter(check => check._zod.def.check === 'custom')
          .length ?? 0
      const custom = definition.type === 'custom' || customChecks > 0
      if (
        (custom && !refinement) ||
        customChecks > 1 ||
        ![
          'string',
          'number',
          'boolean',
          'literal',
          'enum',
          'object',
          'array',
          'record',
          'lazy',
          'custom',
          'optional',
          'default',
        ].includes(definition.type)
      )
        throw new Error('Unmapped Manifest validation logic')
      if (refinement) jsonSchema['x-flowtools-check'] = refinement
    },
  })
}

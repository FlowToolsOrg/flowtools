import { z } from 'zod'

import { commandManifestSchema } from '../manifest/command'
import { isJsonValue } from '../manifest/json-schema'

import {
  dependencyBuildFlavorSchema,
  dependencyDigestSchema,
  dependencyIdSchema,
  dependencyIdentityShape,
  dependencyRangeSchema,
  dependencyTargetSchema,
  dependencyVersionSchema,
} from './primitives'

/** Old references remain metadata. An explicit selector is needed for resolution. */
export const serviceDependencySchema = z
  .strictObject({
    ...dependencyIdentityShape,
    service: dependencyIdSchema.optional(),
    interfaceVersion: dependencyRangeSchema.optional(),
  })
  .refine(
    reference =>
      (reference.service !== undefined) ===
      (reference.interfaceVersion !== undefined),
    'Service and interfaceVersion must be declared together'
  )

/** Tool selectors describe artifacts; they cannot install, launch or grant IO. */
export const toolDependencySchema = z
  .strictObject({
    ...dependencyIdentityShape,
    target: dependencyTargetSchema.optional(),
    buildFlavor: dependencyBuildFlavorSchema.optional(),
    digest: dependencyDigestSchema.optional(),
  })
  .refine(reference => {
    const fields = [reference.target, reference.buildFlavor, reference.digest]
    return (
      fields.every(value => value === undefined) ||
      fields.every(value => value !== undefined)
    )
  }, 'Tool target, buildFlavor and digest must be declared together')

/** Provider publisher/package identity comes from the enclosing Manifest. */
export const serviceDefinitionSchema = z
  .strictObject({
    id: dependencyIdSchema,
    version: dependencyVersionSchema,
    operations: z.array(commandManifestSchema).min(1).max(64),
  })
  .superRefine((definition, ctx) => {
    const ids = definition.operations.map(operation => operation.id)
    if (new Set(ids).size !== ids.length)
      ctx.addIssue({
        code: 'custom',
        message: 'Duplicate service operation ID',
      })
    if (
      definition.operations.some(
        operation => !operation.headless || operation.interaction !== 'none'
      )
    )
      ctx.addIssue({
        code: 'custom',
        message: 'Service operations must be headless without interaction',
      })
  })

export const serviceDefinitionsSchema = z
  .array(serviceDefinitionSchema)
  .max(64)
  .refine(
    definitions =>
      new Set(definitions.map(item => item.id)).size === definitions.length,
    'Duplicate service definition ID'
  )

export const dependencyDeclarationsSchema = z
  .strictObject({
    services: z.array(serviceDependencySchema).max(128),
    tools: z.array(toolDependencySchema).max(128),
  })
  .superRefine((declarations, ctx) => {
    const serviceKeys = new Set<string>()
    const serviceModes = new Map<string, boolean>()
    for (const reference of declarations.services) {
      const provider = `${reference.publisher}/${reference.id}`
      const selected = reference.service !== undefined
      if (serviceModes.has(provider) && serviceModes.get(provider) !== selected)
        ctx.addIssue({
          code: 'custom',
          message: 'Cannot mix legacy and selected service dependencies',
        })
      serviceModes.set(provider, selected)
      const key = `${provider}/${reference.service ?? ''}`
      if (serviceKeys.has(key))
        ctx.addIssue({
          code: 'custom',
          message: 'Duplicate service dependency',
        })
      serviceKeys.add(key)
    }
    const toolKeys = new Set<string>()
    const toolModes = new Map<string, boolean>()
    for (const reference of declarations.tools) {
      const provider = `${reference.publisher}/${reference.id}`
      const selected = reference.target !== undefined
      if (toolModes.has(provider) && toolModes.get(provider) !== selected)
        ctx.addIssue({
          code: 'custom',
          message: 'Cannot mix legacy and selected tool dependencies',
        })
      toolModes.set(provider, selected)
      // One consumer cannot request two artifacts for the same target/flavor.
      // Different consumers may resolve different immutable versions.
      const key = `${provider}/${reference.target?.platform ?? ''}/${reference.target?.arch ?? ''}/${reference.buildFlavor ?? ''}`
      if (toolKeys.has(key))
        ctx.addIssue({ code: 'custom', message: 'Ambiguous tool dependency' })
      toolKeys.add(key)
    }
  })

export type ServiceDefinition = z.infer<typeof serviceDefinitionSchema>
export type ServiceDependency = z.infer<typeof serviceDependencySchema>
export type ToolDependency = z.infer<typeof toolDependencySchema>
export type DependencyDeclarations = z.infer<
  typeof dependencyDeclarationsSchema
>

function assertBoundedDeclarations(value: unknown): void {
  if (
    !isJsonValue(value) ||
    new TextEncoder().encode(JSON.stringify(value)).length > 1_048_576
  )
    throw new Error('Dependency declarations must contain bounded JSON data')
}

/** No resolver, package IO, lock authority or grant is exposed by this subpath. */
export function parseDependencyDeclarations(
  value: unknown
): DependencyDeclarations {
  assertBoundedDeclarations(value)
  return dependencyDeclarationsSchema.parse(value)
}

export function parseServiceDefinitions(value: unknown): ServiceDefinition[] {
  assertBoundedDeclarations(value)
  return serviceDefinitionsSchema.parse(value)
}

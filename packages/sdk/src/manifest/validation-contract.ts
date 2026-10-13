import { portablePluginPathSchema } from '../compat/catalog'
import {
  dependencyRangeSchema,
  dependencyVersionSchema,
} from '../dependencies/primitives'
import {
  dependencyDeclarationsSchema,
  serviceDefinitionsSchema,
  serviceDefinitionSchema,
  serviceDependencySchema,
  toolDependencySchema,
} from '../dependencies/schema'

import { commandManifestSchema } from './command'
import { exportOperationValidationSchema, operationSchema } from './json-schema'
import { pluginManifestSchema } from './schema'
import { exportCheckedSchema } from './validation-export'

/** Structural contract plus explicit native semantics; never trust or grants. */
export function exportManifestValidationContract() {
  const command = commandManifestSchema.shape
  const refinements = new Map<object, string>([
    [pluginManifestSchema, 'manifest-v1'],
    [commandManifestSchema, 'command-v1'],
    [command.effects, 'unique'],
    [command.permissions.element.shape.operations, 'unique'],
    [command.permissions.element.shape.scopes, 'unique'],
    [dependencyVersionSchema, 'version'],
    [dependencyRangeSchema, 'range'],
    [portablePluginPathSchema, 'path'],
    [dependencyDeclarationsSchema, 'dependencies-v1'],
    [serviceDefinitionsSchema, 'services-v1'],
    [serviceDefinitionSchema, 'service-v1'],
    [serviceDependencySchema, 'service-selector-v1'],
    [toolDependencySchema, 'tool-selector-v1'],
    [operationSchema, 'operation-v1'],
  ])
  return {
    formatVersion: 1,
    semantics: 'flowtools-manifest-v1',
    manifest: exportCheckedSchema(pluginManifestSchema, refinements),
    operation: exportOperationValidationSchema(),
  }
}

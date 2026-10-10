import type {
  ServiceDefinition,
  ServiceDependency,
  ToolDependency,
} from '../../src/dependencies'
import type { PluginManifestV1 } from '../../src/manifest'

import { manifestFixture } from './manifest-v1'

export function serviceDefinitionFixture(): ServiceDefinition {
  return {
    id: 'text-transform',
    version: '1.2.0',
    operations: [manifestFixture().commands[0]!],
  }
}

export function serviceDependencyFixture(): ServiceDependency {
  return {
    publisher: 'flowtools',
    id: 'provider-plugin',
    version: '^1.0.0',
    service: 'text-transform',
    interfaceVersion: '^1.2.0',
  }
}

export function toolDependencyFixture(): ToolDependency {
  return {
    publisher: 'flowtools',
    id: 'fixture-tool',
    version: '^2.0.0',
    target: { platform: 'windows', arch: 'x64' },
    buildFlavor: 'standard',
    digest: '1'.repeat(64),
  }
}

export function serviceManifestFixture(): PluginManifestV1 {
  const manifest = manifestFixture()
  manifest.entries.services = 'dist/services.js'
  manifest.files.push({
    path: 'dist/services.js',
    size: 0,
    sha256: '1'.repeat(64),
  })
  manifest.services = [serviceDefinitionFixture()]
  return manifest
}

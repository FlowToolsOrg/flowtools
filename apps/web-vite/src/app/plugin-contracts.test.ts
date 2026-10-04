/// <reference types="bun-types" />

import type { FlowToolPlugin, PluginManifestEntry } from '@flowtools/sdk'

import { describe, expect, test } from 'bun:test'

import { CommandRegistry, PluginLoader, PluginRegistry } from '@flowtools/sdk'

import { builtInManifests, pluginCategories } from '../plugin/manifests'
import { executeCommand } from '../stores/command-store'
import { pluginRegistryInternals } from '../stores/plugin-registry-store'

import { registerPluginCommands } from './plugin-commands'

// These integration cases import built packages and their UI dependency graph.
// Cold Windows runners need an explicit bound, not an execution-speed assertion.
const pluginLoadTimeoutMs = 30_000

function registerEnabledFixture(
  registry: PluginRegistry,
  plugin: FlowToolPlugin,
  manifestType: PluginManifestEntry['type']
): void {
  const { id, name, version } = plugin.meta

  registry.register({
    id,
    name,
    version,
    type: manifestType,
    loader: () => Promise.resolve({ default: plugin }),
  })
  registry.updateState(id, { plugin, state: 'enabled' })
}

describe('plugin command registration', () => {
  test('palette execution reaches the host registry and records recent commands', async () => {
    const commandRegistry = new CommandRegistry()
    const executed: string[] = []
    const previousRegistry = pluginRegistryInternals._commandRegistry

    commandRegistry.register({
      id: 'plugin:fixture-palette',
      pluginId: 'fixture-palette',
      title: 'Fixture Palette',
      mode: 'panel',
      keywords: [],
      handler: () => {
        executed.push('fixture-palette')
      },
    })
    pluginRegistryInternals._commandRegistry = commandRegistry

    try {
      await executeCommand('plugin:fixture-palette')
      expect(executed).toEqual(['fixture-palette'])
      expect(commandRegistry.getRecent().map(command => command.id)).toEqual([
        'plugin:fixture-palette',
      ])
    } finally {
      pluginRegistryInternals._commandRegistry = previousRegistry
    }
  })

  test('palette execution rejects an uninitialized host instead of silently succeeding', async () => {
    const previousRegistry = pluginRegistryInternals._commandRegistry
    pluginRegistryInternals._commandRegistry = null

    try {
      const failure = await executeCommand('plugin:fixture-palette').catch(
        (error: unknown) => error
      )
      expect(failure).toBeInstanceOf(Error)
      expect(failure instanceof Error ? failure.message : undefined).toBe(
        'Command registry not initialized'
      )
    } finally {
      pluginRegistryInternals._commandRegistry = previousRegistry
    }
  })

  test('maps app and tool modes while skipping unknown runtime types', async () => {
    const registry = new PluginRegistry()
    const commandRegistry = new CommandRegistry()
    const navigations: string[] = []
    const appPlugin: FlowToolPlugin = {
      type: 'app',
      meta: {
        id: 'fixture-app',
        name: 'Fixture App',
        version: '1.0.0',
      },
      setup: () => () => null,
    }
    const toolPlugin: FlowToolPlugin = {
      type: 'tool',
      meta: {
        id: 'fixture-tool',
        name: 'Fixture Tool',
        version: '1.0.0',
      },
      run: () => undefined,
    }
    const unknownPlugin = {
      type: 'widget',
      meta: {
        id: 'fixture-unknown',
        name: 'Fixture Unknown',
        version: '1.0.0',
      },
    } as unknown as FlowToolPlugin

    registerEnabledFixture(registry, appPlugin, 'app')
    registerEnabledFixture(registry, toolPlugin, 'tool')
    registerEnabledFixture(registry, unknownPlugin, 'app')

    registerPluginCommands(registry, commandRegistry, path => {
      navigations.push(path)
    })

    expect(
      commandRegistry.getAll().map(command => [command.id, command.mode])
    ).toEqual([
      ['plugin:fixture-app', 'panel'],
      ['plugin:fixture-tool', 'headless'],
    ])
    expect(commandRegistry.get('plugin:fixture-unknown')).toBeUndefined()

    await commandRegistry.execute('plugin:fixture-app')
    await commandRegistry.execute('plugin:fixture-tool')
    expect(navigations).toEqual(['/tools/fixture-app', '/tools/fixture-tool'])
  })
})

describe('built-in plugin contracts', () => {
  test('manifest ids, host command ids, and category membership are unique', () => {
    const manifestIds = builtInManifests.map(manifest => manifest.id)
    const commandIds = manifestIds.map(pluginId => `plugin:${pluginId}`)
    const categorizedPluginIds: string[] = pluginCategories.flatMap(
      category => [...category.pluginIds]
    )

    expect(new Set(manifestIds).size).toBe(manifestIds.length)
    expect(new Set(commandIds).size).toBe(commandIds.length)
    expect(new Set(categorizedPluginIds).size).toBe(categorizedPluginIds.length)
    expect([...categorizedPluginIds].sort()).toEqual([...manifestIds].sort())

    for (const id of manifestIds) {
      expect(id).toMatch(/^[a-z0-9]+(?:-[a-z0-9]+)*$/)
    }
  })

  test.each(builtInManifests)(
    'manifest $id metadata matches the loaded SDK plugin contract',
    async manifest => {
      const { default: plugin } = await manifest.loader()

      expect({
        id: plugin.meta.id,
        name: plugin.meta.name,
        version: plugin.meta.version,
        maturity: plugin.meta.maturity,
        description: plugin.meta.description,
        type: plugin.type,
        permissions: plugin.meta.permissions ?? [],
        tags: plugin.meta.tags ?? [],
        category: plugin.meta.category,
        cliAvailable: typeof plugin.run === 'function',
      }).toEqual({
        id: manifest.id,
        name: manifest.name,
        version: manifest.version,
        maturity: manifest.maturity,
        description: manifest.description,
        type: manifest.type,
        permissions: manifest.permissions ?? [],
        tags: manifest.tags ?? [],
        category: manifest.category,
        cliAvailable: manifest.cliAvailable ?? false,
      })
    },
    pluginLoadTimeoutMs
  )

  test(
    'enabled plugins register one executable host command each',
    async () => {
      const registry = new PluginRegistry()
      const commandRegistry = new CommandRegistry()
      const loader = new PluginLoader(registry)
      const navigations: string[] = []

      registry.registerAll(builtInManifests)
      await loader.loadAll()
      await loader.enableAll()
      registerPluginCommands(registry, commandRegistry, path => {
        navigations.push(path)
      })

      const commands = commandRegistry.getAll()
      expect(commands).toHaveLength(builtInManifests.length)
      expect(commands.map(command => command.id)).toEqual(
        builtInManifests.map(manifest => `plugin:${manifest.id}`)
      )

      for (const command of commands) {
        const plugin = registry.get(command.pluginId)?.plugin
        if (!plugin) {
          throw new Error(`Command references unloaded plugin: ${command.id}`)
        }

        expect(command.title).toBe(plugin.meta.name)
        expect(command.mode).toBe(plugin.type === 'app' ? 'panel' : 'headless')
      }

      const firstCommand = commands[0]
      if (!firstCommand) {
        throw new Error('Expected at least one built-in command')
      }

      await commandRegistry.execute(firstCommand.id)
      expect(navigations).toEqual([`/tools/${firstCommand.pluginId}`])
    },
    pluginLoadTimeoutMs
  )
})

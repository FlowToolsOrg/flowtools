import type { RegisteredCommand } from '../src/registry/command-registry'

import { describe, expect, test } from 'bun:test'

import { CommandRegistry } from '../src/registry/command-registry'

async function getError(promise: Promise<unknown>): Promise<Error> {
  try {
    await promise
  } catch (error) {
    return error instanceof Error ? error : new Error(String(error))
  }

  throw new Error('Expected promise to reject')
}

function createCommand(
  id: string,
  overrides: Partial<RegisteredCommand> = {}
): RegisteredCommand {
  return {
    id,
    title: id,
    pluginId: 'fixture-plugin',
    mode: 'headless',
    keywords: [],
    handler: () => {},
    ...overrides,
  }
}

describe('CommandRegistry search', () => {
  test('orders matches by title, description, keyword, then plugin id', () => {
    const registry = new CommandRegistry()
    registry.register(
      createCommand('title-command', {
        title: 'Needle title',
        pluginId: 'title-plugin',
      })
    )
    registry.register(
      createCommand('description-command', {
        title: 'Description only',
        description: 'Contains needle here',
        pluginId: 'description-plugin',
      })
    )
    registry.register(
      createCommand('keyword-command', {
        title: 'Keyword only',
        keywords: ['needle'],
        pluginId: 'keyword-plugin',
      })
    )
    registry.register(
      createCommand('plugin-command', {
        title: 'Plugin id only',
        pluginId: 'needle-plugin',
      })
    )

    expect(registry.search('  NEEDLE ').map(command => command.id)).toEqual([
      'title-command',
      'description-command',
      'keyword-command',
      'plugin-command',
    ])
  })
})

describe('CommandRegistry execution history', () => {
  test('records a command only after its handler succeeds', async () => {
    const registry = new CommandRegistry()
    const error = new Error('execution failed')
    let successfulRuns = 0
    registry.register(
      createCommand('successful-command', {
        handler: () => {
          successfulRuns += 1
        },
      })
    )
    registry.register(
      createCommand('failing-command', {
        handler: () => {
          throw error
        },
      })
    )

    await registry.execute('successful-command')
    expect(await getError(registry.execute('failing-command'))).toBe(error)
    await registry.execute('missing-command')

    expect(successfulRuns).toBe(1)
    expect(registry.getRecent().map(command => command.id)).toEqual([
      'successful-command',
    ])
  })

  test('keeps five unique commands in most-recent-first order', async () => {
    const registry = new CommandRegistry()
    for (let index = 1; index <= 6; index += 1) {
      registry.register(createCommand(`command-${index}`))
      await registry.execute(`command-${index}`)
    }

    expect(registry.getRecent().map(command => command.id)).toEqual([
      'command-6',
      'command-5',
      'command-4',
      'command-3',
      'command-2',
    ])

    await registry.execute('command-4')
    expect(registry.getRecent().map(command => command.id)).toEqual([
      'command-4',
      'command-6',
      'command-5',
      'command-3',
      'command-2',
    ])
  })

  test('unregisters plugin commands and recent entries with one notification', async () => {
    const registry = new CommandRegistry()
    registry.register(
      createCommand('first-command', { pluginId: 'first-plugin' })
    )
    registry.register(
      createCommand('second-command', { pluginId: 'first-plugin' })
    )
    registry.register(
      createCommand('kept-command', { pluginId: 'kept-plugin' })
    )
    await registry.execute('first-command')
    await registry.execute('kept-command')

    let notifications = 0
    registry.subscribe(() => {
      notifications += 1
    })

    registry.unregisterByPlugin('first-plugin')
    registry.unregisterByPlugin('first-plugin')

    expect(registry.getAll().map(command => command.id)).toEqual([
      'kept-command',
    ])
    expect(registry.getRecent().map(command => command.id)).toEqual([
      'kept-command',
    ])
    expect(notifications).toBe(1)
  })
})

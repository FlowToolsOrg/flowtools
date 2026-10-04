import type { PluginManifestEntry } from '../src/registry/types'
import type { FlowToolPlugin } from '../src/types/plugin'

import { expect, test } from 'bun:test'

import { projectPluginCommands } from '../src/registry/command-projection'
import { CommandRegistry } from '../src/registry/command-registry'
import { PluginLoader } from '../src/registry/plugin-loader'
import { PluginRegistry } from '../src/registry/plugin-registry'

function fixture() {
  const registry = new PluginRegistry()
  const commands = new CommandRegistry()
  const loader = new PluginLoader(registry)
  const plugin: FlowToolPlugin = {
    type: 'tool',
    meta: { id: 'fixture', name: 'Fixture', version: '1.0.0' },
    run: () => undefined,
  }
  const manifest: PluginManifestEntry = {
    id: 'fixture',
    name: 'Fixture',
    version: '1.0.0',
    type: 'tool',
    loader: async () => ({ default: plugin }),
  }
  registry.register(manifest)
  let calls = 0
  const dispose = projectPluginCommands(registry, commands, entry => [
    {
      id: 'fixture:run',
      title: 'Run',
      pluginId: entry.id,
      mode: 'headless',
      keywords: [],
      handler: () => {
        calls++
      },
    },
  ])
  return {
    registry,
    commands,
    loader,
    manifest,
    plugin,
    dispose,
    calls: () => calls,
  }
}

test('commands follow enabled/dependency state and invalidate singleton generations', async () => {
  const { loader, commands, calls, dispose } = fixture()
  expect(commands.getAll()).toHaveLength(0)
  await loader.enable('fixture')
  const stale = commands.get('fixture:run')!
  await commands.execute('fixture:run')
  await loader.disable('fixture')
  expect(commands.getAll()).toHaveLength(0)
  expect(commands.getRecent()).toHaveLength(0)
  expect(
    await Promise.resolve()
      .then(stale.handler)
      .catch((e: unknown) => e)
  ).toBeInstanceOf(Error)
  await loader.enable('fixture')
  await loader.reload('fixture')
  expect(
    await Promise.resolve()
      .then(stale.handler)
      .catch((e: unknown) => e)
  ).toBeInstanceOf(Error)
  expect(calls()).toBe(1)
  dispose()
  await loader.reload('fixture')
  expect(commands.getAll()).toHaveLength(0)
})

test('accepted execution drains before update; later old handler rejects', async () => {
  const { registry, loader, commands, manifest } = fixture()
  await loader.enable('fixture')
  const stale = commands.get('fixture:run')!
  let release!: () => void
  let entered!: () => void
  const started = new Promise<void>(resolve => {
    entered = resolve
  })
  const pending = loader.withPlugin('fixture', async () => {
    entered()
    await new Promise<void>(resolve => {
      release = resolve
    })
    expect(registry.get('fixture')?.manifest.version).toBe('1.0.0')
  })
  await started
  const updated = loader.update('fixture', {
    ...manifest,
    version: '2.0.0',
    loader: async () => ({
      default: {
        type: 'tool',
        meta: { id: 'fixture', name: 'Fixture', version: '2.0.0' },
        run: () => undefined,
      },
    }),
  })
  const late = Promise.resolve()
    .then(stale.handler)
    .catch((e: unknown) => e)
  release()
  await pending
  await updated
  expect(await late).toBeInstanceOf(Error)
  expect(registry.get('fixture')?.manifest.version).toBe('2.0.0')
  await commands.execute('fixture:run')
  await loader.uninstall('fixture')
  expect(commands.getAll()).toHaveLength(0)
})

test('dependency refusal does not activate or publish commands', async () => {
  const { loader, commands, manifest } = fixture()
  await loader.update('fixture', { ...manifest, dependenciesSatisfied: false })
  expect(
    await loader.enable('fixture').catch((e: unknown) => e)
  ).toBeInstanceOf(Error)
  expect(commands.getAll()).toHaveLength(0)
})

test('reload and uninstall clean timers, subscriptions, views and real child process', async () => {
  const { registry, loader, manifest, plugin } = fixture()
  const target = new EventTarget()
  let listeners = 0
  let timers = 0
  let views = 0
  plugin.lifecycle = {
    onLoad: scope => {
      const timer = setInterval(() => {}, 1000)
      timers++
      scope.add(() => {
        clearInterval(timer)
        timers--
      })
    },
    onActivate: scope => {
      const listener = () => {
        listeners++
      }
      target.addEventListener('fixture', listener)
      scope.add(() => target.removeEventListener('fixture', listener))
    },
  }
  await loader.enable('fixture')
  expect(registry.isRunning('fixture')).toBe(false)
  const child = Bun.spawn(
    [process.execPath, '-e', 'setInterval(() => {}, 1000)'],
    { stdout: 'ignore', stderr: 'ignore' }
  )
  registry.resourceScope('fixture', 'runner').add(async () => {
    child.kill()
    await child.exited
  })
  views++
  registry.resourceScope('fixture', 'view').add(() => {
    views--
  })
  expect(registry.isRunning('fixture')).toBe(true)
  await loader.reload('fixture')
  expect(await child.exited).toBeNumber()
  expect(registry.isRunning('fixture')).toBe(false)
  expect(views).toBe(0)
  expect(timers).toBe(1)
  target.dispatchEvent(new Event('fixture'))
  expect(listeners).toBe(1)
  await loader.update('fixture', manifest)
  await loader.uninstall('fixture')
  target.dispatchEvent(new Event('fixture'))
  expect(listeners).toBe(1)
  expect(timers).toBe(0)
})

test('resource cleanup failure remains owned and blocks uninstall until recovered', async () => {
  const { registry, loader } = fixture()
  await loader.enable('fixture')
  let failing = true
  registry.resourceScope('fixture', 'view').add(() => {
    if (failing) throw new Error('view failed')
  })
  expect(
    await loader.uninstall('fixture').catch((e: unknown) => e)
  ).toBeInstanceOf(Error)
  expect(registry.get('fixture')?.state).toBe('error')
  failing = false
  await loader.uninstall('fixture')
  expect(registry.size).toBe(0)
})

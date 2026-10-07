import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'

import { PluginRegistry, PluginLoader } from '../dist/index.js'

const registry = new PluginRegistry()
const loader = new PluginLoader(registry)
const target = new EventTarget()
let listeners = 0,
  timers = 0,
  views = 0
/** @type {import('../dist/index.js').FlowToolPlugin} */
const plugin = {
  type: 'tool',
  meta: { id: 'fixture', name: 'Fixture', version: '1.0.0' },
  run: () => undefined,
  lifecycle: {
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
  },
}
const manifest = {
  id: 'fixture',
  name: 'Fixture',
  version: '1.0.0',
  type: /** @type {const} */ ('tool'),
  loader: async () => ({ default: plugin }),
}
registry.register(manifest)
await loader.enable('fixture')
assert.equal(registry.isRunning('fixture'), false)
const child = spawn(
  process.execPath,
  ['-e', "process.stdout.write('ready');setInterval(()=>{},1000)"],
  { windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'] }
)
const exited = /** @type {Promise<[number|null,string|null]>} */ (
  once(child, 'exit')
)
await once(child.stdout, 'data')
registry.resourceScope('fixture', 'runner').add(async () => {
  child.kill()
  await exited
})
views++
registry.resourceScope('fixture', 'view').add(() => {
  views--
})
assert.equal(registry.isRunning('fixture'), true)
await loader.reload('fixture')
const [code, signal] = await exited
assert.ok(typeof code === 'number' || signal !== null)
assert.equal(registry.isRunning('fixture'), false)
assert.equal(views, 0)
assert.equal(timers, 1)
target.dispatchEvent(new Event('fixture'))
assert.equal(listeners, 1)
await loader.update('fixture', manifest)
await loader.uninstall('fixture')
target.dispatchEvent(new Event('fixture'))
assert.equal(listeners, 1)
assert.equal(timers, 0)
process.stdout.write('Resource cleanup fixture passed\n')

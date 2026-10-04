import type { ToolContext } from '@flowtools/sdk/types'

import base64 from '@flowtools/plugins/plugin-base64-encoder'
import { createExecutionHistory, executePlugin } from '@flowtools/sdk/execution'
import { ExecutionPanel } from '@flowtools/ui'
import { expect, it } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

const context = (id: string): ToolContext => ({
  env: { pluginId: id, pluginType: 'app', platform: 'web', mode: 'test' },
  signal: new AbortController().signal,
  ui: { toast: () => {}, openPanel: () => {}, closePanel: () => {} },
  log: () => {},
  utils: { now: Date.now },
})

it('shows actual result, schema rejection, safe history and JSON parse errors', async () => {
  const history = createExecutionHistory(undefined, 'fixture')
  const screen = await render(
    <ExecutionPanel
      meta={base64.meta}
      history={history}
      execute={(input, signal) =>
        executePlugin(base64, input, context(base64.meta.id), { signal })
      }
    />
  )
  const user = userEvent.setup()
  const input = screen.getByRole('textbox', { name: 'JSON input' })
  await input.fill('{"text":"hello"}')
  await user.click(screen.getByRole('button', { name: 'Run JSON' }))
  await expect
    .element(screen.getByTestId('execution-output'))
    .toHaveTextContent('aGVsbG8=')
  expect(history.getSnapshot().entries[0]).toMatchObject({
    status: 'success',
    resultType: 'json',
  })
  await input.fill('{"text":3}')
  await user.click(screen.getByRole('button', { name: 'Run JSON' }))
  await expect
    .element(screen.getByRole('status'))
    .toHaveTextContent('INPUT_INVALID')
  await input.fill('{secret-token')
  await user.click(screen.getByRole('button', { name: 'Run JSON' }))
  await expect
    .element(screen.getByRole('status'))
    .toHaveTextContent('INPUT_INVALID')
  expect(JSON.stringify(history.getSnapshot())).not.toContain('secret-token')
  await user.click(screen.getByRole('button', { name: 'Clear history' }))
  expect(history.getSnapshot().entries).toEqual([])
})

it('cancels actual pending run without accepting its late result', async () => {
  let finish: (value: string) => void = () => {}
  const plugin = {
    meta: { id: 'ui-cancel-fixture', name: 'Cancel fixture', version: '1.0.0' },
    run: () =>
      new Promise<string>(resolve => {
        finish = resolve
      }),
  }
  const history = createExecutionHistory(undefined, 'cancel-fixture')
  const screen = await render(
    <ExecutionPanel
      meta={plugin.meta}
      history={history}
      execute={(input, signal) =>
        executePlugin(plugin, input, context(plugin.meta.id), { signal })
      }
    />
  )
  const user = userEvent.setup()
  await user.click(screen.getByRole('button', { name: 'Run JSON' }))
  await expect
    .element(screen.getByRole('button', { name: 'Cancel' }))
    .toBeEnabled()
  await user.click(screen.getByRole('button', { name: 'Cancel' }))
  await expect.element(screen.getByRole('status')).toHaveTextContent('ABORTED')
  expect(history.getSnapshot().entries[0]?.status).toBe('cancelled')
  await expect
    .element(screen.getByRole('button', { name: 'Run JSON' }))
    .toBeEnabled()
  finish('late-success-must-not-win')
  await expect.element(screen.getByRole('status')).toHaveTextContent('ABORTED')
  expect(history.getSnapshot().entries).toHaveLength(1)
})

it('records cancellation when the running panel unmounts', async () => {
  let signal: AbortSignal | undefined
  const plugin = {
    meta: {
      id: 'ui-unmount-fixture',
      name: 'Unmount fixture',
      version: '1.0.0',
    },
    run: (ctx: ToolContext) => {
      signal = ctx.signal
      return new Promise<string>(() => {})
    },
  }
  const history = createExecutionHistory(undefined, 'unmount')
  const screen = await render(
    <ExecutionPanel
      meta={plugin.meta}
      history={history}
      execute={(input, upstream) =>
        executePlugin(plugin, input, context(plugin.meta.id), {
          signal: upstream,
        })
      }
    />
  )
  await userEvent
    .setup()
    .click(screen.getByRole('button', { name: 'Run JSON' }))
  await expect
    .element(screen.getByRole('button', { name: 'Cancel' }))
    .toBeEnabled()
  await screen.unmount()
  await expect.poll(() => signal?.aborted).toBe(true)
  await expect
    .poll(() => history.getSnapshot().entries[0]?.status)
    .toBe('cancelled')
})

it('shows actual exceptions and invalid JSON output as failed attempts', async () => {
  let throwing = true
  const plugin = {
    meta: { id: 'ui-error-fixture', name: 'Error fixture', version: '1.0.0' },
    run: () => {
      if (throwing) throw new Error('actual plugin exception')
      return { type: 'json', value: 1n }
    },
  }
  const history = createExecutionHistory(undefined, 'errors')
  const screen = await render(
    <ExecutionPanel
      meta={plugin.meta}
      history={history}
      execute={(input, signal) =>
        executePlugin(plugin, input, context(plugin.meta.id), { signal })
      }
    />
  )
  const user = userEvent.setup()
  await user.click(screen.getByRole('button', { name: 'Run JSON' }))
  await expect
    .element(screen.getByRole('status'))
    .toHaveTextContent('EXECUTION_FAILED')
  throwing = false
  await user.click(screen.getByRole('button', { name: 'Run JSON' }))
  await expect
    .element(screen.getByRole('status'))
    .toHaveTextContent('OUTPUT_INVALID')
  expect(history.getSnapshot().entries[0]?.status).toBe('error')
})

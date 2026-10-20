import type { Outcome, RuntimeStatus } from '@flowtools/runtime-client'

import { RuntimeClientError } from '@flowtools/runtime-client'
import { beforeEach, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { controlManagedRuntime } from '../../../../desktop/src/runtime/managed-client'
import { RuntimeStatusSettings } from '../../../../desktop/src/settings/runtime-status-settings'

const environment = vi.hoisted(() => ({ native: true }))
const user = userEvent.setup()
vi.mock('../../../../desktop/src/runtime/desktop-capabilities', () => ({
  isTauriRuntime: () => environment.native,
}))
vi.mock('../../../../desktop/src/runtime/managed-client', () => ({
  controlManagedRuntime: vi.fn(),
}))

const snapshot: RuntimeStatus = {
  instanceId: 'fixture-status-host',
  mode: 'managed',
  jobs: 12,
  activeJobs: 3,
}

beforeEach(() => {
  environment.native = true
  vi.clearAllMocks()
  vi.mocked(controlManagedRuntime).mockResolvedValue({
    type: 'status',
    data: snapshot,
  })
})

it('mount and remount never query or start Runtime; explicit keyboard query shows a snapshot', async () => {
  const first = await render(<RuntimeStatusSettings />)
  await expect.element(first.getByRole('status')).toHaveTextContent('尚未查询')
  expect(controlManagedRuntime).not.toHaveBeenCalled()
  await first.unmount()
  const view = await render(<RuntimeStatusSettings />)
  const query = view.getByRole('button', { name: '查询 Runtime 状态' })
  query.element().focus()
  await user.keyboard('{Enter}')
  await expect.element(view.getByRole('status')).toHaveTextContent('查询成功')
  await expect
    .element(view.getByRole('region', { name: 'Runtime 状态' }))
    .toHaveTextContent('3 个活动任务')
  await expect.element(view.getByText('fixture-status-host')).toBeVisible()
  await expect.element(view.getByText(/查询时间/)).toBeVisible()
  expect(controlManagedRuntime).toHaveBeenCalledExactlyOnceWith(
    { method: 'runtime.status' },
    false
  )
})

it('clears the previous snapshot while querying and after a disconnection', async () => {
  const view = await render(<RuntimeStatusSettings />)
  const query = view.getByRole('button', { name: '查询 Runtime 状态' })
  await user.click(query)
  await expect.element(view.getByText('fixture-status-host')).toBeVisible()
  let reject: (error: unknown) => void = () => {}
  vi.mocked(controlManagedRuntime).mockImplementationOnce(
    () =>
      new Promise((_resolve, fail) => {
        reject = fail
      })
  )
  await user.click(query)
  await expect.element(query).toBeDisabled()
  await expect
    .element(view.getByText('fixture-status-host'))
    .not.toBeInTheDocument()
  reject(new RuntimeClientError('RUNTIME_DISCONNECTED'))
  await expect.element(view.getByRole('status')).toHaveTextContent('无法确认')
  await expect
    .element(view.getByRole('status'))
    .toHaveTextContent('RUNTIME_DISCONNECTED')
  await expect
    .element(view.getByText('fixture-status-host'))
    .not.toBeInTheDocument()
  await expect.element(query).toBeEnabled()
  expect(controlManagedRuntime).toHaveBeenCalledTimes(2)
})

it('reports unsupported or uninitialized access without claiming offline or retrying', async () => {
  vi.mocked(controlManagedRuntime).mockRejectedValueOnce(
    new RuntimeClientError('APPROVAL_REQUIRED')
  )
  const view = await render(<RuntimeStatusSettings />)
  await user.click(view.getByRole('button', { name: '查询 Runtime 状态' }))
  await expect.element(view.getByRole('status')).toHaveTextContent('无法确认')
  await expect
    .element(view.getByRole('status'))
    .toHaveTextContent('APPROVAL_REQUIRED')
  expect(controlManagedRuntime).toHaveBeenCalledTimes(1)
})

it('rejects a mismatched outcome instead of claiming a successful query', async () => {
  vi.mocked(controlManagedRuntime).mockResolvedValueOnce({ type: 'stopping' })
  const view = await render(<RuntimeStatusSettings />)
  await user.click(view.getByRole('button', { name: '查询 Runtime 状态' }))
  await expect
    .element(view.getByRole('status'))
    .toHaveTextContent('INVALID_RESPONSE')
  await expect
    .element(view.getByText('fixture-status-host'))
    .not.toBeInTheDocument()
})

it('browser preview has an honest disabled control and performs no native call', async () => {
  environment.native = false
  const view = await render(<RuntimeStatusSettings />)
  await expect
    .element(view.getByRole('button', { name: '查询 Runtime 状态' }))
    .toBeDisabled()
  await expect
    .element(view.getByRole('status'))
    .toHaveTextContent('原生 Desktop')
  expect(controlManagedRuntime).not.toHaveBeenCalled()
})

it('keeps unknown Host modes visible as unknown without rendering object members', async () => {
  vi.mocked(controlManagedRuntime).mockResolvedValueOnce({
    type: 'status',
    data: { ...snapshot, mode: 'toString' },
  })
  const view = await render(<RuntimeStatusSettings />)
  await user.click(view.getByRole('button', { name: '查询 Runtime 状态' }))
  await expect.element(view.getByText('未知模式')).toBeVisible()
  await expect.element(view.getByText('fixture-status-host')).toBeVisible()
})

it('late completion from an unmounted page does not populate a new page', async () => {
  let complete: (value: Outcome) => void = () => {}
  vi.mocked(controlManagedRuntime).mockImplementationOnce(
    () =>
      new Promise(resolve => {
        complete = resolve
      })
  )
  const first = await render(<RuntimeStatusSettings />)
  await user.click(first.getByRole('button', { name: '查询 Runtime 状态' }))
  await first.unmount()
  const next = await render(<RuntimeStatusSettings />)
  complete({ type: 'status', data: snapshot })
  await expect.element(next.getByRole('status')).toHaveTextContent('尚未查询')
  await expect
    .element(next.getByText('fixture-status-host'))
    .not.toBeInTheDocument()
  expect(controlManagedRuntime).toHaveBeenCalledTimes(1)
})

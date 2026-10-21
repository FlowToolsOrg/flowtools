import type { Outcome, RuntimeStatus } from '@flowtools/runtime-client'

import { RuntimeClientError } from '@flowtools/runtime-client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { controlManagedRuntime } from '../../../../desktop/src/runtime/managed-client'
import { readDesktopDiagnosticVersions } from '../../../../desktop/src/settings/runtime-diagnostic-download'
import { RuntimeStatusSettings } from '../../../../desktop/src/settings/runtime-status-settings'

const environment = vi.hoisted(() => ({ native: true }))
const user = userEvent.setup()
vi.mock('../../../../desktop/src/runtime/desktop-capabilities', () => ({
  isTauriRuntime: () => environment.native,
}))
vi.mock('../../../../desktop/src/runtime/managed-client', () => ({
  controlManagedRuntime: vi.fn(),
}))
vi.mock(
  '../../../../desktop/src/settings/runtime-diagnostic-download',
  async importOriginal => ({
    ...(await importOriginal<
      typeof import('../../../../desktop/src/settings/runtime-diagnostic-download')
    >()),
    readDesktopDiagnosticVersions: vi.fn(),
  })
)

const snapshot: RuntimeStatus = {
  instanceId: 'fixture-status-host',
  mode: 'managed',
  jobs: 12,
  activeJobs: 3,
}

beforeEach(() => {
  environment.native = true
  vi.clearAllMocks()
  vi.mocked(readDesktopDiagnosticVersions).mockResolvedValue({
    appVersion: '0.1.0',
    tauriVersion: '2.11.0',
  })
  vi.mocked(controlManagedRuntime).mockResolvedValue({
    type: 'status',
    data: snapshot,
  })
})

afterEach(() => vi.restoreAllMocks())

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

it('previews an unqueried report before a separate explicit download without querying Runtime', async () => {
  const blobs: Blob[] = []
  const create = vi.spyOn(URL, 'createObjectURL').mockImplementation(value => {
    if (!(value instanceof Blob)) throw new Error('Expected diagnostic Blob')
    blobs.push(value)
    return 'blob:fixture-diagnostic'
  })
  const revoke = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {})
  const links: string[] = []
  const click = vi
    .spyOn(HTMLAnchorElement.prototype, 'click')
    .mockImplementation(function (this: HTMLAnchorElement) {
      links.push(this.download)
    })
  const view = await render(<RuntimeStatusSettings />)
  expect(readDesktopDiagnosticVersions).not.toHaveBeenCalled()
  await user.click(view.getByRole('button', { name: '生成诊断预览' }))
  await expect
    .element(view.getByRole('region', { name: '诊断预览' }))
    .toBeVisible()
  expect(controlManagedRuntime).not.toHaveBeenCalled()
  expect(create).not.toHaveBeenCalled()
  expect(click).not.toHaveBeenCalled()
  await user.click(view.getByRole('button', { name: '下载诊断 JSON' }))
  expect(readDesktopDiagnosticVersions).toHaveBeenCalledTimes(1)
  expect(controlManagedRuntime).not.toHaveBeenCalled()
  expect(links).toEqual(['flowtools-runtime-status.json'])
  expect(revoke).toHaveBeenCalledExactlyOnceWith('blob:fixture-diagnostic')
  const report: unknown = JSON.parse(await blobs[0]!.text())
  expect(report).toMatchObject({
    observation: { state: 'not-queried' },
    desktop: { appVersion: '0.1.0', tauriVersion: '2.11.0' },
  })
  await expect.element(view.getByText(/已请求下载/)).toBeVisible()
})

it('a new failed query invalidates the old report and exports only its stable failure code', async () => {
  const view = await render(<RuntimeStatusSettings />)
  await user.click(view.getByRole('button', { name: '查询 Runtime 状态' }))
  await expect.element(view.getByText('fixture-status-host')).toBeVisible()
  await user.click(view.getByRole('button', { name: '生成诊断预览' }))
  const preview = view.getByRole('region', { name: '诊断预览' })
  await expect.element(preview).toHaveTextContent('fixture-status-host')
  vi.mocked(controlManagedRuntime).mockRejectedValueOnce(
    new RuntimeClientError('RUNTIME_DISCONNECTED')
  )
  await user.click(view.getByRole('button', { name: '查询 Runtime 状态' }))
  await expect.element(preview).not.toBeInTheDocument()
  await user.click(view.getByRole('button', { name: '生成诊断预览' }))
  await expect.element(preview).toHaveTextContent('RUNTIME_DISCONNECTED')
  await expect.element(preview).not.toHaveTextContent('fixture-status-host')
})

it('metadata failure never echoes private text or prevents another Runtime query', async () => {
  vi.mocked(readDesktopDiagnosticVersions).mockRejectedValueOnce(
    new Error('C:\\PRIVATE_VERSION_SECRET')
  )
  const view = await render(<RuntimeStatusSettings />)
  await user.click(view.getByRole('button', { name: '生成诊断预览' }))
  await expect.element(view.getByText(/无法生成诊断预览/)).toBeVisible()
  await expect
    .element(view.getByRole('region', { name: 'Runtime 状态' }))
    .not.toHaveTextContent('PRIVATE_')
  await expect
    .element(view.getByRole('button', { name: '生成诊断预览' }))
    .toBeEnabled()
  await user.click(view.getByRole('button', { name: '查询 Runtime 状态' }))
  await expect.element(view.getByText('fixture-status-host')).toBeVisible()
})

it('a query invalidates in-flight metadata without blocking the query or reviving its old report', async () => {
  let complete: (version: {
    appVersion: string
    tauriVersion: string
  }) => void = () => {}
  vi.mocked(readDesktopDiagnosticVersions).mockImplementationOnce(
    () =>
      new Promise(resolve => {
        complete = resolve
      })
  )
  const view = await render(<RuntimeStatusSettings />)
  await user.click(view.getByRole('button', { name: '生成诊断预览' }))
  await expect
    .element(view.getByRole('button', { name: '生成诊断预览' }))
    .toBeDisabled()
  await user.click(view.getByRole('button', { name: '查询 Runtime 状态' }))
  await expect.element(view.getByText('fixture-status-host')).toBeVisible()
  complete({ appVersion: '0.1.0', tauriVersion: '2.11.0' })
  await expect
    .element(view.getByRole('region', { name: '诊断预览' }))
    .not.toBeInTheDocument()
  await user.click(view.getByRole('button', { name: '生成诊断预览' }))
  await expect
    .element(view.getByRole('region', { name: '诊断预览' }))
    .toHaveTextContent('fixture-status-host')
})

it('releases the download URL after a dispatch failure and retains a usable preview', async () => {
  vi.spyOn(URL, 'createObjectURL').mockReturnValue(
    'blob:fixture-failed-diagnostic'
  )
  const revoke = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {})
  const click = vi
    .spyOn(HTMLAnchorElement.prototype, 'click')
    .mockImplementation(() => {
      throw new Error('PRIVATE_DOWNLOAD_SECRET')
    })
  const view = await render(<RuntimeStatusSettings />)
  await user.click(view.getByRole('button', { name: '生成诊断预览' }))
  await user.click(view.getByRole('button', { name: '下载诊断 JSON' }))
  expect(click).toHaveBeenCalledTimes(1)
  expect(revoke).toHaveBeenCalledExactlyOnceWith(
    'blob:fixture-failed-diagnostic'
  )
  await expect.element(view.getByText(/无法确认下载/)).toBeVisible()
  await expect
    .element(view.getByRole('region', { name: 'Runtime 状态' }))
    .not.toHaveTextContent('PRIVATE_')
  await expect
    .element(view.getByRole('button', { name: '下载诊断 JSON' }))
    .toBeEnabled()
})

it('browser preview cannot read native diagnostic metadata or create a report', async () => {
  environment.native = false
  const view = await render(<RuntimeStatusSettings />)
  await expect
    .element(view.getByRole('button', { name: '生成诊断预览' }))
    .toBeDisabled()
  expect(readDesktopDiagnosticVersions).not.toHaveBeenCalled()
})

it('oversized native metadata is refused before a report or download is created', async () => {
  vi.mocked(readDesktopDiagnosticVersions).mockResolvedValueOnce({
    appVersion: '0.1.0',
    tauriVersion: '2.11.0+' + 'PRIVATE_'.repeat(100),
  })
  const view = await render(<RuntimeStatusSettings />)
  await user.click(view.getByRole('button', { name: '生成诊断预览' }))
  await expect.element(view.getByText(/无法生成诊断预览/)).toBeVisible()
  await expect
    .element(view.getByRole('region', { name: '诊断预览' }))
    .not.toBeInTheDocument()
  await expect
    .element(view.getByRole('region', { name: 'Runtime 状态' }))
    .not.toHaveTextContent('PRIVATE_')
})

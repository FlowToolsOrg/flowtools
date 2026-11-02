import type { ToolInventoryPage } from '@flowtools/runtime-client'

import { RuntimeClientError } from '@flowtools/runtime-client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { controlManagedRuntime } from '../../../../desktop/src/runtime/managed-client'
import { ToolSettings } from '../../../../desktop/src/settings/tool-settings'

const environment = vi.hoisted(() => ({ native: true }))
const user = userEvent.setup()
vi.mock('../../../../desktop/src/runtime/desktop-capabilities', () => ({
  isTauriRuntime: () => environment.native,
}))
vi.mock('../../../../desktop/src/runtime/managed-client', () => ({
  controlManagedRuntime: vi.fn(),
}))
const empty: ToolInventoryPage = {
  availability: 'not-configured',
  snapshot: null,
  total: 0,
  nextOffset: null,
  storage: null,
  items: [],
}
const observed: ToolInventoryPage = {
  availability: 'observed',
  snapshot: 'a'.repeat(64),
  total: 9,
  nextOffset: 8,
  storage: { bytes: 1048576, groups: 9, maxBytes: 268435456, maxGroups: 384 },
  items: Array.from({ length: 8 }, (_, n) => ({
    publisher: 'signed-publisher',
    id: `ffmpeg-${n}`,
    version: '1.0.0',
    buildFlavor: 'static',
    archiveDigest: n.toString(16).padStart(64, '0'),
    sourceRegistry: 'registry',
    sourceArtifact: 'ffmpeg',
    archiveBytes: 10,
    materialized: true,
    dependents: [
      { publisher: 'signed-publisher', id: 'converter', version: '1.0.0' },
    ],
    retainedJobs: 1,
    deletionBlockers: [
      'deletion-not-implemented',
      'installed-dependent',
      'retained-task',
    ],
  })),
}
beforeEach(() => {
  environment.native = true
  vi.clearAllMocks()
  vi.mocked(controlManagedRuntime).mockResolvedValue({
    type: 'tool-inventory',
    data: empty,
  })
})
afterEach(() => vi.restoreAllMocks())

it('does not query on mount; keyboard query explicitly reports absent tool configuration', async () => {
  const view = await render(<ToolSettings />)
  expect(controlManagedRuntime).not.toHaveBeenCalled()
  view.getByRole('button', { name: '重新查询工具' }).element().focus()
  await user.keyboard('{Enter}')
  await expect
    .element(view.getByRole('status'))
    .toHaveTextContent('尚未配置受管工具')
  expect(controlManagedRuntime).toHaveBeenCalledExactlyOnceWith(
    { method: 'tools.inventory', payload: { offset: 0, snapshot: null } },
    false
  )
  await expect.element(view.getByText(/共享安装仓库/)).not.toBeInTheDocument()
})

it('shows source, shared space, dependents and blockers; changed pagination clears stale observations', async () => {
  vi.mocked(controlManagedRuntime).mockResolvedValueOnce({
    type: 'tool-inventory',
    data: observed,
  })
  const view = await render(<ToolSettings />)
  await user.click(view.getByRole('button', { name: '重新查询工具' }))
  await expect
    .element(view.getByText(/共享安装仓库/))
    .toHaveTextContent('1.00 MiB')
  await expect
    .element(view.getByRole('heading', { name: 'ffmpeg-0 · 1.0.0' }))
    .toBeVisible()
  await expect
    .element(view.getByRole('region', { name: '受管工具' }))
    .toHaveTextContent('signed-publisher/converter@1.0.0')
  await expect
    .element(view.getByRole('region', { name: '受管工具' }))
    .toHaveTextContent('任务仍保留此版本')
  vi.mocked(controlManagedRuntime).mockRejectedValueOnce(
    new RuntimeClientError('REVISION_CONFLICT')
  )
  await user.click(view.getByRole('button', { name: '下一页' }))
  expect(controlManagedRuntime).toHaveBeenLastCalledWith(
    {
      method: 'tools.inventory',
      payload: { offset: 8, snapshot: observed.snapshot },
    },
    false
  )
  await expect
    .element(view.getByRole('heading', { name: 'ffmpeg-0 · 1.0.0' }))
    .not.toBeInTheDocument()
  await expect
    .element(view.getByRole('status'))
    .toHaveTextContent('REVISION_CONFLICT')
})

it('rejects malformed native pages and reports offline errors without retry or stale rows', async () => {
  vi.mocked(controlManagedRuntime).mockResolvedValueOnce({
    type: 'tool-inventory',
    data: { ...observed, total: 1 },
  })
  const view = await render(<ToolSettings />)
  const query = view.getByRole('button', { name: '重新查询工具' })
  await user.click(query)
  await expect.element(view.getByRole('status')).toHaveTextContent('无法核对')
  vi.mocked(controlManagedRuntime).mockRejectedValueOnce(
    new RuntimeClientError('RUNTIME_DISCONNECTED')
  )
  await user.click(query)
  await expect
    .element(view.getByRole('status'))
    .toHaveTextContent('RUNTIME_DISCONNECTED')
  expect(controlManagedRuntime).toHaveBeenCalledTimes(2)
})

it('keeps web preview inert and handles an unmounted native query', async () => {
  environment.native = false
  const web = await render(<ToolSettings />)
  await expect
    .element(web.getByRole('button', { name: '重新查询工具' }))
    .toBeDisabled()
  expect(controlManagedRuntime).not.toHaveBeenCalled()
  await web.unmount()
  environment.native = true
  let resolve: (
    value: Awaited<ReturnType<typeof controlManagedRuntime>>
  ) => void = () => {}
  vi.mocked(controlManagedRuntime).mockImplementationOnce(
    () =>
      new Promise(done => {
        resolve = done
      })
  )
  const native = await render(<ToolSettings />)
  await user.click(native.getByRole('button', { name: '重新查询工具' }))
  await expect
    .element(native.getByRole('button', { name: '正在查询…' }))
    .toBeDisabled()
  await native.unmount()
  resolve({ type: 'tool-inventory', data: empty })
  expect(controlManagedRuntime).toHaveBeenCalledTimes(1)
})

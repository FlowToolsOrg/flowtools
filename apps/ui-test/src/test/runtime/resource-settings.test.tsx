import type {
  Call,
  Outcome,
  ResourceInfo,
  ResourceOwner,
  JobSnapshot,
} from '@flowtools/runtime-client'

import { expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { ResourceSettings } from '../../../../desktop/src/settings/resource-settings'
const id = '00000000-0000-4000-8000-000000000001',
  hash = 'a'.repeat(64)
const owner: ResourceOwner = {
  id,
  identity: {
    caller: 'local-desktop',
    publisher: 'flowtools',
    pluginId: 'plugin-resource-reader',
    version: '0.0.1',
    packageDigest: hash,
    commandId: 'run',
  },
  expiresAt: Date.now() + 300000,
  scopes: [{ slot: 'source', operations: ['read', 'stat'] }],
}
const resource: ResourceInfo = {
  ref: 'private-ref',
  kind: 'file',
  name: '测试文件.txt',
  size: 3,
  revision: 'private-revision',
  rights: ['read', 'stat'],
}
const terminal: JobSnapshot = {
  formatVersion: 1,
  runId: id,
  parentRunId: null,
  rootCaller: 'local-desktop',
  pluginId: 'plugin-resource-reader',
  commandId: 'run',
  packageVersion: '0.0.1',
  packageDigest: hash,
  dependencyLock: hash,
  deadline: Date.now() + 30000,
  grantEpoch: 1,
  resources: {},
  state: 'succeeded',
  sequence: 3,
  result: null,
}
it('resource picker cancellation leaves run disabled; explicit selection binds only on run and releases on unmount', async () => {
  const select = vi
    .fn()
    .mockResolvedValue({ owner, resource })
    .mockResolvedValueOnce(null)
  const call = vi.fn(async (request: Call): Promise<Outcome> => {
    if (request.method === 'resources.submit')
      return {
        type: 'receipt',
        data: {
          formatVersion: 1,
          receiptType: 'job',
          runId: id,
          instanceId: id,
          acceptedAt: Date.now(),
        },
      }
    if (request.method === 'jobs.status') return { type: 'job', data: terminal }
    throw new Error('INVALID_REQUEST')
  })
  const close = vi.fn(),
    control = vi.fn(
      async (): Promise<Outcome> => ({ type: 'resource-released' })
    ),
    connect = vi.fn(async () => ({ call, close }))
  const view = await render(
      <ResourceSettings select={select} connect={connect} control={control} />
    ),
    user = userEvent.setup()
  await user.click(view.getByRole('button', { name: '选择文件并确认权限' }))
  await expect
    .element(view.getByRole('button', { name: '运行读取' }))
    .toBeDisabled()
  expect(connect).not.toHaveBeenCalled()
  await user.click(view.getByRole('button', { name: '选择文件并确认权限' }))
  await expect.element(view.getByText(/已选择：测试文件.txt/)).toBeVisible()
  expect(call).not.toHaveBeenCalled()
  await user.click(view.getByRole('button', { name: '运行读取' }))
  await expect.element(view.getByRole('status')).toHaveTextContent('读取完成')
  expect(call.mock.calls[0]?.[0]).toMatchObject({
    method: 'resources.submit',
    payload: {
      owner: id,
      submit: {
        pluginId: 'plugin-resource-reader',
        commandId: 'run',
        input: {},
        background: false,
      },
    },
  })
  expect(JSON.stringify(call.mock.calls)).not.toContain(resource.name)
  expect(close).toHaveBeenCalled()
  await user.click(view.getByRole('button', { name: '选择目录并确认权限' }))
  await view.unmount()
  expect(control).toHaveBeenCalledWith({
    method: 'resources.drop',
    payload: { owner: id },
  })
})

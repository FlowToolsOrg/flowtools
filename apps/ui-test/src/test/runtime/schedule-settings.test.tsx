import type { Call, ScheduleRecord } from '@flowtools/runtime-client'

import { RuntimeClientError } from '@flowtools/runtime-client'
import { beforeEach, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { controlManagedRuntime } from '../../../../desktop/src/runtime/managed-client'
import { ScheduleSettings } from '../../../../desktop/src/settings/schedule-settings'

const environment = vi.hoisted(() => ({ native: true }))
vi.mock('../../../../desktop/src/runtime/desktop-capabilities', () => ({
  isTauriRuntime: () => environment.native,
}))
vi.mock('../../../../desktop/src/runtime/managed-client', () => ({
  controlManagedRuntime: vi.fn(),
}))
const user = userEvent.setup()
let records: ScheduleRecord[]
beforeEach(() => {
  records = []
  environment.native = true
  vi.clearAllMocks()
  vi.mocked(controlManagedRuntime).mockImplementation(async (call: Call) => {
    if (call.method === 'schedules.resolve-time')
      return {
        type: 'schedule-time',
        data: {
          dueMs: Date.parse('2026-10-28T01:00:00Z'),
          utc: '2026-10-28T01:00:00Z',
          zoned: '2026-10-28T09:00:00+08:00[Asia/Shanghai]',
          timeZone: 'Asia/Shanghai',
        },
      }
    if (call.method === 'schedules.create') {
      const create = call.payload
      const record: ScheduleRecord = {
        id: '00000000-0000-4000-8000-000000000001',
        revision: 1,
        definition: create.definition,
        binding: {
          identity: {
            caller: 'local-desktop',
            publisher: 'flowtools',
            pluginId: create.pluginId,
            commandId: create.commandId,
            packageDigest: 'a'.repeat(64),
            version: '0.1.0',
          },
          inputDigest: 'b'.repeat(64),
          grantEpoch: 1,
          timeoutMs: create.timeoutMs,
        },
        nextIndex: 0,
        lastObservedMs: null,
        cancelled: false,
        skipped: 0,
        lastClaim: null,
      }
      records = [record]
      return { type: 'schedule', data: record }
    }
    if (call.method === 'schedules.cancel') {
      records = records.map(record => ({
        ...record,
        revision: 2,
        cancelled: true,
      }))
      return { type: 'schedule', data: records[0]! }
    }
    if (call.method === 'schedules.list')
      return { type: 'schedules', data: records }
    throw new Error('unexpected operation')
  })
})
it('mount does not query or start a Host and browser controls remain disabled', async () => {
  environment.native = false
  const view = await render(<ScheduleSettings />)
  expect(controlManagedRuntime).not.toHaveBeenCalled()
  await expect
    .element(view.getByRole('button', { name: '查询调度' }))
    .toBeDisabled()
  await expect
    .element(view.getByRole('button', { name: '创建调度并确认' }))
    .toBeDisabled()
})
it('explicit preview creates a structured finite schedule and cancel uses its revision', async () => {
  const view = await render(<ScheduleSettings />)
  await user.fill(
    view.getByRole('textbox', { name: '首次触发当地时间' }),
    '2026-10-28T09:00'
  )
  await user.fill(
    view.getByRole('textbox', { name: 'IANA 时区' }),
    'Asia/Shanghai'
  )
  await user.fill(
    view.getByRole('textbox', { name: '文本', exact: true }),
    'private input'
  )
  await user.click(view.getByRole('button', { name: '预览触发时间' }))
  await expect.element(view.getByText(/UTC 2026-10-28/)).toBeVisible()
  await user.click(view.getByRole('button', { name: '创建调度并确认' }))
  await expect.element(view.getByRole('status')).toHaveTextContent('调度已创建')
  const create = vi
    .mocked(controlManagedRuntime)
    .mock.calls.find(([call]) => call.method === 'schedules.create')?.[0]
  expect(create).toMatchObject({
    method: 'schedules.create',
    payload: {
      target: 'desktop',
      input: { text: 'private input', mode: 'encode' },
      definition: { occurrences: 1, intervalMs: null, missed: 'skip' },
    },
  })
  await user.click(view.getByRole('button', { name: '取消此调度' }))
  expect(controlManagedRuntime).toHaveBeenCalledWith({
    method: 'schedules.cancel',
    payload: { id: records[0]!.id, revision: 1 },
  })
  await expect
    .element(view.getByRole('button', { name: '取消此调度' }))
    .toBeDisabled()
})
it('DST refusal prevents creation and editing a resolved time invalidates its preview', async () => {
  const view = await render(<ScheduleSettings />)
  await user.fill(
    view.getByRole('textbox', { name: '首次触发当地时间' }),
    '2026-11-01T01:30'
  )
  vi.mocked(controlManagedRuntime).mockRejectedValueOnce(
    new RuntimeClientError('INPUT_INVALID')
  )
  await user.click(view.getByRole('button', { name: '预览触发时间' }))
  await expect
    .element(view.getByRole('status'))
    .toHaveTextContent('INPUT_INVALID')
  await expect
    .element(view.getByRole('button', { name: '创建调度并确认' }))
    .toBeDisabled()
  await user.click(view.getByRole('button', { name: '预览触发时间' }))
  await user.fill(
    view.getByRole('textbox', { name: '首次触发当地时间' }),
    '2026-11-01T02:30'
  )
  await expect
    .element(view.getByRole('button', { name: '创建调度并确认' }))
    .toBeDisabled()
})

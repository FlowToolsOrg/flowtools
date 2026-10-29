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
  connectManagedRuntime: vi.fn(),
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

it('saved policies stay specific to each record and editing preserves its grace', async () => {
  const view = await render(<ScheduleSettings />)
  await user.fill(
    view.getByRole('textbox', { name: '首次触发当地时间' }),
    '2026-10-28T09:00'
  )
  await user.click(view.getByRole('button', { name: '预览触发时间' }))
  await user.click(view.getByRole('button', { name: '创建调度并确认' }))
  const first = records[0]!
  records = [
    { ...first, definition: { ...first.definition, graceMs: 1250 } },
    {
      ...first,
      id: '00000000-0000-4000-8000-000000000002',
      definition: {
        ...first.definition,
        intervalMs: 90_000,
        occurrences: 3,
        graceMs: 60_000,
        missed: 'coalesce-latest',
      },
    },
  ]
  await user.click(view.getByRole('button', { name: '查询调度' }))
  await expect
    .element(view.getByText(/一次性触发 · 休眠或暂停后，迟于 1.25 秒/))
    .toBeVisible()
  await expect.element(view.getByText(/每 90 秒触发，共 3 次/)).toBeVisible()
  await expect.element(view.getByText(/只补最近一次，其余跳过/)).toBeVisible()
  await user.selectOptions(
    view.getByRole('combobox', { name: '漏触发策略' }),
    'coalesce-latest'
  )
  await user.fill(view.getByRole('spinbutton', { name: '触发宽限秒数' }), '0')
  await expect
    .element(view.getByText(/一次性触发 · 休眠或暂停后，迟于 1.25 秒/))
    .toBeVisible()
  await user.click(view.getByRole('button', { name: '编辑此调度' }).first())
  await expect
    .element(view.getByRole('spinbutton', { name: '触发宽限秒数' }))
    .toHaveValue(1.25)
  await user.click(view.getByRole('button', { name: '预览触发时间' }))
  vi.mocked(controlManagedRuntime).mockImplementationOnce(async call => {
    expect(call).toMatchObject({
      method: 'schedules.replace',
      payload: { create: { definition: { graceMs: 1250, missed: 'skip' } } },
    })
    return { type: 'schedule', data: first }
  })
  await user.click(view.getByRole('button', { name: '保存修改并确认' }))
})

it('editing requires a new preview and sends the old revision with freshly entered input', async () => {
  const view = await render(<ScheduleSettings />)
  await user.fill(
    view.getByRole('textbox', { name: '首次触发当地时间' }),
    '2026-10-28T09:00'
  )
  await user.click(view.getByRole('button', { name: '预览触发时间' }))
  await user.click(view.getByRole('button', { name: '创建调度并确认' }))
  const old = records[0]!
  await user.click(view.getByRole('button', { name: '编辑此调度' }))
  await expect
    .element(view.getByRole('button', { name: '保存修改并确认' }))
    .toBeDisabled()
  await user.fill(
    view.getByRole('textbox', { name: '文本', exact: true }),
    'changed content'
  )
  await user.click(view.getByRole('button', { name: '预览触发时间' }))
  vi.mocked(controlManagedRuntime).mockImplementationOnce(async call => {
    expect(call).toMatchObject({
      method: 'schedules.replace',
      payload: {
        previous: { id: old.id, revision: 1 },
        create: { input: { text: 'changed content', mode: 'encode' } },
      },
    })
    return {
      type: 'schedule',
      data: { ...old, id: '00000000-0000-4000-8000-000000000002' },
    }
  })
  await user.click(view.getByRole('button', { name: '保存修改并确认' }))
  await expect
    .element(view.getByRole('button', { name: '创建调度并确认' }))
    .toBeDisabled()
})

it('restore disposal is explicit and binds the reviewed challenge', async () => {
  const view = await render(<ScheduleSettings />)
  vi.mocked(controlManagedRuntime).mockResolvedValueOnce({
    type: 'schedule-recovery',
    data: { challenge: 'c'.repeat(64), count: 2 },
  })
  await user.click(view.getByRole('button', { name: '检查恢复状态' }))
  expect(controlManagedRuntime).toHaveBeenCalledTimes(1)
  await expect.element(view.getByText(/全部 2 条旧调度/)).toBeVisible()
  vi.mocked(controlManagedRuntime).mockResolvedValueOnce({
    type: 'schedules',
    data: [],
  })
  await user.click(view.getByRole('button', { name: '作废旧调度并确认' }))
  expect(controlManagedRuntime).toHaveBeenCalledWith({
    method: 'schedules.discard-restored',
    payload: { challenge: 'c'.repeat(64) },
  })
})

it('reminders use title and content fields without terminal input', async () => {
  const view = await render(<ScheduleSettings />)
  await user.selectOptions(
    view.getByRole('combobox', { name: '调度命令' }),
    'plugin-local-reminder:run'
  )
  await user.fill(view.getByRole('textbox', { name: '提醒标题' }), '每日整理')
  await user.fill(view.getByRole('textbox', { name: '提醒内容' }), '核对待办')
  await user.fill(
    view.getByRole('textbox', { name: '首次触发当地时间' }),
    '2026-10-28T09:00'
  )
  await user.click(view.getByRole('button', { name: '预览触发时间' }))
  await user.click(view.getByRole('button', { name: '创建调度并确认' }))
  const create = vi
    .mocked(controlManagedRuntime)
    .mock.calls.find(([call]) => call.method === 'schedules.create')?.[0]
  expect(create).toMatchObject({
    method: 'schedules.create',
    payload: {
      pluginId: 'plugin-local-reminder',
      input: { title: '每日整理', message: '核对待办' },
    },
  })
})

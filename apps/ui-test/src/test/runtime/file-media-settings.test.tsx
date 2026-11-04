import type {
  Call,
  FilePlanView,
  MediaPlanPreview,
  Outcome,
  RuntimeClient,
  JobSnapshot,
} from '@flowtools/runtime-client'

import { expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import { FileSettings } from '../../../../desktop/src/settings/file-settings'
import { MediaSettings } from '../../../../desktop/src/settings/media-settings'

const id = '00000000-0000-4000-8000-000000000001'
const hash = 'a'.repeat(64)
const user = userEvent.setup()
const plan: FilePlanView = {
  record: {
    id,
    revision: 1,
    digest: hash,
    files: 1,
    skipped: 0,
    totalBytes: 3,
    cancelled: false,
  },
  preview: {
    digest: hash,
    files: [
      {
        source: '学习笔记.txt',
        target: '整理_学习笔记.txt',
        bytes: 3,
        sha256: hash,
      },
    ],
    skipped: 0,
    totalBytes: 3,
  },
  execution: null,
}
it('file selection previews without writes then confirms the same plan and undoes it', async () => {
  let current = structuredClone(plan)
  const control = vi.fn(async (call: Call): Promise<Outcome> => {
    if (call.method === 'files.recovery')
      return { type: 'file-recovery', data: null }
    if (call.method === 'files.plan' || call.method === 'files.preview')
      return { type: 'file-plan', data: structuredClone(current) }
    if (call.method === 'files.list')
      return { type: 'file-plans', data: [current.record] }
    if (call.method === 'files.commit' || call.method === 'files.undo') {
      current = {
        ...current,
        execution: {
          planId: id,
          phase: call.method === 'files.commit' ? 'complete' : 'undone',
          completed: call.method === 'files.commit' ? 1 : 0,
          pending: false,
        },
      }
      return { type: 'file-execution', data: current.execution! }
    }
    throw new Error('INVALID_REQUEST')
  })
  const view = await render(
    <FileSettings
      control={control}
      selectDirectory={async () => 'D:/selected'}
    />
  )
  await user.click(view.getByRole('button', { name: '选择目录并预览' }))
  await expect
    .element(view.getByRole('cell', { name: '整理_学习笔记.txt', exact: true }))
    .toBeVisible()
  expect(control.mock.calls.some(([c]) => c.method === 'files.commit')).toBe(
    false
  )
  await user.click(view.getByRole('button', { name: '确认提交 / 重试' }))
  await expect.element(view.getByRole('status')).toHaveTextContent('提交完成')
  expect(control).toHaveBeenCalledWith({
    method: 'files.commit',
    payload: { id, revision: 1, digest: hash },
  })
  await user.click(view.getByRole('button', { name: '撤销未修改文件' }))
  await expect.element(view.getByRole('status')).toHaveTextContent('撤销完成')
  expect(control).toHaveBeenCalledWith({
    method: 'files.undo',
    payload: { id, revision: 1, digest: hash },
  })
})
it('cancelled directory selection performs no Runtime operation and conflicts stay visible', async () => {
  const control = vi.fn(async (): Promise<Outcome> => {
    throw new Error('REVISION_CONFLICT')
  })
  const select = vi.fn(async () => null as string | null)
  const view = await render(
    <FileSettings control={control} selectDirectory={select} />
  )
  await user.click(view.getByRole('button', { name: '选择目录并预览' }))
  expect(control).not.toHaveBeenCalled()
  select.mockResolvedValue('D:/selected')
  await user.click(view.getByRole('button', { name: '选择目录并预览' }))
  await expect.element(view.getByRole('alert')).toBeVisible()
})
const media: MediaPlanPreview = {
  id,
  digest: hash,
  action: 'audio',
  inputName: 'input.wav',
  inputBytes: 32044,
  inputSha256: hash,
  outputName: 'output.wav',
}
const job: JobSnapshot = {
  formatVersion: 1,
  runId: id,
  parentRunId: null,
  rootCaller: 'local-desktop',
  pluginId: 'plugin-media-converter',
  commandId: 'run',
  packageVersion: '0.0.1',
  packageDigest: hash,
  dependencyLock: hash,
  deadline: 1,
  grantEpoch: 1,
  resources: {},
  state: 'succeeded',
  sequence: 3,
  result: {
    formatVersion: 1,
    runId: id,
    pluginId: 'plugin-media-converter',
    pluginVersion: '0.0.1',
    startedAt: 1,
    finishedAt: 2,
    durationMs: 1,
    inputSummary: { kind: 'object', size: 2 },
    success: true,
    data: {
      type: 'json',
      value: {
        result: {
          formats: ['wav'],
          durationMs: 1000,
          sourceBytes: 32044,
          streams: [
            {
              kind: 'audio',
              codec: 'pcm_s16le',
              width: null,
              height: null,
              sampleRate: 16000,
              channels: 1,
            },
          ],
          published: true,
          outputBytes: 32078,
          outputSha256: hash,
        },
      },
    },
  },
}
it('media preview waits for explicit consent and renders the observed completed result', async () => {
  const control = vi.fn(async (c: Call): Promise<Outcome> => {
    if (c.method === 'media.plan') return { type: 'media-plan', data: media }
    if (c.method === 'media.start')
      return {
        type: 'receipt',
        data: {
          formatVersion: 1,
          receiptType: 'job',
          runId: id,
          instanceId: id,
          acceptedAt: 1,
        },
      }
    throw new Error('INVALID_REQUEST')
  })
  const call = vi.fn(async (c: Call): Promise<Outcome> => {
    if (c.method === 'jobs.status') return { type: 'job', data: job }
    throw new Error('INVALID_REQUEST')
  })
  const close = vi.fn()
  const connect = async () =>
    ({ call, close }) as Pick<RuntimeClient, 'call' | 'close'>
  const view = await render(
    <MediaSettings
      control={control}
      connect={connect}
      selectInput={async () => 'D:/input.wav'}
      selectOutput={async () => 'D:/output.wav'}
    />
  )
  await user.click(
    view.getByRole('button', { name: '提取 WAV 音频', exact: true })
  )
  await user.click(view.getByRole('button', { name: '选择媒体并预览' }))
  await expect
    .element(view.getByText('输入：input.wav · 32044 字节'))
    .toBeVisible()
  expect(control.mock.calls.some(([c]) => c.method === 'media.start')).toBe(
    false
  )
  await user.click(view.getByRole('button', { name: '确认执行媒体计划' }))
  await expect.element(view.getByRole('status')).toHaveTextContent('已完成')
  await expect.element(view.getByText('已保存新文件：32078 字节')).toBeVisible()
  expect(control).toHaveBeenCalledWith({
    method: 'media.start',
    payload: { id, digest: hash, target: 'desktop' },
  })
  await expect.poll(() => close.mock.calls.length).toBeGreaterThan(0)
})
it('media information preserves unknown duration and cancellation uses the existing run', async () => {
  const control = vi.fn(async (c: Call): Promise<Outcome> => {
    if (c.method === 'media.plan')
      return {
        type: 'media-plan',
        data: { ...media, action: 'inspect', outputName: null },
      }
    if (c.method === 'media.start')
      return {
        type: 'receipt',
        data: {
          formatVersion: 1,
          receiptType: 'job',
          runId: id,
          instanceId: id,
          acceptedAt: 1,
        },
      }
    throw new Error('INVALID_REQUEST')
  })
  let cancelled = false
  const call = vi.fn(async (c: Call): Promise<Outcome> => {
    if (c.method === 'jobs.cancel') {
      cancelled = true
      return { type: 'job', data: { ...job, state: 'cancelled', result: null } }
    }
    if (c.method === 'jobs.status')
      return {
        type: 'job',
        data: {
          ...job,
          state: cancelled ? 'cancelled' : 'running',
          result: null,
        },
      }
    if (c.method === 'media.progress')
      return {
        type: 'media-progress',
        data: { runId: id, stage: 'inspecting' },
      }
    throw new Error('INVALID_REQUEST')
  })
  const view = await render(
    <MediaSettings
      control={control}
      connect={async () =>
        ({ call, close: () => {} }) as Pick<RuntimeClient, 'call' | 'close'>
      }
      selectInput={async () => 'D:/input.wav'}
    />
  )
  await user.click(view.getByRole('button', { name: '选择媒体并预览' }))
  await user.click(view.getByRole('button', { name: '确认执行媒体计划' }))
  await expect
    .element(view.getByRole('status'))
    .toHaveTextContent('读取媒体信息')
  await user.click(view.getByRole('button', { name: '取消媒体任务' }))
  await expect.element(view.getByRole('status')).toHaveTextContent('已取消')
  expect(call).toHaveBeenCalledWith({
    method: 'jobs.cancel',
    payload: { runId: id },
  })
})

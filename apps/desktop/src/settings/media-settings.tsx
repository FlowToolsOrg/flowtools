import type {
  Call,
  Outcome,
  JobSnapshot,
  MediaPlanPreview,
  RuntimeClient,
  UserMediaAction,
} from '@flowtools/runtime-client'

import { useEffect, useState } from 'react'

import { describeRuntimeError } from '@flowtools/runtime-client'

import { Button, Card, Input, Label, TextField } from '@heroui/react'

import {
  connectManagedRuntime,
  controlManagedRuntime,
} from '../runtime/managed-client'

type Client = Pick<RuntimeClient, 'call' | 'close'>
const terminal = (job: JobSnapshot | null) =>
  !!job &&
  ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(job.state)
const stages: Record<string, string> = {
  queued: '排队中',
  inspecting: '读取媒体信息',
  converting: '正在转换',
  validating: '校验输出',
  publishing: '保存结果',
  succeeded: '已完成',
  failed: '失败',
  cancelled: '已取消',
  interrupted: '已中断',
  running: '执行中',
  cancelling: '正在取消',
}
export function MediaSettings({
  control = controlManagedRuntime,
  connect = connectManagedRuntime,
  selectInput = chooseInput,
  selectOutput = chooseOutput,
}: {
  control?: (call: Call) => Promise<Outcome>
  connect?: () => Promise<Client>
  selectInput?: () => Promise<string | null>
  selectOutput?: (action: UserMediaAction) => Promise<string | null>
}) {
  const [action, setAction] = useState<UserMediaAction>('inspect')
  const [plan, setPlan] = useState<MediaPlanPreview | null>(null)
  const [planId, setPlanId] = useState('')
  const [runId, setRunId] = useState('')
  const [job, setJob] = useState<JobSnapshot | null>(null)
  const [jobs, setJobs] = useState<JobSnapshot[]>([])
  const [stage, setStage] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [watch, setWatch] = useState(0)
  const running = !!runId && !terminal(job)
  const perform = async (fn: () => Promise<void>) => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      await fn()
    } catch (e) {
      setError(explain(e))
    } finally {
      setBusy(false)
    }
  }
  useEffect(() => {
    if (!runId) return
    let active = true,
      client: Client | undefined
    const observe = async () => {
      try {
        client = await connect()
        while (active) {
          const result = await client.call({
            method: 'jobs.status',
            payload: { runId },
          })
          if (result.type !== 'job') throw new Error('INVALID_RESPONSE')
          if (!active) break
          setJob(result.data)
          if (terminal(result.data)) {
            setStage(result.data.state)
            break
          }
          const progress = await client.call({
            method: 'media.progress',
            payload: { runId },
          })
          if (progress.type !== 'media-progress')
            throw new Error('INVALID_RESPONSE')
          if (active) setStage(progress.data.stage)
          await new Promise(resolve => setTimeout(resolve, 200))
        }
      } catch (e) {
        if (active) setError(explain(e))
      } finally {
        client?.close()
      }
    }
    void observe()
    return () => {
      active = false
      client?.close()
    }
  }, [runId, connect, watch])
  const summary = readSummary(job?.result?.data)
  return (
    <Card>
      <Card.Header>
        <Card.Title>媒体处理</Card.Title>
        <Card.Description>
          查看信息、提取 WAV 音频或转换为 WebM。Windows 原型每次支持 1 MiB
          输入与输出；使用内置工具，结果须保存为新文件。
        </Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <div className="flex flex-wrap gap-2">
          {(['inspect', 'audio', 'webm'] as const).map(mode => (
            <Button
              key={mode}
              variant={action === mode ? 'primary' : 'secondary'}
              aria-pressed={action === mode}
              isDisabled={busy || running}
              onPress={() => {
                setAction(mode)
                setPlan(null)
              }}
            >
              {
                {
                  inspect: '查看媒体信息',
                  audio: '提取 WAV 音频',
                  webm: '转为 WebM',
                }[mode]
              }
            </Button>
          ))}
        </div>
        <Button
          isDisabled={busy || running}
          onPress={() =>
            void perform(async () => {
              const inputPath = await selectInput()
              if (!inputPath) return
              const outputPath =
                action === 'inspect' ? null : await selectOutput(action)
              if (action !== 'inspect' && !outputPath) return
              const result = await control({
                method: 'media.plan',
                payload: {
                  requestKey: crypto.randomUUID(),
                  inputPath,
                  outputPath,
                  action,
                },
              })
              if (result.type !== 'media-plan')
                throw new Error('INVALID_RESPONSE')
              setPlan(result.data)
              setPlanId(result.data.id)
              setJob(null)
              setRunId('')
              setStage('')
            })
          }
        >
          选择媒体并预览
        </Button>
        <TextField
          value={planId}
          onChange={setPlanId}
          isDisabled={busy || running}
        >
          <Label>已有媒体计划 ID（可从 CLI 打开）</Label>
          <Input />
        </TextField>
        <Button
          variant="secondary"
          isDisabled={busy || running || !planId}
          onPress={() =>
            void perform(async () => {
              const result = await control({
                method: 'media.preview',
                payload: { id: planId, revision: 1 },
              })
              if (result.type !== 'media-plan')
                throw new Error('INVALID_RESPONSE')
              setPlan(result.data)
              setAction(result.data.action)
            })
          }
        >
          查看媒体计划
        </Button>
        {plan && (
          <section aria-label="媒体计划预览" className="flex flex-col gap-2">
            <p>
              输入：{plan.inputName} · {plan.inputBytes} 字节
            </p>
            <p>输出：{plan.outputName ?? '仅查看信息'}</p>
            <p className="break-all">计划 ID：{plan.id}</p>
            <p className="break-all">内容 SHA-256：{plan.inputSha256}</p>
            <Button
              isDisabled={busy || running}
              onPress={() =>
                void perform(async () => {
                  const result = await control({
                    method: 'media.start',
                    payload: {
                      id: plan.id,
                      digest: plan.digest,
                      target: 'desktop',
                    },
                  })
                  if (result.type !== 'receipt')
                    throw new Error('INVALID_RESPONSE')
                  setJob(null)
                  setStage('queued')
                  setRunId(result.data.runId)
                  setWatch(v => v + 1)
                })
              }
            >
              确认执行媒体计划
            </Button>
          </section>
        )}
        <p role="status" aria-live="polite">
          {busy ? '正在处理…' : (stages[stage] ?? stage)}
        </p>
        {error && <p role="alert">{error}</p>}
        {runId && (
          <div className="flex flex-wrap gap-2">
            <p className="break-all">任务 ID：{runId}</p>
            <Button
              variant="secondary"
              isDisabled={busy || terminal(job)}
              onPress={() =>
                void perform(async () => {
                  const client = await connect()
                  try {
                    await client.call({
                      method: 'jobs.cancel',
                      payload: { runId },
                    })
                  } finally {
                    client.close()
                  }
                })
              }
            >
              取消媒体任务
            </Button>
            <Button
              variant="secondary"
              isDisabled={busy}
              onPress={() => setWatch(v => v + 1)}
            >
              继续查看任务
            </Button>
          </div>
        )}
        {job?.result?.error && (
          <p role="alert">
            {explain(new Error(job.result.error.code))}{' '}
            任务不会自动重放；请检查输出位置后再创建新计划。
          </p>
        )}
        {summary && (
          <section aria-label="媒体结果">
            <p>
              格式：{summary.formats.join('、')} · 时长：
              {summary.durationMs === null
                ? '未知'
                : `${summary.durationMs} 毫秒`}
            </p>
            <p>
              {summary.published
                ? `已保存新文件：${summary.outputBytes} 字节`
                : '已完成媒体信息检查'}
            </p>
            <ul>
              {summary.streams.map((s, n) => (
                <li key={n}>
                  {s.kind} · {s.codec ?? '未知编码'}
                  {s.width && s.height ? ` · ${s.width} × ${s.height}` : ''}
                  {s.sampleRate ? ` · ${s.sampleRate} Hz` : ''}
                </li>
              ))}
            </ul>
            {summary.outputSha256 && (
              <p className="break-all">输出 SHA-256：{summary.outputSha256}</p>
            )}
          </section>
        )}
        <Button
          variant="secondary"
          isDisabled={busy}
          onPress={() =>
            void perform(async () => {
              const client = await connect()
              try {
                const result = await client.call({ method: 'jobs.list' })
                if (result.type !== 'jobs') throw new Error('INVALID_RESPONSE')
                setJobs(
                  result.data.filter(
                    j =>
                      j.pluginId === 'plugin-media-inspector' ||
                      j.pluginId === 'plugin-media-converter'
                  )
                )
              } finally {
                client.close()
              }
            })
          }
        >
          刷新媒体任务记录
        </Button>
        {jobs.length > 0 && (
          <ul>
            {jobs.map(j => (
              <li key={j.runId}>
                <Button
                  variant="secondary"
                  isDisabled={busy}
                  onPress={() => {
                    setJob(null)
                    setRunId(j.runId)
                    setWatch(v => v + 1)
                  }}
                >
                  {j.runId.slice(0, 8)} · {stages[j.state] ?? j.state}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Card.Content>
    </Card>
  )
}
function record(v: unknown): v is Record<string, unknown> {
  return !!v && typeof v === 'object' && !Array.isArray(v)
}
function readSummary(value: unknown) {
  if (
    !record(value) ||
    value.type !== 'json' ||
    !record(value.value) ||
    !record(value.value.result)
  )
    return null
  const s = value.value.result
  if (
    !Array.isArray(s.formats) ||
    s.formats.some(f => typeof f !== 'string') ||
    !Array.isArray(s.streams) ||
    s.streams.some(v => !record(v) || typeof v.kind !== 'string')
  )
    return null
  return s as {
    formats: string[]
    durationMs: number | null
    published: boolean
    outputBytes: number | null
    outputSha256: string | null
    streams: Array<{
      kind: string
      codec: string | null
      width: number | null
      height: number | null
      sampleRate: number | null
    }>
  }
}
async function chooseInput() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const path = await open({
    multiple: false,
    directory: false,
    title: '选择媒体（不超过 1 MiB）',
  })
  return typeof path === 'string' ? path : null
}
async function chooseOutput(action: UserMediaAction) {
  const { save } = await import('@tauri-apps/plugin-dialog')
  return save({
    title: '选择尚不存在的输出文件',
    defaultPath: action === 'audio' ? 'output.wav' : 'output.webm',
  })
}

function explain(error: unknown) {
  const report = describeRuntimeError(error)
  return report.summary + '：' + report.action
}

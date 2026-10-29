import type {
  CreateSchedule,
  LocalTimeChoice,
  ResolvedScheduleTime,
  ScheduleRecord,
  ScheduleRecovery,
} from '@flowtools/runtime-client'

import { useEffect, useRef, useState } from 'react'

import {
  describeRuntimeError,
  RuntimeClientError,
  validScheduleDefinition,
  validScheduleRecord,
} from '@flowtools/runtime-client'

import { Button, Card, Input, Label, TextField } from '@heroui/react'

import { builtInManifestData } from '../plugin/manifests'
import { isTauriRuntime } from '../runtime/desktop-capabilities'
import {
  controlManagedRuntime,
  connectManagedRuntime,
} from '../runtime/managed-client'

const available = builtInManifestData.flatMap(plugin =>
  plugin.commands
    .filter(command => command.headless && command.interaction !== 'required')
    .map(command => ({ plugin, command, key: `${plugin.id}:${command.id}` }))
)
const labels: Record<string, string> = {
  text: '文本',
  title: '提醒标题',
  message: '提醒内容',
  mode: '操作',
  todo: '待办事项',
  deadline: '截止日期',
  pattern: '表达式',
  flags: '匹配选项',
}
function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}
function scalar(value: unknown): string {
  return typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
    ? String(value)
    : ''
}
function defaults(schema: unknown): Record<string, unknown> {
  if (!object(schema) || !object(schema.properties)) return {}
  const required = Array.isArray(schema.required) ? schema.required : []
  return Object.fromEntries(
    Object.entries(schema.properties).flatMap(([key, field]) => {
      if (!object(field)) return []
      if (
        typeof field.default === 'string' ||
        typeof field.default === 'number' ||
        typeof field.default === 'boolean'
      )
        return [[key, field.default]]
      if (Array.isArray(field.enum) && typeof field.enum[0] === 'string')
        return [[key, field.enum[0]]]
      if (!required.includes(key)) return []
      return [
        [
          key,
          field.type === 'boolean'
            ? false
            : field.type === 'number' || field.type === 'integer'
              ? 0
              : '',
        ],
      ]
    })
  )
}

export function ScheduleSettings() {
  const native = isTauriRuntime()
  const [selection, setSelection] = useState(available[0]?.key ?? '')
  const selected = available.find(item => item.key === selection)
  const [input, setInput] = useState<Record<string, unknown>>(() =>
    defaults(available[0]?.command.inputSchema)
  )
  const [local, setLocal] = useState('')
  const [zone, setZone] = useState(
    () => Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
  )
  const [choice, setChoice] = useState<LocalTimeChoice>('reject')
  const [count, setCount] = useState('1')
  const [period, setPeriod] = useState('3600')
  const [grace, setGrace] = useState('30')
  const [missed, setMissed] = useState<'skip' | 'coalesce-latest'>('skip')
  const [resolved, setResolved] = useState<ResolvedScheduleTime>()
  const [records, setRecords] = useState<ScheduleRecord[]>([])
  const [editing, setEditing] = useState<ScheduleRecord>()
  const [recovery, setRecovery] = useState<ScheduleRecovery>()
  const [message, setMessage] = useState(
    '尚未查询调度。创建前请在共享 Runtime 中批准该命令的后台权限。'
  )
  const [busy, setBusy] = useState(false)
  const pending = useRef(false)
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  const requestKey = useRef<string | undefined>(undefined)
  const edit = () => {
    requestKey.current = undefined
  }
  const timeEdit = () => {
    edit()
    setResolved(undefined)
  }
  const run = async (action: () => Promise<void>) => {
    if (!native || pending.current) return
    pending.current = true
    setBusy(true)
    try {
      await action()
    } catch (error) {
      const diagnostic = describeRuntimeError(error)
      if (mounted.current)
        setMessage(
          `${diagnostic.code}：${diagnostic.summary}。请查询确认当前状态；不会自动重试创建。`
        )
    } finally {
      pending.current = false
      if (mounted.current) setBusy(false)
    }
  }
  const refresh = async () => {
    if (!mounted.current) return
    setRecords([])
    const outcome = await controlManagedRuntime({ method: 'schedules.list' })
    if (
      outcome.type !== 'schedules' ||
      outcome.data.length > 128 ||
      outcome.data.some(item => !validScheduleRecord(item))
    )
      throw new RuntimeClientError('INVALID_RESPONSE')
    if (!mounted.current) return
    setRecords(outcome.data)
    setMessage(`已查询 ${outcome.data.length} 条调度；列表不会自动刷新。`)
  }
  const preview = async () => {
    setResolved(undefined)
    const outcome = await controlManagedRuntime({
      method: 'schedules.resolve-time',
      payload: { local, timeZone: zone, choice },
    })
    if (outcome.type !== 'schedule-time')
      throw new RuntimeClientError('INVALID_RESPONSE')
    if (!mounted.current) return
    setResolved(outcome.data)
    setMessage(
      '时间已解析。重复或不存在的夏令时时刻默认拒绝，请核对 UTC 和时区偏移。'
    )
  }
  const create = async () => {
    if (!selected || !resolved) throw new RuntimeClientError('INPUT_INVALID')
    const definition: CreateSchedule['definition'] = {
      firstDueMs: resolved.dueMs,
      intervalMs: Number(count) === 1 ? null : Number(period) * 1000,
      occurrences: Number(count),
      graceMs: Number(grace) * 1000,
      missed,
    }
    if (!validScheduleDefinition(definition))
      throw new RuntimeClientError('INPUT_INVALID')
    requestKey.current ??= crypto.randomUUID()
    const payload: CreateSchedule = {
      requestKey: requestKey.current,
      target:
        editing?.binding.identity.caller === 'local-cli' ? 'cli' : 'desktop',
      pluginId: selected.plugin.id,
      commandId: selected.command.id,
      input,
      definition,
      timeoutMs: Math.min(30_000, selected.command.resources.timeoutMs),
    }
    const outcome = await controlManagedRuntime(
      editing
        ? {
            method: 'schedules.replace',
            payload: {
              previous: { id: editing.id, revision: editing.revision },
              create: payload,
            },
          }
        : { method: 'schedules.create', payload }
    )
    if (outcome.type !== 'schedule' || !validScheduleRecord(outcome.data))
      throw new RuntimeClientError('INVALID_RESPONSE')
    if (!mounted.current) return
    await refresh()
    setEditing(undefined)
    setResolved(undefined)
    requestKey.current = undefined
    setMessage(
      '调度已创建。Host 运行且授权有效时，关闭桌面后仍可触发；不会自行启动 Host。'
    )
  }
  const properties =
    selected &&
    object(selected.command.inputSchema) &&
    object(selected.command.inputSchema.properties)
      ? Object.entries(selected.command.inputSchema.properties)
      : []
  const fieldsSupported =
    properties.length <= 32 &&
    properties.every(
      ([, field]) =>
        object(field) &&
        ['string', 'number', 'integer', 'boolean'].includes(String(field.type))
    )
  return (
    <Card aria-label="后台调度">
      <Card.Header>
        <Card.Title>后台调度 · Prototype</Card.Title>
        <Card.Description>
          一次性或有限周期；使用现有后台授权，撤销后不再触发。
        </Card.Description>
      </Card.Header>
      <Card.Content className="space-y-4">
        {!native && <p>浏览器预览不能管理原生调度。</p>}
        <fieldset
          className="grid gap-3 border-0 p-0 sm:grid-cols-2"
          disabled={!native || busy}
        >
          <label className="grid gap-1">
            命令
            <select
              aria-label="调度命令"
              className="rounded-lg border border-border bg-surface p-2"
              value={selection}
              onChange={event => {
                edit()
                setSelection(event.target.value)
                setInput(
                  defaults(
                    available.find(item => item.key === event.target.value)
                      ?.command.inputSchema
                  )
                )
              }}
            >
              {available.map(item => (
                <option key={item.key} value={item.key}>
                  {item.plugin.name} · {item.command.name}
                </option>
              ))}
            </select>
          </label>
          <TextField
            value={local}
            onChange={value => {
              timeEdit()
              setLocal(value)
            }}
          >
            <Label>首次触发当地时间</Label>
            <Input placeholder="2026-10-28T09:00" />
          </TextField>
          <TextField
            value={zone}
            onChange={value => {
              timeEdit()
              setZone(value)
            }}
          >
            <Label>IANA 时区</Label>
            <Input placeholder="Asia/Shanghai" />
          </TextField>
          <label className="grid gap-1">
            夏令时歧义
            <select
              aria-label="夏令时歧义"
              className="rounded-lg border border-border bg-surface p-2"
              value={choice}
              onChange={event => {
                timeEdit()
                setChoice(event.target.value as LocalTimeChoice)
              }}
            >
              <option value="reject">拒绝重复或不存在时刻</option>
              <option value="earlier">明确选择较早时刻</option>
              <option value="later">明确选择较晚时刻</option>
            </select>
          </label>
          <TextField
            value={count}
            onChange={value => {
              edit()
              setCount(value)
            }}
          >
            <Label>触发次数</Label>
            <Input type="number" min={1} max={10000} />
          </TextField>
          <TextField
            value={period}
            onChange={value => {
              edit()
              setPeriod(value)
            }}
          >
            <Label>固定间隔（秒）</Label>
            <Input type="number" min={1} />
          </TextField>
          <label className="grid gap-1">
            休眠后漏触发
            <select
              aria-label="漏触发策略"
              className="rounded-lg border border-border bg-surface p-2"
              value={missed}
              onChange={event => {
                edit()
                setMissed(event.target.value as typeof missed)
              }}
            >
              <option value="skip">超过宽限期则跳过</option>
              <option value="coalesce-latest">只补最近一次，不批量补跑</option>
            </select>
          </label>
          <TextField
            value={grace}
            onChange={value => {
              edit()
              setGrace(value)
            }}
          >
            <Label>触发宽限秒数</Label>
            <Input type="number" min={0} max={60} step={0.001} />
          </TextField>
          {properties.map(([name, field]) =>
            object(field) &&
            Array.isArray(field.enum) &&
            field.enum.every(value => typeof value === 'string') ? (
              <label className="grid gap-1" key={name}>
                {labels[name] ?? name}
                <select
                  aria-label={labels[name] ?? name}
                  className="rounded-lg border border-border bg-surface p-2"
                  value={scalar(
                    input[name] ?? field.default ?? field.enum[0] ?? ''
                  )}
                  onChange={event => {
                    edit()
                    setInput(previous => ({
                      ...previous,
                      [name]: event.target.value,
                    }))
                  }}
                >
                  {field.enum.map(value => (
                    <option key={value} value={value}>
                      {value}
                    </option>
                  ))}
                </select>
              </label>
            ) : object(field) &&
              (field.type === 'string' ||
                field.type === 'number' ||
                field.type === 'integer') ? (
              <TextField
                key={name}
                value={scalar(input[name] ?? field.default ?? '')}
                onChange={value => {
                  edit()
                  setInput(previous => ({
                    ...previous,
                    [name]: field.type === 'string' ? value : Number(value),
                  }))
                }}
              >
                <Label>{labels[name] ?? name}</Label>
                <Input type={field.type === 'string' ? 'text' : 'number'} />
              </TextField>
            ) : object(field) && field.type === 'boolean' ? (
              <label key={name}>
                <input
                  type="checkbox"
                  checked={Boolean(input[name] ?? field.default)}
                  onChange={event => {
                    edit()
                    setInput(previous => ({
                      ...previous,
                      [name]: event.target.checked,
                    }))
                  }}
                />
                {labels[name] ?? name}
              </label>
            ) : null
          )}
        </fieldset>
        {!fieldsSupported && (
          <p>此命令含复合输入，请使用其专用界面或 CLI 配置文件创建调度。</p>
        )}
        <p className="text-xs text-muted">
          周期按实际经过的秒数计算，跨夏令时不会自动改成每天同一墙钟时刻。
        </p>
        {resolved && (
          <p aria-label="解析后的触发时间" className="block break-all">
            {resolved.zoned} · UTC {resolved.utc}
          </p>
        )}
        <div className="flex flex-wrap gap-2">
          <Button
            variant="secondary"
            isDisabled={!native || busy || !local || !zone}
            onPress={() => void run(preview)}
          >
            预览触发时间
          </Button>
          <Button
            isDisabled={!native || busy || !resolved || !fieldsSupported}
            onPress={() => void run(create)}
          >
            {editing ? '保存修改并确认' : '创建调度并确认'}
          </Button>
          <Button
            variant="secondary"
            isDisabled={!native || busy}
            onPress={() => void run(refresh)}
          >
            查询调度
          </Button>
          <Button
            variant="secondary"
            isDisabled={!native || busy}
            onPress={() =>
              void run(async () => {
                setRecovery(undefined)
                const result = await controlManagedRuntime({
                  method: 'schedules.recovery',
                })
                if (
                  result.type !== 'schedule-recovery' ||
                  (result.data &&
                    (!/^[a-f0-9]{64}$/.test(result.data.challenge) ||
                      result.data.count > 128))
                )
                  throw new RuntimeClientError('INVALID_RESPONSE')
                if (!mounted.current) return
                setRecovery(result.data ?? undefined)
                setMessage(
                  result.data
                    ? `备份恢复后有 ${result.data.count} 条旧调度等待作废；不会自动执行。`
                    : '当前没有待处理的调度恢复。'
                )
              })
            }
          >
            检查恢复状态
          </Button>
        </div>
        {editing && (
          <div className="text-sm">
            <p>
              正在修改 {editing.id}
              。请重新填写命令内容并核对新时间、次数；保存前旧计划保持不变。旧任务正在执行时拒绝修改。
            </p>
            <Button
              variant="secondary"
              onPress={() => {
                setEditing(undefined)
                timeEdit()
              }}
            >
              放弃编辑
            </Button>
          </div>
        )}
        {recovery && (
          <div className="text-sm">
            <p>
              作废这次备份恢复中的全部 {recovery.count}{' '}
              条旧调度，保留尝试位置与记录。新的提醒须重新创建。
            </p>
            <Button
              variant="danger"
              isDisabled={busy}
              onPress={() =>
                void run(async () => {
                  const result = await controlManagedRuntime({
                    method: 'schedules.discard-restored',
                    payload: { challenge: recovery.challenge },
                  })
                  if (
                    result.type !== 'schedules' ||
                    result.data.some(item => !validScheduleRecord(item))
                  )
                    throw new RuntimeClientError('INVALID_RESPONSE')
                  if (!mounted.current) return
                  setRecovery(undefined)
                  await refresh()
                })
              }
            >
              作废旧调度并确认
            </Button>
          </div>
        )}
        <p role="status">{message}</p>
        <ul className="space-y-3 p-0">
          {records.map(record => (
            <li
              key={record.id}
              className="list-none rounded-lg border border-border p-3 text-sm"
            >
              <strong>
                {record.binding.identity.pluginId} ·{' '}
                {record.cancelled
                  ? '已取消'
                  : record.nextIndex === record.definition.occurrences
                    ? '周期已结束'
                    : '等待触发'}
              </strong>
              <p>
                首时刻 UTC：
                {new Date(record.definition.firstDueMs).toISOString()} · 已消费{' '}
                {record.nextIndex}/{record.definition.occurrences} · 跳过{' '}
                {record.skipped}
              </p>
              <p aria-label="已保存的调度策略">
                {record.definition.intervalMs === null
                  ? '一次性触发'
                  : `每 ${record.definition.intervalMs / 1000} 秒触发，共 ${record.definition.occurrences} 次（按实际经过时间）`}
                {' · '}
                {record.definition.missed === 'skip'
                  ? `休眠或暂停后，迟于 ${record.definition.graceMs / 1000} 秒则跳过；不会批量补跑`
                  : '休眠或暂停后，只补最近一次，其余跳过；不会批量补跑'}
                {' · '}宽限 {record.definition.graceMs / 1000} 秒
              </p>
              {record.lastClaim && (
                <p>
                  最近尝试：{record.lastClaim.result.state}
                  {record.lastClaim.result.state === 'rejected'
                    ? ` · ${record.lastClaim.result.code}`
                    : ''}
                </p>
              )}
              {record.binding.identity.pluginId === 'plugin-local-reminder' &&
                record.lastClaim?.result.state === 'submitted' && (
                  <Button
                    size="sm"
                    variant="secondary"
                    isDisabled={busy}
                    onPress={() =>
                      void run(async () => {
                        if (record.lastClaim?.result.state !== 'submitted')
                          return
                        const client = await connectManagedRuntime()
                        try {
                          const job = await client.job(
                            record.lastClaim.result.runId
                          )
                          const data = job.result?.success
                            ? job.result.data
                            : undefined
                          const value =
                            object(data) &&
                            object(data.value) &&
                            object(data.value.result)
                              ? data.value.result
                              : undefined
                          const labels: Record<string, string> = {
                            requested: '系统已接受通知请求，尚不代表已显示',
                            unsupported: '此平台不支持系统通知',
                            failed:
                              '系统拒绝通知请求，请检查应用安装和通知设置',
                            unknown: '发送结果未知，不会自动重发',
                          }
                          if (mounted.current)
                            setMessage(
                              typeof value?.status === 'string'
                                ? (labels[value.status] ?? '通知结果无效')
                                : `提醒任务状态：${job.state}`
                            )
                        } finally {
                          client.close()
                        }
                      })
                    }
                  >
                    查看提醒结果
                  </Button>
                )}
              <Button
                size="sm"
                variant="secondary"
                isDisabled={busy || record.cancelled}
                onPress={() => {
                  const key = `${record.binding.identity.pluginId}:${record.binding.identity.commandId}`
                  const item = available.find(item => item.key === key)
                  if (!item) {
                    setMessage('该命令已不可用，不能修改。')
                    return
                  }
                  setEditing(record)
                  setSelection(key)
                  setInput(defaults(item.command.inputSchema))
                  setLocal(
                    new Date(record.definition.firstDueMs)
                      .toISOString()
                      .slice(0, 19)
                  )
                  setZone('UTC')
                  setChoice('reject')
                  setCount(String(record.definition.occurrences))
                  setPeriod(
                    String((record.definition.intervalMs ?? 3_600_000) / 1000)
                  )
                  setMissed(record.definition.missed)
                  setGrace(String(record.definition.graceMs / 1000))
                  timeEdit()
                  setMessage(
                    '请重新填写修改后的命令内容并预览时间。保存前不会改变旧计划。'
                  )
                }}
              >
                编辑此调度
              </Button>
              <Button
                size="sm"
                variant="secondary"
                isDisabled={busy || record.cancelled}
                onPress={() =>
                  void run(async () => {
                    await controlManagedRuntime({
                      method: 'schedules.cancel',
                      payload: { id: record.id, revision: record.revision },
                    })
                    await refresh()
                  })
                }
              >
                取消此调度
              </Button>
            </li>
          ))}
        </ul>
      </Card.Content>
    </Card>
  )
}

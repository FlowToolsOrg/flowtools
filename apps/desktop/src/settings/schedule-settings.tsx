import type {
  CreateSchedule,
  LocalTimeChoice,
  ResolvedScheduleTime,
  ScheduleRecord,
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
import { controlManagedRuntime } from '../runtime/managed-client'

const available = builtInManifestData.flatMap(plugin =>
  plugin.commands
    .filter(command => command.headless && command.interaction !== 'required')
    .map(command => ({ plugin, command, key: `${plugin.id}:${command.id}` }))
)
const labels: Record<string, string> = {
  text: '文本',
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
  const [missed, setMissed] = useState<'skip' | 'coalesce-latest'>('skip')
  const [resolved, setResolved] = useState<ResolvedScheduleTime>()
  const [records, setRecords] = useState<ScheduleRecord[]>([])
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
      graceMs: 30_000,
      missed,
    }
    if (!validScheduleDefinition(definition))
      throw new RuntimeClientError('INPUT_INVALID')
    requestKey.current ??= crypto.randomUUID()
    const outcome = await controlManagedRuntime({
      method: 'schedules.create',
      payload: {
        requestKey: requestKey.current,
        target: 'desktop',
        pluginId: selected.plugin.id,
        commandId: selected.command.id,
        input,
        definition,
        timeoutMs: Math.min(30_000, selected.command.resources.timeoutMs),
      },
    })
    if (outcome.type !== 'schedule' || !validScheduleRecord(outcome.data))
      throw new RuntimeClientError('INVALID_RESPONSE')
    if (!mounted.current) return
    await refresh()
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
              <option value="skip">超过 30 秒则跳过</option>
              <option value="coalesce-latest">只补最近一次，不批量补跑</option>
            </select>
          </label>
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
            创建调度并确认
          </Button>
          <Button
            variant="secondary"
            isDisabled={!native || busy}
            onPress={() => void run(refresh)}
          >
            查询调度
          </Button>
        </div>
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
              {record.lastClaim && (
                <p>
                  最近尝试：{record.lastClaim.result.state}
                  {record.lastClaim.result.state === 'rejected'
                    ? ` · ${record.lastClaim.result.code}`
                    : ''}
                </p>
              )}
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

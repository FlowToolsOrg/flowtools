import type {
  Call,
  FilePlanRecord,
  FilePlanView,
  Outcome,
} from '@flowtools/runtime-client'

import { useState } from 'react'

import { describeRuntimeError } from '@flowtools/runtime-client'

import { Button, Card, Input, Label, TextField } from '@heroui/react'

import { controlManagedRuntime } from '../runtime/managed-client'

export function FileSettings({
  control = controlManagedRuntime,
  selectDirectory = chooseDirectory,
}: {
  control?: (call: Call) => Promise<Outcome>
  selectDirectory?: () => Promise<string | null>
}) {
  const [prefix, setPrefix] = useState('整理_')
  const [plans, setPlans] = useState<FilePlanRecord[]>([])
  const [view, setView] = useState<FilePlanView | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const [recovery, setRecovery] = useState<string | null>(null)
  const perform = async (action: () => Promise<void>) => {
    if (busy) return
    setBusy(true)
    setError('')
    setMessage('')
    try {
      await action()
    } catch (e) {
      const report = describeRuntimeError(e)
      setError(`${report.summary}：${report.action}`)
    } finally {
      setBusy(false)
    }
  }
  const refresh = async () => {
    const recovery = await control({ method: 'files.recovery' })
    if (recovery.type !== 'file-recovery') throw new Error('INVALID_RESPONSE')
    setRecovery(recovery.data?.challenge ?? null)
    if (recovery.data) {
      setPlans([])
      setView(null)
      return
    }
    const result = await control({ method: 'files.list' })
    if (result.type !== 'file-plans') throw new Error('INVALID_RESPONSE')
    setPlans(result.data)
  }
  const preview = async (plan: FilePlanRecord) => {
    const result = await control({
      method: 'files.preview',
      payload: { id: plan.id, revision: plan.revision },
    })
    if (result.type !== 'file-plan') throw new Error('INVALID_RESPONSE')
    setView(result.data)
  }
  const effect = async (undo: boolean) => {
    if (!view) return
    const result = await control({
      method: undo ? 'files.undo' : 'files.commit',
      payload: {
        id: view.record.id,
        revision: view.record.revision,
        digest: view.record.digest,
      },
    })
    if (result.type !== 'file-execution') throw new Error('INVALID_RESPONSE')
    setMessage(
      `${undo ? '撤销' : '提交'}完成：${result.data.completed} 个文件处于目标名称。`
    )
    await preview(view.record)
  }
  return (
    <Card>
      <Card.Header>
        <Card.Title>文件整理</Card.Title>
        <Card.Description>
          选择目录，预览前缀改名计划，再确认提交。失败后可查看原计划并重试；撤销会拒绝外部修改的文件。
        </Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <TextField value={prefix} onChange={setPrefix}>
          <Label>文件名前缀</Label>
          <Input />
        </TextField>
        <div className="flex flex-wrap gap-2">
          <Button
            isDisabled={busy || !prefix}
            onPress={() =>
              void perform(async () => {
                const directory = await selectDirectory()
                if (!directory) return
                const result = await control({
                  method: 'files.plan',
                  payload: {
                    requestKey: crypto.randomUUID(),
                    directory,
                    prefix,
                  },
                })
                if (result.type !== 'file-plan')
                  throw new Error('INVALID_RESPONSE')
                setView(result.data)
                await refresh()
              })
            }
          >
            选择目录并预览
          </Button>
          <Button
            variant="secondary"
            isDisabled={busy}
            onPress={() => void perform(refresh)}
          >
            刷新文件计划
          </Button>
        </div>
        {error && <p role="alert">{error}</p>}
        {recovery && (
          <section aria-label="文件计划恢复">
            <p>
              检测到恢复前的旧计划。当前文件保持原样，请核对实际文件后作废旧计划；不会自动改名或撤销。
            </p>
            <Button
              isDisabled={busy}
              onPress={() =>
                void perform(async () => {
                  await control({
                    method: 'files.discard-restored',
                    payload: { challenge: recovery },
                  })
                  setRecovery(null)
                  await refresh()
                })
              }
            >
              作废恢复前的文件计划
            </Button>
          </section>
        )}
        <p role="status" aria-live="polite">
          {busy ? '正在处理…' : message}
        </p>
        {plans.length > 0 && (
          <ul>
            {plans.map(p => (
              <li key={p.id} className="flex flex-wrap items-center gap-2">
                <span>
                  {p.files} 个文件 · {p.cancelled ? '已取消' : p.id}
                </span>
                <Button
                  size="sm"
                  variant="secondary"
                  isDisabled={busy || p.cancelled}
                  onPress={() => void perform(() => preview(p))}
                >
                  查看计划 {p.id.slice(0, 8)}
                </Button>
              </li>
            ))}
          </ul>
        )}
        {view && (
          <section aria-label="文件计划预览" className="flex flex-col gap-3">
            <p className="break-all">
              计划 ID：{view.record.id} · 版本 {view.record.revision}
            </p>
            <p className="break-all">摘要：{view.record.digest}</p>
            <p>
              共 {view.record.files} 个文件，{view.record.totalBytes} 字节；跳过{' '}
              {view.record.skipped} 项。
              {view.execution &&
                `状态：${view.execution.phase}，已改名 ${view.execution.completed} 项。`}
            </p>
            <div className="overflow-x-auto">
              <table>
                <thead>
                  <tr>
                    <th scope="col">原名称</th>
                    <th scope="col">目标名称</th>
                    <th scope="col">字节</th>
                  </tr>
                </thead>
                <tbody>
                  {view.preview.files.map(file => (
                    <tr key={file.source}>
                      <td>{file.source}</td>
                      <td>{file.target}</td>
                      <td>{file.bytes}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button
                isDisabled={
                  busy ||
                  view.execution?.phase === 'undone' ||
                  view.execution?.phase === 'undoing'
                }
                onPress={() => void perform(() => effect(false))}
              >
                确认提交 / 重试
              </Button>
              <Button
                variant="secondary"
                isDisabled={
                  busy || !view.execution || view.execution.phase === 'undone'
                }
                onPress={() => void perform(() => effect(true))}
              >
                撤销未修改文件
              </Button>
              <Button
                variant="secondary"
                isDisabled={busy || !!view.execution}
                onPress={() =>
                  void perform(async () => {
                    await control({
                      method: 'files.cancel',
                      payload: {
                        id: view.record.id,
                        revision: view.record.revision,
                      },
                    })
                    setView(null)
                    await refresh()
                  })
                }
              >
                取消此计划
              </Button>
            </div>
          </section>
        )}
      </Card.Content>
    </Card>
  )
}
async function chooseDirectory(): Promise<string | null> {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({
    directory: true,
    multiple: false,
    title: '选择要整理的目录',
  })
  return typeof selected === 'string' ? selected : null
}

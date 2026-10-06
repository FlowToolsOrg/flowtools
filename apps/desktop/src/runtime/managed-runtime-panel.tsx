import type {
  JobSnapshot,
  PermissionRecord,
  PolicyImport,
  RuntimeStatus,
  RunDiagnostic,
  StorageReport,
} from '@flowtools/runtime-client'

import { useEffect, useState } from 'react'

import {
  describeRuntimeError,
  RuntimeExecutionError,
  withSubmittedRuntimeJob,
} from '@flowtools/runtime-client'

import { Button, Card, Input, Label, TextField } from '@heroui/react'

import { builtInManifestData } from '../plugin/manifests'

import { isTauriRuntime } from './desktop-capabilities'
import {
  connectManagedRuntime,
  controlManagedRuntime,
  storageManagedRuntime,
} from './managed-client'

export function ManagedRuntimePanel() {
  const [status, setStatus] = useState<RuntimeStatus>()
  const [jobs, setJobs] = useState<JobSnapshot[]>([])
  const [permissions, setPermissions] = useState<PermissionRecord[]>([])
  const [message, setMessage] = useState('尚未连接')
  const [busy, setBusy] = useState(false)
  const [pluginId, setPluginId] = useState('plugin-todo-list')
  const [input, setInput] = useState('{}')
  const [background, setBackground] = useState(false)
  const [diagnostic, setDiagnostic] = useState<RunDiagnostic>()
  const [storage, setStorage] = useState<StorageReport>()
  const [backupId, setBackupId] = useState('')
  const report = (error: unknown) => {
    const diagnostic = describeRuntimeError(error)
    setMessage(
      `${diagnostic.code}：${diagnostic.summary}；${diagnostic.action}${error instanceof RuntimeExecutionError ? `；原幂等键 ${error.idempotencyKey}${error.runId ? `；runId ${error.runId}` : ''}` : ''}`
    )
  }
  const refresh = async () => {
    const client = await connectManagedRuntime()
    try {
      const outcome = await client.call({ method: 'runtime.status' })
      if (outcome.type === 'status') setStatus(outcome.data)
      setJobs(await client.jobs())
      const grants = await controlManagedRuntime({ method: 'permissions.list' })
      if (grants.type === 'permissions') setPermissions(grants.data)
      setMessage('已连接共享 Runtime')
    } finally {
      client.close()
    }
  }
  const action = async (run: () => Promise<void>) => {
    setBusy(true)
    try {
      await run()
    } catch (error) {
      report(error)
    } finally {
      setBusy(false)
    }
  }
  useEffect(() => {
    // Explicit refresh/start avoids mounting a settings page cold-starting business work.
    if (!isTauriRuntime())
      setMessage('共享任务需要原生 Desktop 和已初始化的 Runtime')
  }, [])
  const approve = async () => {
    const manifest = builtInManifestData.find(item => item.id === pluginId)!
    const command = manifest.commands.find(item => item.id === 'run')!
    const catalog = await controlManagedRuntime({ method: 'plugins.list' })
    const packageDigest =
      catalog.type === 'plugins'
        ? catalog.data.find(item => item.pluginId === pluginId)?.packageDigest
        : undefined
    if (!packageDigest) throw new Error('PLUGIN_NOT_FOUND')
    const policy: PolicyImport = {
      formatVersion: 1,
      coldStart: true,
      grants: (['cli', 'desktop'] as const).map(target => ({
        pluginId,
        commandId: command.id,
        target,
        packageDigest,
        effects: command.effects,
        scopes:
          pluginId === 'plugin-todo-list'
            ? [{ kind: 'plugin-data' as const, scope: { key_prefix: 'todos' } }]
            : [],
        expiresAt: Date.now() + 86400000,
        maxCalls: 128,
        coldStart: command.supportsColdStart,
        background,
      })),
    }
    await controlManagedRuntime({ method: 'policy.import', payload: policy })
    await refresh()
  }
  return (
    <Card aria-label="共享 Runtime">
      <Card.Header>
        <Card.Title>共享 Runtime · Prototype</Card.Title>
        <Card.Description>
          GUI 与 CLI 使用同一份任务、授权和数据。后台任务需要单独批准。
        </Card.Description>
      </Card.Header>
      <Card.Content className="space-y-4">
        <p role="status" data-testid="managed-runtime-status">
          {message}
        </p>
        {status && (
          <p data-testid="managed-runtime-instance">
            {status.instanceId} · {status.activeJobs} 个活动任务 / {status.jobs}{' '}
            个已记录任务
          </p>
        )}
        <div className="flex flex-wrap gap-2">
          <Button
            isDisabled={busy || !isTauriRuntime()}
            onPress={() => void action(refresh)}
          >
            启动 / 刷新 Runtime
          </Button>
          <Button
            isDisabled={busy || !isTauriRuntime()}
            variant="secondary"
            onPress={() =>
              void action(async () => {
                await controlManagedRuntime(
                  {
                    method: 'policy.import',
                    payload: { formatVersion: 1, coldStart: false, grants: [] },
                  },
                  true
                )
                setMessage('已初始化。选择命令并明确批准授权后启动。')
              })
            }
          >
            初始化 Runtime
          </Button>
          <Button
            isDisabled={busy || !status}
            variant="danger"
            onPress={() =>
              void action(async () => {
                await controlManagedRuntime({ method: 'runtime.stop' })
                setStatus(undefined)
                setJobs([])
                setMessage(
                  '停止请求已接受；Runtime 正在取消并排空活动任务。备份前请确认写入者已释放。'
                )
              })
            }
          >
            停止 Runtime（{status?.activeJobs ?? 0} 个活动任务）
          </Button>
        </div>
        <label className="block">
          内置命令{' '}
          <select
            aria-label="Runtime 命令"
            value={pluginId}
            onChange={event => setPluginId(event.target.value)}
          >
            {builtInManifestData
              .filter(item =>
                item.commands.some(
                  command =>
                    command.id === 'run' &&
                    command.effects.every(effect =>
                      ['data-read', 'data-write'].includes(effect)
                    )
                )
              )
              .map(item => (
                <option key={item.id} value={item.id}>
                  {item.name}
                </option>
              ))}
          </select>
        </label>
        <label className="flex gap-2">
          <input
            type="checkbox"
            checked={background}
            onChange={event => setBackground(event.target.checked)}
          />
          批准后台执行，并以后台任务提交
        </label>
        <Button
          isDisabled={busy || !isTauriRuntime()}
          variant="secondary"
          onPress={() => void action(approve)}
        >
          批准所选命令（GUI / CLI，24 小时）
        </Button>
        <TextField value={input} onChange={setInput}>
          <Label>任务输入 JSON</Label>
          <Input />
        </TextField>
        <Button
          isDisabled={busy || !status}
          onPress={() =>
            void action(async () => {
              const value: unknown = JSON.parse(input)
              await withSubmittedRuntimeJob(
                connectManagedRuntime,
                {
                  pluginId,
                  commandId: 'run',
                  input: value,
                  idempotencyKey: crypto.randomUUID(),
                  background,
                  deadline: Date.now() + 30000,
                },
                async (client, receipt) => {
                  setMessage(`已接受：${receipt.runId}`)
                  if (!background) {
                    const terminal = await client.waitForResult(receipt.runId)
                    setMessage(`${terminal.runId}：${terminal.state}`)
                  }
                }
              )
              await refresh()
            })
          }
        >
          提交任务
        </Button>
        <ul aria-label="Runtime 授权" className="space-y-2">
          {permissions.map(record => (
            <li key={`${record.identity.pluginId}-${record.identity.caller}`}>
              {record.identity.pluginId} · {record.identity.caller} · epoch{' '}
              {record.epoch} · {record.grant ? '已授权' : '已撤销'}
              {record.grant && (
                <Button
                  size="sm"
                  variant="secondary"
                  isDisabled={busy}
                  onPress={() =>
                    void action(async () => {
                      await controlManagedRuntime({
                        method: 'permissions.revoke',
                        payload: {
                          pluginId: record.grant!.pluginId,
                          commandId: record.grant!.commandId,
                          target: record.grant!.target,
                        },
                      })
                      await refresh()
                    })
                  }
                >
                  撤销 {record.identity.pluginId} {record.grant.target}
                </Button>
              )}
            </li>
          ))}
        </ul>
        <ul aria-label="共享任务" className="space-y-2">
          {jobs.map(job => (
            <li key={job.runId} data-testid="managed-job">
              <span>
                {job.runId} · {job.pluginId} · {job.rootCaller} · {job.state}
              </span>
              <Button
                size="sm"
                variant="secondary"
                isDisabled={busy}
                onPress={() =>
                  void action(async () => {
                    const client = await connectManagedRuntime()
                    try {
                      setDiagnostic(await client.diagnose(job.runId))
                    } finally {
                      client.close()
                    }
                  })
                }
              >
                诊断 {job.runId}
              </Button>
              {!['succeeded', 'failed', 'cancelled', 'interrupted'].includes(
                job.state
              ) && (
                <Button
                  size="sm"
                  isDisabled={busy}
                  onPress={() =>
                    void action(async () => {
                      const client = await connectManagedRuntime()
                      try {
                        await client.cancel(job.runId)
                      } finally {
                        client.close()
                      }
                      await refresh()
                    })
                  }
                >
                  取消 {job.runId}
                </Button>
              )}
            </li>
          ))}
        </ul>
        {diagnostic && (
          <section aria-label="运行诊断" className="space-y-2">
            <p>
              {diagnostic.requiresReview
                ? '需核对实际副作用后决定下一步；不会自动重试。'
                : '以下摘要只包含运行元数据。'}
            </p>
            <pre className="overflow-auto" data-testid="managed-run-diagnostic">
              {JSON.stringify(diagnostic, null, 2)}
            </pre>
            <Button
              variant="secondary"
              onPress={() => {
                const url = URL.createObjectURL(
                  new Blob([JSON.stringify(diagnostic, null, 2)], {
                    type: 'application/json',
                  })
                )
                const link = document.createElement('a')
                link.href = url
                link.download = `flowtools-run-${diagnostic.runId}.json`
                link.click()
                URL.revokeObjectURL(url)
              }}
            >
              导出诊断摘要
            </Button>
          </section>
        )}
        <section aria-label="Runtime 备份恢复" className="space-y-2">
          <p>
            先停止 Runtime
            再备份或恢复。恢复会回滚数据、撤销所有授权并中断未完成任务；原库和私有任务正文保留供核对。
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="secondary"
              isDisabled={busy || !isTauriRuntime()}
              onPress={() =>
                void action(async () => {
                  setStorage(await storageManagedRuntime({ operation: 'list' }))
                  setMessage('已读取离线备份')
                })
              }
            >
              读取备份
            </Button>
            <Button
              variant="secondary"
              isDisabled={busy || !isTauriRuntime()}
              onPress={() =>
                void action(async () => {
                  setStorage(
                    await storageManagedRuntime({ operation: 'create' })
                  )
                  setMessage('备份已创建')
                })
              }
            >
              创建备份
            </Button>
            <Button
              variant="danger"
              isDisabled={
                busy ||
                !storage?.recovery ||
                storage.recovery.phase === 'complete'
              }
              onPress={() =>
                void action(async () => {
                  setStorage(
                    await storageManagedRuntime({ operation: 'retry' })
                  )
                  setMessage('原恢复已完成；请重新批准授权')
                })
              }
            >
              继续未完成恢复
            </Button>
          </div>
          {storage?.recovery && (
            <p data-testid="managed-recovery-state">
              {storage.recovery.recoveryId} · {storage.recovery.phase} ·{' '}
              {storage.recovery.grantsRevoked ? '授权已撤销' : '恢复待完成'}
            </p>
          )}
          <label className="block">
            已验证备份{' '}
            <select
              aria-label="Runtime 备份"
              value={backupId}
              onChange={event => setBackupId(event.target.value)}
            >
              <option value="">请选择</option>
              {storage?.backups.map(backup => (
                <option
                  key={backup.id}
                  value={backup.id}
                  disabled={!backup.usable}
                >
                  {backup.id} · {backup.schemaVersion ?? backup.errorCode}
                </option>
              ))}
            </select>
          </label>
          <Button
            variant="danger"
            isDisabled={busy || !backupId}
            onPress={() =>
              void action(async () => {
                setStorage(
                  await storageManagedRuntime({
                    operation: 'restore',
                    parameters: { backupId },
                  })
                )
                setStatus(undefined)
                setJobs([])
                setMessage('已恢复并撤销全部授权；核对数据后重新批准')
              })
            }
          >
            恢复所选备份
          </Button>
        </section>
      </Card.Content>
    </Card>
  )
}

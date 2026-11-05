import type {
  ResourceKind,
  ResourceOwner,
  ResourceInfo,
  RuntimeClient,
  JobSnapshot,
  Call,
  Outcome,
} from '@flowtools/runtime-client'

import { useEffect, useRef, useState } from 'react'

import {
  describeRuntimeError,
  RuntimeClientError,
} from '@flowtools/runtime-client'

import { Button, Card } from '@heroui/react'

import {
  connectManagedRuntime,
  controlManagedRuntime,
} from '../runtime/managed-client'
import { commands } from '../utils/bindings'
type Choice = { owner: ResourceOwner; resource: ResourceInfo }
async function selectResource(kind: ResourceKind): Promise<Choice | null> {
  const response = await commands.managedResourceSelect(
    kind,
    'plugin-resource-reader',
    'run',
    'source'
  )
  if (response.status === 'error')
    throw new RuntimeClientError(response.error.code)
  return response.data
}
export function ResourceSettings({
  select = selectResource,
  connect = connectManagedRuntime,
  control = controlManagedRuntime,
}: {
  select?: (kind: ResourceKind) => Promise<Choice | null>
  connect?: () => Promise<Pick<RuntimeClient, 'call' | 'close'>>
  control?: (call: Call) => Promise<Outcome>
}) {
  const [choice, setChoice] = useState<Choice | null>(null),
    [busy, setBusy] = useState(false),
    [job, setJob] = useState<JobSnapshot | null>(null),
    [error, setError] = useState('')
  const owned = useRef<{
    choice: Choice | null
    client: Pick<RuntimeClient, 'call' | 'close'> | null
  }>({ choice: null, client: null })
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    const resources = owned.current
    return () => {
      mounted.current = false
      resources.client?.close()
      resources.client = null
      const id = resources.choice?.owner.id
      if (id)
        void control({
          method: 'resources.drop',
          payload: { owner: id },
        }).catch(() => {})
    }
  }, [control])
  const perform = async (action: () => Promise<void>) => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      await action()
    } catch (e) {
      const explanation = describeRuntimeError(e)
      setError(`${explanation.summary} ${explanation.action}`)
    } finally {
      setBusy(false)
    }
  }
  const pick = (kind: ResourceKind) =>
    perform(async () => {
      if (owned.current.choice)
        await control({
          method: 'resources.drop',
          payload: { owner: owned.current.choice.owner.id },
        })
      owned.current.choice = null
      setChoice(null)
      setJob(null)
      const selected = await select(kind)
      if (!mounted.current) {
        if (selected)
          await control({
            method: 'resources.drop',
            payload: { owner: selected.owner.id },
          })
        return
      }
      owned.current.choice = selected
      setChoice(selected)
    })
  const run = () =>
    perform(async () => {
      if (!choice) return
      const client = await connect()
      if (!mounted.current) {
        client.close()
        return
      }
      owned.current.client = client
      try {
        const accepted = await client.call({
          method: 'resources.submit',
          payload: {
            owner: choice.owner.id,
            submit: {
              pluginId: 'plugin-resource-reader',
              commandId: 'run',
              input: {},
              idempotencyKey: crypto.randomUUID(),
              background: false,
              deadline: Date.now() + 30000,
            },
          },
        })
        if (accepted.type !== 'receipt') throw new Error('INVALID_RESPONSE')
        owned.current.choice = null
        setChoice(null)
        for (;;) {
          if (!mounted.current) throw new RuntimeClientError('ABORTED')
          const result = await client.call({
            method: 'jobs.status',
            payload: { runId: accepted.data.runId },
          })
          if (result.type !== 'job') throw new Error('INVALID_RESPONSE')
          if (!mounted.current) break
          setJob(result.data)
          if (
            ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(
              result.data.state
            )
          ) {
            if (result.data.result?.success === false)
              throw new RuntimeClientError(
                result.data.result.error?.code ?? 'INVALID_RESPONSE'
              )
            break
          }
          await new Promise(resolve => setTimeout(resolve, 100))
        }
      } finally {
        client.close()
        owned.current.client = null
      }
    })
  return (
    <Card>
      <Card.Header>
        <Card.Title>资源读取</Card.Title>
        <Card.Description>
          将所选资源绑定到一次“资源读取”命令。只读文件，或统计一层目录条目。
        </Card.Description>
      </Card.Header>
      <Card.Content className="space-y-3">
        <div className="flex flex-wrap gap-2">
          <Button
            variant="secondary"
            isDisabled={busy}
            onPress={() => void pick('file')}
          >
            选择文件并确认权限
          </Button>
          <Button
            variant="secondary"
            isDisabled={busy}
            onPress={() => void pick('directory')}
          >
            选择目录并确认权限
          </Button>
        </div>
        {choice && (
          <p>
            已选择：{choice.resource.name} ·{' '}
            {choice.resource.kind === 'file'
              ? `${choice.resource.size ?? 0} 字节，只读`
              : '目录，仅列出一层'}
          </p>
        )}
        <p className="text-sm text-muted">
          选择有效期五分钟。内容和路径不写入任务历史；目录条目不会授予子文件读取权限。
        </p>
        {job && (
          <p role="status">
            {
              (
                {
                  accepted: '已接受',
                  queued: '排队中',
                  running: '正在读取',
                  cancelling: '正在取消',
                  succeeded: '读取完成',
                  failed: '读取失败',
                  cancelled: '已取消',
                  interrupted: '已中断',
                } as const
              )[job.state]
            }
          </p>
        )}
        {error && <p role="alert">{error}</p>}
      </Card.Content>
      <Card.Footer>
        <Button isDisabled={busy || !choice} onPress={() => void run()}>
          运行读取
        </Button>
        {job &&
          !['succeeded', 'failed', 'cancelled', 'interrupted'].includes(
            job.state
          ) && (
            <Button
              variant="danger"
              onPress={() => {
                void owned.current.client
                  ?.call({
                    method: 'jobs.cancel',
                    payload: { runId: job.runId },
                  })
                  .catch(() => {})
              }}
            >
              取消读取
            </Button>
          )}
      </Card.Footer>
    </Card>
  )
}

import type { RuntimeStatus } from '@flowtools/runtime-client'

import { useEffect, useRef, useState } from 'react'

import {
  describeRuntimeError,
  RuntimeClientError,
} from '@flowtools/runtime-client'

import { Button, Card } from '@heroui/react'

import { isTauriRuntime } from '../runtime/desktop-capabilities'
import { controlManagedRuntime } from '../runtime/managed-client'

const modes = new Map([
  ['managed', '运行模式'],
  ['management', '仅管理模式'],
  ['validation', '验证模式'],
])

export function RuntimeStatusSettings() {
  const native = isTauriRuntime()
  const mounted = useRef(true)
  const [busy, setBusy] = useState(false)
  const [snapshot, setSnapshot] = useState<{
    status: RuntimeStatus
    checkedAt: string
  }>()
  const [message, setMessage] = useState('尚未查询 Runtime 状态。')

  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  const query = async () => {
    if (busy || !native) return
    setBusy(true)
    setSnapshot(undefined)
    setMessage('正在查询 Runtime 状态…')
    try {
      const result = await controlManagedRuntime(
        { method: 'runtime.status' },
        false
      )
      if (result.type !== 'status')
        throw new RuntimeClientError('INVALID_RESPONSE')
      if (!mounted.current) return
      setSnapshot({ status: result.data, checkedAt: new Date().toISOString() })
      setMessage('查询成功；以下为最近一次状态快照，不会自动更新。')
    } catch (error) {
      if (!mounted.current) return
      const diagnostic = describeRuntimeError(error)
      setMessage(
        `无法确认 Runtime 当前状态。${diagnostic.code}：${diagnostic.summary}`
      )
    } finally {
      if (mounted.current) setBusy(false)
    }
  }

  return (
    <section aria-label="Runtime 状态">
      <Card>
        <Card.Header>
          <Card.Title>Runtime 状态 · Prototype</Card.Title>
          <Card.Description>
            仅查询已运行的
            Runtime，不会初始化、启动服务或改变授权。桌面驻留不代表后台任务正在运行。
          </Card.Description>
        </Card.Header>
        <Card.Content className="grid gap-3">
          <p className="m-0 text-sm" role="status">
            {native ? message : '请在原生 Desktop 中查询 Runtime 状态。'}
          </p>
          {snapshot ? (
            <dl className="m-0 grid grid-cols-[80px_minmax(0,1fr)] gap-x-3 gap-y-2 text-sm">
              <dt>实例</dt>
              <dd className="m-0 break-all">{snapshot.status.instanceId}</dd>
              <dt>模式</dt>
              <dd className="m-0">
                {modes.get(snapshot.status.mode) ?? '未知模式'}
              </dd>
              <dt>任务</dt>
              <dd className="m-0">
                {snapshot.status.activeJobs} 个活动任务 / {snapshot.status.jobs}{' '}
                个已记录任务
              </dd>
              <dt>查询时间</dt>
              <dd className="m-0">
                <time dateTime={snapshot.checkedAt}>
                  {new Date(snapshot.checkedAt).toLocaleString()}
                </time>
              </dd>
            </dl>
          ) : null}
          <Button
            className="justify-self-start"
            isDisabled={busy || !native}
            onPress={() => void query()}
            size="sm"
            variant="secondary"
          >
            查询 Runtime 状态
          </Button>
        </Card.Content>
      </Card>
    </section>
  )
}

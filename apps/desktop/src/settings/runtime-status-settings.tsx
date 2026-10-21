import type { RuntimeStatus } from '@flowtools/runtime-client'

import { useEffect, useRef, useState } from 'react'

import {
  describeRuntimeError,
  RuntimeClientError,
} from '@flowtools/runtime-client'

import { Button, Card } from '@heroui/react'

import { isTauriRuntime } from '../runtime/desktop-capabilities'
import { controlManagedRuntime } from '../runtime/managed-client'

import {
  downloadRuntimeStatusDiagnostic,
  readDesktopDiagnosticVersions,
} from './runtime-diagnostic-download'
import {
  createRuntimeStatusDiagnostic,
  type RuntimeDiagnosticObservation,
} from './runtime-status-diagnostic'

const modes = new Map([
  ['managed', '运行模式'],
  ['management', '仅管理模式'],
  ['validation', '验证模式'],
])

export function RuntimeStatusSettings() {
  const native = isTauriRuntime()
  const mounted = useRef(true)
  const previewGeneration = useRef(0)
  const [busy, setBusy] = useState(false)
  const [snapshot, setSnapshot] = useState<{
    status: RuntimeStatus
    checkedAt: string
  }>()
  const [message, setMessage] = useState('尚未查询 Runtime 状态。')
  const [failure, setFailure] =
    useState<Extract<RuntimeDiagnosticObservation, { state: 'unconfirmed' }>>()
  const [preview, setPreview] = useState<string>()
  const [previewBusy, setPreviewBusy] = useState(false)
  const [previewMessage, setPreviewMessage] = useState('')

  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  const query = async () => {
    if (busy || !native) return
    previewGeneration.current += 1
    setPreview(undefined)
    setPreviewMessage('')
    setPreviewBusy(false)
    setFailure(undefined)
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
      setFailure({
        state: 'unconfirmed',
        checkedAt: new Date().toISOString(),
        code: diagnostic.code,
      })
      setMessage(
        `无法确认 Runtime 当前状态。${diagnostic.code}：${diagnostic.summary}`
      )
    } finally {
      if (mounted.current) setBusy(false)
    }
  }

  const generatePreview = async () => {
    if (busy || previewBusy || !native) return
    const generation = ++previewGeneration.current
    setPreviewBusy(true)
    setPreview(undefined)
    setPreviewMessage('')
    try {
      const versions = await readDesktopDiagnosticVersions()
      const observation: RuntimeDiagnosticObservation = snapshot
        ? { state: 'observed', ...snapshot }
        : (failure ?? { state: 'not-queried' })
      const text = createRuntimeStatusDiagnostic(
        versions,
        observation,
        new Date().toISOString()
      )
      if (mounted.current && previewGeneration.current === generation)
        setPreview(text)
    } catch {
      if (mounted.current && previewGeneration.current === generation)
        setPreviewMessage(
          '无法生成诊断预览；应用版本或状态信息不可用，请重试。'
        )
    } finally {
      if (mounted.current && previewGeneration.current === generation)
        setPreviewBusy(false)
    }
  }

  const download = () => {
    if (!preview || busy || previewBusy) return
    try {
      downloadRuntimeStatusDiagnostic(preview)
      setPreviewMessage('已请求下载，请核对保存结果。')
    } catch {
      setPreviewMessage(
        '无法确认下载是否完成，请核对保存结果；可重新发起下载。'
      )
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
          <div className="flex flex-wrap gap-2">
            <Button
              isDisabled={busy || !native}
              onPress={() => void query()}
              size="sm"
              variant="secondary"
            >
              查询 Runtime 状态
            </Button>
            <Button
              isDisabled={busy || previewBusy || !native}
              onPress={() => void generatePreview()}
              size="sm"
              variant="secondary"
            >
              生成诊断预览
            </Button>
          </div>
          {preview ? (
            <section aria-label="诊断预览" className="grid min-w-0 gap-3">
              <p className="m-0 text-sm">
                请核对后下载。报告仅包含应用版本、时间与本页状态信息，不含任务内容或日志；生成预览不会重新查询
                Runtime。
              </p>
              <pre
                aria-label="诊断 JSON"
                className="m-0 max-h-72 overflow-auto text-xs break-all whitespace-pre-wrap"
                tabIndex={0}
              >
                {preview}
              </pre>
              <Button
                className="justify-self-start"
                isDisabled={busy || previewBusy}
                onPress={download}
                size="sm"
                variant="secondary"
              >
                下载诊断 JSON
              </Button>
            </section>
          ) : null}
          {previewMessage ? (
            <p aria-live="polite" className="m-0 text-sm">
              {previewMessage}
            </p>
          ) : null}
        </Card.Content>
      </Card>
    </section>
  )
}

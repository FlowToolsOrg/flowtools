import type { ToolInventoryPage } from '@flowtools/runtime-client'

import { useEffect, useRef, useState } from 'react'

import {
  decodeToolInventoryPage,
  describeRuntimeError,
  RuntimeClientError,
} from '@flowtools/runtime-client'

import { Button, Card } from '@heroui/react'

import { isTauriRuntime } from '../runtime/desktop-capabilities'
import { controlManagedRuntime } from '../runtime/managed-client'

const blockers = {
  'installed-dependent': '仍有已安装插件依赖',
  'retained-task': '任务仍保留此版本',
  'deletion-not-implemented': '尚未提供工具删除功能',
} as const
const bytes = (value: number) =>
  value < 1024
    ? `${value} B`
    : value < 1048576
      ? `${(value / 1024).toFixed(2)} KiB`
      : `${(value / 1048576).toFixed(2)} MiB`

export function ToolSettings() {
  const native = isTauriRuntime()
  const mounted = useRef(true)
  const pending = useRef(false)
  const [busy, setBusy] = useState(false)
  const [page, setPage] = useState<ToolInventoryPage>()
  const [message, setMessage] = useState('尚未查询工具库存。')
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  const query = async (next = false) => {
    if (!native || pending.current) return
    const input = {
      offset: next ? (page?.nextOffset ?? 0) : 0,
      snapshot: next ? (page?.snapshot ?? null) : null,
    }
    pending.current = true
    setBusy(true)
    setPage(undefined)
    setMessage('正在核对工具库存…')
    try {
      const result = await controlManagedRuntime(
        { method: 'tools.inventory', payload: input },
        false
      )
      if (result.type !== 'tool-inventory')
        throw new RuntimeClientError('INVALID_RESPONSE')
      const observed = decodeToolInventoryPage(result.data, input)
      if (!mounted.current) return
      setPage(observed)
      setMessage(
        observed.availability === 'not-configured'
          ? '当前 Runtime 尚未配置受管工具。'
          : `共 ${observed.total} 个工具版本；本页显示 ${observed.items.length} 个。`
      )
    } catch (error) {
      if (mounted.current) {
        const diagnostic =
          error instanceof RuntimeClientError
            ? describeRuntimeError(error)
            : undefined
        setMessage(
          diagnostic
            ? `无法核对工具库存。${diagnostic.code}：${diagnostic.summary}`
            : '无法核对工具库存，请重新查询。'
        )
      }
    } finally {
      pending.current = false
      if (mounted.current) setBusy(false)
    }
  }
  return (
    <section aria-label="受管工具">
      <Card>
        <Card.Header>
          <Card.Title>受管工具</Card.Title>
          <Card.Description>
            查看已运行 Runtime 的库存与保留原因。安装记录不代表已获执行授权。
          </Card.Description>
        </Card.Header>
        <Card.Content className="grid gap-3">
          <p className="m-0 text-sm" role="status">
            {native ? message : '请在原生 Desktop 中查询工具库存。'}
          </p>
          {page?.storage ? (
            <p className="m-0 text-sm text-muted">
              共享安装仓库：{bytes(page.storage.bytes)} /{' '}
              {bytes(page.storage.maxBytes)}，{page.storage.groups} /{' '}
              {page.storage.maxGroups} 个存储组（含插件与保留、隔离文件）。
            </p>
          ) : null}
          {page?.items.map(item => (
            <article
              key={item.archiveDigest}
              className="grid gap-2 rounded-xl border border-border p-3 text-sm"
            >
              <h4 className="m-0 font-medium break-all">
                {item.id} · {item.version}
              </h4>
              <p className="m-0 break-all">
                发布者 {item.publisher}；来源 {item.sourceRegistry}/
                {item.sourceArtifact}；归档 {bytes(item.archiveBytes)}；
                {item.materialized ? '已准备工具文件' : '仅保存归档'}。
              </p>
              <p className="m-0 break-all">
                保留任务：{item.retainedJobs}；已安装依赖插件：
                {item.dependents.length
                  ? item.dependents
                      .map(d => `${d.publisher}/${d.id}@${d.version}`)
                      .join('、')
                  : '无'}
                。
              </p>
              <p className="m-0">
                删除阻断：
                {item.deletionBlockers.map(b => blockers[b]).join('；')}。
              </p>
            </article>
          ))}
        </Card.Content>
        <Card.Footer className="flex flex-wrap gap-2">
          <Button
            isDisabled={!native || busy}
            onPress={() => {
              void query()
            }}
            variant="secondary"
          >
            {busy ? '正在查询…' : '重新查询工具'}
          </Button>
          {page?.nextOffset !== null && page?.nextOffset !== undefined ? (
            <Button
              isDisabled={busy}
              onPress={() => {
                void query(true)
              }}
              variant="secondary"
            >
              下一页
            </Button>
          ) : null}
        </Card.Footer>
      </Card>
    </section>
  )
}

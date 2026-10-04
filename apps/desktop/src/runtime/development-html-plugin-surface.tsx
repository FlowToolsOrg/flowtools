import type { HtmlPluginBridgeResponse } from './development-html-plugin-bridge'
import type { IndexedCommand } from './html-command-types'

import { useEffect, useMemo, useRef, useState } from 'react'

import { PluginMaturityBadge, PluginCompatibilityBadge } from '@flowtools/ui'
import { useNavigate } from '@tanstack/react-router'

import { Button } from '@heroui/react'

import { createDesktopRuntimeContext } from './desktop-capabilities'
import {
  loadDevelopmentHtmlFrame,
  resolveDevelopmentHtmlTarget,
} from './development-html-launch'
import {
  handleHtmlPluginBridgeRequest,
  isHtmlPluginBridgeRequest,
} from './development-html-plugin-bridge'
import { assertUnsafeHtmlPreview } from './html-development-policy'

export default function DevelopmentHtmlPluginSurface({
  command,
}: {
  command: IndexedCommand
}) {
  assertUnsafeHtmlPreview()
  const navigate = useNavigate()
  const frameRef = useRef<HTMLIFrameElement>(null)
  const [version, setVersion] = useState(0)
  const [html, setHtml] = useState<string | null>(null)
  const [failed, setFailed] = useState(false)
  const target = resolveDevelopmentHtmlTarget(command)
  const context = useMemo(
    () =>
      createDesktopRuntimeContext({
        pluginId: command.pluginId,
        pluginType: command.pluginType,
        permissions: command.permissions,
      }),
    [command.pluginId, command.pluginType, command.permissions]
  )

  useEffect(() => {
    assertUnsafeHtmlPreview()
    const close = (event: Event) => {
      const detail = (event as CustomEvent<{ pluginId?: string }>).detail
      if (detail?.pluginId && detail.pluginId !== command.pluginId) return
      void navigate({ to: '/' })
    }
    window.addEventListener('flowtools:close-panel', close)
    return () => window.removeEventListener('flowtools:close-panel', close)
  }, [command.pluginId, navigate])

  useEffect(() => {
    assertUnsafeHtmlPreview()
    let active = true
    const message = (event: MessageEvent<unknown>) => {
      const source = event.source
      if (!source || source !== frameRef.current?.contentWindow) return
      if (!isHtmlPluginBridgeRequest(event.data)) return
      const request = event.data
      void (async () => {
        const response: HtmlPluginBridgeResponse = {
          type: 'flowtools:html-plugin-response',
          id: request.id,
          ok: false,
        }
        try {
          response.value = await handleHtmlPluginBridgeRequest(context, request)
          response.ok = true
        } catch {
          response.error = 'HTML development bridge request failed'
        }
        if (active && source === frameRef.current?.contentWindow)
          (source as Window).postMessage(response, '*')
      })()
    }
    window.addEventListener('message', message)
    return () => {
      active = false
      window.removeEventListener('message', message)
    }
  }, [context])

  useEffect(() => {
    assertUnsafeHtmlPreview()
    const controller = new AbortController()
    setHtml(null)
    setFailed(false)
    if (target)
      void loadDevelopmentHtmlFrame(command, target, {
        signal: controller.signal,
      })
        .then(value => {
          if (!controller.signal.aborted) setHtml(value)
        })
        .catch(() => {
          if (!controller.signal.aborted) setFailed(true)
        })
    return () => controller.abort()
  }, [command, target, version])

  return (
    <div className="flex h-full min-h-0 flex-col bg-background">
      <div className="grid gap-2 border-b p-3">
        <p className="m-0 text-sm" role="note">
          危险开发预览：未签名 HTML / preload
          与宿主共享执行环境；没有隔离或用户授权。
          仅使用审阅过的插件与可丢弃测试数据，不代表安装或生产支持。
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <PluginMaturityBadge maturity="prototype" />
          {command.compatibilityEvidence ? (
            <PluginCompatibilityBadge
              evidence={command.compatibilityEvidence}
            />
          ) : null}
          <Button
            onPress={() => setVersion(value => value + 1)}
            size="sm"
            variant="secondary"
          >
            重新载入开发预览
          </Button>
        </div>
      </div>
      {!target ? (
        <div className="p-6">
          目录仅用于发现；未提供可解析的开发入口。请设置 VITE_HTML_PLUGIN_ROOT
          并确认静态产物。
        </div>
      ) : failed ? (
        <div className="p-6" role="alert">
          开发预览加载失败；不会回退直接打开远程页面。
        </div>
      ) : html ? (
        <iframe
          ref={frameRef}
          key={`${target}:${version}`}
          srcDoc={html}
          className="min-h-0 flex-1 border-0 bg-white"
          sandbox="allow-same-origin allow-scripts allow-forms allow-modals allow-popups allow-downloads"
          title={`${command.pluginName} - 开发预览`}
        />
      ) : (
        <div className="p-6" role="status">
          正在加载开发预览...
        </div>
      )}
    </div>
  )
}

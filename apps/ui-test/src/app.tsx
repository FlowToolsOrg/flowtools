import { useState } from 'react'

import {
  HeroSection,
  Settings,
  ToolLayout,
  ToolLayoutMain,
  ToolLayoutSidebar,
  ToolList,
  type SettingsSection,
  type ToolListItem,
} from '@flowtools/ui'

const TOOL_ITEMS: ToolListItem[] = [
  {
    id: 'clipboard-history',
    name: 'Clipboard History',
    description:
      'Track copied text snippets and search recent history instantly.',
    status: 'prototype',
    version: '1.4.2',
    tags: ['clipboard', 'productivity'],
    isInstalled: true,
    isPinned: true,
  },
  {
    id: 'hash-generator',
    name: 'Hash Generator',
    description: 'Generate SHA-256 or MD5 hashes for quick verification tasks.',
    status: 'beta',
    version: '0.9.1',
    tags: ['security', 'dev-tools'],
    isInstalled: true,
  },
  {
    id: 'image-compressor',
    name: 'Image Compressor',
    description: 'Batch optimize PNG/JPG assets before publishing.',
    status: 'experimental',
    version: '0.3.0',
    tags: ['media', 'batch'],
  },
]

export const App = () => {
  const [activeToolId, setActiveToolId] = useState<string | number>(
    'clipboard-history'
  )
  const [autoUpdate, setAutoUpdate] = useState(true)
  const [telemetry, setTelemetry] = useState(false)
  const [startupMode, setStartupMode] = useState<string | number>('workspace')
  const [logLevel, setLogLevel] = useState<string | number>('warn')

  const settingsSections: SettingsSection[] = [
    {
      id: 'runtime',
      title: 'Runtime',
      description: 'Tune host startup and plugin update behavior.',
      items: [
        {
          id: 'auto-update',
          type: 'switch',
          label: 'Auto update plugins',
          description:
            'Keep installed tools synced with latest compatible versions.',
          value: autoUpdate,
          onChange: setAutoUpdate,
        },
        {
          id: 'startup-mode',
          type: 'select',
          label: 'Startup mode',
          description: 'Choose what opens after launching the host.',
          value: startupMode,
          onChange: value => setStartupMode(value ?? 'workspace'),
          options: [
            {
              key: 'workspace',
              label: 'Workspace',
              description: 'Open the previous active workspace.',
            },
            {
              key: 'dashboard',
              label: 'Dashboard',
              description: 'Open tool discovery dashboard first.',
            },
            {
              key: 'quick-run',
              label: 'Quick run',
              description: 'Open command launcher directly.',
            },
          ],
        },
      ],
    },
    {
      id: 'diagnostics',
      title: 'Diagnostics',
      description: 'Control observability and logging outputs.',
      items: [
        {
          id: 'telemetry',
          type: 'switch',
          label: 'Enable telemetry',
          description:
            'Send anonymous diagnostics to improve plugin reliability.',
          value: telemetry,
          onChange: setTelemetry,
        },
        {
          id: 'log-level',
          type: 'select',
          label: 'Log level',
          value: logLevel,
          onChange: value => setLogLevel(value ?? 'warn'),
          options: [
            { key: 'error', label: 'Error only' },
            { key: 'warn', label: 'Warning and above' },
            { key: 'debug', label: 'Verbose debug' },
          ],
        },
      ],
    },
  ]

  return (
    <ToolLayout>
      <ToolLayoutMain>
        <HeroSection
          actions={
            <>
              <button className="rounded-xl bg-slate-900 px-4 py-2 text-sm font-medium text-white transition hover:bg-slate-800">
                Install Marketplace
              </button>
              <button className="rounded-xl border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50">
                Create Tool
              </button>
            </>
          }
          badges={['desktop-first', 'plugin-runtime', 'heroui-v3']}
          description="Composable wrappers in @flowtools/ui for plugin dashboards, discovery lists, and host settings panels."
          eyebrow="Flow Tool"
          stats={[
            { id: 'plugins', label: 'Installed Plugins', value: 12 },
            { id: 'active', label: 'Active Tool', value: String(activeToolId) },
            { id: 'latency', label: 'Runtime Latency', value: '24ms' },
            { id: 'permission', label: 'Granted Permissions', value: 5 },
          ]}
          title="Flow Tool Console"
        />
        <ToolList
          description="Browse available tools and choose one to preview in the host."
          items={TOOL_ITEMS}
          onSelectionChange={keys => {
            if (keys === 'all') {
              return
            }

            const [first] = Array.from(keys)

            if (first !== undefined) {
              setActiveToolId(first as string | number)
            }
          }}
          selectedKeys={new Set([activeToolId])}
          title="Available Tools"
        />
        <p className="text-sm text-slate-600">
          Current tool:{' '}
          <strong data-testid="active-tool" className="text-slate-900">
            {String(activeToolId)}
          </strong>
        </p>
      </ToolLayoutMain>
      <ToolLayoutSidebar>
        <Settings
          description="Settings panel wrapper with switch and select composition."
          sections={settingsSections}
        />
        <div className="rounded-2xl border border-slate-200/80 bg-white/90 p-3 text-xs text-slate-600">
          <p data-testid="settings-summary">
            Auto update: {autoUpdate ? 'on' : 'off'} | Telemetry:{' '}
            {telemetry ? 'on' : 'off'} | Startup: {String(startupMode)} | Logs:{' '}
            {String(logLevel)}
          </p>
        </div>
      </ToolLayoutSidebar>
    </ToolLayout>
  )
}

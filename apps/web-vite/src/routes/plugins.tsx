import type { RegisteredPlugin } from '@flowtools/sdk'

import { useCallback, useRef, useState } from 'react'

import {
  LayersIcon,
  LoaderPinwheelIcon,
  ShieldCheckIcon,
  XIcon,
} from '@flowtools/ui/icons'
import { createFileRoute } from '@tanstack/react-router'
import { useStore } from 'zustand'

import { Button, Chip, Spinner } from '@heroui/react'

import { pluginRegistryStore } from '@/stores/plugin-registry-store'

export const Route = createFileRoute('/plugins')({
  component: PluginsPage,
})

type LoadStatus = 'idle' | 'loading' | 'success' | 'error'

function PluginsPage() {
  const { plugins, externalPluginIds } = useStore(pluginRegistryStore)
  const inputRef = useRef<HTMLInputElement>(null)
  const [dragOver, setDragOver] = useState(false)
  const [loadStatus, setLoadStatus] = useState<LoadStatus>('idle')
  const [loadError, setLoadError] = useState<string | null>(null)
  const [loadedName, setLoadedName] = useState<string | null>(null)

  const builtInPlugins = plugins.filter(p => !externalPluginIds.includes(p.id))
  const externalPlugins = plugins.filter(p => externalPluginIds.includes(p.id))

  const handleLoadFile = useCallback(async (file: File) => {
    setLoadStatus('loading')
    setLoadError(null)
    setLoadedName(null)

    try {
      const entry = await pluginRegistryStore
        .getState()
        .loadPluginFromFile(file)
      setLoadStatus('success')
      setLoadedName(entry.manifest.name)
    } catch (err) {
      setLoadStatus('error')
      setLoadError(err instanceof Error ? err.message : 'Unknown error')
    }
  }, [])

  const handleFileInput = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0]
      if (file) {
        handleLoadFile(file)
      }
      // Reset input so same file can be re-selected
      if (inputRef.current) {
        inputRef.current.value = ''
      }
    },
    [handleLoadFile]
  )

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      setDragOver(false)
      const file = e.dataTransfer.files[0]
      if (file) {
        handleLoadFile(file)
      }
    },
    [handleLoadFile]
  )

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setDragOver(true)
  }, [])

  const handleDragLeave = useCallback(() => {
    setDragOver(false)
  }, [])

  const handleUnload = useCallback(async (pluginId: string) => {
    try {
      await pluginRegistryStore.getState().unloadExternalPlugin(pluginId)
    } catch {
      // Plugin unload failed silently
    }
  }, [])

  const handleToggle = useCallback(async (plugin: RegisteredPlugin) => {
    if (plugin.state === 'enabled') {
      await pluginRegistryStore.getState().disablePlugin(plugin.id)
    } else {
      await pluginRegistryStore.getState().enablePlugin(plugin.id)
    }
  }, [])

  const handleReload = useCallback(async (pluginId: string) => {
    await pluginRegistryStore.getState().reloadPlugin(pluginId)
  }, [])

  return (
    <div className="flex flex-col gap-6 p-6 lg:p-8">
      <header className="flex items-center gap-2.5 space-y-0">
        <LayersIcon className="text-(--accent)" size={22} />
        <div>
          <h1 className="text-2xl font-semibold text-(--foreground)">
            Plugins
          </h1>
          <p className="text-sm text-(--muted)">
            Manage and load plugins from local files.
          </p>
        </div>
      </header>

      <div className="grid min-h-0 flex-1 gap-6 xl:grid-cols-[minmax(0,1fr)_18rem]">
        <div className="flex min-w-0 flex-col gap-6">
          {/* Upload area */}
          <div
            className={`flex cursor-pointer flex-col items-center justify-center gap-3 rounded-(--radius) border-2 border-dashed p-8 transition ${
              dragOver
                ? 'border-[var(--accent)] bg-[var(--accent)]/5'
                : 'border-(--border) hover:border-[var(--accent)]/50'
            }`}
            onClick={() => inputRef.current?.click()}
            onDragLeave={handleDragLeave}
            onDragOver={handleDragOver}
            onDrop={handleDrop}
            role="button"
            tabIndex={0}
          >
            <input
              accept=".js,.mjs,.jsx,.ts,.tsx"
              className="hidden"
              onChange={handleFileInput}
              ref={inputRef}
              type="file"
            />
            {loadStatus === 'loading' ? (
              <Spinner size="sm" />
            ) : (
              <LoaderPinwheelIcon
                className={dragOver ? 'text-(--accent)' : 'text-(--muted)'}
                size={28}
              />
            )}
            <div className="text-center">
              <p className="text-sm font-medium text-(--foreground)">
                {loadStatus === 'loading'
                  ? 'Loading plugin...'
                  : 'Drop a plugin file here or click to browse'}
              </p>
              <p className="mt-1 text-xs text-(--muted)">
                Supports .js, .mjs, .jsx, .ts, and .tsx files
              </p>
            </div>
          </div>

          {/* Load status messages */}
          {loadStatus === 'success' && loadedName ? (
            <div className="flex items-center gap-2 rounded-(--radius) bg-(--accent)/10 px-4 py-2.5 text-sm text-(--accent)">
              <LayersIcon size={16} />
              Plugin "{loadedName}" loaded successfully.
            </div>
          ) : null}
          {loadStatus === 'error' && loadError ? (
            <div className="flex items-center gap-2 rounded-(--radius) bg-red-500/10 px-4 py-2.5 text-sm text-red-500">
              <XIcon size={16} />
              {loadError}
            </div>
          ) : null}

          {/* Security warning */}
          <div className="flex items-start gap-2 rounded-(--radius) border border-amber-500/30 bg-amber-500/5 px-4 py-2.5 text-xs text-amber-600">
            <ShieldCheckIcon className="mt-0.5 shrink-0" size={14} />
            <span>
              External plugins run in the main thread without sandbox isolation.
              Only load plugin files from trusted sources.
            </span>
          </div>

          {/* External plugins section */}
          {externalPlugins.length > 0 ? (
            <section className="flex flex-col gap-3">
              <h2 className="text-sm font-semibold text-(--foreground)">
                External Plugins ({externalPlugins.length})
              </h2>
              {externalPlugins.map(entry => (
                <PluginRow
                  isExternal
                  key={entry.id}
                  onReload={handleReload}
                  onToggle={handleToggle}
                  onUnload={handleUnload}
                  plugin={entry}
                />
              ))}
            </section>
          ) : null}

          {/* Built-in plugins section */}
          <section className="flex flex-col gap-3">
            <h2 className="text-sm font-semibold text-(--foreground)">
              Built-in Plugins ({builtInPlugins.length})
            </h2>
            {builtInPlugins.map(entry => (
              <PluginRow
                key={entry.id}
                onReload={handleReload}
                onToggle={handleToggle}
                plugin={entry}
              />
            ))}
          </section>
        </div>

        {/* Sidebar summary */}
        <aside className="hidden min-w-0 xl:block">
          <div className="sticky top-6 space-y-3 rounded-(--radius) border border-(--border) bg-(--surface) p-4">
            <h3 className="text-sm font-semibold text-(--foreground)">
              Plugin Summary
            </h3>
            <div className="space-y-2 text-sm">
              <div className="flex items-center justify-between">
                <span className="text-(--muted)">Total plugins</span>
                <span className="font-medium text-(--foreground)">
                  {plugins.length}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-(--muted)">Enabled</span>
                <Chip color="success" size="sm" variant="soft">
                  {plugins.filter(p => p.state === 'enabled').length}
                </Chip>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-(--muted)">Disabled</span>
                <Chip size="sm" variant="soft">
                  {plugins.filter(p => p.state === 'disabled').length}
                </Chip>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-(--muted)">External</span>
                <Chip color="secondary" size="sm" variant="soft">
                  {externalPluginIds.length}
                </Chip>
              </div>
              {plugins.filter(p => p.state === 'error').length > 0 ? (
                <div className="flex items-center justify-between">
                  <span className="text-(--muted)">Errors</span>
                  <Chip color="danger" size="sm" variant="soft">
                    {plugins.filter(p => p.state === 'error').length}
                  </Chip>
                </div>
              ) : null}
            </div>
          </div>
        </aside>
      </div>
    </div>
  )
}

interface PluginRowProps {
  plugin: RegisteredPlugin
  isExternal?: boolean
  onToggle: (plugin: RegisteredPlugin) => void
  onReload: (pluginId: string) => void
  onUnload?: (pluginId: string) => void
}

function PluginRow({
  plugin,
  isExternal,
  onToggle,
  onReload,
  onUnload,
}: PluginRowProps) {
  const { id, manifest, state } = plugin
  const isEnabled = state === 'enabled'
  const isError = state === 'error'

  const stateColorMap: Record<string, 'success' | 'danger' | 'default'> = {
    enabled: 'success',
    error: 'danger',
  }

  return (
    <div className="flex items-start gap-4 rounded-(--radius) border border-(--border) bg-(--surface) px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium text-(--foreground)">
            {manifest.name}
          </span>
          <Chip size="sm" variant="tertiary">
            {manifest.type}
          </Chip>
          <Chip
            color={stateColorMap[state] ?? 'default'}
            size="sm"
            variant="soft"
          >
            {state}
          </Chip>
          {isExternal ? (
            <Chip color="secondary" size="sm" variant="flat">
              external
            </Chip>
          ) : null}
        </div>
        {manifest.description ? (
          <p className="mt-1 text-xs text-(--muted)">{manifest.description}</p>
        ) : null}
        <p className="mt-1 text-xs text-(--muted)">
          v{manifest.version} · {id}
        </p>
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        {isError ? (
          <Button onPress={() => onReload(id)} size="sm" variant="ghost">
            Reload
          </Button>
        ) : null}
        <Button
          onPress={() => onToggle(plugin)}
          size="sm"
          variant={isEnabled ? 'ghost' : 'primary'}
        >
          {isEnabled ? 'Disable' : 'Enable'}
        </Button>
        {isExternal && onUnload ? (
          <Button
            isIconOnly
            onPress={() => onUnload(id)}
            size="sm"
            variant="ghost"
          >
            <XIcon size={14} />
          </Button>
        ) : null}
      </div>
    </div>
  )
}

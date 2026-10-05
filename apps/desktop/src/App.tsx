import type {
  CommandSource,
  IndexedCommand,
} from './runtime/html-command-types'
import type {
  AppError,
  CreatePluginDto,
  PluginDto,
  UpdatePluginDto,
} from './utils/bindings'
import type {
  AppPlugin,
  FlowToolPlugin,
  PluginManifestEntry,
} from '@flowtools/sdk'
import type {
  PluginMaturity,
  CompatibilityEvidenceStatus,
} from '@flowtools/sdk/types'
import type { ComponentType, ReactNode } from 'react'

import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useState,
} from 'react'

import { FlowToolRuntimeProvider, PluginErrorBoundary } from '@flowtools/sdk'
import {
  htmlPluginCatalogSchema,
  type HtmlCatalogPlugin,
} from '@flowtools/sdk/compat/catalog'
import { resolvePluginMaturity } from '@flowtools/sdk/types'
import { PluginMaturityBadge, PluginCompatibilityBadge } from '@flowtools/ui'
import {
  BanIcon,
  BlocksIcon,
  ClockIcon,
  CircleCheckIcon,
  LayersIcon,
  LockIcon,
  PlayIcon,
  RefreshCWIcon,
  SearchIcon,
  SettingsIcon,
  ShieldCheckIcon,
  SparklesIcon,
  TerminalIcon,
  WrenchIcon,
} from '@flowtools/ui/icons'
import { Outlet, useNavigate, useParams } from '@tanstack/react-router'

import { Button, Chip, SearchField } from '@heroui/react'

import htmlPluginIndexData from './data/html-plugin-catalog.json'
import { builtInManifests } from './plugin/manifests'
import { BuiltinExecutionPanel } from './runtime/builtin-execution-panel'
import {
  catalogPresentation,
  permissionPresentation,
} from './runtime/catalog-presentation'
import { createDesktopRuntimeContext } from './runtime/desktop-capabilities'
import { unsafeHtmlPreviewEnabled } from './runtime/html-development-policy'
import { commands as desktopCommands } from './utils/bindings'
import './App.css'

type HtmlIndexedPlugin = HtmlCatalogPlugin

interface IconProps {
  className?: string
  size?: number
}

type IconComponent = ComponentType<IconProps>

const htmlPluginIndex = htmlPluginCatalogSchema.parse(htmlPluginIndexData)
const reactPluginById = new Map(
  builtInManifests.map(manifest => [manifest.id, manifest])
)
// Vite removes this entire development import from production, including opt-in=1.
const DevelopmentHtmlPluginSurface =
  import.meta.env.DEV && unsafeHtmlPreviewEnabled
    ? lazy(() => import('./runtime/development-html-plugin-surface'))
    : null
const recentStorageKey = 'flowtools.desktop.recentCommandIds'
const ValidationRuntimePanel = import.meta.env.DEV
  ? lazy(() => import('./runtime/validation-runtime-panel'))
  : null

const builtInActions: IndexedCommand[] = [
  {
    id: 'flowtools:settings',
    title: '设置',
    description: '打开 FlowTools 设置中心',
    type: 'route',
    pluginId: 'flowtools',
    pluginName: 'FlowTools',
    category: '系统',
    pluginType: 'app',
    compatibilityLevel: 'native',
    source: 'system',
    sourceDir: undefined,
    assetDir: undefined,
    main: undefined,
    mainAvailable: undefined,
    preload: undefined,
    developmentMain: undefined,
    permissions: [],
    hasUi: true,
    hasPreload: false,
    requiresNative: false,
  },
  {
    id: 'flowtools:plugin-market',
    title: '插件市场',
    description: '浏览、安装和更新插件',
    type: 'route',
    pluginId: 'flowtools',
    pluginName: 'FlowTools',
    category: '插件',
    pluginType: 'app',
    compatibilityLevel: 'native',
    source: 'system',
    sourceDir: undefined,
    assetDir: undefined,
    main: undefined,
    mainAvailable: undefined,
    preload: undefined,
    developmentMain: undefined,
    permissions: [],
    hasUi: true,
    hasPreload: false,
    requiresNative: false,
  },
  {
    id: 'flowtools:permissions',
    title: '权限中心',
    description: '查看插件权限和安全隔离状态',
    type: 'route',
    pluginId: 'flowtools',
    pluginName: 'FlowTools',
    category: '安全',
    pluginType: 'app',
    compatibilityLevel: 'native',
    source: 'system',
    sourceDir: undefined,
    assetDir: undefined,
    main: undefined,
    mainAvailable: undefined,
    preload: undefined,
    developmentMain: undefined,
    permissions: [],
    hasUi: true,
    hasPreload: false,
    requiresNative: false,
  },
]

const selectedClass = 'bg-[var(--active-bg)]'
const hoverClass = 'hover:bg-[var(--hover-bg)]'

function toHtmlCommandIndex(plugin: HtmlIndexedPlugin): IndexedCommand[] {
  const base = {
    pluginId: plugin.id,
    pluginName: plugin.name,
    category: plugin.category,
    pluginType: plugin.type,
    compatibilityLevel: plugin.html.compatibility.level,
    maturity: plugin.maturity,
    compatibilityEvidence: plugin.evidence.status,
    source: 'html' as const,
    sourceDir: plugin.html.sourceDir,
    assetDir: plugin.html.assetDir,
    main: plugin.html.main,
    mainAvailable: plugin.html.mainAvailable,
    preload: plugin.html.preload,
    developmentMain: undefined,
    permissions: plugin.permissions,
    hasUi: Boolean(plugin.html.mainAvailable && plugin.html.main),
    hasPreload: Boolean(plugin.html.preload),
    requiresNative: plugin.html.compatibility.level === 'native-bridge',
  } satisfies Omit<
    IndexedCommand,
    'id' | 'title' | 'description' | 'type' | 'featureCode'
  >

  if (plugin.html.commands.length === 0) {
    return [
      {
        ...base,
        id: `${plugin.id}:open`,
        title: plugin.name,
        description: plugin.description,
        type: 'open',
      },
    ]
  }

  return plugin.html.commands.map(command => ({
    ...base,
    id: `${plugin.id}:${command.id}`,
    title: command.title,
    description: command.description ?? plugin.description,
    featureCode: command.featureCode,
    type: command.type,
    requiresNative:
      base.requiresNative || ['files', 'img', 'window'].includes(command.type),
  }))
}

function toReactCommandIndex(manifest: PluginManifestEntry): IndexedCommand {
  return {
    id: `${manifest.id}:open`,
    title: manifest.name,
    description: manifest.description,
    type: manifest.type === 'app' ? 'panel' : 'headless',
    pluginId: manifest.id,
    pluginName: manifest.name,
    category: manifest.category,
    pluginType: manifest.type,
    compatibilityLevel: 'sdk',
    maturity: resolvePluginMaturity(manifest.maturity),
    source: 'react',
    permissions: manifest.permissions ?? [],
    hasUi: manifest.type === 'app',
    hasPreload: false,
    requiresNative: Boolean(manifest.permissions?.includes('native')),
  }
}

const marketplaceCommands = [
  ...builtInManifests.map(toReactCommandIndex),
  ...htmlPluginIndex.plugins.flatMap(toHtmlCommandIndex),
]

const allCommands = [...builtInActions, ...marketplaceCommands]
const commandById = new Map(allCommands.map(command => [command.id, command]))
const totalPluginCount =
  builtInManifests.length + htmlPluginIndex.totals.plugins
const totalCommandCount = marketplaceCommands.length

const pinnedIds = new Set([
  'flowtools:settings',
  'flowtools:plugin-market',
  'json-editor:json:text:json:0',
  '2048:2048:text:2048:0',
])

function scoreCommand(command: IndexedCommand, query: string): number {
  const title = command.title.toLowerCase()
  const pluginName = command.pluginName.toLowerCase()
  const category = command.category?.toLowerCase() ?? ''
  const description = command.description?.toLowerCase() ?? ''

  let score = 0

  if (title === query) score += 100
  if (title.startsWith(query)) score += 48
  if (title.includes(query)) score += 28
  if (pluginName.includes(query)) score += 14
  if (category.includes(query)) score += 8
  if (description.includes(query)) score += 4

  return score
}

function getInitials(value: string): string {
  return (Array.from(value.trim())[0] ?? 'F').toUpperCase()
}

function getCommandIcon(command: IndexedCommand): IconComponent {
  if (command.pluginId === 'flowtools') return SparklesIcon
  if (command.requiresNative) return TerminalIcon
  if (command.hasPreload) return LayersIcon
  if (command.pluginType === 'tool') return WrenchIcon
  return BlocksIcon
}

function getCommand(commandId: string): IndexedCommand | undefined {
  return commandById.get(commandId)
}

function readRecentCommandIds(): string[] {
  try {
    const raw = window.localStorage.getItem(recentStorageKey)
    if (!raw) return []

    const value = JSON.parse(raw) as unknown
    if (!Array.isArray(value)) return []

    return value.filter(item => typeof item === 'string')
  } catch {
    return []
  }
}

function writeRecentCommandIds(commandIds: string[]): void {
  window.localStorage.setItem(recentStorageKey, JSON.stringify(commandIds))
}

function getRunLabel(command: IndexedCommand): string {
  if (command.pluginId === 'flowtools') return '打开'
  return command.hasUi ? '启动插件' : '运行命令'
}

function getPrimaryCommandForPlugin(
  pluginId: string
): IndexedCommand | undefined {
  return allCommands.find(command => command.pluginId === pluginId)
}

type PluginCatalogSource = 'react' | 'html'
type PluginLifecycleStage = 'available' | 'downloaded' | 'installed' | 'enabled'
type PluginActionKind =
  | 'install'
  | 'enable'
  | 'enable-and-open'
  | 'disable'
  | 'remove'

interface CommandResult<T> {
  status: 'ok' | 'error'
  data?: T
  error?: AppError
}

interface PluginListItem {
  id: string
  name: string
  version: string
  description?: string
  type: 'app' | 'tool'
  source: PluginCatalogSource
  permissions: readonly string[]
  tags: readonly string[]
  category?: string
  cliAvailable: boolean
  compatibilityLevel: string
  maturity: PluginMaturity
  compatibilityEvidence?: CompatibilityEvidenceStatus
  commandCount: number
  hasUi: boolean
}

const pluginListItems: PluginListItem[] = [
  ...builtInManifests.map(manifest => ({
    id: manifest.id,
    name: manifest.name,
    version: manifest.version,
    description: manifest.description,
    type: manifest.type,
    source: 'react' as const,
    permissions: manifest.permissions ?? [],
    tags: manifest.tags ?? [],
    category: manifest.category,
    cliAvailable: manifest.cliAvailable ?? false,
    compatibilityLevel: 'sdk',
    maturity: resolvePluginMaturity(manifest.maturity),
    commandCount: 1,
    hasUi: manifest.type === 'app',
  })),
  ...htmlPluginIndex.plugins.map(plugin => ({
    id: plugin.id,
    name: plugin.name,
    version: plugin.version,
    description: plugin.description,
    type: plugin.type,
    source: 'html' as const,
    permissions: plugin.permissions,
    tags: [],
    category: plugin.category,
    cliAvailable: false,
    compatibilityLevel: plugin.html.compatibility.level,
    maturity: plugin.maturity,
    compatibilityEvidence: plugin.evidence.status,
    commandCount: Math.max(plugin.html.commands.length, 1),
    hasUi: Boolean(plugin.html.mainAvailable && plugin.html.main),
  })),
]

function getSupportLabel(reference: {
  source: CommandSource
  compatibilityLevel: string
}): string {
  return reference.source === 'html'
    ? `桥接需求：${reference.compatibilityLevel}`
    : reference.source === 'react'
      ? '内置 SDK'
      : '宿主路由'
}

function CommandEvidence({ command }: { command: IndexedCommand }) {
  return (
    <>
      <PluginMaturityBadge maturity={command.maturity} />
      {command.compatibilityEvidence ? (
        <PluginCompatibilityBadge evidence={command.compatibilityEvidence} />
      ) : null}
    </>
  )
}

function getAppErrorMessage(error: AppError | undefined): string {
  return error?.message ?? '插件数据库操作失败'
}

function assertCommandData<T>(result: CommandResult<T>): T {
  if (result.status === 'ok') return result.data as T

  throw new Error(getAppErrorMessage(result.error))
}

function getPluginStage(plugin: PluginDto | undefined): PluginLifecycleStage {
  if (!plugin) return 'available'
  if (plugin.state === 'enabled') return 'enabled'
  if (plugin.status === 'downloaded' || plugin.state === 'registered') {
    return 'downloaded'
  }

  return 'installed'
}

function getPluginStageLabel(stage: PluginLifecycleStage): string {
  return catalogPresentation('react', stage, true).stageLabel
}

function toCreatePluginDto(
  plugin: PluginListItem,
  state: string,
  status: string
): CreatePluginDto {
  return {
    id: plugin.id,
    name: plugin.name,
    version: plugin.version,
    description: plugin.description ?? null,
    author: null,
    link: null,
    type: plugin.type,
    permissions: Array.from(plugin.permissions),
    tags: Array.from(plugin.tags),
    status,
    category: plugin.category ?? null,
    icon: null,
    cliAvailable: plugin.cliAvailable,
    state,
  }
}

function createEmptyPluginUpdate(): UpdatePluginDto {
  return {
    name: null,
    version: null,
    description: null,
    author: null,
    link: null,
    type: null,
    permissions: null,
    tags: null,
    status: null,
    category: null,
    icon: null,
    cliAvailable: null,
    state: null,
  }
}

function toInstallPluginDto(): UpdatePluginDto {
  return {
    ...createEmptyPluginUpdate(),
    status: 'installed',
    state: 'disabled',
  }
}

function usePluginInventory() {
  const [plugins, setPlugins] = useState<PluginDto[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setIsLoading(true)
    setError(null)

    try {
      const result = await desktopCommands.getPlugins()
      setPlugins(assertCommandData(result))
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError.message : '无法读取插件配置状态'
      )
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const pluginById = useMemo(
    () => new Map(plugins.map(plugin => [plugin.id, plugin])),
    [plugins]
  )

  const enabledPluginIds = useMemo(
    () =>
      new Set(
        plugins
          .filter(plugin => plugin.state === 'enabled')
          .map(plugin => plugin.id)
      ),
    [plugins]
  )

  return {
    enabledPluginIds,
    error,
    isLoading,
    pluginById,
    plugins,
    refresh,
  }
}

function RootLayout() {
  const params = new URLSearchParams(window.location.search)
  return (
    <>
      <Outlet />
      {ValidationRuntimePanel &&
        params.get('execution-validation') === '20261003' &&
        params.get('runtime-validation') === '1' && (
          <Suspense fallback={<p>Connecting Runtime</p>}>
            <ValidationRuntimePanel />
          </Suspense>
        )}
    </>
  )
}

function LauncherView() {
  const navigate = useNavigate()
  const inventory = usePluginInventory()
  const [query, setQuery] = useState('')
  const [selectedIndex, setSelectedIndex] = useState(0)
  const [recentCommandIds, setRecentCommandIds] = useState(readRecentCommandIds)
  const normalizedQuery = query.trim().toLowerCase()
  const launcherCommands = useMemo(
    () =>
      allCommands.filter(
        command =>
          command.pluginId === 'flowtools' ||
          inventory.enabledPluginIds.has(command.pluginId)
      ),
    [inventory.enabledPluginIds]
  )
  const launcherCommandById = useMemo(
    () => new Map(launcherCommands.map(command => [command.id, command])),
    [launcherCommands]
  )

  const searchedCommands = useMemo(() => {
    if (!normalizedQuery) return []

    return launcherCommands
      .map(command => ({
        command,
        score: scoreCommand(command, normalizedQuery),
      }))
      .filter(item => item.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, 36)
      .map(item => item.command)
  }, [launcherCommands, normalizedQuery])

  const pinnedCommands = useMemo(
    () =>
      launcherCommands.filter(command => pinnedIds.has(command.id)).slice(0, 8),
    [launcherCommands]
  )

  const recentCommands = useMemo(() => {
    const storedCommands = recentCommandIds
      .map(commandId => launcherCommandById.get(commandId))
      .filter((command): command is IndexedCommand => Boolean(command))
    const storedIds = new Set(storedCommands.map(command => command.id))
    const fallbackCommands = launcherCommands
      .filter(command => !pinnedIds.has(command.id))
      .filter(command => !storedIds.has(command.id))
      .slice(0, 48)

    return [...storedCommands, ...fallbackCommands].slice(0, 56)
  }, [launcherCommandById, launcherCommands, recentCommandIds])

  const recommendedCommands = useMemo(() => {
    const visibleIds = new Set(
      [...pinnedCommands, ...recentCommands].map(command => command.id)
    )

    return launcherCommands
      .filter(command => command.hasUi && !command.requiresNative)
      .filter(command => command.pluginId !== 'flowtools')
      .filter(command => !visibleIds.has(command.id))
      .slice(0, 20)
  }, [launcherCommands, pinnedCommands, recentCommands])

  const activeCommands = normalizedQuery
    ? searchedCommands
    : [...pinnedCommands, ...recentCommands, ...recommendedCommands]
  const selectedCommand = activeCommands[selectedIndex] ?? activeCommands[0]

  useEffect(() => {
    setSelectedIndex(index =>
      activeCommands.length === 0
        ? 0
        : Math.min(index, activeCommands.length - 1)
    )
  }, [activeCommands.length])

  const markRecent = (command: IndexedCommand) => {
    const nextIds = [
      command.id,
      ...recentCommandIds.filter(commandId => commandId !== command.id),
    ].slice(0, 40)

    setRecentCommandIds(nextIds)
    writeRecentCommandIds(nextIds)
  }

  const openCommand = (command: IndexedCommand) => {
    markRecent(command)

    if (command.id === 'flowtools:settings') {
      void navigate({ to: '/settings' })
      return
    }

    if (command.id === 'flowtools:plugin-market') {
      void navigate({ to: '/plugins' })
      return
    }

    if (command.id === 'flowtools:permissions') {
      void navigate({ to: '/permissions' })
      return
    }

    void navigate({
      to: '/run/$commandId',
      params: { commandId: command.id },
    })
  }

  const handleQueryChange = (value: string) => {
    setQuery(value)
    setSelectedIndex(0)
  }

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault()
      setSelectedIndex(index => {
        if (activeCommands.length === 0) return 0
        return Math.min(index + 1, activeCommands.length - 1)
      })
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault()
      setSelectedIndex(index => Math.max(index - 1, 0))
    }

    if (event.key === 'Enter' && selectedCommand) {
      event.preventDefault()
      openCommand(selectedCommand)
    }

    if (event.key === 'Escape') {
      setQuery('')
      setSelectedIndex(0)
    }
  }

  return (
    <DesktopSurface>
      <section className="flex h-[min(720px,calc(100vh-36px))] w-[min(920px,calc(100vw-36px))] flex-col overflow-hidden rounded-[10px] border border-(--border-color) bg-[color-mix(in_srgb,var(--bg-color)_92%,transparent)] shadow-(--window-shadow) backdrop-blur-[18px]">
        <div className="grid grid-cols-[42px_minmax(0,1fr)_34px] items-center gap-2.5 border-b border-(--divider-color) p-3">
          <button
            className="grid size-9 place-items-center rounded-lg bg-(--primary-gradient) text-base font-extrabold text-(--text-on-primary)"
            onClick={() => void navigate({ to: '/' })}
            type="button"
          >
            F
          </button>
          <SearchField
            aria-label="搜索应用、插件、命令或输入内容"
            autoFocus
            className="min-w-0"
            fullWidth
            name="launcher-search"
            onChange={handleQueryChange}
            value={query}
            variant="secondary"
          >
            <SearchField.Group className="grid h-10.5 min-w-0 grid-cols-[24px_minmax(0,1fr)_28px] items-center gap-2.5 rounded-lg border border-transparent bg-(--control-bg) px-3 text-(--text-secondary) focus-within:border-[color-mix(in_srgb,var(--primary-color)_42%,transparent)] focus-within:bg-[color-mix(in_srgb,var(--primary-color)_7%,white)] focus-within:dark:bg-[color-mix(in_srgb,var(--primary-color)_8%,#303133)]">
              <SearchField.SearchIcon className="text-(--text-secondary)">
                <SearchIcon size={22} />
              </SearchField.SearchIcon>
              <SearchField.Input
                className="min-w-0 border-0 bg-transparent p-0 text-xl leading-none text-(--text-color) shadow-none outline-none placeholder:text-[color-mix(in_srgb,var(--text-secondary)_72%,transparent)]"
                onKeyDown={handleKeyDown}
                placeholder="搜索应用、插件、命令或输入内容"
              />
              <SearchField.ClearButton className="grid size-7 place-items-center rounded-md text-(--text-secondary) hover:bg-(--hover-bg)" />
            </SearchField.Group>
          </SearchField>
          <Button
            aria-label="设置"
            className="size-8.5 text-(--text-secondary)"
            isIconOnly
            onPress={() => void navigate({ to: '/settings' })}
            size="sm"
            variant="ghost"
          >
            <SettingsIcon size={18} />
          </Button>
        </div>

        <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_268px] overflow-hidden max-[780px]:grid-cols-1">
          <div className="min-h-0 min-w-0 overflow-y-auto px-3 pt-2.5 pb-3">
            {normalizedQuery ? (
              <CommandSection
                commands={searchedCommands}
                emptyText="没有找到匹配的命令"
                selectedCommandId={selectedCommand?.id}
                title="最佳匹配"
                onFocusCommand={setSelectedIndex}
                onRunCommand={openCommand}
              />
            ) : (
              <>
                <IconGridSection
                  commands={pinnedCommands}
                  selectedCommandId={selectedCommand?.id}
                  title="固定"
                  onFocusCommand={setSelectedIndex}
                  onRunCommand={openCommand}
                />
                <CommandSection
                  commands={recentCommands}
                  offset={pinnedCommands.length}
                  scrollable
                  selectedCommandId={selectedCommand?.id}
                  title="最近使用"
                  onFocusCommand={setSelectedIndex}
                  onRunCommand={openCommand}
                />
                <CommandSection
                  commands={recommendedCommands}
                  offset={pinnedCommands.length + recentCommands.length}
                  emptyText={
                    inventory.isLoading
                      ? '正在同步插件状态...'
                      : '还没有启用插件，请先到插件市场安装并启用'
                  }
                  selectedCommandId={selectedCommand?.id}
                  title="推荐插件"
                  onFocusCommand={setSelectedIndex}
                  onRunCommand={openCommand}
                />
              </>
            )}
          </div>

          <aside className="min-h-0 min-w-0 border-l border-(--divider-color) bg-[color-mix(in_srgb,var(--control-bg)_70%,transparent)] max-[780px]:hidden">
            {selectedCommand ? (
              <CommandInspector command={selectedCommand} onRun={openCommand} />
            ) : null}
          </aside>
        </div>

        <footer className="flex justify-between gap-3 border-t border-(--divider-color) px-3 py-2 text-[11px] text-(--text-secondary) max-[780px]:flex-col">
          <span>
            {inventory.enabledPluginIds.size}/{totalPluginCount} plugins enabled
            · {totalCommandCount} commands
          </span>
          <span>{inventory.error ?? 'Alt+Z 唤起 · Enter 运行 · Esc 清空'}</span>
        </footer>
      </section>
    </DesktopSurface>
  )
}

interface IconGridSectionProps {
  title: string
  commands: IndexedCommand[]
  selectedCommandId?: string
  onFocusCommand: (index: number) => void
  onRunCommand: (command: IndexedCommand) => void
}

function IconGridSection({
  title,
  commands,
  selectedCommandId,
  onFocusCommand,
  onRunCommand,
}: IconGridSectionProps) {
  return (
    <section className="mb-3">
      <SectionHeading title={title} />
      <div className="grid grid-cols-8 gap-0.5 max-[780px]:grid-cols-4">
        {commands.map((command, index) => (
          <Button
            className={`h-20.5 min-w-0 flex-col gap-1.5 rounded-lg bg-transparent px-1 py-2 text-(--text-color) ${hoverClass} ${
              command.id === selectedCommandId ? selectedClass : ''
            }`}
            key={command.id}
            onFocus={() => onFocusCommand(index)}
            onMouseEnter={() => onFocusCommand(index)}
            onPress={() => onRunCommand(command)}
            variant="ghost"
          >
            <CommandIcon command={command} />
            <span className="line-clamp-2 h-8 w-full overflow-hidden px-1 text-center text-xs leading-4 font-medium break-all">
              {command.title}
            </span>
          </Button>
        ))}
      </div>
    </section>
  )
}

interface CommandSectionProps {
  title: string
  commands: IndexedCommand[]
  selectedCommandId?: string
  offset?: number
  emptyText?: string
  scrollable?: boolean
  onFocusCommand: (index: number) => void
  onRunCommand: (command: IndexedCommand) => void
}

function CommandSection({
  title,
  commands,
  selectedCommandId,
  offset = 0,
  emptyText,
  scrollable = false,
  onFocusCommand,
  onRunCommand,
}: CommandSectionProps) {
  return (
    <section className="mb-3">
      <SectionHeading title={title} />
      {commands.length === 0 ? (
        <div className="px-3 py-8 text-center text-[13px] text-(--text-secondary)">
          {emptyText}
        </div>
      ) : (
        <div
          className={`flex flex-col gap-0.5 ${
            scrollable ? 'max-h-[224px] overflow-y-auto pr-1' : ''
          }`}
        >
          {commands.map((command, index) => (
            <Button
              className={`grid min-h-13.5 w-full grid-cols-[38px_minmax(0,1fr)_auto] justify-normal gap-2.5 rounded-lg bg-transparent px-2 py-2 text-left text-(--text-color) ${hoverClass} ${
                command.id === selectedCommandId ? selectedClass : ''
              }`}
              key={command.id}
              onFocus={() => onFocusCommand(offset + index)}
              onMouseEnter={() => onFocusCommand(offset + index)}
              onPress={() => onRunCommand(command)}
              variant="ghost"
            >
              <CommandIcon command={command} />
              <span className="min-w-0">
                <strong className="block truncate text-sm font-semibold text-(--text-color)">
                  {command.title}
                </strong>
                <span className="mt-0.5 block truncate text-xs text-(--text-secondary)">
                  {command.description ?? command.pluginName}
                </span>
              </span>
              <span className="max-w-32 truncate text-xs text-(--text-secondary) max-[780px]:hidden">
                {command.pluginName}
              </span>
            </Button>
          ))}
        </div>
      )}
    </section>
  )
}

function SectionHeading({ title }: { title: string }) {
  return (
    <div className="flex h-7 items-center text-xs font-semibold text-(--text-secondary)">
      {title}
    </div>
  )
}

function CommandIcon({ command }: { command: IndexedCommand }) {
  const Icon = getCommandIcon(command)

  return (
    <span className="relative grid size-8 shrink-0 place-items-center rounded-[7px] bg-(--primary-gradient) text-(--text-on-primary)">
      <Icon className="opacity-90" size={18} />
      <span className="absolute -right-1 -bottom-1 grid h-4 min-w-4 place-items-center rounded-full border border-(--bg-color) bg-(--bg-color) px-1 text-[9px] font-extrabold text-(--text-color) dark:bg-surface-secondary dark:text-(--text-on-primary)">
        {getInitials(command.pluginName)}
      </span>
    </span>
  )
}

interface CommandInspectorProps {
  command: IndexedCommand
  onRun: (command: IndexedCommand) => void
}

function CommandInspector({ command, onRun }: CommandInspectorProps) {
  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4.5">
      <CommandIcon command={command} />
      <h2 className="m-0 text-lg leading-tight text-(--text-color)">
        {command.title}
      </h2>
      <p className="m-0 text-[13px] leading-relaxed text-(--text-secondary)">
        {command.description ??
          '该命令来自插件声明，可由 FlowTools runtime 承载。'}
      </p>

      <div className="flex flex-wrap gap-1.5">
        <CapabilityChip icon={PlayIcon}>
          {command.hasUi ? 'UI 插件' : '无界面'}
        </CapabilityChip>
        <CapabilityChip icon={ShieldCheckIcon}>
          {command.requiresNative ? '原生权限' : '安全沙箱'}
        </CapabilityChip>
        <CapabilityChip icon={LockIcon}>
          {command.hasPreload ? 'API Bridge' : '声明式插件'}
        </CapabilityChip>
        <CapabilityChip icon={ClockIcon}>历史记录</CapabilityChip>
      </div>

      <Button
        className="mt-1 h-9.5 w-full"
        onPress={() => onRun(command)}
        size="sm"
      >
        <PlayIcon size={17} />
        {getRunLabel(command)}
      </Button>

      <dl className="mt-auto grid grid-cols-[72px_minmax(0,1fr)] gap-x-2.5 gap-y-2 border-t border-(--divider-color) pt-3 text-xs">
        <dt className="text-(--text-secondary)">插件</dt>
        <dd className="m-0 truncate font-semibold text-(--text-color)">
          {command.pluginName}
        </dd>
        <dt className="text-(--text-secondary)">分类</dt>
        <dd className="m-0 truncate font-semibold text-(--text-color)">
          {command.category ?? '未分类'}
        </dd>
        <dt className="text-(--text-secondary)">触发类型</dt>
        <dd className="m-0 truncate font-semibold text-(--text-color)">
          {command.type}
        </dd>
      </dl>
    </div>
  )
}

interface CapabilityChipProps {
  icon: IconComponent
  children: string
}

function CapabilityChip({ icon: Icon, children }: CapabilityChipProps) {
  return (
    <Chip className="gap-1" color="accent" size="sm" variant="soft">
      <Icon size={14} />
      <Chip.Label>{children}</Chip.Label>
    </Chip>
  )
}

interface DesktopSurfaceProps {
  children: ReactNode
}

function DesktopSurface({ children }: DesktopSurfaceProps) {
  return (
    <main className="grid h-screen w-screen place-items-center overflow-hidden bg-[radial-gradient(circle_at_24%_20%,rgba(2,132,199,0.12),transparent_32%),radial-gradient(circle_at_76%_88%,rgba(102,126,234,0.1),transparent_30%),#dce4ea] p-4.5 text-(--text-color) dark:bg-[radial-gradient(circle_at_24%_20%,rgba(56,189,248,0.1),transparent_32%),radial-gradient(circle_at_76%_88%,rgba(167,139,250,0.09),transparent_30%),#151719]">
      {children}
    </main>
  )
}

interface PageFrameProps {
  title: string
  description?: string
  children: ReactNode
  actions?: ReactNode
  contentClassName?: string
}

function PageFrame({
  title,
  description,
  children,
  actions,
  contentClassName = 'overflow-y-auto p-4',
}: PageFrameProps) {
  const navigate = useNavigate()

  return (
    <DesktopSurface>
      <section className="flex h-[min(720px,calc(100vh-36px))] w-[min(920px,calc(100vw-36px))] flex-col overflow-hidden rounded-[10px] border border-(--border-color) bg-[color-mix(in_srgb,var(--bg-color)_92%,transparent)] shadow-(--window-shadow) backdrop-blur-[18px]">
        <header className="grid grid-cols-[34px_minmax(0,1fr)_auto] items-center gap-3 border-b border-(--divider-color) p-3">
          <Button
            aria-label="返回启动器"
            className="size-8.5"
            isIconOnly
            onPress={() => void navigate({ to: '/' })}
            size="sm"
            variant="ghost"
          >
            <SearchIcon size={17} />
          </Button>
          <div className="min-w-0">
            <h1 className="m-0 truncate text-base font-semibold text-(--text-color)">
              {title}
            </h1>
            {description ? (
              <p className="m-0 mt-0.5 truncate text-xs text-(--text-secondary)">
                {description}
              </p>
            ) : null}
          </div>
          <div className="flex items-center gap-2">{actions}</div>
        </header>
        <div className={`min-h-0 flex-1 ${contentClassName}`}>{children}</div>
      </section>
    </DesktopSurface>
  )
}

function SettingsView() {
  const navigate = useNavigate()
  const [appearance, setAppearance] = useState('system')

  useEffect(() => {
    if (appearance === 'system') {
      document.documentElement.removeAttribute('data-theme')
      return
    }

    document.documentElement.dataset.theme = appearance
  }, [appearance])

  return (
    <PageFrame
      actions={
        <Button
          onPress={() => void navigate({ to: '/permissions' })}
          size="sm"
          variant="secondary"
        >
          <ShieldCheckIcon size={16} />
          权限
        </Button>
      }
      description="桌面宿主、外观、运行时与插件安全"
      title="设置"
    >
      <div className="grid gap-4">
        <SettingsRow
          label="外观"
          value={
            <SegmentedOptions
              options={['system', 'light', 'dark']}
              selected={appearance}
              titleMap={{ dark: '深色', light: '浅色', system: '系统' }}
              onSelect={setAppearance}
            />
          }
        />
        <SettingsRow label="启动快捷键" value="Alt+Z" />
        <SettingsRow label="命令路由" value="TanStack Router" />
        <SettingsRow label="插件 UI" value="React 声明式运行时" />
        <SettingsRow label="桌面内核" value="Tauri WebView + Rust 能力层" />
        <SettingsRow
          label="HTML 插件目录"
          value={`${htmlPluginIndex.totals.plugins} 个插件`}
        />
      </div>
    </PageFrame>
  )
}

interface SegmentedOptionsProps {
  options: string[]
  selected: string
  titleMap: Record<string, string>
  onSelect: (value: string) => void
}

function SegmentedOptions({
  options,
  selected,
  titleMap,
  onSelect,
}: SegmentedOptionsProps) {
  return (
    <div className="flex rounded-lg border border-(--divider-color) p-0.5">
      {options.map(option => (
        <Button
          className={`h-7 px-2.5 text-xs ${
            selected === option ? 'bg-[var(--active-bg)]' : 'bg-transparent'
          }`}
          key={option}
          onPress={() => onSelect(option)}
          size="sm"
          variant="ghost"
        >
          {titleMap[option]}
        </Button>
      ))}
    </div>
  )
}

interface SettingsRowProps {
  label: string
  value: ReactNode
}

function SettingsRow({ label, value }: SettingsRowProps) {
  return (
    <div className="grid min-h-13 grid-cols-[160px_minmax(0,1fr)] items-center gap-4 border-b border-(--divider-color) py-2 text-sm max-[640px]:grid-cols-1">
      <span className="text-(--text-secondary)">{label}</span>
      <div className="min-w-0 font-medium text-(--text-color)">{value}</div>
    </div>
  )
}

function PluginsView() {
  const navigate = useNavigate()
  const inventory = usePluginInventory()
  const [busyAction, setBusyAction] = useState<{
    pluginId: string
    action: PluginActionKind
  } | null>(null)
  const [notice, setNotice] = useState<string | null>(null)

  const openPlugin = (pluginId: string) => {
    const command = getPrimaryCommandForPlugin(pluginId)
    if (!command) return

    void navigate({
      to: '/run/$commandId',
      params: { commandId: command.id },
    })
  }

  const runPluginAction = async (
    plugin: PluginListItem,
    action: PluginActionKind,
    task: () => Promise<{ openAfterRefresh?: boolean } | void>
  ) => {
    setBusyAction({ pluginId: plugin.id, action })
    setNotice(null)

    try {
      const result = await task()
      await inventory.refresh()

      const actionLabel = {
        disable: '已禁用',
        enable: '已启用',
        'enable-and-open': '已启用，正在启动',
        install: '配置已保存',
        remove: '配置记录已移除',
      } satisfies Record<PluginActionKind, string>

      setNotice(`${plugin.name} ${actionLabel[action]}`)

      if (result?.openAfterRefresh) {
        openPlugin(plugin.id)
      }
    } catch (actionError) {
      setNotice(
        actionError instanceof Error ? actionError.message : '插件操作失败'
      )
    } finally {
      setBusyAction(null)
    }
  }

  const installPlugin = async (
    plugin: PluginListItem,
    stage: PluginLifecycleStage
  ) => {
    if (stage === 'available') {
      assertCommandData(
        await desktopCommands.addPlugin(
          toCreatePluginDto(plugin, 'disabled', 'installed')
        )
      )
      return
    }

    if (stage === 'downloaded') {
      assertCommandData(
        await desktopCommands.updatePlugin(plugin.id, toInstallPluginDto())
      )
    }
  }

  const handlePrimaryAction = (plugin: PluginListItem) => {
    if (plugin.source === 'html') return
    const installedPlugin = inventory.pluginById.get(plugin.id)
    const stage = getPluginStage(installedPlugin)

    if (stage === 'enabled') {
      openPlugin(plugin.id)
      return
    }

    if (stage === 'available' || stage === 'downloaded') {
      void runPluginAction(plugin, 'install', async () => {
        await installPlugin(plugin, stage)
      })
      return
    }

    if (plugin.hasUi) {
      void runPluginAction(plugin, 'enable-and-open', async () => {
        assertCommandData(await desktopCommands.enablePlugin(plugin.id))
        return { openAfterRefresh: true }
      })
      return
    }

    void runPluginAction(plugin, 'enable', async () => {
      assertCommandData(await desktopCommands.enablePlugin(plugin.id))
    })
  }

  const handleDisablePlugin = (plugin: PluginListItem) => {
    void runPluginAction(plugin, 'disable', async () => {
      assertCommandData(await desktopCommands.disablePlugin(plugin.id))
    })
  }

  const handleRemovePlugin = (plugin: PluginListItem) => {
    void runPluginAction(plugin, 'remove', async () => {
      assertCommandData(await desktopCommands.removePlugin(plugin.id))
    })
  }

  return (
    <PageFrame
      actions={
        <Button
          isPending={inventory.isLoading}
          onPress={() => void inventory.refresh()}
          size="sm"
          variant="secondary"
        >
          <RefreshCWIcon size={16} />
          刷新
        </Button>
      }
      description={`${totalPluginCount} 个插件 · ${totalCommandCount} 个命令`}
      title="插件市场"
    >
      <div className="grid gap-3">
        <p
          className="m-0 text-xs text-(--text-secondary)"
          data-testid="catalog-evidence-notice"
        >
          成熟度与兼容证据独立；indexed 仅索引，entry-resolved
          仅找到入口文件，桥接需求不代表 API/安全认证或授权。
          第三方包安装尚未实现；内置工具只保存启用配置，旧目录记录不能授权执行。
        </p>
        {notice || inventory.error ? (
          <div className="rounded-lg border border-(--divider-color) bg-(--control-bg) px-3 py-2 text-sm text-(--text-secondary)">
            {notice ?? inventory.error}
          </div>
        ) : null}
        {pluginListItems.slice(0, 96).map(plugin => (
          <PluginMarketRow
            busyAction={busyAction}
            key={plugin.id}
            plugin={plugin}
            stage={getPluginStage(inventory.pluginById.get(plugin.id))}
            onDisable={handleDisablePlugin}
            onPrimaryAction={handlePrimaryAction}
            onRemove={handleRemovePlugin}
          />
        ))}
      </div>
    </PageFrame>
  )
}

interface PluginMarketRowProps {
  plugin: PluginListItem
  stage: PluginLifecycleStage
  busyAction: { pluginId: string; action: PluginActionKind } | null
  onPrimaryAction: (plugin: PluginListItem) => void
  onDisable: (plugin: PluginListItem) => void
  onRemove: (plugin: PluginListItem) => void
}

function PluginMarketRow({
  plugin,
  stage,
  busyAction,
  onPrimaryAction,
  onDisable,
  onRemove,
}: PluginMarketRowProps) {
  const isBusy = busyAction?.pluginId === plugin.id
  const presentation = catalogPresentation(plugin.source, stage, plugin.hasUi)

  return (
    <div className="grid min-h-18 w-full grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-3 rounded-lg border border-transparent px-2 py-2 text-left text-(--text-color) hover:border-(--divider-color) hover:bg-(--hover-bg) max-[720px]:grid-cols-[40px_minmax(0,1fr)]">
      <span className="grid size-10 place-items-center rounded-[7px] bg-(--primary-gradient) text-xs font-extrabold text-(--text-on-primary)">
        {getInitials(plugin.name)}
      </span>
      <span className="min-w-0">
        <span className="flex min-w-0 flex-wrap items-center gap-1.5">
          <strong className="truncate text-sm">{plugin.name}</strong>
          <Chip color="accent" size="sm" variant="soft">
            {plugin.source === 'react' ? 'React' : 'HTML'}
          </Chip>
          <PluginMaturityBadge maturity={plugin.maturity} />
          {plugin.compatibilityEvidence ? (
            <PluginCompatibilityBadge evidence={plugin.compatibilityEvidence} />
          ) : null}
          <Chip
            color={stage === 'available' ? 'warning' : 'accent'}
            size="sm"
            variant="soft"
          >
            {presentation.stageLabel}
          </Chip>
        </span>
        <span className="mt-1 block truncate text-xs text-(--text-secondary)">
          {plugin.description ?? plugin.id}
        </span>
        <span className="mt-1 flex flex-wrap gap-1.5 text-[11px] text-(--text-secondary)">
          <span>{getSupportLabel(plugin)}</span>
          <span>{plugin.commandCount} 个命令</span>
          <span>{plugin.permissions.length} 项能力声明</span>
          <span>{plugin.version}</span>
        </span>
      </span>
      <span className="flex items-center gap-1.5 max-[720px]:col-span-2 max-[720px]:justify-end">
        {stage === 'enabled' ? (
          <Button
            isPending={isBusy && busyAction?.action === 'disable'}
            onPress={() => onDisable(plugin)}
            size="sm"
            variant="secondary"
          >
            <BanIcon size={15} />
            禁用
          </Button>
        ) : null}
        {stage !== 'available' ? (
          <Button
            isPending={isBusy && busyAction?.action === 'remove'}
            onPress={() => onRemove(plugin)}
            size="sm"
            variant="ghost"
          >
            移除配置记录
          </Button>
        ) : null}
        <Button
          isPending={
            isBusy &&
            ['install', 'enable', 'enable-and-open'].includes(
              busyAction?.action ?? ''
            )
          }
          isDisabled={!presentation.canExecute}
          onPress={() => onPrimaryAction(plugin)}
          size="sm"
        >
          {stage === 'enabled' || (stage === 'installed' && plugin.hasUi) ? (
            <PlayIcon size={15} />
          ) : stage === 'installed' ? (
            <CircleCheckIcon size={15} />
          ) : (
            <LayersIcon size={15} />
          )}
          {presentation.primaryLabel}
        </Button>
      </span>
    </div>
  )
}

function PermissionsView() {
  const permissionRows = permissionPresentation

  return (
    <PageFrame
      description="能力声明与当前限制；插件级授权和隔离尚未实现"
      title="权限中心"
    >
      <div className="grid gap-2">
        {permissionRows.map(([name, description, status]) => (
          <div
            className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-(--divider-color) py-3"
            key={name}
          >
            <span className="min-w-0">
              <strong className="block text-sm">{name}</strong>
              <span className="block truncate text-xs text-(--text-secondary)">
                {description}
              </span>
            </span>
            <Chip
              color={status === '高风险' ? 'warning' : 'accent'}
              size="sm"
              variant="soft"
            >
              {status}
            </Chip>
          </div>
        ))}
      </div>
    </PageFrame>
  )
}

function CommandRunView() {
  const navigate = useNavigate()
  const { commandId } = useParams({ from: '/run/$commandId' })
  const command = getCommand(commandId)
  const inventory = usePluginInventory()
  const [isEnabling, setIsEnabling] = useState(false)

  if (!command) {
    return (
      <PageFrame title="命令不存在">
        <div className="grid place-items-center py-16 text-sm text-(--text-secondary)">
          未找到这个命令入口
        </div>
      </PageFrame>
    )
  }

  // Catalog presence and saved enabled metadata never authorize external code.
  if (command.source === 'html' && !unsafeHtmlPreviewEnabled) {
    return (
      <PageFrame title="外部插件执行已禁用" description={command.pluginName}>
        <div
          className="grid gap-3 p-6 text-sm text-(--text-secondary)"
          role="note"
        >
          <code>EXTERNAL_CODE_DISABLED</code>
          <p>HTML / Legacy 仅供显式开发预览；生产包不执行外部代码。</p>
          <p>
            目录证据和已保存的启用状态不等于签名、隔离或授权；原元数据保留。
          </p>
          <p>
            受控开发需 DEV + VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW=1 和指定本地
            checkout。
          </p>
        </div>
      </PageFrame>
    )
  }

  const installedPlugin = inventory.pluginById.get(command.pluginId)
  const stage = getPluginStage(installedPlugin)
  const requiresPluginActivation = command.pluginId !== 'flowtools'

  const enablePluginFromRun = async () => {
    setIsEnabling(true)

    try {
      assertCommandData(await desktopCommands.enablePlugin(command.pluginId))
      await inventory.refresh()
    } finally {
      setIsEnabling(false)
    }
  }

  if (requiresPluginActivation && inventory.isLoading) {
    return (
      <PageFrame title="正在检查插件状态">
        <div className="grid h-full place-items-center p-6 text-sm text-(--text-secondary)">
          正在同步插件配置状态...
        </div>
      </PageFrame>
    )
  }

  if (requiresPluginActivation && inventory.error) {
    return (
      <PluginActivationGate
        command={command}
        message={inventory.error}
        stage={stage}
        onEnable={enablePluginFromRun}
        onOpenMarket={() => void navigate({ to: '/plugins' })}
      />
    )
  }

  if (requiresPluginActivation && stage !== 'enabled') {
    return (
      <PluginActivationGate
        command={command}
        isEnabling={isEnabling}
        stage={stage}
        onEnable={enablePluginFromRun}
        onOpenMarket={() => void navigate({ to: '/plugins' })}
      />
    )
  }

  return (
    <PageFrame
      actions={
        <Button
          onPress={() => void navigate({ to: '/plugins' })}
          size="sm"
          variant="secondary"
        >
          <LayersIcon size={16} />
          插件
        </Button>
      }
      contentClassName="overflow-hidden p-0"
      description={command.pluginName}
      title={command.title}
    >
      {command.source === 'react' ? (
        <ReactPluginSurface command={command} />
      ) : command.source === 'html' && DevelopmentHtmlPluginSurface ? (
        <Suspense fallback={<div className="p-6">正在加载开发预览...</div>}>
          <DevelopmentHtmlPluginSurface command={command} />
        </Suspense>
      ) : (
        <HeadlessCommandSurface command={command} />
      )}
    </PageFrame>
  )
}

interface PluginActivationGateProps {
  command: IndexedCommand
  stage: PluginLifecycleStage
  message?: string
  isEnabling?: boolean
  onEnable: () => void
  onOpenMarket: () => void
}

function PluginActivationGate({
  command,
  stage,
  message,
  isEnabling = false,
  onEnable,
  onOpenMarket,
}: PluginActivationGateProps) {
  const title =
    stage === 'available'
      ? '内置工具尚未保存配置'
      : stage === 'downloaded'
        ? '内置工具配置待保存'
        : '插件尚未启用'

  return (
    <PageFrame
      actions={
        <Button onPress={onOpenMarket} size="sm" variant="secondary">
          <LayersIcon size={16} />
          插件市场
        </Button>
      }
      title={title}
    >
      <div className="grid h-full place-items-center p-6">
        <div className="grid max-w-120 gap-4 text-center">
          <div className="mx-auto">
            <CommandIcon command={command} />
          </div>
          <div>
            <h2 className="m-0 text-base font-semibold text-(--text-color)">
              {command.pluginName}
            </h2>
            <p className="m-0 mt-2 text-sm leading-relaxed text-(--text-secondary)">
              {message ??
                '内置工具需要先保存并启用配置，才会被桌面运行器加载。此操作不安装第三方包或授予持久权限。'}
            </p>
          </div>
          <div className="flex justify-center gap-1.5">
            <CapabilityChip icon={LayersIcon}>
              {getPluginStageLabel(stage)}
            </CapabilityChip>
            <CommandEvidence command={command} />
            <CapabilityChip icon={BlocksIcon}>
              {getSupportLabel(command)}
            </CapabilityChip>
          </div>
          <div className="flex justify-center gap-2">
            {stage === 'installed' ? (
              <Button isPending={isEnabling} onPress={onEnable} size="sm">
                {command.hasUi ? (
                  <PlayIcon size={16} />
                ) : (
                  <CircleCheckIcon size={16} />
                )}
                {command.hasUi ? '启用并启动' : '启用插件'}
              </Button>
            ) : null}
            <Button onPress={onOpenMarket} size="sm" variant="secondary">
              管理插件
            </Button>
          </div>
        </div>
      </div>
    </PageFrame>
  )
}

function isAppPlugin(plugin: FlowToolPlugin): plugin is AppPlugin {
  return plugin.type === 'app'
}

function ReactPluginSurface({ command }: { command: IndexedCommand }) {
  const manifest = reactPluginById.get(command.pluginId)
  const [plugin, setPlugin] = useState<FlowToolPlugin | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!manifest) return

    let cancelled = false
    setPlugin(null)
    setError(null)

    void manifest
      .loader()
      .then(module => {
        if (cancelled) return
        setPlugin(module.default)
      })
      .catch((loadError: unknown) => {
        if (cancelled) return
        setError(
          loadError instanceof Error
            ? loadError.message
            : 'Failed to load React plugin'
        )
      })

    return () => {
      cancelled = true
    }
  }, [manifest])

  if (!manifest) {
    return <HeadlessCommandSurface command={command} />
  }

  if (error) {
    return (
      <div className="grid h-full place-items-center p-6 text-sm text-(--text-secondary)">
        {error}
      </div>
    )
  }

  if (!plugin) {
    return (
      <div className="grid h-full place-items-center p-6 text-sm text-(--text-secondary)">
        正在加载插件...
      </div>
    )
  }

  if (!isAppPlugin(plugin)) {
    return (
      <div className="h-full overflow-auto p-4">
        <BuiltinExecutionPanel key={plugin.meta.id} plugin={plugin} />
      </div>
    )
  }

  return <ReactAppPluginPanel plugin={plugin} />
}

function ReactAppPluginPanel({ plugin }: { plugin: AppPlugin }) {
  const Panel = useMemo(() => plugin.setup(), [plugin])
  const runtimeContext = useMemo(
    () =>
      createDesktopRuntimeContext({
        pluginId: plugin.meta.id,
        pluginType: plugin.type,
        permissions: plugin.meta.permissions,
        storeShape: plugin.store,
      }),
    [plugin]
  )

  return (
    <div className="h-full min-h-0 overflow-auto bg-(--bg-color) p-4">
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <PluginMaturityBadge maturity={plugin.meta.maturity} />
        <span className="text-xs text-muted">
          内置 SDK · 成熟度不等于授权或兼容认证
        </span>
      </div>
      <PluginErrorBoundary pluginId={plugin.meta.id}>
        <FlowToolRuntimeProvider value={runtimeContext}>
          <Panel />
        </FlowToolRuntimeProvider>
      </PluginErrorBoundary>
      <div className="mt-4">
        <BuiltinExecutionPanel key={plugin.meta.id} plugin={plugin} />
      </div>
    </div>
  )
}

function HeadlessCommandSurface({ command }: { command: IndexedCommand }) {
  const message =
    command.main && command.mainAvailable === false
      ? command.developmentMain
        ? '这个 HTML 插件的静态 main 尚不可直接运行，请先启动插件自己的 development.main 服务。'
        : '这个 HTML 插件的静态 main 尚不可直接运行，请先构建插件产物后刷新 HTML 插件目录。'
      : '这个入口没有声明 UI main，后续会接入 FlowTools headless runner 或 Tauri 原生能力执行。'

  return (
    <div className="grid h-full place-items-center p-6">
      <div className="grid max-w-130 gap-4 text-center">
        <div className="mx-auto">
          <CommandIcon command={command} />
        </div>
        <div>
          <h2 className="m-0 text-base font-semibold text-(--text-color)">
            命令已触发
          </h2>
          <p className="m-0 mt-2 text-sm leading-relaxed text-(--text-secondary)">
            {message}
          </p>
        </div>
        <div className="flex justify-center gap-1.5">
          <CapabilityChip icon={PlayIcon}>{command.type}</CapabilityChip>
          <CommandEvidence command={command} />
          <CapabilityChip icon={BlocksIcon}>
            {getSupportLabel(command)}
          </CapabilityChip>
        </div>
      </div>
    </div>
  )
}

export {
  CommandRunView,
  LauncherView,
  PermissionsView,
  PluginsView,
  RootLayout,
  SettingsView,
}

export default RootLayout

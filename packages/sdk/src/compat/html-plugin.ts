import type { Permission } from '../types/permissions'

export type HtmlPluginCommandType =
  | 'text'
  | 'regex'
  | 'over'
  | 'img'
  | 'files'
  | 'window'

export interface HtmlPluginCommandObject {
  type?: string
  label?: string
  match?: string | Record<string, unknown>
  minLength?: number
  maxLength?: number
  fileType?: string
  extensions?: string[]
}

export type HtmlPluginCommandMatcher = string | HtmlPluginCommandObject

export interface HtmlPluginFeatureManifest {
  code: string
  explain?: string
  icon?: string
  mainPush?: boolean
  mainHide?: boolean
  cmds?: HtmlPluginCommandMatcher[]
}

export interface HtmlPluginManifest {
  $schema?: string
  name?: string
  title?: string
  description?: string
  version?: string
  author?: string
  homepage?: string | null
  main?: string
  preload?: string
  logo?: string
  development?: {
    main?: string
  }
  pluginSetting?: {
    single?: boolean
    height?: number
    backgroundRunning?: boolean
  }
  features?: HtmlPluginFeatureManifest[]
}

export interface HtmlPluginCommandDescriptor {
  id: string
  title: string
  description?: string
  featureCode: string
  type: string
  matcher: HtmlPluginCommandMatcher
  icon?: string
}

export type HtmlPluginCompatibilityLevel =
  | 'metadata'
  | 'webview'
  | 'preload-bridge'
  | 'native-bridge'

export type HtmlPluginCompatibilityEffort = 'low' | 'medium' | 'high'

export interface FlowToolsHtmlPluginManifest {
  id: string
  name: string
  version: string
  description?: string
  author?: string
  link?: string
  type: 'app' | 'tool'
  permissions: Permission[]
  tags: string[]
  category?: string
  icon?: string
  html: {
    sourceDir?: string
    assetDir?: string
    main?: string
    mainAvailable?: boolean
    preload?: string
    developmentMain?: string
    commands: HtmlPluginCommandDescriptor[]
    features: HtmlPluginFeatureManifest[]
    compatibility: {
      level: HtmlPluginCompatibilityLevel
      effort: HtmlPluginCompatibilityEffort
      notes: string[]
      unsupportedCommandTypes: string[]
    }
  }
}

export interface NormalizeHtmlPluginOptions {
  sourceDir?: string
  assetDir?: string
  mainAvailable?: boolean
  category?: string
}

const nativeCommandTypes = new Set(['files', 'window', 'img'])

function kebabCaseId(value: string): string {
  const normalized = value
    .trim()
    .replace(/([a-z0-9])([A-Z])/g, '$1-$2')
    .replace(/[^a-zA-Z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .toLowerCase()

  return normalized || 'html-plugin'
}

function uniqueStrings(values: Iterable<string | undefined>): string[] {
  const seen = new Set<string>()
  const next: string[] = []

  for (const value of values) {
    const trimmed = value?.trim()
    if (!trimmed || seen.has(trimmed)) {
      continue
    }
    seen.add(trimmed)
    next.push(trimmed)
  }

  return next
}

function getCommandLabel(cmd: HtmlPluginCommandMatcher): string | undefined {
  if (typeof cmd === 'string') {
    return cmd
  }

  return cmd.label
}

function getCommandType(cmd: HtmlPluginCommandMatcher): string {
  if (typeof cmd === 'string') {
    return 'text'
  }

  return cmd.type ?? 'text'
}

function commandIdPart(value: string): string {
  return kebabCaseId(value).replace(/^html-plugin$/, 'command')
}

function getPermissions(
  manifest: HtmlPluginManifest,
  commands: HtmlPluginCommandDescriptor[]
): Permission[] {
  const permissions = new Set<Permission>(['storage'])

  if (manifest.preload) {
    permissions.add('native')
    permissions.add('clipboard')
    permissions.add('fs')
    permissions.add('network')
    permissions.add('notification')
    permissions.add('dialog')
    permissions.add('db')
  }

  if (commands.some(command => command.type === 'files')) {
    permissions.add('fs')
  }

  if (commands.some(command => command.type === 'img')) {
    permissions.add('clipboard')
  }

  if (commands.some(command => command.type === 'window')) {
    permissions.add('native')
  }

  return [...permissions]
}

function getCompatibility(
  manifest: HtmlPluginManifest,
  commands: HtmlPluginCommandDescriptor[]
): FlowToolsHtmlPluginManifest['html']['compatibility'] {
  const unsupportedCommandTypes = uniqueStrings(
    commands
      .map(command => command.type)
      .filter(type => type !== 'text' && type !== 'regex' && type !== 'over')
  )
  const notes: string[] = []
  let level: HtmlPluginCompatibilityLevel = 'metadata'
  let effort: HtmlPluginCompatibilityEffort = 'low'

  if (manifest.main || manifest.development?.main) {
    level = 'webview'
    notes.push('UI 插件可先通过 Tauri WebView 壳承载 main 入口。')
  } else {
    notes.push('无 main 入口，适合迁移为 FlowTools headless/tool 插件。')
  }

  if (manifest.preload) {
    level = 'preload-bridge'
    effort = 'medium'
    notes.push('存在 preload，需要实现旧版宿主 API bridge。')
  }

  if (commands.some(command => nativeCommandTypes.has(command.type))) {
    level = 'native-bridge'
    effort = 'high'
    notes.push('包含文件、图片或窗口类命令，需要 Tauri 原生能力适配。')
  }

  if (unsupportedCommandTypes.length > 0) {
    notes.push(`需要适配命令类型：${unsupportedCommandTypes.join(', ')}。`)
  }

  return {
    level,
    effort,
    notes,
    unsupportedCommandTypes,
  }
}

export function getHtmlPluginDisplayName(manifest: HtmlPluginManifest): string {
  return manifest.title ?? manifest.name ?? 'HTML Plugin'
}

export function getHtmlPluginId(
  manifest: HtmlPluginManifest,
  sourceDir?: string
): string {
  return kebabCaseId(manifest.name ?? sourceDir ?? manifest.title ?? '')
}

export function getHtmlPluginCommands(
  manifest: HtmlPluginManifest
): HtmlPluginCommandDescriptor[] {
  const commands: HtmlPluginCommandDescriptor[] = []

  for (const feature of manifest.features ?? []) {
    const featureTitle = feature.explain ?? feature.code

    for (const [index, cmd] of (feature.cmds ?? []).entries()) {
      const label = getCommandLabel(cmd) ?? featureTitle
      const type = getCommandType(cmd)
      commands.push({
        id: [
          commandIdPart(feature.code),
          commandIdPart(type),
          commandIdPart(label),
          String(index),
        ].join(':'),
        title: label,
        description: feature.explain,
        featureCode: feature.code,
        type,
        matcher: cmd,
        icon: feature.icon,
      })
    }
  }

  return commands
}

export function normalizeHtmlPluginManifest(
  manifest: HtmlPluginManifest,
  options: NormalizeHtmlPluginOptions = {}
): FlowToolsHtmlPluginManifest {
  const commands = getHtmlPluginCommands(manifest)
  const id = getHtmlPluginId(manifest, options.sourceDir)
  const tags = uniqueStrings([
    id,
    manifest.name,
    manifest.title,
    ...(manifest.features ?? []).flatMap(feature => [
      feature.code,
      feature.explain,
      ...(feature.cmds ?? []).map(getCommandLabel),
    ]),
  ])

  return {
    id,
    name: getHtmlPluginDisplayName(manifest),
    version: manifest.version ?? '0.0.0',
    description: manifest.description,
    author: manifest.author,
    link: manifest.homepage ?? undefined,
    type: manifest.main || manifest.development?.main ? 'app' : 'tool',
    permissions: getPermissions(manifest, commands),
    tags,
    category: options.category,
    icon: manifest.logo,
    html: {
      sourceDir: options.sourceDir,
      assetDir: options.assetDir,
      main: manifest.main,
      mainAvailable: options.mainAvailable,
      preload: manifest.preload,
      developmentMain: manifest.development?.main,
      commands,
      features: manifest.features ?? [],
      compatibility: getCompatibility(manifest, commands),
    },
  }
}

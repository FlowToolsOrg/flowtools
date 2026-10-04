import { z } from 'zod'

import { permissions } from '../types/permissions'

/** Portable package paths, not OS paths or URLs. Native authorization is separate. */
export const portablePluginPathSchema = z
  .string()
  .min(1)
  .refine(value => {
    if (
      /[\\:%?#<>"|*]/.test(value) ||
      value.startsWith('/') ||
      value.split('').some(character => character.charCodeAt(0) < 32)
    )
      return false
    return value
      .split('/')
      .every(
        part =>
          part !== '' &&
          part !== '.' &&
          part !== '..' &&
          !/[. ]$/.test(part) &&
          !/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part)
      )
  }, 'Expected a canonical package-relative path')

const sha256 = z.string().regex(/^[a-f0-9]{64}$/)
const id = z.string().regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/)
const matcher = z.union([
  z.string(),
  z.object({
    type: z.string().optional(),
    label: z.string().optional(),
    match: z.union([z.string(), z.record(z.string(), z.unknown())]).optional(),
    minLength: z.number().optional(),
    maxLength: z.number().optional(),
    fileType: z.string().optional(),
    extensions: z.array(z.string()).optional(),
  }),
])
const feature = z.object({
  code: z.string(),
  explain: z.string().optional(),
  icon: z.string().optional(),
  mainPush: z.boolean().optional(),
  mainHide: z.boolean().optional(),
  cmds: z.array(matcher).optional(),
})
const compatibility = z.strictObject({
  level: z.enum(['metadata', 'webview', 'preload-bridge', 'native-bridge']),
  effort: z.enum(['low', 'medium', 'high']),
  notes: z.array(z.string()),
  unsupportedCommandTypes: z.array(z.string()),
})
const evidence = z.discriminatedUnion('status', [
  z.strictObject({
    status: z.literal('indexed'),
    reason: z.enum(['absent', 'missing', 'unsafe', 'source-only', 'remote']),
  }),
  z.strictObject({
    status: z.literal('entry-resolved'),
    fixtureId: z.literal('static-entry-v1'),
    path: portablePluginPathSchema,
    sha256,
  }),
])

export const htmlCatalogPluginSchema = z
  .strictObject({
    id,
    name: z.string().min(1),
    version: z.string().min(1),
    maturity: z.literal('prototype'),
    description: z.string().optional(),
    author: z.string().optional(),
    link: z.string().optional(),
    type: z.enum(['app', 'tool']),
    permissions: z.array(z.enum(permissions)),
    tags: z.array(z.string()),
    category: z.string().optional(),
    icon: z.string().optional(),
    package: z.strictObject({
      kind: z.literal('legacy-html'),
      id,
      root: portablePluginPathSchema,
      manifestSha256: sha256,
    }),
    evidence,
    html: z.strictObject({
      sourceDir: portablePluginPathSchema,
      assetDir: portablePluginPathSchema.optional(),
      main: portablePluginPathSchema.optional(),
      mainAvailable: z.boolean(),
      preload: portablePluginPathSchema.optional(),
      hasDevelopmentMain: z.boolean(),
      commands: z.array(
        z.strictObject({
          id: z.string(),
          title: z.string(),
          description: z.string().optional(),
          featureCode: z.string(),
          type: z.string(),
          matcher,
          icon: z.string().optional(),
        })
      ),
      features: z.array(feature),
      compatibility,
    }),
  })
  .superRefine((plugin, ctx) => {
    const reject = (message: string) =>
      ctx.addIssue({ code: 'custom', message })
    if (
      plugin.package.id !== plugin.id ||
      plugin.package.root !== `plugins/${plugin.html.sourceDir}`
    )
      reject('Package identity/path mismatch')
    if (
      new Set(plugin.html.commands.map(command => command.id)).size !==
      plugin.html.commands.length
    )
      reject('Duplicate command identity')
    if (
      plugin.html.mainAvailable !==
      (plugin.evidence.status === 'entry-resolved')
    )
      reject('Entry availability requires scoped evidence')
    if (plugin.evidence.status === 'entry-resolved') {
      const expected = [plugin.html.assetDir, plugin.html.main]
        .filter(Boolean)
        .join('/')
      if (!plugin.html.main || plugin.evidence.path !== expected)
        reject('Entry evidence/path mismatch')
    }
  })

export const htmlPluginCatalogSchema = z
  .strictObject({
    formatVersion: z.literal(1),
    source: z.literal('legacy-html-checkout'),
    fixtures: z
      .array(
        z.strictObject({
          id: z.literal('static-entry-v1'),
          path: z.literal('scripts/fixtures/html-catalog/static-entry-v1.json'),
          sha256,
        })
      )
      .max(1),
    totals: z.strictObject({
      plugins: z.number().int().nonnegative(),
      commands: z.number().int().nonnegative(),
      appPlugins: z.number().int().nonnegative(),
      toolPlugins: z.number().int().nonnegative(),
    }),
    byCompatibility: z.record(z.string(), z.number().int().nonnegative()),
    byCategory: z.record(z.string(), z.number().int().nonnegative()),
    plugins: z.array(htmlCatalogPluginSchema),
  })
  .superRefine((catalog, ctx) => {
    const reject = (message: string) =>
      ctx.addIssue({ code: 'custom', message })
    if (
      new Set(catalog.plugins.map(plugin => plugin.id)).size !==
      catalog.plugins.length
    )
      reject('Duplicate plugin identity')
    if (
      catalog.totals.plugins !== catalog.plugins.length ||
      catalog.totals.commands !==
        catalog.plugins.reduce(
          (count, plugin) => count + plugin.html.commands.length,
          0
        ) ||
      catalog.totals.appPlugins !==
        catalog.plugins.filter(plugin => plugin.type === 'app').length ||
      catalog.totals.toolPlugins !==
        catalog.plugins.filter(plugin => plugin.type === 'tool').length
    )
      reject('Catalog totals mismatch')
    for (const plugin of catalog.plugins) {
      const evidence = plugin.evidence
      if (
        evidence.status === 'entry-resolved' &&
        !catalog.fixtures.some(fixture => fixture.id === evidence.fixtureId)
      )
        reject('Missing evidence fixture')
    }
    for (const [bucket, key] of [
      [
        catalog.byCompatibility,
        (plugin: HtmlCatalogPlugin) => plugin.html.compatibility.level,
      ],
      [
        catalog.byCategory,
        (plugin: HtmlCatalogPlugin) => plugin.category ?? '未分类',
      ],
    ] as const) {
      const actual: Record<string, number> = {}
      for (const plugin of catalog.plugins) {
        const name = key(plugin)
        actual[name] = (actual[name] ?? 0) + 1
      }
      if (
        Object.keys(bucket).length !== Object.keys(actual).length ||
        Object.entries(actual).some(([name, count]) => bucket[name] !== count)
      )
        reject('Catalog classification counts mismatch')
    }
  })

export type HtmlCatalogPlugin = z.infer<typeof htmlCatalogPluginSchema>
export type HtmlPluginCatalog = z.infer<typeof htmlPluginCatalogSchema>

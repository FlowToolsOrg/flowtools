import type {
  HtmlPluginCommandObject,
  HtmlPluginManifest,
} from '../src/compat/html-plugin'

import { describe, expect, test } from 'bun:test'

import {
  getHtmlPluginCommands,
  getHtmlPluginDisplayName,
  getHtmlPluginId,
  normalizeHtmlPluginManifest,
} from '../src/compat/html-plugin'

const sorted = (values: readonly string[]): string[] => [...values].sort()

describe('HTML plugin identity', () => {
  test('normalizes names to stable kebab-case ids', () => {
    expect(getHtmlPluginId({ name: 'MyUseful Plugin_v2' })).toBe(
      'my-useful-plugin-v2'
    )
    expect(getHtmlPluginId({}, 'Fallback Folder')).toBe('fallback-folder')
    expect(getHtmlPluginId({})).toBe('html-plugin')
  })

  test('prefers title, then name, for display names', () => {
    expect(
      getHtmlPluginDisplayName({ title: 'Display', name: 'package-name' })
    ).toBe('Display')
    expect(getHtmlPluginDisplayName({ name: 'package-name' })).toBe(
      'package-name'
    )
    expect(getHtmlPluginDisplayName({})).toBe('HTML Plugin')
  })
})

describe('HTML plugin commands', () => {
  test('normalizes string and object matchers deterministically', () => {
    const regexCommand: HtmlPluginCommandObject = {
      type: 'regex',
      label: 'Match Code',
      match: '^code:',
    }
    const manifest: HtmlPluginManifest = {
      features: [
        {
          code: 'Search Tools',
          explain: 'Search tools',
          icon: 'search.png',
          cmds: ['keyword', regexCommand, { label: 'Plain Text' }],
        },
      ],
    }

    const commands = getHtmlPluginCommands(manifest)

    expect(commands).toEqual([
      {
        id: 'search-tools:text:keyword:0',
        title: 'keyword',
        description: 'Search tools',
        featureCode: 'Search Tools',
        type: 'text',
        matcher: 'keyword',
        icon: 'search.png',
      },
      {
        id: 'search-tools:regex:match-code:1',
        title: 'Match Code',
        description: 'Search tools',
        featureCode: 'Search Tools',
        type: 'regex',
        matcher: regexCommand,
        icon: 'search.png',
      },
      {
        id: 'search-tools:text:plain-text:2',
        title: 'Plain Text',
        description: 'Search tools',
        featureCode: 'Search Tools',
        type: 'text',
        matcher: { label: 'Plain Text' },
        icon: 'search.png',
      },
    ])
  })

  test('uses command indexes to keep repeated labels unique', () => {
    const commands = getHtmlPluginCommands({
      features: [
        {
          code: 'repeat',
          cmds: ['same', 'same'],
        },
      ],
    })

    expect(commands.map(command => command.id)).toEqual([
      'repeat:text:same:0',
      'repeat:text:same:1',
    ])
  })

  test('keeps repeated feature codes and labels discoverable with unique command ids', () => {
    const manifest = {
      features: [
        { code: 'same', cmds: ['same'] },
        { code: 'same', cmds: ['same'] },
      ],
    }
    const commands = getHtmlPluginCommands(manifest)
    expect(commands).toHaveLength(2)
    expect(new Set(commands.map(command => command.id)).size).toBe(2)
    expect(getHtmlPluginCommands(manifest)).toEqual(commands)
  })
})

describe('HTML plugin compatibility normalization', () => {
  test('keeps plugins without an entry point at metadata support', () => {
    const normalized = normalizeHtmlPluginManifest({
      name: 'metadata-only',
      version: '1.0.0',
    })

    expect(normalized.type).toBe('tool')
    expect(normalized.permissions).toEqual(['storage'])
    expect(normalized.html.compatibility.level).toBe('metadata')
    expect(normalized.html.compatibility.effort).toBe('low')
  })

  test('recognizes main and development entries as webview apps', () => {
    const mainPlugin = normalizeHtmlPluginManifest(
      { name: 'main-plugin', main: 'index.html' },
      { assetDir: 'dist/main-plugin', mainAvailable: true }
    )
    const developmentPlugin = normalizeHtmlPluginManifest({
      name: 'development-plugin',
      development: { main: 'http://localhost:5173' },
    })

    expect(mainPlugin.type).toBe('app')
    expect(mainPlugin.html.main).toBe('index.html')
    expect(mainPlugin.html.mainAvailable).toBe(true)
    expect(mainPlugin.html.assetDir).toBe('dist/main-plugin')
    expect(mainPlugin.html.compatibility.level).toBe('webview')

    expect(developmentPlugin.type).toBe('app')
    expect(developmentPlugin.html.developmentMain).toBe('http://localhost:5173')
    expect(developmentPlugin.html.compatibility.level).toBe('webview')
  })

  test('marks preload plugins and records their requested capabilities', () => {
    const normalized = normalizeHtmlPluginManifest({
      name: 'preload-plugin',
      main: 'index.html',
      preload: 'preload.js',
    })

    expect(normalized.html.compatibility.level).toBe('preload-bridge')
    expect(normalized.html.compatibility.effort).toBe('medium')
    expect(sorted(normalized.permissions)).toEqual(
      sorted([
        'storage',
        'native',
        'clipboard',
        'fs',
        'network',
        'notification',
        'dialog',
        'db',
      ])
    )
  })

  test('promotes native command types above preload support', () => {
    const normalized = normalizeHtmlPluginManifest({
      name: 'native-plugin',
      main: 'index.html',
      preload: 'preload.js',
      features: [
        {
          code: 'select-file',
          cmds: [{ type: 'files', label: 'Select file' }],
        },
      ],
    })

    expect(normalized.html.compatibility.level).toBe('native-bridge')
    expect(normalized.html.compatibility.effort).toBe('high')
    expect(normalized.html.compatibility.unsupportedCommandTypes).toEqual([
      'files',
    ])
    expect(normalized.permissions).toContain('fs')
  })

  test('maps native command types to requested capabilities', () => {
    const cases = [
      { type: 'files', permission: 'fs' },
      { type: 'img', permission: 'clipboard' },
      { type: 'window', permission: 'native' },
    ] as const

    for (const item of cases) {
      const normalized = normalizeHtmlPluginManifest({
        name: `${item.type}-plugin`,
        features: [
          {
            code: item.type,
            cmds: [{ type: item.type, label: item.type }],
          },
        ],
      })

      expect(normalized.permissions).toContain('storage')
      expect(normalized.permissions).toContain(item.permission)
      expect(normalized.html.compatibility.level).toBe('native-bridge')
    }
  })

  test('preserves catalog metadata and deduplicates tags', () => {
    const normalized = normalizeHtmlPluginManifest(
      {
        name: 'tag-plugin',
        title: 'Tag Plugin',
        version: '2.0.0',
        homepage: null,
        logo: 'logo.png',
        features: [
          {
            code: 'tag-plugin',
            explain: 'Tag Plugin',
            cmds: ['Tag Plugin'],
          },
        ],
      },
      {
        sourceDir: 'plugins/tag-plugin',
        category: 'developer',
      }
    )

    expect(normalized.id).toBe('tag-plugin')
    expect(normalized.name).toBe('Tag Plugin')
    expect(normalized.version).toBe('2.0.0')
    expect(normalized.link).toBeUndefined()
    expect(normalized.icon).toBe('logo.png')
    expect(normalized.category).toBe('developer')
    expect(normalized.html.sourceDir).toBe('plugins/tag-plugin')
    expect(new Set(normalized.tags).size).toBe(normalized.tags.length)
  })
})

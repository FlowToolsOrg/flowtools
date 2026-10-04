import type { PluginManifestEntry } from '@flowtools/sdk'

import {
  loadManifestModule,
  parseManifestCatalog,
} from '@flowtools/sdk/manifest'

import serializedData from '../../../../plugins/.generated/builtin-manifests.json'

export const builtInManifestData = parseManifestCatalog(
  serializedData as unknown,
  {
    hostVersion: '0.1.0',
    sdkVersion: '0.0.0',
    platform: 'windows',
    arch: 'x64',
  },
  [
    'plugin-base64-encoder',
    'plugin-color-converter',
    'plugin-hash-generator',
    'plugin-image-base64',
    'plugin-json-formatter',
    'plugin-random-picker',
    'plugin-regex-tester',
    'plugin-text-ops',
    'plugin-timestamp-converter',
    'plugin-todo-list',
    'plugin-uuid-generator',
    'plugin-website-latency',
  ]
)

export const builtInManifests: PluginManifestEntry[] = [
  {
    id: 'plugin-base64-encoder',
    name: 'Base64 编解码',
    version: '0.1.0',
    maturity: 'prototype',
    description: '文本与 Base64 编码互转',
    type: 'app',
    permissions: ['clipboard'],
    tags: ['base64', 'encode', 'decode', 'converter'],
    category: '编码工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(value => value.id === 'plugin-base64-encoder'),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-base64-encoder',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-base64-encoder')
      ),
  },
  {
    id: 'plugin-color-converter',
    name: '颜色转换器',
    version: '0.1.0',
    maturity: 'prototype',
    description: 'HEX/RGB/HSL 颜色格式互转',
    type: 'app',
    permissions: ['clipboard'],
    tags: ['color', 'hex', 'rgb', 'hsl', 'converter'],
    category: '开发工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(
          value => value.id === 'plugin-color-converter'
        ),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-color-converter',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-color-converter')
      ),
  },
  {
    id: 'plugin-hash-generator',
    name: '哈希生成器',
    version: '0.1.0',
    maturity: 'prototype',
    description: '生成 SHA-1/SHA-256/SHA-384/SHA-512 哈希值',
    type: 'app',
    permissions: ['clipboard'],
    tags: ['hash', 'sha', 'sha256', 'md5', 'generator'],
    category: '开发工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(value => value.id === 'plugin-hash-generator'),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-hash-generator',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-hash-generator')
      ),
  },
  {
    id: 'plugin-image-base64',
    name: '图片 Base64 互转',
    version: '0.1.0',
    maturity: 'prototype',
    description: '在线图片与 Base64 编码互相转换',
    type: 'app',
    permissions: ['clipboard'],
    tags: ['image', 'base64', 'converter'],
    category: '编码工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(value => value.id === 'plugin-image-base64'),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-image-base64',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-image-base64')
      ),
  },
  {
    id: 'plugin-json-formatter',
    name: 'JSON 格式化',
    version: '0.1.0',
    maturity: 'prototype',
    description: 'JSON 格式化、压缩、验证工具',
    type: 'app',
    permissions: ['clipboard'],
    tags: ['json', 'format', 'minify', 'validate'],
    category: '开发工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(value => value.id === 'plugin-json-formatter'),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-json-formatter',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-json-formatter')
      ),
  },
  {
    id: 'plugin-random-picker',
    name: '随机点名',
    version: '0.1.0',
    maturity: 'prototype',
    description: '在线名单随机点名工具',
    type: 'app',
    tags: ['random', 'picker', 'name'],
    category: '实用工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(value => value.id === 'plugin-random-picker'),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-random-picker',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-random-picker')
      ),
  },
  {
    id: 'plugin-regex-tester',
    name: '正则表达式测试',
    version: '0.1.0',
    maturity: 'prototype',
    description: '测试和调试正则表达式',
    type: 'app',
    permissions: ['clipboard'],
    tags: ['regex', 'regexp', 'test', 'pattern'],
    category: '开发工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(value => value.id === 'plugin-regex-tester'),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-regex-tester',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-regex-tester')
      ),
  },
  {
    id: 'plugin-text-ops',
    name: '文本集合运算',
    version: '0.1.0',
    maturity: 'prototype',
    description: '计算文本的交集、差集、并集',
    type: 'app',
    permissions: ['clipboard'],
    tags: ['text', 'set', 'intersection', 'union', 'difference'],
    category: '文本工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(value => value.id === 'plugin-text-ops'),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-text-ops',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-text-ops')
      ),
  },
  {
    id: 'plugin-timestamp-converter',
    name: '时间戳转换',
    version: '0.1.0',
    maturity: 'prototype',
    description: 'Unix 时间戳与日期时间互转',
    type: 'app',
    permissions: ['clipboard'],
    tags: ['timestamp', 'unix', 'date', 'converter'],
    category: '开发工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(
          value => value.id === 'plugin-timestamp-converter'
        ),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-timestamp-converter',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-timestamp-converter')
      ),
  },
  {
    id: 'plugin-todo-list',
    name: 'Todo List',
    version: '0.0.1',
    maturity: 'prototype',
    type: 'app',
    permissions: ['storage'],
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(value => value.id === 'plugin-todo-list'),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-todo-list',
          version: '0.0.1',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-todo-list')
      ),
  },
  {
    id: 'plugin-uuid-generator',
    name: 'UUID 生成器',
    version: '0.1.0',
    maturity: 'prototype',
    description: '在线生成随机 UUID v4',
    type: 'app',
    permissions: ['clipboard'],
    tags: ['uuid', 'random', 'generator'],
    category: '开发工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(value => value.id === 'plugin-uuid-generator'),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-uuid-generator',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-uuid-generator')
      ),
  },
  {
    id: 'plugin-website-latency',
    name: '网站延迟测试',
    version: '0.1.0',
    maturity: 'prototype',
    description: '测试常用网站的网络延迟（Ping）',
    type: 'app',
    permissions: ['network'],
    tags: ['network', 'latency', 'ping'],
    category: '网络工具',
    cliAvailable: true,
    loader: () =>
      loadManifestModule(
        builtInManifestData.find(
          value => value.id === 'plugin-website-latency'
        ),
        {
          hostVersion: '0.1.0',
          sdkVersion: '0.0.0',
          platform: 'windows',
          arch: 'x64',
        },
        {
          publisher: 'flowtools',
          id: 'plugin-website-latency',
          version: '0.1.0',
          type: 'app',
          maturity: 'prototype',
        },
        () => import('@flowtools/plugins/plugin-website-latency')
      ),
  },
]

export const pluginCategories = [
  {
    id: '编码工具',
    label: '编码工具',
    pluginIds: ['plugin-base64-encoder', 'plugin-image-base64'],
  },
  {
    id: '开发工具',
    label: '开发工具',
    pluginIds: [
      'plugin-color-converter',
      'plugin-hash-generator',
      'plugin-json-formatter',
      'plugin-regex-tester',
      'plugin-timestamp-converter',
      'plugin-uuid-generator',
    ],
  },
  {
    id: '实用工具',
    label: '实用工具',
    pluginIds: ['plugin-random-picker'],
  },
  {
    id: '文本工具',
    label: '文本工具',
    pluginIds: ['plugin-text-ops'],
  },
  {
    id: '其他',
    label: '其他',
    pluginIds: ['plugin-todo-list'],
  },
  {
    id: '网络工具',
    label: '网络工具',
    pluginIds: ['plugin-website-latency'],
  },
] as const

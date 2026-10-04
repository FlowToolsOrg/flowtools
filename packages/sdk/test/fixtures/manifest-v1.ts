import type {
  ManifestTarget,
  PluginManifestV1,
} from '../../src/manifest/schema'

export const manifestTarget: ManifestTarget = {
  hostVersion: '0.1.0',
  sdkVersion: '0.0.0',
  platform: 'windows',
  arch: 'x64',
}

export function manifestFixture(): PluginManifestV1 {
  return {
    formatVersion: 1,
    publisher: 'flowtools',
    id: 'fixture-plugin',
    name: '测试工具',
    description: '有限测试操作',
    version: '0.1.0',
    type: 'tool',
    maturity: 'prototype',
    engines: { host: '^0.1.0', sdk: '>=0.0.0 <1.0.0' },
    targets: [{ platform: 'windows', arch: 'x64' }],
    entries: { executor: 'dist/commands.js' },
    files: [{ path: 'dist/commands.js', sha256: '0'.repeat(64), size: 0 }],
    signature: { status: 'unsigned' },
    dependencies: { services: [], tools: [] },
    commands: [
      {
        id: 'convert',
        name: '转换',
        description: '转换测试文本',
        headless: true,
        supportsColdStart: false,
        interaction: 'none',
        effects: [],
        permissions: [],
        resources: {
          timeoutMs: 30_000,
          maxInputBytes: 4096,
          maxOutputBytes: 4096,
        },
        runtimeValidation: { input: 'schema-only', output: 'schema-only' },
        inputSchema: {
          type: 'object',
          properties: {
            text: { type: 'string' },
            count: { type: 'integer', default: 1, minimum: 0 },
          },
          required: ['text'],
          additionalProperties: false,
        },
        outputSchema: { type: 'string' },
      },
    ],
  }
}

import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { parsePluginManifest } from '@flowtools/sdk/manifest'

import { manifestFixture } from '../../../sdk/test/fixtures/manifest-v1'

// Build-only, fixed fixtures. They are never added to the production inventory.
const directory = resolve(import.meta.dirname, '../../.generated/g4-services')
mkdirSync(directory, { recursive: true })
const bundle = await Bun.build({
  entrypoints: [resolve(import.meta.dirname, 'runner.ts')],
  target: 'bun',
  outdir: directory,
  naming: 'fixture-runner.js',
})
if (!bundle.success) throw new Error('Fixture build failed')
const bytes = readFileSync(resolve(directory, 'fixture-runner.js'))
const hash = createHash('sha256').update(bytes).digest('hex')
const manifests = ['fixture-a', 'fixture-b', 'fixture-c', 'fixture-c'].map(
  (id, index) => {
    const manifest = manifestFixture()
    manifest.id = id
    manifest.publisher = 'flowtools'
    manifest.version = index === 3 ? '1.1.0' : '1.0.0'
    manifest.entries =
      id === 'fixture-a'
        ? { executor: 'fixture-runner.js' }
        : { services: 'fixture-runner.js' }
    manifest.files = [
      { path: 'fixture-runner.js', size: bytes.length, sha256: hash },
    ]
    const operation = structuredClone(manifest.commands[0]!)
    operation.id = 'run'
    operation.inputSchema = {
      type: 'object',
      properties: {
        text: { type: 'string', maxLength: 256 },
        mode: {
          type: 'string',
          enum: ['echo', 'wait', 'allowed', 'extra', 'fail', 'daemon'],
          default: 'echo',
        },
      },
      required: ['text'],
      additionalProperties: false,
    }
    operation.outputSchema = { type: 'string', maxLength: 1024 }
    operation.runtimeValidation = {
      input: 'schema-only',
      output: 'schema-only',
    }
    operation.effects = ['file-read']
    operation.permissions = [
      {
        capability: 'fs',
        operations: ['read'],
        scopes: ['handle:allowed', 'handle:extra'],
      },
    ]
    operation.resources = {
      timeoutMs: 10000,
      maxInputBytes: 2048,
      maxOutputBytes: 8192,
    }
    operation.headless = true
    operation.interaction = 'none'
    operation.supportsColdStart = true
    manifest.commands = id === 'fixture-a' ? [operation] : []
    if (id !== 'fixture-a')
      manifest.services = [
        { id: 'transform', version: '1.0.0', operations: [operation] },
      ]
    manifest.dependencies = {
      services:
        id === 'fixture-c'
          ? []
          : [
              {
                publisher: 'flowtools',
                id: id === 'fixture-a' ? 'fixture-b' : 'fixture-c',
                version: '^1.0.0',
                service: 'transform',
                interfaceVersion: '^1.0.0',
              },
            ],
      tools: [],
    }
    return parsePluginManifest(manifest, {
      hostVersion: '0.1.0',
      sdkVersion: '0.0.0',
      platform: 'windows',
      arch: 'x64',
    })
  }
)
writeFileSync(resolve(directory, 'manifests.json'), JSON.stringify(manifests))

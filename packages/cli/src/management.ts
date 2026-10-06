import type { Call, PermissionGrant } from '@flowtools/runtime-client'

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { once } from 'node:events'
import { lstat, readFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { createInterface } from 'node:readline'
import { createInterface as createPrompt } from 'node:readline/promises'
import { fileURLToPath } from 'node:url'

import { RuntimeClient } from '@flowtools/runtime-client'
import { connectNamedPipe } from '@flowtools/runtime-client/node'
import { Command } from 'commander'

import { connectHost } from './host'
import { regularFile } from './regular-file'

declare const __FLOWTOOLS_NATIVE_BUILD_PATH__: string
declare const __FLOWTOOLS_BUNDLE_RUNTIME_LOCK__: {
  path: string
  sha256: string
} | null

export function userProfile(explicit?: string): string {
  const path =
    explicit ??
    (process.env.LOCALAPPDATA &&
      join(process.env.LOCALAPPDATA, 'FlowToolsRuntimeV1'))
  if (!path || !/^[a-z]:[\\/]/i.test(path)) throw new Error('SETUP_REQUIRED')
  return resolve(path)
}

export async function runtimeExecutable(): Promise<string> {
  const lock =
    typeof __FLOWTOOLS_BUNDLE_RUNTIME_LOCK__ === 'object'
      ? __FLOWTOOLS_BUNDLE_RUNTIME_LOCK__
      : null
  const candidate = lock
    ? fileURLToPath(new URL('../../../' + lock.path, import.meta.url))
    : typeof __FLOWTOOLS_NATIVE_BUILD_PATH__ === 'string'
      ? __FLOWTOOLS_NATIVE_BUILD_PATH__
      : fileURLToPath(
          new URL(
            '../../../target/debug/flowtools-runtime.exe',
            import.meta.url
          )
        )
  const path = await regularFile(candidate)
  if (
    lock &&
    createHash('sha256')
      .update(await readFile(path))
      .digest('hex') !== lock.sha256
  )
    throw new Error('BUNDLE_INTEGRITY_FAILED')
  return path
}

export async function readBootstrap(profile: string) {
  const path = await regularFile(join(profile, 'bootstrap.json'), 2048)
  const value: unknown = JSON.parse(await readFile(path, 'utf8'))
  if (
    !value ||
    typeof value !== 'object' ||
    !('formatVersion' in value) ||
    value.formatVersion !== 1
  )
    throw new Error('SETUP_REQUIRED')
  const record = value as Record<string, unknown>
  for (const field of ['cliToken', 'desktopToken', 'managementToken'] as const)
    if (
      typeof record[field] !== 'string' ||
      !/^[a-f0-9]{64}$/.test(record[field])
    )
      throw new Error('SETUP_REQUIRED')
  return value as {
    formatVersion: 1
    cliToken: string
    desktopToken: string
    managementToken: string
  }
}

export async function startManagement(profile: string, initialize: boolean) {
  if (!initialize) {
    try {
      const client = await connectHost(profile, true)
      return {
        client,
        async close() {
          client.close()
        },
      }
    } catch (error) {
      if (!(error instanceof Error) || error.message !== 'RUNTIME_DISCONNECTED')
        throw error
    }
  }
  const child = spawn(
    await runtimeExecutable(),
    [
      initialize ? '--initialize-profile' : '--profile',
      profile,
      '--management',
    ],
    {
      windowsHide: true,
      stdio: ['pipe', 'pipe', 'pipe'],
      env: {
        SystemRoot: process.env.SystemRoot,
        FLOWTOOLS_RUNTIME_VALIDATION_STDIN_LIFETIME: '1',
      },
    }
  )
  const exited = once(child, 'exit')
  // Native startup emits only a stable code. Never return arbitrary stderr.
  let failure = ''
  child.stderr.on('data', (bytes: Buffer) => {
    if (failure.length < 256) failure += bytes.toString('utf8')
  })
  const lines = createInterface({ input: child.stdout })
  let client: RuntimeClient | undefined
  const timeout = setTimeout(() => child.kill(), 10000)
  try {
    const ready = await Promise.race([
      once(lines, 'line').then(([line]) => {
        const value: unknown = JSON.parse(String(line))
        if (
          !value ||
          typeof value !== 'object' ||
          !('pipe' in value) ||
          typeof value.pipe !== 'string'
        )
          throw new Error('INVALID_RESPONSE')
        return { pipe: value.pipe }
      }),
      exited.then(() => {
        throw new Error(
          /^[A-Z_]+\s*$/.test(failure) ? failure.trim() : 'SETUP_REQUIRED'
        )
      }),
    ])
    client = new RuntimeClient(await connectNamedPipe(ready.pipe))
    await client.connect((await readBootstrap(profile)).managementToken)
    clearTimeout(timeout)
    const connected = client
    return {
      client: connected,
      async close() {
        connected.close()
        child.stdin.end()
        lines.close()
        await exited
      },
    }
  } catch (error) {
    client?.close()
    child.stdin.end()
    lines.close()
    child.kill()
    await exited
    throw error
  } finally {
    clearTimeout(timeout)
  }
}

async function confirm(text: string) {
  if (!process.stdin.isTTY || !process.stdout.isTTY)
    throw new Error('INTERACTION_REQUIRED')
  const prompt = createPrompt({ input: process.stdin, output: process.stdout })
  try {
    if (
      (await prompt.question(text + '\nType yes to approve: ')).trim() !== 'yes'
    )
      throw new Error('APPROVAL_REQUIRED')
  } finally {
    prompt.close()
  }
}

async function policyFile(path: string): Promise<{
  formatVersion: 1
  coldStart: boolean
  grants: PermissionGrant[]
}> {
  if ((await lstat(path)).size > 262144) throw new Error('INVALID_REQUEST')
  const value: unknown = JSON.parse(await readFile(path, 'utf8'))
  if (
    !value ||
    typeof value !== 'object' ||
    !('formatVersion' in value) ||
    value.formatVersion !== 1 ||
    !('coldStart' in value) ||
    typeof value.coldStart !== 'boolean' ||
    !('grants' in value) ||
    !Array.isArray(value.grants) ||
    value.grants.length > 32
  )
    throw new Error('INVALID_REQUEST')
  return value as {
    formatVersion: 1
    coldStart: boolean
    grants: PermissionGrant[]
  }
}

export function addManagementCommands(program: Command) {
  const action = async (
    profile: string,
    initialize: boolean,
    calls: Call[]
  ) => {
    const host = await startManagement(profile, initialize)
    try {
      const outcomes = []
      for (const call of calls) outcomes.push(await host.client.call(call))
      process.stdout.write(
        JSON.stringify({ formatVersion: 1, success: true, data: outcomes }) +
          '\n'
      )
    } finally {
      await host.close()
    }
  }
  program
    .command('init')
    .description('Initialize private Runtime policy without GUI')
    .option('--interactive')
    .option('--policy <file>')
    .option('--profile <directory>')
    .action(
      async (options: {
        interactive?: boolean
        policy?: string
        profile?: string
      }) => {
        if (!options.interactive && !options.policy)
          throw new Error('INTERACTION_REQUIRED')
        const config = options.policy
          ? await policyFile(options.policy)
          : { formatVersion: 1, coldStart: true, grants: [] }
        if (options.interactive)
          await confirm(
            'Initialize Runtime with coldStart=' +
              config.coldStart +
              '. No plugin grants are implicit.'
          )
        await action(userProfile(options.profile), true, [
          { method: 'policy.import', payload: config },
        ])
      }
    )
  const permissions = program
    .command('permissions')
    .description('Manage explicit version/hash-bound grants')
  permissions
    .command('list')
    .option('--profile <directory>')
    .action(async (options: { profile?: string }) =>
      action(userProfile(options.profile), false, [
        { method: 'permissions.list' },
      ])
    )
  permissions
    .command('grant')
    .requiredOption('--policy <file>')
    .option('--interactive')
    .option('--profile <directory>')
    .action(
      async (options: {
        policy: string
        interactive?: boolean
        profile?: string
      }) => {
        const config = await policyFile(options.policy)
        if (options.interactive)
          await confirm(JSON.stringify(config.grants, null, 2))
        await action(userProfile(options.profile), false, [
          { method: 'policy.import', payload: config },
        ])
      }
    )
  permissions
    .command('revoke <plugin-id>')
    .option('--command <id>', 'command', 'run')
    .option('--target <target>', 'cli or desktop', 'cli')
    .option('--profile <directory>')
    .action(
      async (
        pluginId: string,
        options: { command: string; target: string; profile?: string }
      ) => {
        if (options.target !== 'cli' && options.target !== 'desktop')
          throw new Error('INVALID_REQUEST')
        await action(userProfile(options.profile), false, [
          {
            method: 'permissions.revoke',
            payload: {
              pluginId,
              commandId: options.command,
              target: options.target,
            },
          },
        ])
      }
    )
}

export function managementFailure(error: unknown) {
  const code =
    error instanceof Error && /^[A-Z_]+$/.test(error.message)
      ? error.message
      : 'INVALID_REQUEST'
  process.stdout.write(
    JSON.stringify({ formatVersion: 1, success: false, error: { code } }) + '\n'
  )
  process.exitCode = 1
}

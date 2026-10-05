import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

import { RuntimeClient, RuntimeClientError } from '@flowtools/runtime-client'
import { connectNamedPipe } from '@flowtools/runtime-client/node'
import { Command } from 'commander'

import { readBootstrap, runtimeExecutable, userProfile } from './management'

const execute = promisify(execFile)
async function endpoint(profile: string, start: boolean): Promise<string> {
  try {
    const { stdout } = await execute(
      await runtimeExecutable(),
      [start ? '--ensure-runtime' : '--endpoint', profile],
      {
        windowsHide: true,
        timeout: 35000,
        maxBuffer: 4096,
        env: { SystemRoot: process.env.SystemRoot },
      }
    )
    const value: unknown = JSON.parse(stdout)
    if (
      !value ||
      typeof value !== 'object' ||
      !('type' in value) ||
      value.type !== 'endpoint' ||
      !('protocolMajor' in value) ||
      value.protocolMajor !== 1 ||
      !('pipe' in value) ||
      typeof value.pipe !== 'string'
    )
      throw new Error('INVALID_RESPONSE')
    return value.pipe
  } catch (error) {
    if (
      error &&
      typeof error === 'object' &&
      'stderr' in error &&
      typeof error.stderr === 'string' &&
      /^[A-Z_]+\s*$/.test(error.stderr)
    )
      throw new Error(error.stderr.trim())
    throw error instanceof Error && /^[A-Z_]+$/.test(error.message)
      ? error
      : new Error('STARTUP_FAILED')
  }
}
export async function connectHost(
  profile: string,
  manager = false,
  cold = false
): Promise<RuntimeClient> {
  const credentials = await readBootstrap(profile)
  const pipe = await endpoint(profile, false)
  const connect = async () => {
    const client = new RuntimeClient(await connectNamedPipe(pipe))
    try {
      await client.connect(
        manager ? credentials.managementToken : credentials.cliToken
      )
      return client
    } catch (error) {
      client.close()
      throw error
    }
  }
  try {
    return await connect()
  } catch (error) {
    if (
      !(error instanceof RuntimeClientError) ||
      error.code !== 'RUNTIME_DISCONNECTED' ||
      !cold
    )
      throw error
  }
  if ((await endpoint(profile, true)) !== pipe)
    throw new Error('INVALID_RESPONSE')
  // Bounded connection retries reuse the same endpoint; no business submit occurs here.
  const deadline = Date.now() + 5000
  while (true) {
    try {
      return await connect()
    } catch (error) {
      if (
        !(error instanceof RuntimeClientError) ||
        error.code !== 'RUNTIME_DISCONNECTED' ||
        Date.now() >= deadline
      )
        throw error
      await new Promise(resolve => setTimeout(resolve, 25))
    }
  }
}
export function addRuntimeCommands(program: Command) {
  const runtime = program
    .command('runtime')
    .description('Control the shared headless Host')
  for (const command of ['status', 'start', 'stop'] as const) {
    runtime
      .command(command)
      .option('--profile <directory>')
      .action(async (options: { profile?: string }) => {
        const client = await connectHost(
          userProfile(options.profile),
          command === 'stop',
          command === 'start'
        )
        try {
          const outcome = await client.call({
            method: command === 'stop' ? 'runtime.stop' : 'runtime.status',
          })
          if (
            command === 'start' &&
            outcome.type === 'status' &&
            outcome.data.mode !== 'managed'
          )
            throw new Error('RUNTIME_BUSY')
          process.stdout.write(
            JSON.stringify({
              formatVersion: 1,
              success: true,
              data: 'data' in outcome ? outcome.data : { stopping: true },
            }) + '\n'
          )
        } finally {
          client.close()
        }
      })
  }
}

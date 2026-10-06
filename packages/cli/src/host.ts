import { spawn } from 'node:child_process'

import { RuntimeClient, RuntimeClientError } from '@flowtools/runtime-client'
import { connectNamedPipe } from '@flowtools/runtime-client/node'
import { Command } from 'commander'

import { readBootstrap, runtimeExecutable, userProfile } from './management'

async function endpoint(profile: string, start: boolean): Promise<string> {
  const executable = await runtimeExecutable()
  const stdout = await new Promise<string>((resolve, reject) => {
    const child = spawn(
      executable,
      [start ? '--ensure-runtime' : '--endpoint', profile],
      {
        windowsHide: true,
        stdio: ['ignore', 'pipe', 'pipe'],
        env: { SystemRoot: process.env.SystemRoot },
      }
    )
    let output = '',
      failure = '',
      status: number | null | undefined
    const cleanup = () => {
      clearTimeout(timer)
      child.stdout.destroy()
      child.stderr.destroy()
    }
    const finish = () => {
      if (status === undefined) return
      if (status !== 0) {
        cleanup()
        reject(
          new Error(
            /^[A-Z_]+\s*$/.test(failure) ? failure.trim() : 'STARTUP_FAILED'
          )
        )
        return
      }
      if (!output.includes('\n')) return
      cleanup()
      resolve(output)
    }
    const timer = setTimeout(() => {
      child.kill()
      cleanup()
      reject(new Error('STARTUP_FAILED'))
    }, 45000)
    child.stdout.on('data', (bytes: Buffer) => {
      output += bytes.toString('utf8')
      if (output.length > 4096) {
        child.kill()
        cleanup()
        reject(new Error('INVALID_RESPONSE'))
      } else finish()
    })
    child.stderr.on('data', (bytes: Buffer) => {
      if (failure.length < 256) failure += bytes.toString('utf8')
    })
    child.once('error', () => {
      cleanup()
      reject(new Error('STARTUP_FAILED'))
    })
    // A background descendant may inherit the outer pipe: wait for the bounded
    // helper line + process exit, never for every descendant's stdio EOF.
    child.once('exit', code => {
      status = code
      finish()
    })
  })
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

import { spawn } from 'node:child_process'

import { RuntimeClient, RuntimeClientError } from '@flowtools/runtime-client'
import { connectNamedPipe } from '@flowtools/runtime-client/node'
import { Command } from 'commander'

import { readBootstrap, runtimeExecutable, userProfile } from './native-runtime'
import { addStorageCommands } from './storage'

export class RuntimeReadinessError extends RuntimeClientError {
  readonly stage: 'pipe' | 'authenticate' | undefined
  constructor(
    error: RuntimeClientError,
    readonly phase: 'connect' | 'query' | 'reconnect',
    stage?: 'pipe' | 'authenticate'
  ) {
    super(error.code)
    this.stage =
      stage ??
      ('stage' in error &&
      (error.stage === 'pipe' || error.stage === 'authenticate')
        ? error.stage
        : undefined)
  }
}

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
  cold = false,
  expectedInstanceId: string | null = null
): Promise<RuntimeClient> {
  const credentials = await readBootstrap(profile)
  const pipe = await endpoint(profile, false)
  const connect = async () => {
    let client: RuntimeClient
    try {
      client = new RuntimeClient(await connectNamedPipe(pipe))
    } catch (error) {
      throw error instanceof RuntimeClientError
        ? new RuntimeReadinessError(error, 'connect', 'pipe')
        : error
    }
    try {
      await client.connect(
        manager ? credentials.managementToken : credentials.cliToken,
        expectedInstanceId
      )
      return client
    } catch (error) {
      client.close()
      throw error instanceof RuntimeClientError
        ? new RuntimeReadinessError(error, 'connect', 'authenticate')
        : error
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
  addStorageCommands(runtime)
  for (const command of ['status', 'start', 'stop'] as const) {
    runtime
      .command(command)
      .option('--profile <directory>')
      .action(async (options: { profile?: string }) => {
        const profile = userProfile(options.profile)
        let client: RuntimeClient
        try {
          client = await connectHost(
            profile,
            command === 'stop',
            command === 'start'
          )
        } catch (error) {
          if (error instanceof RuntimeClientError)
            throw new RuntimeReadinessError(error, 'connect')
          throw error
        }
        try {
          const outcome =
            command === 'stop'
              ? await client.call({ method: 'runtime.stop' })
              : await readControlStatus(client, async instance => {
                  client.close()
                  client = await connectHost(profile, false, false, instance)
                  return client
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

/** Bounded recovery of a read-only readiness query, never a business submit. */
export async function readControlStatus(
  client: RuntimeClient,
  reconnect: (instance: string | null) => Promise<RuntimeClient>
) {
  const instance = client.instanceId ?? null
  const deadline = Date.now() + 10000
  while (true) {
    try {
      return await client.call({ method: 'runtime.status' })
    } catch (error) {
      if (
        !(error instanceof RuntimeClientError) ||
        error.code !== 'RUNTIME_DISCONNECTED' ||
        Date.now() >= deadline
      )
        throw error instanceof RuntimeClientError
          ? new RuntimeReadinessError(error, 'query')
          : error
      while (true) {
        try {
          client = await reconnect(instance)
          break
        } catch (error) {
          if (
            error instanceof RuntimeClientError &&
            error.code === 'RUNTIME_DISCONNECTED' &&
            Date.now() < deadline
          ) {
            await new Promise(resolve => setTimeout(resolve, 25))
            continue
          }
          throw error instanceof RuntimeClientError
            ? new RuntimeReadinessError(error, 'reconnect')
            : error
        }
      }
    }
  }
}

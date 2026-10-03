import type { ToolContext } from '../types/ctx'

export type ExecutionErrorCode =
  | 'INPUT_INVALID'
  | 'NOT_RUNNABLE'
  | 'PLUGIN_ID_MISMATCH'
  | 'PLUGIN_NOT_FOUND'
  | 'LOAD_FAILED'
  | 'CONTEXT_FAILED'
  | 'EXECUTION_FAILED'
  | 'OUTPUT_INVALID'
  | 'ABORTED'
  | 'TIMEOUT'
  | 'TIMEOUT_INVALID'

export interface ExecutionError {
  code: ExecutionErrorCode
  message: string
  issues?: readonly { path: readonly PropertyKey[]; message: string }[]
}

export interface ExecutionInputSummary {
  kind: string
  size: number
}

export interface ExecutionMetadata {
  pluginId: string
  pluginVersion: string | null
  startedAt: number
  finishedAt: number
  durationMs: number
  inputSummary: ExecutionInputSummary
}

export type PluginExecutionResult = ExecutionMetadata &
  ({ success: true; data: unknown } | { success: false; error: ExecutionError })

interface SchemaValidation {
  success: boolean
  data?: unknown
  error?: {
    issues: readonly { path: readonly PropertyKey[]; message: string }[]
  }
}

export interface ExecutablePlugin<TInput = never> {
  meta: { id: string; version: string }
  inputSchema?: { safeParse: (input: unknown) => SchemaValidation }
  run?: (ctx: ToolContext, input: TInput) => unknown
}

export interface ExecutePluginOptions {
  signal?: AbortSignal
  timeoutMs?: number
  /** Injectable scheduler for deterministic host/SDK regression tests. */
  startTimeout?: (callback: () => void, ms: number) => () => void
}

/** Shape only: no values, field names, or string contents are retained. */
export function summarizeExecutionInput(input: unknown): ExecutionInputSummary {
  if (input === null) return { kind: 'null', size: 0 }
  if (typeof input === 'string') return { kind: 'string', size: input.length }
  if (Array.isArray(input)) return { kind: 'array', size: input.length }
  if (typeof input === 'object') {
    try {
      return { kind: 'object', size: Object.keys(input).length }
    } catch {
      return { kind: 'object', size: 0 }
    }
  }
  return { kind: typeof input, size: 0 }
}

/** Record failures before a plugin can be loaded, without pretending it ran. */
export function createExecutionFailure(
  pluginId: string,
  pluginVersion: string | null,
  input: unknown,
  error: ExecutionError,
  startedAt = Date.now()
): PluginExecutionResult {
  const finishedAt = Math.max(startedAt, Date.now())
  return {
    pluginId,
    pluginVersion,
    startedAt,
    finishedAt,
    durationMs: finishedAt - startedAt,
    inputSummary: summarizeExecutionInput(input),
    success: false,
    error,
  }
}

class ExecutionInterrupted extends Error {
  constructor(
    readonly code: 'ABORTED' | 'TIMEOUT',
    message: string
  ) {
    super(message)
  }
}

/**
 * Execute trusted app/tool run() through one schema, cancellation and outcome
 * boundary. This is not a sandbox: synchronous loops and prior side effects
 * cannot be stopped or rolled back inside the host JavaScript realm.
 */
export async function executePlugin<TInput>(
  plugin: ExecutablePlugin<TInput>,
  input: unknown,
  ctx: ToolContext,
  options: ExecutePluginOptions = {}
): Promise<PluginExecutionResult> {
  const startedAt = Date.now()
  const fail = (error: ExecutionError) =>
    createExecutionFailure(
      plugin.meta.id,
      plugin.meta.version,
      input,
      error,
      startedAt
    )
  if (ctx.env.pluginId !== plugin.meta.id) {
    return fail({
      code: 'PLUGIN_ID_MISMATCH',
      message: 'Host context does not match plugin identity',
    })
  }
  if (typeof plugin.run !== 'function')
    return fail({
      code: 'NOT_RUNNABLE',
      message: `Plugin ${plugin.meta.id} has no run() function`,
    })
  if (ctx.signal.aborted || options.signal?.aborted) {
    return fail({ code: 'ABORTED', message: 'Plugin execution cancelled' })
  }
  const timeoutMs = options.timeoutMs ?? 30_000
  if (
    !Number.isInteger(timeoutMs) ||
    timeoutMs < 1 ||
    timeoutMs > 2_147_483_647
  ) {
    return fail({
      code: 'TIMEOUT_INVALID',
      message: 'Timeout must be a positive integer within the timer range',
    })
  }

  let validatedInput: unknown = input
  if (plugin.inputSchema) {
    try {
      const parsed = plugin.inputSchema.safeParse(input)
      if (!parsed.success) {
        return fail({
          code: 'INPUT_INVALID',
          message: 'Plugin input does not match inputSchema',
          issues: parsed.error?.issues,
        })
      }
      validatedInput = parsed.data
    } catch {
      return fail({
        code: 'INPUT_INVALID',
        message: 'Plugin input schema validation failed',
      })
    }
  }

  const controller = new AbortController()
  const upstreamSignals = new Set([
    ctx.signal,
    ...(options.signal ? [options.signal] : []),
  ])
  let cancelTimeout: (() => void) | undefined
  let rejectAbort: (error: ExecutionInterrupted) => void = () => {}
  const aborted = new Promise<never>((_, reject) => {
    rejectAbort = reject
  })
  const onCallerAbort = () => {
    if (controller.signal.aborted) return
    const error = new ExecutionInterrupted(
      'ABORTED',
      'Plugin execution cancelled'
    )
    controller.abort(error)
    rejectAbort(error)
  }
  const onTimeout = () => {
    if (controller.signal.aborted) return
    const error = new ExecutionInterrupted(
      'TIMEOUT',
      `Plugin execution timed out after ${timeoutMs}ms`
    )
    controller.abort(error)
    rejectAbort(error)
  }
  try {
    for (const signal of upstreamSignals)
      signal.addEventListener('abort', onCallerAbort, { once: true })
    // Validation may invoke user code that aborts before listeners are attached.
    if ([...upstreamSignals].some(signal => signal.aborted)) onCallerAbort()
    const startTimeout =
      options.startTimeout ??
      ((callback, ms) => {
        const timer = setTimeout(callback, ms)
        return () => clearTimeout(timer)
      })
    cancelTimeout = startTimeout(onTimeout, timeoutMs)
    const run = plugin.run
    const data = await Promise.race([
      Promise.resolve().then(() => {
        if (controller.signal.aborted) throw controller.signal.reason
        return run.call(
          plugin,
          { ...ctx, signal: controller.signal },
          validatedInput as TInput
        )
      }),
      aborted,
    ])
    const finishedAt = Math.max(startedAt, Date.now())
    return {
      pluginId: plugin.meta.id,
      pluginVersion: plugin.meta.version,
      startedAt,
      finishedAt,
      durationMs: finishedAt - startedAt,
      inputSummary: summarizeExecutionInput(input),
      success: true,
      data,
    }
  } catch (error) {
    return fail({
      code:
        error instanceof ExecutionInterrupted ? error.code : 'EXECUTION_FAILED',
      message: error instanceof Error ? error.message : String(error),
    })
  } finally {
    cancelTimeout?.()
    for (const signal of upstreamSignals)
      signal.removeEventListener('abort', onCallerAbort)
  }
}

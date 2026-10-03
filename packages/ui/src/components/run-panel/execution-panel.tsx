import type {
  ExecutionHistory,
  PluginExecutionResult,
} from '@flowtools/sdk/execution'

import { useEffect, useRef, useState, useSyncExternalStore } from 'react'

import { createExecutionFailure } from '@flowtools/sdk/execution'

import { Button, TextArea } from '@heroui/react'

export interface ExecutionPanelProps {
  meta: { id: string; name: string; version: string }
  execute: (
    input: unknown,
    signal: AbortSignal
  ) => Promise<PluginExecutionResult>
  inputSchema?: unknown
  history?: ExecutionHistory
}

const emptyHistory = { entries: [], persistenceError: null }
const emptySnapshot = () => emptyHistory
const noSubscribe = () => () => {}

function formatJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2) ?? 'undefined'
  } catch {
    return 'OUTPUT_INVALID: Output cannot be displayed as JSON'
  }
}

function displayableResult(
  result: PluginExecutionResult
): PluginExecutionResult {
  try {
    JSON.stringify(result)
    return result
  } catch {
    return {
      success: false,
      pluginId: result.pluginId,
      pluginVersion: result.pluginVersion,
      startedAt: result.startedAt,
      finishedAt: result.finishedAt,
      durationMs: result.durationMs,
      inputSummary: result.inputSummary,
      error: {
        code: 'OUTPUT_INVALID',
        message: 'Output cannot be displayed as JSON',
      },
    }
  }
}

/** Shared UI only; hosts bind the real plugin and its SDK capability context. */
export function ExecutionPanel({
  meta,
  execute,
  inputSchema,
  history,
}: ExecutionPanelProps) {
  const [input, setInput] = useState('{}')
  const [running, setRunning] = useState(false)
  const [result, setResult] = useState<PluginExecutionResult | null>(null)
  const active = useRef<AbortController | null>(null)
  const snapshot = useSyncExternalStore(
    history?.subscribe ?? noSubscribe,
    history?.getSnapshot ?? emptySnapshot,
    emptySnapshot
  )

  useEffect(
    () => () => {
      active.current?.abort()
      active.current = null
    },
    [meta.id, meta.version]
  )

  const run = async () => {
    if (active.current) return
    const controller = new AbortController()
    active.current = controller
    setRunning(true)
    setResult(null)
    let outcome: PluginExecutionResult
    let parsed: unknown
    try {
      parsed = JSON.parse(input)
    } catch {
      outcome = createExecutionFailure(meta.id, meta.version, input, {
        code: 'INPUT_INVALID',
        message: 'Input must be valid JSON',
      })
      history?.record(outcome, meta.name)
      setResult(outcome)
      setRunning(false)
      active.current = null
      return
    }
    try {
      outcome = await execute(parsed, controller.signal)
    } catch {
      outcome = createExecutionFailure(meta.id, meta.version, parsed, {
        code: 'EXECUTION_FAILED',
        message: 'Host execution adapter failed',
      })
    }
    outcome = displayableResult(outcome)
    history?.record(outcome, meta.name)
    // Cleanup cancels the request; never put old results into a new route.
    if (active.current !== controller) return
    active.current = null
    setResult(outcome)
    setRunning(false)
  }

  return (
    <section
      aria-label="SDK execution"
      className="space-y-4 rounded-xl border border-border bg-surface p-4"
    >
      <div>
        <h2 className="text-base font-semibold">Run JSON — {meta.name}</h2>
        <p className="text-sm text-muted">
          v{meta.version} · Calls the actual plugin run() · 30s async deadline
        </p>
      </div>
      {inputSchema !== undefined ? (
        <details>
          <summary className="cursor-pointer text-sm">
            Input schema and defaults
          </summary>
          <pre className="max-h-64 overflow-auto text-xs whitespace-pre-wrap">
            {formatJson(inputSchema)}
          </pre>
        </details>
      ) : null}
      <TextArea
        aria-label="JSON input"
        fullWidth
        rows={5}
        value={input}
        disabled={running}
        onChange={event => setInput(event.target.value)}
      />
      <div className="flex flex-wrap gap-2">
        <Button
          isDisabled={running}
          onPress={() => {
            void run()
          }}
        >
          Run JSON
        </Button>
        <Button
          isDisabled={!running}
          variant="secondary"
          onPress={() => active.current?.abort()}
        >
          Cancel
        </Button>
      </div>
      <p role="status" aria-live="polite" className="text-sm">
        {running
          ? 'Running…'
          : result
            ? result.success
              ? 'Success · ' + result.durationMs + 'ms'
              : result.error.code +
                ' · ' +
                result.error.message +
                ' · ' +
                result.durationMs +
                'ms'
            : 'Ready — input and output are not persisted in history'}
      </p>
      {result ? (
        <pre
          data-testid="execution-output"
          className="max-h-96 overflow-auto rounded-lg border border-border p-3 text-xs break-all whitespace-pre-wrap"
        >
          {formatJson(result)}
        </pre>
      ) : null}
      {history ? (
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-2">
            <h3 className="text-sm font-semibold">
              Execution history (metadata only)
            </h3>
            <Button
              size="sm"
              variant="ghost"
              onPress={() => history.clear(meta.id)}
            >
              Clear history
            </Button>
          </div>
          {snapshot.persistenceError ? (
            <p role="alert" className="text-sm text-danger">
              {snapshot.persistenceError}
            </p>
          ) : null}
          <ul className="space-y-1 text-xs text-muted">
            {snapshot.entries
              .filter(entry => entry.pluginId === meta.id)
              .slice(0, 10)
              .map(entry => (
                <li key={entry.id}>
                  {new Date(entry.startedAt).toLocaleString()} · v
                  {entry.pluginVersion ?? 'unknown'} · {entry.status} ·{' '}
                  {entry.errorCode ?? entry.resultType} · {entry.durationMs}ms
                </li>
              ))}
          </ul>
        </div>
      ) : null}
    </section>
  )
}

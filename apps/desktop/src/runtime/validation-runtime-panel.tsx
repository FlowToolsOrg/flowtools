import { useEffect, useState } from 'react'

import {
  RuntimeClient,
  RuntimeClientError,
  describeRuntimeError,
} from '@flowtools/runtime-client'

import { commands } from '../utils/bindings'

function diagnosticText(error: unknown): string {
  const diagnostic = describeRuntimeError(error)
  return `${diagnostic.code}: ${diagnostic.summary}；${diagnostic.action}`
}

/** Disposable G2 fixture: native code binds endpoint, caller and bootstrap token. */
export default function ValidationRuntimePanel() {
  const [runId, setRunId] = useState('')
  const [status, setStatus] = useState('Disconnected')
  const [client, setClient] = useState<RuntimeClient>()
  useEffect(() => {
    const connection = new RuntimeClient({
      async exchange(request) {
        const response = await commands.validationRuntime(request)
        if (response.status === 'error')
          throw new RuntimeClientError(response.error.code)
        return response.data
      },
      close() {
        void commands.validationRuntimeDisconnect().catch(() => {})
      },
    })
    let active = true
    let handshake: Promise<void> | undefined
    const timer = setTimeout(() => {
      handshake = connection
        .connect('native-bootstrap')
        .then(() => {
          if (active) {
            setClient(connection)
            setStatus('Connected: prototype validation')
          }
        })
        .catch(error => {
          if (active) setStatus(diagnosticText(error))
        })
    }, 0)
    return () => {
      active = false
      clearTimeout(timer)
      // A started handshake finishes before releasing its native connection.
      if (handshake) void handshake.then(() => connection.close())
    }
  }, [])
  const inspect = async () => {
    if (!client) return
    try {
      const job = await client.waitForResult(runId)
      setStatus(`${job.runId} | ${job.state} | ${job.rootCaller}`)
    } catch (error) {
      setStatus(diagnosticText(error))
    }
  }
  return (
    <section aria-label="Runtime validation" style={{ padding: 24 }}>
      <p>Prototype · disposable Runtime profile</p>
      <label>
        Run ID{' '}
        <input
          aria-label="Run ID"
          value={runId}
          onChange={event => setRunId(event.target.value)}
        />
      </label>
      <button
        disabled={!client || !runId}
        onClick={() => {
          void inspect()
        }}
      >
        Inspect Runtime job
      </button>
      <p role="status" data-testid="runtime-job-status">
        {status}
      </p>
    </section>
  )
}

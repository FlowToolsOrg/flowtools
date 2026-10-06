import type {
  ErrorCode,
  Outcome,
  PermissionRecord,
  StorageReport,
} from '@flowtools/runtime-client'

import { RuntimeClient, RuntimeClientError } from '@flowtools/runtime-client'
import { beforeEach, expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import {
  connectManagedRuntime,
  controlManagedRuntime,
  storageManagedRuntime,
} from '../../../../desktop/src/runtime/managed-client'
import { ManagedRuntimePanel } from '../../../../desktop/src/runtime/managed-runtime-panel'

vi.mock('../../../../desktop/src/runtime/desktop-capabilities', () => ({
  isTauriRuntime: () => true,
}))
vi.mock('../../../../desktop/src/runtime/managed-client', () => ({
  connectManagedRuntime: vi.fn(),
  controlManagedRuntime: vi.fn(),
  storageManagedRuntime: vi.fn(),
}))

const backupId = '55d5f410-714b-4622-b68c-18fb20527467'
let records: PermissionRecord[]
let recoveryPhase: 'prepared' | 'complete'
let failure: ErrorCode | undefined
let permissionReadFails: boolean

function report(phase?: 'prepared' | 'complete'): StorageReport {
  return {
    formatVersion: 1,
    backups: [
      {
        id: backupId,
        bytes: 45056,
        schemaVersion: 2,
        usable: true,
        errorCode: null,
      },
    ],
    recovery: phase
      ? {
          recoveryId: '91e4b1c1-4f9a-4f9a-849f-596d96d8d169',
          backupId,
          phase,
          grantsRevoked: phase === 'complete',
          payloadsQuarantined: true,
        }
      : null,
  }
}

beforeEach(() => {
  vi.clearAllMocks()
  recoveryPhase = 'complete'
  failure = undefined
  permissionReadFails = false
  records = [
    {
      identity: {
        caller: 'local-desktop',
        publisher: 'flowtools',
        pluginId: 'plugin-todo-list',
        version: '0.0.1',
        packageDigest: 'a'.repeat(64),
        commandId: 'run',
      },
      epoch: 1,
      grant: {
        pluginId: 'plugin-todo-list',
        commandId: 'run',
        target: 'desktop',
        packageDigest: 'a'.repeat(64),
        effects: ['data-read', 'data-write'],
        scopes: [{ kind: 'plugin-data', scope: { key_prefix: 'todos' } }],
        expiresAt: Date.now() + 86400000,
        maxCalls: 128,
        coldStart: true,
        background: false,
      },
    },
  ]
  vi.mocked(connectManagedRuntime).mockImplementation(async () => {
    const client = new RuntimeClient({
      async exchange(request) {
        let outcome: Outcome
        switch (request.call.method) {
          case 'session.open':
            outcome = {
              type: 'session',
              data: { sessionId: 'fixture-native', instanceId: 'fixture-host' },
            }
            break
          case 'runtime.status':
            outcome = {
              type: 'status',
              data: {
                instanceId: 'fixture-host',
                mode: 'managed',
                activeJobs: 0,
                jobs: 0,
              },
            }
            break
          case 'jobs.list':
            outcome = { type: 'jobs', data: [] }
            break
          default:
            throw new Error('Unexpected business call: ' + request.call.method)
        }
        return { version: 1, requestId: request.requestId, outcome }
      },
      close() {},
    })
    await client.connect('controlled native transport')
    return client
  })
  vi.mocked(controlManagedRuntime).mockImplementation(async call => {
    if (call.method !== 'permissions.list')
      throw new Error('Unexpected management call')
    if (permissionReadFails)
      throw new RuntimeClientError('RUNTIME_DISCONNECTED')
    return { type: 'permissions', data: records }
  })
  vi.mocked(storageManagedRuntime).mockImplementation(async action => {
    if (action.operation === 'list')
      return report(recoveryPhase === 'prepared' ? 'prepared' : undefined)
    if (failure) throw new RuntimeClientError(failure)
    records = records.map(record => ({ ...record, epoch: 2, grant: null }))
    return report('complete')
  })
})

async function connectedPanel() {
  const screen = await render(<ManagedRuntimePanel />)
  const user = userEvent.setup()
  await user.click(screen.getByRole('button', { name: '启动 / 刷新 Runtime' }))
  await expect
    .element(screen.getByRole('list', { name: 'Runtime 授权' }))
    .toHaveTextContent('epoch 1 · 已授权')
  await user.click(screen.getByRole('button', { name: '读取备份' }))
  return { screen, user }
}

for (const operation of ['restore', 'retry'] as const) {
  it(`${operation} shows fresh revoked permissions after completion`, async () => {
    if (operation === 'retry') recoveryPhase = 'prepared'
    const { screen, user } = await connectedPanel()
    if (operation === 'restore')
      await user.selectOptions(
        screen.getByRole('combobox', { name: 'Runtime 备份' }),
        backupId
      )
    await user.click(
      screen.getByRole('button', {
        name: operation === 'restore' ? '恢复所选备份' : '继续未完成恢复',
      })
    )
    await expect
      .element(screen.getByRole('list', { name: 'Runtime 授权' }))
      .toHaveTextContent('epoch 2 · 已撤销')
    await expect
      .element(
        screen.getByRole('button', { name: '撤销 plugin-todo-list desktop' })
      )
      .not.toBeInTheDocument()
    await expect
      .element(screen.getByRole('button', { name: '提交任务' }))
      .toBeDisabled()
    await expect
      .element(screen.getByRole('button', { name: '继续未完成恢复' }))
      .toBeDisabled()
    expect(
      vi
        .mocked(storageManagedRuntime)
        .mock.calls.filter(([action]) => action.operation === operation)
    ).toHaveLength(1)
  })
}

it('cancelled restore preserves the verified permissions and instance', async () => {
  failure = 'APPROVAL_REQUIRED'
  const { screen, user } = await connectedPanel()
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Runtime 备份' }),
    backupId
  )
  await user.click(screen.getByRole('button', { name: '恢复所选备份' }))
  await expect
    .element(screen.getByTestId('managed-runtime-status'))
    .toHaveTextContent('APPROVAL_REQUIRED')
  await expect
    .element(screen.getByRole('list', { name: 'Runtime 授权' }))
    .toHaveTextContent('epoch 1 · 已授权')
  await expect
    .element(screen.getByTestId('managed-runtime-instance'))
    .toHaveTextContent('fixture-host')
})

it('failed restore hides stale approvals instead of claiming revocation', async () => {
  failure = 'STORAGE_FAILED'
  vi.mocked(storageManagedRuntime).mockResolvedValueOnce(report('complete'))
  const { screen, user } = await connectedPanel()
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Runtime 备份' }),
    backupId
  )
  await user.click(screen.getByRole('button', { name: '恢复所选备份' }))
  await expect
    .element(screen.getByTestId('managed-runtime-status'))
    .toHaveTextContent('STORAGE_FAILED')
  await expect
    .element(screen.getByRole('list', { name: 'Runtime 授权' }))
    .not.toHaveTextContent('已授权')
  await expect
    .element(screen.getByTestId('managed-permissions-status'))
    .toHaveTextContent('尚未核对')
  await expect
    .element(screen.getByRole('button', { name: '提交任务' }))
    .toBeDisabled()
  await expect
    .element(screen.getByTestId('managed-recovery-state'))
    .not.toBeInTheDocument()
  await expect
    .element(screen.getByRole('button', { name: '恢复所选备份' }))
    .toBeDisabled()
})

it('runtime connection timeout asks for state review without claiming a job deadline', async () => {
  vi.mocked(connectManagedRuntime).mockRejectedValueOnce(
    new RuntimeClientError('TIMEOUT')
  )
  const screen = await render(<ManagedRuntimePanel />)
  await userEvent
    .setup()
    .click(screen.getByRole('button', { name: '启动 / 刷新 Runtime' }))
  await expect
    .element(screen.getByTestId('managed-runtime-status'))
    .toHaveTextContent('TIMEOUT：Runtime 连接或管理操作')
  await expect
    .element(screen.getByTestId('managed-runtime-status'))
    .not.toHaveTextContent('任务超过截止时间')
  await expect
    .element(screen.getByRole('button', { name: '提交任务' }))
    .toBeDisabled()
  expect(connectManagedRuntime).toHaveBeenCalledTimes(1)
  expect(controlManagedRuntime).not.toHaveBeenCalled()
  expect(storageManagedRuntime).not.toHaveBeenCalled()
})

it('completed recovery remains complete when the fresh permission read fails', async () => {
  const { screen, user } = await connectedPanel()
  permissionReadFails = true
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Runtime 备份' }),
    backupId
  )
  await user.click(screen.getByRole('button', { name: '恢复所选备份' }))
  await expect
    .element(screen.getByTestId('managed-recovery-state'))
    .toHaveTextContent('complete')
  await expect
    .element(screen.getByRole('list', { name: 'Runtime 授权' }))
    .not.toHaveTextContent('已授权')
  await expect
    .element(screen.getByTestId('managed-permissions-status'))
    .toHaveTextContent('尚未核对')
  expect(
    vi
      .mocked(storageManagedRuntime)
      .mock.calls.filter(([action]) => action.operation === 'restore')
  ).toHaveLength(1)
})

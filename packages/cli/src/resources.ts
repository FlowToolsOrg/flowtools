import type { ResourceKind, ResourceOwner } from '@flowtools/runtime-client'

import {
  RuntimeClientError,
  RuntimeExecutionError,
} from '@flowtools/runtime-client'
import { isResourceCommand } from '@flowtools/sdk/manifest'
import { manifestPackageDigest } from '@flowtools/sdk/manifest/package'
import { Command } from 'commander'

import { getBuiltinCommandManifest } from './discovery'
import { connectHost, startUserRuntime } from './host'
import { startManagement } from './management'
import { userProfile } from './native-runtime'
import { parseJsonInput } from './schema'

export function addResourceCommands(program: Command) {
  const resources = program
    .command('resources')
    .description(
      'Explicitly select and bind a local read-only resource; paths never enter run input'
    )
  resources
    .command('select <path>')
    .option('--kind <file|directory>', 'Resource kind', 'file')
    .option('--plugin <id>', 'Build-owned plugin', 'plugin-resource-reader')
    .option('--command <id>', 'Declared command', 'run')
    .option('--slot <slot>', 'Declared resource slot', 'source')
    .option(
      '--approve-read',
      'Approve read-only access for the selected command and resource slot'
    )
    .option('--profile <directory>')
    .action(
      async (
        path: string,
        o: {
          kind: string
          approveRead?: boolean
          profile?: string
          plugin: string
          command: string
          slot: string
        }
      ) => {
        if (!o.approveRead) throw new Error('APPROVAL_REQUIRED')
        if (!['file', 'directory'].includes(o.kind))
          throw new Error('INPUT_INVALID')
        const manifest = getBuiltinCommandManifest(o.plugin)
        const command = manifest?.commands.find(c => c.id === o.command)
        if (!manifest || !command || !isResourceCommand(command))
          throw new Error('CAPABILITY_UNAVAILABLE')
        const permission = command.permissions[0]!
        if (permission.capability !== 'fs')
          throw new Error('CAPABILITY_UNAVAILABLE')
        if (
          !permission.scopes.some(
            s =>
              typeof s === 'object' &&
              s.kind === 'resource' &&
              s.slot === o.slot
          )
        )
          throw new Error('SCOPE_DENIED')
        const required = o.kind === 'file' ? 'read' : 'list'
        if (!permission.operations.includes(required))
          throw new Error('SCOPE_DENIED')
        const operations: ('read' | 'list' | 'stat')[] = [required]
        if (permission.operations.includes('stat')) operations.push('stat')
        const profile = userProfile(o.profile)
        await startUserRuntime(profile)
        const host = await startManagement(profile, false)
        let owner: ResourceOwner | undefined
        try {
          await host.client.call({
            method: 'resources.grant',
            payload: {
              formatVersion: 2,
              capabilityProfile: 'flowtools-native-v2',
              pluginId: manifest.id,
              commandId: o.command,
              target: 'cli',
              packageDigest: manifestPackageDigest(manifest),
              scopes: [
                {
                  slot: o.slot,
                  operations,
                },
              ],
              expiresAt: Date.now() + 300000,
              maxCalls: 4096,
            },
          })
          const created = await host.client.call({
            method: 'resources.owner',
            payload: {
              key: {
                pluginId: manifest.id,
                commandId: o.command,
                target: 'cli',
              },
            },
          })
          if (created.type !== 'resource-owner')
            throw new Error('INVALID_RESPONSE')
          owner = created.data
          const selected = await host.client.call({
            method: 'resources.select',
            payload: {
              owner: owner.id,
              slot: o.slot,
              kind: o.kind as ResourceKind,
              path,
            },
          })
          if (selected.type !== 'resource') throw new Error('INVALID_RESPONSE')
          print({
            owner: owner.id,
            pluginId: manifest.id,
            commandId: o.command,
            slot: o.slot,
            resource: selected.data,
          })
        } catch (error) {
          if (owner)
            await host.client
              .call({ method: 'resources.drop', payload: { owner: owner.id } })
              .catch(() => {})
          throw error
        } finally {
          await host.close()
        }
      }
    )
  resources
    .command('run <owner>')
    .option('--plugin <id>', 'Build-owned plugin', 'plugin-resource-reader')
    .option('--command <id>', 'Declared command', 'run')
    .option('--input <json>', 'Command input JSON', '{}')
    .option('--profile <directory>')
    .action(
      async (
        owner: string,
        o: { profile?: string; plugin: string; command: string; input: string }
      ) => {
        const input = parseJsonInput(o.input)
        const manifest = getBuiltinCommandManifest(o.plugin),
          command = manifest?.commands.find(c => c.id === o.command)
        if (!command || !isResourceCommand(command))
          throw new Error('CAPABILITY_UNAVAILABLE')
        const client = await connectHost(userProfile(o.profile), false, false)
        const idempotencyKey = crypto.randomUUID()
        let runId: string | undefined
        const cancel = () => {
          if (runId)
            void client
              .call({ method: 'jobs.cancel', payload: { runId } })
              .catch(() => {})
        }
        process.once('SIGINT', cancel)
        try {
          const accepted = await client.call({
            method: 'resources.submit',
            payload: {
              owner,
              submit: {
                pluginId: o.plugin,
                commandId: o.command,
                input,
                background: false,
                deadline:
                  Date.now() + Math.min(30000, command.resources.timeoutMs),
                idempotencyKey,
              },
            },
          })
          if (accepted.type !== 'receipt') throw new Error('INVALID_RESPONSE')
          runId = accepted.data.runId
          for (;;) {
            const job = await client.call({
              method: 'jobs.status',
              payload: { runId },
            })
            if (job.type !== 'job') throw new Error('INVALID_RESPONSE')
            if (
              ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(
                job.data.state
              )
            ) {
              print(job.data)
              if (job.data.state !== 'succeeded') process.exitCode = 1
              break
            }
            await new Promise(resolve => setTimeout(resolve, 100))
          }
        } catch (error) {
          if (error instanceof RuntimeClientError)
            throw new RuntimeExecutionError(
              error.acceptanceUnknown ? 'ACCEPTANCE_UNKNOWN' : error.code,
              idempotencyKey,
              runId
            )
          throw error
        } finally {
          process.removeListener('SIGINT', cancel)
          client.close()
        }
      }
    )
  resources
    .command('release <owner>')
    .option('--profile <directory>')
    .action(async (owner: string, o: { profile?: string }) => {
      const host = await startManagement(userProfile(o.profile), false)
      try {
        await host.client.call({ method: 'resources.drop', payload: { owner } })
        print({ released: true })
      } finally {
        await host.close()
      }
    })
}
function print(data: unknown) {
  process.stdout.write(
    JSON.stringify({ formatVersion: 1, success: true, data }) + '\n'
  )
}

import type { DependencyPlan } from '@flowtools/runtime-client'

import { Command } from 'commander'

import { getBuiltinPluginInfo } from './discovery'
import { connectHost } from './host'
import { userProfile } from './native-runtime'

export function formatDependencyPlan(plan: DependencyPlan): string {
  const lock = plan.lock
  return (
    [
      'Dependency plan (plan-only)',
      `Target: ${lock.target.platform}/${lock.target.arch}`,
      `Lock: ${lock.digest}`,
      `Roots: ${lock.roots.map(root => `${root.publisher}/${root.id}`).join(', ')}`,
      `Packages: ${lock.packages.length}; services: ${lock.services.length}; tools: ${lock.tools.length}`,
      ...lock.packages.map(
        pin =>
          `  Package ${pin.publisher}/${pin.id}@${pin.version} sha256:${pin.digest}`
      ),
      ...lock.services.map(
        pin =>
          `  Service ${pin.consumer.publisher}/${pin.consumer.id} -> ${pin.provider.publisher}/${pin.provider.id}@${pin.provider.version} ${pin.service}@${pin.version}`
      ),
      ...lock.tools.map(
        pin =>
          `  Tool ${pin.consumer.publisher}/${pin.consumer.id} -> ${pin.publisher}/${pin.id}@${pin.version} ${pin.target.platform}/${pin.target.arch}/${pin.buildFlavor} sha256:${pin.digest}`
      ),
      `Provider order: ${lock.topology.map(identity => `${identity.publisher}/${identity.id}`).join(' -> ')}`,
      'This plan does not install, activate or grant access to packages.',
    ].join('\n') + '\n'
  )
}

export function addDependencyCommands(program: Command) {
  const dependencies = program
    .command('dependencies')
    .description('Inspect Host-owned dependency plans')
  dependencies
    .command('plan <plugin-ids...>')
    .description('Resolve fixed built-ins without installing or activating')
    .option('--profile <directory>')
    .option('-f, --format <format>', 'json or text', 'text')
    .action(
      async (
        pluginIds: string[],
        options: { profile?: string; format: string }
      ) => {
        if (!['json', 'text'].includes(options.format))
          throw new Error('INVALID_REQUEST')
        if (pluginIds.length > 64) throw new Error('DEPENDENCY_BUDGET_EXCEEDED')
        if (new Set(pluginIds).size !== pluginIds.length)
          throw new Error('DEPENDENCY_INVALID')
        // Reject caller-selected unknown identities before profile or native IO.
        if (pluginIds.some(id => !getBuiltinPluginInfo(id)))
          throw new Error('DEPENDENCY_MISSING')
        const client = await connectHost(userProfile(options.profile))
        try {
          const plan = await client.dependenciesPlan(pluginIds)
          process.stdout.write(
            options.format === 'json'
              ? JSON.stringify({
                  formatVersion: 1,
                  success: true,
                  data: plan,
                }) + '\n'
              : formatDependencyPlan(plan)
          )
        } finally {
          client.close()
        }
      }
    )
  dependencies
    .command('unload-plan <plugin-id>')
    .description('Review reverse dependencies without uninstalling or stopping')
    .option('--profile <directory>')
    .option('-f, --format <format>', 'json or text', 'text')
    .action(
      async (
        pluginId: string,
        options: { profile?: string; format: string }
      ) => {
        if (!['json', 'text'].includes(options.format))
          throw new Error('INVALID_REQUEST')
        if (!getBuiltinPluginInfo(pluginId))
          throw new Error('DEPENDENCY_MISSING')
        const client = await connectHost(userProfile(options.profile))
        try {
          const plan = await client.providerUnloadPlan(pluginId)
          process.stdout.write(
            options.format === 'json'
              ? JSON.stringify({
                  formatVersion: 1,
                  success: true,
                  data: plan,
                }) + '\n'
              : `Provider unload plan (plan-only)\nProvider: ${plan.provider.publisher}/${plan.provider.id}@${plan.provider.version}\nPlan: ${plan.digest}\nConsumers: ${plan.consumers.map(id => `${id.publisher}/${id.id}`).join(', ') || 'none'}\nReview only; no packages or accepted tasks are removed.\n`
          )
        } finally {
          client.close()
        }
      }
    )
  dependencies
    .command('calls <run-id>')
    .description(
      'Inspect bounded service call metadata without private payloads'
    )
    .option('--profile <directory>')
    .option('-f, --format <format>', 'json or text', 'text')
    .action(
      async (runId: string, options: { profile?: string; format: string }) => {
        if (
          !['json', 'text'].includes(options.format) ||
          !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(
            runId
          )
        )
          throw new Error('INVALID_REQUEST')
        const client = await connectHost(userProfile(options.profile))
        try {
          const calls = await client.serviceCalls(runId)
          process.stdout.write(
            options.format === 'json'
              ? JSON.stringify({
                  formatVersion: 1,
                  success: true,
                  data: calls,
                }) + '\n'
              : `Service calls for ${runId}\n${calls.map(call => `${call.runId} parent:${call.parentRunId} ${call.provider.publisher}/${call.provider.id}@${call.provider.version} ${call.service}/${call.operation} ${call.state}${call.failureCode ? ' ' + call.failureCode : ''}`).join('\n') || 'No service calls'}\n`
          )
        } finally {
          client.close()
        }
      }
    )
}

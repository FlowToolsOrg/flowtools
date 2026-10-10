import {
  extensionContributionLimits,
  jsonBytes,
  parseContributionOwner,
  parseExtensionContributions,
} from './schema'
import {
  ExtensionContributionError,
  type ExtensionContributionDocument,
  type ExtensionContributionOwner,
  type ExtensionContributionSnapshot,
  type RegisteredExtensionContribution,
} from './types'

interface OwnedContributions {
  owner: ExtensionContributionOwner
  document: ExtensionContributionDocument
  bytes: number
  token: symbol
}

const sameOwner = (
  left: ExtensionContributionOwner,
  right: ExtensionContributionOwner
): boolean =>
  left.pluginId === right.pluginId &&
  left.version === right.version &&
  left.generation === right.generation

const compare = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0

/**
 * Host-only, cooperative T1 bookkeeping. This is neither a plugin capability
 * nor a loader, isolation boundary, grant or persistent store.
 */
export class ExtensionContributionRegistry {
  private entries = new Map<string, OwnedContributions>()
  // Retained after revocation to refuse older contribution epochs.
  private owners = new Map<string, ExtensionContributionOwner>()
  private listeners = new Set<() => void>()
  private snapshot: ExtensionContributionSnapshot = Object.freeze([])

  readonly getSnapshot = (): ExtensionContributionSnapshot => this.snapshot

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  /** Host allocates fresh epochs even after plugin reinstall or projector restart. */
  createOwner(pluginId: string, version: string): ExtensionContributionOwner {
    const known = this.owners.get(pluginId)
    if (
      (!known && this.owners.size >= extensionContributionLimits.maxOwners) ||
      known?.generation === Number.MAX_SAFE_INTEGER
    )
      throw new ExtensionContributionError('BUDGET_EXCEEDED')
    const owner = parseContributionOwner({
      pluginId,
      version,
      generation: (known?.generation ?? 0) + 1,
    })
    this.owners.set(pluginId, owner)
    return owner
  }

  /** Validate the complete replacement before changing any visible state. */
  replace(ownerValue: ExtensionContributionOwner, value: unknown): () => void {
    const owner = parseContributionOwner(ownerValue)
    const document = parseExtensionContributions(value)
    const known = this.owners.get(owner.pluginId)
    if (
      known &&
      (owner.generation < known.generation ||
        (owner.generation === known.generation &&
          owner.version !== known.version))
    )
      throw new ExtensionContributionError('OWNER_EXPIRED')
    if (!known && this.owners.size >= extensionContributionLimits.maxOwners)
      throw new ExtensionContributionError('BUDGET_EXCEEDED')

    const bytes = jsonBytes(document)
    const current = this.entries.get(owner.pluginId)
    let totalBytes = bytes
    let totalContributions = document.contributions.length
    for (const [pluginId, entry] of this.entries) {
      if (pluginId === owner.pluginId) continue
      totalBytes += entry.bytes
      totalContributions += entry.document.contributions.length
    }
    if (
      totalBytes > extensionContributionLimits.maxRegistryBytes ||
      totalContributions > extensionContributionLimits.maxContributions
    )
      throw new ExtensionContributionError('BUDGET_EXCEEDED')

    const token = Symbol(owner.pluginId)
    this.owners.set(owner.pluginId, owner)
    this.entries.set(owner.pluginId, { owner, document, bytes, token })
    if (
      !current ||
      !sameOwner(current.owner, owner) ||
      JSON.stringify(current.document) !== JSON.stringify(document)
    )
      this.publish()

    return () => {
      if (this.entries.get(owner.pluginId)?.token === token) this.revoke(owner)
    }
  }

  /** An old generation cannot revoke a newer generation's declarations. */
  revoke(ownerValue: ExtensionContributionOwner): boolean {
    const owner = parseContributionOwner(ownerValue)
    const current = this.entries.get(owner.pluginId)
    if (!current || !sameOwner(current.owner, owner)) return false
    this.entries.delete(owner.pluginId)
    this.publish()
    return true
  }

  private publish(): void {
    const next: RegisteredExtensionContribution[] = []
    for (const { owner, document } of this.entries.values())
      for (const contribution of document.contributions)
        next.push(
          Object.freeze({
            ...contribution,
            key: `${owner.pluginId}:${contribution.id}`,
            owner,
          })
        )
    next.sort(
      (left, right) =>
        compare(left.owner.pluginId, right.owner.pluginId) ||
        compare(left.kind, right.kind) ||
        compare(left.id, right.id)
    )
    // Sorting makes input order irrelevant to both snapshots and notifications.
    if (JSON.stringify(next) === JSON.stringify(this.snapshot)) return
    this.snapshot = Object.freeze(next)
    const listeners = Array.from(this.listeners)
    for (const listener of listeners) {
      try {
        listener()
      } catch {
        // Observer failures cannot undo committed data or block other observers.
      }
    }
  }
}

/** Declarative metadata only; contribution application starts with E02. */
export type ExtensionContributionKind = 'theme' | 'locale' | 'settings'

export type ExtensionContributionValue =
  | null
  | boolean
  | number
  | string
  | readonly ExtensionContributionValue[]
  | { readonly [key: string]: ExtensionContributionValue }

export interface ExtensionContribution {
  /** Unique within this owner's document, including across kinds. */
  readonly id: string
  readonly kind: ExtensionContributionKind
  /** Opaque JSON data; this does not authorize CSS, translation or storage. */
  readonly value: ExtensionContributionValue
}

export interface ExtensionContributionDocument {
  readonly formatVersion: 1
  readonly contributions: readonly ExtensionContribution[]
}

/** Supplied by the trusted Host, never taken from contribution payloads. */
export interface ExtensionContributionOwner {
  readonly pluginId: string
  readonly version: string
  /** Monotonic Host contribution epoch, independent of loaded-module generations. */
  readonly generation: number
}

export interface RegisteredExtensionContribution extends ExtensionContribution {
  /** Namespaced by the Host; contribution IDs cannot contain colons. */
  readonly key: string
  readonly owner: ExtensionContributionOwner
}

export type ExtensionContributionSnapshot =
  readonly RegisteredExtensionContribution[]

export type ExtensionContributionErrorCode =
  | 'INVALID_DOCUMENT'
  | 'INVALID_OWNER'
  | 'BUDGET_EXCEEDED'
  | 'OWNER_EXPIRED'

export class ExtensionContributionError extends Error {
  constructor(readonly code: ExtensionContributionErrorCode) {
    super(code)
    this.name = 'ExtensionContributionError'
  }
}

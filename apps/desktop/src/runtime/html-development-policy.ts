import { ExternalCodeDisabledError } from '@flowtools/sdk'

/** Host build settings only: metadata, saved state and caller modes cannot grant. */
interface PreviewBuildMeta {
  readonly env?: {
    readonly DEV?: unknown
    readonly VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW?: unknown
  }
}

export const unsafeHtmlPreviewEnabled =
  (import.meta as PreviewBuildMeta).env?.DEV === true &&
  (import.meta as PreviewBuildMeta).env?.VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW ===
    '1'

export function assertUnsafeHtmlPreview(): void {
  if (!unsafeHtmlPreviewEnabled) throw new ExternalCodeDisabledError()
}

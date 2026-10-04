import { ExternalCodeDisabledError } from './plugin-file-loader'

interface PreviewImportMeta {
  readonly env?: {
    readonly DEV?: unknown
    readonly VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW?: unknown
  }
}

/** Host build settings, never plugin metadata, query parameters or saved grants. */
export function unsafeDevelopmentPreviewEnabled(): boolean {
  return (
    (import.meta as PreviewImportMeta).env?.DEV === true &&
    (import.meta as PreviewImportMeta).env
      ?.VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW === '1'
  )
}

export function assertUnsafeDevelopmentPreview(): void {
  if (!unsafeDevelopmentPreviewEnabled()) {
    throw new ExternalCodeDisabledError()
  }
}

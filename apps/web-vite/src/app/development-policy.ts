import { ExternalCodeDisabledError } from '@flowtools/sdk'

export const unsafePluginPreviewEnabled =
  import.meta.env?.DEV === true &&
  import.meta.env?.VITE_ENABLE_UNSAFE_PLUGIN_PREVIEW === '1'

export function assertUnsafePluginPreview(): void {
  if (!unsafePluginPreviewEnabled) {
    throw new ExternalCodeDisabledError()
  }
}

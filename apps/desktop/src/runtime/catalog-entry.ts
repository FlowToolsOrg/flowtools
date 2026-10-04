import { portablePluginPathSchema } from '@flowtools/sdk/compat/catalog'

/** Development path resolution only; not package verification or a native scope. */
export function resolveCatalogEntry(
  reference: { packageRoot: string; entry: string },
  options: { development: boolean; checkoutRoot?: string }
): string | undefined {
  const root = options.checkoutRoot
  if (
    !options.development ||
    !root ||
    !/^(?:[a-z]:[\\/]|\/)/i.test(root) ||
    /[\u0000-\u001f?#]/.test(root)
  )
    return undefined
  if (
    !portablePluginPathSchema.safeParse(reference.packageRoot).success ||
    !portablePluginPathSchema.safeParse(reference.entry).success
  )
    return undefined
  return `${root.replace(/\\/g, '/').replace(/\/+$/, '')}/${reference.packageRoot}/${reference.entry}`
}

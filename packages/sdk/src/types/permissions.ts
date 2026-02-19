/**
 * Permission keys declared by plugins and enforced by the host runtime.
 */
export type Permission =
  | 'fs'
  | 'network'
  | 'clipboard'
  | 'dialog'
  | 'notification'
  | 'storage'
  | 'db'
  | 'native'

/**
 * Static list of all supported permissions.
 */
export const permissions: readonly Permission[] = [
  'fs',
  'network',
  'clipboard',
  'dialog',
  'notification',
  'storage',
  'db',
  'native',
]

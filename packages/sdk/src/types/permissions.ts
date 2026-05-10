/**
 * Permission keys declared by plugins and enforced by the host runtime.
 */
export type Permission = (typeof permissions)[number]

/**
 * Static list of all supported permissions.
 */
export const permissions = [
  'fs',
  'network',
  'clipboard',
  'dialog',
  'notification',
  'storage',
  'db',
  'native',
] as const

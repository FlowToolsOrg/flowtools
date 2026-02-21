import { Permission } from '../types/permissions'

export function pickCapability<T>(
  permission: Permission,
  allowedPermissions: ReadonlySet<Permission>,
  createCapability: () => T
): T | undefined {
  if (!allowedPermissions.has(permission)) {
    return undefined
  }

  return createCapability()
}

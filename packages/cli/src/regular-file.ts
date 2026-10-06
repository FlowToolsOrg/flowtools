import { lstat, realpath } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'

/** Accept Windows short-name aliases, but never a redirected ancestor. */
export async function regularFile(path: string, maxBytes = Infinity) {
  const absolute = resolve(path)
  let current = absolute
  while (true) {
    const stat = await lstat(current)
    if (
      stat.isSymbolicLink() ||
      (current === absolute && (!stat.isFile() || stat.size > maxBytes)) ||
      (current !== absolute && !stat.isDirectory())
    )
      throw new Error('SETUP_REQUIRED')
    const parent = dirname(current)
    if (parent === current) break
    current = parent
  }
  return realpath(absolute)
}

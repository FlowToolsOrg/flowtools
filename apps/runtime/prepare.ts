import { createHash } from 'node:crypto'
import {
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  writeFileSync,
} from 'node:fs'
import { resolve } from 'node:path'

// Local validation bootstrap is created by the build, never by an IPC caller.
const root = resolve(import.meta.dir, '../..')
const regular = (path: string) => {
  if (
    !lstatSync(path).isFile() ||
    lstatSync(path).isSymbolicLink() ||
    realpathSync(path).toLowerCase() !== path.toLowerCase()
  )
    throw new Error('Redirected runner artifact')
  return {
    path,
    sha256: createHash('sha256').update(readFileSync(path)).digest('hex'),
  }
}
mkdirSync(resolve(import.meta.dir, '.generated'), { recursive: true })
writeFileSync(
  resolve(import.meta.dir, '.generated/runner.json'),
  JSON.stringify({
    bun: regular(process.execPath),
    runner: regular(resolve(root, 'packages/plugin-runner/dist/runner.js')),
  })
)

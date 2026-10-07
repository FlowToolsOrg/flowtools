import { expect, test } from 'bun:test'
import { mkdtemp, mkdir, realpath, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { regularFile } from './regular-file'

test('regular file canonicalizes aliases and refuses redirected ancestors and oversized files', async () => {
  const root = await mkdtemp(join(tmpdir(), 'flowtools-validation-path-'))
  const directory = join(root, 'ordinary')
  await mkdir(directory)
  const file = join(directory, 'fixture')
  await writeFile(file, 'fixture')
  expect(await regularFile(file)).toBe(await realpath(file))
  expect(await regularFile(join(directory, '.', 'fixture'))).toBe(
    await realpath(file)
  )
  expect(regularFile(file, 3)).rejects.toThrow('SETUP_REQUIRED')
  expect(regularFile(directory)).rejects.toThrow('SETUP_REQUIRED')
  const redirected = join(root, 'redirected')
  await symlink(directory, redirected, 'junction')
  expect(regularFile(join(redirected, 'fixture'))).rejects.toThrow(
    'SETUP_REQUIRED'
  )
})

import { expect, test } from 'bun:test'

import { Command } from 'commander'

import { addToolCommands } from './tools'

test('tool inventory rejects malformed cursors before any profile or native lookup', async () => {
  for (const args of [
    ['--offset', '1'],
    ['--offset', '-8'],
    ['--offset', '72'],
    ['--offset', '8'],
    ['--snapshot', 'C:/private-selector'],
    ['--snapshot', 'A'.repeat(64)],
  ]) {
    const program = new Command()
      .exitOverride()
      .configureOutput({ outputError: () => {} })
    addToolCommands(program)
    const failure: unknown = await program
      .parseAsync([
        'node',
        'flowtools',
        'tools',
        'list',
        ...args,
        '--profile',
        'PRIVATE_NONEXISTENT_INVENTORY_PROFILE',
      ])
      .catch((error: unknown) => error)
    if (!(failure instanceof Error))
      throw new Error('Expected parameter refusal')
    expect(failure.message).toContain('INPUT_INVALID')
  }
})

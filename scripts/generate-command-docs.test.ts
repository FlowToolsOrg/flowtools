import { expect, test } from 'bun:test'

import { generateCommandDocs, renderCommandDocs } from './generate-command-docs'
import { verifyBuiltinManifests } from './verify-manifests'

test('generated command reference is current and uses actual serialized contracts', async () => {
  await generateCommandDocs(true)
  const manifests = verifyBuiltinManifests()
  const output = renderCommandDocs(manifests)
  for (const manifest of manifests)
    for (const command of manifest.commands) {
      expect(output).toContain(`flowtools/${manifest.id}/${command.id}`)
      expect(output).toContain(JSON.stringify(command.inputSchema, null, 2))
      expect(output).toContain(JSON.stringify(command.outputSchema, null, 2))
    }
})

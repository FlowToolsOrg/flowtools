// Public deterministic TEST keys only. Never use these keys as production pins.
// Run with Node after SDK build; ordinary tests read the committed golden bytes.
import {
  createHash,
  createPrivateKey,
  createPublicKey,
  sign,
} from 'node:crypto'
import { writeFileSync } from 'node:fs'

import {
  PACKAGE_PAYLOAD_TYPE,
  ROOT_PAYLOAD_TYPE,
  packagePreAuthenticationEncoding,
  parsePluginManifest,
  serializePackageDescriptor,
} from '../../dist/manifest.js'

const sha256 = bytes => createHash('sha256').update(bytes).digest('hex')
const key = byte => {
  const privateKey = createPrivateKey({
    key: Buffer.concat([
      Buffer.from('302e020100300506032b657004220420', 'hex'),
      Buffer.alloc(32, byte),
    ]),
    format: 'der',
    type: 'pkcs8',
  })
  const raw = createPublicKey(privateKey)
    .export({ format: 'der', type: 'spki' })
    .subarray(-32)
  return {
    privateKey,
    public: { keyid: sha256(raw), publicKey: raw.toString('base64') },
  }
}
const roots = [key(1), key(2)]
const publisher = key(3)
const now = 1_800_000_000
// A deterministic Python zipfile ZIP_STORED fixture (regular files, 2026-01-01).
const archive = Buffer.from(
  'UEsDBBQAAAAAAAAAIVxTBtFCLQIAAC0CAAANAAAAbWFuaWZlc3QuanNvbnsiZm9ybWF0VmVyc2lvbiI6MSwicHVibGlzaGVyIjoiZml4dHVyZS1wdWJsaXNoZXIiLCJpZCI6ImZpeHR1cmUtcGx1Z2luIiwibmFtZSI6IkZpeHR1cmUiLCJkZXNjcmlwdGlvbiI6IlNpZ25lZCBkZXNjcmlwdG9yIGZpeHR1cmUsIG5ldmVyIGV4ZWN1dGUiLCJ2ZXJzaW9uIjoiMS4wLjAiLCJ0eXBlIjoidG9vbCIsIm1hdHVyaXR5IjoicHJvdG90eXBlIiwiZW5naW5lcyI6eyJob3N0IjoiXjAuMS4wIiwic2RrIjoiPj0wLjAuMCA8MS4wLjAifSwidGFyZ2V0cyI6W3sicGxhdGZvcm0iOiJ3aW5kb3dzIiwiYXJjaCI6Ing2NCJ9XSwiZW50cmllcyI6eyJleGVjdXRvciI6ImRpc3QvY29tbWFuZHMuanMifSwiZmlsZXMiOlt7InBhdGgiOiJkaXN0L2NvbW1hbmRzLmpzIiwic2l6ZSI6NDQsInNoYTI1NiI6IjczZjcxY2RhNWQxZDE2ZGY3MWQ2MWI3NTYwMDk5MDk0ZGFkMDkzZDhkZTM5ZmM2NGE1MDRlZGI3NTVmMjAyMjYifV0sImNvbW1hbmRzIjpbXSwiZGVwZW5kZW5jaWVzIjp7InNlcnZpY2VzIjpbXSwidG9vbHMiOltdfSwic2lnbmF0dXJlIjp7InN0YXR1cyI6InVuc2lnbmVkIn19UEsDBBQAAAAAAAAAIVwQx5hPLAAAACwAAAAQAAAAZGlzdC9jb21tYW5kcy5qc3Rocm93IG5ldyBFcnJvcignRklYVFVSRV9NVVNUX05PVF9FWEVDVVRFJykKUEsBAhQAFAAAAAAAAAAhXFMG0UItAgAALQIAAA0AAAAAAAAAAAAAAKSBAAAAAG1hbmlmZXN0Lmpzb25QSwECFAAUAAAAAAAAACFcEMeYTywAAAAsAAAAEAAAAAAAAAAAAAAApIFYAgAAZGlzdC9jb21tYW5kcy5qc1BLBQYAAAAAAgACAHkAAACyAgAAAAA=',
  'base64'
)
// These offsets belong only to this fixed stored fixture; this is no ZIP parser.
const manifestBytes = archive.subarray(43, 43 + 557)
/** @type {unknown} */
const manifestInput = JSON.parse(manifestBytes.toString('utf8'))
const manifest = parsePluginManifest(manifestInput)
const descriptor = {
  formatVersion: 1,
  kind: 'plugin',
  publisher: manifest.publisher,
  id: manifest.id,
  version: manifest.version,
  releaseSequence: 1,
  target: manifest.targets[0],
  issuedAt: now - 10,
  expiresAt: now + 86_400,
  manifest: { sha256: sha256(manifestBytes), size: manifestBytes.length },
  archive: { format: 'zip', sha256: sha256(archive), size: archive.length },
  files: manifest.files,
  source: { registry: 'fixture-registry', artifact: 'fixture-plugin' },
  license: 'MIT',
  buildFlavor: 'default',
}
const rootPayload = {
  formatVersion: 1,
  version: 1,
  previousRootSha256: null,
  issuedAt: now - 10,
  expiresAt: now + 86_400,
  rootKeys: roots.map(key => key.public),
  rootThreshold: 2,
  publishers: [
    {
      publisher: manifest.publisher,
      keys: [publisher.public],
      threshold: 1,
      packages: [
        { kind: 'plugin', id: manifest.id, targets: manifest.targets },
      ],
      provenance: {
        authority: 'fixture-maintainer',
        subject: 'fixture-author',
      },
    },
  ],
  revokedKeyIds: [],
  revokedPackageDigests: [],
}
/**
 * @param {typeof PACKAGE_PAYLOAD_TYPE | typeof ROOT_PAYLOAD_TYPE} payloadType
 * @param {Uint8Array} payload
 * @param {ReturnType<typeof key>[]} keys
 */
const envelope = (payloadType, payload, keys) => ({
  payloadType,
  payload: Buffer.from(payload).toString('base64'),
  signatures: keys.map(key => ({
    keyid: key.public.keyid,
    sig: sign(
      null,
      packagePreAuthenticationEncoding(payloadType, payload),
      key.privateKey
    ).toString('base64'),
  })),
})
const payload = serializePackageDescriptor(descriptor)
const rootBytes = Buffer.from(JSON.stringify(rootPayload))
const fixture = {
  warning:
    'Public deterministic fixture keys; never install or execute this package.',
  now,
  pins: roots.map(key => key.public),
  threshold: 2,
  rootEnvelope: envelope(ROOT_PAYLOAD_TYPE, rootBytes, roots),
  packageEnvelope: envelope(PACKAGE_PAYLOAD_TYPE, payload, [publisher]),
  rootDigest: sha256(rootBytes),
  packageDigest: sha256(archive),
  descriptorDigest: sha256(payload),
  paeBase64: Buffer.from(
    packagePreAuthenticationEncoding(PACKAGE_PAYLOAD_TYPE, payload)
  ).toString('base64'),
  descriptor,
  manifestBase64: manifestBytes.toString('base64'),
  archiveBase64: archive.toString('base64'),
}
writeFileSync(
  new URL('./signed-package-v1.json', import.meta.url),
  `${JSON.stringify(fixture, null, 2)}\n`
)

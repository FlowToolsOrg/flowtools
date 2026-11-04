import { spawnSync } from 'node:child_process'
// Explicit developer packaging; normal build/Runtime never reads signing keys.
import {
  createHash,
  createPrivateKey,
  createPublicKey,
  generateKeyPairSync,
  sign,
} from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

const hash = (data: Uint8Array) =>
  createHash('sha256').update(data).digest('hex')
const encode = (value: unknown) => Buffer.from(JSON.stringify(value))
type Key = { keyid: string; publicKey: string; privateKey: string }
function fresh(): Key {
  const pair = generateKeyPairSync('ed25519')
  const publicBytes = pair.publicKey
    .export({ type: 'spki', format: 'der' })
    .subarray(-32)
  return {
    keyid: hash(publicBytes),
    publicKey: publicBytes.toString('base64'),
    privateKey: pair.privateKey
      .export({ type: 'pkcs8', format: 'der' })
      .toString('base64'),
  }
}
function keyStore(
  path: string,
  mode: 'Protect' | 'Unprotect',
  input?: string
): string {
  const child = spawnSync(
    'pwsh',
    [
      '-NoProfile',
      '-File',
      join(import.meta.dir, 'media-candidate/protect-local-signing.ps1'),
      '-Mode',
      mode,
      '-Path',
      path,
    ],
    {
      input,
      encoding: 'utf8',
      timeout: 15000,
      maxBuffer: 32768,
      windowsHide: true,
    }
  )
  if (child.status !== 0 || child.error)
    throw new Error('Local protected signing material unavailable')
  return child.stdout
}
function envelope(type: string, value: unknown, keys: Key[]) {
  const payload = encode(value)
  const pae = Buffer.concat([
    Buffer.from(`DSSEv1 ${Buffer.byteLength(type)} ${type} ${payload.length} `),
    payload,
  ])
  return {
    payloadType: type,
    payload: payload.toString('base64'),
    signatures: keys.map(key => ({
      keyid: key.keyid,
      sig: sign(
        null,
        pae,
        createPrivateKey({
          key: Buffer.from(key.privateKey, 'base64'),
          type: 'pkcs8',
          format: 'der',
        })
      ).toString('base64'),
    })),
  }
}
function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff
  for (const byte of bytes) {
    crc ^= byte
    for (let n = 0; n < 8; n++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0)
  }
  return (crc ^ 0xffffffff) >>> 0
}
function zip(entries: Array<[string, Buffer]>) {
  const files: Buffer[] = [],
    central: Buffer[] = []
  let offset = 0
  for (const [path, bytes] of entries) {
    const name = Buffer.from(path),
      crc = crc32(bytes)
    const local = Buffer.alloc(30)
    local.writeUInt32LE(0x04034b50)
    local.writeUInt16LE(20, 4)
    local.writeUInt16LE(33, 12)
    local.writeUInt32LE(crc, 14)
    local.writeUInt32LE(bytes.length, 18)
    local.writeUInt32LE(bytes.length, 22)
    local.writeUInt16LE(name.length, 26)
    files.push(local, name, bytes)
    const item = Buffer.alloc(46)
    item.writeUInt32LE(0x02014b50)
    item.writeUInt16LE(20, 4)
    item.writeUInt16LE(20, 6)
    item.writeUInt16LE(33, 14)
    item.writeUInt32LE(crc, 16)
    item.writeUInt32LE(bytes.length, 20)
    item.writeUInt32LE(bytes.length, 24)
    item.writeUInt16LE(name.length, 28)
    item.writeUInt32LE(offset, 42)
    central.push(item, name)
    offset += local.length + name.length + bytes.length
  }
  const index = Buffer.concat(central),
    end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50)
  end.writeUInt16LE(entries.length, 8)
  end.writeUInt16LE(entries.length, 10)
  end.writeUInt32LE(index.length, 12)
  end.writeUInt32LE(offset, 16)
  return Buffer.concat([...files, index, end])
}
if (import.meta.main) {
  const [buildArgument, keysArgument] = process.argv.slice(2)
  if (!buildArgument || !keysArgument || process.argv.length !== 4)
    throw new Error(
      'Expected verified official build root and protected developer key file'
    )
  const build = resolve(buildArgument),
    keysPath = resolve(keysArgument)
  const pin = JSON.parse(
    readFileSync(
      join(import.meta.dir, 'media-candidate/official-8.1.3-lock.json'),
      'utf8'
    )
  ) as {
    programs: Record<string, string>
    notices: Array<{ path: string; sha256: string }>
  }
  const entries: Array<[string, Buffer]> = []
  for (const role of ['ffmpeg', 'ffprobe']) {
    const data = readFileSync(join(build, `candidate/bin/${role}.exe`))
    if (
      data.length < 1024 ||
      data.length > 33554432 ||
      hash(data) !== pin.programs[role]
    )
      throw new Error('Exact verified official-source program required')
    entries.push([`bin/${role}.exe`, data])
  }
  for (const notice of pin.notices) {
    const bytes = readFileSync(join(build, notice.path))
    if (hash(bytes) !== notice.sha256)
      throw new Error('Exact license material required')
    entries.push([notice.path.replace('candidate/', ''), bytes])
  }
  entries.sort(([a], [b]) => a.localeCompare(b, 'en'))
  const publicKey = ({ keyid, publicKey }: Key) => ({ keyid, publicKey })
  let keys: Key[]
  if (existsSync(keysPath))
    keys = JSON.parse(keyStore(keysPath, 'Unprotect')) as Key[]
  else {
    keys = [fresh(), fresh(), fresh()]
    keyStore(keysPath, 'Protect', JSON.stringify(keys))
  }
  if (
    keys.length !== 3 ||
    new Set(keys.map(key => key.keyid)).size !== 3 ||
    keys.some(
      key =>
        hash(
          createPublicKey(
            createPrivateKey({
              key: Buffer.from(key.privateKey, 'base64'),
              format: 'der',
              type: 'pkcs8',
            })
          )
            .export({ format: 'der', type: 'spki' })
            .subarray(-32)
        ) !== key.keyid
    )
  )
    throw new Error('Local signing key set refused')
  const publisher = 'flowtools-builtin',
    target = { platform: 'windows', arch: 'x64' }
  const files = entries.map(([path, bytes]) => ({
    path,
    size: bytes.length,
    sha256: hash(bytes),
  }))
  const source = {
    registry: 'ffmpeg-official',
    artifact: 'ffmpeg-8-1-3-flowtools-t1',
  }
  const license = 'LGPL-2.1-or-later',
    buildFlavor = 'flowtools-t1-lgpl-static-v1'
  const manifest = encode({
    formatVersion: 1,
    kind: 'tool',
    publisher,
    id: 'ffmpeg',
    version: '8.1.3',
    target,
    buildFlavor,
    source,
    license,
    files,
    programs: [
      { role: 'ffmpeg', path: 'bin/ffmpeg.exe' },
      { role: 'ffprobe', path: 'bin/ffprobe.exe' },
    ],
    resources: entries
      .filter(([path]) => path.startsWith('licenses/'))
      .map(([path]) => path),
  })
  const archive = zip([['manifest.json', manifest], ...entries])
  const issuedAt = Math.floor(Date.now() / 1000) - 30
  const root = {
    formatVersion: 1,
    version: 1,
    previousRootSha256: null,
    issuedAt,
    expiresAt: issuedAt + 31536000,
    rootKeys: keys.slice(0, 2).map(publicKey),
    rootThreshold: 2,
    publishers: [
      {
        publisher,
        keys: [publicKey(keys[2]!)],
        threshold: 1,
        packages: [{ kind: 'tool', id: 'ffmpeg', targets: [target] }],
        provenance: {
          authority: 'flowtools-local-t1',
          subject: 'maintainer-approved-windows-prototype',
        },
      },
    ],
    revokedKeyIds: [],
    revokedPackageDigests: [],
  }
  const payload = {
    formatVersion: 1,
    kind: 'tool',
    publisher,
    id: 'ffmpeg',
    version: '8.1.3',
    releaseSequence: 1,
    target,
    issuedAt,
    expiresAt: issuedAt + 2592000,
    manifest: { sha256: hash(manifest), size: manifest.length },
    archive: { format: 'zip', sha256: hash(archive), size: archive.length },
    files,
    source,
    license,
    buildFlavor,
  }
  const output = join(
    import.meta.dir,
    '../packages/runtime-core/assets/builtin-media'
  )
  mkdirSync(output, { recursive: true })
  for (const [name, data] of [
    ['package.zip', archive],
    ['manifest.json', manifest],
    [
      'pins.json',
      encode({ keys: keys.slice(0, 2).map(publicKey), threshold: 2 }),
    ],
    [
      'root.dsse.json',
      encode(
        envelope(
          'application/vnd.flowtools.trust-root.v1+json',
          root,
          keys.slice(0, 2)
        )
      ),
    ],
    [
      'package.dsse.json',
      encode(
        envelope('application/vnd.flowtools.package.v1+json', payload, [
          keys[2]!,
        ])
      ),
    ],
    [
      'bundle-lock.json',
      encode({
        formatVersion: 1,
        maturity: 'prototype',
        archiveSha256: hash(archive),
        manifestSha256: hash(manifest),
        programs: pin.programs,
        issuedAt,
        expiresAt: payload.expiresAt,
        sourceSigner: 'FCF986EA15E6E293A5644F10B4322F04D67658D8',
        authorization:
          'Windows fixed T1 prototype; formal distribution and independent review pending',
      }),
    ],
  ] as const) {
    const path = join(output, name)
    if (existsSync(path))
      throw new Error(
        'Existing packaged release preserved; use a separately reviewed release update'
      )
    writeFileSync(path, data, { flag: 'wx' })
  }
  process.stdout.write(
    `Packaged fixed Windows/T1 media: ${archive.length} bytes; archive ${hash(archive)}; private signing keys remain protected outside the repository.\n`
  )
}

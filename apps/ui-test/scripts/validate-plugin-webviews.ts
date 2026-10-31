import type { Browser, Page } from 'playwright'

import assert from 'node:assert/strict'
import { execFileSync, spawn, spawnSync } from 'node:child_process'
import { once } from 'node:events'
import { mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises'
import { createServer, get } from 'node:https'
import { createServer as createTcpServer } from 'node:net'
import { tmpdir } from 'node:os'
import { basename, join, resolve, win32 } from 'node:path'

import { chromium } from 'playwright'

import {
  assertOwnedWebviewCdp,
  getOwnedNativeDescendants,
  isValidationProcessRoot,
  probeLoopbackCdp,
  terminateNativeProcess,
  type NativeListener,
  type NativeProcess,
} from './plugin-webview-cdp.ts'

const root = resolve(import.meta.dirname, '../../..')
const interruptAfterReady = process.argv.includes(
  '--interrupt-after-native-ready'
)
const interruptBeforeSnapshot = process.argv.includes(
  '--interrupt-before-cdp-snapshot'
)
const expectInterruption = interruptAfterReady || interruptBeforeSnapshot
const executable = join(
  process.env.CARGO_TARGET_DIR
    ? resolve(process.env.CARGO_TARGET_DIR)
    : join(root, 'apps/desktop/src-tauri/target'),
  'debug/validate-plugin-webviews.exe'
)
const identity = JSON.parse(
  execFileSync(executable, ['--validation-identity'], {
    windowsHide: true,
    encoding: 'utf8',
    env: { SystemRoot: process.env.SystemRoot },
  })
) as { identifier: string; title: string; mode: string }
assert.deepEqual(identity, {
  identifier: 'com.flowtools.g5-plugin-webviews-validation-20261012',
  title: 'FlowTools G5 WebView validation',
  mode: 'g5-webview-feasibility-v1',
  nativeAclCleared: true,
  productionAclControl: true,
})

// Reserve simultaneously, then close. A competing bind fails the validation;
// there is no port retry or alternate browser fallback.
const reservations = await Promise.all(
  Array.from({ length: 4 }, async () => {
    const server = createTcpServer()
    server.listen(0, '127.0.0.1')
    await once(server, 'listening')
    const address = server.address()
    assert.ok(address && typeof address === 'object')
    return { server, port: address.port }
  })
)
const ports = reservations.map(({ port }) => port)
await Promise.all(
  reservations.map(
    ({ server }) => new Promise<void>(resolve => server.close(() => resolve()))
  )
)
const scratch = await mkdtemp(join(tmpdir(), 'flowtools-plugin-webview-'))
const instance = basename(scratch)
const nativeEnvironment = {
  SystemRoot: process.env.SystemRoot,
  USERPROFILE: process.env.USERPROFILE,
  TEMP: process.env.TEMP,
  TMP: process.env.TMP,
  FLOWTOOLS_PLUGIN_WEBVIEW_VALIDATION_ROOT: scratch,
  FLOWTOOLS_PLUGIN_WEBVIEW_VALIDATION_CDP_PORTS: ports.join(','),
}
const output = join(
  root,
  interruptBeforeSnapshot
    ? 'execution-validation/g5-webviews-startup-interruption'
    : interruptAfterReady
      ? 'execution-validation/g5-webviews-interruption'
      : 'execution-validation/g5-webviews'
)
await mkdir(output, { recursive: true })
await writeFile(
  join(output, 'evidence.json'),
  JSON.stringify({ status: 'running', identity })
)
const mismatchedInstance = spawnSync(
  executable,
  ['--validation-instance=invalid'],
  { env: nativeEnvironment, encoding: 'utf8', windowsHide: true, timeout: 5000 }
)
assert.equal(mismatchedInstance.error, undefined)
assert.equal(mismatchedInstance.status, 101)
assert.match(
  mismatchedInstance.stderr,
  /Exact disposable validation instance argument required/
)
assert.deepEqual(
  await readdir(scratch),
  [],
  'Instance refusal must precede profile creation'
)
await writeFile(
  join(output, 'instance-admission.json'),
  JSON.stringify({ status: 101, profileEntries: [] })
)
let externalRequests = 0
let externalConnections = 0
let externalTlsConnections = 0
const canary = createServer(
  {
    key: await readFile(
      join(root, 'apps/ui-test/fixtures/plugin-webviews/loopback.key')
    ),
    cert: await readFile(
      join(root, 'apps/ui-test/fixtures/plugin-webviews/loopback.crt')
    ),
  },
  (_, response) => {
    externalRequests++
    response.end('must not be fetched by a WebView')
  }
)
canary.on('connection', () => externalConnections++)
canary.on('secureConnection', () => externalTlsConnections++)
canary.listen(0, '127.0.0.1')
await once(canary, 'listening')
const canaryAddress = canary.address()
assert.ok(canaryAddress && typeof canaryAddress === 'object')
const canaryUrl = `https://127.0.0.1:${canaryAddress.port}/private`
// This public test certificate is accepted only for this Host positive control.
// WebViews keep certificate validation enabled. HTTP request refusal and OS
// network isolation are separate observations; record speculative connections.
await new Promise<void>((resolve, reject) => {
  const request = get(
    canaryUrl,
    { rejectUnauthorized: false, agent: false, timeout: 1000 },
    response => {
      response.resume()
      response.once('end', () =>
        response.statusCode === 200
          ? resolve()
          : reject(new Error('Canary control failed'))
      )
    }
  )
  request.once('error', reject)
  request.once('timeout', () =>
    request.destroy(new Error('Canary control timeout'))
  )
})
assert.equal(
  externalRequests,
  1,
  'The independent TLS canary must be reachable'
)
assert.equal(externalConnections, 1)
assert.equal(externalTlsConnections, 1)
externalRequests = 0
externalConnections = 0
externalTlsConnections = 0
const desktop = spawn(executable, [`--validation-instance=${instance}`], {
  windowsHide: true,
  stdio: ['ignore', 'pipe', 'pipe'],
  env: nativeEnvironment,
})
let nativeOutput = ''
desktop.stdout.on('data', value => (nativeOutput += String(value)))
desktop.stderr.on('data', value => (nativeOutput += String(value)))
const desktopExited = once(desktop, 'exit')
const observedNativePids = new Set<number>()
const observedNativeProcesses = new Map<number, NativeProcess>()
if (desktop.pid) observedNativePids.add(desktop.pid)
const browsers: Browser[] = []
let evidencePayload: Record<string, unknown> = { identity }
let allObservedExited = false
const ownedProfiles = new Set(
  ['host', 'a', 'b', 'a-partition'].map(name =>
    win32.normalize(join(scratch, name, 'EBWebView')).toLowerCase()
  )
)
function readNativeProcesses(): (NativeProcess & {
  Name: string
  ExecutablePath: string | null
})[] {
  const processes = JSON.parse(
    execFileSync(
      'pwsh',
      [
        '-NoProfile',
        '-Command',
        '@(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,ExecutablePath,CommandLine,CreationDate) | ConvertTo-Json -Compress',
      ],
      { windowsHide: true, encoding: 'utf8', maxBuffer: 4194304, timeout: 5000 }
    )
  ) as (NativeProcess & { Name: string; ExecutablePath: string | null })[]
  return processes
}
function refreshOwnedNativePids(): NativeProcess[] {
  const processes = readNativeProcesses()
  const owned = new Map<number, NativeProcess>()
  const rootProcess = processes.find(
    process => process.ProcessId === desktop.pid
  )
  if (
    rootProcess &&
    isValidationProcessRoot(rootProcess, executable, instance)
  ) {
    for (const process of getOwnedNativeDescendants(
      processes,
      rootProcess.ProcessId
    ))
      owned.set(process.ProcessId, process)
  }
  // Preserve birth identity when a known parent survives the original root.
  for (const previous of observedNativeProcesses.values()) {
    if (
      processes.some(
        process =>
          process.ProcessId === previous.ProcessId &&
          process.CreationDate === previous.CreationDate
      )
    ) {
      for (const process of getOwnedNativeDescendants(
        processes,
        previous.ProcessId
      ))
        owned.set(process.ProcessId, process)
    }
  }
  for (const process of processes) {
    const profile = process.CommandLine?.match(
      /--user-data-dir(?:=|\s+)(?:"([^"]+)"|([^\s]+))/
    )
    const ownsProfile =
      process.Name.toLowerCase() === 'msedgewebview2.exe' &&
      profile &&
      ownedProfiles.has(
        win32
          .normalize((profile[1] ?? profile[2]!).replace(/^\\\\\?\\/, ''))
          .toLowerCase()
      )
    if (ownsProfile) owned.set(process.ProcessId, process)
  }
  for (const process of owned.values()) {
    observedNativePids.add(process.ProcessId)
    observedNativeProcesses.set(process.ProcessId, process)
  }
  return [...owned.values()]
}
type Snapshot = {
  accepted: number
  rejected: number
  invokeDenied: number
  navigationDenied: number
  popupDenied: number
  downloadDenied: number
  profilesConfigured: number
  sessionsRevoked: number
  documentsServed: number
  externalRequestsDenied: number
  crossOriginResourcesDenied: number
}
type Session = {
  source: string
  session: string
  nonce: string
  generation: number
  sequence: number
}
const evaluate = <T>(page: Page, source: string) => page.evaluate<T>(source)
const post = (page: Page, value: unknown) =>
  evaluate<number>(
    page,
    `fetch('/probe',{method:'POST',headers:{'Content-Type':'application/json'},body:${JSON.stringify(JSON.stringify(value))}}).then(r=>r.status)`
  )
try {
  const deadline = Date.now() + 25000
  for (const [index, port] of ports.entries()) {
    const endpoint = `http://127.0.0.1:${port}`
    let ready = false
    while (Date.now() < deadline && desktop.exitCode === null) {
      try {
        if (
          await probeLoopbackCdp(
            port,
            Math.max(1, Math.min(250, deadline - Date.now()))
          )
        ) {
          ready = true
          break
        }
      } catch {
        await new Promise(resolve => setTimeout(resolve, 100))
      }
    }
    assert.ok(ready, `Native WebView ${port} startup: ${nativeOutput}`)
    if (interruptBeforeSnapshot)
      throw new Error('VALIDATION_EXPECTED_NATIVE_INTERRUPT')
    const listeners = JSON.parse(
      execFileSync(
        'pwsh',
        [
          '-NoProfile',
          '-Command',
          `@(Get-NetTCPConnection -State Listen -LocalPort ${port} | Select-Object LocalAddress,OwningProcess) | ConvertTo-Json -Compress -AsArray`,
        ],
        { windowsHide: true, encoding: 'utf8' }
      )
    ) as NativeListener[]
    const processes = readNativeProcesses()
    assert.ok(desktop.pid)
    const rootProcess = processes.find(
      process => process.ProcessId === desktop.pid
    )
    assert.ok(
      rootProcess && isValidationProcessRoot(rootProcess, executable, instance),
      'CDP root must bind the exact disposable validation instance'
    )
    refreshOwnedNativePids()
    assertOwnedWebviewCdp(
      processes,
      listeners,
      desktop.pid,
      join(scratch, ['host', 'a', 'b', 'a-partition'][index]!, 'EBWebView')
    )
    const browser = await chromium.connectOverCDP(endpoint)
    // Playwright normally installs a CDP download denial before WebView2 can
    // emit DownloadStarting. Restore native behavior for this owned environment;
    // the native profile already points its downloads at a disposable directory.
    const cdp = await browser.newBrowserCDPSession()
    const browserContexts = await cdp.send('Target.getBrowserContexts')
    assert.deepEqual(
      browserContexts.browserContextIds,
      [],
      'Each dedicated WebView environment must have only its native default context'
    )
    assert.equal(browser.contexts().length, 1)
    await cdp.send('Browser.setDownloadBehavior', {
      behavior: 'default',
      eventsEnabled: true,
    })
    await cdp.detach()
    browsers.push(browser)
  }
  if (interruptAfterReady) {
    assert.ok(desktop.pid)
    desktop.kill()
    await desktopExited
    throw new Error('VALIDATION_EXPECTED_NATIVE_INTERRUPT')
  }
  const pages = browsers.map(browser => {
    const pages = browser.contexts().flatMap(context => context.pages())
    assert.equal(pages.length, 1)
    return pages[0]!
  })
  const [host, a, b, partition] = pages as [Page, Page, Page, Page]
  await Promise.all(
    pages.map(page =>
      page.waitForFunction(
        () => (globalThis as unknown as { g5Ready: boolean }).g5Ready === true,
        undefined,
        { polling: 50, timeout: 3000 }
      )
    )
  )
  assert.deepEqual(
    pages.map(page => page.url()),
    [
      'https://g5host.localhost/index.html',
      'https://g5a.localhost/index.html',
      'https://g5b.localhost/index.html',
      'https://g5a.localhost/index.html',
    ]
  )
  await evaluate(
    a,
    `localStorage.setItem('private','a');document.cookie='private=a;Secure;SameSite=Strict;path=/';(async()=>{const db=await new Promise((resolve,reject)=>{const r=indexedDB.open('private-db',1);r.onupgradeneeded=()=>r.result.createObjectStore('private');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});db.close();const cache=await caches.open('private-cache');await cache.put('/private',new Response('a'));return true})()`
  )
  for (const page of [host, b, partition]) {
    assert.deepEqual(
      await evaluate(
        page,
        `(async()=>({local:localStorage.getItem('private'),cookie:document.cookie,databases:(await indexedDB.databases()).map(x=>x.name),caches:await caches.keys()}))()`
      ),
      { local: null, cookie: '', databases: [], caches: [] }
    )
  }
  assert.equal(await evaluate(a, `localStorage.getItem('private')`), 'a')
  assert.deepEqual(
    await evaluate(
      a,
      `(async()=>({cookie:document.cookie,databases:(await indexedDB.databases()).map(x=>x.name),caches:await caches.keys(),body:await (await (await caches.open('private-cache')).match('/private')).text()}))()`
    ),
    {
      cookie: 'private=a',
      databases: ['private-db'],
      caches: ['private-cache'],
      body: 'a',
    },
    'The source profile must actually contain every storage canary'
  )
  assert.equal(
    await evaluate(partition, 'location.origin'),
    await evaluate(a, 'location.origin')
  )
  assert.deepEqual((await readdir(scratch)).sort(), [
    'a',
    'a-partition',
    'b',
    'download-a',
    'download-a-partition',
    'download-b',
    'download-host',
    'host',
  ])

  const aSession = await evaluate<Session>(a, 'window.__G5_SESSION__')
  const bSession = await evaluate<Session>(b, 'window.__G5_SESSION__')
  const accepted = { ...aSession, operation: 'echo' }
  assert.equal(await post(a, accepted), 200)
  assert.equal(await post(a, accepted), 403, 'Replay must be refused')
  assert.equal(
    await post(b, accepted),
    403,
    'Other native WebView must be refused'
  )
  for (const change of [
    { nonce: 'forged' },
    { session: bSession.session },
    { generation: 0 },
    { source: 'g5-host' },
    { operation: 'file.read' },
    { unknown: 'schema-smuggling' },
  ]) {
    assert.equal(await post(a, { ...accepted, sequence: 1, ...change }), 403)
  }
  assert.equal(
    await post(a, { ...accepted, sequence: 1, nonce: 'x'.repeat(4096) }),
    403
  )
  assert.equal(await post(b, { ...bSession, operation: 'echo' }), 200)
  await evaluate(b, `localStorage.setItem('private','b')`)
  await b.reload()
  await b.waitForFunction(
    () => (globalThis as unknown as { g5Ready: boolean }).g5Ready === true,
    undefined,
    { polling: 50, timeout: 3000 }
  )
  assert.equal(await evaluate(b, `localStorage.getItem('private')`), 'b')
  assert.equal(await evaluate(a, `localStorage.getItem('private')`), 'a')
  assert.equal(
    await post(b, { ...bSession, sequence: 1, operation: 'echo' }),
    403,
    'Native navigation revokes the old session generation'
  )
  for (const command of [
    'get_plugins',
    'managed_runtime',
    'plugin:fs|read_file',
    'validation_snapshot',
    'plugin:event|listen',
  ]) {
    const result = await evaluate<string>(
      a,
      `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)},{source:'g5-host'}).then(()=> 'unexpected-success', e=>String(e))`
    )
    assert.notEqual(result, 'unexpected-success')
  }
  await evaluate(
    a,
    `window.__cspCanary=false;const script=document.createElement('script');script.textContent='window.__cspCanary=true';document.body.append(script)`
  )
  assert.equal(await evaluate(a, 'window.__cspCanary'), false)
  const remoteScriptBlocked = await evaluate(
    a,
    `new Promise(resolve=>{const remote=document.createElement('script');remote.nonce='g5-fixture';remote.src=${JSON.stringify(canaryUrl)};remote.onload=()=>resolve(false);remote.onerror=()=>resolve(true);document.body.append(remote)})`
  )
  assert.equal(remoteScriptBlocked, true)
  assert.equal(
    await evaluate(
      a,
      `fetch(${JSON.stringify(canaryUrl)}).then(()=>false,()=>true)`
    ),
    true
  )
  assert.equal(externalRequests, 0)
  assert.equal(
    externalConnections,
    0,
    'Ordinary CSP must prevent canary connections'
  )
  assert.equal(externalTlsConnections, 0)
  const snapshot = () =>
    evaluate<Snapshot>(
      host,
      `window.__TAURI_INTERNALS__.invoke('validation_snapshot')`
    )
  const nativeFilterProbe = await a.context().newCDPSession(a)
  const networkFailures: unknown[] = []
  const browserConsole: string[] = []
  a.on('console', message => browserConsole.push(message.text()))
  nativeFilterProbe.on('Network.loadingFailed', event =>
    networkFailures.push(event)
  )
  nativeFilterProbe.on('Network.requestWillBeSent', event =>
    networkFailures.push({ url: event.request.url, type: event.type })
  )
  try {
    // The ordinary CSP assertions above stay intact. Temporarily bypass only
    // renderer CSP to exercise the independent native request rejection layer.
    await nativeFilterProbe.send('Page.enable')
    await nativeFilterProbe.send('Page.setBypassCSP', { enabled: true })
    // CSP bypass takes effect at document initialization. Reload only this
    // owned fixture, then restore policy with another reload in finally.
    await a.reload()
    await a.waitForFunction(
      () => (globalThis as unknown as { g5Ready: boolean }).g5Ready === true,
      undefined,
      { polling: 50, timeout: 3000 }
    )
    await nativeFilterProbe.send('Network.enable')
    const beforeCanaryRequest = await snapshot()
    const nativeFetch = await evaluate<{
      status: number
      error?: string
      violations: string[]
    }>(
      a,
      `(async()=>{const violations=[];const listener=e=>violations.push(e.effectiveDirective+':'+e.blockedURI);document.addEventListener('securitypolicyviolation',listener);try{return await fetch(${JSON.stringify(canaryUrl)}).then(r=>({status:r.status,violations}),e=>({status:0,error:String(e),violations}))}finally{document.removeEventListener('securitypolicyviolation',listener)}})()`
    )
    await new Promise(resolve => setTimeout(resolve, 50))
    await writeFile(
      join(output, 'independent-filter-probe.json'),
      JSON.stringify(
        {
          canaryUrl,
          beforeCanaryRequest,
          afterCanaryRequest: await snapshot(),
          nativeFetch,
          networkFailures,
          browserConsole,
          externalRequests,
          externalConnections,
          externalTlsConnections,
        },
        null,
        2
      )
    )
    assert.ok(nativeFetch.status === 403 || nativeFetch.status === 0)
    assert.equal(externalRequests, 0)
    assert.ok(
      (await snapshot()).externalRequestsDenied >
        beforeCanaryRequest.externalRequestsDenied,
      'The independent live canary request must reach the native refusal layer'
    )
    const beforeForeignResource = await snapshot()
    const foreignResource = await evaluate<number>(
      a,
      `fetch('https://g5b.localhost/index.html').then(r=>r.status,()=>0)`
    )
    assert.ok(foreignResource === 403 || foreignResource === 0)
    const afterForeignResource = await snapshot()
    assert.ok(
      afterForeignResource.externalRequestsDenied >
        beforeForeignResource.externalRequestsDenied,
      'The native all-source filter must observe the foreign-origin request'
    )
    assert.equal(
      afterForeignResource.documentsServed,
      beforeForeignResource.documentsServed,
      'The foreign-origin document must not reach resource delivery'
    )
  } finally {
    await nativeFilterProbe.send('Page.setBypassCSP', { enabled: false })
    await a.reload()
    await a.waitForFunction(
      () => (globalThis as unknown as { g5Ready: boolean }).g5Ready === true,
      undefined,
      { polling: 50, timeout: 3000 }
    )
    await nativeFilterProbe.detach()
  }
  await evaluate(a, `window.open('https://example.invalid/escape')`)
  await evaluate(a, `location.href=${JSON.stringify(canaryUrl)}`)
  await a.waitForTimeout(250)
  assert.equal(a.url(), 'https://g5a.localhost/index.html')
  await evaluate(
    a,
    `const link=document.createElement('a');link.id='native-download';link.href='/download';link.download='denied.txt';link.textContent='验证下载拒绝';document.body.append(link)`
  )
  await a.locator('#native-download').click()
  const documentsBeforeFrames = (await snapshot()).documentsServed
  await evaluate(
    a,
    `window.__frameViolations=[];document.addEventListener('securitypolicyviolation', e=>{if(e.effectiveDirective==='frame-src')window.__frameViolations.push(e.blockedURI)});for(const src of ['https://g5a.localhost/index.html','https://g5b.localhost/index.html']){const frame=document.createElement('iframe');frame.src=src;document.body.append(frame)}`
  )
  await a.waitForFunction(
    () =>
      (globalThis as unknown as { __frameViolations: string[] })
        .__frameViolations.length === 2,
    undefined,
    { polling: 50, timeout: 3000 }
  )
  assert.equal(
    (await snapshot()).documentsServed,
    documentsBeforeFrames,
    'CSP must refuse both same-origin and cross-origin embedded frames before native resource delivery'
  )
  await evaluate(
    a,
    `document.querySelectorAll('iframe').forEach(frame=>frame.remove())`
  )
  const downloadDeadline = Date.now() + 3000
  let evidence = await snapshot()
  while (evidence.downloadDenied === 0 && Date.now() < downloadDeadline) {
    await new Promise(resolve => setTimeout(resolve, 50))
    evidence = await snapshot()
  }
  await writeFile(
    join(output, 'observed-counters.json'),
    JSON.stringify(evidence)
  )
  assert.equal(evidence.accepted, 2)
  assert.equal(evidence.rejected, 10)
  assert.ok(
    evidence.invokeDenied >= 3,
    'Actual native handler must observe denials'
  )
  assert.ok(evidence.navigationDenied >= 1)
  assert.ok(evidence.popupDenied >= 1)
  assert.ok(evidence.downloadDenied >= 1)
  assert.equal(evidence.profilesConfigured, 4)
  assert.ok(evidence.sessionsRevoked >= 1)
  assert.ok(
    evidence.externalRequestsDenied >= 1,
    'Actual native all-source request handler must refuse the independent network probe'
  )
  assert.equal(externalRequests, 0)
  for (const name of ['host', 'a', 'b', 'a-partition']) {
    assert.deepEqual(
      await readdir(join(scratch, `download-${name}`)),
      [],
      'Native denial must precede download file creation'
    )
  }
  assert.equal(
    externalTlsConnections,
    0,
    'The owned canary must receive no completed TLS handshake'
  )
  assert.equal(
    browsers.flatMap(browser =>
      browser.contexts().flatMap(context => context.pages())
    ).length,
    4
  )
  await Promise.all(
    pages.map((page, index) =>
      page.screenshot({ path: join(output, `window-${index}.png`) })
    )
  )
  await writeFile(
    join(output, 'evidence.json'),
    JSON.stringify(
      (evidencePayload = {
        status: 'checks-passed-cleanup-pending',
        date: new Date().toISOString(),
        identity,
        scope:
          'prototype Windows/x64 WebView2 feasibility; no installed plugin or production integration',
        origins: pages.map(page => page.url()),
        storage: ['localStorage', 'cookies', 'IndexedDB', 'Cache API'],
        evidence,
        externalRequests,
        externalConnections,
        externalTlsConnections,
        limitations: [
          'Native HTTP request refusal does not establish OS network isolation; CSP-bypass probes may create speculative TCP connections.',
        ],
      }),
      null,
      2
    )
  )
} catch (error) {
  if (
    !expectInterruption ||
    !(error instanceof Error) ||
    error.message !== 'VALIDATION_EXPECTED_NATIVE_INTERRUPT'
  ) {
    await writeFile(
      join(output, 'evidence.json'),
      JSON.stringify({
        status: 'failed',
        code: 'VALIDATION_ASSERTION_FAILED',
        identity,
      })
    )
    throw error
  }
  await writeFile(
    join(output, 'evidence.json'),
    JSON.stringify(
      (evidencePayload = { status: 'native-interrupt-observed', identity })
    )
  )
} finally {
  await cleanupOwnedValidation()
}
async function cleanupOwnedValidation() {
  const cleanupErrors: Error[] = []
  const attempt = async (action: () => unknown) => {
    try {
      await action()
    } catch (error) {
      cleanupErrors.push(
        error instanceof Error ? error : new Error(String(error))
      )
    }
  }
  const bounded = async (work: Promise<unknown>, message: string) => {
    let timer: ReturnType<typeof setTimeout> | undefined
    try {
      await Promise.race([
        work,
        new Promise((_, reject) => {
          timer = setTimeout(() => reject(new Error(message)), 5000)
        }),
      ])
    } finally {
      clearTimeout(timer)
    }
  }
  // Each stage runs even if a prior scan, disconnect or wait failed. Such
  // failures still prevent a passing evidence record.
  await attempt(() => refreshOwnedNativePids())
  await attempt(() =>
    bounded(
      Promise.all(browsers.map(browser => browser.close())),
      'Browser disconnect did not complete'
    )
  )
  if (desktop.exitCode === null && desktop.signalCode === null && desktop.pid) {
    await attempt(() => desktop.kill())
    await attempt(() =>
      bounded(desktopExited, 'Native validation child did not exit')
    )
  }
  let ownedSurvivors: NativeProcess[] = []
  await attempt(() => {
    // The parent may have exited before descendants. Only exact disposable UDF
    // owners qualify for final orphan termination; never kill a browser by name.
    ownedSurvivors = refreshOwnedNativePids()
  })
  for (const record of ownedSurvivors)
    await attempt(() => terminateNativeProcess(record))
  await attempt(async () => {
    const cleanupDeadline = Date.now() + 10000
    let alive: number[] = []
    do {
      const current = readNativeProcesses()
      alive = [...observedNativeProcesses.values()]
        .filter(previous =>
          current.some(
            process =>
              process.ProcessId === previous.ProcessId &&
              process.CreationDate === previous.CreationDate
          )
        )
        .map(process => process.ProcessId)
      if (alive.length === 0) break
      await new Promise(resolve => setTimeout(resolve, 100))
    } while (Date.now() < cleanupDeadline)
    assert.deepEqual(
      alive,
      [],
      'All observed native browser child processes must exit'
    )
    allObservedExited = cleanupErrors.length === 0
  })
  await attempt(() => {
    canary.closeAllConnections()
    return bounded(
      new Promise<void>((resolve, reject) =>
        canary.close(error => (error ? reject(error) : resolve()))
      ),
      'Canary server did not close'
    )
  })
  await attempt(() =>
    writeFile(join(output, 'native-output.log'), nativeOutput)
  )
  await attempt(() =>
    writeFile(
      join(output, 'retained-fixture.json'),
      JSON.stringify(
        {
          root: scratch,
          observedNativePids: [...observedNativePids],
          observedNativeProcesses: [...observedNativeProcesses.values()].map(
            ({ ProcessId, CreationDate }) => ({ ProcessId, CreationDate })
          ),
          allObservedExited,
          cleanupErrors: cleanupErrors.map(error => error.message),
          externalRequests,
          externalConnections,
          externalTlsConnections,
          purpose:
            'Disposable native UDFs retained for inspection; never a user profile',
        },
        null,
        2
      )
    )
  )
  if (cleanupErrors.length > 0) {
    await writeFile(
      join(output, 'evidence.json'),
      JSON.stringify({
        status: 'failed',
        code: 'VALIDATION_CLEANUP_FAILED',
        identity,
      })
    )
    throw new AggregateError(cleanupErrors, 'Native validation cleanup failed')
  }
}
await writeFile(
  join(output, 'evidence.json'),
  JSON.stringify(
    {
      ...evidencePayload,
      status: expectInterruption ? 'interruption-cleanup-passed' : 'passed',
    },
    null,
    2
  )
)
process.stdout.write(
  (expectInterruption
    ? 'G5 native interruption cleanup passed: all observed child processes exited and logs preserved.'
    : 'G5 native WebView feasibility passed: four isolated profiles, origins, storage, IPC/session and browser-effect rejection; all observed child processes exited.') +
    '\n'
)

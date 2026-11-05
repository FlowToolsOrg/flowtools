import { afterEach, expect, it } from 'vitest'

import config from '../../../../desktop/src-tauri/tauri.conf.json'

const frames: HTMLIFrameElement[] = []
afterEach(() => frames.splice(0).forEach(frame => frame.remove()))

async function fixture(moduleSource = '') {
  // Tauri appends per-document hashes/nonces to bundled scripts. Only this
  // synthetic harness script gets a nonce; injected scripts below do not.
  const nonce = 'flowtoolsCspRegressionFixture'
  const policy = Object.entries(config.app.security.csp)
    // frame-ancestors is HTTP-header-only; the native policy retains it.
    .filter(([name]) => name !== 'frame-ancestors')
    .map(
      ([name, value]) =>
        `${name} ${value}${name === 'script-src' ? ` 'nonce-${nonce}'` : ''}`
    )
    .join('; ')
  const frame = document.createElement('iframe')
  frames.push(frame)
  const loaded = new Promise<void>(resolve => {
    frame.onload = () => resolve()
  })
  frame.srcdoc = `<html><head><meta http-equiv="Content-Security-Policy" content="${policy}">
    <script nonce="${nonce}">
      document.documentElement.dataset.trusted = 'yes';
      document.documentElement.dataset.violations = '[]';
      document.addEventListener('securitypolicyviolation', event => {
        const root = document.documentElement;
        root.dataset.violations = JSON.stringify([...JSON.parse(root.dataset.violations), event.effectiveDirective]);
      });
    </script></head><body><div id="root">local content</div>
    <script type="module" nonce="${nonce}">${moduleSource}</script></body></html>`
  document.body.append(frame)
  await loaded
  const doc = frame.contentDocument!
  expect(doc.documentElement.dataset.trusted).toBe('yes')
  return { frame, doc }
}

it('production CSP actually blocks injected scripts, eval, nested frames, objects and base replacement', async () => {
  const { frame, doc } = await fixture()
  const inner = frame.contentWindow as typeof window
  const inline = doc.createElement('script')
  inline.textContent = "document.documentElement.dataset.injected = 'yes'"
  doc.body.append(inline)
  const dataScript = doc.createElement('script')
  dataScript.src =
    'data:text/javascript,document.documentElement.dataset.injected=%22yes%22'
  doc.body.append(dataScript)
  expect(() => inner.eval('1 + 1')).toThrow()
  const nested = doc.createElement('iframe')
  nested.src = 'data:text/html,blocked'
  doc.body.append(nested)
  const object = doc.createElement('object')
  object.data = 'data:text/html,blocked'
  doc.body.append(object)
  const base = doc.createElement('base')
  base.href = 'https://csp-base.invalid/'
  doc.head.append(base)
  const form = doc.createElement('form')
  form.action = 'https://csp-form.invalid/'
  doc.body.append(form)
  form.submit()
  await inner.fetch('data:text/plain,blocked').catch(() => undefined)
  const workerUrl = URL.createObjectURL(
    new Blob(['postMessage("unexpected-worker")'], { type: 'text/javascript' })
  )
  const worker = new inner.Worker(workerUrl)
  worker.onerror = event => event.preventDefault()
  worker.onmessage = () => {
    doc.documentElement.dataset.worker = 'ran'
  }
  try {
    await expect
      .poll(
        () => JSON.parse(doc.documentElement.dataset.violations!) as string[]
      )
      .toEqual(
        expect.arrayContaining([
          'script-src-elem',
          'script-src',
          'frame-src',
          'object-src',
          'base-uri',
          'form-action',
          'connect-src',
          'worker-src',
        ])
      )
  } finally {
    worker.terminate()
    URL.revokeObjectURL(workerUrl)
  }
  expect(doc.documentElement.dataset.injected).toBeUndefined()
  expect(doc.documentElement.dataset.worker).toBeUndefined()
  expect(doc.baseURI).not.toBe('https://csp-base.invalid/')
  const local = doc.getElementById('root')!
  local.style.color = 'rgb(1, 2, 3)'
  expect(frame.contentWindow!.getComputedStyle(local).color).toBe(
    'rgb(1, 2, 3)'
  )
})

it('the actual built Runtime client runs under production CSP without eval permission', async () => {
  const entry = new URL(
    '../../../../../packages/runtime-client/dist/index.js',
    import.meta.url
  ).href
  const policy = new URL(
    '../../../../desktop/src/csp-schema-policy.ts',
    import.meta.url
  ).href
  const { doc } = await fixture(`
    try {
      await import(${JSON.stringify(policy)});
      const {RuntimeClient, encodeRequest} = await import(${JSON.stringify(entry)});
      const request = {version:1,requestId:'csp',session:null,call:{method:'runtime.status'}};
      if (JSON.parse(encodeRequest(request)).requestId !== 'csp') throw new Error('encode');
      try { encodeRequest({...request,version:65536}); throw new Error('missing refusal'); }
      catch (error) { if (error.message !== 'INVALID_REQUEST') throw error; }
      const client = new RuntimeClient({close(){}, async exchange(request) {
        return {version:1,requestId:request.requestId,outcome:{type:'error',data:{code:'INVALID_REQUEST'}}};
      }});
      try { await client.call({method:'runtime.status'}); throw new Error('missing response refusal'); }
      catch (error) { if (error.code !== 'INVALID_REQUEST') throw error; }
      document.documentElement.dataset.client = 'passed';
    } catch (error) { document.documentElement.dataset.client = error.message; }
  `)
  await expect.poll(() => doc.documentElement.dataset.client).toBe('passed')
  expect(
    JSON.parse(doc.documentElement.dataset.violations!) as string[]
  ).not.toContain('script-src')
})

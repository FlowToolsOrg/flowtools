import { useState } from 'react'

import base64 from '@flowtools/plugins/plugin-base64-encoder'
import { executePlugin } from '@flowtools/sdk/execution'
import { ExecutionPanel } from '@flowtools/ui'
import { expect, it, vi } from 'vitest'
import { render } from 'vitest-browser-react'
import { userEvent } from 'vitest/browser'

import {
  LauncherInputPanel,
  LauncherInputProvider,
  useLauncherInputPreset,
  admitLauncherFile,
  prepareLauncherInput,
} from '../../../../desktop/src/runtime/launcher-input'

it('clipboard acquisition is explicit and transferring context only prefills a preview', async () => {
  const read = vi
    .spyOn(navigator.clipboard, 'readText')
    .mockResolvedValue('private clipboard value')
  const execute = vi.fn((input: unknown, signal: AbortSignal) =>
    executePlugin(
      base64,
      input,
      {
        env: {
          pluginId: base64.meta.id,
          pluginType: 'app',
          platform: 'web',
          mode: 'test',
        },
        ui: { toast() {}, openPanel() {}, closePanel() {} },
        utils: { now: Date.now },
        signal,
        log() {},
      },
      { signal }
    )
  )
  function Preview() {
    const preset = useLauncherInputPreset(base64.meta.id)
    return (
      <ExecutionPanel
        meta={base64.meta}
        initialInput={preset?.input}
        execute={execute}
      />
    )
  }
  function Harness() {
    const [open, setOpen] = useState(false)
    return (
      <LauncherInputProvider>
        <LauncherInputPanel
          pluginId={base64.meta.id}
          onOpen={() => setOpen(true)}
        />
        {open && <Preview />}
      </LauncherInputProvider>
    )
  }
  const view = await render(<Harness />)
  expect(read).not.toHaveBeenCalled()
  const user = userEvent.setup()
  await user.click(view.getByText('带入文本、剪贴板或文件', { exact: true }))
  await user.click(view.getByRole('button', { name: '读取剪贴板文本' }))
  await expect
    .element(view.getByRole('textbox', { name: '触发上下文文本' }))
    .toHaveValue('private clipboard value')
  expect(read).toHaveBeenCalledTimes(1)
  await user.click(view.getByRole('button', { name: '带入所选命令，先预览' }))
  await expect
    .element(view.getByRole('textbox', { name: 'JSON input' }))
    .toHaveValue(JSON.stringify({ text: 'private clipboard value' }, null, 2))
  expect(execute).not.toHaveBeenCalled()
  await user.click(view.getByRole('button', { name: 'Run JSON' }))
  await expect.poll(() => execute.mock.calls.length).toBe(1)
  await expect
    .element(view.getByTestId('execution-output'))
    .toHaveTextContent(btoa('private clipboard value'))
  read.mockRestore()
})
it('user-selected files/images are bounded, inert and matched only to known inputs', async () => {
  const text = await admitLauncherFile(
    new File(['hello'], 'sample.txt', { type: 'text/plain' })
  )
  expect(prepareLauncherInput('plugin-hash-generator', text)).toEqual({
    text: 'hello',
  })
  expect(prepareLauncherInput('external-html', text)).toBeUndefined()
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = 1
  canvas.getContext('2d')!.fillRect(0, 0, 1, 1)
  const dataUrl = canvas.toDataURL('image/png')
  const png = new File(
    [
      Uint8Array.from(atob(dataUrl.split(',')[1]!), value =>
        value.charCodeAt(0)
      ),
    ],
    'image.png',
    { type: 'image/png' }
  )
  const image = await admitLauncherFile(png)
  expect(image.kind).toBe('image')
  expect(prepareLauncherInput('plugin-image-base64', image)).toEqual({
    base64: dataUrl,
  })
  expect(prepareLauncherInput('plugin-json-formatter', image)).toBeUndefined()
  await expect(
    admitLauncherFile(new File(['<svg/>'], 'x.svg', { type: 'image/svg+xml' }))
  ).rejects.toThrow()
  await expect(
    admitLauncherFile(new File(['bad'], 'x.png', { type: 'image/png' }))
  ).rejects.toThrow()
  await expect(
    admitLauncherFile(
      new File([new Uint8Array(262145)], 'large.png', { type: 'image/png' })
    )
  ).rejects.toThrow()
  expect(
    prepareLauncherInput('plugin-base64-encoder', {
      kind: 'text',
      text: '密'.repeat(30000),
    })
  ).toBeUndefined()
})

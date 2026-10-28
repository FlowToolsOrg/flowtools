import {
  createContext,
  useContext,
  useRef,
  useState,
  useEffect,
  type ReactNode,
} from 'react'

import { Button, TextArea } from '@heroui/react'

export type LauncherInput =
  | {
      kind: 'text' | 'clipboard' | 'selection' | 'file'
      text: string
      name?: string
    }
  | { kind: 'image'; dataUrl: string; name: string }
type Preset = { key: string; pluginId: string; input: Record<string, string> }
const Context = createContext<
  | { preset?: Preset; setPreset: (preset: Preset | undefined) => void }
  | undefined
>(undefined)
export function LauncherInputProvider({ children }: { children: ReactNode }) {
  const [preset, setPreset] = useState<Preset>()
  return (
    <Context.Provider value={{ preset, setPreset }}>
      {children}
    </Context.Provider>
  )
}
export function useLauncherInputPreset(pluginId: string) {
  const context = useContext(Context)
  const [preset] = useState(() =>
    context?.preset?.pluginId === pluginId ? context.preset : undefined
  )
  useEffect(() => {
    if (preset && context?.preset?.key === preset.key)
      context.setPreset(undefined)
  }, [context, preset])
  return preset
}
export function prepareLauncherInput(
  pluginId: string,
  value: LauncherInput
): Record<string, string> | undefined {
  if (value.kind === 'image')
    return pluginId === 'plugin-image-base64'
      ? { base64: value.dataUrl }
      : undefined
  if (
    !value.text.length ||
    new TextEncoder().encode(value.text).length > 65_536
  )
    return undefined
  return [
    'plugin-base64-encoder',
    'plugin-hash-generator',
    'plugin-json-formatter',
    'plugin-regex-tester',
  ].includes(pluginId)
    ? { text: value.text }
    : undefined
}
export async function admitLauncherFile(file: File): Promise<LauncherInput> {
  if (file.size > 262_144 || file.name.length > 256)
    throw new Error('CONTEXT_LIMIT')
  const bytes = new Uint8Array(await file.arrayBuffer())
  if (bytes.length !== file.size || bytes.length > 262_144)
    throw new Error('CONTEXT_LIMIT')
  const image =
    (file.type === 'image/png' &&
      [137, 80, 78, 71, 13, 10, 26, 10].every((v, i) => bytes[i] === v)) ||
    (file.type === 'image/jpeg' &&
      bytes[0] === 255 &&
      bytes[1] === 216 &&
      bytes[2] === 255) ||
    (file.type === 'image/gif' &&
      String.fromCharCode(...bytes.subarray(0, 6)).match(/^GIF8[79]a$/)) ||
    (file.type === 'image/webp' &&
      String.fromCharCode(...bytes.subarray(0, 4)) === 'RIFF' &&
      String.fromCharCode(...bytes.subarray(8, 12)) === 'WEBP')
  if (image) {
    let binary = ''
    for (let start = 0; start < bytes.length; start += 8192)
      binary += String.fromCharCode(...bytes.subarray(start, start + 8192))
    return {
      kind: 'image',
      name: file.name,
      dataUrl: `data:${file.type};base64,${btoa(binary)}`,
    }
  }
  if (
    file.type.startsWith('image/') ||
    bytes.length > 65_536 ||
    !(
      file.type.startsWith('text/') ||
      file.type === 'application/json' ||
      /\.(txt|json|csv|md)$/i.test(file.name)
    )
  )
    throw new Error('CONTEXT_UNSUPPORTED')
  return {
    kind: 'file',
    name: file.name,
    text: new TextDecoder('utf-8', { fatal: true }).decode(bytes),
  }
}

/** Only explicit user gestures acquire data. No OS paths, persistence or execution. */
export function LauncherInputPanel({
  pluginId,
  onOpen,
}: {
  pluginId?: string
  onOpen: () => void
}) {
  const context = useContext(Context)
  const [value, setValue] = useState<LauncherInput>({ kind: 'text', text: '' })
  const [message, setMessage] = useState(
    '只在本次打开中使用，不保存到搜索历史。'
  )
  const [busy, setBusy] = useState(false)
  const pending = useRef(false)
  const selected = useRef('')
  const generation = useRef(0)
  useEffect(
    () => () => {
      generation.current++
    },
    []
  )
  const acquire = async (read: () => Promise<LauncherInput>) => {
    if (pending.current) return
    pending.current = true
    setBusy(true)
    setValue({ kind: 'text', text: '' })
    context?.setPreset(undefined)
    const token = ++generation.current
    try {
      const next = await read()
      if (
        next.kind !== 'image' &&
        new TextEncoder().encode(next.text).length > 65_536
      )
        throw new Error('CONTEXT_LIMIT')
      if (generation.current === token) {
        setValue(next)
        setMessage('请核对内容与目标命令，再带入输入预览。')
      }
    } catch {
      if (generation.current === token)
        setMessage(
          '无法读取或内容超出限制。文本最多 64 KiB，PNG/JPEG/GIF/WebP 图片最多 256 KiB；请重新选择。'
        )
    } finally {
      pending.current = false
      if (generation.current === token) setBusy(false)
    }
  }
  const prepared = pluginId ? prepareLauncherInput(pluginId, value) : undefined
  return (
    <details className="mb-3 rounded-lg border border-(--divider-color) p-2 text-sm">
      <summary>带入文本、剪贴板或文件</summary>
      <div className="grid gap-2 py-2">
        <TextArea
          aria-label="触发上下文文本"
          value={value.kind === 'image' ? '' : value.text}
          disabled={busy}
          onChange={event => {
            generation.current++
            setValue({ kind: 'text', text: event.target.value.slice(0, 65536) })
          }}
        />
        <div className="flex flex-wrap gap-2">
          <Button
            size="sm"
            variant="secondary"
            isDisabled={busy}
            onPress={() =>
              void acquire(async () => ({
                kind: 'clipboard',
                text: await navigator.clipboard.readText(),
              }))
            }
          >
            读取剪贴板文本
          </Button>
          <Button
            size="sm"
            variant="secondary"
            isDisabled={busy}
            onPointerDown={() => {
              selected.current = window.getSelection()?.toString() ?? ''
            }}
            onPress={() =>
              void acquire(async () => ({
                kind: 'selection',
                text:
                  selected.current || window.getSelection()?.toString() || '',
              }))
            }
          >
            读取本窗口选中文本
          </Button>
        </div>
        <label>
          选择文本文件或图片
          <input
            aria-label="选择上下文文件"
            type="file"
            disabled={busy}
            accept=".txt,.json,.csv,.md,image/png,image/jpeg,image/gif,image/webp"
            onChange={event => {
              const file = event.target.files?.[0]
              event.target.value = ''
              if (file) void acquire(() => admitLauncherFile(file))
            }}
          />
        </label>
        {value.kind === 'image' && (
          <p>
            已选择图片：{value.name}。仅带入图片 Base64
            命令，不读取目录或赋予文件写权限。
          </p>
        )}
        {value.kind === 'file' && <p>已选择文件：{value.name}</p>}
        <p role="status">{message} 文本最多 64 KiB，图片最多 256 KiB。</p>
        <div className="flex gap-2">
          <Button
            size="sm"
            isDisabled={busy || !prepared || !pluginId || !context}
            onPress={() => {
              if (!prepared || !pluginId || !context) return
              context.setPreset({
                key: crypto.randomUUID(),
                pluginId,
                input: prepared,
              })
              onOpen()
            }}
          >
            带入所选命令，先预览
          </Button>
          <Button
            size="sm"
            variant="secondary"
            isDisabled={busy}
            onPress={() => {
              generation.current++
              context?.setPreset(undefined)
              setValue({ kind: 'text', text: '' })
              setMessage('已清空本次上下文。')
            }}
          >
            清空上下文
          </Button>
        </div>
        {!prepared && (
          <p>
            请在搜索结果中选择支持此内容的内置命令。其他命令不会收到这些内容。
          </p>
        )}
      </div>
    </details>
  )
}

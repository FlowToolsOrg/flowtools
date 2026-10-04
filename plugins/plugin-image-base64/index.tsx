import { useCallback, useRef, useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import { Button, Card, Label, TextArea } from '@flowtools/ui/plugin'

import commandPlugin from './commands'

export default definePlugin({
  ...commandPlugin,
  setup() {
    return function ImageBase64Panel() {
      const { clipboard } = useCapability()
      const fileInputRef = useRef<HTMLInputElement>(null)
      const [mode, setMode] = useState<'imageToBase64' | 'base64ToImage'>(
        'imageToBase64'
      )
      const [base64Output, setBase64Output] = useState('')
      const [base64Input, setBase64Input] = useState('')
      const [imagePreview, setImagePreview] = useState('')
      const [fileName, setFileName] = useState('')
      const [fileSize, setFileSize] = useState('')
      const [copied, setCopied] = useState(false)
      const [error, setError] = useState('')
      const formatSize = (bytes: number): string => {
        if (bytes < 1024) return `${bytes} B`
        if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
        return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
      }
      const handleFileChange = useCallback(
        (e: React.ChangeEvent<HTMLInputElement>) => {
          const file = e.target.files?.[0]
          if (!file) return
          if (!file.type.startsWith('image/')) {
            setError('请选择图片文件')
            return
          }
          setError('')
          setFileName(file.name)
          setFileSize(formatSize(file.size))
          const reader = new FileReader()
          reader.onload = () => {
            const data = reader.result as string
            setBase64Output(data)
            setImagePreview(data)
          }
          reader.readAsDataURL(file)
        },
        []
      )
      const handleBase64Convert = useCallback(() => {
        if (!base64Input.trim()) {
          setError('请输入 Base64 编码')
          return
        }
        setError('')
        let src = base64Input.trim()
        if (!src.startsWith('data:')) {
          src = `data:image/png;base64,${src}`
        }
        setImagePreview(src)
        setFileName('从 Base64 还原')
        setFileSize('')
      }, [base64Input])
      const copyBase64 = useCallback(async () => {
        await clipboard.writeText(base64Output)
        setCopied(true)
        setTimeout(() => setCopied(false), 1500)
      }, [clipboard, base64Output])
      const clearAll = useCallback(() => {
        setBase64Output('')
        setBase64Input('')
        setImagePreview('')
        setFileName('')
        setFileSize('')
        setError('')
        if (fileInputRef.current) {
          fileInputRef.current.value = ''
        }
      }, [])
      return (
        <div className="w-full max-w-3xl mx-auto">
          <Card className="p-6">
            <h2 className="text-lg font-semibold mb-4">图片 Base64 互转</h2>

            <div className="flex gap-2 mb-4">
              <Button
                variant={mode === 'imageToBase64' ? 'primary' : 'ghost'}
                onPress={() => {
                  setMode('imageToBase64')
                  clearAll()
                }}
              >
                图片 → Base64
              </Button>
              <Button
                variant={mode === 'base64ToImage' ? 'primary' : 'ghost'}
                onPress={() => {
                  setMode('base64ToImage')
                  clearAll()
                }}
              >
                Base64 → 图片
              </Button>
            </div>

            {error && (
              <div className="mb-4 rounded-lg bg-red-50 border border-red-200 px-4 py-2 text-sm text-red-600">
                {error}
              </div>
            )}

            {mode === 'imageToBase64' ? (
              <div className="space-y-4">
                <div>
                  <input
                    ref={fileInputRef}
                    type="file"
                    accept="image/*"
                    onChange={handleFileChange}
                    className="block w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-lg file:border-0 file:text-sm file:font-semibold file:bg-blue-50 file:text-blue-700 hover:file:bg-blue-100 cursor-pointer"
                  />
                </div>

                {imagePreview && (
                  <div className="space-y-3">
                    <div className="rounded-lg border p-3">
                      <div className="flex items-center justify-between mb-2">
                        <span className="text-sm font-medium">预览</span>
                        {fileName && (
                          <span className="text-xs text-gray-400">
                            {fileName}
                            {fileSize && ` (${fileSize})`}
                          </span>
                        )}
                      </div>
                      <img
                        src={imagePreview}
                        alt="预览"
                        className="max-h-64 mx-auto rounded object-contain"
                      />
                    </div>

                    <div className="relative">
                      <Label>Base64 输出</Label>
                      <TextArea
                        value={base64Output}
                        readOnly
                        rows={4}
                        className="font-mono text-xs"
                      />
                      <Button
                        size="sm"
                        variant="ghost"
                        className="absolute top-1 right-1"
                        onPress={copyBase64}
                      >
                        {copied ? '已复制!' : '复制'}
                      </Button>
                    </div>
                  </div>
                )}
              </div>
            ) : (
              <div className="space-y-4">
                <Label>粘贴 Base64 编码</Label>
                <TextArea
                  placeholder="data:image/png;base64,... 或纯 base64 字符串"
                  value={base64Input}
                  onChange={e => setBase64Input(e.target.value)}
                  rows={4}
                  className="font-mono text-xs"
                />

                <Button variant="primary" onPress={handleBase64Convert}>
                  转换为图片
                </Button>

                {imagePreview && (
                  <div className="rounded-lg border p-3">
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-sm font-medium">还原结果</span>
                      {fileName && (
                        <span className="text-xs text-gray-400">
                          {fileName}
                        </span>
                      )}
                    </div>
                    <img
                      src={imagePreview}
                      alt="还原"
                      className="max-h-64 mx-auto rounded object-contain"
                    />
                  </div>
                )}
              </div>
            )}
          </Card>
        </div>
      )
    }
  },
})

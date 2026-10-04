import { useCallback, useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import { Button, Card, Input, Label, TextField } from '@flowtools/ui/plugin'

import commandPlugin, { type ColorFormats, parseColor } from './commands'

export default definePlugin({
  ...commandPlugin,
  setup() {
    return function ColorConverterPanel() {
      const { clipboard } = useCapability()
      const [input, setInput] = useState('#3b82f6')
      const [color, setColor] = useState<ColorFormats | null>(null)
      const [error, setError] = useState<string | null>(null)
      const [copiedField, setCopiedField] = useState<string | null>(null)
      const convert = useCallback(() => {
        setError(null)
        if (!input.trim()) {
          setError('请输入颜色值')
          setColor(null)
          return
        }
        const result = parseColor(input)
        if (!result) {
          setError('无法解析颜色格式')
          setColor(null)
          return
        }
        setColor(result)
      }, [input])
      const copyValue = useCallback(
        async (value: string, field: string) => {
          await clipboard.writeText(value)
          setCopiedField(field)
          setTimeout(() => setCopiedField(null), 1500)
        },
        [clipboard]
      )
      const presetColors = [
        '#ff0000',
        '#00ff00',
        '#0000ff',
        '#ffff00',
        '#ff00ff',
        '#00ffff',
        '#3b82f6',
        '#10b981',
        '#f59e0b',
        '#ef4444',
        '#8b5cf6',
        '#ec4899',
      ]
      return (
        <div className="w-full max-w-2xl mx-auto">
          <Card className="p-6">
            <h2 className="text-lg font-semibold mb-4">颜色转换器</h2>

            <div className="mb-4">
              <Label>输入颜色值</Label>
              <div className="flex gap-2">
                <TextField
                  className="flex-1"
                  value={input}
                  onChange={v => setInput(v)}
                >
                  <Input placeholder="#3b82f6 或 rgb(59, 130, 246) 或 hsl(217, 91%, 60%)" />
                </TextField>
                <Button variant="primary" onPress={convert}>
                  转换
                </Button>
              </div>
              {error && <div className="mt-2 text-sm text-danger">{error}</div>}
            </div>

            <div className="mb-4">
              <Label>预设颜色</Label>
              <div className="flex flex-wrap gap-2 mt-1">
                {presetColors.map(c => (
                  <button
                    key={c}
                    className="rounded-lg border-2 cursor-pointer hover:scale-110 transition-transform size-8"
                    style={{
                      backgroundColor: c,
                      borderColor:
                        color?.hex === c
                          ? 'var(--color-primary)'
                          : 'transparent',
                    }}
                    onClick={() => {
                      setInput(c)
                      const result = parseColor(c)
                      if (result) setColor(result)
                    }}
                  />
                ))}
              </div>
            </div>

            {color && (
              <>
                <div
                  className="w-full h-24 rounded-lg mb-4 border"
                  style={{ backgroundColor: color.hex }}
                />

                <div className="space-y-3">
                  <div className="flex items-center justify-between p-3 rounded-lg border">
                    <div>
                      <div className="text-xs text-gray-500">HEX</div>
                      <div className="font-mono">{color.hex}</div>
                    </div>
                    <Button
                      size="sm"
                      variant="ghost"
                      onPress={() => copyValue(color.hex, 'hex')}
                    >
                      {copiedField === 'hex' ? '已复制!' : '复制'}
                    </Button>
                  </div>

                  <div className="flex items-center justify-between p-3 rounded-lg border">
                    <div>
                      <div className="text-xs text-gray-500">RGB</div>
                      <div className="font-mono">
                        rgb({color.rgb.r}, {color.rgb.g}, {color.rgb.b})
                      </div>
                    </div>
                    <Button
                      size="sm"
                      variant="ghost"
                      onPress={() =>
                        copyValue(
                          `rgb(${color.rgb.r}, ${color.rgb.g}, ${color.rgb.b})`,
                          'rgb'
                        )
                      }
                    >
                      {copiedField === 'rgb' ? '已复制!' : '复制'}
                    </Button>
                  </div>

                  <div className="flex items-center justify-between p-3 rounded-lg border">
                    <div>
                      <div className="text-xs text-gray-500">HSL</div>
                      <div className="font-mono">
                        hsl({color.hsl.h}, {color.hsl.s}%, {color.hsl.l}%)
                      </div>
                    </div>
                    <Button
                      size="sm"
                      variant="ghost"
                      onPress={() =>
                        copyValue(
                          `hsl(${color.hsl.h}, ${color.hsl.s}%, ${color.hsl.l}%)`,
                          'hsl'
                        )
                      }
                    >
                      {copiedField === 'hsl' ? '已复制!' : '复制'}
                    </Button>
                  </div>

                  <div className="flex items-center justify-between p-3 rounded-lg border">
                    <div>
                      <div className="text-xs text-gray-500">RGB 数值</div>
                      <div className="font-mono text-sm">
                        R: {color.rgb.r} | G: {color.rgb.g} | B: {color.rgb.b}
                      </div>
                    </div>
                    <Button
                      size="sm"
                      variant="ghost"
                      onPress={() =>
                        copyValue(
                          `${color.rgb.r}, ${color.rgb.g}, ${color.rgb.b}`,
                          'rgb-values'
                        )
                      }
                    >
                      {copiedField === 'rgb-values' ? '已复制!' : '复制'}
                    </Button>
                  </div>
                </div>
              </>
            )}
          </Card>
        </div>
      )
    }
  },
})

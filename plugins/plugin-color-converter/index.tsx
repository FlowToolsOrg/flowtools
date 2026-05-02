import { useCallback, useState } from 'react'

import { definePlugin, useCapability } from '@flow-tool/sdk'
import { result } from '@flow-tool/sdk/result'
import { Button, Card, Input, Label, TextField } from '@flow-tool/ui/plugin'
import { z } from 'zod'

interface ColorFormats {
  hex: string
  rgb: { r: number; g: number; b: number }
  hsl: { h: number; s: number; l: number }
}

function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
  return result
    ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16),
      }
    : null
}

function rgbToHex(r: number, g: number, b: number): string {
  return (
    '#' +
    [r, g, b]
      .map(x => {
        const hex = Math.max(0, Math.min(255, Math.round(x))).toString(16)
        return hex.length === 1 ? '0' + hex : hex
      })
      .join('')
  )
}

function rgbToHsl(
  r: number,
  g: number,
  b: number
): { h: number; s: number; l: number } {
  r /= 255
  g /= 255
  b /= 255

  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  let h = 0
  let s = 0
  const l = (max + min) / 2

  if (max !== min) {
    const d = max - min
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min)

    switch (max) {
      case r:
        h = ((g - b) / d + (g < b ? 6 : 0)) / 6
        break
      case g:
        h = ((b - r) / d + 2) / 6
        break
      case b:
        h = ((r - g) / d + 4) / 6
        break
    }
  }

  return {
    h: Math.round(h * 360),
    s: Math.round(s * 100),
    l: Math.round(l * 100),
  }
}

function parseColor(input: string): ColorFormats | null {
  const trimmed = input.trim()

  const hexMatch = trimmed.match(/^#?([a-f\d]{3}|[a-f\d]{6})$/i)
  if (hexMatch) {
    let hex = hexMatch[1]
    if (hex.length === 3) {
      hex = hex
        .split('')
        .map(c => c + c)
        .join('')
    }
    hex = '#' + hex.toLowerCase()

    const rgb = hexToRgb(hex)
    if (!rgb) return null

    return {
      hex,
      rgb,
      hsl: rgbToHsl(rgb.r, rgb.g, rgb.b),
    }
  }

  const rgbMatch = trimmed.match(
    /^rgb\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*\)$/i
  )
  if (rgbMatch) {
    const r = parseInt(rgbMatch[1])
    const g = parseInt(rgbMatch[2])
    const b = parseInt(rgbMatch[3])

    if (r > 255 || g > 255 || b > 255) return null

    return {
      hex: rgbToHex(r, g, b),
      rgb: { r, g, b },
      hsl: rgbToHsl(r, g, b),
    }
  }

  return null
}

const inputSchema = z.object({
  color: z
    .string()
    .describe('Color value (hex like #3b82f6 or rgb(59,130,246))'),
})

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-color-converter',
    name: '颜色转换器',
    version: '0.1.0',
    description: 'HEX/RGB/HSL 颜色格式互转',
    permissions: ['clipboard'],
    tags: ['color', 'hex', 'rgb', 'hsl', 'converter'],
    category: '开发工具',
  },
  inputSchema,
  async run(_ctx, input: z.infer<typeof inputSchema>) {
    const { color } = input
    if (!color) return result.text('Error: color is required')
    const parsed = parseColor(color)
    if (!parsed) return result.text('Error: invalid color format')
    return result.json({
      hex: parsed.hex,
      rgb: parsed.rgb,
      hsl: parsed.hsl,
      cssRgb: `rgb(${parsed.rgb.r}, ${parsed.rgb.g}, ${parsed.rgb.b})`,
      cssHsl: `hsl(${parsed.hsl.h}, ${parsed.hsl.s}%, ${parsed.hsl.l}%)`,
    })
  },
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

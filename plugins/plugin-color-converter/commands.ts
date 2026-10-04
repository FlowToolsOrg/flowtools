import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export interface ColorFormats {
  hex: string
  rgb: { r: number; g: number; b: number }
  hsl: { h: number; s: number; l: number }
}

export function hexToRgb(
  hex: string
): { r: number; g: number; b: number } | null {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
  return result
    ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16),
      }
    : null
}

export function rgbToHex(r: number, g: number, b: number): string {
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

export function rgbToHsl(
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

export function parseColor(input: string): ColorFormats | null {
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

export const inputSchema = z.object({
  color: z
    .string()
    .describe('Color value (hex like #3b82f6 or rgb(59,130,246))'),
})

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-color-converter',
    name: '颜色转换器',
    version: '0.1.0',
    maturity: 'prototype',
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
      result: {
        hex: parsed.hex,
        rgb: `${parsed.rgb.r},${parsed.rgb.g},${parsed.rgb.b}`,
        hsl: `${parsed.hsl.h},${parsed.hsl.s}%,${parsed.hsl.l}%`,
      },
    })
  },
})

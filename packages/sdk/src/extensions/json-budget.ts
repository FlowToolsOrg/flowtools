import { ExtensionContributionError } from './types'

/**
 * Count UTF-8 JSON bytes without materializing the serialized document.
 * Call only after strict JSON validation; stop as soon as the budget is exceeded.
 */
export function boundedJsonBytes(value: unknown, maxBytes: number): number {
  let bytes = 0
  const add = (count: number): void => {
    bytes += count
    if (bytes > maxBytes)
      throw new ExtensionContributionError('BUDGET_EXCEEDED')
  }
  const stringBytes = (text: string): void => {
    add(2)
    for (let index = 0; index < text.length; index++) {
      const code = text.charCodeAt(index)
      if (code === 0x22 || code === 0x5c) {
        add(2)
      } else if (code <= 0x1f) {
        add([0x08, 0x09, 0x0a, 0x0c, 0x0d].includes(code) ? 2 : 6)
      } else if (code <= 0x7f) {
        add(1)
      } else if (code <= 0x7ff) {
        add(2)
      } else if (code >= 0xd800 && code <= 0xdbff) {
        const next = text.charCodeAt(index + 1)
        if (next >= 0xdc00 && next <= 0xdfff) {
          add(4)
          index++
        } else {
          // Well-formed JSON.stringify escapes an unpaired surrogate.
          add(6)
        }
      } else if (code >= 0xdc00 && code <= 0xdfff) {
        add(6)
      } else {
        add(3)
      }
    }
  }
  const visit = (item: unknown): void => {
    if (item === null) {
      add(4)
    } else if (typeof item === 'boolean') {
      add(item ? 4 : 5)
    } else if (typeof item === 'number') {
      // Finite JSON numbers use the same spelling as String(number), including -0.
      add(String(item).length)
    } else if (typeof item === 'string') {
      stringBytes(item)
    } else if (Array.isArray(item)) {
      add(2)
      for (let index = 0; index < item.length; index++) {
        if (index) add(1)
        visit(item[index])
      }
    } else if (typeof item === 'object') {
      add(2)
      const object = item as Record<string, unknown>
      const keys = Object.keys(object)
      for (let index = 0; index < keys.length; index++) {
        if (index) add(1)
        const key = keys[index]!
        stringBytes(key)
        add(1)
        visit(object[key])
      }
    } else {
      throw new ExtensionContributionError('INVALID_DOCUMENT')
    }
  }
  visit(value)
  return bytes
}

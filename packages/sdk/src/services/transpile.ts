import { transform } from 'sucrase'

const TSX_EXTENSIONS = ['.ts', '.tsx', '.jsx']

/**
 * Check if a file needs transpilation (TypeScript or JSX).
 */
export function needsTranspilation(filename: string): boolean {
  const dotIndex = filename.lastIndexOf('.')
  const ext = dotIndex >= 0 ? filename.slice(dotIndex).toLowerCase() : ''

  return TSX_EXTENSIONS.includes(ext)
}

/**
 * Transpile TSX/TS/JSX source code to vanilla JavaScript using Sucrase.
 */
export function transpile(code: string, filePath: string): string {
  try {
    const result = transform(code, {
      transforms: ['typescript', 'jsx'],
      jsxRuntime: 'automatic',
      production: true,
    })

    return result.code
  } catch (error) {
    throw new Error(
      `[transpile] Failed to transpile "${filePath}": ${error instanceof Error ? error.message : String(error)}`
    )
  }
}

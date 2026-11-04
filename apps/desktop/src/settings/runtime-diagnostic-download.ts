import type { DesktopDiagnosticVersions } from './runtime-status-diagnostic'

import { getTauriVersion, getVersion } from '@tauri-apps/api/app'

export async function readDesktopDiagnosticVersions(): Promise<DesktopDiagnosticVersions> {
  const [appVersion, tauriVersion] = await Promise.all([
    getVersion(),
    getTauriVersion(),
  ])
  return { appVersion, tauriVersion }
}

/** Called only by the explicit download button, after the report is reviewed. */
export function downloadRuntimeStatusDiagnostic(text: string) {
  const url = URL.createObjectURL(
    new Blob([text], { type: 'application/json' })
  )
  try {
    const link = document.createElement('a')
    link.href = url
    link.download = 'flowtools-runtime-status.json'
    link.click()
  } finally {
    URL.revokeObjectURL(url)
  }
}

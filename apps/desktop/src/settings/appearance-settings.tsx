import { Button } from '@heroui/react'

import { useDesktopAppearance } from './appearance'

const options = [
  { mode: 'system', label: '系统' },
  { mode: 'light', label: '浅色' },
  { mode: 'dark', label: '深色' },
] as const

export function DesktopAppearanceSettings() {
  const { mode, notice, select, reset } = useDesktopAppearance()
  return (
    <section
      aria-labelledby="desktop-appearance-title"
      className="grid grid-cols-[160px_minmax(0,1fr)] items-start gap-4 border-b border-(--divider-color) py-2 text-sm max-[640px]:grid-cols-1"
    >
      <h2
        className="m-0 pt-2 text-sm font-normal text-(--text-secondary)"
        id="desktop-appearance-title"
      >
        外观
      </h2>
      <div className="grid gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <div
            aria-label="显示模式"
            className="flex rounded-lg border border-(--divider-color) p-0.5"
            role="group"
          >
            {options.map(option => (
              <Button
                aria-pressed={mode === option.mode}
                className={`h-7 px-2.5 text-xs ${mode === option.mode ? 'bg-(--active-bg) font-semibold ring-1 ring-(--primary-color) ring-inset' : 'bg-transparent'}`}
                key={option.mode}
                onPress={() => select(option.mode)}
                size="sm"
                variant="ghost"
              >
                {option.label}
              </Button>
            ))}
          </div>
          <Button onPress={reset} size="sm" variant="tertiary">
            重置外观
          </Button>
        </div>
        <p className="m-0 text-xs text-(--text-secondary)" role="status">
          {notice ?? '外观保存在此设备，重启后继续使用。'}
        </p>
      </div>
    </section>
  )
}

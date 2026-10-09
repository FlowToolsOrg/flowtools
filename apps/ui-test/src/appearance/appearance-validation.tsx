import { useCallback, useMemo, useState, useSyncExternalStore } from 'react'

import { createAppearanceResolver } from '@flowtools/sdk/extensions'
import { AppearanceScope, SettingsInputField, ToolCard } from '@flowtools/ui'

import { Button, Card, Input, Label, TextField } from '@heroui/react'

import { createPreviewRegistry, previewDefaults } from './fixtures'
import './appearance-validation.css'

interface Selection {
  selectedKey: string | null
  mode: 'system' | 'light' | 'dark'
  radius: number | null
}

const initialSelection: Selection = {
  selectedKey: null,
  mode: 'system',
  radius: null,
}

function useMediaPreference(query: string): boolean {
  const media = useMemo(() => window.matchMedia(query), [query])
  const subscribe = useCallback(
    (listener: () => void) => {
      media.addEventListener('change', listener)
      return () => media.removeEventListener('change', listener)
    },
    [media]
  )
  const snapshot = useCallback(() => media.matches, [media])
  return useSyncExternalStore(subscribe, snapshot, () => false)
}

function themeName(key: string | null): string {
  if (key === 'theme-lab:bay') return '海湾'
  if (key === 'theme-lab:ink') return '墨线'
  if (key === 'theme-lab:broken') return '损坏示例'
  return '默认外观'
}

export function AppearanceValidation() {
  const [contributions] = useState(createPreviewRegistry)
  const [resolve] = useState(() => createAppearanceResolver(previewDefaults))
  const entries = useSyncExternalStore(
    contributions.registry.subscribe,
    contributions.registry.getSnapshot,
    contributions.registry.getSnapshot
  )
  const [draft, setDraft] = useState(initialSelection)
  const [applied, setApplied] = useState(initialSelection)
  const [message, setMessage] = useState('选择主题即可预览。')
  const [workspace, setWorkspace] = useState('我的工作空间')
  const [note, setNote] = useState('整理本周资料')
  const systemDark = useMediaPreference('(prefers-color-scheme: dark)')
  const systemReducedMotion = useMediaPreference(
    '(prefers-reduced-motion: reduce)'
  )
  const appearance = resolve({
    contributions: entries,
    selectedKey: draft.selectedKey,
    mode: draft.mode,
    systemMode: systemDark ? 'dark' : 'light',
    systemReducedMotion,
    overrides:
      draft.radius === null
        ? undefined
        : {
            formatVersion: 1,
            common: {
              radii: { panelRem: draft.radius, controlRem: draft.radius },
            },
          },
  })
  const enabled = entries.length > 0

  return (
    <main className="appearance-lab">
      <header className="appearance-lab__header">
        <p className="appearance-lab__eyebrow">FLOWTOOLS · 外观实验室</p>
        <h1>让工作空间更像你</h1>
        <p>预览颜色、圆角与字体。应用仅保留在本次打开期间。</p>
      </header>
      <div className="appearance-lab__layout">
        <aside aria-label="外观控制" className="appearance-lab__controls">
          <h2>外观设置</h2>
          <label>
            主题
            <select
              aria-label="主题"
              onChange={event =>
                setDraft(current => ({
                  ...current,
                  selectedKey: event.target.value || null,
                }))
              }
              value={draft.selectedKey ?? ''}
            >
              <option value="">默认外观</option>
              <option value="theme-lab:bay">海湾</option>
              <option value="theme-lab:ink">墨线</option>
              <option value="theme-lab:broken">损坏示例</option>
            </select>
          </label>
          <label>
            显示模式
            <select
              aria-label="显示模式"
              onChange={event => {
                const mode = event.target.value
                if (mode === 'system' || mode === 'light' || mode === 'dark')
                  setDraft(current => ({ ...current, mode }))
              }}
              value={draft.mode}
            >
              <option value="system">跟随系统</option>
              <option value="light">浅色</option>
              <option value="dark">深色</option>
            </select>
          </label>
          <label className="appearance-lab__check">
            <input
              checked={draft.radius !== null}
              onChange={event =>
                setDraft(current => ({
                  ...current,
                  radius: event.target.checked ? 1 : null,
                }))
              }
              type="checkbox"
            />
            自定义圆角
          </label>
          {draft.radius !== null && (
            <label>
              圆角大小（{draft.radius} rem）
              <input
                aria-label="圆角大小"
                max="3"
                min="0"
                onChange={event =>
                  setDraft(current => ({
                    ...current,
                    radius: Number(event.target.value),
                  }))
                }
                step="0.25"
                type="range"
                value={draft.radius}
              />
            </label>
          )}
          <button
            className="appearance-lab__apply"
            onClick={() => {
              setApplied(draft)
              setMessage('已应用外观。')
            }}
            type="button"
          >
            应用主题
          </button>
          <button
            onClick={() => {
              setDraft(applied)
              setMessage('已取消预览。')
            }}
            type="button"
          >
            取消预览
          </button>
          <button
            onClick={() => {
              setDraft(initialSelection)
              setApplied(initialSelection)
              setMessage('已恢复默认外观。')
            }}
            type="button"
          >
            恢复默认
          </button>
          <p className="appearance-lab__saved">
            已应用：{themeName(applied.selectedKey)}
          </p>
          <p aria-live="polite" role="status">
            {message}
          </p>
          <div className="appearance-lab__recovery">
            <h3>恢复演练</h3>
            <p>停用主题包后会回到默认外观，再次启用可恢复原选择。</p>
            <button
              onClick={() =>
                enabled ? contributions.disable() : contributions.enable()
              }
              type="button"
            >
              {enabled ? '停用主题包' : '启用主题包'}
            </button>
          </div>
        </aside>
        <div className="appearance-lab__stage">
          <div className="appearance-lab__preview-label">
            <h2>实时预览</h2>
            <span>
              {appearance.title} ·{' '}
              {appearance.mode === 'dark' ? '深色' : '浅色'}
            </span>
          </div>
          {appearance.fallback === 'missing-theme' && (
            <p role="status">主题包已停用，正在使用默认外观。原选择已保留。</p>
          )}
          {appearance.fallback === 'invalid-theme' && (
            <p role="status">主题数据无效，已回到默认外观。</p>
          )}
          <AppearanceScope appearance={appearance} label="主题预览">
            <div className="appearance-lab__canvas">
              <div className="appearance-lab__workspace">
                <span className="appearance-lab__mark" aria-hidden="true">
                  F
                </span>
                <div>
                  <p>工作空间</p>
                  <h2>{workspace}</h2>
                </div>
                <span className="appearance-lab__count">12 个工具</span>
              </div>
              <Card aria-label="快速开始" className="appearance-lab__feature">
                <Card.Header>
                  <Card.Title>把重复的工作，留给工具</Card.Title>
                  <Card.Description>
                    从一个小任务开始，让今天的工作轻一点。
                  </Card.Description>
                </Card.Header>
                <Card.Content>
                  <TextField onChange={setNote} value={note}>
                    <Label>任务名称</Label>
                    <Input placeholder="输入一个任务" />
                  </TextField>
                </Card.Content>
                <Card.Footer>
                  <Button>开始整理</Button>
                </Card.Footer>
              </Card>
              <div className="appearance-lab__cards">
                <ToolCard aria-label="文本工具">
                  <ToolCard.Header>
                    <ToolCard.Title>文本整理</ToolCard.Title>
                    <ToolCard.Description>
                      去除多余空格，统一文本格式。
                    </ToolCard.Description>
                  </ToolCard.Header>
                  <ToolCard.Actions>
                    <Button variant="secondary">打开工具</Button>
                  </ToolCard.Actions>
                </ToolCard>
                <ToolCard aria-label="颜色工具">
                  <ToolCard.Header>
                    <ToolCard.Title>色彩转换</ToolCard.Title>
                    <ToolCard.Description>
                      快速查看与转换常用颜色。
                    </ToolCard.Description>
                  </ToolCard.Header>
                  <ToolCard.Actions>
                    <Button variant="secondary">查看色彩</Button>
                  </ToolCard.Actions>
                </ToolCard>
              </div>
              <SettingsInputField
                data-testid="appearance-settings-field"
                description="这个名字只用于预览。"
                label="工作空间名称"
                onChange={setWorkspace}
                value={workspace}
              />
              <p className="appearance-lab__motion">
                {appearance.reducedMotion
                  ? '已减少动态效果'
                  : '动态效果跟随系统偏好'}
              </p>
            </div>
          </AppearanceScope>
        </div>
      </div>
    </main>
  )
}

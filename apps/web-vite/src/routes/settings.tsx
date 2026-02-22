import { useMemo, useState } from 'react'

import {
  SettingsCenterItem,
  SettingsCenterPage,
  SettingsGroupCard,
  SettingsInputField,
  SettingsSelectField,
  SettingsSwitchField,
  ToolLayout,
  ToolLayoutMain,
  ToolLayoutSidebar,
  type SettingsNavSection,
} from '@flow-tool/ui'
import { createFileRoute } from '@tanstack/react-router'

import { Button } from '@heroui/react'

const navSections: SettingsNavSection[] = [
  { id: 'general', label: 'General', description: 'Basic host options' },
  { id: 'runtime', label: 'Runtime', description: 'Execution behavior' },
  { id: 'developer', label: 'Developer', description: 'Debug and logs' },
]

export const Route = createFileRoute('/settings')({
  component: SettingsPage,
})

function SettingsPage() {
  const [activeSectionId, setActiveSectionId] = useState(
    navSections[0]?.id ?? 'general'
  )

  const [autoUpdate, setAutoUpdate] = useState(true)
  const [telemetry, setTelemetry] = useState(false)
  const [startupMode, setStartupMode] = useState<string | number>('workspace')
  const [workspaceName, setWorkspaceName] = useState('Flow Workspace')

  const sectionTitle = useMemo(() => {
    return (
      navSections.find(section => section.id === activeSectionId)?.label ??
      'General'
    )
  }, [activeSectionId])

  return (
    <ToolLayout>
      <ToolLayoutMain>
        <SettingsCenterPage>
          <SettingsCenterPage.Nav
            activeSectionId={activeSectionId}
            onSectionChange={setActiveSectionId}
            sections={navSections}
          />
          <SettingsCenterPage.Content>
            <SettingsGroupCard>
              <SettingsGroupCard.Header>
                <SettingsGroupCard.Title>
                  {sectionTitle}
                </SettingsGroupCard.Title>
                <SettingsGroupCard.Description>
                  Pure presentation settings page built with settings-center
                  components.
                </SettingsGroupCard.Description>
              </SettingsGroupCard.Header>
              <SettingsGroupCard.Content>
                <SettingsSwitchField
                  description="Automatically update installed tools."
                  isSelected={autoUpdate}
                  label="Auto update tools"
                  onChange={setAutoUpdate}
                />
                <SettingsSwitchField
                  description="Send anonymous usage diagnostics."
                  isSelected={telemetry}
                  label="Enable telemetry"
                  onChange={setTelemetry}
                />
                <SettingsSelectField
                  description="Select the first screen after startup."
                  label="Startup mode"
                  onChange={value => setStartupMode(value ?? 'workspace')}
                  options={[
                    { key: 'workspace', label: 'Workspace' },
                    { key: 'market', label: 'Market' },
                    { key: 'run', label: 'Run Panel' },
                  ]}
                  value={startupMode}
                />
                <SettingsInputField
                  description="Display name in host header and logs."
                  label="Workspace name"
                  onChange={setWorkspaceName}
                  value={workspaceName}
                />
                <SettingsCenterItem>
                  <div>
                    <SettingsCenterItem.Label>
                      Open advanced docs
                    </SettingsCenterItem.Label>
                    <SettingsCenterItem.Description>
                      Jump to architecture and runtime documentation.
                    </SettingsCenterItem.Description>
                  </div>
                  <SettingsCenterItem.Control>
                    <Button size="sm" variant="ghost">
                      Open
                    </Button>
                  </SettingsCenterItem.Control>
                </SettingsCenterItem>
              </SettingsGroupCard.Content>
            </SettingsGroupCard>
          </SettingsCenterPage.Content>
        </SettingsCenterPage>
      </ToolLayoutMain>
      <ToolLayoutSidebar>
        <h3 className="text-sm font-semibold text-slate-900">
          Settings Snapshot
        </h3>
        <p className="text-sm text-slate-600">
          auto-update: <strong>{autoUpdate ? 'on' : 'off'}</strong>
        </p>
        <p className="text-sm text-slate-600">
          telemetry: <strong>{telemetry ? 'on' : 'off'}</strong>
        </p>
        <p className="text-sm text-slate-600">
          startup: <strong>{String(startupMode)}</strong>
        </p>
        <p className="text-sm text-slate-600">
          workspace: <strong>{workspaceName}</strong>
        </p>
      </ToolLayoutSidebar>
    </ToolLayout>
  )
}

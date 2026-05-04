import { useState } from 'react'

import {
  SettingsCenterItem,
  SettingsCenterPage,
  SettingsGroupCard,
  SettingsInputField,
  SettingsSelectField,
  SettingsSwitchField,
  type SettingsNavSection,
} from '@flowtools/ui'
import {
  SettingsIcon,
  SlidersHorizontalIcon,
  TerminalIcon,
  ZapIcon,
} from '@flowtools/ui/icons'
import { createFileRoute } from '@tanstack/react-router'
import { useStore } from 'zustand'

import { Button, Chip } from '@heroui/react'

import { settingsStore } from '@/stores/settings-store'

const navSections: (SettingsNavSection & {
  icon: typeof SlidersHorizontalIcon
})[] = [
  {
    id: 'general',
    label: 'General',
    description: 'Basic host options',
    icon: SlidersHorizontalIcon,
  },
  {
    id: 'runtime',
    label: 'Runtime',
    description: 'Execution behavior',
    icon: ZapIcon,
  },
  {
    id: 'developer',
    label: 'Developer',
    description: 'Debug and logs',
    icon: TerminalIcon,
  },
]

export const Route = createFileRoute('/settings')({
  component: SettingsPage,
})

function SettingsPage() {
  const [activeSectionId, setActiveSectionId] = useState(
    navSections[0]?.id ?? 'general'
  )

  const {
    autoUpdate,
    telemetry,
    startupMode,
    workspaceName,
    setAutoUpdate,
    setTelemetry,
    setStartupMode,
    setWorkspaceName,
  } = useStore(settingsStore)

  const activeSection = navSections.find(
    section => section.id === activeSectionId
  )
  const sectionTitle = activeSection?.label ?? 'General'
  const SectionIcon = activeSection?.icon ?? SlidersHorizontalIcon

  return (
    <div className="flex flex-col gap-6 p-6 lg:p-8">
      <header className="flex items-center gap-2.5 space-y-0">
        <SettingsIcon className="text-(--accent)" size={22} />
        <div>
          <h1 className="text-2xl font-semibold text-(--foreground)">
            Settings
          </h1>
          <p className="text-sm text-(--muted)">
            Configure your workspace and runtime behavior.
          </p>
        </div>
      </header>

      <div className="grid min-h-0 flex-1 gap-6 xl:grid-cols-[minmax(0,1fr)_18rem]">
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
                  <span className="flex items-center gap-2">
                    <SectionIcon size={16} />
                    {sectionTitle}
                  </span>
                </SettingsGroupCard.Title>
                <SettingsGroupCard.Description>
                  Manage your host preferences.
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
                  onChange={value =>
                    setStartupMode(String(value ?? 'workspace'))
                  }
                  options={[
                    { key: 'workspace', label: 'Dashboard' },
                    { key: 'market', label: 'Tools' },
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

        <aside className="hidden min-w-0 xl:block">
          <div className="sticky top-6 space-y-3 rounded-(--radius) border border-(--border) bg-(--surface) p-4">
            <h3 className="text-sm font-semibold text-(--foreground)">
              Current Settings
            </h3>
            <div className="space-y-2 text-sm">
              <div className="flex items-center justify-between">
                <span className="text-(--muted)">Auto-update</span>
                <Chip
                  color={autoUpdate ? 'success' : 'default'}
                  size="sm"
                  variant="soft"
                >
                  {autoUpdate ? 'on' : 'off'}
                </Chip>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-(--muted)">Telemetry</span>
                <Chip
                  color={telemetry ? 'success' : 'default'}
                  size="sm"
                  variant="soft"
                >
                  {telemetry ? 'on' : 'off'}
                </Chip>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-(--muted)">Startup</span>
                <span className="font-medium text-(--foreground)">
                  {startupMode}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-(--muted)">Workspace</span>
                <span className="font-medium text-(--foreground)">
                  {workspaceName}
                </span>
              </div>
            </div>
          </div>
        </aside>
      </div>
    </div>
  )
}

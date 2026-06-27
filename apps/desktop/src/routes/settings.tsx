import { createFileRoute } from '@tanstack/react-router'

import { SettingsView } from '../App'

export const Route = createFileRoute('/settings')({
  component: SettingsView,
})

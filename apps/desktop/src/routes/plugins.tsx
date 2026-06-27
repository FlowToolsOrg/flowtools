import { createFileRoute } from '@tanstack/react-router'

import { PluginsView } from '../App'

export const Route = createFileRoute('/plugins')({
  component: PluginsView,
})

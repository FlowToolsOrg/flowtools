import { createFileRoute } from '@tanstack/react-router'

import { LauncherView } from '../App'

export const Route = createFileRoute('/')({
  component: LauncherView,
})

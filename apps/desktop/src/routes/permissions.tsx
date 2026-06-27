import { createFileRoute } from '@tanstack/react-router'

import { PermissionsView } from '../App'

export const Route = createFileRoute('/permissions')({
  component: PermissionsView,
})

import { createFileRoute } from '@tanstack/react-router'

import { CommandRunView } from '../../App'

export const Route = createFileRoute('/run/$commandId')({
  component: CommandRunView,
})

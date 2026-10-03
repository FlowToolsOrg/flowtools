import type { ComponentProps } from 'react'

import {
  CommandPalette,
  ToolLayoutMain,
  ToolLayoutSidebar,
  type CommandPaletteItem,
  type ToolEntity,
  type ToolLayoutMainProps,
} from '@flowtools/ui'
import { HomeIcon, WrenchIcon } from '@flowtools/ui/icons'
import {
  Button,
  ColorPicker,
  toast,
  type Key,
  type Selection,
  type ValidationResult,
} from '@flowtools/ui/plugin'

export const runtimeExports = [
  CommandPalette,
  ToolLayoutMain,
  ToolLayoutSidebar,
  Button,
  ColorPicker,
  toast,
  HomeIcon,
  WrenchIcon,
] as const

export const commandItem: CommandPaletteItem = {
  id: 'consumer-contract',
  title: 'Consumer contract',
}

export const toolEntity: ToolEntity = {
  id: 'consumer-contract',
  name: 'Consumer contract',
  description: 'Validates the generated package declarations',
  status: 'stable',
}

export interface ConsumerTypeContract {
  rootProps: ToolLayoutMainProps
  inferredRootProps: ComponentProps<typeof ToolLayoutMain>
  pluginButtonProps: ComponentProps<typeof Button>
  iconProps: ComponentProps<typeof HomeIcon>
  key: Key
  selection: Selection
  validationResult: ValidationResult
}

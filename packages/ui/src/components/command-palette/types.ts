/**
 * A searchable command entry shown in the command palette.
 */
export interface CommandPaletteItem {
  /**
   * Unique command id.
   */
  id: string
  /**
   * Display title.
   */
  title: string
  /**
   * Optional description shown below the title.
   */
  description?: string
  /**
   * Source plugin or section name.
   */
  source?: string
  /**
   * Optional icon identifier.
   */
  icon?: string
  /**
   * Optional keyboard shortcut hint (display only).
   */
  shortcut?: string
  /**
   * Whether this command is the most recently used.
   */
  isRecent?: boolean
}

/**
 * Props for the CommandPalette root component.
 */
export interface CommandPaletteRootProps {
  /**
   * Whether the palette is visible.
   */
  open: boolean
  /**
   * Called when the palette requests to close.
   */
  onClose: () => void
  /**
   * All available commands.
   */
  items: CommandPaletteItem[]
  /**
   * Called when a command is selected and confirmed.
   */
  onSelect: (item: CommandPaletteItem) => void
  /**
   * Optional placeholder text for the search input.
   */
  placeholder?: string
}

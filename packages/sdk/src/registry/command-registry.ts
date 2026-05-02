import type { CommandMode } from '../types/command'

/**
 * A command registered in the command registry.
 */
export interface RegisteredCommand {
  /**
   * Unique command id (typically pluginId:commandId).
   */
  id: string
  /**
   * Display title shown in command palette.
   */
  title: string
  /**
   * Optional description.
   */
  description?: string
  /**
   * Source plugin id.
   */
  pluginId: string
  /**
   * Command execution mode.
   */
  mode: CommandMode
  /**
   * Keywords for search matching.
   */
  keywords: string[]
  /**
   * Optional icon identifier.
   */
  icon?: string
  /**
   * Optional keyboard shortcut hint (display only).
   */
  shortcut?: string
  /**
   * Execute the command.
   */
  handler: () => void | Promise<void>
}

/**
 * Listener for command registry changes.
 */
export type CommandRegistryListener = () => void

const MAX_RECENT = 5

/**
 * Registry for all host and plugin commands.
 * Used by the command palette to search and execute commands.
 */
export class CommandRegistry {
  private commands = new Map<string, RegisteredCommand>()
  private recentIds: string[] = []
  private listeners = new Set<CommandRegistryListener>()

  /**
   * Register a command.
   */
  register(command: RegisteredCommand): void {
    this.commands.set(command.id, command)
    this.notify()
  }

  /**
   * Unregister all commands from a plugin.
   */
  unregisterByPlugin(pluginId: string): void {
    let changed = false

    for (const [id, cmd] of this.commands) {
      if (cmd.pluginId === pluginId) {
        this.commands.delete(id)
        changed = true
      }
    }

    this.recentIds = this.recentIds.filter(id => this.commands.has(id))

    if (changed) {
      this.notify()
    }
  }

  /**
   * Get all registered commands.
   */
  getAll(): RegisteredCommand[] {
    return [...this.commands.values()]
  }

  /**
   * Get a command by id.
   */
  get(commandId: string): RegisteredCommand | undefined {
    return this.commands.get(commandId)
  }

  /**
   * Search commands by query string.
   * Matches against title, description, keywords, and pluginId.
   */
  search(query: string): RegisteredCommand[] {
    const trimmed = query.trim().toLowerCase()

    if (trimmed.length === 0) {
      return this.getOrdered()
    }

    const all = this.getAll()
    const scored: Array<{ cmd: RegisteredCommand; score: number }> = []

    for (const cmd of all) {
      let score = 0

      if (cmd.title.toLowerCase().includes(trimmed)) {
        score += 10
      }

      if (cmd.description?.toLowerCase().includes(trimmed)) {
        score += 5
      }

      if (cmd.keywords.some(k => k.toLowerCase().includes(trimmed))) {
        score += 3
      }

      if (cmd.pluginId.toLowerCase().includes(trimmed)) {
        score += 1
      }

      if (score > 0) {
        scored.push({ cmd, score })
      }
    }

    scored.sort((a, b) => b.score - a.score)

    return scored.map(s => s.cmd)
  }

  /**
   * Get commands ordered with recent items first.
   */
  getOrdered(): RegisteredCommand[] {
    const all = this.getAll()
    const recentSet = new Set(this.recentIds)

    const recent = this.recentIds
      .map(id => all.find(cmd => cmd.id === id))
      .filter(Boolean) as RegisteredCommand[]

    const rest = all.filter(cmd => !recentSet.has(cmd.id))

    return [...recent, ...rest]
  }

  /**
   * Get recently executed commands.
   */
  getRecent(): RegisteredCommand[] {
    return this.recentIds
      .map(id => this.commands.get(id))
      .filter(Boolean) as RegisteredCommand[]
  }

  /**
   * Execute a command by id.
   * Records it in recent history on success.
   */
  async execute(commandId: string): Promise<void> {
    const cmd = this.commands.get(commandId)
    if (!cmd) {
      return
    }

    await cmd.handler()
    this.recordRecent(commandId)
  }

  /**
   * Subscribe to registry changes.
   * Returns an unsubscribe function.
   */
  subscribe(listener: CommandRegistryListener): () => void {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  /**
   * Record a command as recently used.
   */
  private recordRecent(commandId: string): void {
    this.recentIds = [
      commandId,
      ...this.recentIds.filter(id => id !== commandId),
    ].slice(0, MAX_RECENT)

    this.notify()
  }

  /**
   * Notify all listeners of a change.
   */
  private notify(): void {
    for (const listener of this.listeners) {
      listener()
    }
  }
}

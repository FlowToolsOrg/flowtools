import type { CommandMode } from '../types/command'

import { CommandSearchIndex } from '../search'

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
  /** Host aliases are search metadata, never command IDs or permissions. */
  aliases?: string[]
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
  private index = new CommandSearchIndex()
  private recentIds: string[] = []
  private listeners = new Set<CommandRegistryListener>()

  /**
   * Register a command.
   */
  register(command: RegisteredCommand): void {
    const existing = this.commands.get(command.id)
    if (existing && existing.pluginId !== command.pluginId) {
      throw new Error('Command id belongs to another plugin')
    }
    this.index.upsert(command)
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
        this.index.remove(id)
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

    return this.index
      .search(trimmed, { recent: this.recentIds })
      .flatMap(result => {
        const command = this.commands.get(result.id)
        return command ? [command] : []
      })
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
    if (this.commands.get(commandId) === cmd) this.recordRecent(commandId)
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

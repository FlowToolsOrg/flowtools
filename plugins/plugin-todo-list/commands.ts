import { result } from '@flowtools/sdk/result'
import { z } from 'zod'

import { defineBuiltinCommand } from '../command-contract'

export const inputSchema = z.object({
  todo: z.string().optional().describe('Todo item text to add'),
  deadline: z.string().optional().describe('Deadline date (YYYY-MM-DD)'),
})

export const todoItemsSchema = z.array(
  z
    .object({
      todo: z.string(),
      deadline: z.string().default(''),
    })
    .passthrough()
)

export const todoStateSchema = z.object({ todos: todoItemsSchema })

export default defineBuiltinCommand({
  type: 'app',
  meta: {
    id: 'plugin-todo-list',
    name: 'Todo List',
    version: '0.0.1',
    maturity: 'prototype',
    permissions: ['storage'],
  },
  inputSchema,
  async run(ctx, input: z.infer<typeof inputSchema>) {
    if (!ctx.store && !ctx.storage)
      throw new Error('Todo storage capability is unavailable')
    // App hosts share their declared store with setup(); CLI keeps its existing key.
    // Validate before writing, preserve unrelated fields and do not migrate old keys.
    const items = ctx.store
      ? todoStateSchema.parse(ctx.store.getState()).todos
      : todoItemsSchema.parse(ctx.storage?.get('todos') ?? [])
    if (input.todo) {
      const item = { todo: input.todo, deadline: input.deadline ?? '' }
      const todos = [...items, item]
      if (ctx.store) ctx.store.setState({ todos })
      else ctx.storage?.set('todos', todos)
      return result.json({ result: { added: item.todo, total: todos.length } })
    }
    return result.json({
      result: items.map(
        (item, idx) =>
          `${idx + 1}. [${item.deadline || 'No Deadline'}] ${item.todo}`
      ),
      count: items.length,
    })
  },
})

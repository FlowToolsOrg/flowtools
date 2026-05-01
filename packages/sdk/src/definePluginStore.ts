import type { PluginStoreShape, PluginStoreState } from './types/store'

/**
 * Define a typed plugin store shape with initial state and optional actions.
 *
 * @example
 * const todoStore = definePluginStore({
 *   initialState: {
 *     todos: [] as TodoItem[],
 *   },
 *   actions: (set, get) => ({
 *     addTodo(item: TodoItem) {
 *       set(state => ({ todos: [...state.todos, item] }))
 *     },
 *     removeTodo(index: number) {
 *       set(state => ({
 *         todos: state.todos.filter((_, i) => i !== index),
 *       }))
 *     },
 *   }),
 * })
 */
export function definePluginStore<
  TState extends PluginStoreState,
  TActions extends Record<string, (...args: any[]) => void> = Record<
    string,
    never
  >,
>(
  shape: PluginStoreShape<TState, TActions>
): PluginStoreShape<TState, TActions> {
  return shape
}

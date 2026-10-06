import type { DataCapability, DataSnapshot } from '@flowtools/sdk/data'
import type { FormEvent } from 'react'

import { useEffect, useState } from 'react'

import {
  type InferStoreActions,
  type InferStoreState,
  definePlugin,
  useCapability,
  usePluginStore,
  usePluginStoreApi,
  definePluginStore,
} from '@flowtools/sdk'
import { hydratePluginData } from '@flowtools/sdk/data'
import { isJsonValue } from '@flowtools/sdk/manifest'
import {
  Button,
  Calendar,
  DateField,
  DatePicker,
  Form,
  Input,
  Label,
  TextField,
} from '@flowtools/ui/plugin'

import commandPlugin, { todoItemsSchema } from './commands'

export interface TodoItem {
  todo: string
  deadline: string
}

export const todoStore = definePluginStore({
  initialState: {
    todos: [] as TodoItem[],
  },
  actions: set => ({
    addTodo(item: TodoItem) {
      set(state => ({ todos: [...state.todos, item] }))
    },
    removeTodo(index: number) {
      set(state => ({
        todos: state.todos.filter((_, i) => i !== index),
      }))
    },
  }),
})

type TodoState = InferStoreState<typeof todoStore>

type TodoActions = InferStoreActions<typeof todoStore>

function readValue(input: FormDataEntryValue | null): string {
  if (typeof input !== 'string') {
    return ''
  }

  return input.trim()
}

export default definePlugin({
  ...commandPlugin,
  store: todoStore,
  setup() {
    return function TodoPanel() {
      const runtime = useCapability()
      return runtime.data ? (
        <SharedTodoPanel data={runtime.data} />
      ) : (
        <LocalTodoPanel />
      )
    }
  },
})

function SharedTodoPanel({ data }: { data: DataCapability }) {
  const [snapshot, setSnapshot] = useState<DataSnapshot>()
  const [todos, setTodos] = useState<TodoItem[]>([])
  const [message, setMessage] = useState('正在加载 Runtime 数据…')
  const [busy, setBusy] = useState(false)
  const [failed, setFailed] = useState(false)
  const [reload, setReload] = useState(0)
  const report = (error: unknown) => {
    const code =
      error instanceof Error && /^[A-Z_]+$/.test(error.message)
        ? error.message
        : 'INPUT_INVALID'
    setMessage(
      `${code}：${['RUNTIME_DISCONNECTED', 'SESSION_INVALID', 'INVALID_RESPONSE'].includes(code) ? '状态尚未确认；请重新读取并核对数据，不自动重试写入。' : '操作未完成，请检查授权或重新读取数据。'}`
    )
    setFailed(true)
  }
  const apply = (value: DataSnapshot) => {
    const items = todoItemsSchema.parse(value.value ?? [])
    setTodos(items)
    setSnapshot(value)
    setFailed(false)
    setMessage(`Runtime revision ${value.revision}`)
  }
  useEffect(() => {
    const abort = new AbortController()
    void hydratePluginData(
      data,
      'todos',
      value => {
        if (!abort.signal.aborted) apply(value)
      },
      abort.signal
    ).catch(error => {
      if (!abort.signal.aborted) report(error)
    })
    return () => abort.abort()
  }, [data, reload])
  const save = async (items: TodoItem[]) => {
    if (!snapshot || busy || failed) return false
    setBusy(true)
    try {
      if (!isJsonValue(items)) throw new Error('INPUT_INVALID')
      const value = await data.write({
        key: 'todos',
        expectedRevision: snapshot.revision,
        value: items,
      })
      apply(value)
      return true
    } catch (error) {
      report(error)
      return false
    } finally {
      setBusy(false)
    }
  }
  return (
    <section aria-label="共享 Todo 数据" className="space-y-4">
      <p role="status" data-testid="shared-todo-revision">
        {message}
      </p>
      <Button
        variant="secondary"
        onPress={() => setReload(value => value + 1)}
        isDisabled={busy}
      >
        重新读取 Todo
      </Button>
      <Form
        className="flex flex-wrap gap-3"
        onSubmit={event => {
          event.preventDefault()
          const form = event.currentTarget
          const values = new FormData(form)
          const todo = readValue(values.get('todo'))
          if (todo)
            void save([
              ...todos,
              { todo, deadline: readValue(values.get('deadline')) },
            ]).then(saved => {
              if (saved) form.reset()
            })
        }}
      >
        <TextField isRequired name="todo">
          <Label>Todo</Label>
          <Input />
        </TextField>
        <TextField name="deadline">
          <Label>Deadline</Label>
          <Input placeholder="YYYY-MM-DD" />
        </TextField>
        <Button type="submit" isDisabled={!snapshot || failed || busy}>
          Add
        </Button>
      </Form>
      <ul aria-label="共享 Todo Items" className="space-y-2">
        {todos.map((item, index) => (
          <li key={`${index}-${item.todo}`} className="flex items-center gap-3">
            <span>{item.todo}</span>
            <span>{item.deadline}</span>
            <Button
              variant="secondary"
              isDisabled={failed || busy}
              onPress={() =>
                void save(todos.filter((_, position) => position !== index))
              }
            >
              Delete {item.todo}
            </Button>
          </li>
        ))}
      </ul>
    </section>
  )
}

function LocalTodoPanel() {
  const { todos } = usePluginStore<TodoState>()
  const { actions } = usePluginStoreApi<TodoState, TodoActions>()
  const onSubmit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const formData = new FormData(e.currentTarget)
    const todo = readValue(formData.get('todo'))
    const deadline = readValue(formData.get('deadline'))
    if (!todo) {
      return
    }
    actions.addTodo({ todo, deadline })
    e.currentTarget.reset()
  }
  return (
    <div className="w-full">
      <div className="flex">
        <Form className="flex flex-row grow w-full" onSubmit={onSubmit}>
          <TextField
            isRequired
            name="todo"
            type="text"
            className="mx-auto w-72"
          >
            <Label>Todo</Label>
            <Input />
          </TextField>
          <DateField isRequired name="deadline" className="mx-auto w-72">
            <DatePicker className="w-full" name="date">
              <Label>Deadline</Label>
              <DateField.Group fullWidth>
                <DateField.Input>
                  {segment => <DateField.Segment segment={segment} />}
                </DateField.Input>
                <DateField.Suffix>
                  <DatePicker.Trigger>
                    <DatePicker.TriggerIndicator />
                  </DatePicker.Trigger>
                </DateField.Suffix>
              </DateField.Group>
              <DatePicker.Popover>
                <Calendar aria-label="Event date">
                  <Calendar.Header>
                    <Calendar.YearPickerTrigger>
                      <Calendar.YearPickerTriggerHeading />
                      <Calendar.YearPickerTriggerIndicator />
                    </Calendar.YearPickerTrigger>
                    <Calendar.NavButton slot="previous" />
                    <Calendar.NavButton slot="next" />
                  </Calendar.Header>
                  <Calendar.Grid>
                    <Calendar.GridHeader>
                      {day => <Calendar.HeaderCell>{day}</Calendar.HeaderCell>}
                    </Calendar.GridHeader>
                    <Calendar.GridBody>
                      {date => <Calendar.Cell date={date} />}
                    </Calendar.GridBody>
                  </Calendar.Grid>
                  <Calendar.YearPickerGrid>
                    <Calendar.YearPickerGridBody>
                      {({ year }) => <Calendar.YearPickerCell year={year} />}
                    </Calendar.YearPickerGridBody>
                  </Calendar.YearPickerGrid>
                </Calendar>
              </DatePicker.Popover>
            </DatePicker>
          </DateField>
          <Button className="mt-auto mx-auto w-48" type="submit">
            Add
          </Button>
        </Form>
      </div>
      <div className="mt-6">
        <h3 className="text-base font-semibold">Todo Items</h3>
        {todos.length === 0 ? (
          <p className="text-sm text-gray-500">No todo items yet.</p>
        ) : (
          <ul className="mt-2 space-y-2">
            {todos.map((item, index) => (
              <li
                key={`${item.todo}-${index}`}
                className="flex items-center justify-between rounded border px-3 py-2"
              >
                <span>{item.todo}</span>
                <span className="text-sm text-gray-500">{item.deadline}</span>
                <Button onPress={() => actions.removeTodo(index)}>
                  Delete
                </Button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  )
}

import type { StorageCapability } from '@flow-tool/sdk'
import type { FormEvent } from 'react'

import { definePlugin, useStorage } from '@flow-tool/sdk'
import {
  Input,
  Form,
  Button,
  DateField,
  DatePicker,
  Calendar,
  Label,
  TextField,
} from '@flow-tool/ui/plugin'
import { useStore } from 'zustand'
import { createJSONStorage, persist } from 'zustand/middleware'
import { createStore } from 'zustand/vanilla'

interface TodoItem {
  todo: string
  deadline: string
}

interface TodoStoreState {
  todos: TodoItem[]
  addTodo: (item: TodoItem) => void
}

type TodoStore = ReturnType<typeof createTodoStore>

let todoStore: TodoStore | undefined

function createTodoStore(storage: StorageCapability) {
  return createStore<TodoStoreState>()(
    persist(
      set => ({
        todos: [],
        addTodo: item =>
          set(state => ({
            todos: [...state.todos, item],
          })),
      }),
      {
        name: 'root',
        storage: createJSONStorage(() => storage.zustand('todo-list')),
      }
    )
  )
}

function useTodoStore<T>(selector: (state: TodoStoreState) => T): T {
  const storage = useStorage()
  const store = todoStore ?? createTodoStore(storage)

  if (!todoStore) {
    todoStore = store
  }

  return useStore(store, selector)
}

function readValue(input: FormDataEntryValue | null): string {
  if (typeof input !== 'string') {
    return ''
  }

  return input.trim()
}

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-todo-list',
    name: 'Todo List',
    version: '0.0.1',
    permissions: ['storage'],
  },
  setup() {
    return function () {
      const todos = useTodoStore(state => state.todos)
      const addTodo = useTodoStore(state => state.addTodo)

      const onSubmit = (e: FormEvent<HTMLFormElement>) => {
        e.preventDefault()
        const formData = new FormData(e.currentTarget)
        const todo = readValue(formData.get('todo'))
        const deadline = readValue(formData.get('deadline'))

        if (!todo) {
          return
        }

        addTodo({
          todo,
          deadline,
        })
        e.currentTarget.reset()
      }

      return (
        <div className="w-full">
          <div className="flex">
            <Form
              className="flex flex-row flex-grow w-full"
              onSubmit={onSubmit}
            >
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
                          {day => (
                            <Calendar.HeaderCell>{day}</Calendar.HeaderCell>
                          )}
                        </Calendar.GridHeader>
                        <Calendar.GridBody>
                          {date => <Calendar.Cell date={date} />}
                        </Calendar.GridBody>
                      </Calendar.Grid>
                      <Calendar.YearPickerGrid>
                        <Calendar.YearPickerGridBody>
                          {({ year }) => (
                            <Calendar.YearPickerCell year={year} />
                          )}
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
                    <span className="text-sm text-gray-500">
                      {item.deadline}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      )
    }
  },
})

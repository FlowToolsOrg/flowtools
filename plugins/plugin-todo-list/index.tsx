import type { FormEvent } from 'react'

import {
  type InferStoreActions,
  type InferStoreState,
  definePlugin,
  usePluginStore,
  usePluginStoreApi,
  definePluginStore,
} from '@flowtools/sdk'
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

import commandPlugin from './commands'

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
    return function () {
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
  },
})

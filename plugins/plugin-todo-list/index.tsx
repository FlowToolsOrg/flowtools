import { useStorage, definePlugin } from '@flow-tool/sdk'
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
      const storage = useStorage()

      const onSubmit = (e: React.FormEvent<HTMLFormElement>) => {
        e.preventDefault()
        const formData = new FormData(e.currentTarget)
        const data: Record<string, string> = {}
        // Convert FormData to plain object
        formData.forEach((value, key) => {
          data[key] = value.toString()
        })
        console.log(data)
        const pre = JSON.parse(storage.get('todo') || '[]')
        storage.set('todo', JSON.stringify([...pre, data]))
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
        </div>
      )
    }
  },
})

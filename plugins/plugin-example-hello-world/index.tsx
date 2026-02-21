import { definePlugin, useCapability } from '@flow-tool/sdk'
import { useEffect } from 'react'

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-example-hello-world',
    name: 'Hello World Plugin',
    version: '0.0.1',
    permissions: ['network', 'storage', 'notification'],
  },
  setup() {
    return function HelloWorldPanel() {
      // const request = useRequest()
      // const storage = useStorage()
      const { request, storage } = useCapability()

      console.log(request, storage)

      useEffect(() => {
        request('https://jsonplaceholder.typicode.com/todos/1')
          .then(response => response.json())
          .then(console.log)
      }, [request])

      return (
        <h1>
          hello-world
          <button onClick={() => storage.set('hello', 'world')}>set</button>
          <button onClick={() => console.log(storage.get('hello'))}>get</button>
        </h1>
      )
    }
  },
})

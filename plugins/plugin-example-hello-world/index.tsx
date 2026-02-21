import { definePlugin, useRequest, useStorage } from '@flow-tool/sdk'
import { useEffect } from 'react'

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-example-hello-world',
    name: 'Hello World Plugin',
    version: '0.0.1',
    permissions: ['network', 'storage'],
  },
  setup() {
    return function HelloWorldPanel() {
      const request = useRequest()
      const store = useStorage()

      useEffect(() => {
        request('https://jsonplaceholder.typicode.com/todos/1')
          .then(response => response.json())
          .then(console.log)
      }, [request])

      return (
        <h1>
          hello-world
          <button onClick={() => store.set('hello', 'world')}>set</button>
          <button onClick={() => console.log(store.get('hello'))}>get</button>
        </h1>
      )
    }
  },
})

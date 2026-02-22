import { useEffect } from 'react'

import { definePlugin, useCapability } from '@flow-tool/sdk'

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

      useEffect(() => {
        request('https://jsonplaceholder.typicode.com/todos/1')
          .then(response => response.json())
          // eslint-disable-next-line no-console
          .then(console.log)
      }, [request])

      return (
        <h1>
          hello-world
          <button onClick={() => storage.set('hello', 'world')}>set</button>
          {/* eslint-disable-next-line no-console */}
          <button onClick={() => console.log(storage.get('hello'))}>get</button>
        </h1>
      )
    }
  },
})

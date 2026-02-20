import { definePlugin } from '@flow-tool/sdk'

export default definePlugin({
  type: 'app',
  meta: {
    id: 'plugin-example-hello-world',
    name: 'Hello World Plugin',
    version: '0.0.1',
  },
  setup() {
    return function HelloWorldPanel() {
      return <h1>hello-world</h1>
    }
  },
})

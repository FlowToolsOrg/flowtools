import { definePlugin } from '@flow-tool/sdk'

export default definePlugin({
  type: 'tool',
  meta: {
    id: 'plugin-example-run-hello',
    name: 'run hello',
    version: '0.0.1',
  },
  run(_, input: string) {
    // eslint-disable-next-line no-console
    console.log('hello world', input)
    return 'hello world'
  },
})

import helloWorldPlugin from '@plugins/plugin-example-hello-world'
import imageBase64Plugin from '@plugins/plugin-image-base64'
import randomPickerPlugin from '@plugins/plugin-random-picker'
import textOpsPlugin from '@plugins/plugin-text-ops'
import todoListPlugin from '@plugins/plugin-todo-list'
import uuidGeneratorPlugin from '@plugins/plugin-uuid-generator'
import websiteLatencyPlugin from '@plugins/plugin-website-latency'

export {
  helloWorldPlugin,
  imageBase64Plugin,
  randomPickerPlugin,
  textOpsPlugin,
  todoListPlugin,
  uuidGeneratorPlugin,
  websiteLatencyPlugin,
}
export default [
  helloWorldPlugin,
  todoListPlugin,
  websiteLatencyPlugin,
  uuidGeneratorPlugin,
  imageBase64Plugin,
  randomPickerPlugin,
  textOpsPlugin,
]

import type { AppPlugin } from '@flow-tool/sdk'

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

const appPlugins: AppPlugin[] = [
  helloWorldPlugin,
  todoListPlugin,
  websiteLatencyPlugin,
  uuidGeneratorPlugin,
  imageBase64Plugin,
  randomPickerPlugin,
  textOpsPlugin,
]

export default appPlugins

export const getPluginById = (id: string): AppPlugin | undefined =>
  appPlugins.find(plugin => plugin.meta.id === id)

export const pluginCategories = [
  {
    id: 'text',
    label: 'Text & Data',
    description: 'Text processing and data tools',
    pluginIds: ['plugin-text-ops', 'plugin-uuid-generator'],
  },
  {
    id: 'image',
    label: 'Image',
    description: 'Image processing tools',
    pluginIds: ['plugin-image-base64'],
  },
  {
    id: 'network',
    label: 'Network',
    description: 'Network and connectivity tools',
    pluginIds: ['plugin-website-latency'],
  },
  {
    id: 'utility',
    label: 'Utility',
    description: 'General purpose tools',
    pluginIds: [
      'plugin-todo-list',
      'plugin-random-picker',
      'plugin-example-hello-world',
    ],
  },
] as const

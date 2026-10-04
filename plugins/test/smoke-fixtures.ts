export interface PluginSmokeFixture {
  id: string
  input: Record<string, unknown>
  expected: object
}

/** Complete inventory: only real compiled plugins, never a replacement run(). */
export const pluginSmokeFixtures: PluginSmokeFixture[] = [
  {
    id: 'plugin-base64-encoder',
    input: { text: 'hello' },
    expected: { type: 'json', value: { result: 'aGVsbG8=', mode: 'encode' } },
  },
  {
    id: 'plugin-color-converter',
    input: { color: '#ff0000' },
    expected: {
      type: 'json',
      value: { result: { hex: '#ff0000', rgb: '255,0,0' } },
    },
  },
  {
    id: 'plugin-hash-generator',
    input: { text: 'abc' },
    expected: {
      type: 'json',
      value: {
        result:
          'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
        algorithm: 'SHA-256',
      },
    },
  },
  {
    id: 'plugin-image-base64',
    input: { base64: 'aGVsbG8=' },
    expected: { type: 'json', value: { result: { estimatedBytes: 6 } } },
  },
  {
    id: 'plugin-json-formatter',
    input: { text: '{"a": 1}', mode: 'minify' },
    expected: { type: 'text', text: '{"a":1}' },
  },
  {
    id: 'plugin-random-picker',
    input: { names: 'Ada' },
    expected: { type: 'json', value: { result: ['Ada'], total: 1 } },
  },
  {
    id: 'plugin-regex-tester',
    input: { pattern: 'a+', text: 'caaad' },
    expected: {
      type: 'json',
      value: { result: [{ match: 'aaa', index: 1 }], matchCount: 1 },
    },
  },
  {
    id: 'plugin-text-ops',
    input: { setA: 'apple\nbanana', setB: 'banana\npear' },
    expected: {
      type: 'json',
      value: { result: ['banana'], operation: 'intersection' },
    },
  },
  {
    id: 'plugin-timestamp-converter',
    input: { timestamp: '0' },
    expected: { type: 'json', value: { result: '1970-01-01T00:00:00.000Z' } },
  },
  {
    id: 'plugin-todo-list',
    input: { todo: 'fixture task' },
    expected: {
      type: 'json',
      value: { result: { added: 'fixture task', total: 1 } },
    },
  },
  {
    id: 'plugin-uuid-generator',
    input: { count: 2 },
    expected: { type: 'json', value: { count: 2 } },
  },
  {
    id: 'plugin-website-latency',
    input: { urls: ['https://latency.fixture.invalid/'] },
    expected: { type: 'json', value: { tested: 1 } },
  },
]

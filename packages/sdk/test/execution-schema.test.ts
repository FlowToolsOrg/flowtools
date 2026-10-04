import { expect, test } from 'bun:test'

import { z } from 'zod'

import { describeInputSchema } from '../src/execution/schema'

test('schema presentation exposes defaults without changing runtime validation', () => {
  const schema = z.object({ count: z.number().default(2) })
  expect(describeInputSchema(schema)).toMatchObject({
    properties: { count: { default: 2 } },
  })
  expect(describeInputSchema()).toBeUndefined()
  expect(
    describeInputSchema(z.string().transform(value => value.length))
  ).toEqual({
    notice:
      'Schema cannot be represented as JSON; runtime validation remains enabled',
  })
  expect(schema.safeParse({ count: 'bad' }).success).toBe(false)
})

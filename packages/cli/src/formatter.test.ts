import { describe, expect, test } from 'bun:test'

import { formatResult } from './formatter'

describe('formatResult', () => {
  test('formats text results', () => {
    expect(formatResult({ type: 'text', text: 'hello' }, 'text')).toBe('hello')
  })

  test('formats JSON result display values', () => {
    expect(
      formatResult(
        {
          type: 'json',
          value: { result: ['one', 'two'], count: 2 },
        },
        'text'
      )
    ).toBe('one\ntwo')
  })

  test('formats table results', () => {
    const output = formatResult(
      {
        type: 'table',
        columns: [
          { key: 'name', title: 'Name' },
          { key: 'count', title: 'Count' },
        ],
        rows: [{ name: 'alpha', count: 2 }],
      },
      'text'
    )

    expect(output).toContain('Name  │ Count')
    expect(output).toContain('alpha │ 2')
  })

  test('formats empty tables and missing cells deterministically', () => {
    expect(formatResult({ type: 'table', columns: [], rows: [] }, 'text')).toBe(
      '(empty)'
    )

    const output = formatResult(
      {
        type: 'table',
        columns: [
          { key: 'name', title: 'Name' },
          { key: 'count', title: 'Count' },
        ],
        rows: [{ name: 'alpha' }],
      },
      'text'
    )
    expect(output).toContain('alpha │')
  })

  test('formats file results', () => {
    expect(
      formatResult({ type: 'file', path: '/tmp/report.txt' }, 'text')
    ).toBe('/tmp/report.txt')
  })

  test('formats open results', () => {
    expect(
      formatResult({ type: 'open', target: 'https://example.com' }, 'text')
    ).toBe('https://example.com')
  })

  test('formats multi results in order', () => {
    expect(
      formatResult(
        {
          type: 'multi',
          items: [
            { type: 'text', text: 'first' },
            { type: 'file', path: '/tmp/second.txt' },
          ],
        },
        'text'
      )
    ).toBe('first\n/tmp/second.txt')
  })

  test('uses stdio overrides for text output', () => {
    const result = {
      type: 'json',
      value: { result: { count: 2 } },
      stdio: '2 matches',
    }

    expect(formatResult(result, 'stdio')).toBe('2 matches')
    expect(formatResult(result, 'text')).toBe('2 matches')
  })

  test('preserves the full result for JSON output', () => {
    const result = {
      type: 'text',
      text: 'hello',
      stdio: 'override',
    }

    expect(formatResult(result, 'json')).toBe(JSON.stringify(result, null, 2))
  })
})

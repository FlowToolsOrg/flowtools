import { expect, test } from 'bun:test'

import {
  defaultLauncherPreferences,
  parseLauncherPreferences,
  recordLauncherOpen,
} from '../src/settings/launcher-preferences'

test('history stores bounded IDs/counts and opt-out retains no history', () => {
  let preferences = defaultLauncherPreferences()
  for (let i = 0; i < 300; i++)
    preferences = recordLauncherOpen(preferences, `p:${i}`)
  expect(preferences.recentIds).toHaveLength(40)
  expect(Object.keys(preferences.frequencies)).toHaveLength(256)
  expect(parseLauncherPreferences(JSON.stringify(preferences))).toEqual(
    preferences
  )
  const disabled = {
    ...preferences,
    historyEnabled: false,
    recentIds: [],
    frequencies: {},
  }
  expect(recordLauncherOpen(disabled, 'private-query-never-stored')).toBe(
    disabled
  )
  expect(JSON.stringify(disabled)).not.toContain('private-query')
})
test('bad versions, excessive fields, content and inconsistent privacy records refuse', () => {
  const value = defaultLauncherPreferences()
  for (const extra of [
    { formatVersion: 2 },
    { query: 'private' },
    { recentIds: Array(41).fill('same') },
    { historyEnabled: false, recentIds: ['x'] },
    { aliases: { x: Array(9).fill('alias') } },
    { frequencies: { x: -1 } },
  ])
    expect(() =>
      parseLauncherPreferences(JSON.stringify({ ...value, ...extra }))
    ).toThrow()
  expect(() => parseLauncherPreferences('x'.repeat(65537))).toThrow()
  expect(recordLauncherOpen(value, '__proto__').frequencies.__proto__).toBe(1)
})

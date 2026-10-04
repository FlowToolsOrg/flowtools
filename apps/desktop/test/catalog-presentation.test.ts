import { expect, test } from 'bun:test'

import {
  catalogPresentation,
  permissionPresentation,
  type CatalogStage,
} from '../src/runtime/catalog-presentation'

test('saved enabled HTML metadata never implies an install or execution grant', () => {
  for (const stage of [
    'available',
    'downloaded',
    'installed',
    'enabled',
  ] as CatalogStage[]) {
    for (const hasUi of [true, false])
      expect(catalogPresentation('html', stage, hasUi)).toMatchObject({
        primaryLabel: '不可安装',
        canExecute: false,
      })
  }
})

test('built-in actions describe configuration and preserve panel access', () => {
  expect(catalogPresentation('react', 'available', true).primaryLabel).toBe(
    '保存配置'
  )
  expect(catalogPresentation('react', 'enabled', true).primaryLabel).toBe(
    '打开'
  )
  expect(catalogPresentation('react', 'installed', false).primaryLabel).toBe(
    '启用配置'
  )
  expect(
    permissionPresentation.filter(row => row[2] === '授权未实现')
  ).toHaveLength(4)
  expect(permissionPresentation.find(row => row[0] === 'preload')?.[2]).toBe(
    '默认拒绝'
  )
})

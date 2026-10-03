/// <reference types="bun-types" />

import { expect, test } from 'bun:test'

import { REQUIRED_DOCUMENTS, validateDocuments } from './docs-check'

function fixture() {
  const documents = new Map<string, string>(
    REQUIRED_DOCUMENTS.map(path => [path, '# Fixture\n'])
  )
  documents.set(
    'docs/adr/0001-plugin-trust-boundaries.md',
    'accepted-design T0 T1 T2 T3 TL\n## 决策\n## 当前实现\n## 后续验证\n'
  )
  documents.set(
    'docs/adr/0002-capability-and-package-policy.md',
    'accepted-design\n## 决策\n## 当前实现\n## 后续验证\n'
  )
  documents.set(
    'docs/security/threat-model.md',
    Array.from({ length: 12 }, (_, index) => {
      const id = `SEC-${String(index + 1).padStart(3, '0')}`
      return `### ${id} Fixture
- 入口：untrusted input
- 现状：open prototype gap
- 证据：[source](../../src/entry.ts)
- Owner：Rust / Security
- 缓解：deny before execution
- 验证：P2.7 malicious fixture
- 残余风险：upstream compromise
`
    }).join('\n')
  )
  documents.set(
    '.github/pull_request_template.md',
    '## 安全评审\n威胁 ID\n安全 Reviewer\n身份与 scope\n拒绝路径\n撤销与恢复\n残余风险\n'
  )
  const exists = (path: string) =>
    documents.has(path) || path === 'src/entry.ts'
  return { documents, exists }
}

test('accepts complete design contracts and source evidence', () => {
  const { documents, exists } = fixture()
  expect(validateDocuments(documents, exists)).toEqual([])
})

test('rejects missing required documents and removed or duplicate threat IDs', () => {
  for (const path of REQUIRED_DOCUMENTS) {
    const { documents, exists } = fixture()
    documents.delete(path)
    expect(validateDocuments(documents, exists).length).toBeGreaterThan(0)
  }
  for (const replacement of ['### SEC-002', '### removed']) {
    const { documents, exists } = fixture()
    const path = 'docs/security/threat-model.md'
    documents.set(
      path,
      documents.get(path)!.replace('### SEC-001', replacement)
    )
    expect(validateDocuments(documents, exists).length).toBeGreaterThan(0)
  }
})

test('rejects each missing, empty or placeholder threat field', () => {
  for (const field of [
    '入口',
    '现状',
    '证据',
    'Owner',
    '缓解',
    '验证',
    '残余风险',
  ]) {
    for (const replacement of ['', `- ${field}：   `, `- ${field}：TBD`]) {
      const { documents, exists } = fixture()
      const path = 'docs/security/threat-model.md'
      documents.set(
        path,
        documents
          .get(path)!
          .replace(new RegExp(`^- ${field}：.*$`, 'm'), replacement)
      )
      expect(validateDocuments(documents, exists).length).toBeGreaterThan(0)
    }
  }
})

test('rejects broken links, encoded escapes, absolute paths and missing source evidence', () => {
  for (const target of [
    './missing.md',
    '%ZZ',
    '../%2e%2e/secrets',
    'D:/private/file',
    '/private/file',
    'file:///private/file',
  ]) {
    const { documents, exists } = fixture()
    documents.set('README.md', `[broken](${target})`)
    expect(validateDocuments(documents, exists).length).toBeGreaterThan(0)
  }
  const { documents, exists } = fixture()
  const path = 'docs/security/threat-model.md'
  documents.set(
    path,
    documents
      .get(path)!
      .replace(
        '[source](../../src/entry.ts)',
        '[design](../adr/0001-plugin-trust-boundaries.md)'
      )
  )
  expect(validateDocuments(documents, exists).length).toBeGreaterThan(0)
})

test('ignores examples and external links while checking angle and encoded local links', () => {
  const { documents, exists } = fixture()
  documents.set(
    'README.md',
    '```md\n[example](missing.md)\n```\n~~~md\n[example](missing.md)\n~~~\n' +
      '[web](https://example.invalid/) [mail](mailto:security@example.invalid)\n' +
      '[local](<src/entry.ts>) [encoded](src/%65ntry.ts#symbol)'
  )
  expect(validateDocuments(documents, exists)).toEqual([])
})

test('rejects ambiguous ADR status, missing trust levels and missing security review fields', () => {
  for (const text of [
    'accepted-design T0 T1 T2 TL\n## 决策\n## 当前实现\n## 后续验证',
    'production T0 T1 T2 T3 TL\n## 决策\n## 当前实现\n## 后续验证',
    'accepted-design T0 T1 T2 T3 TL\n## 决策\n## 后续验证',
  ]) {
    const { documents, exists } = fixture()
    documents.set('docs/adr/0001-plugin-trust-boundaries.md', text)
    expect(validateDocuments(documents, exists).length).toBeGreaterThan(0)
  }
  for (const field of [
    '安全评审',
    '威胁 ID',
    '安全 Reviewer',
    '身份与 scope',
    '拒绝路径',
    '撤销与恢复',
    '残余风险',
  ]) {
    const { documents, exists } = fixture()
    const path = '.github/pull_request_template.md'
    documents.set(path, documents.get(path)!.replace(field, 'removed'))
    expect(validateDocuments(documents, exists).length).toBeGreaterThan(0)
  }
})

test('validates the actual repository contracts', async () => {
  const { checkRepositoryDocuments } = await import('./docs-check')
  expect(checkRepositoryDocuments()).toEqual([])
})

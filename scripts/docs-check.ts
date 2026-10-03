/// <reference types="bun-types" />

import { existsSync, readFileSync } from 'node:fs'
import { posix, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

export const REQUIRED_DOCUMENTS = [
  'README.md',
  'AGENTS.md',
  'architecture.md',
  'docs/structure.md',
  'docs/plugin.md',
  'docs/production-roadmap.md',
  'docs/adr/0001-plugin-trust-boundaries.md',
  'docs/adr/0002-capability-and-package-policy.md',
  'docs/security/threat-model.md',
  '.github/pull_request_template.md',
] as const

const threatFields = [
  '入口',
  '现状',
  '证据',
  'Owner',
  '缓解',
  '验证',
  '残余风险',
]
const reviewFields = [
  '安全评审',
  '威胁 ID',
  '安全 Reviewer',
  '身份与 scope',
  '拒绝路径',
  '撤销与恢复',
  '残余风险',
]

/** This checks document structure and local paths, not runtime security or URLs. */
function withoutExamples(text: string): string {
  let fence: { character: string; length: number } | undefined
  return text
    .replace(/<!--[\s\S]*?-->/g, '')
    .split(/\r?\n/)
    .map(line => {
      const marker = /^\s{0,3}(`{3,}|~{3,})/.exec(line)?.[1]
      if (fence) {
        if (
          marker?.[0] === fence.character &&
          marker.length >= fence.length &&
          line.trim() === marker
        ) {
          fence = undefined
        }
        return ''
      }
      if (marker) {
        fence = { character: marker[0]!, length: marker.length }
        return ''
      }
      return line.replace(/`+[^`\n]*`+/g, '')
    })
    .join('\n')
}

function linkTargets(text: string): string[] {
  return Array.from(
    text.matchAll(
      /!?\[[^\]\n]*\]\(\s*(?:<([^>\n]+)>|([^\s)]+))(?:\s+["'][^"']*["'])?\s*\)/g
    ),
    match => match[1] ?? match[2]!
  )
}

function localTarget(document: string, target: string): string | undefined {
  if (/^(?:https?:|mailto:|#)/i.test(target)) return undefined
  const path = decodeURIComponent(target.split(/[?#]/)[0]!).replaceAll(
    '\\',
    '/'
  )
  if (/^(?:\/|[a-z][a-z\d+.-]*:)/i.test(path)) {
    throw new Error('absolute paths and unsupported schemes are not allowed')
  }
  const result = posix.normalize(posix.join(posix.dirname(document), path))
  if (result === '..' || result.startsWith('../')) {
    throw new Error('local link escapes the repository')
  }
  return result
}

export function validateDocuments(
  documents: ReadonlyMap<string, string>,
  targetExists: (path: string) => boolean
): string[] {
  const errors: string[] = []
  const texts = new Map<string, string>()
  for (const path of REQUIRED_DOCUMENTS) {
    const text = documents.get(path)
    if (!text?.trim()) {
      errors.push(`${path}: required document is missing or empty`)
      continue
    }
    const content = withoutExamples(text)
    texts.set(path, content)
    for (const target of linkTargets(content)) {
      try {
        const local = localTarget(path, target)
        if (local && !targetExists(local)) {
          errors.push(`${path}: missing local link target ${target}`)
        }
      } catch (error) {
        errors.push(`${path}: invalid link ${target}: ${String(error)}`)
      }
    }
  }

  for (const path of REQUIRED_DOCUMENTS.filter(path =>
    path.startsWith('docs/adr/')
  )) {
    const text = texts.get(path) ?? ''
    for (const required of [
      'accepted-design',
      '## 决策',
      '## 当前实现',
      '## 后续验证',
    ]) {
      if (!text.includes(required)) errors.push(`${path}: missing ${required}`)
    }
  }
  const trust = texts.get('docs/adr/0001-plugin-trust-boundaries.md') ?? ''
  for (const level of ['T0', 'T1', 'T2', 'T3', 'TL']) {
    if (!new RegExp(`\\b${level}\\b`).test(trust)) {
      errors.push(`trust ADR: missing ${level} boundary`)
    }
  }

  const threatPath = 'docs/security/threat-model.md'
  const threatText = texts.get(threatPath) ?? ''
  const threats = Array.from(threatText.matchAll(/^### (SEC-\d{3})\b[^\n]*$/gm))
  const ids = new Set<string>()
  for (const [index, threat] of threats.entries()) {
    const id = threat[1]!
    if (ids.has(id)) errors.push(`${threatPath}: duplicate ${id}`)
    ids.add(id)
    const section = threatText.slice(threat.index, threats[index + 1]?.index)
    for (const field of threatFields) {
      const value = new RegExp(`^- ${field}：[ \\t]*(.*)$`, 'm')
        .exec(section)?.[1]
        ?.trim()
      if (
        !value ||
        /^(?:TBD|TODO|待定|待补充|none|n\/a)(?:\s|$)/i.test(value)
      ) {
        errors.push(`${threatPath}: ${id} missing meaningful ${field}`)
      }
      if (field === '证据') {
        const hasSource = linkTargets(value ?? '').some(target => {
          try {
            const local = localTarget(threatPath, target)
            return (
              local !== undefined &&
              /\.(?:[cm]?[jt]sx?|rs|json|ya?ml|ps1|toml)$/.test(local) &&
              targetExists(local)
            )
          } catch {
            return false
          }
        })
        if (!hasSource)
          errors.push(`${threatPath}: ${id} missing source evidence`)
      }
    }
  }
  for (let index = 1; index <= 12; index++) {
    const id = `SEC-${String(index).padStart(3, '0')}`
    if (!ids.has(id))
      errors.push(`${threatPath}: required threat ${id} is missing`)
  }

  const template = texts.get('.github/pull_request_template.md') ?? ''
  for (const field of reviewFields) {
    if (!template.includes(field)) errors.push(`PR template: missing ${field}`)
  }
  return errors
}

export function checkRepositoryDocuments(): string[] {
  const root = fileURLToPath(new URL('../', import.meta.url))
  const documents = new Map<string, string>()
  for (const path of REQUIRED_DOCUMENTS) {
    const absolute = resolve(root, path)
    if (existsSync(absolute))
      documents.set(path, readFileSync(absolute, 'utf8'))
  }
  return validateDocuments(documents, path => existsSync(resolve(root, path)))
}

if (import.meta.main) {
  const errors = checkRepositoryDocuments()
  if (errors.length) {
    process.stderr.write(`${errors.join('\n')}\n`)
    process.exitCode = 1
  } else {
    process.stdout.write(
      `Documentation contracts passed: ${REQUIRED_DOCUMENTS.length} documents, 12 required threat IDs\n`
    )
  }
}

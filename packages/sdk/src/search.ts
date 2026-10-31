import { pinyin } from 'pinyin-pro'
import { RE2JS } from 're2js'

export interface SearchDocument {
  id: string
  pluginId: string
  title: string
  description?: string
  keywords?: readonly string[]
  aliases?: readonly string[]
}
export type SearchReason =
  | 'exact'
  | 'prefix'
  | 'title'
  | 'alias'
  | 'keyword'
  | 'description'
  | 'pinyin'
  | 'initials'
  | 'fuzzy'
  | 'regex'
  | 'pinned'
  | 'frequency'
  | 'recent'
export interface SearchMatch {
  id: string
  score: number
  reasons: SearchReason[]
}
export interface SearchOptions {
  mode?: 'text' | 'regex'
  limit?: number
  pinned?: ReadonlySet<string>
  frequency?: ReadonlyMap<string, number>
  recent?: readonly string[]
}
interface Indexed {
  id: string
  pluginId: string
  title: string
  aliases: string[]
  keywords: string
  description: string
  full: string
  pinyin: string
  initials: string
}
export class SearchError extends Error {
  constructor(
    readonly code: 'SEARCH_LIMIT' | 'INVALID_PATTERN' | 'SEARCH_BUDGET'
  ) {
    super(code)
  }
}
const normalize = (value: string) =>
  value.normalize('NFKC').toLowerCase().trim()
const compare = (a: SearchMatch, b: SearchMatch) =>
  b.score - a.score || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0)
function subsequence(text: string, query: string): boolean {
  if (query.length < 2 || query.length > 64) return false
  let cursor = 0
  for (const c of text) {
    if (c.codePointAt(0) === query.codePointAt(cursor)) cursor += c.length
    if (cursor === query.length) return true
  }
  return false
}
function match(
  item: Indexed,
  query: string
): [number, SearchReason] | undefined {
  if (item.title === query) return [1000, 'exact']
  if (item.title.startsWith(query)) return [800, 'prefix']
  if (item.aliases.some(alias => alias === query || alias.startsWith(query)))
    return [700, 'alias']
  if (item.title.includes(query)) return [650, 'title']
  if (item.aliases.some(alias => alias.includes(query))) return [600, 'alias']
  if (item.keywords.includes(query)) return [450, 'keyword']
  if (item.description.includes(query)) return [480, 'description']
  if (item.pinyin.includes(query)) return [350, 'pinyin']
  if (item.initials.startsWith(query)) return [300, 'initials']
  if (subsequence(item.title, query) || subsequence(item.pinyin, query))
    return [200, 'fuzzy']
  if (item.pluginId.toLowerCase().includes(query)) return [100, 'keyword']
  return undefined
}

/** Inert metadata only. Registration/removal must follow Host lifecycle. */
export class CommandSearchIndex {
  private items = new Map<string, Indexed>()
  get size() {
    return this.items.size
  }
  upsert(document: SearchDocument): void {
    if (
      !document.id ||
      document.id.length > 256 ||
      !document.pluginId ||
      document.pluginId.length > 128 ||
      !document.title ||
      document.title.length > 256 ||
      (document.description?.length ?? 0) > 1024 ||
      (document.aliases?.length ?? 0) > 8 ||
      (document.keywords?.length ?? 0) > 32 ||
      [...(document.aliases ?? []), ...(document.keywords ?? [])].some(
        v => v.length > 128
      ) ||
      (!this.items.has(document.id) && this.items.size >= 10_000)
    )
      throw new SearchError('SEARCH_LIMIT')
    const title = normalize(document.title)
    const aliases = (document.aliases ?? []).map(normalize)
    const keywords = normalize((document.keywords ?? []).join(' '))
    const description = normalize(document.description ?? '')
    const full = [
      title,
      ...aliases,
      keywords,
      description,
      normalize(document.pluginId),
    ].join('\n')
    if (full.length > 4096) throw new SearchError('SEARCH_LIMIT')
    const chinese = /[\u3400-\u9fff]/.test(title)
    const syllables = chinese
      ? pinyin(title, { toneType: 'none', type: 'array' })
      : []
    this.items.set(document.id, {
      id: document.id,
      pluginId: document.pluginId,
      title,
      aliases,
      keywords,
      description,
      full,
      pinyin: syllables.join('').replace(/\s+/g, ''),
      initials: syllables.map(part => part[0] ?? '').join(''),
    })
  }
  remove(id: string): void {
    this.items.delete(id)
  }
  removePlugin(pluginId: string): void {
    for (const [id, item] of this.items)
      if (item.pluginId === pluginId) this.items.delete(id)
  }
  search(value: string, options: SearchOptions = {}): SearchMatch[] {
    if (value.length > 128) throw new SearchError('SEARCH_LIMIT')
    const query = normalize(value)
    const limit = Math.max(1, Math.min(100, Math.trunc(options.limit ?? 50)))
    if (!Number.isFinite(limit)) throw new SearchError('SEARCH_LIMIT')
    const started = performance.now()
    let expression: RE2JS | undefined
    if (options.mode === 'regex' && query) {
      // Reject counted repetition before compilation; bound both compiled cost
      // and total scan work. User patterns never enter native JS RegExp.
      if (value.includes('{') || value.includes('}'))
        throw new SearchError('INVALID_PATTERN')
      try {
        expression = RE2JS.compile(
          value,
          RE2JS.CASE_INSENSITIVE | RE2JS.DISABLE_UNICODE_GROUPS
        )
        if (expression.programSize() > 512) throw new Error('cost')
      } catch {
        throw new SearchError('INVALID_PATTERN')
      }
    }
    const terms = query.split(/\s+/).filter(Boolean)
    if (terms.length > 8) throw new SearchError('SEARCH_LIMIT')
    const recent = new Map(
      (options.recent ?? []).slice(0, 40).map((id, i) => [id, 40 - i])
    )
    const results: SearchMatch[] = []
    let scanned = 0
    for (const item of this.items.values()) {
      if ((scanned++ & 63) === 0 && performance.now() - started > 35)
        throw new SearchError('SEARCH_BUDGET')
      const reasons: SearchReason[] = []
      let score = 0
      if (expression) {
        if (!expression.matcher(item.full).find()) continue
        score = 500
        reasons.push('regex')
      } else {
        let all = true
        for (const term of terms) {
          const found = match(item, term)
          if (!found) {
            all = false
            break
          }
          score += found[0]
          reasons.push(found[1])
        }
        if (!all) continue
      }
      if (options.pinned?.has(item.id)) {
        score += 60
        reasons.push('pinned')
      }
      const frequency = options.frequency?.get(item.id) ?? 0
      if (Number.isSafeInteger(frequency) && frequency > 0) {
        score += Math.min(30, Math.log2(frequency + 1) * 3)
        reasons.push('frequency')
      }
      if (recent.has(item.id)) {
        score += (recent.get(item.id) ?? 0) / 10
        reasons.push('recent')
      }
      results.push({ id: item.id, score, reasons: [...new Set(reasons)] })
    }
    results.sort(compare)
    return results.slice(0, limit)
  }
}

import { useCallback, useMemo, useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import { Button, Card, Chip, Label, TextArea } from '@flowtools/ui/plugin'

import commandPlugin, { type MatchResult } from './commands'

export default definePlugin({
  ...commandPlugin,
  setup() {
    return function RegexTesterPanel() {
      const { clipboard } = useCapability()
      const [pattern, setPattern] = useState('')
      const [flags, setFlags] = useState('g')
      const [testString, setTestString] = useState('')
      const [error, setError] = useState<string | null>(null)
      const [copied, setCopied] = useState(false)
      const results = useMemo(() => {
        if (!pattern || !testString) return null
        try {
          const regex = new RegExp(pattern, flags)
          const matches: MatchResult[] = []
          if (flags.includes('g')) {
            let match
            while ((match = regex.exec(testString)) !== null) {
              matches.push({
                match: match[0],
                index: match.index,
                groups: match.groups ? { ...match.groups } : undefined,
              })
              if (!match[0]) break
            }
          } else {
            const match = regex.exec(testString)
            if (match) {
              matches.push({
                match: match[0],
                index: match.index,
                groups: match.groups ? { ...match.groups } : undefined,
              })
            }
          }
          setError(null)
          return matches
        } catch (e) {
          setError(e instanceof Error ? e.message : '正则表达式无效')
          return null
        }
      }, [pattern, flags, testString])
      const highlightedText = useMemo(() => {
        if (!results || results.length === 0) return testString
        const parts: Array<{
          text: string
          isMatch: boolean
        }> = []
        let lastIndex = 0
        for (const result of results) {
          if (result.index > lastIndex) {
            parts.push({
              text: testString.slice(lastIndex, result.index),
              isMatch: false,
            })
          }
          parts.push({
            text: result.match,
            isMatch: true,
          })
          lastIndex = result.index + result.match.length
        }
        if (lastIndex < testString.length) {
          parts.push({
            text: testString.slice(lastIndex),
            isMatch: false,
          })
        }
        return parts
      }, [testString, results])
      const toggleFlag = useCallback((flag: string) => {
        setFlags(prev =>
          prev.includes(flag) ? prev.replace(flag, '') : prev + flag
        )
      }, [])
      const copyMatches = useCallback(async () => {
        if (!results || results.length === 0) return
        const text = results.map(r => r.match).join('\n')
        await clipboard.writeText(text)
        setCopied(true)
        setTimeout(() => setCopied(false), 1500)
      }, [clipboard, results])
      const commonPatterns = [
        { name: '邮箱', pattern: '[\\w.-]+@[\\w.-]+\\.\\w+' },
        { name: '手机号', pattern: '1[3-9]\\d{9}' },
        { name: 'URL', pattern: 'https?://[^\\s]+' },
        {
          name: 'IP 地址',
          pattern: '\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}',
        },
        { name: '日期', pattern: '\\d{4}[-/]\\d{1,2}[-/]\\d{1,2}' },
        { name: '中文', pattern: '[\\u4e00-\\u9fa5]+' },
      ]
      return (
        <div className="w-full max-w-4xl mx-auto">
          <Card className="p-6">
            <h2 className="text-lg font-semibold mb-4">正则表达式测试</h2>

            <div className="mb-4">
              <Label>正则表达式</Label>
              <div className="flex gap-2">
                <div className="flex-1 flex items-center border rounded-lg px-3 py-2 bg-gray-50">
                  <span className="text-gray-400 mr-1">/</span>
                  <input
                    className="flex-1 bg-transparent outline-none font-mono"
                    value={pattern}
                    onChange={e => setPattern(e.target.value)}
                    placeholder="输入正则表达式"
                  />
                  <span className="text-gray-400 ml-1">/</span>
                  <span className="ml-1 font-mono text-primary">{flags}</span>
                </div>
              </div>
              <div className="flex gap-2 mt-2">
                {['g', 'i', 'm', 's'].map(flag => (
                  <label
                    key={flag}
                    className="flex items-center gap-1 cursor-pointer"
                  >
                    <input
                      type="checkbox"
                      checked={flags.includes(flag)}
                      onChange={() => toggleFlag(flag)}
                      className="rounded"
                    />
                    <span className="text-sm font-mono">{flag}</span>
                  </label>
                ))}
              </div>
              {error && <div className="mt-2 text-sm text-danger">{error}</div>}
            </div>

            <div className="mb-4">
              <Label>常用正则</Label>
              <div className="flex flex-wrap gap-2 mt-1">
                {commonPatterns.map(p => (
                  <Button
                    key={p.name}
                    size="sm"
                    variant="ghost"
                    onPress={() => setPattern(p.pattern)}
                  >
                    {p.name}
                  </Button>
                ))}
              </div>
            </div>

            <div className="mb-4">
              <Label>测试文本</Label>
              <TextArea
                placeholder="输入要测试的文本"
                value={testString}
                onChange={e => setTestString(e.target.value)}
                rows={6}
              />
            </div>

            {results && testString && (
              <div className="mb-4">
                <Label>匹配预览</Label>
                <div className="p-3 rounded-lg border bg-gray-50 font-mono text-sm whitespace-pre-wrap break-all">
                  {typeof highlightedText === 'string'
                    ? highlightedText
                    : highlightedText.map((part, i) =>
                        part.isMatch ? (
                          <mark
                            key={i}
                            className="bg-yellow-200 px-0.5 rounded"
                          >
                            {part.text}
                          </mark>
                        ) : (
                          <span key={i}>{part.text}</span>
                        )
                      )}
                </div>
              </div>
            )}

            {results && results.length > 0 && (
              <div>
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <Label>匹配结果</Label>
                    <Chip size="sm" variant="soft">
                      {results.length} 个匹配
                    </Chip>
                  </div>
                  <Button size="sm" variant="ghost" onPress={copyMatches}>
                    {copied ? '已复制!' : '复制全部'}
                  </Button>
                </div>
                <div className="space-y-2 max-h-60 overflow-y-auto">
                  {results.map((result, i) => (
                    <div
                      key={i}
                      className="flex items-start gap-3 p-2 rounded border text-sm"
                    >
                      <span className="text-gray-400 font-mono w-8 text-right">
                        {i + 1}
                      </span>
                      <div className="flex-1">
                        <div className="font-mono">
                          &quot;{result.match}&quot;
                        </div>
                        <div className="text-xs text-gray-500">
                          位置: {result.index} -{' '}
                          {result.index + result.match.length}
                        </div>
                        {result.groups && (
                          <div className="text-xs text-gray-500 mt-1">
                            命名组:{' '}
                            {Object.entries(result.groups)
                              .map(([k, v]) => `${k}="${v}"`)
                              .join(', ')}
                          </div>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {results && results.length === 0 && testString && (
              <div className="text-center py-4 text-gray-500">没有找到匹配</div>
            )}
          </Card>
        </div>
      )
    }
  },
})

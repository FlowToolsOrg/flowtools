import { useCallback, useEffect, useState } from 'react'

import { definePlugin, useCapability } from '@flowtools/sdk'
import {
  Button,
  Card,
  Chip,
  Input,
  Label,
  TextField,
} from '@flowtools/ui/plugin'

import commandPlugin, {
  type TimestampUnit,
  formatDate,
  parseDateInput,
} from './commands'

export default definePlugin({
  ...commandPlugin,
  setup() {
    return function TimestampConverterPanel() {
      const { clipboard } = useCapability()
      const [timestamp, setTimestamp] = useState('')
      const [dateInput, setDateInput] = useState('')
      const [unit, setUnit] = useState<TimestampUnit>('seconds')
      const [currentTimestamp, setCurrentTimestamp] = useState(0)
      const [copiedField, setCopiedField] = useState<string | null>(null)
      useEffect(() => {
        const update = () => setCurrentTimestamp(Date.now())
        update()
        const timer = setInterval(update, 1000)
        return () => clearInterval(timer)
      }, [])
      const copyToClipboard = useCallback(
        async (text: string, field: string) => {
          await clipboard.writeText(text)
          setCopiedField(field)
          setTimeout(() => setCopiedField(null), 1500)
        },
        [clipboard]
      )
      const timestampToDate = timestamp
        ? formatDate(Number(timestamp), unit)
        : ''
      const dateToTimestamp = dateInput
        ? (() => {
            const ms = parseDateInput(dateInput)
            if (ms === null) return '无效日期'
            return unit === 'seconds'
              ? Math.floor(ms / 1000).toString()
              : ms.toString()
          })()
        : ''
      const useCurrentTimestamp = useCallback(() => {
        setTimestamp(
          unit === 'seconds'
            ? Math.floor(Date.now() / 1000).toString()
            : Date.now().toString()
        )
      }, [unit])
      return (
        <div className="w-full max-w-2xl mx-auto">
          <Card className="p-6">
            <h2 className="text-lg font-semibold mb-4">时间戳转换</h2>

            <div className="mb-4 p-3 rounded-lg bg-gray-50">
              <div className="text-sm text-gray-500">当前时间戳</div>
              <div className="font-mono text-lg">
                {unit === 'seconds'
                  ? Math.floor(currentTimestamp / 1000)
                  : currentTimestamp}
              </div>
            </div>

            <div className="flex gap-2 mb-4">
              <Button
                variant={unit === 'seconds' ? 'primary' : 'ghost'}
                onPress={() => setUnit('seconds')}
              >
                秒 (s)
              </Button>
              <Button
                variant={unit === 'milliseconds' ? 'primary' : 'ghost'}
                onPress={() => setUnit('milliseconds')}
              >
                毫秒 (ms)
              </Button>
            </div>

            <div className="space-y-4">
              <div>
                <Label>时间戳 → 日期</Label>
                <div className="flex gap-2">
                  <TextField
                    className="flex-1"
                    value={timestamp}
                    onChange={v => setTimestamp(v)}
                  >
                    <Input placeholder="输入时间戳" className="font-mono" />
                  </TextField>
                  <Button variant="ghost" onPress={useCurrentTimestamp}>
                    当前时间
                  </Button>
                </div>
                {timestamp && (
                  <div className="mt-2 flex items-center gap-2">
                    <Chip size="sm" variant="soft">
                      {timestampToDate}
                    </Chip>
                    <Button
                      size="sm"
                      variant="ghost"
                      onPress={() => copyToClipboard(timestampToDate, 'date')}
                    >
                      {copiedField === 'date' ? '已复制!' : '复制'}
                    </Button>
                  </div>
                )}
              </div>

              <div>
                <Label>日期 → 时间戳</Label>
                <div className="flex gap-2">
                  <TextField
                    className="flex-1"
                    value={dateInput}
                    onChange={v => setDateInput(v)}
                  >
                    <Input placeholder="2024-01-15 12:30:00 或 ISO 格式" />
                  </TextField>
                  <Button
                    variant="ghost"
                    onPress={() =>
                      setDateInput(new Date().toISOString().slice(0, 19))
                    }
                  >
                    当前时间
                  </Button>
                </div>
                {dateInput && dateToTimestamp && (
                  <div className="mt-2 flex items-center gap-2">
                    <Chip size="sm" variant="soft" className="font-mono">
                      {dateToTimestamp}
                    </Chip>
                    <Button
                      size="sm"
                      variant="ghost"
                      onPress={() =>
                        copyToClipboard(dateToTimestamp, 'timestamp')
                      }
                    >
                      {copiedField === 'timestamp' ? '已复制!' : '复制'}
                    </Button>
                  </div>
                )}
              </div>
            </div>
          </Card>
        </div>
      )
    }
  },
})

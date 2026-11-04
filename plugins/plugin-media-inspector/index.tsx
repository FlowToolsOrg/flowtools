import { definePlugin } from '@flowtools/sdk'

import command from './commands'
export default definePlugin({
  ...command,
  setup() {
    return function MediaPanel() {
      return (
        <div className="space-y-3 p-6">
          <h2 className="text-lg font-semibold">媒体信息</h2>
          <p>
            打开应用设置 → 媒体处理，选择文件并预览后执行。Windows/T1 原型支持 1
            MiB 以内的媒体；无需手动安装工具。
          </p>
          <p>
            工具、文件身份与本次授权逐阶段复核，输出采用全新文件名；取消和失败不会自动重放。
          </p>
        </div>
      )
    }
  },
})

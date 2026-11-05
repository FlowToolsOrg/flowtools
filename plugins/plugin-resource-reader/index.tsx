import { definePlugin } from '@flowtools/sdk'

import command from './commands'
export default definePlugin({
  ...command,
  setup() {
    return function ResourcePanel() {
      return (
        <div className="space-y-3 p-6">
          <h2 className="text-lg font-semibold">资源读取</h2>
          <p>打开应用设置 → 资源读取，选择文件或目录，确认本次权限后运行。</p>
          <p>
            每次选择只绑定一个命令，目录列表不会授权读取子文件内容。历史仅保存读取字节数或条目数。
          </p>
        </div>
      )
    }
  },
})

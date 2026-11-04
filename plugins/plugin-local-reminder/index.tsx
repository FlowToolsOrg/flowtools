import { definePlugin } from '@flowtools/sdk'

import command from './commands'

export default definePlugin({
  ...command,
  setup() {
    return function ReminderPanel() {
      return (
        <div className="p-6 space-y-3">
          <h2 className="text-lg font-semibold">本地提醒</h2>
          <p>
            打开应用设置 →
            后台命令调度，选择“本地提醒”，填写标题、内容与时间。可查看、编辑或取消有限次数的提醒。
          </p>
          <p>
            请先在共享 Runtime 中批准本地提醒的后台通知权限。关闭桌面后，需要
            Runtime 保持运行。系统接受发送请求不代表通知一定已显示。
          </p>
        </div>
      )
    }
  },
})

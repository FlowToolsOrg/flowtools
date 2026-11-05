# NA1：资源选择与读取

日期：2026-11-05；固定 Windows/x64/T1 原型。依赖 G8 #17 的
`0b99b4c52d482be77f8be048fe17b7f7e524e6cc`、NA0 #19 的
`2d539c994e23c1495fb793edf2ac64bbe34fc8e8`。核对时两者均 open/draft、mergeable，
Windows CI 成功：[G8 41841689327](https://github.com/FlowToolsOrg/flowtools/actions/runs/41841689327)、
[NA0 41988574389](https://github.com/FlowToolsOrg/flowtools/actions/runs/41988574389)。
依赖尚未合入 main，本分支叠加依赖；依赖提交不算 NA1 新实现。

## API 与权限

[SDK](../../packages/sdk/src/resources.ts) 的可选 `ctx.resources` 提供 resolve、stat、
分块 read、单层 list 和幂等 release。参数不含路径、身份、owner 或 grant。read
要求 revision/offset/maxBytes，返回 Uint8Array/nextOffset/eof；list 要求 revision/
cursor/limit。子条目只允许 stat，内容须另选；名称只作文本。React interactive
分支尚未注入，NA1 使用可信设置页选择，runner 无 UI 分支。

[Core](../../packages/runtime-core/src/runtime_resources.rs) 将选择 owner 绑定到当前
catalog/package/command/caller/epoch/实例，前台 submit 消费一次 owner，重新 mint
run refs。候选 Ref、跨 run、撤销和重启均拒绝。普通 jobs.submit 没有绑定时拒绝，
复制输入不绑定。grant、选择和执行绑定是独立管理步骤，旧 v1 grant 不升级。

[PolicyStore](../../packages/runtime-core/src/policy/resources.rs) 的独立 format-2
记录复用原 grants 表和 DataStore writer，SQLite schema 保持 2。有界 TEXT、精确
缓存前驱、affected/actual/readback 保护新行及与旧策略交互，撤销推进原 epoch。
旧 PermissionGrant/Scope/import、v1 行字节、405-case corpus/结构摘要均保留。
资源任务还复用原 jobs.metadata 精确前驱/事务 affected/actual/readback 门禁；
读取、取消和终态写入遇到行替换或 trigger 改写均拒绝，保留已持有输入并停止准入。

[NTFS](../../packages/runtime-core/src/file_workflows/resource_source.rs) 复用原 held
ancestor/relative open、NTFS、reparse/profile 检查。新读取仅 READ_DATA，不请求旧
rename 的 DELETE；share 拒绝 writer、替换和可写映射竞争。完整 SHA-256/64 KiB
块 pin 在每次读前后重验，返回块另对比块 pin。分页重验完整目录名字/身份/
metadata 快照；变化拒绝旧 cursor，不递归或隐式授予 child 内容。

[Runtime IO](../../apps/runtime/src/resource_io.rs) 在 Core 锁外执行，每次 native
读取回调与交付均重验当前授权。独立任务持有 blocking worker，信号/撤销/断开/
deadline/停止会取消 owned thread 的同步 IO，重复取消覆盖尚未发起 IO 的竞争。
JoinHandle 真完成后才清除 active，任务终态还等待原 Bun/Job 排空。
[CancelSynchronousIo](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelsynchronousio)
只请求取消，不能作为 drain。未知状态保留并停止 admission。

wire major 仍为 1、帧仍 1 MiB；client 0.5.0、private runner 3，旧 peer 在业务前拒绝。
新 DTO/schema/standalone validators 由 Rust 同源生成，原 LF/content drift、numeric
format/no-dynamic-code 门禁不变。strict JSON 仍拒绝任意深度重复键；取消帧计入
原 256 帧预算，取消中的半帧读取状态由持久 reader 保留。

## 使用与上限

桌面：应用设置 → 资源读取 → 选择文件/目录 → 原生确认命令和只读范围 → 运行。
新增 managed_resource_select 的显式 main/local AppManifest permission；保留原
Debug/main/dev-origin/profile 检查，不添加 default、remote、validation 权限或 CSP。
选择取消返回 null，确认取消不执行。路径不进入 renderer/plugin，界面显示名称/大小。

CLI 管理选择和执行分开：

```powershell
flowtools resources select 'D:\input\notes.txt' --approve-read --profile <initialized-profile>
flowtools resources select 'D:\input' --kind directory --approve-read --profile <initialized-profile>
flowtools resources run <returned-owner> --profile <initialized-profile>
flowtools resources release <unused-owner> --profile <initialized-profile>
```

select 默认使用 resource-reader，可用 --plugin/--command/--slot 指定本构建已声明的
只读资源命令；明确批准 target cli 的五分钟 grant，随后创建 owner/
选择；缺 approve-read 在 IO/启动前拒绝。run 不接受资源路径，前台等待结果；
SIGINT 走 jobs.cancel，连接关闭也取消。CLI/桌面只能使用各自 caller 的 owner。
原 15 个插件保留 v1；第 16 个固定 plugin-resource-reader 为 v2，只输出字节数/
条目数，不把名称/内容/Ref 写入历史。Web、无 adapter、后台、冷启动和服务委托拒绝。

选择文件要求 read、目录要求 list，只有 stat 的 grant 在 OS IO 前拒绝。
文件 64 MiB、chunk 64 KiB；每次读取仍消耗原 256 帧预算，并非承诺一个 run
能完整遍历 64 MiB。目录观察 512 条/128 文件，只列一层。页请求最多 64，
还取 run 32/profile 128 的剩余 Ref/handle/cursor 预算。drive/每层父目录、文件与
child 元数据句柄均计费；选择前预留受保护 profile 检查和 IO 临时句柄，因此深路径
可能更早触及上限。每次选择一个 slot/资源，
owner 最多 32/五分钟。并发 IO 最多 4，每 IO 五秒后请求取消，仍等待实际 drain。
释放 Ref 的 tombstone 在原 run 有界保留（最多 544）；目录父 Ref 释放后，活跃
children 仍计入原 observation 和共享父目录句柄预算。并发页预留预算；256 MiB observed 合并已有 file
plans 和保守 media input/output 预留，没有第二份资源贷款。原 Runtime tick 清理
失效 owner，先取消并等待 IO 排空。

## 验证记录

- Core resource 22 项通过（含 3 项原测试）：真实 NTFS junction/hardlink/替换、ADS/UNC/设备/profile、
  超限、可写 mapping 实际拒绝与排空后正对照；真实 pending ReadFile 取消及不关闭
  writer 的普通读取正对照；跨 caller/run/epoch/候选 Ref、IO 后撤销、取消终态延迟、
  目录变化/单次 cursor/配额/父引用预算、深路径与并发选择句柄预留、SQL trigger/
  前驱与旧字节、资源 jobs.metadata 替换/实际终态 trigger、metadata canary。
- Runtime client：生成 operation/response 拒绝注入，chunk/base64/offset 检查；
  abort 等 Host 完成再拒绝，不把取消请求当完成。
- 实际编译 CLI → Native Runtime → Bun reader 与 Desktop 协议：分别选文件/目录，
  manager/执行角色分离，无绑定提交拒绝，重启 owner 拒绝，文件/history canary 保持。
  使用 disposable profile。
- 真实 Windows 原生窗口（独立 Debug identity/profile/WebView）：取消 picker、拒绝
  权限确认均不提交；确认 29 字节 UTF-8 名称文件和一层目录后分别运行，两个任务
  实际 succeeded。停止 Runtime 后只读核对原 SQLite，路径/名称/内容 canary 不在
  历史，源文件字节不变。实际 UIA 可读按钮、disabled 与 status；完整读屏听测未做。
  [文件完成](assets/na1/file-complete.png)、[目录确认](assets/na1/directory-consent.png)、
  [目录已选择](assets/na1/directory-selected.png)、[目录完成](assets/na1/directory-complete.png)。
  该验证发现并修复 Desktop business 白名单漏接 resources.submit；管理选择/授权
  仍拒绝从 business transport 进入，原 native guard 测试保留。
- Pinned Chromium ResourceSettings 的选择取消、显式运行与卸载释放回归通过。
- 干净提交 `c65c695d93efeff14ce4e801354bfcfbf5827883` 完整执行原
  `pwsh -NoProfile -File scripts/check-ci.ps1`，exit 0，首尾 tracked/untracked
  均干净；lint/types/test/build 各 11/11 工作区成功。Core 547 + 11 + 13、
  Runtime 31 原生 + 6 隔离 + 19 集成、Desktop 121 Bun + 原 Node 两例 + 38
  原生 + 8 bindings、Chromium 38 文件/120 用例通过，未过滤或跳过原测试。
  Web 38/Desktop 34 个生产产物与 opt-in/canary 重建字节相同；Rust
  fmt/locked check/all-target Clippy 与最终生成漂移检查通过。
- 全历史脱敏扫描、Bun 全严重度审计和四份 Cargo 锁实时审计通过。首次联网审计
  registry 超时被门禁拒绝；清理新夹具类型/日志警告后从干净提交完整重跑。
  7 项既有 Rust informational warnings 保留，依赖锁、scanner pins/例外、
  原期限与工作流均未改；告警处置仍归 G8 独立审阅。远端同 SHA CI 随 PR 记录。

工程自查：Codex，2026-11-05。独立安全 Reviewer/date/conclusion pending。
非 Windows、第三方隔离、E07、write/create/export/opener/secrets/network、发行/
签名与 G8 批准仍 pending；G7 的旧人工 waived 不替代本轮验证。

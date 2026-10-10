# G4 P1.6b：服务调用链、租约与恢复

- 范围：固定 Windows/T1 inventory 与构建时完整 SDK 校验的可丢弃 A/B/C fixtures。
- 状态：P1.6b 实现已提交；G4 综合验收 pending，maturity 为 prototype。
- 独立提交：`feat(runtime): bind delegated service calls and drain providers (P1.6b)`。
- 威胁：SEC-014、SEC-015 保持 open；独立人工安全 reviewer/date/conclusion pending。

## 身份、数据与授权

根任务 accepted 时持久化 Host 生成的不可变 lock，取得所有 provider 版本引用。
幂等重试使用原 accepted lock，不因当前目录更新重新绑定。旧无依赖历史保留
`t1-no-dependencies-v1`；不把历史格式兼容当权限恢复。
Rust `prepare_service` 从父任务/运行 session 派生 root caller、直接 consumer、
provider 包摘要与 parentRunId。子进程只发送 publisher/id/service/operation 和
有限 JSON，不能选择版本、目录、路径、命令函数、grants 或调用者。

所有祖先声明与适用 grants、leaf operation 声明与 grant 共同约束实际效果。
每个 hop 保留 epoch、expiry、deadline、调用预算；调用和效果都消耗祖先剩余预算。
中间 grant 撤销也会终止实际 C。跨 provider 的 PluginData 作用域默认拒绝，
因为现有作用域表示自身 namespace，没有实现可委托 provider 私有数据协议。
原始路径、Host 对象、凭据和函数不是服务 selector；业务 JSON 内容本身仍需
插件正确处理，不能声称通用 JSON 会识别用户主动传入的敏感文本。

SDK `@flowtools/sdk/services` 执行实际 handler，验证输入/defaults、输出、大小、
取消与 timeout。Host 独立检查输出和实际预算。每根任务最多 64 次服务调用、
16 层绑定、累计输出不超过根预算及帧上限；每帧最多 1 MiB，每子进程最多
256 帧。失败记录稳定代码，不保存输入、输出、原始异常或本地路径。
服务和 RPC 消息有独立严格校验，固定 inventory 的编译服务入口也验证包字节。
导入入口前先核对完整 manifest 摘要与 Host 的 accepted package pin；匹配后才
验证包字节并导入。可丢弃编译入口的副作用标记与匹配摘要对照证明，摘要不匹配
时入口代码未执行。这不是签名验证，也不能防止可信 T1 文件被整体替换。

## 租约、更新与卸载

一个 profile 的同一 publisher/provider 仅有一个服务版本，实际运行按 provider
串行；根任务并发许可与 provider 许可独立，避免多层调用占满根许可后死锁。
排队调用和已接受根任务也保留版本引用；根任务取消不能提前释放仍在回收的 child。
每次调用结束关闭自己的 runner，Windows Job Object 终止并检查完整进程组为空，
随后才释放服务许可和 lease。普通 T1 Bun 仍能访问 OS；这属于生命周期收敛，
不构成权限沙箱或可运行不可信第三方代码的证据。

默认卸载有反向依赖的 provider 失败；级联停用要求确认完整、未过期的计划摘要。
更新先在 Host 内验证候选目录的 package/interface 约束与 DAG，再停止新任务准入，
等待已接受、排队和实际运行引用全部排空。忙碌时迁移回调不执行、旧版本不切换。
迁移前创建真实数据库备份，prepared journal 先落盘；迁移和 committed 状态在
同一 SQLite transaction 提交后切换内存目录。确认 rollback 后重新开放旧目录；
无法确定 commit/rollback 结果时停止准入。重启要求 journal 已完成且目录摘要匹配，
prepared 或新数据库配旧目录返回 RECOVERY_PENDING，使用已有离线备份恢复流程处理。
旧 lock、目录与备份保留；历史 provider pins 和诊断不改写为新版本。
未完成调用恢复为 interrupted，不重新使用旧 runner session。

本项只验证 T0 内部固定候选和 fixture 的事务边界，没有新增公开 apply/update/
install API。实际跨包文件安装、签名验证、目录持久切换和自动包回滚在 G5 验收。
接口版本满足声明不是对 handler 语义兼容的认证。当前十二个生产内置插件没有
服务定义；A/B/C fixture 不进入生产 inventory，也不打入独立发行包。
工具 pins 随 accepted lock 保留；真实工具仓库、磁盘 lease 和 GC 仍属 G6。

## 可查看结果与验证

Runtime client 和 CLI 只读查询，复用同一个 Host：

```text
flowtools dependencies unload-plan plugin-base64-encoder --profile <profile> --format json
flowtools dependencies calls <run-id> --profile <profile> --format json
```

计划和诊断不会删除包、停止任务或更改 grants；非法 selector 在 profile IO 前拒绝。
诊断含实际 provider 版本/摘要、consumer、root/parent IDs、lock、deadline 和稳定错误。
没有新增 GUI 流程；实际 CLI 在可丢弃 profile 中验证，已有 UI 自动回归已执行。

专项测试：

- 实际编译 A → B → C 返回、文件 handle 允许/拒绝、异常不泄漏、取消传递；
- 两个真实根任务并发激活时 provider 串行，取消一个不破坏另一个；
- 中间 grant 撤销、根调用预算不在下一跳重置、SDK 输入/输出和 timeout；
- 循环与接口不兼容候选在排空前拒绝，旧任务阻止更新/卸载；
- 排空后切到 1.1.0，旧 1.0.0 lock/诊断保留，迁移 SQL 失败实际 rollback；
- prepared/commit 失败恢复拒绝、卸载状态重启保留、目录摘要匹配；
- 完整 Windows 进程组回收，fixture 后代进程在服务结果交付前终止；
- 实际编译 runner 拒绝截断、超长和伪造 Host 帧；CLI/TS schema 拒绝注入；
- 实际编译服务入口在摘要不匹配时拒绝导入，匹配摘要对照执行副作用标记；
- Rust 生成 Runtime bindings/Schema/fixture，保留只读内容漂移检查。

取消/撤销集成用例等待 C 的实际 running 状态，使用 Host 已有的根任务截止时间，
每次等待检查任务仍有效；没有另设 5 秒冷启动 SLA、延长任务预算或重试失败。
服务 fixtures 串行创建 profile，各用例内部仍执行真实并发根任务；所有断言运行。

2026-10-09 最终验收：

- `docs:check`（11 份文档、12 个必需威胁 ID）、标准 workspace tasks、portable
  catalog、generated manifests 和 Rust-derived Runtime contracts 只读检查通过；
- 全仓 `lint` / `check-types` 均 11 个 workspace 强制执行成功；最后的原生测试
  同步调整另通过 Runtime lint、Rust fmt 和 all-targets clippy `-D warnings`；
- 全仓 `test` 11/11、零缓存通过（8m22.434s）：Core 73 项 Rust、Native 17 项
  Rust（含 9 项真实服务专项）、compiled runner 4 项、Chromium 26 文件 66 项；
  实际 CLI 9 项执行回归、持久管理/恢复和独立发行包 50 并发冷启动均通过；
  Desktop 87 项 JS、21 项 Rust 与 3 项原生 binding 测试通过；
- 测试结束后顺序执行全仓 `build`：11/11 强制构建通过（44.555s）；
- `verify:production-entrypoints`：Web 22 / Desktop 24 个产物在 child-only
  opt-in/canary 重建后 byte-identical，已知 unsafe/certification 开关与 canary
  未泄漏。这不等于签名、隔离或安全认证；
- Desktop 原生 Rust fmt/check/all-targets codegen clippy 已通过；未启动日常
  Desktop，也未访问用户数据库。没有新增视觉流程，因此没有新增人工 UI 验收。

首轮最终全仓测试暴露两项用例额外 5 秒 readiness 限制；修正为检查既有 Host
截止时间后，9 项专项和全部全仓测试重新通过，未降低断言、延长任务时限或
启用失败重试。
本机 Bun 1.4.2、Rust 1.99.0、Node 26.5.0；远端固定 Bun 1.3.14 /
Rust 1.96.0 CI 尚未执行。用户要求仅保留本地提交。
没有人工安全批准、生产认证或其他平台验收；G5/G6 仍按依赖顺序另行实施。

## 2026-10-09 主分支集成与交付复核

维护者后续明确授权推送，以上“仅保留本地提交”是此前交付状态。
P1.6a / P1.6b 提交现为 `c32adcb` / `ecf8812`，作者与提交者均为当前
GitHub 登录账号 `Getaway-Wuji`，使用其 GitHub noreply 邮箱；两条实现提交的
文件树与此前验收版本完全一致。集成主分支 `14a7e79` 时保留了 E01/E02 的
扩展和主题实现，以及 G4 的公开导出、构建入口和文档。

集成复核处理了以下测试问题，未降低断言、增加超时或启用失败重试：

- SDK 导出 fixture 在支持时使用 Node 同步 loader hooks，旧 Node 20 仍使用
  实际异步 loader；新增两项注册选择回归，保留 ESM/CJS 拒绝和空 stderr 断言。
- 旧远端 CI 的服务并发用例命中额外 5 秒排队就绪限制。改为 B 取得执行锁后
  启动第二个真实 A，使 B/C 与第二个 A 的启动重叠；保留 5 秒检查上限、
  9.5 秒 Host 预算及全部取消、串行和实际返回值断言，并检查两个根任务有效。
- 管理用例的子进程创建开销使完整正确流程超过 60 秒。现验证独立发行包采用的
  Node 编译 CLI 消费路径，保持原 60 秒限制、原环境和全部授权/撤销断言。
  这不证明 Bun 在本机的进程创建开销已修复。
- 独立发行包曾出现一次 `connect/pipe` 阶段失败，尚未确定唯一底层原因。
  增加 50 个启动结果的有限错误码、阶段和耗时诊断；并发数量、时限和所有
  断言不变。随后专项与全仓检查均通过，不能据此宣称底层连接问题已修复。

集成后的全仓 `test` 11/11、零缓存通过（7m39.383s）：SDK 257 项、CLI
100 项、Core 73 项 Rust、Native 17 项 Rust、Chromium 27 文件 72 项；
管理、恢复与独立发行包 50 并发冷启动完整通过；Desktop 88 项 JS、21 项
Rust 和 3 项原生 binding 测试通过。测试结束后顺序执行全仓 `build`，
11/11 强制构建通过（48.993s）。全仓 lint/types 与变更后的 Runtime、SDK
定向 lint/types、Rust fmt/all-targets clippy 均通过。
生产入口复核中 Web 22 / Desktop 24 个产物保持 byte-identical，未发现已知
unsafe/certification 开关或 canary 泄漏；portable catalog、generated manifests
及 Rust-derived Runtime contracts 的只读检查也通过。

失败路径仍依赖 Tokio 运行时关闭、`kill_on_drop` 和 Windows Job Object
析构终止子进程；正常路径有实际排空断言，但没有独立 fixture 级异常排空验收。
远端固定版本 CI 的当前结果以 PR Checks 为准；独立人工安全审查仍 pending，
SEC-014/SEC-015 保持 open，交付 PR 保持 Draft，maturity 仍为 prototype。

## 2026-10-10 CI 执行超时修复

提交 `2784c89` 的 push CI 成功，PR CI 失败。后者的两个错误分别出现在
并发测试的第二个实际调用返回，以及重启测试的前置 A → B → C 返回，均为
`Timeout`；5 秒就绪检查已通过，重启/快照断言尚未开始，不能归因于数据恢复。

每个 hop 启动前仍重新读取和验证完整 Bun 文件及 runner。受控性能对照对
同一 86,096,984 字节文件逐次读入、计算 SHA-256 并比对固定期望摘要：

| sha2 0.10.9 后端         | 原 Debug 单次哈希 | 仅 sha2 opt-level=3 |
| ------------------------ | ----------------- | ------------------- |
| 默认硬件后端             | 1.21–1.98 秒      | 34–40 毫秒          |
| 官方 force-soft 软件后端 | 8.77–9.90 秒      | 163–180 毫秒        |

12 次实际摘要比对均通过；没有缓存、size/mtime 替代或跳过校验。软件后端单次
校验就可能耗尽 9.5 秒根任务预算，证明原 Debug 热路径存在 CPU 相关开销；
远端失败日志没有分阶段计时，因此此对照不等于远端 CPU 型号或耗时的直接证据。
数值仅是本机对照，不是产品启动 SLA。

修复仅在 Runtime/Core 的根 Cargo workspace 为 `sha2` 设置开发包优化级别 3。
标准 test profile 继承 dev；应用自身仍用 Debug，调试/溢出检查保留。没有修改
实际执行、逐次完整性校验、权限、租约、进程组排空、任务期限或任何原有断言。
Desktop 是独立 workspace，本次不改变其编译配置。构建策略回归通过实际
Runtime manifest 的 Cargo metadata 定位有效 workspace，检查仅有该依赖优化，
防止放在无效的成员 manifest 或全局弱化调试检查。

固定 Bun 1.3.14 / Rust 1.96.0 的本地复核：

- 全仓 lint/types 均 11/11 强制执行通过；全仓 test 11/11、零缓存通过
  （4m58.669s），随后顺序 build 11/11 强制执行通过（45.902s）。
- Core 73 项、Native 17 项（22.16s）、Chromium 72 项、Desktop 89 项 JS、
  21 项 Rust 和 3 项 binding 均通过；独立发行包 50 并发冷启动通过。
- 单独启用官方 `sha2/force-soft` 的真实原生测试 17/17 通过（34.21s），
  包含两项此前失败用例；完整 CI contracts 12/12、102 项断言通过。
- 冻结依赖安装、workspace tasks、portable catalog、generated manifests、
  command docs 和 Rust-derived Runtime contracts 只读检查通过。
- Web 22 / Desktop 24 个生产产物在 opt-in/canary 重建后 byte-identical；
  Desktop Rust fmt/check/all-targets codegen clippy `-D warnings` 通过。

远端验收以 [PR #9 Checks](https://github.com/FlowToolsOrg/flowtools/pull/9/checks)
为准。本次仅修复 CI 开销，G4 综合验收与独立人工安全审查仍 pending，
不推进 G5、maturity 或第三方执行准入。

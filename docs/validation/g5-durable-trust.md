# G5 P2.5b 持久 descriptor 信任第三步

日期：2026-10-11。状态：prototype；P2.5b 整体和独立安全批准 pending。

## 实现与边界

`packages/runtime-core/src/packages/trust.rs` 的 T0 `PackageTrustStore` 借用
Runtime 现有 `DataStore` 单写者连接，没有第二个数据库、连接、目录、IPC
或客户端模式。schema v2 保持不变，状态保存在私有 `core_metadata` 的
`package-trust-state-v1`。`DescriptorVerification` 是借用 bytes 的 T0 Rust
参数，不是可反序列化的客户端请求。真实 production pins 尚未提供。

状态绑定经过验证并排序的 bootstrap pins/threshold，保存原始 DSSE 根链、
各根观察时间、最高观察时间和精确版本/release/ZIP digest。每次从 SQLite
加载并重新验签整个历史双签链、检查累计撤销与当前 pins；不缓存可变根或
floors。历史按其接受时间验证，当前包用最新根和当前 T0 时间检查，过期根
不能准入包。过期历史可认证当前有效的 successor，不能绕过双签/版本/hash
链接或删除撤销记录。

可信 T0 时间先独立提交，即使随后签名/过期校验拒绝；无效描述不记录版本。
成功描述的不可变版本与 release 高水位在 SQLite IMMEDIATE/FULL 事务中
提交，提交成功后才返回 opaque receipt。写失败和提交前退出不推进候选
floor；已提交后退出，重启仍拒绝旧 release。相同版本、sequence 和原始 ZIP
digest 可用新鲜描述续期，不重复占用记录。不同 hash/sequence 的同版本、
重复 sequence 与降级默认拒绝。没有持久回滚审批，不能用 run flag 放行。

观察 descriptor 不等于安装或 last-known-good；后续 ZIP/Manifest 校验拒绝
也不降低已经观察的 release floor。已有 read-only in-memory policy helpers
仍用于独立协议 fixtures，不代表客户端能提供持久 floors。安装必须继续
通过实际 ZIP、完整 Manifest、Host/SDK/target 和真实依赖锁门禁。

状态严格拒绝未知字段、重复原始 JSON 键、坏 pins/链/时间/版本/digest 和
重复版本/release 身份。读取前检查数据库值大小，16 MiB 上限；根链最多
32 项，版本观察最多 1024 项。耗尽拒绝且保留旧记录，不静默裁剪。
未来 checkpoint、受控信任恢复与回滚审批需要独立设计/验证。

## 备份与恢复

现有 managed 恢复 sanitizer 和 low-level `DataStore::restore_backup` 都在
替换数据库前写入 `package-trust-recovery-required`，包括没有 trust state 的
旧备份。adapter 也识别已有 `recovery-generation`；存在任一标记便返回
`PACKAGE_TRUST_RECOVERY_REQUIRED`，拒绝 snapshot/bootstrap/轮换/准入。
没有自动清除或重新 bootstrap 接口。T1 数据恢复的原有行为仍可使用。

正常同一数据库的崩溃/重启保证不等于任意磁盘回退保护。完全控制 OS/T0
的对手直接替换/删除 SQLite 不在威胁模型范围；没有独立防篡改单调锚点。
该原型不能宣称生产级备份信任恢复。安装 journal/取消/健康检查/LKG、真实
provider/tool locks、生产 pins 和平台 sandbox 继续 pending。

## 拒绝与崩溃回归

`packages/runtime-core/src/packages/tests/trust_tests.rs` 的 13 项测试覆盖：

- 实际单写者数据库 reopen 后 root/time/release floors、第二连接拒绝与
  重新 bootstrap 拒绝；没有新 store 或用户 profile。
- 同版本/sequence/hash 不可变、新鲜描述续期、实际 ZIP 新版本与降级拒绝。
- 两个 T0 adapter 不共享可变缓存；根轮换后即时/重启撤销与撤销删除拒绝。
- 过期/时间回退拒绝，历史认证当前有效双签 successor；无效签名只观察
  T0 时间，不生成 grants、jobs 或 plugin_data。
- pins 换绑、损坏/未知/重复/超大状态与配额耗尽拒绝，原记录不被重建/裁剪。
- SQLite trigger 注入候选写失败，没有返回 receipt 或推进候选 release；
  可信观察时间独立保留。
- 真实原生测试子进程在候选事务提交前和提交后 `process::exit(17)`；父进程
  reopen 核对时间和版本记录、实际旧描述是否拒绝。子进程仅使用独立 UUID
  fixture profile，控制环境变量只在测试 target。
- managed 旧备份早于初始化与 low-level 旧版本备份，均拒绝 bootstrap/准入。

测试仅使用公开 golden fixture keys、惰性签名 ZIP 和临时 UUID profile。
不读取用户数据、不执行包代码，不新增 production trust pins。

## 验证记录

针对性 13 项与 core 全部 127 项 Rust 回归、docs:check、rustfmt、all-target
clippy `-D warnings` 已通过。
完整 Windows gate 在本步提交后执行，最终记录随证据提交更新。
没有 UI/native command/Tauri capability/CSP 变更或新的人工 UI 流程。
SEC-001/008/014/015 保持 open；Codex 工程自查不替代独立实际 Reviewer、
日期和结论，独立安全批准 pending。T2/T3/TL、G5 整体和 G6 执行继续拒绝。

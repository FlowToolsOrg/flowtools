# G6 P2.8a2 惰性工具归档安装事务

日期：2026-10-29。Windows/x64 T0 prototype；本子步骤不产生可执行身份，
G6/G7 整体及独立安全审核保持 pending。

后续 P2.8a5 补充 [原始 ZIP 孤儿的显式恢复核对](./g6-tool-reconciliation.md)，
支持受控恢复后 plan/quarantine/resume/abort，仍保留字节和 trust floors。
这不恢复密封文件孤儿或生成 accepted leases/可执行身份。

`packages/runtime-core/src/packages/tools/install.rs` 提供独立的显式安装、
只读归档库存和显式 abort。调用方为已保留/保护 profile 的 T0 Rust Host，
仍只有同一个 DataStore writer，没有客户端路径、pins、target、policy flag、
argv 或新的 IPC。正常生产 review profiles 继续为空；公开 fixture profile
只在 cfg(test) 中可用，正常入口对有效签名 fixture 也在创建存储池前拒绝。

## 实际行为

工具归档库存使用 `tool-archive-inventory-v1` / pending metadata keys，记录
formatVersion 1、实际 canonical main DB 路径 hash、原始 DSSE envelope、
原始签名 payload/root provenance、artifact UUID 和提交时刻。ToolArchivePin
是可构造的诊断元数据，独立于 G4 semantic ToolPin 和插件 SignedArchiveLock；
InstalledInertTool 为不可构造的惰性 ZIP 收据，没有路径、lease 或启动 API。

文件层复用既有 held-parent/create-only blob 操作，只增加固定
`tool-package-store-v1/{artifacts,staging,quarantine}` 命名空间。每组仍只有
manifest.json/package.zip/seal.json，不解包 bin/ffmpeg.exe 等文件，不运行
安装脚本。插件旧 namespace、inventory/journal record v2、锁及原始字节不变。
抽象只投影 artifact ID 和原始 PackagePayload，未重新解释 Manifest 或 G4 锁。

顺序为当前持久 trust/严格 tool admission -> 全池检查与预算 -> durable
journal -> fsync/create-only stage -> durable prepared -> held-parent publish
-> 新鲜时间/根/撤销/expiry/floors 再准入 -> 精确磁盘复核 -> 最终 T0 时间
检查 -> 同一事务原子提交库存并清除 journal。最终 SQL 事务再次比较旧库存/
journal，并在提交前核对实际写入状态；未知/损坏状态不重新初始化。

同一原始 envelope 重复安装仍在磁盘复核后重查当前 trust，保留原 installedAt、
artifact/seal 和库存字节。续签可以更新 envelope/provenance，复用相同原始 ZIP，
不接受同版本不同 hash 或 release-floor 回退。不同已安装版本只作为惰性库存
保留，不据此允许执行旧版本或替换已有任务锁。

插件、工具和 reconciliation pending 相互阻止普通安装/库存操作；即使工具
journal 已损坏，它的存在也只用于拒绝，不授权解析、清理或接管。
两个 pool 的实际 retained/quarantine 字节与预留新组共享原 256MiB/384 组预算，
没有各自扩大一份配额。超量在 journal/stage 前拒绝。

显式 abort 只验证并 quarantine 本 journal 拥有的 stage/artifact，不 roll
forward、不采用 orphan、不删除原字节或降低 trust floors。实际提交失败保留
pending；未知损坏 pending 保留字节并拒绝 abort。复制数据库因 profile hash
不匹配而 StateInvalid；受支持 backup restore 因 trust recovery marker 拒绝。
两者都不清空库存或自动重新授予权限。完整工具 orphan reconciliation、解包、
健康/LKG、leases、更新/GC 与执行恢复仍待后续实施。

## 验证

十五项实际 SQLite/NTFS 回归包括：生产清单拒绝；同归档共享/重启且没有解包；
跨 pool pending；损坏/重复 JSON；SQL 提交失败；内容替换/同版本新 hash；
最终时钟过期/回退；发布后撤销；最后 trust observation 替换 journal；跨 pool
256MiB/384 组；复制/支持备份拒绝；五个 barrier 的真实进程无析构退出。
正常状态、原始归档和 grants/jobs/plugin_data/imports 的无副作用均有断言。

当前十五项通过；复用文件层的既有安装/恢复 36 项、原生移动 5 项及三个
不可伪造 compile-fail 文档测试通过，Core all-target Clippy warnings-as-errors
通过。复制专项最初错误预期 TrustRecoveryRequired，实际是独立 profile hash
拒绝 StateInvalid，已据机制分别断言：原始复制为 StateInvalid，受支持 restore
为 TrustRecoveryRequired；字节保留和无新存储池断言均保留。

该结果不是当前修改整仓/远端完成，也不是真实 FFmpeg、PE 加载、共享
executable files、accepted leases、用户文件发布或 G7 媒体闭环证据。
生产 pins、真实发行/许可证与独立安全审批没有由测试 fixture 代替。

本机 Core 标准 test（保留 SDK corpus/catalog、inventory 和原生串行全套）
exit 0：inventory 301，287 单元 + 11 调度 + 3 compile-fail。日志
`execution-validation/g6/core-tool-store-20261029.log`；单元部分约 456.93s。
随后追加 stored-record 原生 target/manifest/file 预算拒绝，十五项与 all-target
Clippy 再次通过。最终本提交的整仓及远端验收仍单独记录。

## 2026-10-31 安装 SQL 边界补充

原始工具 ZIP 的库存/commit/abort 与显式恢复现使用有限槽位事务保护，
包括替换 pending 后又被清除的 SQL 拒绝。原格式、惰性身份、字节保留和
生产空审核清单不变；本次结果另记于
[安装元数据保护](./g5-install-metadata-guards.md)。

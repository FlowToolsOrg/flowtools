# G5 P2.5b 惰性库存与安装事务第五步

日期：2026-10-11。状态：Windows/x64 T0 prototype，P2.5b/G5/G6 与独立
安全批准 pending。

## 真实数据与准入

`packages/runtime-core/src/packages/install.rs` 只在 Windows 编译；实际支持
x64。T0 的 `InertPackageBytes` 是原始 DSSE/Manifest/ZIP 借用参数，没有 Serde、
路径、grant 或 validation flag。`install_inert_plugin` 使用已有
`PackageTrustStore` 与 `DataStore`，不增加数据库/连接或 schema 版本。
时钟是 Native Host 提供的 T0 callback，提交前再次采样，不能由客户端设置。

同一 `core_metadata` 保存 `package-install-inventory-v1` 与
`package-install-pending-v1`。私有记录包括原始 signed descriptor provenance、
payload、G4 lock、root/时间和 artifact UUID；读取要求匹配不可变持久 descriptor
观察，严格拒绝未知/重复字段、坏 payload、错误 lock digest 或缺 provider。
公开 snapshot 只给历史 inert pins/locks，不是当前执行批准；commit receipt
仅表示惰性库存提交，不能构造 runner/grant/session 或 last-known-good。

入库必须经过实际 descriptor -> ZIP -> SDK 完整 Manifest -> G4 依赖准入。
provider 从本 profile 的真实安装库存选择：按 exact publisher/ID/target 遍历
必要 service 闭包，取已安装最高 release，读取并验签原始 descriptor/ZIP。
已有 provider 的传递 lock pins 必须在新计划中保持，否则 conflict，需先显式
重锁。旧版本库存/blob 保留，不能绕过更高已观察 release floors；新候选失败
也不回退观察 floors。相同 ZIP 的新鲜 descriptor 续期和显式重锁复用原 blob。
没有 ToolCandidate、安装脚本、低代码/模型插件或生产工具执行路径。

## 文件与提交过程

`install/files.rs` 从 DataStore 的既有 profile 派生
`package-store-v1/{artifacts,staging,quarantine}`。Native Host 必须先保留并
保护 profile；没有客户端可选 root。artifact 目录名是已验证 ZIP digest，
stage/quarantine 使用严格 UUID；只写 `package.zip`、原始 `manifest.json`
与 `seal.json`，不按 Manifest 文件路径解包。seal 绑定 artifact UUID 和
原始 ZIP/Manifest digest，防止更换 journal 身份后处理别的事务。

1. 当前签名、byte admission、真实库存/依赖与预算通过后，FULL SQLite
   事务先提交 pending journal，库存仍保持原样。
2. create-only stage 写入固定 blob，sync 每个文件并设置 readonly，核对
   完整字节/seal；随后独立提交 prepared journal。
3. create-only 同卷目录发布，拒绝覆盖/跨卷 copy。重新读取发布内容。
4. 新采样 T0 时间，重新验签当前根、撤销/expiry/floors、当前 Host/SDK、
   provider 原始字节和一致锁；核对库存及原始 journal 没有中途变化。
5. 同一 FULL SQLite 事务写入库存并删除 pending journal；提交成功后才
   返回惰性 receipt。业务数据/grants/jobs/imports 没有写入。

每次实际文件打开使用 OPEN_REPARSE_POINT 与 handle 信息，拒绝目录冒充、
reparse 和硬链接；读时核对长度/hash/readonly。profile/root directory guards
在操作期间保留，普通状态下内部目录也保留 guards。Windows rename 与内部
父目录 guards 的 sharing 约束冲突已在实测定位：仅移动时释放内部 guards，
根/profile 保持，然后重新获取。这依赖 T0 单写者及 Native Host 的私有
profile，**不是完整 handle-relative 路径竞争防护或同 OS 用户攻陷隔离**。
readonly 属性不证明 ACL；生产文件竞争与 ACL 平台验收仍 pending。
API 依据为 [Microsoft CreateFile](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea)
与 [MoveFileEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)。

## 恢复与预算

存在 journal 时，新安装和正常库存 inspection 返回 RecoveryPending，不会
自动采用已发布的 orphan。显式 `abort_inert_install` 只隔离 journal 所有的
stage/new artifact，保留原库存和旧 blob/锁；不滚动恢复安装或调用代码。
隔离移动和最后 marker 删除可重试，已经隔离的字节会再次核对；不递归删除。
恢复不降低 floors，不清除 backup trust-recovery 标记。恢复旧 SQLite 后即使
旧库存存在，三个 API 都拒绝，等未来受控 trust recovery。断电文件系统
持久性、健康/LKG、自动取消/恢复、激活/迁移及产品管理入口尚未交付。

库存最多 128 records、单项 metadata 16 MiB，读取先查 SQLite byte length。
blob 总量包括 retained/quarantine，上限 256 MiB、384 groups、每组仅三个
固定文件。ZIP/Manifest 仍沿用原始 64 MiB/1 MiB bounds；依赖闭包聚合 Manifest
16 MiB、展开 bytes 128 MiB，在读取/解码下一个 provider 前拒绝超限。
新候选在 journal 前预留 staging/artifact group 和 blob byte 配额，耗尽拒绝；
不静默裁剪、删除、改变当前锁或降低 floors。GC 尚未实现。

## 自动化与未完成验收

`packages/runtime-core/src/packages/tests/install_tests.rs` 的 15 项测试使用
公开 fixture keys、真实签名 ZIP、独立 UUID profile 和实际 SQLite：

- 三包实际安装 -> 磁盘 provider -> 持久 service pins、拓扑和 metadata，
  reopen/idempotent retry 保持字节，第二 writer 拒绝，grants/jobs/data/imports 空。
- 坏签名/改 ZIP、非法 Manifest、缺 dependency/tool selector、错误 native
  target 在 journal/staging 前拒绝。
- 原生测试子进程在 journal/prepared/published/before-commit/committed 五个
  barrier 实际 exit(21)；父进程 reopen 检查旧库存或完整提交，未提交显式
  abort 幂等、保留旧字节与 floors，之后新候选可重试。
- SQLite journal/prepared/inventory/delete 故障注入，最终库存写失败保留
  pending，quarantine 后 marker 删除失败可重试，不能伪造成功 receipt。
- 提交时 expiry、新根撤销、provider 字节改变都拒绝；旧库存不变化。
- 新版本 blob 保留，已有 provider 传递锁不能静默重新绑定；显式重锁后
  新 consumer 才可准入。descriptor 续期复用同一 seal，同版本改 hash 拒绝。
- 真实 junction、hardlink、缺文件/改 blob/seal、未知 stage、重复 raw keys、
  未知 metadata、丢失持久观察与 journal/库存变化拒绝。
- 实际 SQLite 备份/恢复保留安装 metadata，但所有安装 API 返回 trust
  recovery required，没有自动清除或重新 bootstrap。
- 超量 metadata、目录、物理保留总量拒绝；两个实际有效压缩 provider 各自
  准入，合并依赖池超过展开预算即拒绝，无 candidate staging。

15 项针对性回归全部通过，严格 all-targets clippy 通过；完整已提交 Windows
门禁结果将在最终验证后记录。
实现提交 `b196809` 的首次完整 gate 在既有多命令 CLI envelope 回归超时，
退出码 1；本轮 core 全部 152 项和 50 路并发冷启动通过，不能因此把整体
gate 记为成功。后续单独修订测试的 Node 子进程/stdio 所有权，保留所有
实际断言与超时，Runtime 18 项原生/16 项 JS 通过；见
[CLI 回归记录](./g3-standalone-cli.md)。完整 gate 必须在修订提交后重跑。
没有 UI/路由/native command/Tauri capability/CSP 变更，人工 UI 验收不适用。
不触碰用户 profile、不解包/执行包、不提供 production pins 或第三方执行。
SEC-001/008/014/015 保持 open；ADR-0003 增加原型事务约束，签名格式不变。
Reviewer：Codex，2026-10-11，工程自查；独立实际安全 Reviewer/date/conclusion
pending。安装脚本、可执行 artifact layout、健康/LKG、产品管理、平台隔离、
生产文件竞争/ACL/断电验收、受控 trust recovery/rollback、tool admission/GC
及 G6 工具运行仍待后续。

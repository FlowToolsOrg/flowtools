# ADR-0004：备份恢复后的受控包信任恢复

- 日期：2026-10-11；2026-10-13 移植到当前主线契约并新增库存集成验证
- 状态：implemented-prototype；独立安全批准 pending
- 责任：Repository Maintainer / Release / Security
- 范围：G5 P2.5b 的受支持备份恢复；不是 root 泄漏应急重建或版本回滚批准
- 关联：[ADR-0003](./0003-signed-package-protocol.md)、
  [受控恢复验证](../validation/g5-controlled-trust-recovery.md)

## 问题与决策

备份会丢失备份之后的版本、时间、根轮换和撤销观察。恢复旧 SQLite 后直接
清除拒绝标记或重新 bootstrap，可能重新准入旧包。旧备份自身无法证明丢失
期间的最高观察；不能仅靠恢复后的本地数据库重建安全高水位。

恢复仍由 T0 发起，须提供带完整根链和不可变观察的外部权威 checkpoint。
checkpoint 的根链先重放到既有 T0 pins；其当前 root quorum 与发布侧独立
recovery authority quorum 必须共同为本次请求和 checkpoint 原始字节 hash
签署同一短期 DSSE 批准。recovery pins 由 T0 发布配置固定，至少两把 keys/
两票，不能与任何历史/当前 root 或 publisher 角色复用。普通 `new(pins)`
默认不配置 authority，拒绝恢复。旧备份可能丢失根退役记录，因此只有旧根
的“新鲜批准”仍不足以恢复；独立发布 authority 负责核实丢失期间的最新
观察/撤销，缺失数据或任一 quorum 时继续拒绝。有效签名不是历史完整性的
独立证明，恶意/失陷的恢复 authority 不属于该机制可以消除的风险。

当前代码不包含生产 pins、私钥或自动批准服务；测试用公开 fixture keys。
没有客户端 reset/approved flag、任意 pins、回滚 bypass 或降 threshold。
只增加 T0 内部 Rust API，未新增 SDK plugin 能力、产品管理 IPC 或运行权限。

## 请求、批准与提交

1. 健康 T0 在备份恢复前导出观察 checkpoint，导出本身不产生签名。原始
   checkpoint 最多 8 MiB；保留 DSSE 根链原字节、pins digest、观察时间和
   全部不可变 kind/publisher/ID/target/version/sequence/ZIP 观察。
2. 受支持恢复保持拒绝标记。低层 restore 的 marker 改用每次生成的 UUID，
   管理 restore 保留其独立 recovery generation。T0 显式创建持久 challenge，
   请求绑定 UUID nonce、实际 SQLite main 文件规范路径 hash、两个标记、
   当前 raw trust state hash、此前恢复
   receipt hash 和可信发起时间。请求有效期最多 24 小时；同一未变化请求
   可重读，变化或明确重新发起后的新请求不能使用旧批准。只保存路径 hash，
   不记录原始路径；完整复制数据库的请求或 receipt 也不能授权另一个 profile。
3. DSSE domain 为 `application/vnd.flowtools.trust-recovery.v1+json`。
   严格 payload：formatVersion 1、requestDigest、checkpointSha256、
   rootSha256、authoritySha256、issuedAt、expiresAt。未知/重复字段拒绝；
   批准最长 300 秒。分别用 checkpoint 最终根和固定 recovery authority
   验签，两个阈值不能互相借票；publisher key、单组/旧根 keys、重复签名
   和改 domain 均不能批准。bootstrap、rotate 和历史 replay 同样拒绝
   recovery keys 被写入 root/publisher 角色，根不能自行撤销/更换 release
   authority；更换 authority 属于外部 T0 发布配置，旧 receipt 无有效 authority
   时拒绝，不隐式迁移或降低 threshold。
4. checkpoint 必须保留恢复后数据库和此前已签 receipt 的所有已知观察、
   完整根链前缀及非下降时间。原始根记录不能替换；新根仍须 N+1、双签与
   append-only 撤销。缺失/冲突/截断/过期/不兼容 pins 均拒绝。没有有效
   当前根或原 state 损坏时不自动重建。
5. 借用现有 DataStore 单写者和 FULL SQLite immediate transaction，写入
   恢复后的 trust state、原始 checkpoint/批准 receipt，删除该 challenge。
   提交前再次采样 T0 时钟；时钟倒退或批准/root/请求到期会回滚整个事务。
   两个恢复标记保留作审计，业务数据、jobs、grants 和安装库存不写入。
6. 后续每次 trust load 都重验历史 receipt 的原始批准、该次实际提交时间和
   当前 marker/profile scope，并要求当前 state 不低于签署 baseline。批准过期不
   撤销已经完成的恢复；正常包准入仍检查当前 root/expiry/revocations。
   再次 restore 会产生新的 marker/generation，旧 receipt 不解除拒绝。

## 失败与边界

恢复事务不恢复丢失的库存/journal，也不采用未引用 blob。当前惰性库存仍
使用 formatVersion 2 和独立 signed archive lock；健康库存恢复后保持原始
字节/锁可检查，备份遗漏 artifact 的库存继续 StateInvalid，等待显式文件
reconciliation。信任 receipt 不授权清除这个所有权拒绝。

请求、receipt 和 trust state 共用现有 `core_metadata`，schema v2 不变。
元数据在读取前检查 16 MiB 上限；checkpoint 8 MiB，批准 envelope 使用
既有 1.5 MiB DSSE 上限，根历史 32、不可变观察 1024 的原限额保留。超过
限额拒绝，不静默裁剪 floors 或根链。写失败/提交前崩溃保持原 state 和
challenge、无 receipt；提交后崩溃重开可重验完整 receipt，无半完成状态。

这是受支持备份恢复后的信任观察恢复，不是数据授权恢复、降级、健康/LKG
或第三方 sandbox；管理 restore 的撤销/grant epoch 行为保持。T0/root
signers 的失陷、恢复历史的真实性与完整性、可信时钟可靠性、物理突然
断电和生产签名材料须另外验收。所有 SEC-001/008/014/015 保持 open，
工程原型通过不表示独立安全批准。

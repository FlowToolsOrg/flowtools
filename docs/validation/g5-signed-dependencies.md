# G5 P2.5b 签名依赖规划第四步

日期：2026-10-11。状态：prototype；P2.5b、G5/G6 与独立安全批准 pending。

## 实现与身份

`packages/runtime-core/src/packages/dependencies.rs` 的 T0
`plan_signed_plugin` 仅接受已通过 descriptor、原始 ZIP 和完整 Manifest
校验的 `CompatiblePluginArchive`。`SignedDependencyRequest` 是借用 opaque
archives、当前 T0 Host/SDK/target 和可信时间的 Rust 参数，没有 Serde/IPC
或客户端 pins/floors/tool list。`VerifiedPluginDependencyPlan` 仅由本模块
构造；只读 getter 可检查 candidate、G4 lock、当前 root digest 和验证时间。

复用 G4 resolver 的声明检查、精确 publisher/package/service/interface
选择、DAG、确定性拓扑、反向依赖与搜索预算。crate 内 archive constructor
将 metadata digest 替换为原始签名 ZIP digest，并把 targets 收窄为 descriptor
的实际准入 target。G4 原有固定 T1 metadata 规划保持其 metadata digest。
候选根的精确 publisher/ID/version/ZIP pin 必须出现在结果中；同身份 provider
不能让解析器改选根版本。其他同版本重复 provider 继续拒绝，不猜测或覆盖。

所有输入是惰性内存字节，测试入口包含 throw canary，没有 import、loader、
文件 staging、网络请求或包代码执行。无 new command、Tauri permission、CSP、
remote URL、数据 scope 或 schema 变更。

## 当前信任与预算

规划先借用 Runtime 原有 `DataStore` 单写者，重放当前持久原始 DSSE 根链、
pins 与观察 floors，独立提交当前可信观察时间。每个 receipt 检查实际 target、
当前撤销、验证时间、最新 root digest、精确 kind/publisher/ID/version/target/
release/ZIP 观察、当前 package/root expiry 和不可回退 release floors。
只通过纯 read-only verifier、未持久观察的 receipt 不能进入此路径。

根轮换后即使没有撤销也拒绝旧 receipt，必须重新验签 descriptor。当前
Host/SDK/target 再校验原始 Manifest bytes，不沿用缓存时的兼容结论。
缺失/损坏信任状态、恢复标记、时间回退或可信时间写失败均拒绝计划。
拒绝的兼容性/解析/expiry 候选不回退已经提交的可信观察时间。

最多 512 个 archive（根加 providers），合计原始 Manifest 最大 16 MiB；
超限在 trust IO 前拒绝。原有 G4 128 packages、4096 edges、10000 search
steps 等预算不变。没有 unverified ToolCandidate 参数，完整 tool selector
返回 missing；旧 tool metadata 拒绝，不能据声明生成工具库存。

所有提供的 providers 都必须通过当前 release floor，包括最终没有被选择
的候选。该保守新鲜候选池不是已安装多版本库存；观察更新版本后，旧 provider
不能混入其中。真实安装库存、多版本保留、受控回滚与持久 lock 尚待后续。

## 拒绝回归

`packages/runtime-core/src/packages/tests/dependency_tests.rs` 的 10 项测试
使用公开 fixture keys、真实 DSSE、完整 SDK-compatible Manifest、实际 ZIP
及 Runtime 内存 SQLite 单写者，覆盖：

- 三包签名 DAG 的精确 ZIP/service/interface pins、实际 target、拓扑、反向
  依赖和输入顺序无关的 lock digest；metadata digest 不能替代 ZIP identity。
- 缺 provider、错 publisher、package/interface 范围冲突、legacy service
  selectors 与依赖环拒绝。
- 重复 provider、根候选替换拒绝；同身份新 release 后旧 archive 拒绝。
- metadata 声明多平台不能扩大实际准入 target；混入其他 target 拒绝；当前
  Host/SDK 升级后重新检查 cached Manifest。
- 有完整 selector 的 tool 缺真实库存拒绝，legacy tool metadata 拒绝。
- 未持久观察的纯 verifier receipt、不同 ZIP 身份不能凭兼容 metadata 放行。
- 当前根轮换使旧 receipt 失效；撤销拒绝；重新验签后才可生成当前根计划。
- package/root 过期、可信时间回退、更高 release 后陈旧 provider pool 拒绝，
  expiry 拒绝仍保留观察时间。
- 信任缺失/损坏、恢复 marker、SQLite trigger 注入观察写失败均拒绝。
- 超量 candidate/合计 Manifest 预算拒绝；没有 grants/jobs/plugin_data/imports
  写入，不持久化安装锁。

## 验证与未交付项

针对性 10 项及 core 全部 137 项 Rust 回归、lint、all-target clippy
`-D warnings`、types、rustfmt 与 docs:check 已通过。实现提交 `6e383c4` 在
干净工作区运行 `pwsh -NoProfile -File scripts/check-ci.ps1`，完整退出码 0，
无 tracked/untracked 漂移。冻结依赖、workspace/docs/catalog 契约、固定
Chromium 安装、包构建前置、Host routes/Rust bindings 生成和 read-only
contracts 校验均通过；11/11 工作区 lint、types、test、build 全部通过。
root test/build 顺序执行，测试 uncached。

本轮 SDK 368 项、core 137 项 Rust、Runtime 18 项 Rust + 16 项 JS、Desktop
89 项 JS + 21/3 项 Rust、Chromium 27 文件/72 项通过。独立 CLI 分发真实
50 路并发冷启动、实际服务链、共享数据、ACK 丢失、撤销和恢复回归通过。
root test 6m25.949s，build 1m5.323s。Web 22 / Desktop 24 个生产产物在
child opt-in / canary 重构建后 byte-identical，已知 unsafe/认证开关/canary
缺席；Desktop locked check、rustfmt、all-target codegen clippy 通过。

`a8280f1` 单独保留冷启动 fixture 安全单项失败诊断；此前同一 `afd0cde`
远程 CI 一次成功、一次冷启动非零退出，具体阶段因批量诊断截断未确认。
本地单 CPU fixture 与本次完整 gate 通过均未复现原失败，不能据此认定
间歇原因已修复；见 [冷启动证据](./g3-standalone-cli.md)。没有重试断言、
减少 50 路启动、暴露原始输出或放宽超时。

最后证据提交仅文档，另跑 docs:check；最终 SHA 的远程 CI 由 PR checks
独立记录。该路径没有 UI/native 流程变更，人工 UI 验收不适用。

计划仅证明该验证时间的 byte/trust/compatibility/dependency 结论，不能作为
安装 identity、grant、lease、session 或 last-known-good。真实 installer 必须
在最终提交时重新核对信任、库存和原始 artifact。本第四步不保存安装库存；
后续[第五步](./g5-inert-installation.md)已交付 Windows/x64 T0 惰性库存、
持久锁、固定 blob store 和 journal/显式 abort 恢复。产品安装/激活、
tool Manifest/admission、下载、生产文件竞争/断电验收、健康检查/LKG、
生产 pins、受控 trust recovery/rollback approval、第三方平台隔离及 G6
工具执行仍 pending。

SEC-001/008/014/015 保持 open，ADR-0003 增加规划约束，签名协议格式不变。
Reviewer：Codex，2026-10-11，工程自查与拒绝回归；独立实际安全 Reviewer、
日期和结论 pending。工程测试不是签名发布或独立安全认证。

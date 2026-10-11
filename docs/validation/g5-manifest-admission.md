# G5 P2.5b Manifest 准入第二步

日期：2026-10-11。状态：prototype；P2.5b 整体和独立安全批准 pending。

## 实现与边界

`packages/runtime-core/src/packages/manifest.rs` 在只读验签与 ZIP 准入之后，
消费 opaque `ValidatedArchive` 的原始 `manifest.json`，检查完整 Plugin
Manifest v1 与 T0 提供的 Host/SDK/平台兼容性。返回不可由 JSON 构造的
`CompatiblePluginArchive`，保留原始 signed bytes 和独立的规范化 metadata。
遗漏 maturity 仅在 metadata 中补为 prototype，不改原始文件或 packageDigest。
tool descriptor 不能借此取得 plugin receipt。

SDK 仍拥有 Manifest 结构 schema；公开的 `exportManifestValidationContract`
不产生信任或授权。Runtime 的现有 `generate-catalog.ts` 从 SDK 公共出口生成
ignored `.generated/manifest-validation.json`，Rust 编译时嵌入同一构建契约。
未手写 Manifest DTO，也未修改 Rust-derived IPC bindings。

Zod 自定义检查不能直接完整导出为 JSON Schema，见
[Zod JSON Schema](https://zod.dev/json-schema)。本步使用版本化 semantics
`flowtools-manifest-v1` 和显式 `x-flowtools-check`：版本/range、portable path、
唯一性、命令、服务、依赖 selectors 和有限 operation schema。生成器拒绝
未映射的 custom、额外叠加的 custom check、transform 与未支持的 Zod 类型。
Rust 对未知标签默认拒绝；SDK/Rust 共同样本检查这些 native 语义。
修改已有 refinement 的行为仍需同步语义、契约版本及拒绝样本，不能仅靠结构
JSON Schema 宣称等价。

检查包含未知字段、entries/inventory、重复命令/文件/target、服务入口与定义、
服务 headless/interaction、完整依赖选择器、重复/混合依赖声明、权限请求重复、
冷启动条件、operation 子集/递归/default/enum 与资源声明。版本与 ranges
复用 G4 的 npm 语义实现，不引入另一套 semver parser。T0 兼容版本使用
canonical exact semver；签名安装路径继续使用已有更严格的 lowercase ASCII
ZIP profile。UTF-16 字符串限额与 Unicode scalar operation 长度语义分别保留。
raw JSON 重复键、危险属性、超过 32 层/100000 节点、1 MiB Manifest 和
64 KiB operation schema 均拒绝；不读取 schema URL、加载代码或调用 capability。

本步验证依赖声明，不解析真实已安装 provider/tool inventory，也不创建新的
dependency lock。G4 固定 T1 lock 的内容 digest 不能冒充 signed ZIP digest。
仍须完成真实安装锁绑定、持久 trust/root/time/release floors、staging/journal、
artifact/metadata 提交、取消/崩溃恢复和 last-known-good 健康检查。
receipt 不是已安装或可执行身份，不会创建 grants、runner session 或新 IPC。
T2/T3/TL 与 G6 受管工具执行继续等待实际平台/runtime 门禁。

## 拒绝回归

- SDK `test/fixtures/manifest-admission-v1.json` 的 90 项共同接受/拒绝样本，
  由 SDK `test/generate-manifest-admission.ts` 构造并先核对实际 SDK parser；
  SDK 单独报告每项，Rust 使用同一 JSON 和目标参数。
- SDK 91 项测试覆盖共同样本及 export guard、确定性、未知/叠加 custom 和
  transform 拒绝；native 测试另验证十二个实际 build-owned Manifest。
- 实际测试公钥/DSSE -> stored ZIP -> 完整 Manifest 的 inert 链路；正确签名
  但冷启动语义错误拒绝，错 SDK/arch 拒绝，tool role 混用拒绝。
- 原始 bytes 与 packageDigest 不变、代码中执行 canary 未加载；额外回归
  duplicate raw keys、危险 schema 属性、depth/nodes/bytes/schema budget，
  JSON 数字 1/1.0 等价与结构 default 检查。

仅使用公开测试 keys 和 in-memory fixtures；没有生产 pins 或用户数据。
SEC-001/008/014/015 保持 open，独立安全 Reviewer/date/conclusion pending。

## 验证记录

本机 Bun 1.3.14 / Rust 1.96.0 的针对性检查已通过：`docs:check`、SDK
lint/types、core rustfmt 与 all-target clippy `-D warnings`、SDK 新增 91 项
测试和 core 全部 114 项 Rust 测试。十二个实际 Manifest 也通过 native
校验；其构建 chunk 路径可含大写，metadata 使用 SDK portable path 规则，
不会把更严格的 signed ZIP profile 错当成已有 T1 Manifest 限制。

UI、IPC、现有 T1 runtime 行为未改变，本步没有新的人工 UI 流程；未启动
用户 profile。全量 Windows gate 的最终结果在交付前记录；不以构建或共同
样本通过代替任意外部包安全性、平台隔离或独立审阅。

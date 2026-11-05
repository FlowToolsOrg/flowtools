# NA0：Manifest 与 wire 版本契约

- 日期：2026-11-05；范围：Windows/T1 契约基础，implemented-prototype。
- 来源：[四项 API 计划](../plugin-native-api-design.md) 的 NA0；NA1–NA7 保持 pending。
- 安全前置：本分支继承 G8 PR #17 的 `0b99b4c`，该提交的
  [Windows 门禁](https://github.com/FlowToolsOrg/flowtools/actions/runs/41841689327)
  已通过。PR #17 尚待合入 main；本变更不代为合并或批准 G8/发行。

## 交付边界

本项交付显式 Manifest v2 声明、SDK/Rust 一致校验、签名包/G4 元数据准入、
wire client 0.4.0 和私有 runner frame 2。v1 元数据的解析与字节继续保持，
现有内置库存仍全部为 v1。v2 的纯命令可以在受控固定 T1 宿主执行；带效果或
权限的 v2 命令在调用代码/接受效果之前拒绝，尚无新文件、opener、secret 或
network adapter，也没有新增生产 grant、Tauri command/permission 或 CSP 来源。

原计划的 policy migration 在 NA0 具体化为“兼容读取并保留原记录，拒绝新的
效果授权”。旧 PolicyImport 的 format 1 和 Scope 枚举不扩展，误送 format 2
或新增 resource/endpoint/url/credential scope 拒绝；当前 SQLite schema 仍为 2，
不迁移、不重写原 grant。
真实 v2 grant 的持久格式/逐效果准入与对应 NA1+ adapter 一起验收，不能因本项
的声明类型已可解析而提前开放。本项没有把一份空权限映射成第二套授权系统。

## Manifest 分派与一致性

- [v1 schema](../../packages/sdk/src/manifest/schema.ts) 与
  `parsePluginManifest()` 继续仅接受 v1。
- [显式新入口](../../packages/sdk/src/manifest/v2.ts)
  `parseVersionedPluginManifest()` 按 formatVersion 1/2 分派。
  v2 在包和每个 command/service operation 中都要求
  `capabilityProfile: "flowtools-native-v2"`；未知版本、profile 与字段拒绝。
- 旧 string scope 不可混入 v2。v2 scope 为 resource、endpoint、url、credential、
  data、clipboard、notification、tool 的封闭 union；能力、操作、效果、scope
  类型和 HTTP 读/发送方法必须一致。槽位与 origin 仅为声明，不是可用 Ref 或 DNS 授权。
- v2 当前 scope pathPrefix 仅接受最多 16 个 ASCII 字母/数字/下划线/短横线段，
  endpoint 为规范化、明确端口的 HTTPS 域名；不接受原生路径、任意 header/scheme。
- [结构导出](../../packages/sdk/src/manifest/validation-contract.ts) 独立生成
  `manifest-validation-v2.json`，旧 `manifest-validation.json` 的内容不变。
  Rust 沿用严格 JSON、原数字归一化、UTF-16、finite operation 与包预算规则。
  v2 的自定义语义由 [native_contract.rs](../../packages/runtime-core/src/native_contract.rs)
  明确校验，不依赖 JSON Schema 自动表达所有 refinement。
- [签名 ZIP 准入](../../packages/runtime-core/src/packages/manifest.rs) 保留实际
  signed archive 的原始 manifest 字节，再返回版本化规范元数据；签名/目标/
  engines/文件清单校验与 G4 完整 Manifest digest/immutable lock 继续独立成立。
  新格式不产生安装/执行许可或新的 provider/tool 身份。

原 405 条 corpus 的 SHA-256 固定为
`fe6b2c4934c1bec9d4184a1721de52bdd98efd7e9f0605e0384a7cb827ff16fb`；
原 v1 结构导出（规范 JSON 加 LF）固定为
`1146cf6c8eb2e4517bdddb754ca53a57562c809a8f3e597adcfc5abaef70a333`。
两者均有回归，不能刷新旧样本/摘要来掩盖行为变化。
新增 [110 条 v2 corpus](../../packages/runtime-core/manifest-v2-conformance.json)
单独生成、按 LF 固定，并在 Core generate/verify/test 中只读检查。

## wire 与 runner

[Rust protocol](../../packages/runtime-core/src/protocol.rs) 保留 major 1、1 MiB
帧上限与原业务 DTO；client 精确版本升为 0.4.0。0.3.0 和未知 client 在
session.open 时返回 CLIENT_INCOMPATIBLE，发生在插件/业务效果之前。
新增稳定错误 CAPABILITY_UNAVAILABLE、RESOURCE_EXPIRED、NETWORK_FAILED，
原错误码语义不变。新增 Rust 声明类型/支持格式常量通过原 codegen 导出，
TS 不维护 DTO 镜像。旧客户端不与新协议静默混跑。

[私有 runner](../../packages/plugin-runner/src/protocol.ts) 与
[Host pipe](../../apps/runtime/src/service_runner.rs) 的所有 start/reply/
service/capability/media/result 帧明确携带 version 2。初始版本错时在导入插件
前拒绝；Host 对 child 输出仍先走 G8 递归重复 key 拒绝，再解析严格 envelope。
关联 ID 单调递增，最多 255 次请求加最终 result，沿用原 256 帧总量、deadline、
根/服务授权检查、取消和实际 OS Job drain。未知字段、旧帧、过量与重放不产生调用。
新版本不提供任何通用 capability/原生路径 selector。

SDK/CLI/catalog/package/UI loader/执行器显式消费版本化元数据，保留固定 known-id
compiled entry 与原完整文件校验。runner 在导入命令入口之前核对 Host 的包摘要；
导入后的摘要检查仍保留。未知 ID 的原 LOAD_FAILED 结果、旧 Package changed
拒绝及原 text/JSON/退出码行为保持。无源码 fallback、TSX 改写或任意路径加载。

此次 framing 不实现资源流/网络 body、背压或独立传输取消通道；它们随相应
adapter 在 NA1/NA5 实施。取消仍由当前 root/provider 生命周期、期限与真实
进程/Job 排空约束，不能宣称普通 Bun runner 已是第三方 sandbox。

## 验证

- SDK：显式版本分派、旧 parser 关闭性、未知/嵌套字段、类型化 scope、效果/
  操作/method 混淆、纯命令执行和效果调用前拒绝；v1 结构导出摘要回归。
- Rust：原 405 条及新 110 条 corpus；真实重新签名 ZIP 原字节保留、G4 完整
  digest 绑定与篡改拒绝；v2 不能借旧 grant、旧持久 grant 字节不变/错误导入不写入。
- 实际 compiled runner：缺失/旧/未知版本拒绝、真实 Base64 的 v2 纯命令正对照、
  旧 Host pin 的导入前拒绝；原服务 pin 拒绝和 Todo 的实际受控 CAS 均保留。
  新 v2 场景统一由一个 Node Host 驱动，复制实际 CLI 分发及 chunks，并用新 Bun
  编译进程打包实际 Base64 实现；整组沿用 25 秒 Host / 30 秒测试上限，每次提交
  仍有原 10 秒调用期限。初版多次从 Bun 启动 Host 在整仓运行时超时，未计为
  通过；改为单 Host 后专项通过，原协议夹具与原期限/拒绝断言未放宽。
- wire：实际 named pipe 旧 0.3.0 client 拒绝；原客户端正常握手；static validator
  与数字格式/无动态代码回归、codegen LF/漂移和原 G8 安全测试不放宽。

初次直接 Cargo 专项使用了未重建的 isolation-probe，既有 LPAC 测试的 report.write
为 Null 而失败。按 Core 标准准备流程重建实际探针后，用完整测试名确认收集了
该一个用例并通过；生产源码、原断言、期限与隔离策略未修改。零收集的过滤运行
不计入通过证据。正常 Core/root 门禁仍按原流程构建探针并运行完整 inventory。

完整交付运行 `scripts/check-ci.ps1` 的原顺序；同一提交的具体本机根门禁、
远端 Windows CI 与 PR 状态在本次 PR 记录。生成物来自真实 Rust/SDK 源码变更，
验证使用原只读比较；不得暂存刷新来绕过清洁工作区或 validator 拒绝。

前置核查遇到 RustSec 直连失败和 crates.io 查询超时，均未计入通过。实际观察到
固定 cargo-audit 0.22.2 可在 registry 查询报错后仍返回 0，因此四份实时审计
额外启用 `Invoke-QualityCommand -RejectErrorOutput`：保留输出与既有 native
非零退出拒绝，去除 ANSI 后发现 `error:` 即中止；informational warnings 继续
完整显示。`scripts/ci-contracts.test.ts` 以实际 Node 子进程覆盖 stdout/stderr 的
彩色错误加 exit 0、非零退出、告警正对照及禁止继续执行。原 pins、扫描选项、
实际扫描器拒绝 controls、完整历史与精确例外不变，无离线/陈旧数据库回退。

安全评审：Codex 工程自查；实际独立 Security Reviewer/date/conclusion pending。
SEC-001/003/004/005/006/007/008/009/010/012/013/014/015 保持 open。
正式发行、平台隔离、签名分发、四项 IO 的原生/UI 验收均不由本项批准。

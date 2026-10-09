# G4 P1.6a：依赖计划与不可变锁

- 范围：固定 T1 inventory、Host 内部可丢弃 fixtures；maturity 为 prototype。
- 状态：P1.6a 已实现；2026-10-08 独立验证并提交。G4 父里程碑仍进行中。
- 安全风险：SEC-014、SEC-015；独立安全批准 pending。

## 声明与解析

SDK 的 `@flowtools/sdk/dependencies` 提供严格、有限的纯数据声明解析。
Manifest 可选 `services` 声明 versioned service 和 headless operation Schema；
非空声明要求包文件清单包含 `entries.services`。服务依赖包含固定 publisher、
provider plugin ID、包版本范围、service ID 和接口版本范围。工具依赖绑定
publisher/id、版本范围、target、buildFlavor 和 artifact digest。
旧裸依赖可作为元数据读取，但实际解析时拒绝缺失接口或工具 selector，
不会猜测入口、接口或 artifact。缺省字段不填入旧清单，保留原有 digest。

Runtime 从自身固定目录解析 DAG。服务 provider 在一个计划中采用单包版本；
所有 consumer 的包和接口约束共同参与有界回溯。工具按 consumer 精确绑定
artifact，可保留不同版本。锁包含 target、roots、包与服务/工具 artifact、
依赖边、前置 provider 顺序、反向依赖及确定性 SHA-256。锁没有本地 checkout
路径、凭据、开发 URL 或调用者选择的可执行路径。

内部候选读取器只防御性检查依赖语义；完整 Manifest 准入依赖构建时 SDK
校验的固定目录。受控 fixtures 的简化 metadata 不等于可安装或可执行的包。
锁绑定完整 manifest 摘要和既有文件 digest，不代替实际字节验证或签名。
反向依赖当前记录直接服务 consumer；工具是独立 consumer leaves。多平台
工具声明只选择 Host 对应 target，每组缺失该目标明确拒绝。

版本范围采用 npm 语法，在 SDK/Rust 共用 fixtures 的受控数值域内验证。
显式核心版本组件和纯数字预发布标识符不超过 `900719925474099`；
自动生成的排他上界不计入源声明上限。字母数字预发布标识符和范围中的
build metadata 不受该数值限制。exact version 沿原 SDK 规则拒绝 build
metadata，防止同身份不同规范化结果。此 prototype 不声明全域 npm 等价。

## Host 与 CLI

`dependencies.plan` 只接收固定 `pluginIds`；publisher、目录与目标平台由 Host
派生。返回 `dependency-plan`，mode 为 `plan-only`。调用必须通过既有绑定连接
的 session。计划不会提交任务、激活 provider、安装包、修改 grants 或持久化锁。
P1.6a 不替换既有任务的 `t1-no-dependencies-v1`；执行绑定在 P1.6b 实现。
当前十二个 built-in 未声明服务依赖；非空 DAG 使用内部受控 fixtures 验证。

CLI 可对已经启动的 Host 查看 text/JSON 计划：

```text
flowtools dependencies plan plugin-base64-encoder --profile <profile> --format json
flowtools dependencies plan plugin-base64-encoder --profile <profile>
```

解析错误使用稳定 `DEPENDENCY_*` 代码，失败 JSON 输出到 stdout，退出非零。
缺失身份和非法 format 在 profile/native IO 前拒绝。输出不构成安装或授权确认。

## 验证边界

专项回归覆盖：缺失、循环、重复 provider、版本与接口冲突、假 publisher、
平台与 digest 不符；跨 consumer 多服务约束、有界回溯、工具多版本；顺序无关
且 artifact/Schema 变化改变锁；旧清单 digest 不漂移；未知字段、原始路径、
超量 JSON 与 catalog 注入拒绝。真实编译 CLI、Runtime 与 Node transport 验证
重复查询锁一致、text/JSON 可读，查询前后 jobs/grants 未变化。

2026-10-08 本地验证：

- `docs:check`、`verify:workspace-tasks`、`verify:plugin-catalog`、生成清单只读检查通过。
- `verify:runtime-contracts` 通过；三份 TypeScript/Schema/fixture 来自 Rust 生成器。
- 全仓 `lint --force`：11 个 workspace 通过，代码检查零警告、零错误。
- 全仓 `check-types`：11 个 workspace 通过。
- 全仓 `test`：11 个 workspace 全部执行并通过；固定 Playwright 1.59.1 Chromium
  浏览器 26 个文件 / 66 tests；Runtime Core 67 Rust tests；实际 native CLI 9 tests。
- SDK 依赖专项 20 tests / 323 assertions；Runtime client 全包 17 tests / 299 assertions。
  两端共有 47 个 npm 匹配向量；声明边界 fixtures 分别检验接受/拒绝。
- 全仓 `build --force`、`verify:production-entrypoints` 与 Desktop Rust fmt/check/clippy
  通过。入口 gate 不代替服务授权、签名或沙箱验收。
- 新增 CLI text/JSON 在可丢弃 profile 中使用实际编译入口和 Host 验证；本次未变更
  GUI 流程，未启动用户数据库。已有 GUI 的自动回归完整运行。

本机 Bun 1.4.2、Rust 1.99.0、Node 26.5.0；远端 Windows CI 按仓库固定
Bun 1.3.14 / Rust 1.96.0 另行验收。Node 工具链弃用提示与既有构建 chunk-size
提示仍存在，不属于本次代码 lint 失败。独立人工安全审阅尚未获得批准。

提交：`feat(runtime): resolve immutable dependency plans (P1.6a)`。
此子项不开放服务 RPC、权限委托、排空更新、卸载或第三方安装。
P1.6b/G5 的退出条件不能由解析计划或构建成功代替。
SEC-014/015 保持 open，其他平台和真实签名安装未获认证。

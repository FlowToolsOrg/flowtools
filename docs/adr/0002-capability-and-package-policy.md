# ADR-0002：能力授权、包准入与恢复策略

- 日期：2026-10-03
- 范围修订：2026-10-04；增加冷启动/副作用授权、服务委托与共享工具包生命周期
- 状态：accepted-design；实现未完成，不能作为生产安全认证
- 决策责任：Repository Maintainer；Rust / SDK / Data / Release 模块负责人
- 路线图：P0.4a；实现由 P1.1、P1.3–P1.6、P2.3–P2.8、P3.5 验收

## 背景

当前权限粒度是 `fs/network/storage/db/native` 等字符串，SDK 注入能力来自
manifest；Desktop adapter 提供 raw SQL 和通用 invoke。目录解析与数据库
记录不具备包准入的完整性或授权含义。具体入口见
[威胁模型](../security/threat-model.md)，执行边界见
[ADR-0001](./0001-plugin-trust-boundaries.md)。

## 决策

### 授权与调用

每次原生或敏感操作均由 Rust broker 再授权：

```text
Host 会话身份有效
AND 已验证包的 manifest 声明允许
AND 用户持久 grant 允许
AND operation 参数落在 scope 内
AND grant epoch 与资源预算仍有效
=> 执行；否则返回稳定拒绝码，不产生业务副作用
```

grant 绑定 package identity、明确操作与规范化 scope；升级新增能力、扩大
scope 或变更 publisher/hash 时重新审查与征求授权，不能凭相同 ID 继承。
拒绝/撤销立即影响活跃会话和排队调用；对已执行副作用明确恢复限制。
受控 last-known-good 回滚也重新核对身份与 grant，不恢复已撤销权限。

| 能力                    | 目标窄接口与 scope                                                                  | 禁止提供给 T2/T3/TL                                                        |
| ----------------------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| 文件                    | broker 发放的 file handle 或限定目录的相对操作；规范化、symlink/junction 与竞争检查 | 任意绝对路径、根目录/home scope、调用方指定基础目录                        |
| 网络                    | scheme/host/port/method allowlist、逐跳重定向及解析后地址策略；超时与响应配额       | 无策略 fetch、Host cookie/token 继承、private/link-local/metadata 地址绕过 |
| 数据                    | Host 绑定 namespace 的 KV/受控 query；参数化值与 allowlisted schema                 | raw SQL、跨 namespace 表名、任意连接字符串、Host metadata/grants 表        |
| Native                  | 带版本的 typed operation enum、严格 payload schema 和大小上限                       | 字符串 command + 任意 payload、shell、Rust reflection                      |
| 工具                    | 验证后的工具 artifact/版本 lock、typed operation、任务私有 workspace 和预算         | 任意 executable path、raw argv/env/cwd、PATH 漂移与未经准入的二进制        |
| Clipboard/dialog/opener | 每项独立操作、用户交互与 scope；URL/路径/外部应用策略                               | 绕过授权直接调用 opener、任意 scheme/路径、后台无限读取剪贴板              |

文件读取需校验真实路径，写新文件需校验父目录与最终打开目标；简单 substring
`..` 检查或先 canonicalize 后重新按字符串打开不足以解决竞争。Windows drive、
UNC、ADS、reparse point、大小写和符号链接必须进入恶意 fixture。
网络 CSP 只作纵深防御，不能替代 broker 出站策略或用户 grant。

冷启动、Host 后台驻留、开机自启、调度触发与业务资源权限分开。
supportsColdStart 声明只表示命令无需界面；Host bootstrap policy 只允许启动受管
内核。内核接受命令后重新检查包身份、命令 grant、scope 与 epoch。非交互 CLI
缺授权返回 APPROVAL_REQUIRED，确需界面返回 INTERACTION_REQUIRED，不静默打开
GUI。覆盖、删除、发送需要明确持久授权或绑定 action digest/有效期/次数的一次
授权；修改参数不能复用旧批准，普通 run flag 不能创建 grants。
CLI-only 首次初始化和显式策略管理是必选交付；管理模式只配置已验证内核，
不加载插件。未配置的业务调用返回 SETUP_REQUIRED；会话角色由 Host 绑定，
普通调用不能升级为管理调用。交互不证明调用者一定是人，同 OS 用户攻陷不在
此保证内。持久 grants 基于 P2.6a 的迁移和单写者，不另建临时权限数据库。

服务依赖不自动授予权限。Host 保留 root caller、provider、parentRunId、委托
handle/scope、剩余 deadline 与撤销 epoch；有效权限不能超过调用方授权、
可委托范围、operation 上限和 provider 适用授权。服务私有数据仍由 provider
namespace 拥有，调用方不能指定其内部表或路径。

兼容 IPC 使用版本化 schema、来源/目标、会话 nonce、唯一 request ID、大小与
并发上限。来源绑定 actual webview/session，不凭主窗口 URL 或插件自报 ID。
禁止 `postMessage('*')`；不允许远程 fallback 自动获取原生权限。

### 包与更新准入

在执行任何入口代码之前，验证 versioned manifest、文件清单/hash、发布者
签名与受信 provenance、Host/SDK 版本范围和平台/架构。签名、hash 与 TLS
分别解决不同问题，不能相互替代。

- staging 与 installed 分离；归档大小/文件数/压缩展开量有限制；拒绝绝对
  路径、Zip Slip、symlink 越界和特殊文件；所有最终文件重新核对 hash。
- 已发布版本不可变；同 publisher/ID/version 对应不同 hash 时拒绝覆盖。
- 私钥不进源码、前端 bundle、`.env` 示例或日志；trust root、签名格式、
  key rotation/revocation 与离线策略在 P2.5 实施前补充协议 ADR 与 fixtures，
  此处不发明未实现的加密协议。
- 生产包必须签名；未签名开发包只能在未来显式开发模式、醒目风险提示和
  独立测试数据区运行，不能改变 production trust store。
- 更新先验证再原子切换；失败恢复 last-known-good，恶意/撤销版本不得恢复。
  防降级策略与用户确认的受控回滚分开处理。
- 应用更新使用官方 Tauri updater 的签名校验；插件包签名是独立协议，
  不能用应用 updater 的成功推断插件供应链已经安全。
  [Tauri 官方 updater 文档](https://v2.tauri.app/plugin/updater/)

工具包采用独立 versioned manifest，记录 publisher、工具 ID、版本、目标平台/
架构、build flavor、文件清单/hash、来源和许可证。插件侧载与下载走同一准入，
不运行安装脚本；同版本不同 hash 拒绝。依赖范围解析为 Host 生成的不可变锁。
工具文件可内容寻址共享，多版本并存；服务 v1 单 profile 单 provider 版本，
冲突失败，不静默切换。不共享授权、进程环境、工作目录、用户数据或任务结果。
服务更新停止接收新调用、排空或明确取消旧任务，备份/迁移后切换；不允许新旧
provider 同时写同一 namespace。P1.6 先验证 T1/fixtures 的 plan-only 依赖，
第三方实际下载/安装必须等待 P2.5 签名准入。
工具辅助 executable/DLL 也在签名清单内；loader 搜索不得含可写 cwd/workspace、
插件目录或继承 PATH，须验证伪造辅助文件不被加载。

安装用 staging/journal 恢复文件系统与 metadata 的跨资源提交；不能假设一次
SQLite transaction 就能覆盖文件落盘。工具更新新 artifact 并健康检查后切新任务锁，
任务 accepted 并固定 lock 时取得 lease，覆盖排队、运行与可恢复 interrupted
任务。GC 从安装锁、持久任务引用/leases、用户 pin、last-known-good 根集合
计算，宽限后回收。撤销版本不得新执行或恢复。

### 持久化、诊断与恢复

Core metadata、grants、history 与 plugin data 分开 ownership。使用版本化、
幂等迁移与升级前备份；N-1 → N、失败重试、downgrade 拒绝/恢复策略有 fixture。
卸载是否删除数据必须显式确认，Debug 启动也不得隐式删库。

目标 Runtime 为单写者；GUI/CLI 经异步、带 revision 的 data API 共享状态。
旧同步 SDK store 用明确 hydration/订阅适配，旧临时存储与 localStorage 先校验、
用户选择后导入，保留原数据。可恢复 job payload 位于独立私有 workspace，
限额与保留期按数据分类；secret 用 credentialRef，history/log 仍只存安全元数据。
Host 崩溃后不自动重放非幂等副作用；文件/外部发送不承诺通用回滚或 exactly-once。
客户端提交前生成 request/幂等 key；Runtime 持久化 accepted 后返回 receipt。
ACK 丢失按同 key 查询/重提，绑定 caller、包版本/hash、命令、依赖 lock 与 action
digest；无法查证非幂等任务时保留待核实状态，不生成新 key 重做副作用。

日志仅记录 Host 生成的 run/request ID、package identity、operation、拒绝码、
duration 和 scope 摘要。不记录输入/输出正文、剪贴板、文件内容、凭据或带
query/token 的 URL；限制体积、保留期与导出范围。授权/撤销/安装/更新决定
可审计，但普通本机日志不宣称具备防管理员篡改能力。

## 当前实现

- P2.3a 已新增验证 Runtime 的 T1 内存策略 broker，Host 绑定命令身份并逐项
  检查声明、grant、scope、epoch 和预算；授权管理与敏感 IO 尚未开放。
  [验收与拒绝证据](../validation/g3-capability-broker.md)。此基础不改变本文
  production 默认拒绝策略；持久授权与数据仍等待 P2.6a/P2.4a。

- 没有可用于 production 的包签名、Host 范围准入、持久 grant 或 Rust broker。
- Desktop FS adapter 透传路径给官方插件；仍受其 permission/scope 限制，
  但 SDK 未绑定 per-plugin scope，不能宣称已能读任意 OS 文件或已隔离。
- 插件 DB adapter 共享 SQLite 连接；CRUD 参数化和 identifier 校验不能提供
  namespace 授权；`db.query` 接受原始 SQL。
- `native.invoke` 是通用 Tauri 调用；HTML opener 分支直接调用官方 API。
- Tauri `csp: null`，未配置 app command allowlist；官方插件 capability
  集合属于主窗口，不是每插件授权。
- Debug 初始化会删除 `app.sqlite`；CLI key 前缀/临时目录不是文件 scope。
  现有 logger 可写 payload/details，没有通用脱敏审计日志。

## 后续验证

- P1.1/P2.5：签名前改文件、伪发布者、同版本不同 hash、不兼容 Host、未知
  critical 字段、截断包、zip bomb 与解包穿越都在代码加载前失败。
- P2.3/P2.4：直接 IPC、重定向、路径竞争、跨 namespace、payload 注入、
  撤销竞争与升级扩大 scope 全部拒绝；拒绝后无写入/联网等副作用。
- P2.6：空库、N-1、迁移中断/备份恢复、卸载取消与 Debug 重启不丢数据。
- P1.5/P3.5：secret canary 不出现在日志/bundle；错误签名与撤销 key 更新
  不安装；健康检查失败可回滚且不恢复过期授权。
- P1.6/P2.8：缺失/循环/冲突依赖拒绝；前置服务不能代理越权；两插件共享工具
  仍独立授权；排空失败不切 provider；工具输入/协议/间接路径、伪造 DLL/辅助
  程序、raw argv、平台不符与撤销版本拒绝，排队/恢复任务保护 artifact。
- P1.3/P1.4/P2.6/P3.1：冷启动并发、GUI/CLI revision 竞争、幂等键输入冲突、
  CLI-only 初始化、提交 ACK 丢失/同 key 恢复、不重放发送、调度重复/休眠/时区
  与崩溃租约恢复均有 fixture。

这些属于待实现的验收目标，不由 `docs:check` 或本 ADR 的通过代替。

2026-10-05 P2.6a update: Runtime single-writer SQLite, CAS/transactions,
N-1 migration/rollback, explicit source-preserving import and backup recovery
are covered in [shared data acceptance](../validation/g3-shared-data.md). Ordinary Desktop Debug reset
is removed and corrupt legacy DB startup preserves the original. Historical
G2 reset evidence above remains accurate for that incident; no lost user data
recovery is claimed. Durable grants/jobs and third-party boundaries remain open.
Security Reviewer/date/conclusion: pending independent review.

2026-10-05 P2.4a: [persistent policy evidence](../validation/g3-persistent-grants.md) adds Host-bound management, current-user private profile ACL, atomic grant import and durable epoch/audit. T1 only; same-account compromise, isolation, signing and independent Security Reviewer/date/conclusion remain pending.

2026-10-05 P1.3b: [durable job evidence](../validation/g3-durable-jobs.md)
binds receipts and replay to Host identity, immutable action/package lock and
durable grant epochs. Private DPAPI payloads are separate from metadata. Running
non-idempotent tasks interrupt after crash. This T1 lifetime guard is not a
third-party sandbox; SEC risks and independent security review remain open.

### G3 P1.4a standalone CLI

Windows x64 CLI-only bundle and bounded, authenticated cold-start coordination
are implemented for the fixed T1 inventory. See [standalone CLI evidence](../validation/g3-standalone-cli.md).
The package pins relative compiled artifacts and supports explicit GUI-free init,
runtime start/status/stop and granted bundle execution. Source CLI/GUI integration
and operational diagnostics remain P1.4b/P1.5b. Maturity remains prototype;
signed releases, third-party sandbox and independent security review are pending.

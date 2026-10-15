# G5 P2.5b 受支持备份的受控信任恢复

日期：2026-10-13；2026-10-14 继续集成验证。状态：T0 prototype；完整
G5/G6 交付与独立安全批准未完成。
移植并复核旧本地实现 `da7c6a3`；当前分支以主线 `a5f847e` 为基线，
不包含尚未合入的 G6 工具准入 PR。本次验证独立于旧实现的历史测试结果。
协议见 [ADR-0004](../adr/0004-controlled-trust-recovery.md)。

## 实现

[trust/recovery.rs](../../packages/runtime-core/src/packages/trust/recovery.rs)
新增 unsigned checkpoint 导出、持久 challenge 和根/独立恢复双 quorum 批准后的显式
恢复。checkpoint 先验证既有 pins 和完整 root chain，并保留当前及历史
receipt 的全部已知 floors；批准绑定 raw bytes、当前恢复事件、nonce 和实际
SQLite main 文件规范路径 hash；不保存原始路径。
FULL 单事务写 state/receipt 并删除 challenge，提交前新采样可信时间。
恢复标记保持，正常 trust load 每次验签 receipt 并拒绝降低 baseline。

低层 `DataStore::restore_backup` 每次生成新 UUID trust marker，防止回放
旧恢复批准；管理 restore 的 recovery generation 继续保持。grants、业务
数据、jobs、插件库存和包文件不在该恢复事务内，原管理恢复撤销行为不变。
不会因为一个有效 checkpoint 就将安装 ZIP 升级为可执行身份。

## 实际验证

[恢复回归](../../packages/runtime-core/src/packages/tests/trust_tests/recovery_tests.rs)
使用实际原始 DSSE、根轮换/双签、真实 SQLite backup/restore 与拥有的 UUID
profile；覆盖：

- 初始化 trust 前后的实际备份恢复；显式批准恢复完整历史，并保留拒绝标记。
- 当前 root quorum 和固定发布侧 recovery quorum 共同批准；append-only
  撤销恢复，publisher/仅根票/仅恢复票/单个旧 root key/重复票数/错误 domain
  不能批准。旧备份丢失新根/高 release 后，退役根用旧 checkpoint 签“新鲜”
  请求仍因缺少独立 authority 拒绝；正常包调用拒绝撤销包与低 release。
- 普通 trust store 未配置恢复 authority 时拒绝；authority keys 与 bootstrap、
  publisher 或后续 root 复用时拒绝，不写入冲突根或恢复 receipt。
- 跨 profile、完整复制 SQLite 请求/receipt、变更 marker、不同 nonce、checkpoint 字节改动、state 改动
  和再次 restore 的旧批准重放；不发布恢复 receipt。
- 丢弃已知 observations、修改根链前缀、过期批准、时钟倒退、超长有效期、
  未知 payload 字段、损坏和超过 8 MiB 的 checkpoint；均拒绝。
- 实际 SQLite receipt 写入/challenge 删除触发器故障，state/receipt/request
  同事务回滚；原 state、标记和 challenge 保留。
- receipt 篡改或普通 mutable state 低于签署 baseline，后续 trust load 拒绝。
- 管理 restore generation 保留并绑定；再次 restore 的新 generation 需要
  新批准，不自动解除拒绝。
- 原生子进程在提交前/后实际 exit(23)，重开只得到 pending 或完整恢复；
  提交前失败仍可显式提交有效批准，无半完成 receipt。
- 审批过程中到期或时钟倒退，最终提交采样拒绝并回滚；时钟正常推进后
  receipt 和持久 observed time 记录实际提交时刻。

本轮重跑恢复矩阵 17 项（包含专用 crash child），全部通过（5.76 秒）；
当前主线全 Core 186 项与 1 项 compile-fail 通过，严格 all-target Clippy
`-D warnings` 通过。之后新增一项真实 formatVersion 2 库存集成回归也通过：
正常两包库存/原始 locks 在 backup/双 quorum 恢复后字节不变且可检查；
先备份再安装的包留下真实 orphan，恢复只补齐 trust observations，库存
inspection/new install 继续 StateInvalid，不采用 orphan、建立 journal 或
改变原库存/字节。两条路径都没有新增 grants/jobs/plugin_data/imports，
保留 restore marker。

实现提交 `3a0222a` 的 clean Windows 根门禁：所有 11 个工作区 lint/types、
read-only 405 项实际 SDK corpus、Rust bindings、Chromium 27 文件/72 项通过；
全 Core 187 项与 1 项 compile-fail 通过。Runtime 原生 21 项通过、3 项失败：
`deadline_retains_lease_until_the_actual_provider_group_drains`、
`real_job_access_denial_stops_host_and_refuses_provider_migration`、
`root_cancel_retains_lease_until_the_actual_provider_group_drains`。
失败仍是既有 C fixture 未生成后代 PID 后超时，root test 退出 101，
后续 root build/产物/native gates 未由该脚本执行，Desktop tests 被依赖失败阻断。
因此本地完整根门禁没有通过；当前提交的远端 CI 单独确认。

2026-10-14 的临时阶段诊断确认失败 C fixture 已接收并验证请求、进入实际
处理函数，停在 `Bun.spawn()` 返回之前。只读管道最小复现 16 项通过；
真实 Windows Job 下的固定 detached 后代 spawn 最小复现出现阻塞。
原生 Web stream、按需读取、Node 兼容 spawn 和控制台启动参数对照均未消除
阻塞，尚未确定 Bun/Windows 内部根因，不能据此替换协议或声称已修复。
临时诊断已撤除，原 Job 归属/终止/租约断言和全部 timeout 保持；没有跳过
用例、自动 retry 或用独立成功覆盖原失败。产品管理和独立安全批准仍 pending。

独立补验：`bun run build --force` 11/11 工作区通过（1 分 17 秒）；随后
生产入口 Web 22/Desktop 24 个实际产物在 child-only opt-in/canary 重构建后
byte-identical，通过。Desktop rustfmt、locked check 和 all-target codegen
Clippy `-D warnings` 通过；没有启动 Desktop 或访问用户 profile。
这些后续检查不改变上述本地 root test/完整门禁失败结论。

## 残余风险

checkpoint 的丢失历史完整性由外部权威 T0/root signers 核实，本地旧备份
不能独立推断丢失的高水位。生产 pins/私钥、可靠时间来源及实际 signers/
独立 reviewer 仍需外部提供；不存在自动批准或随包导入 trust。根 key 泄漏
应急重建、受控版本 rollback、执行健康/LKG、OS/文件竞争/物理断电仍另行
验收；此代码不授予能力或开放第三方执行。

Reviewer：Codex，2026-10-14，工程自查与上述拒绝回归；独立实际安全
reviewer/date/conclusion pending。SEC-001/008/014/015 保持 open，
ADR-0001/0002 执行边界不变；无新增 Tauri command/permission/CSP/remote
URL/任意路径/第二 DB，UI 人工验收不适用。

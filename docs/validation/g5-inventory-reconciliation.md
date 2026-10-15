# G5 P2.5b 备份恢复后的惰性包文件核对

日期：2026-10-14。状态：Windows/x64 T0 prototype；G5/G6 和独立安全验收 pending。
移植旧未合入实现 `75a6b78`，基于受控信任恢复 PR #13，重新验证当前
formatVersion 2 库存和持有父句柄的原生 create-only rename，不使用历史
通过结果替代本次验证。本项独立提交并创建堆叠 Draft PR。

## 行为与边界

旧 SQLite 备份可以恢复旧库存，却不会同步删除备份之后的 ZIP 或暂存目录。
信任恢复后，普通库存检查继续拒绝这些无引用文件，不自动收编或执行。
[reconciliation.rs](../../packages/runtime-core/src/packages/install/reconciliation.rs)
新增 T0 只读 opaque plan、显式 apply/resume/abort。计划只保存实际 main DB
规范路径 hash、恢复 scope/receipt hash、库存 digest、固定 digest/UUID 名称
及惰性文件的大小/hash；摘要给出 artifact/stage 数量和字节数，没有调用方路径。

先验完整物理清单、现有库存字节、所有候选 fingerprints 与隔离区冲突，
再在同一 `DataStore`/schema v2 的 `core_metadata` 写持久 journal。
每个移动前重验 trust chain/恢复 baseline、库存和 journal，源固定期间重验
字节，以持续持有父句柄的 native create-only rename 隔离；不覆盖、不复制、
不删 ZIP、不收编 orphan、不清除恢复标记。未引用内容的 fingerprint 只是
字节观察，不能替代包验签、Manifest/依赖准入或授权。

完成 receipt 和 journal 删除由 SQLite FULL 单事务提交。中断后普通安装/
检查/安装 abort 拒绝，T0 可显式 resume；已经隔离且 hash 一致的组不重复移动。
显式 abort 保留已移动和未移动的全部文件，只记录取消并删除该 journal。
旧恢复 scope 的 journal 不能继续；当前信任先受控恢复后，可明确 abort，
再生成新的核对计划。库存改变、跨 profile、字节篡改或不可确认的文件拒绝，
不自动清除 journal。

保持原 16 MiB metadata、384 groups/256 MiB pool、单 ZIP 64 MiB、Manifest
1 MiB、seal 512 bytes 与固定三个文件名限额。quarantine 仍计入预算，未实施
GC。额外名称、特殊文件、hardlink、junction/reparse、缺失隔离父目录拒绝。

## 实际验证

[回归](../../packages/runtime-core/src/packages/tests/install_tests/reconciliation_tests.rs)
包含实际签名 ZIP/backup/restore、双 quorum 信任恢复、Windows native 移动和
拥有的 UUID profile。覆盖：

- 备份后完成安装或停在 prepared 的包，恢复后分别保留 ZIP/stage 到隔离区；
  原库存、旧 blob、信任 floors/marker 不变，更低 release 拒绝，新版须重新准入。
- 未信任原始字节只隔离，不生成库存/lock/grants；只读计划和空计划无文件写入。
- 全批预检拒绝变化字节、额外文件、目录冒充文件、hardlink、异常名称和损坏库存；
  第一组也不移动，outside canary 保持。
- 真实 `mklink /J` 在 orphan 组及文件叶位置的重定向拒绝；不读取或修改目标 canary。
- 跨 profile 计划、完整复制 pending DB、库存 revision、安装 journal 互斥；
  伪造 journal 指向有引用包、路径穿越、未知/重复原始字段、重复组或超大 metadata 拒绝。
- 隔离目标碰撞、journal 后文件变化均保留 pending，未覆盖已有目标。
- 实际 SQLite journal/receipt INSERT 或 journal DELETE 失败无半完成 metadata；
  重开后幂等继续，取消写失败仍保留 journal 和全部文件。
- 真正 restore 了 pending journal 后，旧 scope 拒绝继续；先双 quorum 恢复信任，
  再显式取消和重新规划。取消本身不改变 trust 或恢复标记。
- 原生子进程在 journal、两个 group、before-commit、committed 共五个 barrier
  实际 exit(24)；重开只有完整 pending 或完整 receipt，原库存一直保持。
- 内存 profile、超过 groups/bytes 配额、缺失 quarantine 父目录拒绝，无新 journal。
- 当前 formatVersion 2 的真实 provider→middle→consumer 三层签名锁在备份、
  双 quorum 信任恢复和隔离后字节不变；重新准入更新 provider 后仍保留原锁和
  旧版本 blob，不自动重绑定，grants/jobs/plugin_data/imports 无新增。
- 旧 formatVersion 1 库存在 plan/apply/resume/abort 四个新入口都 StateInvalid，
  保留原库存字节、trust floors、孤立 stage，无新 journal/receipt 或移动。

本次 focused 19 项全部通过（10.39 秒）；全 Core 206 项和 1 项 compile-fail
全部通过（64.24 秒），strict all-target Clippy `-D warnings`、docs、
read-only 405 项实际 SDK corpus 和 Rust bindings 通过。保留既有原生
publication/quarantine/collision/reparse 断言，复用同一个持有父句柄的
create-only rename，无释放 guard 重试或路径回退。完整根门禁及当前 SHA
远端 CI 在后续证据提交单独记录；旧实现的历史通过不作为本次验收。

实现提交 `7311446` 的 clean Windows 根门禁中，11/11 lint/types、
405 项 SDK corpus/Rust bindings、全 Core 206 + 1 compile-fail、SDK 786、
CLI 100、Chromium 27 文件/72 项通过。Runtime 原生 20 项通过、4 项失败：
`malformed_provider_frame_retains_lease_until_its_real_group_drains`、
`provider_error_retains_lease_until_its_real_descendant_drains`、
`real_job_access_denial_stops_host_and_refuses_provider_migration`、
`root_cancel_retains_lease_until_the_actual_provider_group_drains`。
失败仍为既有 C fixture 后代 PID 缺失/等待超时；root test 退出 101，
Desktop tests 被上游依赖阻断。没有修改这些测试、timeout 或原协议，
也没有 retry/跳过用例。本地完整根门禁没有通过，后续独立构建/产物检查
与当前提交的远端 CI 分别记录，不覆盖这次失败。相关启动阶段诊断见
[受控恢复证据](./g5-controlled-trust-recovery.md)。

独立后续补验：全工作区 `bun run build --force` 11/11 通过（1 分 18 秒）；
实际 Web 22/Desktop 24 个 production 产物的 child-only opt-in/canary
重构建 byte-identical，通过。Desktop rustfmt、locked check 与 all-target
codegen Clippy `-D warnings` 通过，生成内容没有 tracked/untracked 漂移。
没有启动 Desktop 或访问用户数据库；这些检查不覆盖 root test 的原失败。

## 未完成项与评审

文件 readonly 和 native guards 不替代私有 profile ACL、全部文件写入竞争或
突然断电验收。篡改 journal/未识别内容需 T0 取证，不能自动删除或重新授权。
本项没有下载、可执行解包、激活/健康/LKG、rollback approval、卸载/GC 或
第三方 sandbox，也未新增 Tauri command/permission/CSP、产品 IPC、plugin
capability、原始路径或第二 DB；UI 人工验收不适用。

SEC-001/008/014/015 保持 open；ADR-0001/0002 执行边界不变，包 ownership
遵循 ADR-0003，信任恢复遵循 ADR-0004。Reviewer：Codex，2026-10-14，
工程自查及实际拒绝/故障/进程回归。独立安全 reviewer/date/conclusion pending。

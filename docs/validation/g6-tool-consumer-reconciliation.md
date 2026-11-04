# G6 P2.8a8：惰性消费方库存的显式 orphan 恢复

日期：2026-10-31。状态：in-progress / prototype。此步只补齐 P2.8a7
`tool-bound-plugin-store-v1` 中没有被当前有效库存引用的原始 blob；不收编、
不删除、不重锁、不创建 lease/grant 或执行身份，生产审核清单保持空。

`packages/runtime-core/src/packages/install/tool_consumers/reconciliation.rs`
提供 T0 只读 opaque plan 与显式 apply/resume/abort。同一 DataStore writer
保存独立 pending/receipt，绑定 canonical profile、完整消费方库存、其他安装
库存与当前 trust/recovery scope。先整批复核字节指纹和预算，再借用原 G5
held-parent/create-only 原生移动；每次实际移动前，在持有源句柄的 callback
内部检查冻结的全部相关元数据。没有调用方 namespace、路径或验证 flags。

普通工具/消费方读取继续完整验证并拒绝未知文件。只有此恢复路径会读取
已经严格验证的 raw 库存、实际已知原始 ZIP 和消费方引用，再观察消费方
孤儿；同时遗留的 raw ZIP 孤儿不会被返回、采纳或移动，便于先隔离消费方
孤儿，再按原 image-before-raw 约束完成其余域恢复。

中断保留 exact pending 和所有字节。resume 只能继续原 scope；新还原 scope
须先完成原有受控信任恢复，再显式 abort，保留已移动和未移动组。receipt
记录原计划与提交时实际 scope；写 receipt/清 journal 前后再次比较库存、
scope 和原 journal，SQL 触发器不能清除替换后的记录。全部 pending 仍互斥，
四个固定物理池含 quarantine 合计保持原 256MiB / 384 groups。

## 当前验证状态

二十项专项通过：只读空观察、真实备份/受控恢复及消费方
先于 raw 孤儿隔离、旧锁/字节保留、整批变更、跨 profile/库存/scope、所有
pending、损坏/伪造 journal、签名撤销中断、SQL 触发器、总预算、已知内容
损坏、create-only 碰撞、junction、五处真实进程退出，以及实际 held-source
callback 内的元数据拒绝。十九项实际集成专项通过（238.84s），另一个
held-source callback 原生拒绝用例通过（0.10s）；日志
`execution-validation/g6/consumer-reconciliation-focused-20261031.log` 和
`consumer-reconciliation-native-guard-20261031.log`。Core all-target Clippy
`-D warnings` 通过（16.06s），Rust fmt 与 docs:check 通过。随后完整 Core
395 项单元（1051.68s）、11 项调度与 9 项 compile-fail 全部通过，日志
`execution-validation/g6/consumer-reconciliation-core-compat-20261031.log`，exit 0。
随后 `b45f960` 完整本机根门禁 exit 0，最终工作区无漂移：11/11 workspace
lint/types/test/build，Core 395+11+9（单元 1019.92s）、Runtime 24+6/Bun 17
（含原 50 路实际冷启动）、Chromium 109/34、Desktop 106 Bun+Node 1+原生
36+6、Web 23/Desktop 25 产物一致、production refusal 及最终 Rust 检查通过。
日志 `execution-validation/g6/root-ci-b45f960-20261031.log`。
同 SHA 的 [PR 41177618924](https://github.com/FlowToolsOrg/flowtools/actions/runs/41177618924)
和 [push 41177615465](https://github.com/FlowToolsOrg/flowtools/actions/runs/41177615465)
均 SUCCESS，远端 ref 已核对与本地相同。

accepted leases、产品执行、更新/GC/LKG、真实发行/许可证与独立安全批准、
G7 用户文件提交/撤销和可播放媒体输出仍待完成。本步骤不会把孤儿字节
重新登记为已安装包，也不降低信任 floors。

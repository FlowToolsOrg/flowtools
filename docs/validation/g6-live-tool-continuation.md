# G6 已在途工具调用的有限续接

日期：2026-11-02。P2.8c1，prototype；当前专项与最终整仓结果分别记录。
只处理已经以当前 release floor 准入、创建实际 suspended process 的有限逻辑
调用；不恢复排队/中断任务，不开启新调用、用户文件发布、更新/GC 或产品 Host。
生产 review/pins 仍为空；G5/G6/G7 父项保持 pending，G7 人工项为 waived。

## 作用域与信任

`trust/held.rs` 的 `HeldDescriptor` 无公开构造器、Clone 或 Serde。普通当前
准入后才能持有，绑定原 signed descriptor hash、实际 writer 的随机实例、
canonical DB hash、恢复代次和 T0 pins。writer UUID 仅在 DataStore 内存中，
不改 SQL schema/JSON，不是第二 writer，也不会写入任务历史。恢复代次在
SQLite 内限定 TEXT/1–128 字节后再读；错误/未知值拒绝，不当作不存在。

普通 `verify_package`、`verify_descriptor`、`check_receipts` 仍拒绝旧 release。
专用路径只容许同一原始已知身份因 floor 前进出现的 ReleaseRollback；其他
拒绝全部保留。精确原 descriptor bytes、manifest/ZIP hash、签名、当前根、
撤销、有效期和单调时钟仍重查，不传 rollback flag、不改 floors/known versions、
不续期或采纳同 ZIP 的新 descriptor。根变更也中止旧调用，保持既有保守策略。
`ExistingToolArchive` 与普通安装/规划的 `InertToolArchive` 是不同类型。

`leases/continuation.rs` 的 `LiveToolCall` 进一步绑定原任务完整 lease、root
caller/package/lock/epoch/deadline、具体逻辑 UUID、实际 run/consumer/action。
只在原生准备成功后、首个 ResumeThread 前以普通当前 floors 再次验证并生成。
此后每次运行观察、第二进程准备/resume、提交及收取仍检查当前整个 broker
授权链和原 task row；continuity 不是用户 grant，也不增加调用预算或期限。

## 实际 IO 与恢复

续接仅重读原 pin 的实际 signed ZIP 和既有 seal/image，沿用完整原清单、共享
预算、pending 互斥、owner/DACL/label/handles 及两次当前检查；不能用引用重新
materialize 或绑定新文件身份。旧/新调用共享真实映像，仍有独立私有输入和
LPAC/Job。原 5s/500ms、管道/输入预算和未知排空保留策略不变。

新逻辑调用、同步 T0 callback 和普通 take_run 仍走普通 floors。没有 live
凭据的排队/中断/重开任务不能续接；旧版本导致 Host 启动拒绝时保留原记录及
私密输入，不自动更新锁、恢复权限或重放。新版本切换/健康检查、accepted 队列
策略与更新/GC/LKG 仍需后续明确事务。

## 验证状态

源码准备包含八项真实签名/SQLite proof 拒绝回归，以及独立候选六项实际
LPAC 续接用例；driver 保留原 6/10/8/9，新增 6，精确总计 39。
类型、Clippy 与专项结果见下方；当前完整根门禁和远端结果另记。

首轮 Core 编译在新增测试读取 SQLite length 为 u64 处拒绝（rusqlite 不提供
该 FromSql）；改为原生 SQLite i64，保持 1048576 精确断言和 SQL 读取上限。
产品验证逻辑未改。原失败日志
`execution-validation/g6/live-tool-descriptor-initial-20261102.log` 保留。
修正后八项签名/SQLite 回归 **8 pass / 0 fail / 0 ignored**（9.04s，编译
25.68s，469 filtered），Core 实际 inventory 477。日志
`execution-validation/g6/live-tool-descriptor-fixed-20261102.log`。这不是原生
执行或完整门禁通过；其他专项继续另记。

前一步 WebM 提交 `ec78ae1` 已完成本机完整根门禁 exit 0、最终无漂移：
Core 468+11+10（1205.10s）、11/11 workspace、Runtime 24+6（44.06s/2.15s）
与 Bun 17、Chromium 109/34、Desktop 108+Node 2+原生 36+6（3.43s/0.11s），
Web 23/Desktop 25 产物一致和 production refusal、最终 Rust 检查通过。
已推送并核对远端完整 SHA；该 SHA 的
[PR 41386659144](https://github.com/FlowToolsOrg/flowtools/actions/runs/41386659144)
与 [push 41386653132](https://github.com/FlowToolsOrg/flowtools/actions/runs/41386653132)
于 2026-11-02 核实均 success。本次修改不引用前一步结果作为当前树的整仓通过。

当前四项生产/旧策略定向 **4 pass / 0 fail / 0 ignored**（13.88s，473 filtered），
包括新 production empty-review 拒绝与原 queued old-release、撤销/重授、根撤销
三项策略不变；Core all-target Clippy 通过（11.73s）。日志
`execution-validation/g6/live-tool-policy-focused-20261102.log` 与
`live-tool-core-clippy-initial-20261102.log`。

当前独立候选的六项实际 LPAC 续接 **6 pass / 0 fail / 0 ignored**（76.77s，
编译 24.78s，510 filtered）；实际 wrapper inventory 516。覆盖原 signed
版本下限前进后的信息检查、WAV 独立 probe、WebM 提交/收取；新调用/另一
旧 accepted root 仍拒绝、当前 grant/包撤销仍拒绝，重开没有 proof 且保留两份
输入。原 program/source hashes、实际 PID/Job 回读与 5s/500ms 未放宽。
日志 `execution-validation/g6/live-tool-native-six-initial-20261102.log`。
完整 39 项、普通 Core 全套与最终根门禁继续另记，不以定向结果替代。

首轮完整显式 39 项 driver **exit 0**：原六项 6 pass（47.76s）、信息 10 pass
（130.67s）、WAV 8 pass（120.79s）、WebM 9 pass（133.92s）、在途续接 6 pass
（78.79s），均 0 fail / 0 ignored；实际 6/10/8/9/6/39 清单、格式、探针与
all-target Clippy（12.59s）通过。日志
`execution-validation/g6/live-tool-full-driver-initial-20261102.log`。

随后复核发现原 lease scope 的恢复代次读取先分配 String 再检查长度，现也改为
在 SQLite 内校验 TEXT 与 1–128 字节后才读取；格式及合法值原字节保持，
非法已存在记录返回 StoreCorrupt，不按缺失处理或删除。新增真实 SQLite 用例
包含空值、129 字节、UTF8 字节数、1MiB TEXT/BLOB 及合法边界。相关十三项
**13 pass / 0 fail / 0 ignored**（26.24s，465 filtered），当前 Core inventory
478；当前普通 Core all-target Clippy 再次通过（6.46s）。日志
`execution-validation/g6/live-tool-bounded-scope-focused-20261102.log` 与
`live-tool-core-clippy-final-20261102.log`。修改后的完整 39 项另记。

包含 lease 恢复代次有界读取的最终当前树完整 driver 再次 **exit 0**：原六项
**6 pass**（48.07s）、信息 **10 pass**（127.31s）、WAV **8 pass**（114.60s）、
WebM **9 pass**（138.70s）、续接 **6 pass**（84.01s），均 0 fail / 0 ignored。
精确 6/10/8/9/6/39 inventory、格式、实际探针和候选 all-target Clippy（5.93s）
通过；当前 wrapper 总数 517。日志
`execution-validation/g6/live-tool-full-driver-final-20261102.log`。
原程序/source hashes、原六项及所有 deadlines/Job/lease/拒绝断言保持。
最终干净提交仍须完整根门禁，不以专项结果关闭 G5/G6/G7 或生产审查。

主工作区整合后实际 Runtime all-target `cargo check --locked` 通过（6.93s）；
全部二十项 CI 契约 **20 pass / 0 fail**（46.05s，630 assertions），包含扩展
后的精确清单、原失败传播和内容漂移保护。日志
`execution-validation/g6/live-tool-host-check-main-20261102.log` 与
`live-tool-ci-contracts-main-20261102.log`。文档及严格类型检查通过；当前提交
完整根门禁与对应远端仍须单独运行。

`b4f8049151b536967ed5757e5e919e1ba1941cc8` 随后完整本机根门禁 **exit 0**、
最终无漂移：Core 478+11+10（1410.25s）、11/11 workspace lint/types/test/build、
Runtime 24+6（44.21s/2.20s）及 Bun 17、Chromium 109/34、Desktop
108 Bun+Node 2+原生 36+6（3.81s/0.15s），Web 23/Desktop 25 一致性与
production refusal、最终 Rust check/clippy（12.15s/9.02s）通过。
日志 `execution-validation/g6/root-ci-b4f8049-20261102.log`。已推送并以
ls-remote 核对同一 SHA；[PR 41395085530](https://github.com/FlowToolsOrg/flowtools/actions/runs/41395085530)
与 [push 41395080112](https://github.com/FlowToolsOrg/flowtools/actions/runs/41395080112)
当时仍运行中，不能记为 success。PR 保持 Draft；G5/G6/G7 不关闭。

# 版本切换恢复日志的事务边界

日期：2026-11-02。范围：既有 T0 provider 迁移边界的故障拒绝加固；不是
工具版本激活、GC/LKG 或 G7 完成。

## 问题与行为

原 `commit_provider_change` 在准备、提交和恢复时直接覆盖单行日志。SQLite
触发器的忽略、替换或附带写入可能不被发现；迁移回调更改的日志也可能被
最终写入覆盖。运行中的 actor 没有保留已确认日志的原始字节，无法识别
已提交 receipt 被删除后又被缓存状态重建。

`runtime_service_journal.rs` 只处理原 `service-catalog-state-v1` 固定槽位。
读取先在 SQLite 限定 TEXT 和 65536 字节，再分配 Rust 字符串。原 format 1、
JSON 字段、disabled 上限 128、catalog/backup/phase 校验和单 DataStore writer
保持；没有新数据库字段、授予权限的 token 或安装域。

actor 在加载或确认提交后保存精确原字节；提交前必须与数据库相符。准备、
最终提交和回滚恢复全部在原 writer 的事务中比较精确前值，检查一行受影响、
总变化恰好一次和实际回读。触发器即使恢复了看似正确的最终值，附带写入也
会拒绝。迁移回调的其他正常 SQL 仍在原事务内，不能替换日志前值。

只有明确完成迁移回滚并确认恢复原字节后，才重新开放旧 catalog。未知
提交、回滚或恢复失败停止准入，保留现有日志和恢复状态；不能
清除异常记录并假称成功。已有非规范空白 JSON 在重开校验后按原字节保留，
回滚不重新序列化旧值。

此内存前值检查不声称阻止数据库所有者在重开前重写整个数据库。G5/G6 的
四安装域仍使用原 `packages/install/metadata.rs`、原记录与预算；没有借此
改变其恢复/信任约束。

## 验证

- 修复前实际八项：1 pass / 7 fail（1.47s），确认问题可复现；失败日志保留。
- 修复后原八项：8 pass / 0 fail（1.54s）。扩展后十二项加原三项恢复契约：
  15 pass / 0 fail（3.50s）。包括准备替换/忽略、提交替换、迁移替换、删除
  receipt、回滚 UPDATE/DELETE、附带 SQL、非 TEXT/超限、非规范原字节恢复
  和原 deferred-FK 提交失败。原测试与断言未移除。
- Core all-target Clippy 通过（20.46s）。完整 44 项实际媒体专项、本机完整
  根门禁和对应远端 CI 分别记录；尚未完成的结果不记为 passed。

日志位于忽略的 `execution-validation/g6/`：`provider-journal-red-20261102.log`、
`provider-journal-green-initial-20261102.log`、
`provider-journal-service-regressions-20261102.log`。

生产 Host 仍无工具 authority，production reviews/pins 仍为空。没有新 IPC、
路径/argv/执行选项或用户文件效果；人工 G7 项维持 waived，整体仍
**in-progress / prototype**。后续仍需版本激活/队列策略、GC/LKG、文件 grant/
提交/撤销、媒体输出发布及 GUI/CLI 闭环，见 [完成矩阵](./g7-completion-matrix.md)。

原生专项首次运行在原六项中的最终 write-open 遇到 Windows error 32，
5 pass / 1 fail（24.63s），driver 按原规则 fatal 退出。只读检查对应保留的
SQLite：两项任务均 succeeded、toolArtifacts.retained=false、无错误；没有
证明具体文件占用来源，不能笼统归因于杀毒软件。同一原用例单独运行
1 pass（5.50s）；未修改产品或 fixture、断言/期限、哈希/Job 检查，随后按原
完整 44 项 driver 重跑，结果另记。原失败与诊断日志保留：
`provider-journal-full-driver-20261102.log`、
`provider-journal-sharing-diagnostic-20261102.log`。

完整 driver 重跑随后 **exit 0**：原 6 pass（24.12s）、信息 10 pass
（62.81s）、WAV 8 pass（58.61s）、WebM 9 pass（67.00s）、续接 6 pass
（36.12s）、健康 5 pass（42.51s），均 0 fail / 0 ignored。精确
6/10/8/9/6/5/44 inventory、实际原探针、格式与候选 all-target Clippy
（8.40s）通过。所有 source/program hashes、实际 LPAC/PID/Job 和原拒绝/
时间断言保持。日志 `provider-journal-full-driver-rerun-20261102.log`；首次
失败没有被删除、跳过或改成成功。最终提交的本机根门禁和远端 CI 仍须独立记录。

主工作区实际 Host 的原服务集成 **15 pass / 0 fail**（46.10s）：包括真实
A/B/C、原 owned PID/Job drain、迁移等待、拒绝/取消/撤权、旧任务和失败回滚/
重启。实际 Runtime all-target `cargo check --locked` 通过（5.08s）；未更改
这些 fixture、生产 Host 或原期限。日志
`provider-journal-native-host-focused-20261102.log`。文档/格式契约通过；
最终干净提交仍需原完整根门禁，不能用聚焦测试代替。

`16665da87aafaf743ee96da4e2ce220372660a54` 随后完整本机根门禁 **exit 0**、
最终无漂移：Core 498+11+11（541.60s）、11/11 workspace、Runtime 24+6
（46.49s/2.09s）及 Bun 17、Chromium 113/35、Desktop 108 Bun+Node 2+
原生 37+6（4.21s/0.11s），Web 23/Desktop 25 产物一致及 production refusal、
最终 Rust check/clippy（8.43s/6.88s）通过。已推送并核对相同远端 SHA，
[PR 41430253763](https://github.com/FlowToolsOrg/flowtools/actions/runs/41430253763)
与 [push 41430245804](https://github.com/FlowToolsOrg/flowtools/actions/runs/41430245804)
分别核对 success。PR 保持 Draft，G7 成熟度不提升。日志
`execution-validation/g6/root-ci-16665da-20261102.log`。

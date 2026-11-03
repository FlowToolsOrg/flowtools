# 未派发工具队列的显式处置

日期：2026-11-03。范围：T0 对确认从未开始执行、现已 Interrupted 的工具
队列进行显式取消并释放它自己的 artifact 引用。G7 保持 in-progress /
prototype；没有客户端操作、工具删除、GC/LKG 或产品执行授权。

## 确认与存储

`runtime_tool_queue_discard.rs` 提供不可构造、不可序列化的
`QueuedToolDiscardPlan`。只接受本 Core 的原任务，计划绑定实例、最后确认的
整行原字节摘要及当前 trust/安装/profile/recovery 元数据。创建计划只读；
确认时重新核对完整材料与精确 challenge，重启、复制或改变的记录不能复用。

只允许仍保留工具引用的 Interrupted 队列：started_at 等于 accepted_at，
完整连续事件仅含 Queued/Interrupted，最后事件及序号匹配当前快照。曾经
Running、服务调用、任何 inspection/WAV/WebM 原生准备，以及未排空的实际
调用全部拒绝；这里不处置无法证明副作用状态的原生历史。pending runner、
broker session 和服务 root 都必须已经结束。停止准入或管理恢复副本拒绝。

原保留引用读取器复核实际签名来源/密封镜像和当前 scope，而不是为旧版本
重新准入。确认通过既有 jobs.metadata 单写者、原 32KiB 界限及事务写入；
事务内比较冻结 metadata stamp、精确前值、one affected/actual row 和回读。
SQL 替换/触发器或未知提交停止准入，保留 actor 的原句柄及所有私有字节。

仅在持久确认后将任务记录为 Cancelled / ABORTED，追加事件并释放本任务
的引用。包、锁、epoch、deadline、原 image references 与输入不改绑；不
重放任务、不刷新 grant 或降低 floor。其他消费方的引用保持。私有输入不在
此操作中删除，仍交给原已确认终态清理路径；工具物理文件始终保留。

## 验证

独立真实 `native_queue_discard.rs` 六项：

- 普通 Queued、Running、已确认终态拒绝，不改原行。
- 实际 signed-fixture 安装、两个消费方和提高 floor 的原 Interrupted
  收据；精确确认只释放自己的根，保留原 pins/epoch/deadline/密文，错误
  challenge/其他 Core/重复处置拒绝，正常当前目录重启后不重放。
- 相同 JSON 含义但原字节被 SQL 替换时拒绝，保留外国字节、根与输入。
- SQL 触发器替换释放写入时回滚，原根/输入不丢，未知写入停止后续准入。
- 当前信任变化和重启分别要求新计划，不为旧收据续权。
- 实际已准备原生输入的任务重启后继续保留，不能伪装成未使用的队列处置。

六项专项最终 **6 pass / 0 fail / 0 ignored**（32.04s）。首次新测试在旧
catalog / 新 floor 的冷启动前置拒绝，4 pass / 2 fail；按原协议显式提交
已安装新版本的整批目录，再验证正常重启，未放宽启动准入或原 floor。
日志 `execution-validation/g6/queued-discard-focused-20261103.log` 与
`queued-discard-focused-final-20261103.log` 均保留。

完整 driver 保留此前 61 项，再增加六项独立 fatal 分组，共 67 项；原
source/program hashes、实际 LPAC/SID/PID/Job、5s/500ms 和物理预算不变。
driver 契约专项 1 pass / 308 assertions、严格 TypeScript 已通过。完整
67 项、当前提交的整仓门禁与远端结果分开记录，未完成不计通过。

随后完整 driver **exit 0**：6/10/8/9/6/5/5/4/4/4/6 共 67 项，全部
0 fail / 0 ignored；实际原 probe、精确 inventory、fmt 与 all-target Clippy
（7.47s）通过。各组分别为
21.36/52.22/42.94/47.00/30.39/29.58/22.58/41.80/20.72/26.55/27.68s。
完整日志 `execution-validation/g6/queued-discard-full-driver-20261103.log`。
当前修改树的干净根门禁和同提交远端检查仍独立记录。

前一步官方来源提交 `2cfb8e1` 已完成干净本机整仓门禁 exit 0、最终无漂移：
Core 511+11+12、全部 11/11 workspace、Runtime 24+6/Bun 17、Chromium
113/35、Desktop 108 Bun+Node 2+37+6、Web 23/Desktop 25 产物等值与生产
拒绝、最终 Rust 检查均通过。同 SHA
[PR 41640531467](https://github.com/FlowToolsOrg/flowtools/actions/runs/41640531467)
与 [push 41640518873](https://github.com/FlowToolsOrg/flowtools/actions/runs/41640518873)
也均 completed / success。这不替代当前队列处置修改的整仓验收。

普通 Host/review/pins 不改变。已执行或未知原生历史的显式恢复/处置、
健康激活、GC/LKG、工具正式发行/独立审核，以及 G7 文件事务/媒体发布
继续 pending，见 [G7 矩阵](./g7-completion-matrix.md)。

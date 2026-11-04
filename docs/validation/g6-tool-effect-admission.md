# G6 固定媒体动作的 T0 当前调用准入

日期：2026-11-01。状态：prototype；专项验证中，完整门禁另记。

`runtime_media.rs` 将固定媒体动作的权限描述符与当前任务实际持有的工具
引用结合。仅明确配置工具 authority 的固定 T1 T0 构造可进入；普通产品
Host 仍无 authority，生产 review profiles/pins 为空。没有新增 IPC、SDK
方法、Tauri command、路径、argv、Host selector 或原生进程/文件 adapter。

`Scope::ToolLock` 仍是精确声明/grant key，不是已准入 artifact，也不是 G4
语义 digest。后者包含 Manifest 自身，不能要求 Manifest 预先声明自己的
最终 digest。本路径由 Rust 固定映射 `ffmpeg` 槽与三个版本化动作；不是
由客户端选择 slot/pins。进入效果前另在当前 root 的不可变锁中选择实际
consumer 的唯一 ffmpeg pin，并要求当前 signed ZIP 与原 seal/文件身份、
字节、权限和持有句柄全部复核。不能借 root 的工具 pin 为无依赖的 provider
建立引用，也不能用同 ID 的其他版本、publisher 或未知字节替换。

检查顺序：实际 Running 的 root/service 会话；原 Manifest AND 当前 grant
及所有委托 hop 的 scope/epoch/预算早期拒绝；当前任务原字节记录；当前
工具重新准入与持有；记录再次比较；效果点前重新检查会话/期限/epoch、
原记录和管理授权；精确 consumer/pin 绑定；普通 broker 再检查并计一次
调用预算，才进入 T0 回调。未开始的服务、已结束任务与仅管理 Host 均拒绝。

返回的 `AdmittedMediaTool` 只在回调动态范围借用文件引用，只有有限动作/
程序角色与诊断 pins，不能序列化、克隆或取得 executable/path/capability。
它不能逃出回调成为可重用许可；异步效果须在 resume/commit 重新检查，
仍需未来原生 runner 的 owned PID/Job/drain 与输出发布事务。服务运行中
的能力拒绝保持 sticky failure，provider 不能忽略错误后报告成功。

专项覆盖十三个用例：固定动作/精确 pin、声明与 grant 的独立拒绝、
普通预算、IO 后撤销/替换授权/取消、未知任务记录、当前时钟/签名撤销、
所有 pending、provider 自有绑定、委托双方 scope、provider 撤销、sticky
failure、真实命令期限到达与管理恢复。canary 条目不被执行，回调只观察
元数据。此页不把尚未运行的测试或前一提交的门禁当作本实现成功。

首次十三项实际为 **12 pass / 1 fail**（98.91s）：已记录能力拒绝的服务仍可
返回成功。原因是 `finish_service` 只在 provider 返回失败时采用已记录的
能力错误。现改为完成前统一采用该错误，成功结果不能覆盖拒绝。相同用例
扩展覆盖新的媒体回调和现有 `invoke_service` 路径，**1 pass / 0 fail**
（两条路径，6.98s）。原失败与修复后日志分别为
`execution-validation/g6/tool-effect-admission-focused-20261101.log` 和
`tool-effect-admission-sticky-green-20261101.log`。没有删除用例或放宽断言。

定向复验准备阶段曾误选 Desktop Cargo workspace，随后测试新增分支缺少
模块导入，两次均在执行前失败，日志另存 `sticky-green-20261101-wrong-workspace`
与 `sticky-green-20261101-missing-test-import`；修正测试调用和导入后才得到
上述实际通过结果。

随后扩展租约组 **41 项全部通过**（250.14s，原二十八项加新增十三项），
包括六个真实进程退出 barrier、实际 LPAC/Job 排空、复制/恢复和完整任务
记录比较。原 broker 十项通过（0.11s），服务恢复三项通过（0.86s），十项
compile-fail 通过（2.31s），包括借用上下文无法逃逸；Core 实际单元 inventory
为 453。all-target Clippy `-D warnings` 通过（14.78s）。日志前缀为
`execution-validation/g6/tool-effect-admission-`，对应 `leases`、`broker`、
`service-recovery`、`doctests`、`inventory` 和 `clippy`，日期均 `20261101`，
均 exit 0。服务恢复首次过滤用了源文件名，实际收集零项，`services` 日志
保留且不计成功；改用实际模块名并要求恰好三项，才得到上述恢复结果。

Rust fmt、文档格式和 docs:check 通过；当前提交完整根门禁和远端结果另记。
G5/G6/G7 父项、accepted 旧版本续接、原生 runner、用户文件发布、真实发行/
法律审核和独立安全批准仍 pending，生产 review profiles/pins 保持空。

随后 `41fd82b` 完整本机根门禁 exit 0、最终无漂移：11/11 workspace
lint/types/test/build；Core 453+11+10（单元 1267.65s）；Runtime 原生 24+6
（43.81s/2.38s）及 Bun 17（含原 50 路冷启动）；Chromium 109/34；Desktop
106 Bun+Node 1+原生 36+6（3.75s/0.14s）；Web 23/Desktop 25 产物一致、
production refusal 和最终 Rust 检查通过。日志
`execution-validation/g6/root-ci-41fd82b-20261101.log` / exit 0。

远端 ref 已核对为 `41fd82b810bf1395df7814a1290eabeb1c8db8fb`；
[PR 41274144420](https://github.com/FlowToolsOrg/flowtools/actions/runs/41274144420) /
[push 41274141160](https://github.com/FlowToolsOrg/flowtools/actions/runs/41274141160)
当前运行中。此前 `30ce0a0` 的两路远端已 success。

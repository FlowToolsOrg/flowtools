# G7 完成矩阵与剩余依赖

日期：2026-11-02。G7 整体为 **in-progress / prototype**。

Core [有限媒体进度](./g6-native-media-progress.md) 区分转换、独立验证与完成；
每次观察仍检查当前授权，原始 stderr 保密，重复查询不收取结果或消耗新调用。
它不是用户文件媒体入口，GUI/CLI 发布、工具激活/GC 继续 pending。

既有 provider [恢复日志边界](./g6-provider-journal-fence.md) 增加固定槽位的
有界读取、原字节前值、trigger-aware 事务与确认回滚恢复。它是版本切换前置
的故障拒绝加固；工具激活、GC/LKG 及用户文件/媒体闭环继续 pending。

P2.8c3 的 [固定健康检查](./g6-native-tool-health.md) 复用原授权/排空链，
对固定合成 PCM 的真实转换及 probe 结果逐样本核对，再作当前 floor 检查。
当前两项 Core/五项原生通过，完整门禁另记；版本激活与 G7 执行闭环仍 pending。

P2.8c2 增加 [工具库存管理](./g6-tool-inventory-management.md) 的共享 Runtime
查询、GUI 与 CLI 只读入口；普通 Host 明确返回尚未配置，查询不启用执行。
当前自动验证另记，文件事务/撤销、媒体发布、更新/GC 等实际闭环仍 pending。

P2.8c1 在 [有限续接](./g6-live-tool-continuation.md) 实现单次已在途调用的
writer/descriptor/任务绑定，普通旧版本准入不放宽，不恢复排队任务或用户 grant。
当前签名与 39 项专项已通过，整仓另记；更新/GC、用户文件及 GUI/CLI 闭环继续 pending。

P2.8b8 增加 [Core 有界 WebM](./g6-native-video-transcoding.md)：固定管道输出、
独立新鲜输入的全帧解码检查和原完整授权/排空路径。未知时长保持未知，原
V1 完成条件不放宽；当前修改树验证另记，用户文件及 GUI/CLI 发布仍 pending。

P2.8b7 新增 [Core 有界 WAV 提取](./g6-native-audio-extraction.md)：一个调用
预算、两个实际 LPAC/Job、硬 stdout/progress 预算、独立同字节 probe 和当前
授权/信任复查。6/10/8 专项及固定 Chromium 逐 PCM 解码通过，当前整仓另记；
用户文件 grant、WebM adapter、GUI/CLI 发布/撤销/恢复仍未完成。

P2.8b6 接入 [Core 原生媒体信息检查](./g6-native-media-inspection.md)：有界
T0 输入、固定 ffprobe、独立 LPAC/Job、持久 phase 和逐阶段授权/信任复查。
当前仍无用户文件 read/write grant、转换输出发布或 GUI/CLI 媒体闭环，普通
产品 Host 无 tool authority；专项与本次整仓门禁分别记录，不能称 G7 完成。
前一步 `bb5ec8c` 本机完整门禁及两路远端已 success，详见上述记录。

P2.8b5 的 [实际安装媒体候选](./g6-installed-media-candidate.md) 专项六项通过
（43.08s），在独立非发布 workspace 中连接签名 fixture ZIP、密封文件、
任务持有、当前工具准入与真实 LPAC；原候选六项复验也通过（3.31s）。
这是同步测试回调的验证，产品异步 adapter、用户文件发布与实际 G7 闭环
仍 pending，当前整仓门禁另记。

P2.8b4 新增 [当前 T0 工具调用准入](./g6-tool-effect-admission.md)，将实际
consumer 的固定动作、不可变 pin/held image 与当前权限及任务原记录结合；
借用上下文不提供 executable/path，普通产品 Host 无 authority。原生媒体
进程、异步撤销/输出发布和文件提交/撤销仍未闭环，专项与完整门禁另记。

该准入提交 `41fd82b` 随后完整本机根门禁 exit 0、最终无漂移：Core
453+11+10（1267.65s）、11/11 workspace、Runtime 24+6/Bun 17、Chromium
109/34、Desktop 106+Node 1+原生 36+6、Web 23/Desktop 25 及最终 Rust
检查通过；当前两路远端在上述专项另记，前一步 `30ce0a0` 两路已 success。

2026-11-01：工具任务的 [已提交整行比较](./g6-tool-job-metadata-fence.md)
修复跨事务 SQL 改动被后续缓存写入覆盖的问题；原四项实际失败，修复后
相同四项通过。初次 create-only、未知状态保留文件/输入，摘要不授予权限。
该改动不开放原生执行或关闭 G7，当前完整门禁另记。

任务记录修复 `30ce0a0` 随后完整本机根门禁 exit 0、最终无漂移：Core
440+11+9（1159.42s）、11/11 workspace、Runtime 24+6/Bun 17、Chromium
109/34、Desktop 106+Node 1+原生 36+6、Web 23/Desktop 25 和最终 Rust
检查通过。同 SHA 两路远端结果在上述专项中另记；`24981c1` 两路已 success。

P2.8b3 新增 [固定媒体完成投影](./g6-media-completion-projection.md)，十七项
Core 媒体契约与原六项真实 LPAC driver 已过。它联合 exit/progress/probe/
大小，不证明 fresh 输出身份、Job drain、grant 或发布；原生 Runtime 工具
adapter 和 G7 实际媒体闭环仍未接入。

媒体投影提交 `24981c1` 随后完整本机根门禁 exit 0、无漂移：Core 431+11+9
（1281.27s）、11/11 workspace、Runtime 24+6/Bun 17、Chromium 109/34、
Desktop 106+Node 1+原生 36+6、Web 23/Desktop 25 与最终 Rust 检查通过；
同 SHA 的远端结果在上述专项记录中另记。此前 `4887b40` 两路远端已 success。

P2.8a9 新增 [任务 artifact 保留](./g6-accepted-tool-artifacts.md)：明确 T0 固定
T1 任务在 accepted 前持有原始 signed ZIP 与已有 seal，同原任务行持久化。
两个消费方共享原生句柄，未知/Interrupted 保留根，终态/后代排空后释放；
复制/恢复、损坏引用、撤销与新旧版本共存已有专项。普通产品 Host 不配置
authority，生产 review 为空；accepted 旧版本 continuation、原生媒体 adapter、
更新/GC 和 G7 文件提交/撤销/媒体发布仍待完成。当前完整门禁另记。

任务保留提交 `4887b40` 随后完整本机门禁 exit 0、无漂移：11/11 workspace，
Core 425+11+9（1129.28s）、Runtime 24+6/Bun 17、Chromium 109/34、Desktop
106+Node 1+原生 36+6、Web 23/Desktop 25、最终 Rust 检查通过。相同 SHA
首次未指定约定 target 导致 Desktop 绑定定位失败，失败与完整重跑日志均
保留；两路远端结果在上述专项记录中另记。

工具 accepted leases 前的检查发现旧安装事务存在“SQL 替换后又清除
journal”的缺口。四个安装域及其恢复路径现统一接入有限元数据保护；
旧实现六项实际失败、修复后同六项及五项 SQLite/四项旧 SQL 专项通过，
Clippy 和实际 inventory 406 项核对通过，见
[安装 SQL 边界](./g5-install-metadata-guards.md)。不据此关闭 G5/G6 或 G7。
该修复随后以 `060f235` 完整本机门禁通过（Core 406+11+9，单元 976.02s，
11/11 workspace、Runtime 24+6/Bun 17、Chromium 109/34、Desktop 106+Node 1+
原生 36+6、Web 23/Desktop 25、最终 Rust 与无漂移）；同 SHA 的
[PR 41196464140](https://github.com/FlowToolsOrg/flowtools/actions/runs/41196464140) /
[push 41196459042](https://github.com/FlowToolsOrg/flowtools/actions/runs/41196459042)
也均为 success。

G6 P2.8a8 新增 [消费方孤儿恢复](./g6-tool-consumer-reconciliation.md)：只读计划、
整批复核、实际 held-source callback 元数据检查、create-only 隔离与显式
续接/作废，保留所有字节/旧锁。专项二十项、完整 Core 395+11+9（单元
1051.68s）、all-target Clippy 与文档检查通过。随后 `b45f960` 完整本机门禁
exit 0、最终工作区无漂移：11/11 workspace、Core 395+11+9（1019.92s）、
Runtime 24+6/Bun 17、Chromium 109/34、Desktop 106 Bun+Node 1+原生 36+6、
Web 23/Desktop 25 产物一致及最终 Rust 检查通过。同 SHA
[PR 41177618924](https://github.com/FlowToolsOrg/flowtools/actions/runs/41177618924)
与 [push 41177615465](https://github.com/FlowToolsOrg/flowtools/actions/runs/41177615465)
均 SUCCESS。accepted leases、产品执行和 G7 文件/媒体闭环仍未完成。

G6 P2.8a7 新增 [实际消费方安装锁](./g6-tool-consumer-installation.md)，同一 writer
将签名插件/service 图与实际 held 工具 ZIP 绑定到独立惰性域；旧格式、旧恢复
摘要和 transitive pins 保留。accepted leases 与 G7 文件/
媒体执行仍待完成，不能将该安装收据计为办公闭环。
该修改专项 18+2、Core 全套 375+11+8、all-target Clippy 与文档检查通过；
`6422974` 完整本机根门禁随后 exit 0、最终工作区无漂移：11/11 workspace、
Runtime 原生 24+6/Bun 17、Chromium 109/34、Desktop 106 Bun+Node 1+原生
36+6，Web 23/Desktop 25 产物一致和最终 Rust 检查通过。
同 SHA [PR 41171644146](https://github.com/FlowToolsOrg/flowtools/actions/runs/41171644146)
和 [push 41171640372](https://github.com/FlowToolsOrg/flowtools/actions/runs/41171640372)
均 SUCCESS。

`7c81a45` 的本机完整门禁在 Chromium 连接中断处失败；原样完整浏览器
专项复验 109/109 通过，首次中断根因未确定。该 SHA 的 PR/push 远端均在
Core 351 pass / 4 fail 停止，四例共用的部分树 fixture 未明确指定 T0
owner。现修正测试构造并验证原生 owner 与未变的 DACL/label；产品 owner
拒绝保持。修正后的完整检查另记，详见 [恢复记录](./g6-tool-image-reconciliation.md)。
修正后本机全部二十一项恢复专项及 Core all-target Clippy 通过；完整本机
根门禁 `9f60332` 随后 exit 0，最终工作区无漂移：11/11 workspace、Core
355+11+7、Runtime 原生 24+6/Bun 17、Chromium 109/34、Desktop 106 Bun+
Node 1+原生 36+6、Web 23/Desktop 25 产物一致及最终 Rust 检查通过。
该 SHA 的 [PR 41168248139](https://github.com/FlowToolsOrg/flowtools/actions/runs/41168248139)
和 [push 41168245056](https://github.com/FlowToolsOrg/flowtools/actions/runs/41168245056)
均 SUCCESS，历史 `7c81a45` 失败保留。

G6 P2.8a6 新增 [工具文件孤儿恢复](./g6-tool-image-reconciliation.md)，
先隔离并撤销 artifact capability，再核对原始 ZIP；不收编、不产生 accepted
leases 或执行身份。二十一项专项、既有密封十八项、七项 compile-fail 和
Clippy 通过；本提交完整门禁另记，G7 文件/媒体仍未闭环。

撤销修复 `ec973a4` 的完整本机根门禁 exit 0，最终工作区无漂移：11/11
lint/types/test/build、Core 334+11+6、Runtime 24+6/Bun 17（原 50 路冷启动）、
Chromium 109、Desktop 106 Bun+Node 1+原生 36+6，Web 23/Desktop 25 产物
一致。此前 `ba10b39` 的失败记录保留，详见 [durable jobs](./g3-durable-jobs.md)。

G6 新增 [工具 ZIP 显式恢复核对](./g6-tool-reconciliation.md)，只隔离经完整
计划绑定的惰性孤儿；密封文件孤儿、accepted leases、产品执行和 G7 文件/
媒体闭环继续等待。该修改十八项专项、既有 G5 恢复十九项、Clippy 和六项
compile-fail 已通过。`ba10b39` PR/push 均 SUCCESS；本机完整门禁在 Todo
出队撤销的错误码竞争处失败，后续 Desktop/build/产物检查未运行。该问题已
由 `ec973a4` 稳定复现并修复，上方记录其整套本机验收，见
[durable jobs](./g3-durable-jobs.md)。

前置签名绑定 `b3db65c` 的完整本机根门禁 exit 0，Core 313+11+5、11/11
工作区与生产产物/最终干净检查通过；该 SHA 的 PR/push 均 SUCCESS。
首次本机 Clippy 退出 4、无修改重跑通过的记录保留于
[签名工具绑定](./g6-signed-tool-bindings.md)。

前置 `554bde6` 密封落盘已通过完整本机根门禁，Core 306+11+4、Runtime
原生 24+6/Bun 17、Chromium 109、11/11 工作区与生产产物/最终干净检查通过。
远端 PR/push 均 SUCCESS；详见 [密封落盘记录](./g6-tool-materialization.md)。
后续 [签名插件/工具绑定](./g6-signed-tool-bindings.md) 提供独立 ZIP 锁域，
旧 profile 与 G4 不变；它仍不建立 accepted leases 或可执行身份。

更早的真实媒体候选提交 `c24b274` 的完整本机根门禁 exit 0，
[PR 40879978485](https://github.com/FlowToolsOrg/flowtools/actions/runs/40879978485)
和 [push 40879967308](https://github.com/FlowToolsOrg/flowtools/actions/runs/40879967308)
均 SUCCESS。首次本机 standalone copyfile 失败及未修改原测试的复验保留在
[真实媒体候选记录](./g6-media-candidate.md)。
后续 P2.8a3 新增 [私有密封工具文件](./g6-tool-materialization.md)，专项具备
原生 ACL/身份、真实 LPAC 共享读取与隔离撤销；仍不提供 accepted leases 或
产品可执行身份，不把这一步计为 G7 媒体闭环。
前置归档事务提交 `ab73889` 的完整本机根门禁 exit 0，
[PR 40868819104](https://github.com/FlowToolsOrg/flowtools/actions/runs/40868819104)
和 [push 40868809341](https://github.com/FlowToolsOrg/flowtools/actions/runs/40868809341)
均 SUCCESS。后续新增 [真实媒体候选专项](./g6-media-candidate.md)，已具备
具体源码锁/构建/测试材料，仍不是产品受管工具身份、accepted leases 或办公闭环。

G6 后续新增 [惰性工具归档库存/journal](./g6-inert-tool-installation.md)，仍未
解包为可执行工具或建立 accepted leases；不把归档收据计为媒体闭环。
原生 feasibility 修复 `8d62d87` 的
[PR 40846856086](https://github.com/FlowToolsOrg/flowtools/actions/runs/40846856086)
和 [push 40846845674](https://github.com/FlowToolsOrg/flowtools/actions/runs/40846845674)
均已成功；下方 `955d88d` 的远端失败记录保留，不再作为最新状态。
维护者已豁免剩余 G7 人工验收，不再重复请求；豁免不改变自动门禁和 G5/G6
执行边界，也不能替代实际办公闭环。

后续 `da495e1` 的完整本机根门禁通过，最终工作区无漂移；对应远端
[PR 检查 40825126648](https://github.com/FlowToolsOrg/flowtools/actions/runs/40825126648)
与 [push 检查 40825119894](https://github.com/FlowToolsOrg/flowtools/actions/runs/40825119894)
均成功，Chromium 增至 109 项。它们取代下方当时的待验证状态，历史失败不删除。
维护者随后要求继续实现 G5/G6；新增
[Windows 原生隔离可行性](./g5-windows-native-isolation.md) 正在补齐平台前置，
不能把测试专用进程边界称为真实工具库存或 G7 媒体闭环。
另新增 [固定媒体 recipes/投影](./g6-media-contracts.md)，只涵盖参数与解析；
未启动真实媒体工具，不把十项纯契约回归计为媒体办公闭环。

`955d88d` 本机完整门禁 exit 0：11/11 lint/types/test/build；Core 272+11+2、
Runtime 24+6 原生、Desktop 105 Bun+Node 1+Rust 36+6、Chromium 109、产物与
最终干净工作区通过。远端 [40842642460](https://github.com/FlowToolsOrg/flowtools/actions/runs/40842642460)
失败于新增 child-policy 错误码断言（Server 2022=5，本机 Windows 11=367）。
后续修复补充实际策略/Job 容量回读与两进程正向条件，保持真实拒绝与 marker
不存在断言；该修复的最终远端仍须完成。见 [平台记录](./g5-windows-native-isolation.md)。

本轮 `9e7c13d` 完整本机根门禁通过：11/11 工作区 lint/types/test/build，
SDK 791、Core 262+11+2、Runtime 24、Desktop 36+6、Chromium 108，
生产产物对照及最终干净工作区通过。日志
`execution-validation/g7-shell/root-ci-9e7c13d-20261029.log`。
同提交[远端 40821067976](https://github.com/FlowToolsOrg/flowtools/actions/runs/40821067976)
失败：Core 260 pass / 2 fail，通知前返回 Timeout，另一调度用例取不到运行任务。
两例使用 1 秒任务期限；没有修改期限或错误断言。Core 原生 harness 改为串行
执行完整用例以隔离案例之间的负载，保留 inventory、内部真实并发及全部拒绝
路径；新增 CI 契约回归校验完整脚本与既有 1 秒 fixture 声明。此调整仍须最终整仓/
远端验证，不将本机通过冒充远端成功。首次并行运行 CI 契约专项还在原 5s
`cargo metadata` 用例失败，独立重跑也失败，原记录保留。实际探针：Node
执行同一命令约 219ms；Bun 异步 spawn 调用本身约 10,112ms，总计
10,246ms，同步调用另一次约 4,997ms。底层 Bun/OS 原因未确定。
该实际 Cargo 检查移到测试专用 Node companion，保持原 5s 总期限、真实
PID/退出码、有效 workspace、输出上限和精确 1 用例计数；Bun companion
继续使用原 TOML 解析及完整 profile 等值断言，两个 suite 必须均通过。
Desktop 原 Bun suite 后以 `&&` 必跑 Node，再执行原 Rust inventory/全部原生
测试；Node 失败保持 fatal。新增独立严格 JS types/lint，未更改生产 Host、
配置、workflow、产品期限或 Job/lease/drain 断言。

修复后专项：Core 串行全套 262+11+2 通过（单元部分 283.34s，包含两条
原 1s 调度用例）；CI 契约 16 项、实际 Cargo Node 1 项、Chromium 调度
8 项及新增 Node JS types/lint 均通过。Node 只负责实际 workspace 检查，
Bun 继续解析 TOML 的完整 profile；不以文本匹配替换原语义校验。

| 范围             | 当前实际交付                                                                                                          | 未完成条件                                                                                            |
| ---------------- | --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| P3.1a 桌面壳     | 托盘、单实例/快捷键恢复、显式自启动、外观、状态/诊断/托盘快照；工程范围已收尾                                         | 人工项 waived，成熟度不提升                                                                           |
| P3.1b 日程/提醒  | GUI/CLI 创建、查看、修改、取消，UTC/IANA/DST/漏触发策略、有限周期、私有输入、durable claim、后台授权和无 GUI 实际提醒 | 安装后 OS 展示不以 requested 冒充；跨平台支持按实际状态返回                                           |
| P3.2 搜索/上下文 | SDK/UI/Desktop/CLI 同源拼音/模糊/别名/有界正则、解释/置顶/历史隐私、显式上下文预览后普通执行                          | 不作为文件/媒体工作流证据                                                                             |
| P3.4a 文件整理   | NTFS 只读规划/整批复核；单写者持久 plan ID、DPAPI 私密定义、取消与恢复作废                                            | G5/G6 执行门槛；GUI/CLI 同计划提交、逐效果授权/journal、重试和未修改文件撤销                          |
| P3.4a 媒体处理   | signed ZIP/镜像/任务保留；T0 信息/WAV/WebM actor、进度、续接、健康与只读库存                                          | 正式审核/签名发行、版本激活/队列/GC；两个媒体插件与 GUI/CLI 用户输入/输出发布、空间不足/更新/恢复回归 |
| 自动验收         | 专项与完整门禁逐次记录                                                                                                | 当前 HEAD 的整仓/远端检查必须完成；专项成功不覆盖历史失败                                             |

具体证据：[桌面收尾](./g7-shell-closeout.md)、
[调度派发](./g7-scheduling-dispatch.md)、
[编辑/恢复](./g7-scheduling-edit-recovery.md)、
[本地提醒](./g7-local-reminders.md)、[搜索](./g7-launcher-search.md)、
[文件只读准入](./g7-file-plan-admission.md)、[计划持久化](./g7-file-plan-store.md)。

## 不可跳过的真实前置

`packages/runtime-core/src/packages/tools.rs` 的 `REVIEWED_TOOLS` 仍为空，
只有测试条件下存在 fixture profile。Core 已有固定 T0 授权的实际受限媒体
actor；普通产品 Host 未配置工具 authority，不能据此开放用户文件入口。
真实 FFmpeg 构建候选已提供 source/hash/recipe/test 材料；其
publisher/source/license/flavor/target/program/resource 审核、实际签名发行和独立
安全验收仍无通过证据。安装共享、accepted 保留和实际 LPAC 链已有上述原型
证据；产品版本激活/队列策略、更新/LKG/GC 及 G5 完整平台执行/文件竞争/
恢复门槛仍待实施和验收，详见
[工具准入](./g6-tool-admission.md)、[G5 受保护发布](./g5-guarded-publication.md)
及 [实施依赖顺序](../next-milestones.md)。

不能用系统里任意 FFmpeg 路径、公开 fixture 密钥、子进程成功或操作系统 codec
替代受管工具协议；也不能把 inert archive proof 升级成 executable identity。
维护者可提供真实审核/发行材料以继续该路径，但本记录不代为签署法律/安全批准。
在前置满足前，文件写入/媒体执行入口保持关闭；这是实际依赖，不是等待 G7
人工验收或重复请求推送许可。

## 自动门禁记录

远端 `3c73bc12` 的
[Windows quality 40692322700](https://github.com/FlowToolsOrg/flowtools/actions/runs/40692322700)
实际通过 Core 243 + 11 + 2、Runtime 24 原生测试及 34 个浏览器测试文件；
10/11 workspace test 成功，最终 Desktop 测试发现提醒命令生成文档过期。
已从实际编译契约重新生成并验证该文档，未刷新产物或绕过 drift gate。

同提交的独立本机根门禁因 CLI 搜索测试在一个默认 5s 用例里累计两个实际
子进程而超时，100 pass / 1 fail / 1 error。拆分为两个独立语义用例，保留每个
用例的 5s 限制、真实子进程、退出码、JSON 和 source canary 拒绝断言；修复后
完整 CLI 102 项、346 assertions 通过。嵌套 worktree 的首次 Cargo workspace
定位失败单独保留，后续验证副本已移到仓库外。所有旧失败保留，不称为通过。
最新提交仍需完整根门禁与远端验收，不合并、部署或改仓库设置。

`29ec8a0` 的本机完整门禁再次在单独的 regex CLI 测试失败（9.39s，原限额
5s；101 pass / 1 fail / 1 error）。因此拆分不足以解决启动阻塞。实际诊断：
Bun 父进程启动 Node 子进程的 `spawn` 调用返回耗时 10,068ms；Node 父进程
启动同一真实 Bun CLI 的 spawn 54.9ms、完整退出 580.2ms。底层 OS/引擎原因
未确定，不能归因于搜索算法或擅自更改工具链。

两个搜索子进程用例现迁到严格类型检查的 Node test Host，仍启动实际 Bun CLI，
分别保留原 5s test deadline、真实 PID/退出码/JSON/source canary 断言，增加
输出上限、取消时终止自有子进程及准确的两用例计数。标准 CLI `test` 在原 Bun
suite 成功后以 `&&` 运行 Node suite，任何失败仍阻断根门禁；PowerShell 回归
同时验证 Bun/Node 的非零退出均阻止后续步骤。没有改变生产代码/执行期限、
Job/drain/lease 或任何 CI 缓存/权限设置。

Node 宿主的首次整合检查发现测试注册阶段提前等待首个测试会使全局计数 hook
在第二例注册前运行，已改为先注册完整两例再由 Node test runner 执行；两例
与最终计数均通过。CLI 原有 Bun 100 项 / 336 assertions 通过，Node 两例
最新分别约 517/531ms；types/lint、生成命令文档与实际 manifest，以及含
Bun/Node 失败传播的 17 项仓库回归通过。Core 最新完整 252 + 11 + 2、
all-target Clippy 通过。以上均为专项证据，修复树需再次执行完整根门禁。

`bf5f20a` 的完整本机门禁通过新 CLI 两例和 Core 252+11+2 后，在 Runtime
19/24 失败：deadline、malformed、oversized、provider-error、root-cancel
仍缺少真实后代 marker/排空边界。没有改旧期限或断言。七个异常注入 fixture
改用测试专用 Node 宿主驱动同一编译处理器，C 仍创建 hash 固定的真实 Bun
后代；正常服务与产品仍用 Bun。原样 15 项 native service 回归全部通过，
Job 内 PID 存活/退出、lease 与迁移拒绝断言保留。详见
[测试宿主与证据](./g7-runner-protocol-host.md)；底层 Bun 阻塞未宣称修复。

后续 `c329d2c` 的门禁发现 package-pin 正向对照在 Node 宿主启动前生成
绝对 deadline，宿主启动可能耗尽窗口。该无已接受 job 的测试请求现由实际
Node 宿主在提交时创建同样的 10s 窗口，并断言准确窗口及完成时间；保留
旧 pin 拒绝和新 pin import marker、30s test/25s watchdog、全部产品期限。
修复后 runner 4 项 / 58 assertions、types/lint 通过，详见上面的宿主记录。

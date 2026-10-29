# G7 完成矩阵与剩余依赖

日期：2026-10-29。G7 整体为 **in-progress / prototype**。

最新已交付提交 `c24b274` 的完整本机根门禁 exit 0，
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

| 范围             | 当前实际交付                                                                                                          | 未完成条件                                                                                                           |
| ---------------- | --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| P3.1a 桌面壳     | 托盘、单实例/快捷键恢复、显式自启动、外观、状态/诊断/托盘快照；工程范围已收尾                                         | 人工项 waived，成熟度不提升                                                                                          |
| P3.1b 日程/提醒  | GUI/CLI 创建、查看、修改、取消，UTC/IANA/DST/漏触发策略、有限周期、私有输入、durable claim、后台授权和无 GUI 实际提醒 | 安装后 OS 展示不以 requested 冒充；跨平台支持按实际状态返回                                                          |
| P3.2 搜索/上下文 | SDK/UI/Desktop/CLI 同源拼音/模糊/别名/有界正则、解释/置顶/历史隐私、显式上下文预览后普通执行                          | 不作为文件/媒体工作流证据                                                                                            |
| P3.4a 文件整理   | NTFS 只读规划/整批复核；单写者持久 plan ID、DPAPI 私密定义、取消与恢复作废                                            | G5/G6 执行门槛；GUI/CLI 同计划提交、逐效果授权/journal、重试和未修改文件撤销                                         |
| P3.4a 媒体处理   | 惰性 signed tool ZIP 库存；固定 recipes/projections；独立真实媒体构建候选专项                                         | 产品签名发行/可执行库存/lease/平台边界；两个插件共享工具与 GUI/CLI、用户输出发布、可播放结果、空间不足/更新/恢复回归 |
| 自动验收         | 专项与完整门禁逐次记录                                                                                                | 当前 HEAD 的整仓/远端检查必须完成；专项成功不覆盖历史失败                                                            |

具体证据：[桌面收尾](./g7-shell-closeout.md)、
[调度派发](./g7-scheduling-dispatch.md)、
[编辑/恢复](./g7-scheduling-edit-recovery.md)、
[本地提醒](./g7-local-reminders.md)、[搜索](./g7-launcher-search.md)、
[文件只读准入](./g7-file-plan-admission.md)、[计划持久化](./g7-file-plan-store.md)。

## 不可跳过的真实前置

`packages/runtime-core/src/packages/tools.rs` 的 `REVIEWED_TOOLS` 仍为空，
只有测试条件下存在 fixture profile。G6 现状是惰性准入，不是可执行工具库存。
真实 FFmpeg 构建候选已提供 source/hash/recipe/test 材料；其
publisher/source/license/flavor/target/program/resource 审核、实际签名发行和独立
安全验收仍无通过证据。工具共享 registry/lease、
更新/LKG/GC 及 G5 平台执行/文件竞争/恢复门槛仍待实施和验收，详见
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

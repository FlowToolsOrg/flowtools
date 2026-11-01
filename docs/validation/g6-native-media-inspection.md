# G6 Core 持有的原生媒体信息检查

日期：2026-11-01。P2.8b6，prototype；专项与最终整仓结果分别记录。
G5/G6/G7 父项保持未完成，G7 剩余人工验收仍为 waived。

`RuntimeCore::start_media_inspection` 接受最多 1MiB 的非空 T0 输入字节，
以 opaque、不可序列化的 actor handle 查询结果。仅支持固定 inspect-v1 /
ffprobe recipe；不接用户路径、可执行文件、argv、环境、shell、URL 或客户端
tool selector。普通产品 Host 没有 ToolAuthority，production review/pins 仍为空。
新原生边界在 `packages/tools/install/images/inspection/`，与仅测试存在的
旧 feasibility launcher 分开；没有将测试选择器或不受限正向对照移入产品。

## 当前检查和实际资源

每次工具调用通过普通全链 Manifest AND grant、identity、epoch、deadline、
背景策略和调用预算检查后计一次费用。私有 ToolCallClaim 无 Clone、Serde
或公开构造；准备、恢复线程、运行轮询和结果提交均重进当前检查，不重复
计费，也不能把耗尽预算的 claim 用于第二次调用。既有同步回调保持原语义。

实际 consumer pin 从 accepted immutable lock 独立解析。准备、恢复线程和
结果提交重新读取实际 signed ZIP、seal、原生身份/策略与当前信任。运行期间
持有真实 source/image 句柄，复查当前 durable trust、scope、pending 状态与
原任务完整行；收据属于本次 lease，不由共享旧 image 缓存更新或授予权限。

每次调用新建唯一 AppContainer、受保护的 Low 私有只读输入与独立 Job，
文件通过持有父目录的单组件 NT create-only 打开，拒绝 reparse/hardlink。
所有源句柄和输入句柄跟随实际进程，未知排空不得释放。固定 Job 只容一个
进程，512MiB process memory、25% hard CPU cap；全 Core 最多四项在途。
实际 LPAC/AppContainer SID、两个 enabled capabilities、child policy、loader
policy、owned image path、Job 限制和未继承私有对象都在 ResumeThread 前回读。
只继承三个 stdio handles；stdout/stderr 聚合上限 65536 字节，stderr 不进历史。
不继承 Host 环境，输入文件为 Low 且无执行权限，NoLowLabelImages 保持开启。

原生执行期限固定五秒，取消要求 owned process 已退出且 owned Job active=0，
观察期限为原 500ms。结果必须为零退出、实际排空及严格有界 ffprobe 投影，
提交与收取仍要求当前授权。提前结束 root/service 不得掩盖尚在途的工具调用。
Runtime 的既有 25ms maintenance 路径执行有限轮询，无客户端轮询也执行期限
和撤销检查；没有新增进程启动配置或 wire 操作。

## 持久化和拒绝

Preparing 在任何 OS profile、workspace 或进程创建前提交；Running 在恢复
线程前提交。只使用原 jobs.metadata writer、32KiB 界限、完整 predecessor
与 trigger guards。记录只含逻辑 UUID、consumer/archive pin、输入长度/hash、
deadline 和有限 phase；不保存原始输入、路径、PID、stderr 或结果。

每任务最多八次、profile 最多 64 次保留记录/目录及 64MiB 输入；没有隐式
删除、GC、收编或重试。整批读取实际持久行并核对 actor 原摘要后才允许使用
保留 ID，未知/损坏目录保留字节并拒绝。准备失败保守中断并停止 admission。
成功 owned drain 后才释放本次 native guards；未知 SQL 不得覆盖外来字节。
Interrupted/未知排空仍保留工具根和原私密输入，不能借终态清理隐藏失败。
重启只把未结束调用标为 Interrupted，不恢复 claim、不创建进程或重放输入。

## 验证

独立非发布 `managed-media-candidate` workspace 编译实际 Core 源代码，只在
cfg(test) 下使用公开确定性测试签名和原固定候选。原六项安装媒体测试保持，
另增加十项 Core 原生执行检查；driver 分别核对 6、10、总计 16 个实际名称，
运行 Clippy 后串行执行两组，缺失或多收集即失败。普通根 CI 不执行候选程序。

```powershell
$env:CARGO_TARGET_DIR='D:/code/projects/flowtools/apps/desktop/src-tauri/target'
pwsh -NoProfile -File scripts/test-managed-media-candidate.ps1 -BuildRoot <verified-build-root>
```

十项涵盖实际私有 WAV 信息检查和单次预算、两消费方独立 LPAC/Job 与排空、
恢复线程前撤销、未知整行 SQL、投影提交时撤销、重启保留中断、输入/未知目录
拒绝、当前签名撤销、损坏 retention 拒绝恢复和原五秒期限。六项 broker 回归
独立覆盖末次预算、撤销/同值重新批准、关闭、时间、祖先链、scope 和 foreign broker。

首次十项扩展前的八项运行 7 pass / 1 fail：重启测试误打开 runtime.sqlite3，
实际 fixture 使用 runtime.sqlite；修正真实路径后的重启单项 1 pass（9.66s）。
早先编译错误和 Clippy 的 slice-size/loop-counter 拒绝日志保留；按实际代码
修复，没有降低 lint、期限、PID/Job 或预算断言。最终十项和整仓门禁另记。

首次完整扩展 driver 的原六项 6 pass（43.56s）；新十项 9 pass / 1 fail
（110.31s）。损坏 retention 已被拒绝，但测试预期 StoreCorrupt；实际现有
ToolAuthority catalog setup 先验证 retained roots，稳定映射为
DependencyArtifactMismatch。修正为该精确拒绝码，继续断言原记录/输入/工具
字节不变；没有放宽为任意错误或修改生产错误映射。原失败日志保留。

修正后完整显式 driver exit 0：原六项 **6 pass / 0 fail / 0 ignored**
（49.84s），Core 原生十项 **10 pass / 0 fail / 0 ignored**（124.79s），
实际 6/10/16 inventory、格式、探针构建及 all-target Clippy `-D warnings`
（8.61s）通过。日志 `execution-validation/g6/native-inspection-full-driver-retry-20261101.log`。
随后新增 Host 排空计数与 fatal 退出路径，并以实际信息检查补验计数；此
Host 集成和当前 Core 源仍须完整根门禁，不把独立候选当作普通 Runtime 验收。

构建图/fixture 拒绝契约 **1 pass / 246 assertions**，T0 入口/测试钩子/
Host fatal-drain 契约 **1 pass / 10 assertions**，严格 TypeScript 通过。
初次单项正则收集零项和新契约遗漏本地 read helper 均失败并保留日志；
补齐实际名称/既有读取方式后通过，没有放宽无测试失败规则。

最后 Core all-target Clippy `-D warnings` 通过（19.67s），实际 drain count
补验 **1 pass**（11.05s），主工作区 Runtime all-target `cargo check --locked`
通过（19.56s），文档契约与严格 TypeScript 通过。主工作区二十项 CI 契约
首次 **19 pass / 1 fail**：既有 CSS lint 子进程用例超过原 30s（37.01s），
Native 新增契约均通过。该重复的本机启动问题需单独修复/验证，保留原失败；
不提升原 30s 或任何产品期限。当前提交尚无完整根门禁通过声明。

后续独立修复将既有 CSS 回归移至已经必跑的测试专用 Node companion，仍
启动实际 Bun x oxlint，按 root/consumer 分别执行已定义类成功、未知类拒绝，
四次实际执行和 no-unknown-classes 断言均保留。原 30s 总界限不变；增加
每次剩余时间、真实 PID、无信号/启动错误和聚合输出上限检查。Cargo 实际
解析仍保持原 5s，两个用例必须全部完成；Desktop 既有 fatal &&、严格 JS
types/lint 和生产 Host/工具链/期限均保持。

Node companion **2 pass / 0 fail**：CSS 四次共 1.76s、Cargo 391ms，日志
`execution-validation/g6/css-lint-node-companion-initial-20261101.log`。
这验证测试宿主调整，不宣称已解决底层 Bun/OS 启动阻塞。完整根门禁另记。

随后主 Bun 二十项 **20 pass / 0 fail**（44.29s，617 assertions）；Node 两项
再次通过，CSS 1.98s / Cargo 395ms。JS 增加 unknown 与实际结构检查来保留
严格类型检查，没有使用 any、ts-ignore 或 lint suppression。

## 仍未交付

当前输入是有界 T0 字节，尚不是用户文件选择/read grant 或 GUI/CLI 办公流程。
转换、fresh 输出持有与独立发布/撤销/恢复事务、可播放交付、accepted 旧版本
continuation、更新/LKG/GC、真实签名发行/完整 source offer/许可证审核和独立
安全批准仍待完成。不把本路径或公开 fixture key 作为 production 工具准入。

前一步 `bb5ec8c` 本机完整根门禁 exit 0、最终无漂移：Core 453+11+10、
11/11 lint/types/test/build、Runtime 24+6/Bun 17、Chromium 109/34、Desktop
107+Node 1+原生 36+6、Web 23/Desktop 25 和最终 Rust 检查通过。同 SHA
[PR 41304328425](https://github.com/FlowToolsOrg/flowtools/actions/runs/41304328425)
与 [push 41304322289](https://github.com/FlowToolsOrg/flowtools/actions/runs/41304322289)
均已 success；这不代替当前修改树的完整门禁。

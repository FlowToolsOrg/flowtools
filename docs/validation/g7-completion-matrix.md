# G7 完成矩阵与剩余依赖

日期：2026-10-28。G7 整体为 **in-progress / prototype**。
维护者已豁免剩余 G7 人工验收，不再重复请求；豁免不改变自动门禁和 G5/G6
执行边界，也不能替代实际办公闭环。

| 范围             | 当前实际交付                                                                                                          | 未完成条件                                                                                                                     |
| ---------------- | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| P3.1a 桌面壳     | 托盘、单实例/快捷键恢复、显式自启动、外观、状态/诊断/托盘快照；工程范围已收尾                                         | 人工项 waived，成熟度不提升                                                                                                    |
| P3.1b 日程/提醒  | GUI/CLI 创建、查看、修改、取消，UTC/IANA/DST/漏触发策略、有限周期、私有输入、durable claim、后台授权和无 GUI 实际提醒 | 安装后 OS 展示不以 requested 冒充；跨平台支持按实际状态返回                                                                    |
| P3.2 搜索/上下文 | SDK/UI/Desktop/CLI 同源拼音/模糊/别名/有界正则、解释/置顶/历史隐私、显式上下文预览后普通执行                          | 不作为文件/媒体工作流证据                                                                                                      |
| P3.4a 文件整理   | NTFS 只读前缀改名规划、整批内容/身份/目标复核与拒绝回归                                                               | G5/G6 执行门槛；持久 plan ID、GUI/CLI 同计划提交、逐效果授权/journal、重试和未修改文件撤销                                     |
| P3.4a 媒体处理   | 现有 G6 惰性 signed tool admission                                                                                    | 真实受管工具发行/库存/lease/平台边界；两个插件共享工具、inspect/extract/transcode、实际可播放结果、进度/取消/空间不足/更新回归 |
| 自动验收         | 专项与完整门禁逐次记录                                                                                                | 当前 HEAD 的整仓/远端检查必须完成；专项成功不覆盖历史失败                                                                      |

具体证据：[桌面收尾](./g7-shell-closeout.md)、
[调度派发](./g7-scheduling-dispatch.md)、
[编辑/恢复](./g7-scheduling-edit-recovery.md)、
[本地提醒](./g7-local-reminders.md)、[搜索](./g7-launcher-search.md)、
[文件只读准入](./g7-file-plan-admission.md)。

## 不可跳过的真实前置

`packages/runtime-core/src/packages/tools.rs` 的 `REVIEWED_TOOLS` 仍为空，
只有测试条件下存在 fixture profile。G6 现状是惰性准入，不是可执行工具库存。
真实 FFmpeg 构建的 publisher/source/license/flavor/target/program/resource
审核、实际签名发行和独立安全验收尚无输入或通过证据。工具共享 registry/lease、
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

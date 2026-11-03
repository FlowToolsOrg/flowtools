# 多消费方目录整批切换

日期：2026-11-03。范围：Core T0 的固定目录整批更新。G7 保持
**in-progress / prototype**；人工项 waived，不计 passed。

## 行为与边界

原 `stage_provider_update` 一次只允许替换一个成员。两个消费方同时固定到
新工具摘要时，需要在同一次迁移后切换完整目录；逐个提交会得到混合版本。
新增 `runtime_provider_cohorts.rs`，提供不透明的 stage/begin/commit/abort。
计划绑定 Core 实例、旧/新目录摘要和原有每个 provider 的完整变更计划。
只允许相同成员和 publisher，实际变化的成员必须提升版本；确认不授予执行权。

最多 64 个候选/变更成员和 64 份历史目录，继续使用原 provider lease、
accepted root、service call 和实际 owned drain。begin 重查整个已接受图；
进入中途失败只取消本次已进入的成员。每个旧成员都排空前不备份、不迁移。
单成员 API 不能拆开一个已暂存整批变更。旧目录只用于计算当前 held 引用的
排空计划，不重新准入旧工具，也不降低 release floor。

commit 重新解析全部新成员。涉及工具时，通过原 `ToolAuthority.prepare`
持有当前签名 ZIP/原生镜像，经过所有候选（含未使用候选）的现行检查；不
借用旧目录或旧任务的执行凭据。候选只有归档、缺少当前镜像时，在迁移回调前
拒绝；可以显式完成 materialization 后重试同一尚未提交的计划。

整批仅做一次备份，复用 `runtime_service_journal.rs` 的固定槽位和 format 1
日志。准备、最终提交及原字节恢复仍使用同一个 writer 和原事务守卫；有界
TEXT、精确前值、实际 row/trigger 计数和事务内 readback 不变。单成员迁移
也复用该实现。只有确认数据库回滚与精确日志恢复后，才解除所有本次排空。
未知提交/恢复停止准入并保留 prepared 状态，重启对旧/新目录均拒绝自动继续。

整批回调前后和日志写后还比较有界 grants、策略、信任/安装域快照；不能用
业务迁移 SQL 偷换先前验证的 authority。成功后一次更换目录及已经验证的
依赖锁，原 grants 不复制给新包。后续任务仍经过普通当前授权和 artifact
准入。旧镜像、历史任务与 floor 保留；失败不将新 floor 回退到旧版本。

没有新增 IPC/配置/Host selector、生产 review、签名发行、健康激活、排队
任务续权、LKG 或 GC。固定健康观察仍只是诊断；不能把目录提交称为正式
工具发行或 G7 用户文件/媒体发布完成。

## 验证

- Core 迁移/恢复 26 项通过（新增十一项、原十五项），覆盖两成员单次提交、
  每个旧 root 排空、确认失效、原字节回滚、未知提交、禁止拆分、版本/身份/
  成员拒绝、策略及工具元数据替换、跨实例和重叠暂存；后两项增加有界授权
  行与恢复触发器失败，26 项 9.73s。十二项 compile-fail 文档测试（新增一项
  禁止整批计划序列化）及 all-target Clippy（6.38s）通过。
- 首次运行 23 pass / 1 fail：新测试误将“未变更成员”当作“同版本内容变化”。
  修正该测试的名称内容，使其确实携带同版本变化；原拒绝断言保留，随后
  24 pass / 0 fail。没有放宽产品约束或原十五项测试。
- 独立实际媒体 workspace 新增四项全部通过（53.46s），all-target Clippy
  （20.00s）通过：两个旧消费方都结束前拒绝迁移；两个新消费方分别重新
  授权并经实际 WAV/独立 probe 健康检查；缺镜像在迁移前拒绝，显式补全
  后可重试；失败恢复不降 floor/不删除旧镜像；重启只接收精确新目录且
  不复用旧 grants。actual PID/Job 都在收取前确认已排空。driver 保留原
  6/10/8/9/6/5/5 分组，另外精确四项，总计 53。CI driver 契约 1 pass /
  269 assertions 和严格 TypeScript 通过；完整原生/根门禁与远端分别记录。

当前未完成的检查不计 passed。日志位于忽略目录 `execution-validation/g6/`，
前缀 `provider-cohort-`。后续边界见 [G7 完成矩阵](./g7-completion-matrix.md)、
[原日志守卫](./g6-provider-journal-fence.md) 和 [工具健康](./g6-native-tool-health.md)。

随后完整实际媒体 driver **exit 0**：原六项 16.70s、信息十项 54.21s、WAV
八项 55.92s、WebM 九项 54.30s、续接六项 33.32s、健康五项 37.58s、进度
五项 28.02s、整批更新四项 51.80s；全部 0 fail / 0 ignored。精确分组/总数
53、固定源码/程序 hash、实际 probe、格式及 all-target Clippy（7.10s）通过。
日志 `execution-validation/g6/provider-cohort-full-driver-20261103.log`。

主工作区 CI 契约运行通过 36 项 / 797 assertions（37.48s）：其中包括当前
全部二十项，Bun 的路径过滤还收集了两份忽略目录中的历史副本，未将它们
算作新增覆盖。文档只读检查通过 11 documents / 12 required threat IDs。
当前提交的干净根门禁与远端结果仍须分别完成。

随后提交 `144cbf0` 的干净本机根门禁 **exit 0**：11/11 workspace lint/types/
test/build；Core 511+11+12（单元 570.75s），Runtime 24+6（48.79s/2.53s）
及 Bun 17，Chromium 113/35（34.09s），Desktop 108 Bun + Node 2 + Rust
37+6（4.05s/0.14s），Web 23/Desktop 25 字节等值与拒绝、最终 Rust 检查/
Clippy（8.15s/8.31s）和无漂移通过。提交已推送并核对远端 ref/PR head；
远端两路结果仍单独记录，不代替独立 review 或 G7 完成。

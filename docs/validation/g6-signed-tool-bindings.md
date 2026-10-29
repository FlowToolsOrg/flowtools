# G6 P2.8a4 签名插件与工具归档绑定

日期：2026-10-29。T0 prototype / inert planning；不推进 G5/G6/G7 父项、
第三方执行成熟度或独立安全批准。

## 实际契约

`packages/runtime-core/src/packages/dependencies/tools.rs` 的 plan_signed_tools
接受 opaque CompatiblePluginArchive、opaque InertToolArchive、T0 构建兼容环境、
既有单写者与 T0 clock。工具的 signed descriptor、完整 archive、Manifest、
source/license/flavor/layout review 已在不可构造的 admission proof 中检查；
生产 review profiles 仍为空，不能用 metadata/ToolCandidate/value pin 伪造 proof。

与现有签名插件 planner 共用有界 solver、根候选不可替换、SDK/native Manifest
语义复核、服务版本交集、DAG/边/搜索预算及原始 ZIP pin 投影。插件与工具候选
合计最多 512 个，原始 Manifest 合计上限仍为 16MiB，超限在 trust IO 前拒绝。
所有提供的候选均按当前根、持久原始 ZIP 观察、target、撤销、expiry 和 release
floors 检查，包含未被 solver 选择的工具。求解后再次采样 T0 clock 并重查所有
receipts，过期/回退/根轮换不能返回计划；不返回到较旧的信任时间。

工具声明按 publisher/id、版本范围、target、buildFlavor 和原始 ZIP hash 精确
选择，service 图中每个 consumer 各自绑定。同一工具 ZIP 可被多个 consumer
引用，保持同一 packageDigest，不复制或生成新 artifact。只包含 admission
确认的目标；metadata canonical hash 与原始 ZIP hash 不互换。

## 锁域与边界

SignedToolArchivePin 单独记录 consumer、publisher/id/version、admitted target、
buildFlavor 和 packageDigest。SignedToolArchiveLock 使用独立
`signed-plugin-tool-archives-v1` profile，完整插件/服务/工具图参与 digest。
共享求解器的临时 G4 tool projection 不作为公共 G4 lock 返回；旧
signed-plugin-archives-v1 锁的格式、工具字段必须为空及旧入口拒绝行为不变。
三个格式不能交叉反序列化，未改写签名 Manifest 或 G4 digest 语义。

公开 value pins、JSON 和 verified_digest 只供诊断/一致性检查；不能构造
VerifiedToolDependencyPlan，也不能用其创建 installed execution identity、
accepted lease、grant、文件路径、argv 或进程。计划不落盘、不导入插件代码、
不创建 image namespace/新数据库/管理 IPC。后续真实安装/租约/执行仍须在
自己的效果点复核当前 trust、scope/epoch、完整实际库存和原始文件。

仍维持保守的候选池策略：观察新 release 后，旧候选即使不使用也不自动绕过
floor。已安装多版本共存与旧 accepted references 是后续生命周期机制，不能
把本 planner 当成降级批准或已安装版本的可执行清单。

## 验证状态

新增七项真实签名 ZIP 测试覆盖共享工具的 service DAG、选择/原始 hash 拒绝、
重复候选与根替换、三个锁域拒绝互换、求解后的时钟拒绝、未使用工具的根/
撤销/floor 拒绝、合并候选预算与空工具控制。原 signed dependency 回归、
七项新专项全部通过（9.04s），原有签名依赖十一项通过（9.77s），Core all-target
Clippy `-D warnings`、五项 compile-fail 和 docs:check 通过。SDK/native corpus、整仓
与远端结果均待本修改实际运行后记录。
不把前一提交的完整门禁当作本修改通过。

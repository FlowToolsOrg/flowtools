# G6 P2.8a7：签名工具消费方的惰性安装锁

日期：2026-10-31。状态：in-progress / prototype。实施范围为签名插件/
service 图与已安装工具 ZIP 的持久绑定；代码与专项正在隔离工作区验证。
没有 accepted lease、可执行身份、grant、JS 加载或产品派发。

事务位于 `packages/runtime-core/src/packages/install/tool_consumers.rs`，
实际工具来源读取位于 `packages/tools/install/consumer_sources.rs`。
后者仅供 T0 内部使用，重新准入受持有句柄保护的原始工具库存 bytes；
生产入口固定使用空的生产 review profile，fixture admission 仅测试编译。

旧 G5 的 `package-store-v1`、formatVersion-2 库存/记录与 tool-free signed
lock 保持原样，仍拒绝工具锁 profile。新路径使用固定的
`tool-bound-plugin-store-v1` 与独立 version-1 库存/journal，复用 crate-private
三文件 blob/native create-only 操作；不向调用方提供 namespace 或路径参数，
不自动迁移/收编旧记录。所有固定物理池含 quarantine 合计保持原来的
256MiB / 384 groups，任一 pending 均阻止其他安装/核对路径。

T0 重新验证候选的原始 signed bytes、SDK 生成的 Manifest 契约及实际库存
providers。工具候选只能来自原始工具库存的实际 held ZIP/descriptor，并通过
同一个私有 release review 准入；生产审核清单继续为空，固定 fixture profile
仅测试编译可用。调用方不能提交 ToolCandidate、工具 pins、验证 flags 或
可执行路径。使用 a4 的独立 `signed-plugin-tool-archives-v1` 锁域，不转换成
G4 ToolPin 或旧 signed lock。

服务选择继续保持单 profile 的 provider 版本策略；已安装 provider 的全部
transitive package/tool pins 必须保持一致，不能借新根安装悄悄重锁。版本/
release floors 与当前根/撤销/expiry 均保持原拒绝规则，本子步不新增旧版本
执行或回滚特例，也不将诊断 pins 当作可信候选。

同一 DataStore writer 在首次效果前保存 journal；stage、create-only 发布及
最终提交都校验实际字节、完整库存/companion 状态和 exact journal。最终以
新 T0 clock 重准入根/providers/工具，重算同一锁并持有相应文件身份，SQL
前后再检查当前 trust/recovery 与库存。显式 abort 只隔离本事务拥有的字节，
保留旧记录、原始 blob、floors 和全部字节；中断不自动继续或采纳文件。

新消费方库存须进入其他安装/恢复路径的 companion 校验及总预算。孤立清理
不得把新消费方的有效引用当成可收编/可删除对象；损坏库存或缺失工具引用
必须拒绝。真实备份、copy/restore scope、进程退出、SQL 触发器、期限/撤销、
固定 profile/格式互换拒绝及原有 G5/G6 回归将在实现后记录实际结果。

完成本子步后仍须实现 accepted 任务 leases、受管执行与更新/GC、真实发行/
许可证及独立安全批准，以及 G7 文件提交/撤销和媒体输出闭环。

## 当前验证状态

十八项实际 SQLite/NTFS 专项已编写，覆盖两个消费方和旧 service 共享同一
原始工具 ZIP、旧 v2 库存字节保留、原 transitive pins、缺失/篡改来源、
全部 pending 互斥、当前签名撤销与最终时钟、SQL 触发器、真实备份/受控恢复、
原共享预算、格式/损坏拒绝、五处真实进程退出及中断后显式 abort。
十八项专项通过（102.68s），另两项固定 golden 摘要兼容测试通过；只有新域
真实存在时才扩展 a5/a6 companion 摘要，旧域未使用时保持原字节含义。
Core all-target Clippy `-D warnings` 随后通过（15.31s），Rust fmt 与
docs:check 通过。全套 Core 随后通过 375 单元（783.33s）+11 调度+8
compile-fail，包含加强后的所有 pending 稳定拒绝码断言及原 G5/G6/G7
回归；日志 `execution-validation/g6/tool-consumer-core-compat-20261031.log`。
完整根门禁和本提交远端 CI 仍须另外完成。

日志 `execution-validation/g6/tool-consumer-focused-guard-20261031.log`、
`tool-consumer-compatibility-20261031.log`、`tool-consumer-clippy-retry-20261031.log`。
首轮实际专项 16 pass / 2 fail 保留于 `tool-consumer-focused-retry-20261031.log`：
一条暴露库存写入触发器可替换 journal 后被直接删除，现补上清除前的 exact
pending 比较；另一条因用例主动安装 legacy provider，却误用禁止任何 legacy
安装记录的旧 helper，现保留旧 helper，并逐表验证无 grants/jobs/plugin_data/
imports，另精确比较旧库存不变。首次可变借用编译错误与 Clippy 测试模块
顺序拒绝日志也保留，未放宽任何原有门禁、断言或时限。

消费方库存备份还原后的未知 orphan 没有自动修复或收编入口；对应文件会
保留并阻止正常安装/核对。该 namespace 的显式 orphan reconciliation 仍须
完成，不能用已有原始 ZIP 或 image 恢复 API 操作此域。

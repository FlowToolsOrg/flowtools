# G6 P2.8a6：孤立工具文件的显式核对与能力撤销

日期：2026-10-30。状态：prototype。此子步接续 P2.8a3/a5，
处理数据库还原后 `tool-image-store-v1` 中未被当前有效库存引用的 artifact
和 stage。观察不采纳 seal、签名身份或可执行工具，不授予 lease/grant，
不删除字节或降低信任下限。生产审核清单与 trust pins 继续为空。

## 范围与效果

`packages/runtime-core/src/packages/tools/install/images/reconciliation.rs`
提供 opaque T0 plan 与显式 apply/resume/abort。仍借用 DataStore 单写者，
固定 namespace/journal/receipt，没有客户端路径、反序列化计划入口或 IPC。
计划绑定 canonical profile、完整 image 库存、插件/原始工具库存与当前
trust/recovery scope。复制、未完成的受控信任恢复和损坏元数据拒绝操作。

只观察由当前 T0 用户拥有的有界树，固定每个节点的路径、native volume/
file ID/creation、文件内容摘要/长度、只读属性，以及 owner/DACL/label
SDDL 摘要。私有 namespace/父目录仍必须满足原有 exact protected policy；
有效已安装 image 仍须通过原来的 seal/完整字节/身份/ACL 验证。孤儿或部分
stage 的观察描述符不必是 active policy，因为其唯一用途是隔离及缩小访问。
它绝不能用于收编或构造 artifact capability。未知节点若超出路径/数量/
磁盘预算即拒绝，junction/reparse/hardlink 均拒绝。

首次效果前整批复核并持久化 exact journal。每组通过 held-parent、create-only
原生移动进入 quarantine，随后逐个文件、最深目录到根写入原有 exact
private policy。叶子先处理，防止部分 stage 的继承在子节点核对前改变其
描述符。每次权限写入前再次比较 SQLite 中冻结的相关元数据行；触发器改变
库存、trust、其他 pending 或 journal 不能随成功提交。

重试仅允许已经隔离的节点保持其原始描述符或 exact private policy；源树
必须完全符合原观察。清除 journal 前所有隔离节点都必须是 private policy，
不再包含 artifact capability。显式 abort 同样先完成已移动组的撤权，保留
尚未移动的源与所有字节；写 receipt/清 journal 失败则保留 exact pending。
scope 改变禁止 resume；当前受控恢复完成、profile/库存/字节仍一致时可
显式 abort。此步骤不提供运行中任务/lease 管理。

备份还原导致 image 与原始 ZIP 同时孤立时，必须先完成本步骤，再执行 a5
原始 ZIP 核对。原始 ZIP 可以暂时是孤儿，但其有效库存记录仍须逐个验证；
不能先移动当前 image 引用的 ZIP。普通插件/工具安装、inspect、materialize
及其他恢复路径在任一 pending 存在时拒绝。三种工具物理池与插件池含
quarantine 合计继续使用原有 256MiB / 384 groups，不增加恢复专用额度。

## 验证计划与当前结果

新增专项覆盖真实数据库备份/受控恢复、image 后 ZIP 的顺序、旧 seal 与
release floors 保留、完整批次变更拒绝、部分继承树逐节点中断、current
scope/跨 profile/pending 互斥、SQL 触发器、损坏/伪造 journal、原总预算及
junction/hardlink。六个真实子进程在 journal、移动、部分权限写入、整组
撤权、最终 SQL 前和提交后直接退出，再由父进程重开实际数据库续接。

真实 LPAC 回归先验证旧 capability 可以读取观察到的文件，移动后仍可读；
随后显式 abort 撤权，即使最终 SQL 删除 journal 失败也必须实际拒绝读取。
实际 token/Job/PID/排空与原始时限断言保持不变。

新增二十一项专项全部通过（161.13s），Core all-target Clippy `-D warnings`
通过（12.20s）。日志 `execution-validation/g6/image-reconciliation-focused-i64-20261030.log`
及 `image-reconciliation-clippy-20261030.log`。首次编译使用 SQLite 未启用
`fallible_uint` 的 `usize`/`u64` 转换被拒绝；核对当前库源码后改为原生
`i64` 计数，没有新增 feature/依赖或放宽断言，两份失败日志保留。既有
密封文件十八项回归通过（92.28s），全部七项 compile-fail 通过（0.83s），
docs:check 和 Rust fmt 通过。完整根门禁与当前提交远端结果另记，不将专项
或前置提交的通过记为本提交整仓通过。

accepted leases、产品安装锁/工具锁与执行身份、更新/LKG/GC、受管媒体和
G7 用户文件提交/撤销/输出闭环仍待完成；独立安全和真实发行/法律审核不
由工程自查替代。

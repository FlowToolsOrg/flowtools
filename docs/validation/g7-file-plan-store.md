# G7 文件计划持久化

日期：2026-10-29。范围：Windows/x64 T0 只读规划，prototype。

`file_workflows/store.rs` 的 `FilePlanStore` 只接收 T0 已生成的 opaque
`RenameDraft`。稳定 UUID plan ID、原定义 digest、revision、文件数、跳过数、
字节数与取消状态存入现有 DataStore 的 `core_metadata`；不建立第二个数据库。
原始路径、文件名与身份定义使用现有 PrivateJobs 的独立随机 UUID 和 DPAPI
加密，先 fsync 私密数据，再提交元数据引用。计划上限 64、元数据上限 64 KiB，
同时沿用 PrivateJobs 的单值、文件数和总空间预算。超量拒绝，不淘汰旧计划。

相同 request key 与原 digest 只返回已存在记录，永不替换定义或恢复已取消计划。
预览返回最初审阅的内容；显式 recheck 重新验证原目录、整批源身份/内容及目标。
外部修改返回冲突，不重算并覆盖原计划。取消使用精确 revision，只修改元数据。
取消后的定义仍保留，不能再预览或复核；此保留策略没有自动 GC。

状态绑定 T0 派生的实际 canonical DB 路径 hash 和 restore generation。
复制数据库或备份恢复后拒绝普通操作。恢复查询产生绑定完整旧状态与当前 scope
的 challenge；T0 显式管理同意后才能传入该 challenge 作废全部旧计划。
作废只取消并递增 revision，不改文件、删除私密字节、创建 grants 或恢复执行。
旧 challenge 在 scope/state 改变后失效，不能取消随后创建的新计划。

Runtime 启动清理孤立私密数据前，会严格加载计划元数据并保留其 input UUID，
包括等待恢复处置的旧 scope。保留不等于接受旧计划；损坏记录使清理失败并保留
原字节。SQLite 提交失败不会发布计划；遗留的无引用加密 blob 可由原有孤立
清理回收。任何实际文件改名仍仅编译在原有单元测试中。

新增 10 个 Rust 测试入口（含真实退出子进程入口），与原有 7 项 NTFS 测试
共同通过。覆盖真实 DPAPI/SQLite 重开、元数据与加密文件 canary、相同/冲突 key、
取消持久化、外部修改、复制/恢复 challenge、原生 Host 启动清理、损坏数据、
SQL 写失败、真实子进程不运行析构直接退出、64 计划预算与重复记录拒绝。
这些是专项证据，当前修改仍须完成干净整仓与远端门禁。

本步骤没有 GUI/CLI 创建/提交 API、broker 文件效果、action journal、断电事务
恢复或撤销接口，也不解决 ADS、ACL 和全部文件竞争门槛。不能将持久计划计作
实际文件闭环或执行授权。后续范围见 [完成矩阵](./g7-completion-matrix.md)；
G5/G6 及独立安全前置不因只读存储而通过，G7 人工项保持 waived。

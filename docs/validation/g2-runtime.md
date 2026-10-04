# G2 lifecycle and headless runtime acceptance

日期：2026-10-04；工程实施与验证：Codex。每子项独立验证并提交。
成熟度保持 prototype；独立安全 Reviewer 尚未批准；SEC 风险不关闭。

## P1.2a

统一 Registry/Loader/LifecycleManager，保留当前状态词表。每插件队列跨
loader 实例共享；不同插件不共用锁。onLoad -> onActivate -> onDeactivate ->
onUnload；失败撤下可用状态并补偿，原始异常与清理异常分别保存。
失败后普通 enable/load 不自动重试，明确 unload/reload 后恢复。
清理失败阻止 removal；已排队加载期间同步 unregister 也拒绝。

回归包括 36 个状态边、100 并发 enable/disable、manager 混用、禁用重启、
原始错误、补偿错误、显式恢复和排队删除竞争。定向 53 tests 通过。
根 docs:check、lint/check-types（七 workspace，force/concurrency=1）、SDK build + 全量 175 tests、Web 23 tests 通过。
本项没有视觉或路由变更；P1.2b 再验证实际启停命令界面。

本项没有新增 capability、网络、文件、grant 或持久化入口。状态锁仅用于
可信 T1 的一致性，不是第三方隔离；用户 DB 不变。既有 unsafe development
卸载改为经过相同 cleanup，DEV + exact opt-in 默认拒绝规则保留。

P1.2b、P1.3a、P1.5a pending。

# G6 P2.8a5：惰性工具 ZIP 的显式恢复核对

日期：2026-10-30。状态：prototype。当前仅处理 `tool-package-store-v1` 中
未被有效库存引用的归档和 stage；不是工具文件恢复、收编、GC、回滚、lease
或执行身份。生产工具审核清单及生产 trust pins 继续为空。

## 事务与边界

`packages/runtime-core/src/packages/tools/install/reconciliation.rs` 提供 T0
只读 plan、显式 apply/resume/abort。plan 是不可反序列化、字段私有的 opaque
对象，只有数量和字节摘要可供诊断。没有新增 IPC、客户端路径、批准开关、
独立数据库或自动启动清理。调用方须已保护并独占当前私有 profile。

计划绑定 canonical 数据库 profile、完整原始归档库存、插件/密封文件库存和
当前 trust/recovery generation 元数据摘要。恢复旧数据库后须先完成已有的
受控信任恢复；普通复制、未恢复 scope、损坏库存和未知文件不能触发清理。
已有密封文件必须仍准确引用有效原始归档，并通过自身内容/身份/ACL 复核。
密封文件孤儿或其库存不一致会拒绝本路径，不能先隔离其原始 ZIP 绕过依赖。

原始 blob 层复用 G5 的固定三文件、有界内容指纹、单组件 NT 相对句柄移动和
create-only quarantine。仅扩大 crate 内部复用可见性；不允许调用方指定
namespace，不改变插件目录或 formatVersion-2 库存/锁字节。未知组名、额外
文件、reparse point、hardlink、源/目标同时存在或同时缺失均拒绝。这里的
孤儿指纹只证明观察到的惰性字节，不证明签名、兼容性或可执行文件身份。

首次效果前整批复核，先持久化同一个 DataStore writer 中的专用 journal，
随后逐组再次核对 scope、库存、journal 与源字节并移动。完成后保存 metadata
receipt，再清除 exact journal；SQL 前后都复核，触发器不能把库存、trust、
receipt 或 journal 替换与成功状态一起提交。原始插件/工具安装、检查和各
恢复入口在其他 pending 存在时拒绝，即使 pending 内容损坏也不自行修复。

进程中断保留 journal 与混合位置；显式 resume 仅继续同一批已经观察到的
字节。abort 不移动或删除任何文件，只记录当前状态并取消该批；剩余源必须
重新 plan。trust/recovery scope 改变后禁止 resume，完成当前受控恢复且库存/
profile/字节仍一致时可显式 abort。已隔离文件仍计入原有三个物理池合计
256MiB / 384 groups，不能以恢复为由额外增加额度。

## 验证

专项覆盖真实备份恢复后的新签名 ZIP 与 interrupted stage、整批变化/未知
文件拒绝、跨 profile/库存/scope 拒绝、各池 pending 互斥、混合位置取消、
损坏/重复键/超长 journal、SQL 触发器替换、三池合计预算、junction/hardlink、
有效密封引用保留及未知密封文件拒绝。另用五个真实子进程在 journal、两次
原生隔离、最终 SQL 前及提交后直接 exit，父进程重新打开实际数据库并续接。
十八项专项全部通过（58.12s）；既有 G5 恢复核对十九项通过（72.96s），
Core all-target Clippy `-D warnings` 通过（12.07s），六项 compile-fail 通过。
首次专项编译发现收据
固定 outcome 的 Rust 生命周期签名错误，已将内部参数收紧为静态固定状态；
原始编译拒绝日志保留，没有放宽用例或断言。完整根门禁与该提交远端结果
另行记录，不把专项当作整仓通过。

accepted leases、安装锁与工具锁的产品绑定、工具文件孤儿恢复、LKG/GC、
真实签名发行/许可证与独立安全批准、受管媒体执行和 G7 用户输出闭环仍待完成。

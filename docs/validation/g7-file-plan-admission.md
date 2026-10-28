# G7 文件整理：只读规划准入

日期：2026-10-28。范围：Windows/x64 NTFS、T0 显式选择的普通目录，prototype。
这一步是文件闭环的前置规划层，尚无 GUI/CLI 共享持久 plan ID、提交或撤销入口。
不将只读预览或测试中的原生改名算作实际办公闭环。

`packages/runtime-core/src/file_workflows.rs` 的 `prepare_rename` 生成不可反序列化、
不可由客户端构造的 `RenameDraft`；`preview` 只投影源/目标名称、大小和 SHA-256。
原始目录和文件身份留在 T0 对象内，不写数据库或日志。`recheck` 重新验证整个
批次的原目录身份、原文件身份/大小/时间/内容与目标不存在条件，无文件写入。
前缀为 1–32 个 UTF-16 单元；已带此前缀的文件跳过，子目录不递归。

`file_workflows/windows.rs` 从盘符根逐组件打开并持有全部祖先；后续枚举/打开
使用真实句柄及官方 Windows ABI。拒绝 UNC/device/相对路径、reparse/junction、
硬链接、大小写敏感或非 NTFS 目录、Runtime profile 子树、非法/保留名称及双向
控制字符。不通过 `read_dir` 在校验后重新按不受控路径查找。持有源文件时拒绝
写入/删除共享。限制 512 个目录条目、128 个动作、单文件 64 MiB、合计 256 MiB；
超量整批拒绝。非 Windows 平台不提供此能力。

原生 create-only rename 目前仅编译在 Rust 单元测试中；生产构建没有该文件
写接口，也没有新 broker operation、IPC、webview permission、SDK capability、
插件或 CLI 文件参数。测试使用 held root/source 与 `ReplaceIfExists=false`，验证
迟到目标冲突不覆盖、原内容不变和真实中文文件名改名。NTFS name tunneling
可能改变改名后的创建时间，测试 helper 保持其他身份/内容字段准确并返回实际
新创建时间，不能沿用旧时间构造未来撤销凭证。参见官方
[tunneling 说明](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/fltkernel/nf-fltkernel-fltgettunneledname)。

七项实际 NTFS 回归通过：只读预览/内容变化、全批次冲突、持有文件拒绝外部
写入/改名及迟到冲突、硬链接/替换身份/profile 子树、真实文件和大小预算、
junction 根/条目拒绝、子目录条目也计入库存预算。此记录不声称完整 filesystem
race、ADS、ACL 隔离、持久 journal、断电恢复、批次原子性或可撤销事务通过。

后续执行必须先满足既定 G5/G6 执行门槛，再接现有单写者/私有存储、profile/
restore 绑定、精确逐效果 grant/epoch/deadline、durable claim、未知状态处置及
不重复改名；撤销只针对记录过且未被外部修改的文件。不能加 arbitrary path /
force overwrite / shell fallback 来绕过。完整 G7 依赖见
[完成矩阵](./g7-completion-matrix.md)。人工 G7 验收仍为 waived，绝非 passed。

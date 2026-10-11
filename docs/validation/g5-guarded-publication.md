# G5 P2.5b 持续目录保护与发布第六步

日期：2026-10-11。状态：Windows/x64 T0 prototype；P2.5b/G5/G6 和独立
安全批准仍 pending。承接 [第五步惰性安装](./g5-inert-installation.md)，只
修补发布/显式 abort 隔离时临时释放内部父目录保护的窗口，不改变库存、
journal、seal、DataStore schema v2 或客户端/插件的执行权限。

## 源码与行为

- [固定文件层](../../packages/runtime-core/src/packages/install/files.rs)：
  profile/root 和 artifacts/staging/quarantine 父目录句柄持续持有。内部父
  目录允许 read/write sharing，delete sharing 全程关闭；profile/root 保持
  原有 write/delete sharing 拒绝。没有释放/重新获取 guard 的移动窗口。
- [native 移动](../../packages/runtime-core/src/packages/install/files/rename.rs)：
  `NtCreateFile` 使用 held source parent 的 `RootDirectory` 和一个已验证
  组件名，只 OPEN 目录；不继承句柄、拒绝 reparse，读取真实 handle 信息。
  源句柄请求 DELETE 且不共享 write/delete，固定候选期间重新校验完整
  blob/seal 或允许的部分 stage；校验失败不移动。
- `NtSetInformationFile` 的 `FileRenameInformation` 使用 held target
  parent 的 `RootDirectory` 和固定 digest/UUID 名。官方 `windows-sys`
  ABI、有界对齐缓冲区、`ReplaceIfExists=false`，只同卷移动。源/父目录
  类型与 reparse 属性重查；异常状态拒绝，保留安装 journal。无覆盖、复制、
  POSIX flags、路径 fallback 或 guard-release 重试。
- 包作者的 Manifest 路径不用于这些磁盘路径；调用方不能提供父句柄、任意
  路径、native flags 或校验回调。模块为内部 T0 实现，没有新 IPC/SDK
  能力。原有签名、当前时间、磁盘 provider/G4 锁重查和 FULL SQLite
  inventory/journal 原子提交继续执行；旧版本字节和观察 floors 保留。

Windows 实测：仅 read sharing 的内部父目录会使 native rename 返回
sharing violation；保留 read/write sharing 且不开放 delete sharing 后，
真实移动和安装回归通过。这是目录项更新所需共享方式，不是写入隔离或
ACL 证据。API 语义参照 Microsoft 的
[NtCreateFile](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntcreatefile)、
[NtSetInformationFile](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntsetinformationfile)
和 [FILE_RENAME_INFORMATION](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information)。

## 实际回归

五项新增 native 测试使用拥有的 UUID 临时 profile 和真实 Windows API：

1. 固定源目录并校验时，重命名两个父目录或源目录都返回实际 sharing
   violation；native 发布成功后目标父目录仍不可重命名，候选字节一致。
2. 目标已存在目录或普通文件时，create-only 移动失败；源及目标原字节
   均保留，父目录保护未释放。
3. 源候选为真实 junction 时，校验回调不会运行；不触碰指向目录的 canary，
   目标未发布。清理只移除拥有的 reparse 本身，不遍历其目标。
4. 空名、越界/分隔符、alternate stream、大小写、NUL 和超长组件在 native
   IO 前拒绝；固定源后的字节校验失败同样保留源、不发布目标。
5. 校验期间对实际持有的目标父目录执行 `FSCTL_SET_REPARSE_POINT`，随后
   handle 信息重查拒绝发布；明确删除测试注入的 tag，源和外部 canary
   原字节保留。这证明该注入点的拒绝，不证明全部文件竞争均被覆盖。

现有 15 项实际安装测试继续覆盖原始 DSSE/ZIP、真实磁盘 provider、五个
进程退出 barrier、显式 quarantine 恢复、备份信任拒绝、故障注入、配额、
seal/readonly/硬链接/junction。现有目录保护回归在 StagePrepared 和
BeforeCommit 两阶段尝试移动 root 及全部三个内部父目录，全部拒绝。

本轮最终 focused native 回归：5 passed / 0 failed；安装回归：15 passed /
0 failed；`cargo clippy --locked --all-targets -- -D warnings` 通过。
完整仓库 clean-checkout 门禁将在提交后运行，结果在通过后记录，不预先
宣称远端 CI 或生产验收完成。

## 验证范围与未完成项

此实现只在 Windows 编译，本轮实际 Windows/x64 验证。原始文件 fsync 与
SQLite FULL 及进程退出恢复回归保留；native rename 没有 write-through
保证，进程退出不是物理突然断电。没有新签名 pins、下载/解包/激活、健康/
LKG、卸载/GC、受控 trust recovery/rollback、管理 UI/IPC 或工具侧加载。

Native Host 必须先保留/保护 T0 私有 profile；readonly 属性不是 ACL。
内部父目录仍允许 write sharing，完整文件内容写入竞争、最终校验至 SQLite
提交的跨资源竞争、ACL、全平台及物理断电验收仍 pending。句柄相对移动
不构成同 OS 用户攻陷隔离、第三方 sandbox、可执行身份或用户 grant。

威胁：SEC-001/008/014/015 保持 open；ADR-0001/0002 的执行边界不变，
ADR-0003 记录此文件层增量。Reviewer：Codex，2026-10-11，结论：工程
自查和上述原型回归通过；独立安全 reviewer/date/conclusion 仍 pending，
PR 保持 Draft。签名和 inert inventory 不能为 T2/T3/TL 授权。

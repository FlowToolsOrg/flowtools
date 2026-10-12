# G5 最终提交期间的原始 artifact 保护

日期：2026-10-12。范围：Windows/x64 T0 惰性安装 prototype；工程验证通过。

此前安装会在提交前重新读取并校验实际 candidate/provider，但读取句柄随后
关闭，文件验证与 SQLite commit 之间仍存在写入/替换窗口。readonly 属性
不能代替有效的共享访问限制。

`packages/runtime-core/src/packages/install/files.rs` 的内部 ArtifactPin
同时持有包目录及 manifest.json/package.zip/seal.json 的只读句柄，排除
write/delete sharing。只从已接受的 signed archive lock 精确选择 provider，
在最终验签、重新规划和 metadata 事务前取得这些句柄，并保留到提交结束。
打开全部文件后再次校验原始字节、seal、类型和硬链接数量；既有写句柄导致
无法取得 pin 时直接失败，保留旧库存和 pending journal。

pin 在 publication 完成后取得；没有释放目录保护重试、降级复制或修改 native
移动协议。Windows 对打开子文件的目录移动有额外约束，此控制不声称文件在
publication 全程不可变；发布后任意变动仍须先通过最终完整准入。

真实回归要求 candidate 与已选择 provider 的三份文件在 FilesPinned 和
SQLite 已提交的 Committed 屏障
拒绝写入、删除及改名，目录改名也拒绝。测试临时取消 readonly 后仍要求
ERROR_SHARING_VIOLATION，证明拒绝来自活跃句柄；成功返回后实际写句柄可
打开，证明保护已释放。已有 provider 写句柄导致拒绝时不提交库存，原字节
保持，显式 abort 后恢复可用。新增屏障也纳入真实进程退出恢复矩阵。

使用固定 Bun 1.3.14 / Rust 1.96.0，Core all-target check、clippy
`-D warnings`、185 项 Rust 回归及 5 项 compile-fail 全部通过。新增两项
真实文件句柄回归通过；六个安装屏障的真实进程退出矩阵通过。首轮 clippy
捕获测试中跨平台 readonly 清除写法，现改用已有 Windows 属性 helper，
没有抑制 lint 或削弱共享访问断言。随后 18 项安装专项回归全部通过
（70.74s），同时见证 FilesPinned 和 Committed 两个阶段的实际拒绝，
证明句柄保护持续跨越 SQLite commit。

无产品 UI、IPC、执行身份或 grant 变更；不
关闭完整路径/ACL/内存映射写入竞争、物理断电、健康/LKG 或平台隔离验收。
SEC-008/014/015 继续 open；实际独立安全 reviewer/date/conclusion pending。

# G5 P2.2a Windows 原生工具隔离可行性

日期：2026-10-29。Windows/x64 prototype，工程自查，不是独立安全批准。
该子步骤为 G6 原生工具执行提供实际 OS 拒绝证据，G5/G6/G7 整体仍 in-progress。
决策见 [ADR-0005](../adr/0005-windows-native-tool-isolation.md)。

## 实际实现

`apps/runtime/tests/windows-isolation.rs` 使用单独编译的
`apps/runtime/test/fixtures/isolation-probe` 原生子进程，未修改产品执行入口。
每例建立 UUID AppContainer、用户私有测试树/注册表/IPC、不同 SID 的第二任务
目录和持有的程序文件。程序只授予读/执行，workspace 带 Low mandatory label，
不更改现有用户目录 ACL。清理只处理本例拥有的确切资源。

以 suspended + detached 创建进程；恢复线程前读取实际 token 的 AppContainer
SID、LPAC attribute 及唯一启用的固定 registryRead capability，并检查未继承
私有文件内核对象。Job 配置 CPU hard cap 25%、128MiB process memory、单进程、
kill-on-close；CPU flags/rate 实际回读。明确 stdio handle allowlist、子进程拒绝、
Low/remote image-load mitigation 和 System32 优先，不继承 PATH/USERPROFILE。
实际工具 profile、签名、broker grants 和文件发布不由此测试产生。

标准 Runtime `test` 以 fatal `&&` 先构建 fixture，再 inventory 并运行全部
原生/Bun 测试；lint/types 也检查 fixture。根 Cargo default-members 仍只有
Core/Runtime，普通 Runtime build 和发布构建不打包探针；probe publish=false。
CI 契约同时检查实际 Cargo metadata 的默认成员与非发布目标。

## 六项真实回归

1. 同一程序普通进程正向对照实际读取/写入私有文件和另一任务目录，读取私有
   注册表，连接私有 named pipe/TCP，分配 256MiB 并加载 Low DLL；LPAC 对应
   操作均拒绝，TCP 为 10013、注册表/IPC 为 5。自己的 workspace 可写。
   普通进程继承的私有文件句柄对象在 LPAC 恢复前证实未继承。真实 junction
   指向私有目录，普通对照可读写，LPAC 仍不能经该链接读取/写入目标。
   Low DLL 不加载，固定 System32 DLL 可加载；原 canary 字节不变。
2. 实际创建的无限循环子进程先写 ready，500ms 时终止所属 Job；随后 500ms
   内真实进程退出、Job accounting 归零，不以 promise/事件替代实际退出。
3. 实际无限循环进程存活且 Job count=1 后关闭唯一 Job handle，500ms 内退出。
   Windows kill-on-close 可报告 exit 0；存活前提与不会自行退出的程序证明回收。
4. 同一程序创建固定自身子进程，普通对照真实写 descendant marker；LPAC 返回
   ERROR_CHILD_PROCESS_BLOCKED (367)，marker 不存在。
5. 实际无限 stdout 超过聚合 64KiB 后终止 Job；原 5s 限期内实际进程退出、
   accounting=0。两个管道都在同一预算内，未知管道错误拒绝。
6. 同样创建的 suspended LPAC 进程在 policy-refusal 点被回收；PID 确实退出、
   accounting=0，无 ready/report，未 ResumeThread，未降级启动普通进程。

本机专项 `cargo test --locked -p flowtools-runtime --test windows-isolation
-- --nocapture --test-threads=1`：6 passed / 0 failed，约 2.36s。
Runtime 与 fixture all-target Clippy warnings-as-errors 通过；本记录的最终
整仓与远端结果随后记录，不把上述专项成功称为完整验收。

## 保留的失败与适用范围

初始 CREATE_NO_WINDOW 组合在本机产生 DLL initialization 0xc0000142；使用
DETACHED_PROCESS 后运行，不关闭 LPAC/mitigations。缺 LOCALAPPDATA 时创建
返回 203；仅传固定 OS bootstrap allowlist，不复制父环境。零 capability 的
Winsock 10107 不是网络隔离成功，改为固定 registryRead 后验证真实拒绝。
新的子进程测试初始错误预期为 5，实际原生错误为 367，已据实际 API 修正。
数字句柄可能被子进程 bootstrap 复用，对其 GetFileType/ReadFile 的初始探针
在原 5s 超时；改用恢复前的真实 kernel object 身份对照，未延长期限。新建
junction 的初始 NT substitute name 含斜杠导致 123，已规范为反斜杠并先做
实际目标读取正向对照，再验证 LPAC 拒绝。

这没有验证受限 JS 引擎、第三方 React/Tauri 容器、COM 全面拒绝、远程映像
实际共享盘正向对照、真实工具 DLL/协议布局或重启/物理断电。CPU 配额只证明
内核接受并回读限额，不声称精确调度性能。当前签名工具审核清单仍空；真实
发行签名/许可证审阅、安装解包、accepted leases、撤销、输出验证/发布与恢复
须继续实现。SEC-003/004/005/010/015 不因本子步骤全部关闭。

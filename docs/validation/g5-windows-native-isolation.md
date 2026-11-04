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
kill-on-close；CPU/内存/active-process limits 实际回读。普通 fixture 单进程，
child-policy 专项的普通/LPAC 对照都允许两个 Job 进程，避免 Job 自身数量
上限替代 child policy 的证据。恢复前额外回读真实 child policy，普通值 0、
LPAC 精确为 NoChildProcessCreation=1，无 audit/secure override。
明确 stdio handle allowlist、子进程拒绝、
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
   Windows 11 返回 ERROR_CHILD_PROCESS_BLOCKED (367)，Server 2022 返回
   ERROR_ACCESS_DENIED (5)，仅接受这两个已观察到的拒绝码；两组 Job 实际
   容量均为 2，child policy 实际回读为 0/1，拒绝后 marker 不存在。
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

`ec66201` 首次完整根门禁失败：lint/types、Core 262+11+2、Chromium 109 和
Runtime 既有 24 项原生测试均通过；新增六项因 Turbo 严格环境缺
LOCALAPPDATA 拒绝启动。没有放宽环境 passthrough，改由 T0 通过 Windows
SHGetKnownFolderPath(FOLDERID_LocalAppData) 获取固定 bootstrap 路径；不
依赖父环境变量。原失败日志保留在
`execution-validation/g5/root-ci-ec66201-20261029.log`，最终修复树须再跑整仓。
清除 LOCALAPPDATA 的专项六项通过；随后严格 Turbo 单工作区复验完整 Runtime
标准 test：inventory 30、既有原生 24、新增 LPAC 6、Bun 17（含真实 50 路冷
启动）全部通过，约 4m46s。此复验没有替代最终根门禁的依赖图与全仓检查。

`955d88d` 完整本机根门禁 exit 0：11/11 lint/types/test/build，Core 272+11+2、
Runtime 24+6 原生及 17 Bun、Desktop 105 Bun+Node 1+Rust 36+6、Chromium 109。
Web 23/Desktop 25 的生产重构建 byte-identical；Rust、codegen 与最终干净工作区
通过。日志 `execution-validation/g5/root-ci-955d88d-20261029.log`。
同 SHA [PR Windows 40842642460](https://github.com/FlowToolsOrg/flowtools/actions/runs/40842642460)
失败：Runtime 24 项通过，新增 LPAC 为 5 pass/1 fail，仅错误码 5 与原断言
367 不同。修复不只接受两个已观察的拒绝码，同时回读实际 child policy/Job
限额，并让专项两组 Job 都容许第二进程；本机六项与 Clippy 再次通过。
当前修复 SHA 的远端结果仍须单独核验，不以该本机通过替代。
修复 `8d62d87` 的远端完整
[PR 40846856086](https://github.com/FlowToolsOrg/flowtools/actions/runs/40846856086)
与 [push 40846845674](https://github.com/FlowToolsOrg/flowtools/actions/runs/40846845674)
均 SUCCESS；Windows Server 2022 实际 policy/Job readback 与六项 LPAC 均通过。
这完成本 native feasibility 子步骤的自动门禁，不是 G5/G6 整体或独立安全批准。

这没有验证受限 JS 引擎、第三方 React/Tauri 容器、COM 全面拒绝、远程映像
实际共享盘正向对照、真实工具 DLL/协议布局或重启/物理断电。CPU 配额只证明
内核接受并回读限额，不声称精确调度性能。当前签名工具审核清单仍空；真实
发行签名/许可证审阅、安装解包、accepted leases、撤销、输出验证/发布与恢复
须继续实现。SEC-003/004/005/010/015 不因本子步骤全部关闭。

# G3 P2.6a：单写者数据与恢复

- 日期：2026-10-05；Windows、固定 T1、可丢弃验证 profile。
- 独立安全 Reviewer：pending；maturity 保持 prototype。

Runtime 的同一个 SQLite writer 管理 core metadata、grants、jobs 和 plugin
data 分区。当前 schema v2；v1 升级先生成同 profile 备份，事务内迁移，失败
回滚。未来版本、损坏数据库、第二 writer、路径重定向均拒绝。无启动删除或
自动合并旧数据；Desktop Debug 也保留 app.sqlite，并在连接前只读检查。

Rust DTO 生成 data.read/write/transaction/import-legacy 的 TS 和 wire schema。
客户端仅选择固定清单中的插件；namespace 由认证 caller 与 Host 解析的包身份
决定，不能提供 SQL、namespace 或路径。broker 在同一 Runtime 锁中检查声明、
scope、epoch 与预算后提交事务。每 key 有 revision，写入要求 expectedRevision；
冲突、重复 key、超限写入不留下部分变更。限额为每值 64 KiB、每事务 16 key /
256 KiB、每插件 256 key / 4 MiB。SDK 异步 data API 与 hydration/watch 适配
现有同步 UI store，不创建第二 writer。

旧 Todo 来源只检查不改写：CLI-v0、Web/Desktop localStorage-v1。显式导入
校验内容与 SHA-256，空目标才写入，并记录来源/digest/revision。同一导入重试
返回已有数据；不同来源或损坏输入不覆盖。原始来源由调用方保留，不自动删除。
当前 native data fixture 是显式受控 Host policy，不是用户持久 grant。

恢复仅为 T0 Rust API：先检查同 profile 备份，再生成 stage；Windows 原子替换
保留损坏原件。pending marker 先落盘，恢复中断后拒绝作为新 profile 启动；
需要维护者核实备份与保留文件后处理，不自动猜测恢复或报告成功。私有 durable
job payload、凭据存储与用户管理命令分别等待后续 G3 子项，不写入运行历史。

回归覆盖 namespace/CAS/事务、重启/第二 writer、N-1 迁移及注入失败回滚、
损坏保留与显式备份恢复、未来 schema 拒绝、显式幂等导入、注入/超限、恢复
中断 fail-closed。SDK 覆盖来源检查与异步 hydration/cancellation。实际 Node
双客户端经 named pipe 读写同一库，并验证 scope 拒绝、重启与冲突。
专用 native WebView harness 验证 CLI 写入、实际 Desktop native-bound session
读取/改写，再由 CLI 读取 revision 2；它验证数据传输，不等于 Todo 用户界面
已经切换 Runtime（P1.4b），也不认证 NVDA 或其他 OS。

本地 docs、lint（零警告）、types 和全仓 test 已通过，十一 workspace 全量
执行；Runtime core 26、native 2、Desktop Rust 17、固定 Chromium 59 项测试
通过。全仓 build、Rust fmt/clippy、生成协议/Manifest 只读核对和实际生产入口
重建门禁通过，无生成内容漂移或危险入口泄漏。原生 harness 的共享数据 revision 2、初始 launcher、键盘任务查询和
断线诊断通过，测试窗口/Host/Vite 已关闭。相关 SEC-003/004/005/006/007/009/010、
ADR-0001/0002 保持 open；同用户账户已被控制、第三方隔离、签名、生产 grants
与独立安全审阅没有由数据库或测试自动解决。

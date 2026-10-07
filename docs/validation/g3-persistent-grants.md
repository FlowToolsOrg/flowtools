# G3 P2.4a：持久授权与 CLI 管理

2026-10-05，固定 T1 / Windows prototype。独立安全 Reviewer：pending。

grants、bootstrap policy、有限 policy audit 与 plugin data 使用同一 SQLite
writer。批准绑定 Host caller、publisher、版本、完整 manifest/artifact digest
和 command；CLI/Desktop 分别授权。epoch 在批准、撤销和清单变化时递增。
清单变化先撤销旧记录，回滚不恢复旧批准。最大 256 records；audit 保留最近
256 条决策元数据（身份 hash/epoch/时间），不记录 token、正文或异常内容。

CLI 提供 `init --interactive`、`init --policy <file>`、`permissions list`、
`permissions grant --policy <file>`、`permissions revoke <plugin-id>`；可用
`--profile` 选择明确的 Host profile。默认用户目录为 LOCALAPPDATA 下
FlowToolsRuntimeV1。交互初始化必须有 TTY 并输入 yes；非交互必须主动提供
预配置文件。策略导入全部校验后原子提交，任何坏条目都不能部分批准。
`describe` 提供 packageDigest；预配置 grant 必须匹配该实际完整包摘要。

配置文件示例：`{"formatVersion":1,"coldStart":false,"grants":[]}`。
grant 字段来自 Rust 生成的 PermissionGrant，包含 pluginId/commandId/target、
packageDigest、effects、typed scopes、expiresAt/maxCalls、coldStart/background。
冷启动 bootstrap 仅允许启动内核；插件效果与后台/冷启动仍需独立 grant。
当前内置 supportsColdStart 仍 false，真实受管执行/独立冷启动后续逐项开放。

native 初始化先创建当前用户 SID 的 protected DACL 目录，子文件继承该 ACL；
重开校验目录和 bootstrap ACL、token 格式及路径重定向。认证 token 在 native
profile，CLI 普通调用/管理调用分别使用固定凭据；wire 不能自报管理角色。
管理模式拒绝所有业务提交和 data 操作，不加载 runner。T1 编译身份一致性
不是 publisher 签名；同 OS 用户账户已被控制不在此保证内。凭据尚未加密，
私有 ACL 不等于 OS sandbox；T2/T3/TL 继续拒绝。

当前管理 CLI 启动固定 workspace native binary，独立受检发行、启动锁和
对已运行实例的管理连接由 P1.4a 接续。原有 CLI run 在 P1.4b 才切换 Runtime。
用户界面、开机自启、调度、网络/文件 adapter、签名包安装未被本步骤开放。

回归：持久批准/重启、撤销/回滚、不声明效果/预算拒绝、坏批次零写入、管理
角色伪造/管理业务拒绝、实际 Windows ACL；已编译 CLI 在新 profile 完成
无 GUI 初始化、授权导入、独立重启 list、revoke、重启后的 tombstone 与
冷启动拒绝。六次真实 CLI 启动测试基线约 27 秒，整体限 60 秒，非启动 SLA。
真正 TTY 的 `init --interactive` 提示已手动输入 yes 验证；随后 list 仍为空，
未创建隐式插件授权。全仓 types、lint（零警告）和生成协议核对已通过。
十一 workspace 的全仓 test/build 通过；Rust core 30、native 3、Desktop
Rust 17、Chromium 59，以及全部既有 JS 回归通过。独立 Native/CLI 管理
回归及十项 CI contracts 复验通过。docs、生成 Manifest/协议只读 gate、
Desktop clippy 和实际 production entrypoint 重建 gate 通过。
独立安全审阅与生产批准仍 pending。

相关 SEC-003/004/005/006/007/009/010、ADR-0001/0002 保持 open。Windows
private directory 实现使用官方 [CreateDirectoryW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createdirectoryw)
与 [SetNamedSecurityInfoW](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setnamedsecurityinfow)
的 security descriptor / protected inheritable ACL；实际 ACL 另有 native 回归。

CI follow-up: remote runs 37307885413 and 37307878058 exposed missing runner.json at native lint after removing the client-to-Host dependency cycle. Root build:packages now explicitly includes Native Runtime; its actual Turbo bootstrap graph is regression-tested, including all prerequisite edges and CARGO_TARGET_DIR passthrough. Local parallel bootstrap hit V8 Zone Allocation OOM; serial bootstrap retains all tasks and bounds the local peak. No gate or drift check was removed.

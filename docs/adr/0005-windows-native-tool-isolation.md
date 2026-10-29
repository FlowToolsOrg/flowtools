# ADR-0005：Windows 受管原生工具的进程边界

- 日期：2026-10-29
- 状态：prototype / Windows native feasibility；独立安全批准 pending
- 范围：G5 P2.2a / G6 P2.8b 的 Windows/x64 原生工具前置

选择每任务唯一的 Less Privileged AppContainer（LPAC），只固定启用系统启动
所需的 `registryRead` capability，不授予网络或调用方选择的 capability。
零 capability 在本机能启动程序，但 Winsock 初始化失败（10107），不能冒充
网络拒绝证据；最小启动配置下实际 TCP connect 返回权限拒绝 10013，私有
注册表/文件/IPC 与另一任务目录均拒绝。环境由 T0 从固定 allowlist 构造，
只含 SystemRoot/SystemDrive/LOCALAPPDATA、专用 TEMP/TMP 及 fixture 参数；
没有继承 PATH/USERPROFILE 或 Host 的整个环境。通过原生
`STARTUPINFOEXW` 设置 security capabilities、All Application Packages opt-out、
明确 handle allowlist、子进程拒绝及 image-load mitigations。原生进程先以
suspended + detached 创建，验证实际 token SID、`WIN://NOALLAPPPKG` 结构化
token security attribute 和唯一启用的 capability SID 后加入限额 Job，最后恢复。
不依赖本机尚未实现的 TokenIsLessPrivilegedAppContainer 查询。
任一步失败终止该已持有句柄的进程，不启动普通进程作为 fallback。

每任务 workspace 使用本任务 SID 与当前用户的专用 DACL/Low mandatory label；
程序目录只授予本任务 SID read/execute，保持 Medium label。阻止加载 Low/remote
image，并优先 System32。实际输入由 broker 核准后复制到 workspace；工具不能
获得原始用户路径、Runtime profile、凭据、其他任务目录或继承的 T0 句柄。
工具输出仍须验证后经独立授权 create-only 发布，不直接写目标用户文件。
私有句柄虽然在 T0 被标记 inheritable，实际 LPAC token 进程创建后、恢复前
以 DuplicateHandle/CompareObjectHandles 证明未继承同一内核对象；普通正向
对照明确加入该句柄并确认继承。不能对可能被复用的数字句柄盲读。

原型先在 `apps/runtime/tests/windows-isolation.rs` 的真实子进程中验证，包含
同程序非隔离正向对照。fixture 选择不进入 Runtime/client 配置；没有生产
exec API、任意 argv、签名 pin、工具库存或 grants。六项真实回归与边界见
[平台证据](../validation/g5-windows-native-isolation.md)。仍须实现签名准入到不变
工具身份、文件发布/竞争、IPC、输出预算、撤销与持久 lease，才能连接真实媒体。

此选择不提供 T2/TL WebView 隔离，也不选择/认证 T3 JavaScript 引擎。T3
仍需没有 Node/Bun OS API 的受限引擎与 broker；其他平台未通过时拒绝执行。
AppContainer 与 Job 不能替代 supply-chain、授权和独立安全审核。

依据：[Microsoft AppContainer launch](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer)、
[process attributes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute)、
[mandatory integrity](https://learn.microsoft.com/en-us/windows/win32/secauthz/mandatory-integrity-control)。
原生 token attribute ABI 对照 [phnt 原始声明](https://github.com/winsiderss/phnt/blob/master/ntseapi.h)。
T0 使用 [SHGetKnownFolderPath](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath)
取得当前用户的 LocalAppData，不要求 Turbo 传递该环境变量；返回内存在成功/
失败时均以 CoTaskMemFree 释放。

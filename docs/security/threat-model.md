# FlowTools 插件威胁模型

- 复核日期：2026-10-04；源码基线：`eed0e4d`（P0.4 开始前）
- 范围：Desktop/Web/CLI 插件入口，以及后续包安装、授权与应用更新设计
- 状态：prototype；高风险项全部 open，未接受 production 风险豁免
- 责任：Repository Maintainer 对发布阻断负责；下列 owner 是实施角色，
  尚未指定自然人的角色由 Repository Maintainer 承接，不是风险已被批准
- 相关决策：[ADR-0001](../adr/0001-plugin-trust-boundaries.md)、
  [ADR-0002](../adr/0002-capability-and-package-policy.md)、
  [后续实施设计](../next-milestones.md)

## 资产、攻击者与边界

保护用户文件/剪贴板/凭据、Host 原生权限、插件数据与 grants、发布私钥、
可恢复的安装/数据库状态及宿主可用性。攻击者包括恶意插件作者、被攻陷发布者
或下载源、网络攻击者、恶意远程内容及攻击性输入。签名插件仍视为不可信。

T0/T1 属于 Host 发布信任域；T2/T3/TL 必须位于独立执行域。当前同 realm
插件可绕过 SDK，所以下列目标控制不是现有安全保障。威胁 ID 持久保留；
后续修复只能增加证据与更新状态，不得删除原始缺口来宣称完成。

本模型不覆盖已完全控制 OS/管理员/Host Rust 的对手、硬件攻击或未知 WebView
漏洞。独立容器、CSP、签名均不能消除这些风险。没有第三方安全生产支持声明。

## 风险台账

2026-10-05 P2.3a 新增 [T1 policy broker](../../packages/runtime-core/src/broker.rs)
及 [Runtime 绑定/拒绝回归](../../packages/runtime-core/src/runtime.rs)，并由
[实际 named-pipe fixture](../../packages/runtime-client/test/native-fixture.mjs)
验证无授权时不接收敏感任务。Host 内存授权绑定 caller/publisher/版本/digest/
command；scope、epoch、过期、预算拒绝不会进入 adapter，重新批准不会恢复旧
session。Rust/TS operation schema 拒绝 raw path/SQL/argv/namespace/身份注入。
这是 SEC-003/004/005/006/007/009/010 的基础证据，以上风险继续 open。
没有持久 grant、真实敏感 IO、文件路径竞争/DNS/redirect 防护、用户数据迁移
或第三方 sandbox；既有 Web/Desktop adapters 与 CLI 执行路径仍待后续迁移。
完整范围与独立安全审阅缺口见 [P2.3a 验收](../validation/g3-capability-broker.md)。

### SEC-001 插件包与发布者伪造

- 入口：市场/目录/外部包到安装、加载；高危，open（Tampering/Spoofing）
- 现状：Catalog 是扫描结果，市场操作主要持久化元数据，没有完整签名下载、
  解包校验与原子安装。发现插件不能证明 publisher 或可执行文件身份。
  P0.3a2 相对目录包含稳定 identity、manifest/entry 扫描 hash 与 fixture scope；
  假认证标签、缺失 fixture、路径逃逸和不一致目录被拒绝。hash 未签名，仅证明
  扫描字节/文件存在，不是供应链或实时安装验证；本项仍 open。
- 证据：[市场与 runner](../../apps/desktop/src/App.tsx)、
  [目录生成器](../../scripts/inspect-html-plugins.ts)、
  [Manifest v1](../../packages/sdk/src/manifest/schema.ts)、
  [包文件只读校验](../../packages/sdk/src/manifest/package.ts)、
  [Manifest 拒绝回归](../../packages/sdk/test/manifest.test.ts)、
  [文件拒绝回归](../../packages/sdk/test/manifest-package.test.ts)
- Owner：Plugin Platform / Release；Repository Maintainer 为发布责任人
- 缓解：加载前验证 versioned manifest、文件 hash、可信 publisher、兼容范围；
  staging 隔离，拒绝 Zip Slip、特殊文件、zip bomb；失败恢复完整旧版本。
- 验证：P1.1/P2.5/P2.7；篡改、错签名、同版本不同 hash、不兼容 Host、
  绝对路径、symlink 越界、超量展开与断电 fixture；检查拒绝后未执行入口。
- 残余风险：发布者私钥/CI 被攻陷可产生合法签名恶意代码；需要撤销策略和
  运行时隔离。包协议与 key rotation 仍待实现前评审。

### SEC-002 外部模块进入宿主 realm

- 入口：浏览器 File/Blob module、动态 import；严重，open（Elevation）
- 现状：危险开发预览仍在宿主 document 注入模块，执行后才检查 default
  export。SDK hooks、metadata 校验或 ErrorBoundary 无法限制已运行代码。
  P0.3b1 普通 SDK 文件/外部对象入口已改为副作用前拒绝；危险实现仅在独立
  development 子入口且 Host DEV + 显式 opt-in 下开放。Web 生产不加载它，
  所有模式不自动恢复旧 IndexedDB 源码、不静默删除数据。拒绝 API 与构建模式
  回归见 SDK/Web external-code-gate tests。P0.3b2 CLI 现只加载 Host 构建内嵌清单
  对应的固定编译文件：未知 ID/路径在 IO 前拒绝，缺失/破损不回退源码，删除
  headless rewrite；junction 与 metadata 不一致拒绝。P0.3b3 Desktop 已在 activation、
  fetch、iframe 前默认拒绝，危险实现仅 DEV + opt-in 动态加载；独立 r3 包
  功能/拒绝人工验收已回报通过，运行路径/身份已核实，不认证隔离或用户授权。
  开发预览仍能访问 Host realm，内置加载器属于可信 Host API；签名/隔离/broker
  未实现，本项保持 open，不把此停用策略称作生产 sandbox。
  P0.3b4 新增 post-build 实际产物/child opt-in byte 不变量门禁，覆盖已知危险
  指纹、伪认证语法与环境泄漏探针；不证明任意数据流或第三方包安全。
- 证据：[文件加载器](../../packages/sdk/src/services/plugin-file-loader.ts)、
  [危险开发实现](../../packages/sdk/src/services/development-plugin-file-loader.ts)、
  [SDK 拒绝回归](../../packages/sdk/test/external-code-gate.test.ts)、
  [Web 拒绝回归](../../apps/web-vite/src/app/external-code-gate.test.ts)、
  [CLI discovery](../../packages/cli/src/discovery.ts)、
  [compiled CLI 拒绝回归](../../packages/cli/src/discovery.test.ts)
- Owner：SDK / Runtime / Desktop
- 缓解：按 ADR-0001 分流 T1 与 T2/T3；生产准入在执行前完成；不可信源码
  不进主 realm，未满足隔离条件的平台拒绝执行。
- 验证：P0.3/P1.1/P2.1/P2.2；恶意顶层代码试读 Host DOM、localStorage、
  环境变量及原生 API；生产构建不得落回 Blob 注入路径。
- 残余风险：T1 与 Host 共域，内置依赖被攻陷仍影响 Host；需要发布供应链控制。

### SEC-003 IPC 冒充与通用原生调用

- 入口：HTML postMessage、SDK native.invoke、自定义 Rust commands；严重，open
- 现状：runner 校验 source/envelope，但没有完整 origin/session/nonce schema；
  双向目标为 `*`；native adapter 转发 command 字符串。build.rs 未配置
  AppManifest command permission allowlist。
  P0.3b3 普通 HTML bridge 一律拒绝；有限开发 bridge 在 Host DEV + opt-in 下开放，
  raw native/SQL/FS/opener 均在 payload 读取前禁用。Host context 来自 command 而
  非请求身份，但开发 iframe 仍同 realm、无真正 session/grant，T1 adapter
  仍有 raw native/SQL，风险不关闭。
- 证据：[HTML bridge](../../apps/desktop/src/runtime/html-plugin-bridge.ts)、
  [开发 bridge](../../apps/desktop/src/runtime/development-html-plugin-bridge.ts)、
  [入口回归](../../apps/desktop/test/html-plugin-bridge-gate.test.ts)、
  [构建矩阵](../../apps/desktop/test/html-development-matrix.test.ts)、
  [runner](../../apps/desktop/src/App.tsx)、
  [native adapter](../../apps/desktop/src/runtime/desktop-capabilities.ts)、
  [Tauri build](../../apps/desktop/src-tauri/build.rs)
- Owner：Rust / Desktop / SDK
- 缓解：Host 绑定实际会话身份；typed allowlist、严格 schema、nonce/过期、
  replay/并发/大小限制；原生边界重新授权，应用 commands 配置独立 permission。
- 验证：P2.1/P2.3/P2.7；伪造 source/origin/ID、重放、旧会话、未知 method、
  超大 payload、直接绕过 SDK 的 invoke 都拒绝且无副作用。
- 残余风险：会话存在不意味着调用合法，仍需逐操作 grant 与 scope；Rust
  command 本身的 bug 不由 Tauri capabilities 修复。

### SEC-004 文件 scope 越界与 TOCTOU

- 入口：fs 路径、dialog 返回路径、CLI storage key；高危，open
- 现状：Desktop 将路径交给官方 FS 插件，未绑定 per-plugin scope；P0.2b1 CLI
  验证 kebab-case 插件 ID、限制 storage key 字符并拒绝 Windows 设备保留名，
  合法已有键文件位置不变。未实现 canonical/symlink/reparse/TOCTOU 防护，仍 open。
  本记录不证明官方插件能读取所有系统文件。
- 证据：[Desktop FS](../../apps/desktop/src/runtime/desktop-capabilities.ts)、
  [CLI context](../../packages/cli/src/context.ts)、
  [主窗口 capability](../../apps/desktop/src-tauri/capabilities/default.json)
- Owner：Rust / SDK / CLI
- 缓解：broker 发放 scoped handle，规范化并校验实际目标及写入父目录；
  防符号链接/junction 逃逸和检查后替换，不接受 caller 指定根目录。
- 验证：P2.3/P2.7；`../`、UNC、drive、ADS、大小写、symlink/reparse、
  新文件父目录及路径竞争 fixture；删除/写入越界都拒绝。
- 残余风险：OS/FS 语义不同，必须按支持平台单独验证；字符串前缀不是充分证明。

### SEC-005 网络出站与凭据外泄

- 入口：SDK request、插件自身 fetch/资源请求、重定向；高危，open
- 现状：Desktop/Web/CLI 直接 fetch；Desktop CSP 为空，没有统一 host/port/
  redirect 策略。未证明跨 origin 请求或所有 private 地址都会成功。
  P0.2c 网站延迟 built-in 的 run/panel 已走 SDK request；缺失能力不会退回 raw
  fetch，受控 fixture 不访问公网。adapter 最终仍 fetch，同 realm 代码仍可绕过，
  网络 broker/出站限制并未实现，本风险保持 open。
- 证据：[Desktop request](../../apps/desktop/src/runtime/desktop-capabilities.ts)、
  [Web context](../../apps/web-vite/src/runtime/ctx.tsx)、
  [Tauri CSP](../../apps/desktop/src-tauri/tauri.conf.json)
- Owner：Rust / Desktop / Runtime
- 缓解：broker 网络 allowlist、逐跳重定向/DNS/IP 检查、超时/配额、不继承
  Host cookie/token；CSP 限制直接请求，进程级出站限制覆盖 T3。
- 验证：P2.1/P2.2/P2.3/P2.7；localhost/private/link-local、DNS rebinding、
  HTTPS→HTTP 重定向、跨域跳转、凭据与超量响应 fixture。
- 残余风险：用户允许的目标仍可能收集授权数据；CSP 不是完整 DLP 或网络授权。

### SEC-006 数据库跨插件读写与迁移丢失

- 入口：db.query/CRUD、localStorage、Debug 初始化；严重，open
- 现状：插件 DB 共享 `sqlite:flowtools.db`，query 接收 raw SQL；storage key
  前缀不是恶意代码边界。Host `app.sqlite` 在 Debug 启动删除；Rust repository
  的元数据校验和内存测试不等于插件数据隔离或生产 migration。
  P0.2c Todo run 使用面板共享 app store；CLI 保留旧 key，损坏数据拒绝写入，
  无能力时不伪报保存；这不是通用 migration/备份/隔离。
- 证据：[DB adapter](../../apps/desktop/src/runtime/desktop-capabilities.ts)、
  [数据库初始化](../../apps/desktop/src-tauri/src/db/init.rs)、
  [repository](../../apps/desktop/src-tauri/src/repositories/plugin_repository.rs)
- Owner：Data / Rust / Desktop
- 缓解：Host 绑定 namespace；取消 raw SQL；Core/grants/data 分开 ownership；
  删除隐式 reset，版本化幂等迁移、备份与兼容窗口。
- 验证：P2.3/P2.6/P2.7；跨插件读写/猜测表名、注入、空库/N-1、迁移中断、
  重启及 downgrade fixture；确认用户数据与 grants 不丢失。
- 残余风险：合法授权写入也可能损坏插件数据；恢复需保留备份并做健康检查。

### SEC-007 权限声明冒充用户授权

- 入口：manifest permissions、升级、权限 UI、活跃会话；严重，open
- 现状：adapter 用 Set(permissions) 注入能力；不存在绑定 package identity
  的持久 grant/revocation broker，UI 状态不能作为授权证据。
- 证据：[capability 注入](../../apps/desktop/src/runtime/desktop-capabilities.ts)、
  [权限页面](../../apps/desktop/src/App.tsx)
- Owner：Desktop / Rust / Product
- 缓解：默认拒绝、显式 user grant、scope diff；升级重新审查；撤销 epoch
  使排队/旧会话调用失效；权限展示与 broker decision 同源。
- 验证：P2.3/P2.4/P2.7；未声明/拒绝/撤销/重启、同 ID 换 publisher/hash、
  升级扩大权限与撤销竞争 fixture；人工授权弹窗和恢复路径验收。
- 残余风险：用户可能被插件诱导授权；Host 授权 UI 必须与插件 UI 可区分。

### SEC-008 应用更新、签名密钥与恶意回滚

- 入口：未来 updater、release CI、trust store、last-known-good；严重，open
- 现状：Cargo/Host 未接入官方 updater，没有已验证的签名发布与回滚链。
  Windows quality workflow 是验证构建，不是发布认证。
- 证据：[Cargo 依赖](../../apps/desktop/src-tauri/Cargo.toml)、
  [Host builder](../../apps/desktop/src-tauri/src/lib.rs)、
  [质量工作流](../../.github/workflows/windows-quality.yml)
- Owner：Release / Security；Repository Maintainer 为密钥与发布责任人
- 缓解：官方 updater 签名验证、密钥隔离/轮换/撤销、TLS、受控回滚与
  防降级策略；验证所有实际发布 artifact，不继承撤销权限。
- 验证：P2.5/P2.7/P3.5；错误签名、篡改/截断包、撤销 key、降级、离线与
  更新中断 fixture；secret canary 不进 bundle/日志。
- 残余风险：合法发布 key 被攻陷、更新可用性和离线撤销传播仍需运维策略。

### SEC-009 日志、错误与历史泄露

- 入口：ctx.log、bridge error、CLI stderr、history/诊断导出；高危，open
- 现状：P0.2b1 CLI 不再自动输出插件 log/details，执行元数据只有输入形状；
  JSON/parser 和输出序列化错误不回显原文，runner 仍返回实际异常 message。
  P0.2b2 Web/Desktop 使用 versioned、限额 200 的元数据-only 历史，不保存原始
  input/output/message；旧未验证 key 保留且不导入，损坏记录与写失败可见。
  缺少统一脱敏审计与保留/配额策略，普通运行历史不能证明授权决定，仍 open。
- 证据：[CLI logger](../../packages/cli/src/context.ts)、
  [bridge error](../../apps/desktop/src/App.tsx)、
  [Web history](../../apps/web-vite/src/stores/run-history-store.ts)
- Owner：Runtime / CLI / Security
- 缓解：稳定错误码、结构化事件、敏感 payload 默认省略；grant/安装决定单独
  审计；限额/保留期/导出 scope；不记录完整 URL query 或文件/剪贴板内容。
- 验证：P1.5/P2.4/P2.7/P3.6；token/路径/剪贴板 canary、换行注入、超量日志
  与导出 fixture；检查 stdout/stderr/history 和原生日志。
- 残余风险：本机管理员可篡改日志；脱敏可能误漏，需持续红线回归。

### SEC-010 无限循环、灾难正则与资源耗尽

- 入口：UI setup/run、CLI import/run、HTML bridge 洪泛；高危，open
- 现状：旧 SDK watchdog 只触发 abort；新 SDK executor/CLI race 可界定异步等待、
  转发取消并丢弃迟到输出，Web/Desktop 同样接入，UI 卸载会取消当前尝试；
  但不能抢占同步循环或撤回已发生副作用；
  UI/Headless 同进程没有完整 CPU/内存/输出/并发硬配额。
- 证据：[SDK watchdog](../../packages/sdk/src/registry/watchdog.ts)、
  [T1 生命周期与资源回收](../../packages/sdk/src/registry/plugin-loader.ts)、
  [资源与在途调用回归](../../packages/sdk/test/command-projection.test.ts)、
  [CLI runner](../../packages/cli/src/runner.ts)、
  [正则插件](../../plugins/plugin-regex-tester/index.tsx)
- Owner：Runtime / Desktop / CLI
- 缓解：可终止边界、wall-clock/CPU/内存/输出预算、grace period 后强杀；
  会话级调用限额、crash budget 与 quarantine。
- 验证：P2.1/P2.2/P2.7；死循环、忽略 abort、内存/输出洪泛、崩溃 fixture；
  deadline 后 Host 保持响应、无孤儿进程/未释放资源。
- 残余风险：共享 OS 资源或 WebView 引擎缺陷仍会影响 Host；UI 隔离需平台证明。
  G2 P1.2a/b 每插件锁、代际 handler 与资源 scope 仅约束合作 T1；hook/handler
  忽略清理或同步阻塞仍可阻止排空，不是强制终止或进程沙箱，本项仍 open。

### SEC-011 Legacy preload、资源与远程导航

- 入口：catalog main/development.main、preload、iframe 导航；严重，open
- 现状：runner 注入 preload 并提供宽松 iframe sandbox；开发 fallback 与
  本机静态目录不是发布包认证；支持级别仅表示需要的 API 类型。
  P0.3a2 发布 Catalog 不含 checkout root、development URL 或源码 main；47 项
  entry-resolved 只证明扫描时普通文件存在，78 项 indexed；本地入口仅在显式
  DEV + 开发 checkout 配置下解析。源态/remote/越界拒绝 fixture 在
  [Catalog 回归](../../scripts/catalog.test.ts)，不证明 iframe/preload 隔离，仍 open。
- 证据：[HTML 注入器](../../apps/desktop/src/runtime/html-plugin-bridge.ts)、
  [iframe runner](../../apps/desktop/src/App.tsx)、
  [HTML normalizer](../../packages/sdk/src/compat/html-plugin.ts)
- Owner：Desktop / Compatibility / Security
- 缓解：TL 独立 origin/session/CSP；preload 只在隔离容器执行；资源仅来自
  已验证包；导航重建/撤销会话；生产禁止未认证 remote fallback。
- 验证：P0.3/P2.1/P2.7/P3.4；preload 改父页面、远程跳转/嵌套 frame、
  development URL 与源态 Vite 入口 fixture；每项旧 API 独立认证。
- 残余风险：旧插件依赖 Node/Electron 私有接口可能永不支持；不承诺 125 个兼容。

### SEC-012 Opener 绕过 SDK grant 与外部协议

- 入口：HTML opener.openUrl/openPath/revealItemInDir；高危，open
- 现状：bridge 分支直接调用官方 opener，不经过 requireCapability；实际行为
  仍取决于 Tauri opener permission/scope，未进行 per-plugin 授权。
- 证据：[opener 分支](../../apps/desktop/src/runtime/html-plugin-bridge.ts)、
  [主窗口权限](../../apps/desktop/src-tauri/capabilities/default.json)
- Owner：Desktop / Rust / SDK
- 缓解：独立 typed opener operation、用户交互及规范化 URL/path allowlist；
  不继承 main capability；拒绝 file/custom scheme 与外部应用参数滥用。
- 验证：P2.3/P2.4/P2.7；未授权 opener、恶意 scheme/路径/参数、跳转与
  用户取消 fixture；拒绝时无外部应用启动。
- 残余风险：获准打开的外部应用仍可能不安全；需要用户提示与可撤销 scope。

### SEC-013 CLI 冷启动、后台任务与调用主体混淆

- 入口：规划中的本地 runtime IPC、CLI bootstrap、后台 job/调度；高危，open
- 现状：CLI 仍在自身进程调用 T1 compiled inventory；尚无无界面服务、Host-bound
  client roles、冷启动 grants 或持久任务恢复。新增设计不会让 run flag 获得授权，
  不声称同用户 token 可隔离同用户恶意进程。
- 证据：[CLI runner](../../packages/cli/src/runner.ts)、
  [CLI context](../../packages/cli/src/context.ts)、
  [当前执行契约](../../packages/sdk/src/execution/executor.ts)
- Owner：Runtime / CLI / Rust；Repository Maintainer 负责开放入口决策
- 缓解：当前用户 ACL、有限 IPC、Host 会话/roles、命令与 scope 授权、epoch；
  bootstrap 仅启动受管内核，CLI-only 显式管理初始化不加载插件；durable accepted
  后 ACK，幂等键绑定包/hash/lock/action digest，失联按同 key 恢复；非幂等中断
  不重放，私有 job payload 与历史分离；GUI/CLI 不建两套 grants 或业务数据。
- 验证：P1.3/P1.4/P1.5/P2.4/P2.6/P3.1；伪造 role/ID、错协议、并发启动、
  stale session、CLI-only 首次初始化、相同幂等键不同输入/包锁、提交 ACK 与发送
  确认丢失、GUI 关闭与权限撤销/调度重复。
- 残余风险：同一 OS 用户被攻陷可访问其本地接口/凭据；外部副作用不能通用回滚
  或保证 exactly-once；未验收平台/运行方式不开放。

### SEC-014 服务依赖代理越权与版本漂移

- 入口：规划中的依赖 resolver、插件服务 RPC、更新/卸载；高危，open
- 现状：当前 Registry/Loader 不提供生产依赖 DAG、不可变锁或服务委托身份；
  插件依赖是新设计范围，未通过 fixtures，不能推断前置插件有安全共享机制。
- 证据：[插件契约](../../packages/sdk/src/types/plugin.ts)、
  [命令契约](../../packages/sdk/src/types/command.ts)、
  [插件 loader](../../packages/sdk/src/registry/plugin-loader.ts)
- Owner：SDK / Runtime / Security
- 缓解：明确 publisher/interface/version、确定 DAG/lock、服务单 profile 单版本；
  Host 保留 root caller/provider/parentRunId、可委托 handles、剩余预算与 epoch，
  调用方和 provider scope 共同约束；服务排空后切版本，禁止同空间新旧双写；
  在途 lease 与反向依赖检查，未准入第三方依赖保持 plan-only。
- 验证：P1.6/P2.3/P2.4/P2.7；循环/冲突/假 publisher、A 借 B 读取无权文件、
  参数注入、递归/输出洪泛、取消传递、排空失败/双写、撤销竞争与并发更新卸载
  拒绝矩阵。
- 残余风险：合法服务可能错误处理受托数据；用户对整条调用链的理解仍有限，
  需明确 provider 与效果展示，不把签名或依赖安装当授权。

### SEC-015 共享工具包、间接 IO 与进程恢复

- 入口：规划中的工具侧载/下载、artifact/lease/GC、媒体进程；严重，open
- 现状：当前只有 Tauri 官方能力 adapter，无集中工具包准入、共享版本仓库或
  已验证的原生工具访问约束。FFmpeg 等工具仍属待实现目标，默认入口未开放。
- 证据：[Desktop adapter](../../apps/desktop/src/runtime/desktop-capabilities.ts)、
  [Rust 依赖](../../apps/desktop/src-tauri/Cargo.toml)、
  [当前 Host 初始化](../../apps/desktop/src-tauri/src/lib.rs)
- Owner：Runtime / Rust / Security / Release
- 缓解：签名/hash/来源/平台/build flavor 准入、无安装脚本、typed operations、
  scoped handles/协议/输出提交、实际平台 file/network 限制、任务私有环境和
  进程树预算；辅助 executable/DLL 清单及受控 loader 路径；不可变 artifact、
  journal、accepted 起生效的任务 leases、持久恢复引用、回滚根集合和 GC 宽限期。
- 验证：P2.5/P2.8/P2.7；错误签名/篡改、路径替换/外部引用/raw argv/未知工具、
  可写 cwd/PATH 的伪造 DLL/辅助程序、未授权网络/文件、进程后代/Host 崩溃、
  排队/恢复任务 GC、PID 复用、并发安装与更新撤销。
- 残余风险：合法签名工具仍可能有漏洞；参数限制与 Job Object 不构成权限沙箱，
  支持平台须实测；磁盘/数据库不同资源的提交需要恢复日志，不保证天然原子。

## 复核与关闭规则

新增入口/能力、包协议、平台、授权或恢复策略必须更新对应威胁 ID 与 ADR，
提供拒绝路径自动化证据和人工验收记录。只有对应实现里程碑退出标准通过后
才能将风险改为 mitigated；接受例外需明确审批人、原因、到期日与发布范围。
本次设计完成不关闭任何高风险项，也不升级任何插件成熟度。

官方机制边界已核对：Tauri
[capabilities](https://v2.tauri.app/security/capabilities/)、
[permissions](https://v2.tauri.app/security/permissions/)、
[CSP](https://v2.tauri.app/security/csp/) 与
[updater](https://v2.tauri.app/plugin/updater/)。这些机制是实施基础，不证明当前
配置安全；尤其 CSP 必须启用，Rust commands 必须自己正确实施授权与 scope。

## P0.3c 状态说明收口

市场内置按钮只保存配置，HTML 不可安装；权限页显示授权/隔离缺口。
证据：[展示策略](../../apps/desktop/src/runtime/catalog-presentation.ts)、
[回归](../../apps/desktop/test/catalog-presentation.test.ts)。
没有新增 native API、grant、数据迁移或第三方执行入口；SEC-001/002/006 保持 open。

P1.1b：固定 CLI [Manifest/文件导入校验](../../packages/cli/src/discovery.ts)、
[导入前拒绝与 runtime 不匹配回归](../../packages/cli/src/discovery.test.ts)、
[无 React/GUI/source 消费](../../plugins/test/headless-artifacts.test.ts)。
身份来自编译 inventory；文件 hash 无签名，不能抵御同时替换可信产物和 catalog，
亦不解决校验后的替换竞争。SEC-001/002/010 保持 open，ADR 无设计变更，
独立安全 Reviewer pending；没有新增 grant、可撤销安装或隔离声明。

P1.1c：三端固定 T1 import 前契约校验使用
[SDK loader](../../packages/sdk/src/manifest/loader.ts)，
[拒绝与 registry 状态回归](../../packages/sdk/test/manifest-loader.test.ts)。
CLI [参数解析](../../packages/cli/src/command-schema.ts) 与
[compiled 回归](../../packages/cli/src/discovery.test.ts) 验证机器发现不 import code、
未知/混合参数预检拒绝和真实 handler 参数值。Web/Desktop JSON run 使用相同
Manifest executor；SDK 回归覆盖非法 runtime validator/default 后预算。
身份由固定 callback/inventory 绑定，权限取命令请求与 host metadata 的交集；
这不是持久 grant 或撤销服务。batch 顺序执行但没有事务/回滚，实际异常仍有
副作用残余风险。SEC-001/002/005/010 保持 open，ADR 无设计变更，工程自检通过，
独立安全 Reviewer pending；没有新增外部安装/可执行路径/原始 native API。

### G2 P1.3a engineering evidence (2026-10-04)

SEC-001/003/006/009/010/012 remain open. Sources: runtime-core catalog/protocol/
runtime, apps/runtime server/security, plugin-runner, runtime-client and Desktop
validation_runtime.rs. Rejection evidence: exact embedded Manifest metadata,
unknown IDs/input/side effects, connection-bound proofs, native origin/identity/
mode table, current-user protected pipe ACL and duplicate first listener. GUI
and Node query one Runtime task in a disposable profile. Token never reaches
Desktop JS; native code supplies the endpoint and caller credential. Event data
is state/sequence/runId only. Revocation is connection close; foreground jobs
cancel and background jobs keep their explicit validation submission contract.
Profiles preserve fixture workspaces, with no automatic durable job recovery.
Root/workspace build gates cover generated artifacts; no reviewer approval is
implied. ADR-0001/0002 accepted designs unchanged. Same-account compromise,
TOCTOU replacement, process trees, OS budgets, third-party sandbox, persistent
grants and Debug user-DB reset are not resolved. Security Reviewer/date/conclusion:
independent reviewer pending / not approved; Codex engineering checks only.

### G2 P1.5a diagnostic evidence (2026-10-04)

SEC-003/006/009/010/012 remain open. Source/rejection evidence adds Rust codegen
and runtime deadline/event tests, TS codec/diagnostics golden/privacy tests,
actual Node raw frame/version/injection/foreground disconnect/output budget tests
and managed child package mismatch/deadline kill/wait. Native identity checks now
require the actual configured origin, preventing alternate localhost port reuse.
Oversized request IDs are bounded before replay-cache insertion; frame and output
budgets cannot publish private errors. Production artifact tests and post-build
gate also refuse the DEV validation panel. Recovery remains explicit same-instance
query; no automatic retries, grants or persistent payloads are introduced.
ADR-0001/0002 unchanged. Reviewer/date/conclusion: independent reviewer pending /
not approved; Codex engineering verification only. Residual same-user compromise,
TOCTOU, process trees/OS budgets, durable recovery and Debug DB reset remain.

Validation startup evidence: metadata-only preflight reads actual compiled config
before a GUI/Host starts; unsupported stale binaries refuse before execution.
Explicit validation rejects the default identifier before Builder/plugins/DB IO.
This repairs a reproduced harness identity bug: Cargo tests rebuilt the default
binary, and a literal-ID scan allowed its 23:33 Debug DB reset. Data preservation
was declined; no pre-reset recovery is claimed. Ordinary Debug reset remains open.

2026-10-05 P2.6a update: Runtime single-writer SQLite, CAS/transactions,
N-1 migration/rollback, explicit source-preserving import and backup recovery
are covered in [shared data acceptance](../validation/g3-shared-data.md). Ordinary Desktop Debug reset
is removed and corrupt legacy DB startup preserves the original. Historical
G2 reset evidence above remains accurate for that incident; no lost user data
recovery is claimed. Durable grants/jobs and third-party boundaries remain open.
Security Reviewer/date/conclusion: pending independent review.

2026-10-05 P2.4a: [persistent policy evidence](../validation/g3-persistent-grants.md) adds Host-bound management, current-user private profile ACL, atomic grant import and durable epoch/audit. T1 only; same-account compromise, isolation, signing and independent Security Reviewer/date/conclusion remain pending.

2026-10-05 P1.3b: [durable job evidence](../validation/g3-durable-jobs.md)
binds receipts and replay to Host identity, immutable action/package lock and
durable grant epochs. Private DPAPI payloads are separate from metadata. Running
non-idempotent tasks interrupt after crash. This T1 lifetime guard is not a
third-party sandbox; SEC risks and independent security review remain open.

### G3 P1.4a standalone CLI

Windows x64 CLI-only bundle and bounded, authenticated cold-start coordination
are implemented for the fixed T1 inventory. See [standalone CLI evidence](../validation/g3-standalone-cli.md).
The package pins relative compiled artifacts and supports explicit GUI-free init,
runtime start/status/stop and granted bundle execution. Source CLI/GUI integration
and operational diagnostics remain P1.4b/P1.5b. Maturity remains prototype;
signed releases, third-party sandbox and independent security review are pending.

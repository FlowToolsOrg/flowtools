# Manifest v1（P1.1a）

公开纯数据契约位于 `@flowtools/sdk/manifest`；Node 文件完整性验证独立位于
`@flowtools/sdk/manifest/package`。Manifest 只包含 JSON，不包含 run/setup、
loader、Zod 函数或 React 对象。`parsePluginManifest(value, target)` 在代码
加载前检查 canonical semver、Host/SDK range、平台/架构配对和严格字段。
命令身份为 publisher/plugin/command；UI-only 包可没有 commands/executor。

Manifest 包含 formatVersion=1、publisher/id/name/description/version/type、
maturity（缺省 prototype）、engines、targets、entries、files、commands、
dependencies 和 signature。v1 当前仅支持 unsigned 标记，签名协议与 trust
root 在 P2.5a 验收；校验成功不允许安装第三方或证明 publisher 身份。

每个 command 必须声明 name/description、inputSchema/outputSchema、
runtimeValidation、headless/supportsColdStart/interaction、effects、
permissions（capability/operations/scopes）和 resources（timeoutMs、
maxInputBytes、maxOutputBytes）。这些是请求和预算，不是 grant；冷启动、
后台任务、服务/tool 执行与独立 CLI 发行继续等待各自后续 gate。

## Schema 子集

支持 object/array/string/number/integer/boolean/null、properties、required、
additionalProperties（显式 boolean）、items、enum、const、default、anyOf、
字符串/数组长度和数值上下界。每个 array 必须有 items，object 必须明确
额外字段策略；拒绝未知关键字、$ref、pattern、format、无限/循环对象和
非 JSON 值。输入顶层必须 object，默认值只在输入校验时填入；输出不填默认值。
不存在“任意 JSON Schema”支持声明。JSON 树限制 32 层/100k 节点，单 Schema
64 Ki 字符；Manifest 1 Mi 字符，包文件最多 1024 个/总计 64 MiB。
命令输入/输出预算按 UTF-8 字节计量；超限返回稳定 INPUT_INVALID/OUTPUT_INVALID。

`exportOperationSchema(zodSchema, 'required')` 明确保留 Zod runtime authority。
自定义 refinement/transform 不能声明 schema-only；不可导出的结构拒绝导出，
不能以空 schema 冒充完整验证。显式输入/输出 runtime validator 缺失时拒绝执行。
`migrateLegacyManifest` 只接受旧元数据与完整的显式包/发布者/操作声明，
不猜权限、路径、publisher 或签名，不把旧函数对象写入协议。

## 文件与执行验证

`verifyManifestPackage(root, manifest, target)` 只读 staging/T1 固定产物目录，
manifest 作为目录外参数传入；逐段拒绝 symlink/junction 与 redirected root，
拒绝额外/缺失/特殊文件、大小错误与 SHA-256 不符。入口必须在文件清单里。
验证不 import 任何代码；文件 hash 未签名，检查后替换的竞争、安装事务与
完整供应链控制继续由 G5 负责。本函数不是授予包加载的通用第三方 API。

`executeManifestCommand` 是显式 legacy run adapter，复用 executePlugin 的
identity、取消/timeout/envelope，并验证真实输入和输出。P1.1b 的 CLI 已接实际
T1 命令分包；P1.1c 扩展机器发现。拒绝路径不修改 registry、
持久状态或 user data；已经发生的业务副作用无法被输出校验回滚。JS 执行仍非 sandbox。

## P1.1a 证据

2026-10-04，SDK lint/check-types/build 与 125 tests 通过（39 项新增 Manifest/
package 回归）；只读 `verify:manifests` 通过协议 fixture。根七 workspace
lint/check-types/test/build 全部通过，tests 无缓存。根门禁记录在本地
`execution-validation/g1a/`。当前仅协议 fixture，不能宣称十二真实包已迁移。
没有 UI 行为变更，人工界面验收不适用。威胁 SEC-001/002/010，ADR-0001/0002
无设计变更；Codex 当日工程自检结论为拒绝路径通过，独立安全 Reviewer pending。
所有风险仍 open，第三方普通入口继续 deny-only；不新增签名安装、grant 或隔离。

## P1.1b 实际构建入口

十二插件分别保留 UI `<id>.js` 并新增纯命令 `<id>.commands.js`，没有 React、
UI/Tauri import。UI setup 通过同一 typed command 对象兼容原行为；Todo
继续使用现有 host store，CLI 仍使用原声明 storage。完整输出 runtime schema
与序列化 operation schema 同源；URL 格式、正则命名组的动态键由显式 runtime
validator 验证，不宣称有限 JSON Schema 表达了这些额外规则。

构建把实际 dist 文件清单/字节 hash 和命令生成到目录外
`plugins/.generated/builtin-manifests.json`。当前十二 Manifest 引用同一内置
发行目录的完整文件清单，这是 monorepo T1 构建，不是十二个独立签名安装包。
CLI 仍只接受编译进自身的固定 ID；执行前验证完整 Manifest、目标兼容与包文件，
然后导入固定 command 文件并核对 runtime metadata/validator/command。
浏览器 UI 是 bundler 绑定的 T1，外部入口继续拒绝；本步不引入第三方安装。

Node 消费回归复制实际 command 依赖图到源码 checkout 外的临时目录，React/UI
不可解析，逐个执行十二真实命令并验证输出、错误输入零副作用、预取消。
完整 smoke 另验每个实际结果/default/text 语义；named capture 缺失值转为可序列化
空 groups，保持原 JSON 输出。CLI 另验畸形 Schema/穿越入口/篡改字节在 import
前拒绝，合法完整性 fixture 单独验证八种 runtime contract 不匹配。

本轮根门禁与前端验收记录位于本地 `execution-validation/g1b/`；Web 和 Desktop
frontend 实际 command/panel 检查不等于 native IPC 或独立安装包验收。
SEC-001/002/010 仍 open；工程自检通过，独立安全 Reviewer pending。

2026-10-04：根 docs/catalog/lint/types/test/build 和 verify:manifests 均通过，
七 workspace 测试零缓存（SDK 125、CLI 53、plugins 43、Web 22、Desktop TS 61、
Rust 13、UI contract 3、Chromium consumer 59）。生产入口 Web 22/Desktop 25
文件在 opt-in/canary 重建后字节相同。Web production preview 实验了真实 Base64、
错误输入、键盘、历史/刷新/清空、network 取消与 Todo 共享 panel。Desktop frontend
重新启动后配置保存/启用、真实 Base64 和权限状态检查通过；IPC 为受控 fixture，
没有启动原生用户应用。最初旧 dev server 在重建期间失效导致一次超时，保留失败
日志；重启后复验成功。截图已检查，本步没有设计或标签视觉变更。

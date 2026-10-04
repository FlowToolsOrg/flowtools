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
identity、取消/timeout/envelope，并验证真实输入和输出。当前 host 仍使用旧
T1 固定入口；P1.1b 接实际分包，P1.1c 扩展 CLI 发现。拒绝路径不修改 registry、
持久状态或 user data；已经发生的业务副作用无法被输出校验回滚。JS 执行仍非 sandbox。

## P1.1a 证据

2026-10-04，SDK lint/check-types/build 与 125 tests 通过（39 项新增 Manifest/
package 回归）；只读 `verify:manifests` 通过协议 fixture。根七 workspace
lint/check-types/test/build 全部通过，tests 无缓存。根门禁记录在本地
`execution-validation/g1a/`。当前仅协议 fixture，不能宣称十二真实包已迁移。
没有 UI 行为变更，人工界面验收不适用。威胁 SEC-001/002/010，ADR-0001/0002
无设计变更；Codex 当日工程自检结论为拒绝路径通过，独立安全 Reviewer pending。
所有风险仍 open，第三方普通入口继续 deny-only；不新增签名安装、grant 或隔离。

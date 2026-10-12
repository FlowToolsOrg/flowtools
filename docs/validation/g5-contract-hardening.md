# G5 契约与签名归档身份加固

日期：2026-10-12。状态：prototype；本轮本地完整集成门禁通过，当前提交的
远端 CI、P2.5b/G5 整体验收与实际独立安全批准 pending。

## 保留的实现

本轮以已有 G5 六步实现为基础，保留 SDK 结构 exporter、严格 ZIP 准入、
持久 descriptor trust/floors、签名 service 依赖规划、Windows/x64 T0 惰性
库存/journal 与持续父目录保护发布。不是将这些已交付控制重新记为未实现。
旧步骤的通过记录属于其各自提交，不能作为本轮修改后的门禁结果。

## SDK 与 native 数据契约

`packages/runtime-core/manifest-conformance.ts` 从刚构建的 SDK 公共
`@flowtools/sdk/manifest` 出口调用实际 `parsePluginManifest`，生成 405 项
接受/拒绝样本，其中 92 项接受。SDK source 测试和 Rust 使用同一 JSON；
保留原有 90 项 schema-export corpus 与十二个真实内嵌 Manifest 检查。
未知/叠加 custom checks、transform 与未支持结构仍在生成时拒绝，不能以
手写 DTO 替代 SDK exporter。

生成器默认只读 `--check`，只有显式 `--write` 更新语料。Core 标准 generate、
verify:bindings 与 test 都执行只读检查；可丢弃副本回归要求篡改后失败且
不改写文件，显式写入只恢复该副本。readonly check 不证明任意 SDK/Rust
语义天然相同，新增 refinements 仍须同步 native semantics 与实际样本。

`packages/runtime-core/src/packages/manifest/operation.rs` 实现有限 operation
schema 与 default/enum/const 比对，保留未知字段、递归、危险属性、结构默认值
和类型限制。JSON 深度 32、节点 100000、字符串/Manifest JSON.stringify
UTF-16 单位 1 MiB、operation schema 64 KiB 等预算继续执行；匹配另有
一百万步预算，耗尽返回 ManifestBudgetExceeded。这是安装 profile 限制，
不能声称 SDK 拒绝全部超预算样本，也不会执行 schema、加载 URL 或注入默认值。

精确 JSON 浮点解析使用 `serde_json` 的 `float_roundtrip`，避免先解析后
二次舍入改变 JavaScript Number 等价关系。原始签名 UTF-8 和 ZIP 字节保持
权威；规范化 metadata 不改写原始文件或 packageDigest。实际样本包括
不同数值字面量舍入为同一 Number，以及相邻字面量必须保持不同的反例。
`ryu-js` 仅用于 NumberToString/预算，不是 JavaScript 执行环境。

版本 range 限额按 UTF-16 单位计算，空白遵循 ECMAScript。NBSP/BOM 的 256
单位样本、BOM 空 range/BOM+caret 接受；257 单位和 NEL 拒绝。SDK 关闭
对象通过自有 property 查询匹配，`toString`、`valueOf`、`hasOwnProperty`、
`__defineGetter__` 不能借原型属性绕过 additionalProperties:false；显式
自有声明和 additionalProperties:true 仍接受。语料还核对目录路径 sigma
上下文小写、ZWJ 与 combining mark，避免未来改成逐字符小写。

公开 Compatibility 构造只读取 T0 编译的 Host/SDK 版本与平台/架构，字段
crate-private；包、IPC、run flag 不能指定兼容环境。当前 build-owned target
不因此扩大支持平台，tool descriptor 也不能取得 plugin 准入身份。

## 独立签名 ZIP 身份

G4 metadata semantic pins 保留旧含义。`SignedArchivePin` 另绑定所选
archive 的原始 ZIP `packageDigest` 与实际准入 target，
`SignedArchiveLock` 使用显式 `signed-plugin-archives-v1` profile。签名锁
不复用 G4 v1 lock，solver 不得将未选择/同名/其他 ZIP 或 target 替代根或
provider；原有拓扑、反向依赖、服务/interface 与搜索预算继续约束内部解析。

任意 metadata 的跨语言 canonical digest 仍 pending；独立 ZIP artifact
绑定已经有专门数据身份，不以 Rust/JS 任意 JSON 序列化相同为前提。签名、
兼容性、依赖锁和惰性安装记录仍不能创建 grant、runner/session 或可执行身份。
当前根、撤销、expiry、持久观察和 release floors 必须继续在准入及最终
安装提交时重检，不能以曾经验签的缓存 receipt 代替。

库存与 pending journal 的 metadata key 保持
`package-install-inventory-v1` / `package-install-pending-v1`，库存与 journal
外层 formatVersion 改为 2。旧 prototype 格式返回 StateInvalid，保留原记录字节
与 trust floors；没有自动转换、清空、降 floor 或重新 bootstrap。
此拒绝不是已实现生产升级迁移；受控迁移/恢复须另有明确协议与验收。

## 本轮验证状态

已在固定 Bun 1.3.14 上通过 SDK 构建、lint、类型检查，30 个测试文件共
786 项测试/2880 条断言通过；实际编译 SDK 重新生成 corpus 后只读检查一致。
完整集成门禁首次失败已保留日志，不记为通过。既有 queued/running 状态观察
竞态修复保持原业务断言、原 25 秒截止时间及 crash/recovery 检查，没有重提
任务、测试失败重试或扩大超时。修改后的完整门禁退出 0，实际结果如下；没有
借用旧 G4 或 G5 成功记录代替本轮结果。

- 固定 Bun 1.3.14 / Rust 1.96.0；workspace task、docs、portable catalog、
  package prerequisites、host generators 和 Rust-derived Runtime contracts
  检查通过。
- 全仓 lint 11/11 强制执行通过（27.913s）；类型检查 11/11 强制执行通过
  （24.79s）；test 11/11 零缓存通过（6m30.632s），随后顺序 build 11/11
  强制执行通过（51.418s）。
- SDK 786 项/2880 条断言；Core 169 项及 1 项外部兼容环境构造 compile-fail；
  Runtime 24 项真实 Windows 原生测试（64.62s）和 16 项 JS 集成测试通过。
  包含真实 Todo 崩溃恢复、独立发行包 50 并发启动和六条异常排空路径。
- Desktop 90 项 JS、21 项 Rust 和 3 项 binding，以及固定 Chromium 72 项
  回归通过。CI corpus 副本篡改/只读拒绝/显式恢复回归包含在 Desktop 测试中。
- Web 22 / Desktop 24 个生产产物在 opt-in/canary 重建后 byte-identical，
  已知不安全入口指纹、认证开关和探针未泄露。Desktop Rust fmt/check/
  all-targets codegen clippy `-D warnings` 通过。

当前提交的远端 CI 以 [PR #11 Checks](https://github.com/FlowToolsOrg/flowtools/pull/11/checks)
为准；本地通过不代替远端 clean-checkout 验收或独立人工安全批准。

G4 六条真实 Windows/T1 异常排空基线见
[服务调用证据](./g4-service-calls.md)。本轮移植后的原生专项与本地完整集成
已通过；future abort/panic 尚无独立 fixture 验收。Windows Job 管理不证明
文件、网络或 OS 权限隔离。

没有新增产品 UI、路由、Tauri 权限、安装/激活 IPC、下载、包代码执行或用户
profile 访问；本步没有新的人工 UI 流程。生产 pins、受控 trust recovery/
rollback、健康/LKG、完整文件竞争/ACL/断电验收、tool admission/GC、第三方
平台隔离与 G6 工具执行仍 pending。SEC-001/002/008/014/015 保持 open。
Reviewer：Codex，2026-10-12，工程自查；实际独立安全 reviewer/date/conclusion
pending，工程检查不替代其批准。

## 2026-10-12 远端浏览器监听修正

提交 `b25e0f4` 的 PR Windows job `114622096409` 在浏览器服务监听
`::1:63315` 时返回 EACCES，尚未收集 UI 测试；不能将其记为完整 CI 通过。
日志没有端口保留表或系统策略证据，不进一步推断拒绝原因。

固定 Vitest 4.1.5 会将 api.port=0 还原成默认端口。因此配置先让操作系统
在 `127.0.0.1:0` 选择可监听端口，再关闭探针并用该非零端口启动实际 API。
strictPort=true 保留端口竞争时的直接失败，没有重试、隐藏失败或浏览器替换。
实际加载配置的回归记录真实 Server.listen 分配结果，并核对 IPv4、端口传递
与探针释放；不假定操作系统永远不会选择某个具体端口。

修正后独立验证：全部 CI contracts 14 项/125 条断言、pinned Chromium 27
文件/72 项（15.98s）、相关 type-aware lint 和 UI types 通过；docs gate
再次通过。浏览器测试仍逐文件串行，原测试和断言全部保留。修改后远端完整
CI 以 PR 当前 Checks 为准；历史本地通过及失败记录均保留。

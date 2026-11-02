# G6 P2.8c2：共享工具库存只读管理

日期：2026-11-02。范围：Windows/T1 prototype，聚焦 Core 验证已通过，原生/前端与完整门禁另记。
G5/G6/G7 整体保持未完成；G7 人工项仍为 waived。

## 行为

Runtime 的 `tools.inventory` 返回固定每页 8 个、最多 64 个工具版本的
只读观察。GUI 设置的“受管工具”和 CLI `tools list` 共用 Runtime
client 的生成类型、响应 schema 与一致性校验。查询不创建工具 authority、
安装、materialize、授权、签发 executable/lease、启动工具或清理文件。
普通产品 Host 的 authority/review/pins 仍为空，返回 `not-configured`；
此状态没有 storage/snapshot/条目，不解释为确认磁盘上没有工具。

配置了现有 T0 authority 的 Core 复用原始 ZIP、image、consumer 库存检查，
逐项保留 profile、恢复、pending、已知签名身份、实际文件内容与原生
owner/DACL/label/identity 校验；不重新准入 fixture 工具，也不授予执行权。
旧版本、来源、实际核对过的归档大小、是否有物化文件、已安装依赖插件及
严格历史 lease 引用数可以显示。历史记录不是当前可执行性证明。

共享安装仓库总量来自四个既有物理池：插件、工具原 ZIP、物化 image 与
工具绑定 consumer，包含现有保留/隔离组，沿用 256 MiB / 384 组共享预算。
归档大小与共享仓库总量分别标示；任务私有媒体输入不计入此安装仓库统计。
每项始终显示“尚未提供工具删除功能”，并按实际依赖/保留任务增加阻断原因；
没有“可安全删除”或自动 GC 判断。

## 查询与一致性边界

只接受安全 `u32` offset 与可空的 64 位小写十六进制 snapshot；后续页必须
携带首个 snapshot。offset 限制为 8 的倍数且不越过实际库存。
摘要绑定完整有序投影和实际空间观察。变化返回 `REVISION_CONFLICT`，
GUI 清除旧页并让用户重新查询；不会拼接不同观察时刻的页面。

复用现有有界 metadata stamp，在读取前后比较所有相关安装/信任/恢复
记录，并重新严格读取保留 leases。损坏、未知 SQL、待恢复或 pending
拒绝，不降级为空列表，不清空历史或采用孤立文件。所有观察均可能随后
过时，不能作为后续执行、卸载或恢复操作的凭据。

分页最多 8 × 128 个依赖摘要，仍在原 1 MiB wire 边界内；实际编码后
再次检查并预留 envelope 空间。响应不含用户输入/结果、run ID、私有路径、
image seal、artifact UUID、token、argv 或 executable。显示的 registry/artifact
是原签名 descriptor 的有限身份，不能作为动态下载 URL。

Desktop 原有 Debug/main/origin 检查及 webview capabilities 保持。
查询和状态共用 attach-only 路径，拒绝 `initialize=true`；没有 manager
fallback、ensure-runtime 或冷启动。CLI 也只用 `connectHost` 的 attach-only
路径，未设置 cold 参数；参数先验证，不因工具查询初始化离线 profile。
浏览器预览不调用原生入口，挂载不自动查询；键盘按钮、busy、卸载和
错误后的旧记录清除由浏览器测试覆盖。

## 验证记录

当前 Core 聚焦 `cargo test --locked ... tool_inventory -- --test-threads=1`
**9 pass / 0 fail / 0 ignored**（24.63s，475 filtered），包括真实 ZIP/consumer/
image/保留任务、无 authority 的原生产拒绝、坏日志/坏字节、64 个版本全分页与
每版本 128 个最长依赖摘要的 wire 边界。Core 当前总数 484。
Rust 实际生成协议通过（20.69s），Core all-target Clippy 通过（13.54s）。
日志 `execution-validation/g6/tool-inventory-core-fixed-20261102.log`、
`tool-inventory-codegen-core-20261102.log`、`tool-inventory-core-clippy-20261102.log`。

首次聚焦为 8 pass / 1 fail（20.22s），原因是测试将损坏 raw pending 日志的
既有 `DependencyArtifactMismatch` 错期望为 `RecoveryPending`。产品拒绝路径
未修改；新增分别验证原始日志损坏拒绝与 image pending 的 RecoveryPending，
均确认私有原文保留后，原专项全部通过。首次失败日志保留。

主工作区通过正常构建生成 Runtime 与 Desktop Rust 绑定（22.18s/59.64s），
实际 Host 的未初始化/在线/离线只读查询 **1 pass**（2.23s）；真实 MockRuntime
IPC 的状态/库存两种操作、三个不可信 window/origin/initialize 组合 **1 pass**
（0.03s）。Desktop all-target/codegen Clippy 通过（14.59s）。日志
`execution-validation/g6/tool-inventory-native-main-20261102.log` 与
`tool-inventory-desktop-clippy-main-20261102.log`。
隔离工作区初次 Host build 因缺少构建生成的 runner.json 停止，未执行原生
断言；没有使用空 runner、绕过生成或弱化测试。

共享 client 两项与 CLI 参数一项 **3 pass / 0 fail**（26 assertions）。
实际编译版 CLI/Host 的原任务控制 fixture 扩展验证：初始化后离线查询
非零退出、原 bootstrap/SQLite 字节不变；在线查询返回 NotConfigured；
真实任务完成后仍不泄漏输入。独立最终 Node fixture **exit 0**，原任务/
幂等/撤销断言和期限保持。日志
`execution-validation/g6/tool-inventory-compiled-cli-final-20261102.log`。
四个消费 workspace 的类型/lint 与 Runtime JS fixture 严格检查通过；
新增测试使用实际生成 DTO，未将测试全局引入生产配置。

固定 Chromium UI **4 pass / 0 fail**（2.35s）：键盘主动查询、无自动请求、
空间/来源/依赖/保留原因、分页变化清除旧页、坏页拒绝、离线及卸载。
最初 **2 pass / 2 fail** 暴露错误分支把结构化诊断当作 React child；已修复为
有限摘要和稳定 code，同一四项重新通过，原失败日志保留。类型检查也发现
测试 fixture 的字符串 widening；改用 Rust 生成类型后通过。
最终日志 `execution-validation/g6/tool-inventory-browser-final-20261102.log`、
`tool-inventory-frontend-checks-fixed-20261102.log`、
`tool-inventory-final-ts-lint-20261102.log`。

上述为聚焦结果，最终干净提交的完整根门禁与对应远端 CI 仍须单独运行。
不能以源码审查或这些聚焦结果代替整仓通过。

仍需完成真正工具更新/健康切换/GC/LKG、正式工具审查及两个媒体插件共享
执行；G7 的用户文件授权、事务/撤销与输出发布仍未闭环。

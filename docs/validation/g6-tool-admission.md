# G6 P2.8a1 工具 Manifest 惰性准入

日期：2026-10-13。范围：Windows/x64 T0 prototype；P2.8a/P2.8b/P2.8c、
G5/G6 整体、生产构建审核与独立安全批准保持 pending。

## 实现与信任来源

[工具准入](../../packages/runtime-core/src/packages/tools.rs) 只接 T0 提供的
原始 envelope/Manifest/ZIP 和 T0 clock callback，目标平台来自当前编译信息。
没有客户端可选 target、source policy、pins、路径、argv 或 validation flag。
复用现有 `PackageTrustStore` 与 `DataStore` 单写者，不增加数据库或 schema。

顺序为 Manifest byte budget -> 持久 descriptor 验签/观察 -> 文件数/kind
检查 -> 严格 ZIP32 inventory -> 工具 Manifest/审核 profile -> 再采 T0
时间与当前根/撤销/expiry/floors 重查。输出 `InertToolArchive` 为不可公开构造
的原始内存字节与逻辑程序角色，不能创建安装、grant、runner 或 lease。
任何未来安装/使用须重新做当前准入，旧 proof 不代表持续授权。

已验签 descriptor 身份是保守持久观察；后续 Manifest、布局或审核拒绝仍保留
版本/release floors。大小在 trust IO 前拒绝，文件数在验签后/解码前拒绝。
返回前过期或时钟倒退拒绝，不能返回早先 receipt。

## Runtime-owned 工具 Manifest v1

这份严格工具格式独立于 SDK Plugin Manifest v1 和 G4 ToolPin，不复用/改变
插件契约、metadata digest 或签名 archive lock。所有字段必填：

| 字段                                            | 语义                                                        |
| ----------------------------------------------- | ----------------------------------------------------------- |
| `formatVersion` / `kind`                        | 精确为 1 / tool                                             |
| `publisher` / `id` / `version`                  | 与原始 signed descriptor 完全一致                           |
| `target` / `buildFlavor` / `source` / `license` | 与 descriptor 一致，再匹配审核 profile                      |
| `files`                                         | 原始 signed 业务文件清单逐项一致；最多 32 项                |
| `programs`                                      | role/path 对，role 仅 ffmpeg 或 ffprobe；路径须来自审核布局 |
| `resources`                                     | 完整资源路径清单，须与审核布局一致                          |

Manifest 上限 64 KiB；重复 JSON key、未知字段/角色、缺字段、额外脚本/argv/
环境/certification 全部拒绝。严格 ZIP 层继续检查中央/本地 header、CRC/hash、
清单、路径、特殊文件、压缩/展开预算；本步没有降低现有归档约束。

审核 profile 绑定 publisher/ID、source registry/artifact、许可证 token、
build flavor、target 与完整程序/资源布局，所有业务文件必须非空并拥有审核
角色。额外 DLL、辅助程序或安装脚本即使已签名也不能隐藏为未知资源。
合法 DLL/helper 布局的扩展必须另行审核与验证，不接受包自行扩展。

生产审核清单为空，没有真实生产 trust pins。公开 fixture profile 仅在
`cfg(test)` 编译下存在。签名来源声明与 license 文件存在不证明许可证义务
履行、真实编译选项、PE 可加载性、静态链接或供应链审核。实际 FFmpeg 二进制
版本与许可证审核、辅助程序/DLL 分析和恶意 loader fixture 属于后续 gate。

## 回归与验证记录

[16 项回归](../../packages/runtime-core/src/packages/tests/tool_tests.rs) 使用
公开确定性 fixture keys、真实 DSSE 签名 ZIP 与 SQLite，不替换实际准入实现。
程序与许可证文件内容均为 inert canary，不是 FFmpeg 可执行文件或法律批准。

- stored/deflate 原始字节及角色；生产清单拒绝有效签名 fixture。
- Manifest 缺/未知/重复字段、role、source/license/flavor 一致性与独立审核。
- 缺/重复资源、任意路径、额外 DLL/helper/script、空许可证拒绝。
- plugin kind、foreign target、字节替换、错误签名与签名恶意 ZIP 拒绝。
- byte/file 预算、最终 expiry/clock rollback、根轮换撤销、SQL 写失败、
  trust recovery marker、真实磁盘 reopen、同版本不同 ZIP 和 release floor。
- grants/jobs/plugin_data/imports 与 installation metadata 始终无新增记录；
  实际磁盘 fixture 不产生 package-store-v1，没有文件提取或进程执行。

Windows/x64 focused 命令：

```powershell
cargo test --locked -p flowtools-runtime-core packages::tests::tool_tests
cargo test --locked -p flowtools-runtime-core --doc
cargo clippy --locked -p flowtools-runtime-core --all-targets -- -D warnings
```

首批 15 项 focused 回归通过；新增不可变 ZIP/release floor 回归后，core
扩展运行的全部 16 项工具回归通过，严格 all-targets Clippy 通过。
该次受限沙箱 core 运行结果为 161 passed / 24 failed：旧有 native rename、
reparse/junction 和备份/恢复 fixtures 报实际 NTSTATUS 0xc0000022 或 OS error 5
访问拒绝。本次失败保留，不记作完整门禁通过。

随后在干净实现提交 `e31ec3d`、正常获准执行环境运行完整
`pwsh -NoProfile -File scripts/check-ci.ps1`：冻结依赖、workspace/docs/catalog、
固定 Chromium、前置包构建、routes/Rust bindings 与 read-only contracts 通过；
11/11 工作区 lint/types 通过。Core 185 Rust + 2 compile-fail doctests、
SDK 786、CLI 100 与 Chromium 27 文件/72 项通过。Root test 失败，Runtime
原生测试为 18 passed / 6 failed，退出码 101；后续 root build/production/final
drift gate 未执行，Desktop tests 因依赖失败也未执行。不能记为完整 CI 通过。

六项既有 G4 服务排空 fixture 缺少实际 descendant PID marker，继而等待超时。
单项诊断先在原始实现返回 Timeout（预期 ExecutionFailed），带临时时间诊断时
通过；全部服务诊断为 14/15，取消场景仍无法在原预算内观察后代 marker。
诊断运行的 artifact 校验/进程 spawn 约 56–140 ms；不足以确认未产生 marker
的根因。所有临时诊断已移除；未改生产 service runner、断言、超时、重试、
fixture 或缓存策略，后续通过不能覆盖原失败。

独立继续检查：全仓 build 完成 10/11，Desktop 子进程被终止（0x40010004），
该失败保留；单独执行原 Desktop build 后通过，全部工作区产物已完成。
Desktop 原生 rustfmt、locked check 与 all-target codegen Clippy 通过。
生产入口检查单独通过：Web 22 / Desktop 24 个实际产物在 child-only opt-in/
canary 重构建后 byte-identical，已知 unsafe/certification 指纹与 canary 未出现。
这是独立通过，root gate 仍失败；当前提交远端 CI 与独立安全批准仍 pending。
P2.8a1 验收收口需解决既有服务门禁和独立 review，不能据 focused pass 推进父项。
本步没有 UI/路由/native command/Tauri permission/CSP 变更，人工 UI 验收不适用。

## 残余边界与下一步

生产审核清单为空；无下载安装、共享只读 artifact、多版本库存、并发安装锁、
journal、accepted/排队/interrupted lease、用户 pins、LKG、GC 或产品管理
入口。G5 平台隔离/私有 profile/完整文件竞争/断电等门禁仍是实际工具执行的
前置；没有 media.inspect/transcode 的实现或真实媒体播放证据。

SEC-001/008/015 保持 open；ADR-0001/0002 的身份、授权与平台边界不变，
ADR-0003 记录本步 Runtime-owned 工具格式。Reviewer：Codex，2026-10-13，
工程自查；独立实际安全 reviewer/date/conclusion pending。PR 使用 Draft。
签名、清单、Job Object 与 inert proof 不认证第三方 sandbox 或生产可用性。

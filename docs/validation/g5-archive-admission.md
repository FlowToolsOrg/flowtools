# G5 P2.5b 首步：签名归档准入

- 日期：2026-10-11；范围：Rust 内存中验证签名 ZIP，不访问用户数据或执行入口。
- 状态：首步实现为 prototype；P2.5b/G5/G6 整体验收 pending。
- 威胁：SEC-001、SEC-008、SEC-015；ADR-0003 增加当前安装 ZIP profile。
- 独立安全 reviewer/date/conclusion：pending，不声明生产认证。

## 实现与拒绝边界

`packages/runtime-core/src/packages/archive.rs` 的 `validate_archive` 只能使用
Rust verifier 创建的 opaque `VerifiedPackageDescriptor`。先重新核对传入
manifest/ZIP 的大小和 SHA-256，不能拿旧 receipt 替换新字节。返回的
`ValidatedArchive` 仅含内存文件字节与原 descriptor，不是 installed identity。
没有公开安装 IPC、filesystem 写入、加载器、grants 或 runner session。

当前受控安装 profile 支持 ZIP32 stored/deflate。只允许签名业务清单、
`manifest.json` 和必要父目录，central/local headers 必须完全一致且完整
覆盖物理 local 区间。重复、重叠、隐藏或额外条目均拒绝；禁止绝对/drive/UNC/
ADS、反斜线、dot 段、设备名、大小写/尾部 dot/空格，以及文件/父目录冲突。
加密、split volumes、ZIP64、data descriptors、extra fields、comments、
未知压缩算法、特殊文件、symlink 和 Windows reparse 属性均不支持。

解压前检查完整 inventory 和展开量。业务清单最多 1024 文件，含必要目录
最多 2049 条目；业务展开最多 64 MiB，manifest 最多 1 MiB。大于 1 MiB 的
条目压缩比不得超过 1000。实际输出限额为签名大小加 1 字节，另核对 CRC、
逐文件 SHA-256 与原始 manifest。Deflate 必须消耗全部压缩输入，不能在结束
标记后夹带另一段数据。所有 byte proof 都是 inert data，不执行 canary。

## 验证

```powershell
cargo test --locked -p flowtools-runtime-core packages::
cargo clippy --locked -p flowtools-runtime-core --all-targets -- -D warnings
bun run docs:check
```

回归使用真实 Ed25519 签名和 ZIP 字节，覆盖 Node golden、stored/deflate、
必要目录、receipt 字节替换、缺失/额外/篡改文件、local/central 不一致、
CRC 错误、Zip Slip/Windows 路径、symlink/特殊文件、加密/ZIP64/descriptor/
extra fields、split volumes、重复/隐藏/重叠条目、截断/任意字节、声明超量、
真实高压缩比 bomb 和 deflate 末尾隐藏数据。拒绝样本先通过描述验签，
避免把“错误签名拒绝”误当作归档安全验证。

首步专项验收：35 项包协议/归档 Rust 回归通过（新增归档回归 17 项）；
all-targets clippy `-D warnings`、Rust fmt 与 `docs:check` 通过。
本机 Bun 1.3.14 / Rust 1.96.0 与 Windows CI 固定版本一致。

## 后续与残余风险

完整 Manifest/Host/SDK/依赖锁验证、持久 trust/root/release/time floors、
staging/journal、只读 artifact 发布、跨资源恢复和取消仍属于 P2.5b 后续步骤。
本项没有引入磁盘路径，因此没有磁盘 symlink/junction/TOCTOU 或断电恢复证据。
兼容性和健康检查尚未验收；签名与归档清单不能授予执行权限。
P2.1/P2.2 的实际 Windows 文件/网络/进程和 UI 边界、P2.3b/P2.4b/P2.6b
以及 G6 工具安装/执行/更新均继续 pending。第三方入口保持默认拒绝。

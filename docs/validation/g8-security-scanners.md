# G8 P2.7a：安全扫描基础

日期：2026-11-04。基于已合入 G7 的 `7fcd9ee`，范围为 Windows x64 构建门禁。
本子项不关闭 P2.7/G8，不升级 prototype，不批准发行或第三方代码。

## 实现

- `bun run test:security` 运行脚本契约、完整 Git 历史的 Gitleaks、Bun 全严重度
  审计、四份独立 Cargo 锁审计和真实扫描器正/负对照。
- Windows CI 在 frozen install 后、构建前运行，PR 与 main/codex/release 分支
  均保留 fatal failure。checkout 必须 `fetch-depth: 0`；shallow 本地库拒绝。
- Gitleaks 8.30.1 和 cargo-audit 0.22.2 从官方固定版本 ZIP 获取，下载和缓存
  程序分别核对独立 SHA-256；没有 PATH fallback。工具只存系统临时目录。
- 不过滤平台/严重度、不忽略 advisory、不允许 stale/offline fallback。
  正常 Rust 审计保留 upstream informational warnings，不视为独立风险批准。
- 密钥扫描固定上游规则，强制 redaction，忽略源码内 `gitleaks:allow` 指令。
  只有三个既有 commit/path/rule/line fingerprint 豁免：公开 loopback TLS
  测试私钥、两处公开来源摘要。没有整路径/规则/未来提交豁免。

真实对照在隔离临时 Git 库中先接受干净历史，再拒绝同一 fixture 路径的新
synthetic GitHub token，删除 token 后仍拒绝旧历史，且输出不得含 token。
Rust 对照必须实际命中 `RUSTSEC-2020-0071`，另有干净锁及无效 TOML 拒绝；
网络失败或 parser error 不能冒充已检测到已知漏洞。临时库只在范围校验后清理。

## 依赖与兼容

首次真实审计发现 Vitest、Zod、Vite 及传递 JS 依赖，以及通知 XML、Desktop
devtools/传递 Rust 依赖中的公开漏洞。更新锁文件与直接安全版本下限；移除
未使用的 glob/ESLint 依赖和 CrabNebula 调试插件，通知库升到官方 0.8.1。
oxfmt 的 tinypool 精确旧版本由根 override 固定到修复版 2.1.2。
Playwright 保持原 1.59.1 与 Chromium gate，Bun/Rust 工具链固定版本不变。

Zod 4.6 的 primitive type 数组只在构建导出转换为现有 anyOf，包输入仍拒绝
type 数组；显式保留 Manifest/command/scope/operation 文本的 UTF-16 限额。
原 405 项 corpus 的输入、接受结果和归一化字节均不变，原生验证语义不改。

## 验证记录

- Bun audit：没有已知漏洞命中。
- 四份 Cargo 锁：没有 vulnerability 项；informational 警告仍公开可见。
- SDK Manifest 专项 49 项通过；原 405-case corpus 和 Rust 生成内容检查通过。
- 完整本机根门禁、实际扫描器对照、同 SHA 远端 CI：进行中，收口后补充。

本机 Windows 原有 loopback 网络代理只用于当前验证进程联网；不提交代理值，
不改用户 Git/系统代理或 CI。无法获取实时 advisory 时 gate 失败。

## 剩余工作

P2.7 仍需高风险入口到自动化证据的完整矩阵、parser/IPC fuzz/property 门禁、
informational warnings 处置和独立安全审阅。GTK 等上游维护状态及平台健全性
警告不因当前 Windows 原型未使用对应 Unix 路径而被隐藏或关闭。
真实签名市场、Windows/CLI/Runtime 签名分发、updater/回滚、SBOM/provenance、
支持文档和实测 Beta 周期仍属于 G8 后续项。既有人工豁免保持 waived，未算通过。

参考：[Gitleaks 官方用法](https://github.com/gitleaks/gitleaks)、
[cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit)、
[Bun audit](https://bun.sh/docs/pm/cli/audit)。这些工具报告已知问题，不构成安全认证。

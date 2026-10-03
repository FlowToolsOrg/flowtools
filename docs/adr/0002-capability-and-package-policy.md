# ADR-0002：能力授权、包准入与恢复策略

- 日期：2026-10-03
- 状态：accepted-design；实现未完成，不能作为生产安全认证
- 决策责任：Repository Maintainer；Rust / SDK / Data / Release 模块负责人
- 路线图：P0.4a；实现由 P1.1、P1.5、P2.3–P2.7、P3.5 验收

## 背景

当前权限粒度是 `fs/network/storage/db/native` 等字符串，SDK 注入能力来自
manifest；Desktop adapter 提供 raw SQL 和通用 invoke。目录解析与数据库
记录不具备包准入的完整性或授权含义。具体入口见
[威胁模型](../security/threat-model.md)，执行边界见
[ADR-0001](./0001-plugin-trust-boundaries.md)。

## 决策

### 授权与调用

每次原生或敏感操作均由 Rust broker 再授权：

```text
Host 会话身份有效
AND 已验证包的 manifest 声明允许
AND 用户持久 grant 允许
AND operation 参数落在 scope 内
AND grant epoch 与资源预算仍有效
=> 执行；否则返回稳定拒绝码，不产生业务副作用
```

grant 绑定 package identity、明确操作与规范化 scope；升级新增能力、扩大
scope 或变更 publisher/hash 时重新审查与征求授权，不能凭相同 ID 继承。
拒绝/撤销立即影响活跃会话和排队调用；对已执行副作用明确恢复限制。
受控 last-known-good 回滚也重新核对身份与 grant，不恢复已撤销权限。

| 能力                    | 目标窄接口与 scope                                                                  | 禁止提供给 T2/T3/TL                                                        |
| ----------------------- | ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| 文件                    | broker 发放的 file handle 或限定目录的相对操作；规范化、symlink/junction 与竞争检查 | 任意绝对路径、根目录/home scope、调用方指定基础目录                        |
| 网络                    | scheme/host/port/method allowlist、逐跳重定向及解析后地址策略；超时与响应配额       | 无策略 fetch、Host cookie/token 继承、private/link-local/metadata 地址绕过 |
| 数据                    | Host 绑定 namespace 的 KV/受控 query；参数化值与 allowlisted schema                 | raw SQL、跨 namespace 表名、任意连接字符串、Host metadata/grants 表        |
| Native                  | 带版本的 typed operation enum、严格 payload schema 和大小上限                       | 字符串 command + 任意 payload、shell、Rust reflection                      |
| Clipboard/dialog/opener | 每项独立操作、用户交互与 scope；URL/路径/外部应用策略                               | 绕过授权直接调用 opener、任意 scheme/路径、后台无限读取剪贴板              |

文件读取需校验真实路径，写新文件需校验父目录与最终打开目标；简单 substring
`..` 检查或先 canonicalize 后重新按字符串打开不足以解决竞争。Windows drive、
UNC、ADS、reparse point、大小写和符号链接必须进入恶意 fixture。
网络 CSP 只作纵深防御，不能替代 broker 出站策略或用户 grant。

兼容 IPC 使用版本化 schema、来源/目标、会话 nonce、唯一 request ID、大小与
并发上限。来源绑定 actual webview/session，不凭主窗口 URL 或插件自报 ID。
禁止 `postMessage('*')`；不允许远程 fallback 自动获取原生权限。

### 包与更新准入

在执行任何入口代码之前，验证 versioned manifest、文件清单/hash、发布者
签名与受信 provenance、Host/SDK 版本范围和平台/架构。签名、hash 与 TLS
分别解决不同问题，不能相互替代。

- staging 与 installed 分离；归档大小/文件数/压缩展开量有限制；拒绝绝对
  路径、Zip Slip、symlink 越界和特殊文件；所有最终文件重新核对 hash。
- 已发布版本不可变；同 publisher/ID/version 对应不同 hash 时拒绝覆盖。
- 私钥不进源码、前端 bundle、`.env` 示例或日志；trust root、签名格式、
  key rotation/revocation 与离线策略在 P2.5 实施前补充协议 ADR 与 fixtures，
  此处不发明未实现的加密协议。
- 生产包必须签名；未签名开发包只能在未来显式开发模式、醒目风险提示和
  独立测试数据区运行，不能改变 production trust store。
- 更新先验证再原子切换；失败恢复 last-known-good，恶意/撤销版本不得恢复。
  防降级策略与用户确认的受控回滚分开处理。
- 应用更新使用官方 Tauri updater 的签名校验；插件包签名是独立协议，
  不能用应用 updater 的成功推断插件供应链已经安全。
  [Tauri 官方 updater 文档](https://v2.tauri.app/plugin/updater/)

### 持久化、诊断与恢复

Core metadata、grants、history 与 plugin data 分开 ownership。使用版本化、
幂等迁移与升级前备份；N-1 → N、失败重试、downgrade 拒绝/恢复策略有 fixture。
卸载是否删除数据必须显式确认，Debug 启动也不得隐式删库。

日志仅记录 Host 生成的 run/request ID、package identity、operation、拒绝码、
duration 和 scope 摘要。不记录输入/输出正文、剪贴板、文件内容、凭据或带
query/token 的 URL；限制体积、保留期与导出范围。授权/撤销/安装/更新决定
可审计，但普通本机日志不宣称具备防管理员篡改能力。

## 当前实现

- 没有可用于 production 的包签名、Host 范围准入、持久 grant 或 Rust broker。
- Desktop FS adapter 透传路径给官方插件；仍受其 permission/scope 限制，
  但 SDK 未绑定 per-plugin scope，不能宣称已能读任意 OS 文件或已隔离。
- 插件 DB adapter 共享 SQLite 连接；CRUD 参数化和 identifier 校验不能提供
  namespace 授权；`db.query` 接受原始 SQL。
- `native.invoke` 是通用 Tauri 调用；HTML opener 分支直接调用官方 API。
- Tauri `csp: null`，未配置 app command allowlist；官方插件 capability
  集合属于主窗口，不是每插件授权。
- Debug 初始化会删除 `app.sqlite`；CLI key 前缀/临时目录不是文件 scope。
  现有 logger 可写 payload/details，没有通用脱敏审计日志。

## 后续验证

- P1.1/P2.5：签名前改文件、伪发布者、同版本不同 hash、不兼容 Host、未知
  critical 字段、截断包、zip bomb 与解包穿越都在代码加载前失败。
- P2.3/P2.4：直接 IPC、重定向、路径竞争、跨 namespace、payload 注入、
  撤销竞争与升级扩大 scope 全部拒绝；拒绝后无写入/联网等副作用。
- P2.6：空库、N-1、迁移中断/备份恢复、卸载取消与 Debug 重启不丢数据。
- P1.5/P3.5：secret canary 不出现在日志/bundle；错误签名与撤销 key 更新
  不安装；健康检查失败可回滚且不恢复过期授权。

这些属于待实现的验收目标，不由 `docs:check` 或本 ADR 的通过代替。

# ADR-0001：插件信任边界与执行位置

- 日期：2026-10-03
- 状态：accepted-design；实现未完成，不能作为生产安全认证
- 决策责任：Repository Maintainer；Desktop / Runtime / Security 模块负责人
- 路线图：P0.4a；实现由 P1.1、P2.1、P2.2、P2.3、P2.7 验收

## 背景

同一 JavaScript realm 中的依赖注入、React ErrorBoundary 和 key 前缀只能约束
合作代码，不能隔离恶意代码。当前外部模块在校验 manifest 前已执行，Desktop
HTML iframe 也不是经过认证的插件沙箱。证据见
[威胁模型](../security/threat-model.md)。

## 决策

信任等级与 `app/tool` 类型、CLI 可发现性、成熟度、签名状态相互独立。
拥有签名不等于可信；签名仅验证内容和发布者，授权与隔离仍必须执行。

| 等级              | 执行位置与身份                                                                               | 能力边界与禁止事项                                                                                                                  |
| ----------------- | -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| T0 Host Core      | Rust broker 与随 Host 发布的最小 UI shell；身份来自 Host 构建与原生调用上下文                | 可以操作宿主资源；不得信任插件自报 ID/版本；仅 T0 创建窗口、注册身份、安装更新和授予权限                                            |
| T1 Built-in       | 随 Host 编译发布的固定 inventory；可使用主 React 树                                          | 使用标准 SDK 与声明能力；与 Host 共 realm，不是安全沙箱；外部文件、目录条目或签名不能自行升级为 T1                                  |
| T2 Third-party UI | 每插件独立 Tauri webview/window、Host 分配不可复用会话 label、独立 origin 与经验证的存储分区 | 只通过 typed broker；不共享 Host DOM、JS realm、存储和凭据，不直接拥有官方插件权限；禁止任意远程导航与窗口创建                      |
| T3 Headless       | Desktop/CLI 默认要求可强制终止的受限执行子进程；Host 绑定进程/会话到包身份                   | 不继承 Host 环境密钥、句柄、用户目录访问或网络；OS/runtime 资源约束与 broker 必须同时生效；普通 Bun/Node 子进程不是 sandbox         |
| TL Legacy         | 与 T2 同等独立隔离容器，使用独立兼容 API allowlist 和身份会话                                | 只实现逐项认证的旧 API；preload 仍是不可信代码，不能注入 Host；禁用原始 native/SQL/任意路径接口；非认证包仅能在未来显式开发模式评估 |

T2/TL 的文件路径不同不等于 origin 不同，window/webview 不同也不自动证明
存储分区或 renderer crash 隔离。P2.1 必须在每个声明支持的平台验证实际 origin、
数据分区、导航策略、CSP 和 capability 的最终合并结果；无法实现时禁用该平台
第三方执行，不使用共享主窗口作为 fallback。

T3 的 Worker 只可用于 T1 的计算隔离，或经过独立评审、能落实全部资源与网络
约束的平台实现。Worker 默认可联网；线程隔离不能代替 OS 权限隔离。第三方
运行边界未通过 P2.2/P2.7 时，目标 production policy 是拒绝运行，而不是把
`AbortSignal`、Promise timeout 或普通 subprocess 当成强制终止保障。

Tauri capability 约束的是 window/webview，而非 React 组件。多个 capability
的权限会合并；自定义应用 commands 默认可被应用窗口/webview 调用，需要
显式命令 permission 配置及 broker 内的身份与 scope 检查。
[Tauri 官方 capability 文档](https://v2.tauri.app/security/capabilities/)

## 身份与数据流

```text
不可信包/目录/CLI 输入
  -> T0 验证 manifest、文件 hash、发布者、Host 兼容性
  -> T0 安装并绑定 package identity 与隔离会话
  -> T2/T3/TL 请求 typed operation（不提供 caller pluginId）
  -> T0 校验会话、声明、用户 grant、scope、撤销 epoch、资源预算
  -> Rust 服务 / 官方 Tauri 插件 / 每插件数据空间
  -> 受限结果 + 脱敏审计事件
```

`package identity` 至少绑定 publisher、plugin ID、版本、包内容 hash。
运行时身份来自 Host 管理的 webview/进程会话，不能采用 payload 中的 ID。
broker 不接受调用方指定目标插件 namespace。会话销毁、停用、更新、撤销后
使旧请求身份失效，延迟响应不得投递给新会话。

## 当前实现

现状为 prototype，不符合上述目标：

- T1 React panel 在主树运行；早期外部 SDK `PluginFileLoader` 曾在同 realm
  执行文件代码，没有按信任等级分流。P0.3b1 普通 SDK 文件服务现已拒绝所有
  外部执行；危险代码移至显式 DEV + opt-in 的 development 子入口，Web 生产
  不加载且不恢复已保存源码。该停用策略符合本 ADR 的 production denial 方向，
  不代表 T2/T3/TL 隔离或包准入已经实现；Desktop 默认拒绝已由 P0.3b3 收口。
- P0.3b3 Desktop TL 默认禁用，仅显式 DEV + opt-in 动态加载主窗口内 iframe、
  `allow-same-origin + allow-scripts` 与 `srcDoc`。普通 bridge 拒绝，开发也移除
  raw native/SQL/FS/opener；但仍只检查 message source 与宽松 envelope，没有
  完整 origin、session nonce、版本或 grant，不能称作隔离。独立 r3 功能/
  拒绝实窗清单已回报通过，运行路径与身份已核实；不是安全审批。
- P0.3b2 CLI 仅 import Host 构建内嵌清单中的内置 compiled artifact，删除直接
  源码/headless fallback，缺失/破损/路径重定向明确拒绝；仍在自身进程运行。
  这不是签名供应链验证或 TOCTOU 防护，T3 restricted runner 不存在，
  Web Worker 也未实现为第三方 sandbox。
- `meta.permissions` 决定 capability 注入，不是持久用户授权。T0 broker、
  生产签名包安装、隔离 origin 和 release policy 尚未交付。

不得通过在同 realm 添加一个权限判断来宣称第三方安全隔离已经完成。

## 备选方案与非目标

- 拒绝第三方源码注入主 React 树：无法阻止绕过 SDK 访问 Host DOM/API。
- 拒绝主窗口 iframe 作为原生权限边界：不能依靠共享 origin、宽松 sandbox
  或主窗口 URL 校验识别插件调用者。
- 拒绝“签名插件等同 T1”：被攻陷发布者与恶意签名代码仍是不可信的。
- v1 不支持插件携带任意 native binary、shell、Node/Electron 私有 API 或
  后台常驻服务；不承诺兼容目录中的全部 125 个 HTML 插件。
- 不承诺抵抗已控制 OS/管理员/Host Rust 的攻击者或 WebView 零日；但禁止
  把这些残余风险当作扩大 capability 的理由。

## 后续验证

- P2.1：恶意 UI fixture 不能读取 Host DOM、其他插件 localStorage/KV 或
  Host IPC；伪造 origin、label、plugin ID、nonce、旧会话响应全部拒绝。
- P2.2：无限循环、忽略 abort、内存/输出超限与进程崩溃 fixture 被终止，
  Host 可用；终止后无孤儿进程，敏感环境变量及非授权路径不可读。
- P2.3/P2.7：每个声明的平台测试直接绕过 SDK、未授权 command、capability
  合并、导航后会话失效和撤销竞争。无证据的平台不能进入支持矩阵。
- 人工验收：主机仍可停用/恢复失控插件；插件不能伪装宿主授权弹窗。

本 ADR 的验收是设计和源码证据审阅；以上攻击 fixture 尚待对应里程碑实现。

# G8 P2.7c2：Desktop 内容安全策略

日期：2026-11-04。生产配置不再使用空 CSP，基于
[Tauri 官方 CSP 机制](https://v2.tauri.app/security/csp/)，保留其自动添加
本地资源 hash/nonce 的默认行为，没有 dangerousDisableAssetCspModification。

## 策略和兼容

- default-src none；脚本仅 self 与 Tauri 编译时 hash/nonce，不允许
  unsafe-inline、unsafe-eval、blob/data 脚本或远程脚本。
- object、frame、worker、base、form、frame-ancestors 均拒绝。
- 图片允许 self/data/blob，字体仅 self/data，媒体仅 self/blob。
- 样式保留 unsafe-inline，供 HeroUI/Motion 的动态属性和主题使用。
- connect 保留 self/ipc/http/https，以维持 Tauri IPC 与现有 T1 网络工具。
  这不是逐插件出站限制；同 realm 的任意 HTTP(S)、DNS/redirect 及凭据边界
  仍待 network broker 与独立审核，不据此关闭 SEC-005。
- devCsp 单独允许 Vite inline refresh、WebSocket 以及显式开发预览的
  self/blob script/frame；生产策略不继承这些来源，也不开放第三方执行。

Desktop bootstrap 先使用公开 Zod config 开启 jitless，再动态加载路由/SDK。
实际浏览器最初发现：Zod 可捕获的 Function 探测仍会产生 CSP violation。
修复加载顺序后，解释器保留原校验规则且不再触发该探测；未放开 eval。
直接声明 Zod workspace 依赖，Bun 锁只新增该 importer 引用，没有版本变更。
P2.7c1 的 wire validators 继续在构建期生成并检查漂移。

## 验证与边界

固定 Playwright 1.59.1 / Chromium 的两个实际浏览器回归通过：

1. 策略拦截无 nonce 的 inline/data 脚本、eval、嵌套 frame、object、base
   替换、表单提交、data fetch 和 blob worker。检查实际 violation 与未执行
   canary，同时验证正常动态样式仍可显示。攻击 URL 只用保留 .invalid 域。
2. 实际构建的 Runtime client 在相同策略、真实 Desktop Zod 初始化之后
   完成请求和响应正反校验，且没有 script-src violation。

fixture 只给测试引导脚本附加 nonce，模拟 Tauri 自动注入；frame-ancestors
是 header-only，未冒充 meta 标签验证。源策略的浏览器测试不等于正式安装包
WebView 的完整实机验收，后者和 macOS、独立安全批准仍 pending。
Desktop/UI-test lint、Desktop 类型检查和实际 Desktop 构建通过；完整根门禁
及同 SHA 远端检查仍待单独运行，不能继承之前提交的通过结果。

没有新增 IPC、command、Tauri capability、路径/程序 selector 或用户 grant。
CSP 不代替授权、签名、OS 隔离或原生最终效果检查；P2.7/G8 保持 pending。

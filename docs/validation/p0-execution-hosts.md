# P0.2b2 / P0.2c Windows 宿主验收

日期：2026-10-03。验证者：Codex；这是工程验收记录，不是独立安全 Reviewer 批准。

## 实际验证

- Web：Vite production preview，Chromium 1200×900，真实 bootstrap/registry/router。
- Desktop：Tauri 2.11.3 Debug app、实际 Windows WebView2 与 Rust 插件元数据 IPC。
  独立测试 identifier `com.flowtools.execution-validation-20261003`，隐藏窗口，
  单独 WebView profile；没有启动默认用户应用或访问用户数据库。
- 两端实际 Base64 `run({text:'hello'})` 都返回 v0.1.0 / `aGVsbG8=` / mode encode。
  耗时来自各自实际运行，不要求两个宿主的时间戳相同。
- JSON 输入、Tab 到 Run JSON、Enter 运行；schema 错误和损坏 JSON 都拒绝。
- 三次尝试的 metadata-only 历史落库、刷新恢复、清空通过；无原始输入、输出、
  错误 message。旧历史 key 未改动。
- 实际网站延迟插件执行中 Cancel 得到 ABORTED，控件恢复可用。
  请求只使用保留 `.invalid` fixture，并由 Playwright 拦截挂起，没有访问公网。
- 已人工检查以下截图：共享运行区域可见、控件无重叠、结果/取消状态可读；
  app panel 保留，长页面/原生容器通过滚动访问，不要求单屏显示全部输出。
- Chromium consumer 回归另测实际抛出异常、不可序列化输出、取消迟到结果与卸载
  abort；异常 fixture 不是替换内置 run() 的虚假成功。
- P0.2c 在修改后重新构建并复验以上两端流程；网站延迟走 SDK request，仍能
  实际取消。两端 Todo JSON 加入 `p0 shared-store fixture` 后，真实 app panel
  立即显示相同待办与 deadline，证明不是仅在 JSON 输出中声称添加。
- 十二插件 smoke 真实 compiled run/defaults、schema/预取消无副作用、实际 Todo
  损坏数据异常通过。CLI/Web/Desktop 的实际缺失 network 异常均返回稳定
  EXECUTION_FAILED，无请求副作用；未向生产 registry 注入异常插件。

![Web 实际成功](./assets/p0-web-success.png)
![Desktop 实际成功](./assets/p0-desktop-success.png)
![Web 实际取消](./assets/p0-web-cancel.png)
![Desktop 实际取消](./assets/p0-desktop-cancel.png)
![Web Todo 共享状态](./assets/p0c-web-todo.png)
![Desktop Todo 共享状态](./assets/p0c-desktop-todo.png)

## 复现与数据保护

使用 `apps/desktop/tauri.execution-validation.conf.json` 构建专用测试实例：

```powershell
bun run --cwd apps/desktop tauri build --debug --no-bundle `
  --config tauri.execution-validation.conf.json
bun run --cwd apps/web-vite preview --host 127.0.0.1 --port 4173 --strictPort
```

不要运行默认 Debug desktop：SEC-006 的现有启动重置数据库问题仍 open。
首次验证先确认专用 identifier 对应的 app data 不存在。以后重复验证只能重置
自己创建的 fixture 数据；不得复用真实用户身份/目录。独立 identifier 的数据路径
依据 [Tauri path 文档](https://tauri.app/reference/javascript/api/namespacepath/)。

仅对该测试子进程临时设置 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 为
`--remote-debugging-address=127.0.0.1 --remote-debugging-port=9223`，并用
`WEBVIEW2_USER_DATA_FOLDER` 指向专用测试 profile；启动后立刻恢复父进程环境。
使用 `Start-Process -WindowStyle Hidden`，确认端口只监听 loopback，随后运行：

```powershell
$env:FLOWTOOLS_VALIDATION_WEB_URL = 'http://127.0.0.1:4173'
$env:FLOWTOOLS_VALIDATION_DESKTOP_CDP = 'http://127.0.0.1:9223'
node apps/ui-test/scripts/validate-execution-hosts.ts
```

不要使用 Bun 运行该 Playwright harness：本机 Bun 浏览器协议连接曾挂起；Node
24.16.0 正常完成。不要把调试端口写入生产配置、全局环境或注册表。
WebView2 临时调试参数见 [Microsoft 文档](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code)。
结束后关闭自己启动的实例与 preview，检查端口停止监听。生成 receipt 与原始
截图在忽略目录 `execution-validation/`；上述提交截图仅含受控测试数据。

## 未验证与非结论

- 未验证 macOS/Linux、NVDA、完整对比度/缩放/触屏、OS 外壳焦点与签名安装包。
- 隐藏原生 WebView 截图/键盘验证不等于人工操作前台 Windows chrome。
- 不认证第三方插件兼容性、原生权限 broker、强制终止或供应链安全。
- 网站延迟已使用 SDK request，但 adapter 的出站策略和全局 fetch 隔离仍缺失；
  smoke 和路由验证不认证 grant。截图的旧 stable 标签不代表生产成熟度，P0.3
  状态/生产准入仍未完成，整体仍 prototype。

# E02a：主题数据内核验收

- 日期：2026-10-09。
- 状态：done，纯数据内核工程范围完成；完整 E02 和独立安全批准仍 pending。
- 范围：`@flowtools/sdk/extensions` 的纯数据主题契约和解析器，prototype。
- 业务清单：[E 宿主扩展](../host-extensions.md)。
- 独立安全 Reviewer / 日期 / 结论：pending / pending / 未批准。

源码：[主题数据内核](../../packages/sdk/src/extensions/appearance.ts)，
回归：[主题契约](../../packages/sdk/test/appearance.test.ts)，
Host 默认样例：[测试 fixture](../../packages/sdk/test/appearance-fixtures.ts)。

E02a 交付严格 versioned 主题/个人覆盖，Host 完整浅深色默认值校验、逐字段优先级、
模式切换、系统减少动画偏好、缺失/坏主题回退、错误覆盖原子忽略及只读稳定结果。
选择由 Host 明确给出，不按注册次序自动接管。目录身份仍来自 E01 Host owner；
本项不改变 Manifest v1、Runtime wire、原生 API、持久化或外部加载规则。

主题只允许有限语义 tokens：数值 sRGB/OKLCH、圆角/边框/字号/行高、
本地字体族别名与阴影预设。任意 CSS、URL、远程字体和布局属性被拒绝。
解析复用非可执行 JSON 校验和序列化前 64 KiB 字节预算，输出复制冻结。

## 实际执行记录

- `bun test test/appearance.test.ts`：14 tests、0 fail、84 assertions。
  覆盖真实 PluginLoader enable/disable 与 E01 投影组合、撤下回退/重新启用恢复，
  以及模式隔离、逐字段覆盖、坏主题/覆盖、命名空间、减少动画、数据拒绝、
  Host 输入变更隔离、输出冻结与稳定引用。
- 独立干净工作树执行 `pwsh -NoProfile -File scripts/check-ci.ps1`：退出 0。
  全仓 docs、lint、types、test、build、生成内容漂移、生产产物拒绝、Rust
  fmt/locked check/clippy 和最终工作树检查通过；SDK 231 tests / 1104 assertions，
  包含编译后的 Node 消费与 React/转译器依赖拒绝检查；既有 Chromium 26 文件、
  66 tests 通过。Web 22 / Desktop 24 产物在子进程 opt-in 下字节相同。
  日志位于本地忽略路径 `execution-validation/logs/e02-ci.log`。
  完整 gate 后仅更新验收事实并复跑 docs gate，代码与完整 gate 版本一致。
- 远程 CI：随 PR 的实际 head SHA 记录，不能沿用 E01a 成功推断 E02a 通过。

## 范围与安全

本项没有 CSS/DOM adapter 或界面改动，视觉、路由和原生人工验收不适用；
E02b 和 G7 真实宿主接入单独记录。未验证全界面外观、对比度、辅助技术或跨平台。
预览在本项仅表示无副作用地解析另一套请求，没有用户界面、确认/取消或持久保存。

SEC-002/003/007/010 继续 open，ADR-0001/0002 原信任与包/授权边界不变。
无 native command、Tauri permission、CSP、remote URL、file/network/data scope 或 grant 变更。
同 realm T1 可绕过 SDK，纯解析不是沙箱。目录快照必须由可信 Host 提供；
主题 title 等文本须由后续 adapter 作为纯文本呈现。可信恢复入口应在可恢复的
Host 控制范围中，不能因主题声明而改变权限或身份含义。预算不是 OS 资源隔离。

恢复不写磁盘：撤下后重新解析得到默认 tokens，保留请求 key；无持久配置、
迁移或 durable 恢复声明。独立安全批准和完整 E02 完成状态仍 pending。

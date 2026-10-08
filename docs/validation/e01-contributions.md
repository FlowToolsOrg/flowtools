# E01a：宿主数据贡献目录基础验收

- 日期：2026-10-08。
- 状态：done，固定可信 T1 的工程范围；clean Windows CI 全链通过。
- 范围：`@flowtools/sdk/extensions`，固定可信 T1 的内存数据贡献目录。
- 业务范围：[E 宿主扩展清单](../host-extensions.md)。
- 独立安全 Reviewer / 日期 / 结论：pending / pending / 未批准。

源码：[解析与预算](../../packages/sdk/src/extensions/schema.ts)、
[目录](../../packages/sdk/src/extensions/registry.ts)、
[生命周期投影](../../packages/sdk/src/extensions/projection.ts)、
[公开入口](../../packages/sdk/src/extensions/index.ts)。
回归：[文档与目录](../../packages/sdk/test/extensions.test.ts)、
[生命周期投影](../../packages/sdk/test/extension-projection.test.ts)、
[增量预算](../../packages/sdk/test/extension-json-budget.test.ts)、
[编译 Node fixture](../../packages/sdk/test/extension-export-fixture.mjs)、
[公开产物契约](../../packages/sdk/test/package-exports.test.ts)。

## 实现范围

formatVersion 1 文档只包含 `theme`、`locale`、`settings` 的有界 JSON
贡献。解析严格、复制冻结；目录由 Host 创建，插件 ID/版本由 Host 从
PluginRegistry 绑定，`createOwner()` 发放本地 owner epoch，独立于插件的
loaded generation。替换原子化，清理绑定到对应替换，过期 owner 不得修改
新实例；快照引用稳定。生命周期投影仅包含 enabled、依赖满足的当前实例。

本子项不修改 Manifest v1 或 Runtime wire，不执行插件，不应用主题、不翻译
文字、不生成设置界面。无持久化、原生/API/CSP/Tauri permission、网络、文件或
授权 scope 变更。完整 E01 的菜单/视图贡献和 E02–E10 仍 pending。

## 必须验证的回归

| 类别       | 验证内容                                                                                                                                | 当前证据                                 |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| 文档边界   | formatVersion、额外 envelope 字段、未知 kind、非法/重复 ID、非 JSON、深度/大小/数量预算；拒绝数组继承 toJSON、稀疏/非索引属性等有损输入 | SDK 文档/预算与 Node fixture 已通过      |
| 数据隔离   | 接受后复制冻结；输入后续修改不影响目录；返回快照不可修改                                                                                | SDK 文档/目录回归已通过                  |
| 原子替换   | 非法替换不损坏旧内容；同 owner 新内容一次发布                                                                                           | SDK 目录回归已通过                       |
| 代际与清理 | 旧 owner 拒绝；旧清理函数不删除新替换；revoke 只匹配当前 owner；同一目录实例内卸载重装与重建投影不复用旧 epoch                          | SDK 目录/生命周期回归已通过              |
| 响应式快照 | 无变化引用稳定；实际变化通知；订阅释放；单观察者异常不阻断其他观察者                                                                    | SDK 目录回归已通过                       |
| 生命周期   | 只投影 enabled 且依赖满足的实例；停用、卸载和销毁撤下；文档读取/通知重入停用不复活；读取失败释放此前投影                                | SDK 生命周期回归已通过                   |
| 公开产物   | 编译后 `@flowtools/sdk/extensions` 可用且不依赖 React；Node 真实消费编译入口及拒绝 fixture；已有入口兼容                                | SDK build/exports 与 Node fixture 已通过 |

SDK 与完整仓库门禁均已实际执行，详见下方记录。root test/build 按要求顺序
运行；本子项完成不代表 E01 父项或主题/翻译/设置产品功能已经完成。

本子项没有改变用户界面，视觉、路由、主题效果、语言切换和原生启动验收
不适用；它们随 E02/E03 或后续 Host 接入单独记录。

## 安全与恢复

- 威胁 ID：SEC-002/003/007/010；ADR-0001 的 T0/T1/T2/T3/TL 边界不变，
  ADR-0002 的包准入与授权要求不变。
- 身份与 scope：Host 提供 owner，严格文档/单项 envelope 拒绝额外身份字段。
  不透明 `value` 可包含 pluginId/permissions 等普通 JSON 键，Host 不从其
  派生 owner、grant 或操作，不实施递归身份键黑名单，也不执行 value。
  owner epoch 由贡献目录发放，只解决合作 T1 一致性，不认证 publisher、
  包 digest 或 IPC caller，也不等于授权撤销 epoch。
- 拒绝路径：严格文档、预算、过期 owner 与非法替换须有实际自动化证据；
  失败后旧有效贡献保持或由生命周期撤下，不创建 IO 副作用。
- 撤销与恢复：停用、卸载、依赖不满足、投影销毁清理贡献；同 owner 替换失败
  保留旧数据。无持久状态，无磁盘迁移或 durable 恢复声明。
- 残余风险：同 realm T1 可绕过 SDK；深度/数量预算不构成 CPU/内存硬隔离；
  贡献值尚无主题/语言/设置业务 Schema，也无用户选择与可信 UI 呈现。
- 安全 Reviewer：Codex 工程实现/自检记录不替代独立安全审阅；风险继续 open，
  maturity 保持 prototype。

共同 JSON helper 的修正仅加强数组输入校验；既有 Manifest 格式和合法数据不变。

## 实际执行记录

2026-10-08，从干净提交运行
`pwsh -NoProfile -File scripts/check-ci.ps1`，配置本机 `CARGO_TARGET_DIR`，
整条 Windows CI 脚本退出 0，末端 clean-worktree 检查通过。实际范围包括：

- frozen install、workspace 任务契约、文档与 portable catalog 门禁、固定版本
  Chromium 安装，以及 package bootstrap。
- Host route/原生 bindings 生成、Rust-derived Runtime 契约与漂移验证。
- 全仓 lint、类型检查、test 和 build；11 个 workspace 的任务通过，root test
  与 root build 顺序执行，root test 为 11/11 tasks。
- SDK 完整包测试（含 build）：23 files、217 tests、0 fail、1017 assertions。
  增量 UTF-8 JSON 预算的 11 tests/34 assertions 属于该完整套件的子集。
- UI Chromium 自动回归：26 files、66 tests 通过。此轮未新增 UI 流程；这些是
  既有浏览器回归，不是主题/翻译/原生 Host 人工验收。
- 实际 production-entrypoint 门禁：Web 22、Desktop 24 个产物通过普通构建与
  child-only opt-in 重建的 byte-identical 检查；无 canary/危险入口泄漏。
- Rust fmt、check、clippy 通过；最终生成内容与工作树无漂移。

`bun run docs:check` 验证 11 个核心文档和 12 个必需威胁 ID；
`scripts/docs-check.test.ts` 独立回归为 7 tests、0 fail、54 assertions。
SDK 最终 lint 零警告，针对性回归 37 tests/388 assertions 与上述完整套件
重叠，不累加计数。

公开产物测试在实际 Node 24 消费编译后的 extensions 子入口，验证 ESM 与
CommonJS 两条依赖拒绝探针，再测试合法调用和非法数据拒绝。fixture 使用
Node 20.19 基线已有的 `module.register` API；本机尝试取得 Node 20 时被 npm
DNS 失败阻断，因此不宣称实际 Node 20 验收。

早期普通 sandbox 尝试曾在 Bun 启动时返回 `CouldntReadCurrentDirectory`，
未进入脚本；受控执行和迁移后的独立 worktree 已完成上述实际门禁。
本地日志保存于忽略的 `execution-validation/logs/e01-ci.log`，不提交机器路径
和完整构建日志。远程 CI 状态随 PR 单独记录；本地成功不表示远程运行已通过。

E01a 固定 T1 工程范围完成；E01 父项与 E02–E10 继续 pending。没有人工界面
验收或独立安全批准结论；这些测试不证明主题、翻译、原生 Host 或第三方隔离
可用，maturity 保持 prototype。

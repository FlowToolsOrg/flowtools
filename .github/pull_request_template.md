## 范围与里程碑

- Milestone ID / issue：
- 变更与非目标：
- 插件/runtime/平台影响：
- 本提交仅完成一个里程碑：

## 验证与文档

- [ ] `bun run docs:check`
- [ ] `bun run lint` / `bun run check-types` / `bun run test` / `bun run build`
- [ ] 手动验证 changed flows（UI 需截图、路由/渲染/无障碍；不适用时说明）
- [ ] README / AGENTS / architecture / structure / plugin / roadmap 同步
- 验证证据、CI URL、已知未验证项：

## 安全评审

新增或修改 capability、bridge/IPC、manifest/包准入、执行边界、持久化、
授权、网络、文件或更新接口必须填写。仅文档/不影响安全边界的变更也须说明
“不适用”的具体理由，不能以构建成功代替安全审阅。

- 威胁 ID 与 ADR：[风险台账](../docs/security/threat-model.md)；新增入口须更新
  [信任边界](../docs/adr/0001-plugin-trust-boundaries.md) / 包策略 ADR。
- 安全 Reviewer：填写实际审阅人、结论与日期；不能只填角色或自称已批准。
- 身份与 scope：调用方身份来源、声明/grant/scope、版本/hash 绑定、默认拒绝；
  是否可能绕过 SDK 或直接调用原生接口？
- 拒绝路径：伪造身份/origin/source/nonce、未授权/越 scope/跨 namespace、
  注入、超量 payload 与攻击 fixture；列出自动化证据及拒绝后的无副作用证明。
- 撤销与恢复：排队/活跃会话、升级新增权限、失败回滚、迁移/备份与停用行为。
- 残余风险：未覆盖平台、手动验收、异常场景及 owner；例外需审批人/到期日/
  发布范围，默认不能将 open 风险用于 production。
- 原生/API 清单变更：逐项列出新增/删除 command、Tauri permission、CSP、
  remote URL、file/network/data scope；禁止 raw invoke/SQL/unscoped FS。

模板只要求记录审阅证据；文档 gate 不验证 reviewer 身份或批准。强制 reviewer
及 required check 由仓库维护者另行配置和验收，本模板不表示合并保护已启用。

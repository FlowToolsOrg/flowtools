# G5 P2.5a 签名包协议验收

- 日期：2026-10-09；Owner：Codex
- 范围：离线原始字节/签名/trust policy 原型；无安装或执行
- 状态：P2.5a 只读原型实施与本地门禁通过；P2.5/G5 父项 pending
- 安全 Reviewer：Codex 工程自查；独立实际审阅人、日期、批准结论 pending
- 协议：[ADR-0003](../adr/0003-signed-package-protocol.md)

## 实现与证据

SDK [作者 contract](../../packages/sdk/src/manifest/signed-package.ts) 经普通
`@flowtools/sdk/manifest` 纯数据出口提供。Rust verifier 在 Runtime core 中
只接收 bytes 与 T0 policy，不读取文件、导入代码或写数据库。
其 [Rust 拒绝回归](../../packages/runtime-core/src/packages/tests.rs) 与 SDK
golden 共用签名原始字节；packageDigest 绑定 ZIP 内容，descriptorDigest
绑定签名元数据，续签保留版本、内容与 releaseSequence，不能绕过回滚批准。
Node 制作的 [公开 golden](../../packages/sdk/test/fixtures/signed-package-v1.json)
由两种语言验证；测试 key 不进入应用的生产 trust pins。
golden 附带真实小型 ZIP，其中代码包含不可执行 canary；验证仅 hash 字节。

[SDK 回归](../../packages/sdk/test/signed-package.test.ts) 覆盖 golden、原始
payload/PAE、错误 payloadType、字节变化、危险路径、冲突/重复/超量文件、
到期窗口、未知字段及无 React 的实际 Node package import。
Rust 拒绝矩阵覆盖篡改/错签/伪发布者/threshold、根轮换、撤销、过期/离线、
时间回退、原始 manifest/archive 绑定、版本不可变与精确回滚。

## 验证记录

- SDK 新增签名格式测试：20 pass，0 fail，纳入根测试门禁。
- Rust 新增签名/trust/回滚/恶意编码测试：18 pass，0 fail，纳入根测试门禁。
- Bun 1.3.14、Rust 1.96.0；全仓库 docs/lint/types/test/build 通过。
  lint/types/test/build 均为 11/11 workspaces，根测试共 632 项。
- 构建前 prerequisites/hosts 生成与只读 Runtime bindings/catalog 检查通过；
  生产产物 gate 真实重编译后 byte-identical，未引入第三方执行入口。
- Desktop Rust fmt、locked check、all-targets codegen clippy 通过；最后
  回滚 guard 修复后 core 定向 lint 和 18 项测试再次通过。
- 手动 UI：不适用，没有 UI、路由、权限提示或原生 commands 的变更。
- 远程 CI：PR 创建后记录；独立安全批准 pending。
- 原有未跟踪 `eval/` 保留，不纳入本提交。未将完整 clean-checkout 脚本的
  初始 clean gate 声称为通过；各质量门禁已执行，生成 tracked 内容无漂移。

## 保留边界与后续顺序

P2.5a 不解包、不证实归档安全、不安装、不创建 grants 或 runner sessions。
full Manifest/Host/SDK/dependencies 校验、ZIP bomb/路径/特殊文件、竞态与
崩溃安装恢复属于 P2.5b；没有持久 antirollback 或实时在线撤销保证。
SDK FileLoader、Desktop HTML bridge、CLI 固定清单和 Runtime T1 catalog
继续拒绝第三方。没有用户数据库或原生 Desktop 启动。

G4 P1.6a/b 尚未实现。先补齐不可变服务锁、RPC 权限交集/取消/更新排空，再
推进 P2.5b 安装。随后顺序为 P2.1 UI 容器、P2.2 平台隔离可行性与 adapter、
P2.3b 真实会话 broker、P2.4b 第三方授权/撤销、P2.6b 数据迁移/恢复。
Windows AppContainer + 受限 JS engine + Job 是待实测候选，不在本次冻结为
已验收方案；普通受管 Bun runner 不能复用为 T3 sandbox。

威胁 SEC-001/002/008/014/015 保持 open；平台、发布者签名私钥、根更新分发、
崩溃持久化及独立安全批准由 Maintainer / Release / Security 后续验收。
P2.5 与 G5 父项保持 pending，maturity 不提升。

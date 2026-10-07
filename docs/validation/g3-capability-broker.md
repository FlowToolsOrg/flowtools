# G3 P2.3a：T1 授权 broker 基础验收

- 日期：2026-10-05
- 范围：Windows 验证 Runtime、固定 T1 清单和 Host 内存策略。
- 本子项只交付统一策略与拒绝路径；G3 其余子项仍 pending。
- 独立安全 Reviewer：pending，未取得 production 安全批准。

## 实现与边界

`packages/runtime-core/src/broker.rs` 将授权绑定到 Host 认证的 caller、
publisher、plugin ID、版本、完整 manifest/artifact digest 和 command ID。
Runtime 提交先检查命令声明与授权，再创建任务；开始执行时重新检查并绑定
不可序列化的 runner session。受管子进程启动前和运行期间检查该 session。
取消/完成清理会话，前台连接丢失进入取消路径。

文件 read/create/replace/delete、网络 read/send、数据 read/write、剪贴板
read/write 和 tool execute 分别有 typed descriptor；它们描述策略参数，
不是已经实现的文件/网络/工具 IO API。无 raw invoke/SQL/argv/绝对路径或
调用方指定 namespace。数据 adapter 的 namespace 必须取 Host 绑定的身份。
效果、permission operation、manifest scope、user grant scope、epoch、
deadline、有效期和每个 runner 的调用预算都需通过，才进入同步 adapter。
异步 adapter 还必须在实际副作用提交点重新检查，不能缓存一次检查结果。

`approve/revoke` 仅为 T0 Rust 内存接口，普通 wire/插件不能调用。
每次批准或撤销递增 epoch；重新批准不会恢复旧 session。重启默认无授权，
publisher、版本、digest 或命令改变不继承旧授权。
持久 grants 与管理角色等待 P2.6a/P2.4a，不创建第二套权限数据库。

本轮保留 G2 验证模式的纯 T1 例外；普通 Runtime 仍不启动，CLI 现有本地
执行路径未切换。Todo/网站延迟在验证 Runtime 中继续返回 APPROVAL_REQUIRED，
没有授权管理或真实敏感 IO。现有 Desktop/Web T1 adapters 未由本子项替换。
文件 handle、工具 lock 由后续 Host adapter 发放；网络精确 HTTPS origin/method
策略不证明 DNS、private 地址或逐跳 redirect 限制，网络 adapter 仍未开放。
`input:urls` 等动态 manifest scope 仍拒绝，需在实际网络 adapter 实施时解析。
普通 Bun 子进程仍不是 sandbox，T2/T3/TL 执行保持默认拒绝。

## 回归证据

- Rust broker 单元测试使用实际 Todo manifest 验证声明；允许路径使用受控
  adapter 记录 Host namespace，所有拒绝路径断言 adapter 未被调用。
- caller/publisher/ID/version/digest/command 变化、重启、未声明操作、manifest
  与 grant scope、跨 namespace 注入、过期、撤销/重新批准、超预算均覆盖。
- 覆盖/删除/发送不继承读取授权；不同 method、handle、lock/action 独立检查。
- Rust operation/schema 与十一条 golden fixtures 自动生成 TS；Rust 和 AJV
  均拒绝额外身份、grant、路径、namespace、SQL、argv 字段。
- Runtime 拒绝 Todo/网络任务时 jobs、pending payload、幂等索引全部保持为空。
- 实际 Node/named-pipe fixture 验证拒绝后任务数不变，伪造 permissions.grant
  与 capabilities.invoke 被 wire schema 拒绝。纯 Base64 实际编译入口继续执行，
  包不匹配、deadline、取消/断线、输出预算及同 runId 查询保留原断言。

本地验证：docs/catalog、lint、check-types、root test 和 root build 全部通过；
十一 workspace 任务均成功。Runtime core 18、Runtime native 2、Runtime client 7、
Chromium 59 项测试通过；其余工作区保留全量回归。Rust wire/operation goldens
和 generated manifests 的只读漂移检查通过。实际 production entrypoint 门禁
检查生产产物及 child-only opt-in 重建，无危险入口/探针或字节漂移。
本轮无 UI 改动；本地原生服务/客户端 fixture 提供改变流程的自动化集成证据，
不宣称新的 GUI、NVDA、独立用户 profile、50 并发冷启动或持久恢复验收。

## 安全记录

关联 SEC-003/004/005/006/007/009/010 和 ADR-0001/0002。仅新增 T1 内核策略
实现证据，风险继续 open。文件/网络平台约束、持久化、用户批准、第三方隔离、
供应链签名与撤销恢复仍待各自里程碑。实现自检不替代独立安全 Reviewer。

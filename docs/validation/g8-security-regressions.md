# G8 P2.7b：严格帧解析与确定性变异回归

日期：2026-11-04。范围为 Windows/T1 原型的输入解析；不关闭 P2.7/G8。

## 修复与验证边界

原生连接原先直接反序列化 Request，其中不透明 `input` 的重复 JSON 键会被
覆盖。修复前，真实 framed duplex 回归
`duplicate_nested_wire_input_is_refused_before_dispatch` 失败：输入没有在
解析阶段返回 INVALID_REQUEST。修复后，请求与私有服务输出共用 Core
`protocol::decode_frame`，先执行已有严格 JSON 解码，再验证实际 DTO。
重复键（包括 Unicode 转义后同名的键）、尾随字节、非法 UTF-8、过深输入
和超长帧均拒绝。服务输出仍映射 OUTPUT_INVALID，不回显原始内容。

原来的长度前置检查、1 MiB 上限、连接 5/60 秒期限和服务输出限制不变。
解码不授予身份、权限、任务或文件能力；没有协议版本、数据库或 DTO 变化。

新增三个真实解析器的固定种子回归，纳入原来的 Core test inventory：

- 协议：4,096 次 bit flip/截断/插入/尾随字节变异，接受项须保持序列化往返等价。
- ZIP：Stored/Deflate 各 256 次变异；逐次更新包摘要并实际重签，确保进入
  ZIP 解析而非只触发 hash 拒绝。接受项的完整文件库存与原样本相等。
- 媒体：4,096 次 ffprobe JSON 变异，接受项保持流数、大小和时长上限；
  所有分块宽度的 progress 解码与一次性解码完全相等，非法 UTF-8 后持续拒绝。

这些是确定性 mutation/property 回归，不是 coverage-guided fuzz campaign、
独立安全审核或沙箱认证。签名 fixture 密钥仍仅在测试中使用。

## 风险与已有回归的对应关系

以下为可复核的测试源索引，完整执行结果另行记录；存在测试不代表风险关闭。
所有 SEC 风险仍 open，范围外场景仍需要实现和独立审核。

| 风险                   | 实际测试入口                                                                                                                                                  | 当前残余                                        |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| SEC-001 包伪造         | [archive](../../packages/runtime-core/src/packages/tests/archive_tests.rs)、[重签变异](../../packages/runtime-core/src/packages/tests/security_properties.rs) | 真实市场下载、生产发布者批准未完成              |
| SEC-002 宿主代码执行   | [SDK 拒绝入口](../../packages/sdk/test/external-code-gate.test.ts)                                                                                            | T1 同 realm；第三方仍拒绝                       |
| SEC-003 IPC 冒充       | [连接](../../apps/runtime/src/server.rs)、[broker](../../packages/runtime-core/src/broker.rs)                                                                 | 新增重复键拒绝不代替 native command 全面审核    |
| SEC-004 文件越界       | [实际文件效果](../../packages/runtime-core/src/file_workflows/store/execution/tests.rs)                                                                       | Windows 固定流程，不批准任意第三方文件 IO       |
| SEC-005 出站泄露       | [scope/方法拒绝](../../packages/runtime-core/src/broker.rs)、[LPAC](../../apps/runtime/tests/windows-isolation.rs)                                            | 同 realm request、DNS/redirect 与其他平台未完成 |
| SEC-006 数据隔离       | [命名空间/CAS/迁移](../../packages/runtime-core/src/data/tests.rs)                                                                                            | 同账户被攻陷及第三方隔离未批准                  |
| SEC-007 grant 冒用     | [撤销/重批/身份](../../packages/runtime-core/src/broker.rs)                                                                                                   | 原生授权与独立安全审阅待完成                    |
| SEC-008 恶意更新       | [完整历史/实时审计](../../scripts/security-contracts.test.ts)、[工具 trust](../../packages/runtime-core/src/packages/tests.rs)                                | 应用 updater/正式签名/受控回滚未交付            |
| SEC-009 诊断泄露       | [canary 拒绝](../../packages/runtime-client/test/diagnostics.test.ts)                                                                                         | 不等同 crash dump 与支持包全面隐私验收          |
| SEC-010 资源耗尽       | [LPAC/Job](../../apps/runtime/tests/windows-isolation.rs)、[解析预算](../../packages/runtime-core/src/packages/tests/security_properties.rs)                  | 长期压力与独立审核待完成                        |
| SEC-011 Legacy/preload | [HTML bridge](../../apps/desktop/test/html-plugin-bridge-gate.test.ts)                                                                                        | CSP/资源策略仍需加固；未开放 Legacy 执行        |
| SEC-012 opener         | [普通 bridge 拒绝](../../apps/desktop/test/html-plugin-bridge-gate.test.ts)                                                                                   | main capability 与完整 URL broker 审核未完成    |
| SEC-013 后台主体       | [持久任务/身份](../../packages/runtime-core/src/runtime.rs)、[调度](../../packages/runtime-core/src/runtime_scheduling.rs)                                    | 生产无人值守/安装身份/其他平台未批准            |
| SEC-014 服务委托       | [祖先预算/epoch](../../packages/runtime-core/src/broker.rs)、[服务恢复](../../packages/runtime-core/src/runtime_service_tests.rs)                             | 严格服务输出只处理语法，不代替委托链校验        |
| SEC-015 工具恢复       | [精确任务/排空](../../packages/runtime-core/src/runtime.rs)、[队列处置](../../packages/runtime-core/src/runtime_tool_queue_discard.rs)                        | 正式审核 profiles 仍空；不开放任意工具路径      |

## 验证记录

修复前连接回归失败；修复后两个新增 framed IO 回归与三个 property 回归通过。
完整原生回归：Core 522 单元、11 scheduling conformance、13 doctests；
Runtime 29 单元、6 实际隔离测试均通过，未过滤、忽略或放宽期限。
根门禁及同 SHA 远端验证仍须独立执行，不能继承 P2.7a 的绿色结果。

维护者确认目前没有发行签名，要求暂缓；也没有独立安全/许可证审核、macOS
签名/实机验收、7 天 dogfood 或 30 天 Beta 记录。这些条件没有被工程测试
替代或改为 waived。市场、updater、插件认证、SBOM/来源证明、灰度/回滚、
支持与真实试用指标继续按 G8 依赖顺序交付。

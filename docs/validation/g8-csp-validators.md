# G8 P2.7c1：无需运行时代码生成的协议校验

日期：2026-11-04。P2.7c 拆为构建期校验 c1 与实际 CSP c2，独立验证提交。
本项只移除 CSP 的客户端阻碍，不提前声称已配置 CSP 或完成 P2.7/G8。

Runtime client 原先在模块初始化调用 AJV.compile，依赖 Function 构造器；
生产 script-src 禁止 unsafe-eval 后客户端无法初始化。现使用固定 AJV 8.20.0
的 standalone generator，从原 Rust wire-schema.json 生成校验函数。
原 strict/allErrors 设置及 double/uint16/uint32/uint64 判定保持。
类型声明仅引用 Rust 生成的 Request/Response/StorageAction/StorageReport，
没有手写 DTO、schema 副本或新的 wire/API。

`generate:hosts` 的 Core generate 在 Rust 产物之后生成两个 validator 文件；
`verify:runtime-contracts` 和 client build 都执行只读精确字节检查。
更新协议时运行正常生成流程并审阅差异；禁止手改或在 CI 自动刷新后放行。
两个新增产物显式 LF，staged formatter 跳过生成文件；只跳过该 AJV 输出的
源码 lint，生成器、消费者及测试仍进行严格类型/lint 检查。生成内容每次
build 校验，不以 lint 替代正确性。

验证：

- 原客户端 25 项加新增 3 项，共 28 项、6,061 次断言通过。
- 与原动态 AJV 参考校验器逐项比较实际协议及数字边界变异，保持接受/拒绝
  决定；含 16/32/64 位数值、非有限数、嵌套 extra field 等。
- 实际构建产物在 Node `--disallow-code-generation-from-strings` 下完成
  request/reply/恢复 schema 的正反校验，不只检查源码字符串。
- client build/lint/check-types 通过；生成检查只读、内容一致。
- CI 的 Windows CRLF/LF fixture 扩展到两个新产物；原测试范围和期限不变。

实现参考：[AJV standalone](https://ajv.js.org/standalone.html)。静态编译不授予
权限，也不改变 Runtime 的原生最终验证。c2 仍需实际浏览器策略拒绝与
Desktop 集成回归；SEC 风险与独立审阅保持 open/pending。

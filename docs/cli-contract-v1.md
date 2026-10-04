# CLI command contract v1 (P1.1c)

当前 CLI 是 monorepo 的固定 T1 adapter，需先 `bun run build:packages`。
独立安装 CLI、无界面 Host、授权存储和后台任务属于 G2/G3；这里不开放任意包、
源码、执行路径或 shell。命令身份为 publisher/plugin/command。

```powershell
bun run packages/cli/dist/cli.mjs commands --format json
bun run packages/cli/dist/cli.mjs describe plugin-base64-encoder run --format json
bun run packages/cli/dist/cli.mjs run plugin-base64-encoder --text hello --format json
bun run packages/cli/dist/cli.mjs run plugin-base64-encoder --command run --input '{"text":"hello"}'
bun run packages/cli/dist/cli.mjs run plugin-base64-encoder --batch-input '[{"text":"hello"},{"text":"world"}]'
```

`commands` 返回 `{formatVersion:1,commands:[...]}`。每个 description 返回同一
纯数据 command、identity、pluginVersion、maturity、engines、targets、flags 和
schema-valid example；authorization 固定为 declarations-only。Schema 包含完整
输入/输出、runtimeValidation、效果、权限请求、交互、cold-start 和资源预算。
名称为中文。发现、info 和帮助只读取受限 JSON 与固定编译文件存在性，不 import
任何 executor；发现结果不证明文件完整性或授权。执行前额外验证全部实际文件 hash。

`list/info/run` 用途保持；省略 `--command` 等于旧 `run`。十二内置插件目前各有
一个 `run` 命令；不存在其他可执行 operation。未知 operation 返回 NOT_RUNNABLE。
`info --all` 继续逐个输出旧 info 对象；机器批量发现使用新的 commands envelope。
旧 `info.schema` FieldMeta 结构保留；sample 改为同源、确定性的有限 Schema 示例，
不再动态导入 Zod/faker。示例仍须通过声明的 runtime validator 和业务语义。

操作 flags 使用 property 的 kebab-case；scalar 只允许一次。数值解析允许负数、
小数和指数，最终必须符合 Schema。布尔支持 `--active`、`--active false`、
`--active=false` 和 `--no-active`；数组可重复 flag 或传 JSON 数组，object/union
值使用 JSON。缺省/default/required 来自同一序列化 Schema。字符串含开头双短横
时使用 `--text=--literal`。host option 与 Schema 名冲突或生成 flag 重名时拒绝。
JSON input、batch input、操作 flags 三选一；未知参数、额外位置参数、重复 scalar、
缺值、未知 JSON 字段都返回 INPUT_INVALID，不能默默忽略。旧库级 Zod helpers
继续导出供兼容；CLI 本身使用纯 Manifest。此严格拒绝是有意的修正。

单次 `--format json` 仍为 SDK PluginExecutionResult，结果在 data，稳定错误在
error.code；失败 stdout 为 JSON、stderr 为固定诊断、退出 1。`text/stdio` 保留
已有 formatter。JSON input/batch 默认 JSON 输出，flags 默认 text。支持 `--key=value`
和 host short aliases `-i/-f/-t`；未知格式/缺失命令也有稳定 INPUT_INVALID。
非法 timeout 为 TIMEOUT_INVALID；缺构建/损坏契约为 LOAD_FAILED；未知插件
为 PLUGIN_NOT_FOUND；未知命令为 NOT_RUNNABLE。诊断不回显原 JSON 或 parser 异常。

## Batch v1

`--batch-input` 必须是 1–100 个 object 的 JSON 数组，整段最多 4 MiB；各输入仍
受命令预算约束。先验证整批 JSON 与有限 Schema，任何错误都不加载 executor、
不执行前面的项。runtime validation 在每次实际执行时进行；执行按数组顺序，
每项使用同一真实 handler 与 SDK envelope，失败项不会伪造成成功。

输出为 `{formatVersion:1,type:'batch',identity,success,results}`；results 保留顺序。
全部 success 才退出 0；有执行失败则退出 1。执行失败后仍处理后续项，结果可逐项
检查；batch 不承诺事务/回滚或任务持久性。timeout 每项适用并受 Manifest 上限限制，
并非整批的总 deadline。预检失败仍输出单个 SDK failure envelope，不伪造批量结果。
text 按顺序使用原 formatter，不添加业务输出包装。没有新增 grants/持久 payload。

## 同源验证

Web/Desktop 的固定 UI callback 在 import 前调用共享 loadManifestModule，验证
相同 Manifest、Host target 与编译身份；模块加载后再核对 metadata/command。
两端 JSON run 同样使用 executeManifestCommand 约束输入/输出及预算。CLI 验证
磁盘 hash；浏览器 bundle 由构建绑定，不宣称可以验证原 staging 文件字节。
普通外部入口继续在更早阶段 deny-only；unsafe DEV legacy preview 不成为 v1 包。
当前 Desktop target 仅 Windows/x64，其他平台仍需独立验收；Web 为 web/wasm32。

`bun run generate:command-docs` 从实际 compiled manifests 生成
[十二命令参考](./builtin-commands.md)。`verify:command-docs` 只读检查 drift；
Desktop/root tests 覆盖该 gate。文档、help、describe 和 flags 使用同一 operation Schema。
实际 CLI 回归验证只靠 describe 准备 Base64 输入并按其输出 Schema 验证结果，
也覆盖批处理顺序、数值负数、text 兼容和错误退出。布尔/数组的 compiled fixture
额外断言传入值，再调用实际 Base64 run，未替换为 synthetic success。

威胁 SEC-001/002/005/010；ADR-0001/0002 无设计变更。Codex 工程自检与独立
安全 Reviewer 分开记录，Reviewer 仍 pending，风险不因协议门禁关闭。

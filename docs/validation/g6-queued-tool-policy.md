# 已接受工具队列的版本与授权策略

日期：2026-11-03。范围：通过实际签名工具安装和普通 Core 任务路径，验证
已有排队策略。没有新增生产行为或执行入口，G7 仍为 in-progress / prototype。

## 明确的现行策略

accepted 收据固定包摘要、依赖锁、grant epoch、deadline 和实际镜像引用。
派发前继续普通 `ToolAuthority.restore` 及 accepted 授权复查。提高签名 floor
后，旧排队任务不能借用在途调用凭据、绑定新工具或获得新 epoch；拒绝派发，
记录 Interrupted，保留原根和 DPAPI 私有输入供恢复处理。相同任务不会重试
到一个更新版本，后续工作必须通过新任务及当前明确授权进入。

受保留的 Interrupted 任务不继续占用活动 provider root，整批新目录可以在
所有活动使用者排空后提交；历史 tool lease/输入/旧镜像仍保留。重启只校验
历史根的来源和 scope，不恢复它的执行权；现行新目录可以提供新的任务。
现行 floor、签名和普通所有候选检查保持，不重建旧在途 descriptor。

同一包重新授权也不会刷新已有排队任务的 accepted epoch。已确认的撤权/
epoch 不匹配以 GRANT_REVOKED 结束该任务；正常确认终态及 drain 后释放其
持有记录，其他消费方的已接受任务继续普通授权。这里没有原生尝试，不将
逻辑拒绝当作 actual PID/Job 回收证据。新版本健康正向对照仍执行真实 WAV
转换与独立 probe，并在收取前核对两个实际 PID/Job 已排空。

## 验证范围

独立 `managed-media-candidate/native_queue.rs` 精确四项，覆盖：

- 旧队列提高 floor 后拒绝派发；原 pin、epoch、deadline、私有密文原字节
  及解密后已填默认值的输入不变；无进程、native attempt 或自动重试。
- 旧 Interrupted 收据保留跨过整批目录提交和重启；新版本必须重新授权，
  新实际健康调用不改写旧记录，旧 lease 仍被保留。
- 同包重新授权不会刷新旧收据 epoch；新的独立任务可使用新 epoch。
- 撤销一个队列消费方不影响另一个已授权的共享消费方。

原 6/10/8/9/6/5/5/4 分组完整保留，再加队列四项，总数 57。driver 契约
精确四项、禁止 ignore/环境选择与单独 fatal 执行；新计数专项和严格 TS 通过。
四项实际结果、完整 driver、根门禁与远端结果各自记录，未完成不计 passed。

新测试初始收据字段、flatten 的结果 JSON 路径和 schema 默认值假设曾失败；
按实际 Rust 协议和 admitted 输入修正测试，保留产品拒绝码、pin/epoch/
deadline/密文/无执行及跨重启断言。未更改 Core、产品期限、授权或旧测试。

这不实现历史输入/保留根的显式处置、健康激活、LKG、GC 或用户文件发布。
没有生产 review/signing/Host authority，也不提供旧版排队继续执行的 grant。
见 [整批目录更新](./g6-provider-cohort-updates.md) 和
[G7 完成矩阵](./g7-completion-matrix.md)。

最终四项专项 **4 pass / 0 fail / 0 ignored**（19.40s），all-target Clippy
通过（8.92s）。driver 契约 1 pass / 273 assertions、严格 TypeScript 通过。
失败与修正后的实际日志均保留在 `execution-validation/g6/queued-tool-policy-*`。
完整 57 项及当前提交根门禁/远端检查尚未完成，不与专项结果混算。

随后完整原生 driver **exit 0**：6/10/8/9/6/5/5/4/4 分组共 57 项，分别
20.91/59.33/44.73/54.96/34.92/39.29/27.90/50.53/23.54s；全部 0 fail /
0 ignored。精确计数、固定 source/program hashes、实际 probe、格式及
all-target Clippy（8.89s）通过。主工作区完整 CI 契约 20 pass / 648 assertions
（38.95s），文档检查 11 documents / 12 required threat IDs 通过。当前
提交的干净根门禁和远端仍独立记录。日志
`execution-validation/g6/queued-tool-policy-full-driver-20261103.log`。

随后 `27902f3` 完整本机根门禁 exit 0，最终无漂移：11/11 workspace、
Core 511+11+12、Runtime 24+6/Bun 17、Chromium 113/35、Desktop 108 Bun+
Node 2+37+6、Web 23/Desktop 25 一致性及生产拒绝检查、最终 Rust 检查通过。
相同 SHA 的 [PR 41573666581](https://github.com/FlowToolsOrg/flowtools/actions/runs/41573666581)
与 [push 41573659340](https://github.com/FlowToolsOrg/flowtools/actions/runs/41573659340)
均 completed / success。这是队列策略提交的证据，新来源修改的完整验收另记。

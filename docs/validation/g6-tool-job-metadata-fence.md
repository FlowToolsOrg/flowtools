# G6 工具任务的已提交记录比较

日期：2026-11-01。状态：prototype；专项验证中，完整门禁另记。

接收阶段的旧任务写入已有 transaction、单行变更计数与 readback，但仍用
UPSERT 写入内存缓存中的新状态。若任务行先被另一次 T0 SQL 效果改变，
后续语句自身没有 trigger 也能覆盖未知旧值。真实信任时钟观察的 UPDATE
trigger 同样可以在工具复核与派发状态写入之间修改任务，旧实现仍派发。

现在 `DataStore::load_job_records` 在原有严格 JSON、TEXT/32KiB/1024 行
边界内同时返回原字节 SHA-256。Runtime actor 只在内存保存每个任务的
最后成功提交摘要；该值不进入数据库、DTO、客户端或授权协议。工具任务
初次接收只允许 INSERT；后续更新须在原 IMMEDIATE 事务内读取并比较该
摘要，再以完整旧 TEXT 的 BLOB 比较条件 UPDATE。保留一个 affected row、
一个实际变更（含 trigger）、精确 readback 及原 trust/inventory 比较。

只有 commit 成功才推进 actor 的摘要。未知、丢失、非 TEXT、超大或被改动
的记录拒绝后续缓存写入，沿原失败路径停止接收并保留已持有文件和私有
输入；丢失的行保持丢失，不自动重建收据或补回授权。正常终态、释放和
service diagnostics 更新同样受保护。恢复 sanitize 借用原外层事务，绑定
整批预检读到的原记录摘要，不新增 writer 或重置 scope/floors。

这只是活跃 T0 actor 的一致性保护。SHA-256 不是认证签名或用户 grant，
不能证明被完全控制的 OS/数据库未遭回退。新 Host 仍从严格记录、当前
profile/generation、当前签名信任、grant/epoch 与实际 seal 重新验证；不把
历史摘要当作可复用执行许可。旧无工具任务字节/格式和数据库 schema 不变。

## 复现与验证

相同新增回归先在旧实现实际执行：取消、派发、完成三项 **0 pass / 3 fail**
（10.01s）；独立信任时钟 trigger 用例 **0 pass / 1 fail**（2.52s）。修复后
相同四项 **4 pass / 0 fail**（14.09s）。记录分别在
`execution-validation/g6/job-metadata-fence-red-20261101.log`、
`job-metadata-fence-trust-red-20261101.log` 和
`job-metadata-fence-green-20261101.log`。没有放宽行为或时间断言。

扩展回归另核对纯空白字节改动、记录删除、相同原字节的重复创建，以及
service 尚未结束时外部改动终态记录。拒绝时核对原记录、DPAPI 输入和实际
NTFS 写句柄，信任时钟的保守观察仍前进，不随任务拒绝回滚。
当前扩展租约/兼容结果及完整门禁在完成后另记。

扩展组随后连同原十九项租约专项 **27 项全部通过**（129.26s），包括真实
进程退出与原生 Job 排空。另新增有效非规范 JSON 的恢复兼容用例通过
（3.55s），防止把重新编码值的摘要误当作实际原字节。Core 当前 inventory
为 440，新增九项；all-target Clippy `-D warnings` 通过（18.79s）。日志
`job-metadata-fence-lease-compat-20261101.log`、
`job-metadata-fence-byte-restore-20261101.log`、
`job-metadata-fence-clippy-20261101.log`，均 exit 0。完整门禁仍须完成。

Runtime 二十四项（5.22s）及恢复五项（0.76s）兼容检查通过，日志
`job-metadata-fence-runtime-compat-20261101.log` 和
`job-metadata-fence-recovery-compat-20261101.log` / exit 0；Rust fmt、
文档格式及 docs:check 通过。未将这些专项当作当前完整门禁或远端结果。

随后 `30ce0a0` 完整本机根门禁 exit 0、最终无漂移：11/11 workspace
lint/types/test/build；Core 440+11+9（单元 1159.42s）；Runtime 原生 24+6
（47.89s/2.02s）及 Bun 17（含原 50 路冷启动）；Chromium 109/34；Desktop
106 Bun+Node 1+原生 36+6（3.84s/0.12s）；Web 23/Desktop 25 产物一致、
production refusal 与最终 Rust 检查通过。日志
`execution-validation/g6/root-ci-30ce0a0-20261101.log` / exit 0。

远端 ref 已核对为 `30ce0a0f1c3a22a7c2f28ab54d7d0da535e53ad0`；
[PR 41270975564](https://github.com/FlowToolsOrg/flowtools/actions/runs/41270975564) /
[push 41270972033](https://github.com/FlowToolsOrg/flowtools/actions/runs/41270972033)
当前运行中，完成结果另记。

本修复不新增 wire、工具权限、执行身份、原生 adapter、GC 或文件发布。
生产 review profiles/pins 保持空；G5/G6/G7 父项与独立安全批准仍 pending。

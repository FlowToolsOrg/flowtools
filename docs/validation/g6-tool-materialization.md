# G6 P2.8a3 私有工具文件密封落盘

日期：2026-10-29。Windows/x64 T0 prototype；G5/G6/G7 父项与独立安全批准
保持 pending。本记录覆盖显式惰性落盘，不是工具执行、accepted leases 或媒体办公闭环。

## 实际边界

`packages/runtime-core/src/packages/tools/install/images.rs` 只接受不可构造的
InstalledInertTool、同一个 DataStore writer、PackageTrustStore 与 T0 clock。
从当前源库存精确匹配 artifact UUID 和完整记录，持有源目录/manifest/ZIP/seal
句柄，重新验证原始 DSSE、完整归档、审核策略与当前持久 trust。生产审核
profiles 仍为空，正常入口对有效签名 fixture 在创建 image namespace 前拒绝。
没有新增客户端 path、pins、target、clock、capability、argv 或 IPC。

独立固定 `tool-image-store-v1/{artifacts,staging,quarantine}` 不改原工具 ZIP/
插件 namespace、记录格式或 G4 ToolPin。全部内容由重新准入的 opaque archive
产生，含 manifest 和已声明资源；seal.json 为保留名。每个文件 create-only
写入、fsync、设置并回读原生安全描述符、只读属性、完整字节与 volume/file ID/
creation time；目录也固定身份。64 个记录、33 个内容文件、64 个目录和 32KiB
seal 上限不接受调用方调整，64 位身份字段使用固定十六进制字符串。

命名空间父目录只允许当前 T0 用户和 SYSTEM；每个工具组额外只授予由实际
canonical DB profile hash + 原始 signed ZIP hash 派生的 capability read/execute。
protected DACL、owner、完整 ACE、实际 auto-inheritance 位和 Medium no-write-up
label 全部精确回读；不把 readonly 属性当成 ACL 隔离，不抵御同用户 T0/admin。

## 事务、恢复与预算

顺序：重准入/库存比较/全池预算 -> durable writing journal -> create-only
stage 与 seal -> durable prepared identities -> held-parent native publication
-> 新鲜 trust/源字节/时间/完整磁盘验证 -> 同一 SQL 事务写库存并删除 pending。
最终事务前后都比较整个源库存、image 库存/journal 与实际 trust/restore metadata；
trigger 改写、提交失败、过期、时钟回退、撤销均不返回 proof。短期 trust hash
只作事务比较，不缓存根/floors，不代替重新准入。最终返回对象只公开诊断 pin/
materializedAt，内部持有源与完整树句柄，没有可执行路径或启动方法。

发布复用既有单组件 NT held-parent create-only rename。移动时保留强 source
句柄；移动后重新取得读句柄并检查完整身份/字节/ACL，才提交。不声称跨该间隙
持有同一组 source 句柄；父句柄始终持有。现有强 rename 拒绝测试保持不变。
多个读取对象持有同一物理文件，重复事务重查当前信任但保留库存和文件身份。

任何 image pending 都阻止普通插件/工具安装、检查、原始 abort 和 reconciliation。
显式 image abort 在移动前预检整批 stage/artifact，且每个原生移动再检查一次；
只移动本 journal 拥有的组，保留全部字节。隔离后先把所有节点收紧为 T0/SYSTEM
私有 ACL，删除 artifact capability，再清除 pending。若 SQL 删除失败或中断，
pending 保留；重试只在已隔离的本组接受两种精确 active/private ACL，以完成
能力撤销。未知 ACL/损坏状态不修复、不采用 orphan、不 roll forward 或 GC。

插件归档、工具原始 ZIP、materialized trees 三个物理池含所有 quarantine，
合计仍为 256MiB/384 组。预留新组/全部内容/最大 seal 预算，未扩大配额。
复制 DB 因 canonical profile 不同拒绝；受支持 restore 的 trust recovery marker
仍阻止复用与清理。没有清空库存、重置 floors 或恢复 grants。

## 验证与限制

十八项实际 SQLite/NTFS 测试覆盖：生产拒绝、多个 held reader、清除 readonly
后仍不能写入/替换、跨安装 pending、损坏/重复/旧格式 metadata、内容/seal/
额外文件/hardlink 篡改、SQL 失败与三个状态 trigger 改写、最终时钟/撤销、
三池字节/组预算、copy/restore、五个真实进程无析构退出、原生 ACL/Low label/
junction 拒绝、create-only collision、整批 abort 预检、源 ZIP 损坏、私有半写
stage 的保留与 abort SQL trigger 改写 trust/源/image 库存的拒绝。

真实非发布 isolation-probe 由 Core 标准 test 在 inventory 前单独构建；普通
build/default-members/bundle 不引入它。两个唯一 LPAC AppSID 在精确回读两项
实际 capability 后，读取同一 native file ID 与 canary；不持有 T0 文件锁且清除
readonly 时仍不能写入/降低 label，另一 profile 与没有 artifact capability 的
进程不能读取。显式 abort 的 SQL 删除失败后，原字节仍在而实际 LPAC 读取
被拒绝；再次 abort 可完成。原六项 native 隔离和六项真实媒体候选不增加能力。

最初 native 安全回读观察到 SetSecurityInfo 的 AI 位，现要求与实际原生结果
精确一致，没有忽略 ACL 字段。预算测试最初误把幂等读取当成新组分配，后续
新版本 fixture 又被正确拒绝复用旧 releaseSequence；已改为新版本与新 release
sequence 后再检查原总预算，保留拒绝/无 journal 断言。

当前十八项专项全部通过（78.02s），Core all-target Clippy `-D warnings` 通过。
工作树直接运行 Runtime native test 曾因未执行 prepare 缺少 runner.json 而在
编译前拒绝，尚未执行 native 用例；完整根门禁保留正常 bootstrap 后再验证。
本修改最终整仓与远端结果另行记录，不以已有提交的检查代替。

提交 `554bde6` 的完整本机根门禁 exit 0，最终工作区无漂移。11/11 workspace
lint/types/test/build、Core 306+11+4、Runtime 原生 24+6/Bun 17（含原 50 路
冷启动）、Chromium 109/34 文件、Desktop 106 Bun+Node 1+原生 36+6，
Web 23/Desktop 25 生产产物逐字节对照通过。日志
`execution-validation/g6/root-ci-554bde6-20261029.log`。
当前远端 [PR 40896796753](https://github.com/FlowToolsOrg/flowtools/actions/runs/40896796753)
与 [push 40896785005](https://github.com/FlowToolsOrg/flowtools/actions/runs/40896785005)
PR 已 SUCCESS，push 仍运行中；最终状态单独核对。独立真实媒体完整 driver
复验通过，实际 inventory 6、all-target Clippy 和六项 LPAC 测试通过（3.96s），
日志 `execution-validation/g6/media-native-554bde6-20261029.log` / exit 0。
原生电源断电、完整文件系统竞争、orphan reconciliation、accepted leases、
LKG/更新/GC、真实产品发行签名与许可证/来源批准、产品 native adapter、
用户文件授权发布及独立安全验收仍待完成。G7 人工项 waived 不替代这些条件。

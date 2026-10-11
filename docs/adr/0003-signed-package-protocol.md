# ADR-0003：签名包协议 v1 与离线信任

- 日期：2026-10-09
- 状态：accepted-design；P2.5a 只读原型实现，独立安全批准 pending
- 责任：Repository Maintainer / Plugin Platform / Release / Security
- 范围：G5 P2.5a；事务安装属于 P2.5b，运行边界属于 P2.1/P2.2/P2.3b
- 关联：[包策略](./0002-capability-and-package-policy.md)、
  [信任边界](./0001-plugin-trust-boundaries.md)、
  [实施顺序](../next-milestones.md)、[验证记录](../validation/g5-package-protocol.md)

## 决策

签名使用 Ed25519 与 DSSE 1.0.2。SDK 只提供包描述的作者 schema、一次性
UTF-8 序列化与 PAE helper；Rust 拥有只读验签和信任决策。不在 SDK 接收
“已认证”标志，不由描述、签名或 receipt 创建 grants、安装记录或执行会话。
现有 Manifest v1 的 `signature.status: unsigned` 是历史的内嵌元数据字段；
它不能覆盖外部 envelope 的真实信任判断，固定 T1 清单仍属于 Host 构建。

### 字节与 envelope

独立 envelope 携带 `payloadType`、base64 原始 `payload` 与最多 16 个
`signatures`。签名项的 `keyid` 仅作候选 key hint，不代表发布者身份。
签名按 DSSE PAE 覆盖 payloadType 与原始字节；禁止验签后重新序列化或
再次从 envelope 提取 payload。SHA-256(raw payload) 是 descriptorDigest；
SHA-256(raw ZIP) 是不可变 packageDigest，并由签名 archive 字段绑定。
不对可重排、可补签的 envelope JSON 求 identity digest。

支持两个精确 payloadType：

- `application/vnd.flowtools.package.v1+json`
- `application/vnd.flowtools.trust-root.v1+json`

标准/URL-safe base64 均接受，可带或不带 padding；非法编码拒绝。外层
envelope 遵循 DSSE 忽略扩展字段的规则，但总字节限额仍适用。签名 payload
的已知结构严格拒绝未知、重复字段、错误类型、截断和尾随 JSON。
所有次数、大小、时间与版本序列是 0–2^53−1 范围的整数。
payload 最多 1 MiB，envelope 最多 1.5 MiB；递归 JSON 有解析深度限制。
验签采用 dalek `verify_strict`；信任 key 另检查 canonical encoding 与弱 key。

### Plugin / tool 描述

描述绑定 kind（plugin/tool）、publisher、ID、稳定 `X.Y.Z` 版本、
releaseSequence、一个 platform/arch、issuedAt/expiresAt、原始 manifest
与 ZIP 的 SHA-256/大小、完整业务文件清单、source registry/artifact ID、
license token 和 buildFlavor。tool 表示集中工具包，和 SDK plugin 的
`type: app/tool` 相互独立。工具安装脚本、自选 executable/raw argv 不存在。
v1 不支持预发行或 build metadata；以后增加须升级协议与 fixtures。

包有效期最多 30 天；manifest 最多 1 MiB；ZIP 与业务文件展开总量各最多
64 MiB；业务清单 1–1024 个文件，每文件最多 64 MiB。全部路径使用小写
ASCII，最多 240 字节；禁止绝对/drive/UNC/ADS、反斜线、空段、dot 段、
末尾 dot、Windows device 名、同路径与文件/目录前缀冲突。
`manifest.json` 保留给单独 hash 的 manifest，不列入业务文件清单。

未来归档仅接受普通文件和必要目录；解包前检查 central/local headers、
压缩算法、重复项、声明及实际展开量，拒绝加密、多磁盘、特殊文件、
symlink/junction/reparse、Zip Slip 与 zip bomb。解包后逐文件匹配清单，
再进行完整 Manifest/Host/SDK/依赖锁校验；未实现这些步骤时 receipt 不能加载。
P2.5a 仅核对传入 manifest/archive 的原始字节，并核对 manifest 身份与文件
声明，不解包 ZIP，也不证明其中条目安全或完整 Manifest 语义兼容。

### Trust root 与 publisher provenance

真实初始 trust root 由 T0 发布流程提供已固定公钥与 threshold，禁止包自带
root、目录声明或客户端 payload 添加可信发布者。当前产品没有生产 trust
pins；公开确定性 fixture keys 只在测试中使用。根至少 2 个不同 key、
threshold 至少 2；每 publisher 的 threshold 至少 1。每个 keyid 固定为
SHA-256(raw 32-byte public key)；公钥全局唯一，根与发布者角色分离。
重复签名、key aliases、未知签名不能增加有效票数。

根 payload 含 formatVersion、单调 version、previousRootSha256、
issuedAt/expiresAt、rootKeys/rootThreshold、publisher keys/threshold、
显式 kind/ID/target allowlist 与 provenance authority/subject，以及
revokedKeyIds/revokedPackageDigests。根最多 16 keys，每 publisher 最多
16 keys，最多 128 publishers，每 publisher 最多 128 package scopes。
根有效期最多一年；provenance 是由 T0 审核后写入可信根的主体绑定，
不将作者自报的来源 URL、TLS 或签名解释为已完成审核。

轮换精确为 N+1，previousRootSha256 必须匹配上一根的原始 payload digest；
同一新根同时满足旧根与新根自身 threshold。发布者 key 更新也通过根轮换。
撤销 key/package 列表只增不减，不允许轮换复活已撤销身份。根到期后只能
验证恢复信任所需的根轮换，不能准入包；新根必须当前有效。
根 key 泄漏的应急重建信任须经过未来 T0 受控恢复流程，不能自动降 threshold。

### 离线、版本不可变与受控回滚

离线使用同一当前有效根，不绕过 expiry/revocation。Host 传入并持久保存
已观察时间与根/包版本 floor；时间倒退拒绝。没有可靠时钟或过期 root 时
拒绝新准入，不能用旧缓存自称已知最新撤销。撤销信息最多受根有效期限制；
这不是完整 TUF repository freshness 协议，没有在线 timestamp/snapshot 服务。

同 kind/publisher/ID/target/version 只能对应一个原始 ZIP packageDigest。
ZIP 包含原始 manifest 和业务文件；更换其中任何字节须发布新版本。
同一 artifact 可以由当前受信 publisher 签署新鲜的描述延长有效期，保持
相同 releaseSequence/packageDigest；描述 digest 单独记录，不充当内容版本。
新描述仍逐项核对 manifest/files/当前 scope；撤销包 digest 后续签也拒绝。
releaseSequence 不得低于 Host 保存的高水位。受控回滚仅允许 T0 已批准的
精确历史 digest，仍检查当前根、publisher 范围、expiry、revocation 与字节。
回滚不降低高水位，不继承旧 grant，不覆盖同版本不同 digest 的拒绝。
批准与 floor 必须来自 Host，不能来自包或 run flag。

P2.5a floor 仅在内存 policy fixtures 中验证。P2.5b 第三步新增下述持久
descriptor 观察 adapter；受控回滚审批仍未提供。纯字节模块不是对外的安装
准入接口，调用方不能将自己的 policy 当成持久信任状态。

## 当前实现

2026-10-11 P2.5b 首步新增签名归档内存准入。安装 profile 仅支持 ZIP32 的
stored/deflate；拒绝 encryption、data descriptors、extra fields、comments、
ZIP64、特殊文件、重叠/隐藏条目和压缩流剩余字节。先检查 local/central
一致性、清单和声明展开量，再限额解码、核对 CRC 与逐文件 hash；业务展开
上限 64 MiB，manifest 最多 1 MiB，大于 1 MiB 的条目压缩比不得超过 1000。
这比只读 P2.5a descriptor 格式更严格，是当前受控安装 profile；不影响
P2.5a 验签语义。返回 opaque inert byte inventory，不访问磁盘或产生可执行
身份。完整 Manifest/兼容性在下一独立步骤提供；持久 trust floors 与事务安装仍 pending。
见 [归档证据](../validation/g5-archive-admission.md)。

同日 P2.5b 第二步增加完整 Plugin Manifest v1、Host/SDK/target 和依赖声明
检查：SDK 生成结构契约，Rust 使用版本化显式 semantics，未知 custom export
拒绝；两端共享 90 个样本。opaque byte archive 必须经过本步，tool descriptor
不能充当 plugin。规范化 metadata 不改签名 bytes。实际安装锁绑定仍 pending；
无 installed identity/IPC/grant，见 [Manifest 证据](../validation/g5-manifest-admission.md)。

同日第三步 `packages/trust.rs` 借用 Runtime 的现有 SQLite 单写者，schema v2
不变。私有状态绑定 T0 bootstrap pins，保存原始 DSSE 根链及各根首次接受
时间、最高观察时间、精确 kind/publisher/ID/target/version/ZIP digest 与
releaseSequence。每次重新验证历史双签链与累计撤销，历史根按当时接受时间
验证；当前描述按当前 T0 时间和最新根验证。过期历史只可认证新鲜 successor，
不能准入包。错误/过期候选仍持久观察 T0 时间，错误签名不记录候选身份。
新描述 receipt 只在 FULL SQLite 事务提交观察身份后返回，崩溃不返回未提交
floor。这个保守观察不等于安装成功；后续归档/Manifest 拒绝不能降低 floor。

根链最多 32 项、版本观察最多 1024 项、状态最多 16 MiB；耗尽拒绝，不能
静默裁剪旧根/floor。没有生产 pins 或可由客户端选择的时钟、floor、回滚审批。
受控回滚全部拒绝，未来 T0 审批与可恢复 checkpoint 必须独立验收。

受支持的 managed/low-level 备份恢复均给替换数据库写入不可自动清除的
trust recovery 标记，即使备份早于信任初始化；adapter 同时识别已有
recovery-generation。恢复后的 T1 数据仍可按原策略使用，但 bootstrap、根轮换
和 descriptor 准入均拒绝，直到未来受控信任恢复完成。重启保留正常数据库
floors；完全控制 OS/T0 的对手直接替换/删除数据库不在此控制或威胁模型范围。
原型没有独立防篡改单调硬件锚点，也不声明对任意磁盘回退的保护。见
[持久信任证据](../validation/g5-durable-trust.md)。

第四步 `packages/dependencies.rs` 接受 opaque compatible archives 与 T0
当前兼容环境，先在同一 writer 重放持久根链并观察当前时间，再检查每个
receipt 的当前 root digest、精确持久 ZIP 身份、target、expiry 与 release
floor。根轮换使旧 receipt 失效，须重新验签；无效候选池也不回退可信时间。
原始 Manifest 再按当前 Host/SDK 校验，G4 package/service pins 使用原始 ZIP
digest 与实际准入 target，禁止解析器替换根候选。G4 的内嵌 T1 metadata
规划仍使用其既有 metadata digest，两条身份来源不混用。

此路径没有 ToolCandidate 参数；工具声明不能创建已验证工具库存。最多 512
个 archive，合计原始 Manifest 上限 16 MiB，超限在 trust IO 前拒绝；G4
原有 DAG/边/搜索预算继续执行。所有提供的候选均须满足当前 release floors，
不容忍旧候选，也不是已安装多版本清单。opaque plan 只证明该时间的惰性规划，
不是已安装 lock、执行 grant 或健康检查；未来安装事务须在提交时重检当前
信任、真实 inventory 与原始 artifacts，见
[签名依赖证据](../validation/g5-signed-dependencies.md)。

第五步新增 Windows/x64 T0 惰性安装事务。原始 ZIP 与 Manifest 仅写入固定
blob 名称，不按 Manifest 路径提取业务文件；seal 绑定 create-only artifact
的事务身份和两份原始 digest。同一 DataStore 保存 signed descriptor provenance、
不同版本库存与 G4 dependency locks，读记录须匹配持久 descriptor 观察。
解析只遍历实际库存中的必要 service 闭包，选择已安装最高 release；旧版本
保留但仍须满足当前 admission floors，不能作为降级批准。已安装 provider
自身的传递 lock pins 必须一致，否则先显式重锁，不能静默更换其依赖。

安装按 journal -> synced stage -> prepared journal -> create-only publication
-> 再次当前准入 -> metadata commit 执行。提交时重新采样 T0 时钟，验签当前
根/撤销/expiry/floors、Host/SDK 和实际 provider 字节，拒绝中途状态/锁变化。
inventory 写入与 pending journal 删除同一 SQLite FULL 事务，成功才返回
inert receipt。任何未提交进程中断均显式 abort 到 quarantine，幂等且不滚动
恢复候选、不降低 floors、不清除 backup trust-recovery 标记。不同版本旧记录
和 blob 保留；descriptor 续期/显式重锁可复用同一 ZIP。

路径只来自 T0 已保留/保护的 DataStore profile、固定目录、验证 digest 和
UUID；拒绝 reparse、硬链接、未知目录或字节/seal 不一致。文件 readonly 属性
不等于 ACL。Windows rename 期间释放内部父目录 guards，profile/root 句柄
保持；当前依赖 Native Host 的私有 profile 和单写者，不能声称完整 handle-
relative 竞争防护或同 OS 用户攻陷隔离。sync/FULL/rename 及实际进程退出
证据不认证突然断电的文件系统持久性；完整平台/断电验收仍 pending。
库存最多 128 records / 16 MiB metadata，blob 总量含隔离区最多 256 MiB /
384 groups；规划 aggregate Manifest 16 MiB、展开字节 128 MiB，超限拒绝。
见 [惰性安装证据](../validation/g5-inert-installation.md)。

SDK 作者 helper、Rust 只读 verifier 与 Node/Rust 共用的签名 golden 已实现。
已提供 inert ZIP 解码与完整 Manifest 校验；没有下载、安装 journal、artifact
store、真实 pins、管理 IPC、信任恢复/回滚审批、UI 或第三方执行入口。
G4 服务锁仍是 P2.5b 的前置依赖；
签名不会升级为 T1，也不会为 T2/T3/TL 提供权限。全部 maturity 保持 prototype。
SEC-001/002/008/014/015 保持 open，工程自查不替代独立安全批准。

## 后续验证

P2.5b 验收归档恶意样本、staging/journal 重启恢复、取消、同版本不可变、
last-known-good 健康检查与 metadata 跨资源提交；依赖、数据、grants 分开
ownership。P2.1/P2.2 必须先用实际 Windows 恶意 fixtures 证明运行与 OS
约束，再接正式 adapter；T1 Job 管理不属于第三方 sandbox。
macOS/Linux target 字段只是签名格式，不能推出安装或隔离平台支持。

协议依据：[DSSE protocol](https://github.com/secure-systems-lab/dsse/blob/master/protocol.md)、
[DSSE envelope](https://github.com/secure-systems-lab/dsse/blob/master/envelope.md)、
[RFC8032](https://www.rfc-editor.org/rfc/rfc8032.html)、
[dalek strict verification](https://docs.rs/ed25519-dalek/2.2.0/ed25519_dalek/struct.VerifyingKey.html)。
轮换/expiry/floor 的设计参考 [TUF 规范](https://theupdateframework.github.io/specification/v1.0.36/)；
本协议没有实现完整 TUF，不声明 TUF 兼容或供应链认证。

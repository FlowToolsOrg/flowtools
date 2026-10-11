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

P2.5a floor 仅在内存 policy fixtures 中验证；持久、防崩溃 floor 和审批
在 P2.5b/后续 T0 管理接入前并未提供。此模块不是对外的安装准入接口。

## 当前实现

SDK 作者 helper、Rust 只读 verifier 与 Node/Rust 共用的签名 golden 已实现。
没有下载、解包、安装 journal、artifact store、真实 pins、管理 IPC、
持久 root/floor、UI 或第三方执行入口。G4 服务锁仍是 P2.5b 的前置依赖；
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

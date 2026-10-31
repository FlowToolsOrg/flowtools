# G6 接收任务时的工具 artifact 保留

日期：2026-10-31。P2.8a9 engineering prototype；专项已过，当前完整门禁另记。

当前实现是固定 T1 catalog 任务的接受阶段持有与元数据保留，不是
原生工具派发。普通 Runtime Host 构造保持不带工具 authority；只有明确的
T0 Rust 构造可以借用同一 DataStore 与 PackageTrustStore，生产 review
profiles/pins 保持空，测试 authority 和秒时钟仅在 cfg(test) 中存在。

新任务候选来自实际已安装原始 ZIP 的当前准入；必须已有匹配的密封文件，
不会隐式安装、解包、创建或发布镜像。实际文件/ancestor/source 句柄、seal
和 provenance 绑定到独立 `accepted-t1-tool-artifacts-v1` 记录，与 G4 的
semantic lock 及 signed-plugin-tool-archives-v1 区分。该记录仍不授予 grant
或可执行身份，不暴露路径、argv、namespace、Host 或 review selector。

保留原 jobs.metadata 单行与 writer：接收前将实际 artifact 引用同任务
原子持久化，原输入仍只在 DPAPI；同一镜像的实际 held handles 共享。当前
grant/epoch 与毫秒 deadline 在工具 IO 后再次检查；签名信任另用 UTC 秒。
release 需要成功持久化终态及普通 runner/service 的实际排空，未知写入、
Interrupted 和未排空后代继续保留。旧无工具行不添加字段或改变格式。

恢复、读取与工具 orphan 预检需要严格核对旧任务引用。损坏、过大、重复
JSON key、不同 profile/generation 或丢失 seal 不清除根，不允许自动重锁。
继续保留已有 32KiB 任务行上限，拒绝写入下次启动无法读取的工具任务。

依赖预览和 Provider 变更复用同一实际签名候选解析，不创建 lease 或镜像。
旧无工具默认路径仍使用原语义解析。普通 T1 runner 的终结与 service ack
是现有进程排空边界；Core 保留句柄直至该边界完成，不将一个 Core 方法
返回值冒充原生工具进程的排空证明。原生工具执行器还未接入。

## 验证

本实现新增十九个 Rust inventory 用例（含专用真实退出 child），总 inventory
由 406 变为 425。先前组合十七项通过（84.72s）；补充保护组五项通过
（16.00s），其中两项为新增加的有效大图与真实文件篡改回归。日志：
`execution-validation/g6/accepted-tools-combined-20261031.log` 和
`accepted-tools-native-guards-20261031.log`，均 exit 0。

Core all-target Clippy `-D warnings` 重跑通过，另有原 Runtime 十四项（2.09s）
和 ServiceRegistry 两项（0.09s）兼容专项通过。第一次 Clippy 拒绝 Windows
测试清除 readonly bit 的跨平台 lint；当前只对该 Windows 专用 helper 加具名
注释与局部 allow，以证明 sharing mode 而非 readonly bit 导致写入拒绝。
未更改产品 ACL 或跨平台权限。日志 `accepted-tools-clippy-retry-20261031.log`、
`accepted-tools-runtime-compat-20261031.log`、`accepted-tools-services-compat-20261031.log`。
Rust fmt、文档格式与 docs:check 通过；未将专项记为当前完整门禁或远端成功。

- 两消费方共用一个物理 seal，实际写句柄/rename 在两个任务持有时均拒绝；
  只释放一个任务不能解锁，两个 durable cancel 后可取得写句柄。
- 实际已有 ZIP 重新准入，缺镜像不隐式 materialize；生产空 review 拒绝
  fixture。Submit 不能注入工具记录，保留文件不能调用 ToolExecute adapter。
- 排队任务同 seal/原 key 恢复，管理 Host 不派发；expiry/时钟回退、当前签名
  根撤销、授权 epoch 撤销后 regrant 都不能让旧任务运行。
- 新版本实际安装并密封时旧原生句柄仍持有；两个物理版本并存。新 release
  floor 保持拒绝旧版本派发，旧任务变 Interrupted 并保留根，不冒充更新续接。
- acceptance、terminal、release 的真实 SQL 触发器失败回滚，停止 admission，
  保留实际文件句柄和 DPAPI 输入。六个实际子进程退出 barrier 覆盖提交前、
  accepted 提交后、queued、running、terminal 提交后、release 提交后；父进程
  要求原 20s 上限与 exit 85，核对 SQLite/所有实际源文件及私有字节保留。
- 真实 LPAC spin 子进程 active=1 时根失败但 service 未结束，文件保持锁定；
  终止后要求原 500ms wait/Job empty、exit 71、PID 已退出，普通 service ack
  完成后才持久释放。此用例不执行惰性工具 canary。
- 缺失/错 run/foreign scope/错 seal/重复镜像/queued 却 released、重复 JSON
  key、超大行与 BLOB 行均阻止 raw/image orphan 计划，原库存、输入与字节不动。
- 实际 profile copy 及两条受支持 backup restore 路径保留旧引用：底层 restore
  保留 queued，native management restore 中断并隔离私有字节/撤销授权；两者
  均不续权或清除 trust marker。恢复后真实 seal 字节改变则拒绝派发并保留根。
- 五十个有效消费方的同工具依赖图超过任务行上限时，在 receipt、私有输入
  写入或不可读元数据之前返回 BudgetExceeded，Host 仍可用，临时句柄已释放。

失败记录保留：最早编译发现 epoch u64/u32 与恢复借用错误；测试曾错误使用
不存在的 invoke API、遗漏 service 文件、使用 resolver 原本拒绝的部分工具
选择器，及复用已绑定的 IPC connection。它们均修正在实现/测试中，未放宽
解析器或 session 约束。依赖预览重构第一次误用 lock/plan 返回类型；恢复
用例第一次把底层 restore 当作 native management sanitize，已分别测试原有
行为。失败日志及后续真实成功日志均保留，不将失败记为通过。

## 后续边界

普通包准入的 release floor 仍拒绝旧版本；本步骤不会把它重置或自动填写
rollback approval。新版本出现后旧排队任务的续接，需要独立的 accepted
continuation 规则及当前签名/撤销/期限/精确已接受字节校验。在该路径尚未
实现时，保留旧引用并拒绝派发，不能报告更新连续性完成。

原生媒体执行器、用户文件逐效果授权/事务/撤销、当前构建的真实签名发行、
完整 source offer/许可证及独立安全批准、更新/LKG/GC 和 G7 GUI/CLI 办公
闭环仍待完成。当前记录不使用旧提交门禁作为本实现通过的证据。

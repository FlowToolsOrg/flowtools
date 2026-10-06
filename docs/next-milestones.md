# FlowTools 下一阶段目标与实施设计

- 决策日期：2026-10-04
- 状态：accepted-design；G1、G2 固定 Windows/T1 验证范围已完成；G3 七项已实现为 Windows/T1 prototype，原生确认和恢复修复已实窗复验，独立安全审阅与辅助技术验收仍 pending；G4–G8 尚待实施
- 决策来源：维护者已确认的产品讨论；实施责任由 Repository Maintainer 分配
- 进度来源：[生产路线图](./production-roadmap.md)
- 安全来源：[信任边界](./adr/0001-plugin-trust-boundaries.md)、
  [能力与包策略](./adr/0002-capability-and-package-policy.md)、
  [威胁模型](./security/threat-model.md)

本文确定后续工作顺序、模块职责、协议设计与退出标准。代码路径标为“拟新增”
的内容当前不存在；接口与命令示例是待实现契约，不是当前可调用 API。
各子里程碑完成后独立验证、提交，再开始下一项，不一次实施全部目标。

## 1. 已确定的产品范围

FlowTools 是面向知识工作者与普通办公用户的 AI 时代快捷工具箱。
人通过 GUI、外部 agents 通过 CLI 使用同一组本地能力，数据、授权和结果一致。
首版关注完成办公任务，外部 agent 接入优先；内置 AI 助手、模型接入、创作工具
和低代码编辑器保留为未来插件，当前不实现编辑器、内置 agent 或云同步。

- React + FlowTools SDK/UI 是主要插件开发方式；第三方 React 不因此成为 T1。
- 每个业务操作有命令契约；纯视图贡献可不提供执行入口，需要交互的命令明确声明。
- CLI 可单独安装、在 GUI 不存在或关闭时使用，无界面内核可按策略冷启动。
- 宿主可轻量后台驻留，任务可在 GUI 关闭后继续；插件默认按需启动、空闲释放。
- 插件可依赖版本化服务和集中管理的二进制工具包，兼容依赖共享只读工具文件。
- 默认允许已授权范围内读取、转换与生成新文件；覆盖、删除、发送需要明确授权。
- Windows 是首个实施和验收平台；macOS/Linux 支持按独立证据逐项开放。
  本地 CLI 独立使用不自动承诺远程服务、服务器/CI 或跨用户部署。
- HTML/Legacy 是迁移兼容层，优先将有价值的旧插件迁移至 React；未认证执行继续拒绝。

## 2. 执行顺序

| 顺序 | 目标                      | 对应路线图                                     | 完成后用户得到什么                    |
| ---- | ------------------------- | ---------------------------------------------- | ------------------------------------- |
| G0   | 关闭 Phase 0 尾项         | P0.3b4、P0.3c                                  | 状态与实际能力一致的可信基线          |
| G1   | 命令中心的 Manifest v1    | P1.1a–P1.1c                                    | 可发现、可验证、可生成 CLI 的业务操作 |
| G2   | 生命周期与无界面内核      | P1.2a–P1.2b、P1.3a、P1.5a                      | GUI/CLI 共用的任务与插件事实来源      |
| G3   | 授权、数据与独立 CLI      | P2.3a、P2.6a、P2.4a、P1.3b、P1.4a–P1.4b、P1.5b | 关闭 GUI 仍能按同一授权执行           |
| G4   | 插件服务依赖              | P1.6a–P1.6b                                    | 可复用的前置能力与确定性依赖版本      |
| G5   | 安全安装与第三方隔离      | P2.5a–P2.5b、P2.1、P2.2、P2.3b、P2.4b、P2.6b   | 可安装、可撤销、可恢复的第三方执行    |
| G6   | 共享二进制工具链          | P2.8a–P2.8c、P2.7                              | 两个插件可靠共享 FFmpeg 等工具        |
| G7   | 办公闭环与统一 React 体验 | P3.1a–P3.1b、P3.2、P3.4a                       | 文件、媒体、日程三条实际任务闭环      |
| G8   | 市场、发布与公开 Beta     | P3.3、P3.4b、P3.5、P3.6、P3.7                  | 签名分发、升级回滚和可支持的产品      |

上表是优先执行顺序。Phase 1/2/3 仍表示能力归属；权限和数据的基础子项前移，
避免先交付无人值守 CLI、再补授权。设计评审与受控 fixture 可并行，开放入口
必须等依赖退出标准通过。现有周数仅是历史估算，不作为这次扩大范围后的承诺。

## 3. G1：命令契约先于界面

### P1.1a 序列化 Manifest 与操作 Schema

在 `packages/sdk/src/manifest/` 定义纯数据 Manifest v1（P1.1a 已实现）。
代码加载前验证 ID、semver、publisher、平台、Host/SDK 范围、入口相对路径、
文件清单及内容 hash；签名格式与 trust root 在 P2.5a 另行确定。

每个命令包含稳定的局部 ID、中文名称与描述、输入/输出 JSON Schema、
headless/interaction 支持、冷启动支持、效果声明和资源预算。
命令唯一键为 publisher + plugin ID + command ID；CLI 接受 Host 解析的已安装身份，
不会把用户参数拼成源码或文件路径。Schema 只允许可序列化的有限子集；
Zod refinement/transform 等不可导出的逻辑必须有显式 runtime 校验，不能伪装成
完整 JSON Schema。普通 `run()` 可通过适配映射为一个命令，但旧函数对象不入包协议。

```json
{
  "formatVersion": 1,
  "id": "media-helper",
  "publisher": "flowtools",
  "version": "0.1.0",
  "maturity": "prototype",
  "entries": {
    "ui": "dist/ui.js",
    "executor": "dist/commands.js"
  },
  "commands": [
    {
      "id": "extract-audio",
      "headless": true,
      "supportsColdStart": true,
      "interaction": "none",
      "effects": ["file-read", "file-create"],
      "inputSchema": { "type": "object" },
      "outputSchema": { "type": "object" }
    }
  ],
  "dependencies": {
    "services": [],
    "tools": [{ "id": "ffmpeg", "publisher": "flowtools", "version": "^7.0.0" }]
  }
}
```

上例仅展示字段关系，缺少完整权限、Schema 与包清单，不能作为有效生产包。
`supportsColdStart` 是声明，不是 user grant；effect 标签也不能替代参数授权。

退出：无效 manifest 在执行前拒绝；输出同样验证；命令重复、未知关键字段、
不可表达 Schema、入口穿越/重定向、版本不兼容均有独立 fixture。

### P1.1b SDK 构建入口与旧插件适配

P1.1b 已将十二内置插件分开构建 `ui` 与 `commands`；`services` 仍为可选未来入口。
执行入口不 import React/React DOM、Host UI 或 Tauri API；当前 UI 显式复用同一
typed command 对象，未来跨进程 SDK client 在 G2/G3 实现。迁移十二插件时保留真实 smoke、默认值、text formatter 与结果
语义，旧 `run/setup` 通过明确适配过渡，不恢复源码扫描或 TSX fallback。

退出：在没有 GUI、React bundle 和源码 checkout 的消费目录中导入执行产物；
每个实际命令正常/错误输入与输出均符合契约；普通 SDK 不导出危险开发入口。

### P1.1c 机器发现与文档

P1.1c 已新增 `flowtools commands` 和 `flowtools describe <plugin> <command> --format json`，
提供 Schema、效果、所需授权、平台与交互要求。保留 `list/info/run` 的现有用途，
新增多命令参数在版本化兼容规范中定义，不默默改变旧命令含义。
帮助、CLI flags 与可执行文档从同一 Manifest 生成；提供批处理参数和 JSON 输入。

退出：外部 agent 只依据 describe 即可准备正确输入；unknown flag、负数、布尔
false、数组、required/default 和 JSON 错误都有稳定结果及非零退出码。

## 4. G2：生命周期与无界面运行内核

### 模块与目标所有权

四个新模块已交付 G2 验证基础；下表中的 broker、数据、安装服务与 T3 adapter
仍是后续目标，分别等待 G3/G5，不能从目录存在推断已实现。

| 模块                                             | 职责                                         | 禁止承担                           |
| ------------------------------------------------ | -------------------------------------------- | ---------------------------------- |
| `packages/runtime-core`（Rust crate）            | 会话、broker、registry、任务、数据与安装服务 | 依赖 Tauri WebView 或 React        |
| `apps/runtime`（Rust binary：flowtools-runtime） | IPC server、单实例、启动与关停               | 插件自行启动的任意服务             |
| `packages/runtime-client`（TS）                  | typed client、传输、重连、订阅               | 独立 grants/安装/插件数据库        |
| `packages/plugin-runner`（受管执行入口）         | T1 命令执行，后继 T3 runtime adapter         | 把普通 Bun/Node 进程宣称为 sandbox |
| 现有 Desktop Rust/UI、CLI                        | 客户端、UI 展示与平台交互 adapter            | 各自持久化第二套业务事实           |

Runtime core 的 wire DTO 由 Rust 定义并沿用 Specta 生成 TS；TS Manifest/Schema
生成 JSON Schema，Rust 加载同一版本的受限 schema artifact 验证。
为完整包元数据和 IPC 做双语言 golden fixtures，不维护手写 DTO 镜像。
业务逻辑不必全部重写 Rust：插件命令保留 TS/JS，Rust 管理授权、身份和任务。
G2 先用可丢弃 profile 和 T1 固定清单验证启动/传输；用户数据库 ownership 的
迁移必须等待 P2.6a，生产无人值守执行必须等待 G3 授权验收。

### P1.2a / P1.2b 生命周期与命令投影

先收敛 SDK 当前 Registry/Loader/Lifecycle，再抽象为可交给 Runtime 管理的接口。
区分 installed/enabled 与进程 running，禁止“启用一个插件就必须常驻”。
每插件操作串行化，定义 hook 顺序、失败补偿与资源 ownership；命令列表是已启用
且满足依赖的插件状态投影，更新/卸载与调用不会产生悬空 handler。

退出：非法转移全拒绝；100 次并发启用只执行一次 load/activate；reload/uninstall
没有 timer/订阅/子进程泄漏，hook 失败保持可诊断状态。

### P1.3a / P1.5a IPC 与任务基础

Windows 首版使用当前用户 ACL 的 named pipe；Unix 设计为私有用户目录的 socket。
首版不开放网络监听。握手检查 protocol major、runtime instance 与客户端版本；
请求有 version/requestId/method/payload/长度限制。caller/plugin 身份由 Host 会话
绑定，不采用 payload 内的 agentId/pluginId；token 用于会话与防误连，不声称抵御
已控制同一 OS 用户账户的进程。Desktop adapter 绑定真实 webview session。

任务独立于 CLI 连接，包含 runId、parentRunId、rootCaller、包/命令/依赖锁身份、
deadline、grant epoch、资源预算、进度序号和幂等键；敏感 payload 不进诊断事件。
终态使用现有 `PluginExecutionResult` 的版本化兼容扩展，禁止把“已入队”当成功结果。

```text
accepted -> queued -> running -> succeeded / failed
queued / running -> cancelling -> cancelled
running -> interrupted（Host 崩溃后恢复判定）
```

CLI 同步 run 默认等待终态，断开连接默认取消该次临时任务；显式后台 job 则独立
运行。进度使用订阅事件，JSON stdout 只输出最终 envelope；后台提交返回单独的
JobReceipt。consumer 不把 receipt 解析为 `PluginExecutionResult`。

退出：请求超限/错版本/旧会话拒绝；GUI 与 CLI 查询同一 runId；实际异常、取消、
过期与失联都有确定结果；重连不会重复提交不具备幂等性的业务操作。

## 5. G3：授权、共享数据与冷启动 CLI

### P2.3a / P2.4a 默认拒绝与效果授权

P2.3a 已交付 T1 验证 Runtime 的内存策略 broker，见
[基础验收](./validation/g3-capability-broker.md)。当前仍只执行纯 T1，敏感 IO、
持久 grants 与管理入口未开放；G3 整体仍 pending。

2026-10-05 P2.4a 已在同一数据库交付持久 grants、原子导入、管理角色、CLI
初始化与撤销，见 [授权验收](./validation/g3-persistent-grants.md)。上段为
P2.3a 当时的基础范围；实际执行与独立发行继续按 G3 后续顺序推进。
实施先完成 P2.3a broker，再完成 P2.6a 迁移与单写者基础，最后交付 P2.4a
持久 grants；不得为权限临时建立第二套无迁移数据库。

基础 broker 首先覆盖 T1，同步定义以后 T2/T3 使用的检查路径：
有效会话 AND Manifest 请求 AND user grant AND 参数 scope AND epoch/预算。
从粗粒度 permissions 迁移为 file-read/create/replace/delete、network operation、
clipboard、tool operation 等窄接口；不向第三方提供 raw invoke/SQL/spawn。

冷启动要求 runtime 启动策略允许、命令 supportsColdStart/headless 声明正确、
对应命令和资源有持久授权。背景驻留、开机自启、调度触发、覆盖/删除/发送分别
授权。CLI 缺授权返回 `APPROVAL_REQUIRED`，需要交互返回 `INTERACTION_REQUIRED`，
不等待无法回答的 stdin，也不静默打开 GUI。
启动内核的 bootstrap policy 由统一用户设置保存，只允许启动验证后的内核，
不授权任何插件执行；内核启动后再读取 grants。CLI 的运行参数不能改写该策略，
不允许启动时返回 `COLD_START_DENIED`，不寻找替代执行程序。
独立 CLI 必须提供 `flowtools init --interactive`、`permissions grant/revoke`
和用户主动导入预配置策略的管理入口，首次安装不依赖 GUI。初始化只允许验证后的
内核以管理模式启动，不加载插件或执行业务；未配置时普通调用返回 `SETUP_REQUIRED`。
管理角色由 Host 会话绑定，普通 run 参数、agent 自报身份和插件依赖不能升级角色
或授予权限。交互 CLI 不是密码学意义的人类身份认证；已控制同一 OS 用户账户的
进程不在此保证内，无人值守部署必须使用用户预先批准的策略。

退出：前端改 metadata、伪造请求、重启、撤销竞争均不能扩大授权；拒绝无业务副作用。
覆盖/删除/发送在缺少明确授权时拒绝；已撤销权限不随升级或回滚复活。

### P2.6a 单写者数据与恢复

2026-10-05 已完成固定 T1 / Windows 基础与实际双客户端、native WebView
传输验收，见 [P2.6a 证据](./validation/g3-shared-data.md)。普通用户 UI
切换、durable jobs 和持久 grants 分别由后续 G3 子项交付。

Runtime 作为 SQLite 单写者，分开 core metadata、grants、jobs 和 plugin data。
新增异步 SDK data API，支持 Host-bound namespace、revision/CAS 与事务；旧同步
store 通过 hydration + 订阅或明确兼容 adapter 过渡，不假设跨进程仍能同步访问。
删除 Debug reset 前补内存/临时数据库回归，生产与验证 profile 保持分离。

旧 CLI 临时存储、Web/Desktop localStorage 不自动合并。先校验、展示可恢复来源，
用户明确选择导入；保留原数据且写入迁移记录，损坏记录不覆盖。
运行历史继续只存元数据。确需后台恢复的任务输入存入独立私有 job workspace，
优先使用 scoped artifact/file reference；凭据只存 credentialRef，限额/保留期/
加密与用户取消清理按数据分类设计，不把敏感正文写入 history/log。

退出：GUI/CLI 读写同一份数据，revision 冲突可见；迁移中断回滚、损坏库恢复、
N-1 升级和明确旧数据导入通过；拒绝启动不删除用户库。

### P1.3b / P1.4a / P1.4b / P1.5b 独立发行与任务控制

CLI 包安装 runtime 与受管 T1 runner，不依赖 Desktop 安装路径、WebView 或系统
PATH 中随意找到的程序。冷启动采用有限启动锁、握手和 deadline；多个 CLI
同时启动只产生一个 runtime。提交前客户端生成 request/幂等 key，Runtime 先
持久化 accepted 记录再返回 receipt；连接重试不能生成新的业务请求。

拟新增 `runtime status/start/stop`、`jobs submit/status/watch/cancel`。
GUI 关闭不影响明确提交的后台任务；完整退出时先停止接收，取消/等待任务并清理
进程树。GUI 停止 Runtime 需展示正在执行的任务；不把未知中断任务自动报告成功。
Host 重启仅自动重试已证明幂等、授权仍有效的任务；非幂等任务进入 interrupted。
幂等键绑定 caller、已解析包版本/hash、命令、依赖 lock 与规范化 action digest；
相同键不同身份或输入拒绝。提交 ACK 丢失时按同 key 查询或重提，返回同一 receipt，
不以新 key 重试写入；无法查证非幂等任务时返回 acceptance-unknown/needs-review。
对外部发送等副作用不承诺 exactly-once；发送后确认丢失时记录待核实状态。

退出：未安装 GUI 的干净 Windows 用户 profile 可执行 T1 命令；50 个并发冷启动
只启动一个内核；task cancel/revoke 后无子进程；连接中断不重复文件写入；
CLI exit code、receipt、终态 envelope 与 text formatter 有实际编译入口回归。

## 6. G4：插件依赖与前置服务

### P1.6a 依赖声明与锁

G4 只用固定 T1 清单和可丢弃 fixtures 验证；依赖变更为 plan-only，不开放未签名
第三方下载、安装或执行。实际跨包安装事务在 P2.5b 签名准入后实现。

- 代码依赖：普通 package dependency，构建时打包，不成为运行时插件关系。
- 服务依赖：插件提供 versioned service/operation Schema，经 Host broker 调用。
- 工具依赖：独立工具包，包含 publisher/id/version/target/buildFlavor/digest。

`packages/sdk/src/dependencies/` 和 Runtime `dependencies/`（拟新增）分别管理
声明与解析。服务依赖先采用同一 profile 内单 provider 版本，冲突时明确失败；
工具允许多版本并存。同一版本区间解析成 Host 生成的不可变 lock，不能仅凭 ID
或版本号切换 artifact。公开服务依赖 v1 要求 publisher 固定，避免名称抢占。
循环、缺失、版本冲突、重复 provider、平台不符在激活前失败。
依赖解析先生成用户可查看的变更计划；后续安装或授权被拒绝时不得留下半安装状态。

### P1.6b RPC、调用链与卸载

Runtime 为调用绑定 root caller、调用插件与 provider，保留 parentRunId。
有效权限不得超过调用方授权/可委托 scope、服务 operation 上限及 provider 的
适用授权；服务自身更宽的 grant 不能代替调用者授权。
服务接口只传受限数据或可委托 artifact handle，不传 Host 对象、原始路径、
凭据或另一个插件的执行函数。每层调用继承剩余 deadline、取消与输出预算。
providers 空闲退出，后台服务租约由 Host 控制；插件不能自行 daemonize。

默认卸载被依赖插件时拒绝，并展示反向依赖；级联停用必须先展示计划并确认。
服务更新先重算 lock 与兼容性，停止接受新调用，再等待或由用户明确取消旧任务，
完成数据备份/迁移后切换 provider 并恢复接收。单 profile 内不允许新旧 provider
同时写入同一数据空间；保留旧 artifact/lock 用于受控恢复，不意味着并行运行。
工具包可多版本并存，在途任务保留旧 lock 与版本租约。

退出：A -> B -> C 的取消可传递；循环依赖拒绝；A 不能借 B 读取额外文件；
受控并发激活/更新无重复 provider；排空失败不切换版本；卸载不会破坏已接受任务；
错误可定位到实际 provider，真实安装事务在 G5 验收。

## 7. G5：签名安装与真实运行边界

### P2.5a / P2.5b 包协议与事务安装

先确定 plugin/tool 的签名格式、信任根、publisher provenance、撤销/轮换、
离线与受控回滚协议，再实现 staging -> 验证 -> artifact store -> metadata commit。
插件、工具、依赖锁及业务数据分别 ownership。zip bomb、路径穿越、特殊文件、
symlink/junction 和同版本不同 hash 均拒绝。失败保持 last-known-good。

### P2.1 / P2.2 / P2.3b / P2.4b / P2.6b

第三方 React 使用独立 origin/session 的容器，通过 broker 操作；Windows 实测
存储分区、导航、CSP 与 Tauri capability 合并，不能把另一个窗口自动当成隔离。
T3 runner 不暴露任意 Node/Bun OS API，不继承环境密钥或 Host 句柄。
先以恶意 fixture 验证受限 JS runtime + 平台进程约束的可行性；运行引擎和 Windows
访问隔离机制在 P2.2 的平台 ADR 中冻结，再实现正式 adapter。普通受管 T1 runner
不能升级为第三方 sandbox。无法满足 OS/runtime/file/network 约束的平台拒绝 T3。

Rust broker 的运行身份由会话/进程绑定；IPC、文件 handle、namespace、出站网络、
撤销 epoch、CPU/内存/输出/并发都进入平台拒绝测试。Windows Job Object 可用于
进程树与资源回收，但不解决文件/网络权限；macOS/Linux 分别给出证据。

退出：包改字节/错签名不能执行；无限循环可强制终止；插件不能访问 Host/其他
插件数据或绕过 broker；可撤销、可回滚，隔离未验收时生产入口继续拒绝。

## 8. G6：共享 FFmpeg 与工具链

### P2.8a 集中工具仓库

Runtime `tools/registry.rs`、`installer.rs`、`leases.rs`（拟新增）管理已验证工具包。
工具包可随插件侧载或按验证目录下载，都须来源/签名/hash/平台/许可证准入。
允许审核后的新增工具及 typed adapter；首批 FFmpeg/ffprobe，任意 executable
path、系统 PATH 替换或未知工具不产生执行权限。工具包只含验证清单里的资源，
不允许安装脚本、任意 DLL 注入宿主或变更系统配置。
工具所需辅助可执行文件与动态库也必须列入签名清单；loader 搜索路径不得包含
可写 workspace、插件目录或继承 PATH。绝对 executable 路径与干净环境本身
不足以证明安全，须验证当前目录/PATH 内伪造 DLL/辅助程序不能被加载。

内容寻址目录只读、多版本共存；相同版本/target/buildFlavor/digest 共享文件。
安装锁避免重复下载；任务 accepted 并固定 lock 时取得 artifact lease，覆盖排队、
运行和仍可恢复的 interrupted 任务，不等启动进程才取得租约。
plugin lock 和 task lease 防止卸载/更新误删已接受任务需要的版本。
安装事务用 journal 恢复，不假设文件系统操作与 SQLite commit 自动构成一个
原子事务。GC 从已安装 locks、持久任务引用/leases、用户 pins 和 last-known-good 版本
计算根集合，先标记、等待宽限期再回收，不只看简单引用计数。

### P2.8b 受管执行与媒体适配

提供 `media.inspect`、`media.extract-audio`、`media.transcode` 等版本化操作。
插件传 input/output handle 与有限 preset，Rust 构造参数数组并直接创建进程，
不使用 shell，也不接受不受约束 argv。对 FFmpeg 输入/字幕/filter/播放列表间接
文件访问及协议进行限制；网络素材由授权 broker 获取后交给本地工具。
参数白名单、协议限制只是纵深防御，实际 file/network 访问边界需平台验收。

每任务私有 workspace、干净环境、CPU/内存/并发/时长/输出预算；输出先写临时
artifact，完成验证后以 create-only 或明确 overwrite 授权提交。文件被外部修改
时返回冲突，不保证转换可逆，也不自动覆盖。FFmpeg 原始 stderr 不直接进日志。

### P2.8c 更新、共享和恢复

安装新版本 -> 健康检查 -> 新任务切新 lock -> 旧任务完成 -> 无租约后 GC。
发现撤销版本时拒绝新调用并按政策终止活跃任务，不回滚到已撤销版本。
权限可被单插件撤销，其他合法调用不受影响；用户可查看工具来源、版本、占用、
依赖插件和删除阻断原因。Host 崩溃后恢复租约与任务状态，清理可识别临时产物。

退出：两个真实媒体插件共享一次安装；不同版本并存；篡改/路径替换拒绝；取消
回收后代进程；更新/卸载不删除排队、运行或可恢复任务的 artifact；伪造 DLL/辅助
程序不被加载；未授权工具或网络/文件参数无副作用。

Tauri [sidecar](https://v2.tauri.app/develop/sidecar/) 用于构建捆绑程序，不提供上述
动态共享工具仓库。[FFmpeg 协议](https://ffmpeg.org/ffmpeg-protocols.html) 表明
工具具有多种 IO 路径；[Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
提供进程组和资源管理，不能据此声称权限沙箱已经实现。

## 9. G7：统一 UI 与三个办公闭环

P3.1a 做托盘、单实例、全局快捷键、可选自启动与后台状态；内核不依赖托盘存活。
P3.1b 做 Host 调度器与通知 adapter：one-shot/有限周期任务、时区/夏令时、
missed-run 和重复触发策略；调度器只能调用已授权命令，插件不自行启动 timer daemon。
没有 GUI 时也能记录提醒结果；无法发送系统通知的平台返回明确支持状态。

`packages/ui` 提供 Form/List/Detail/Progress/Result 标准组合、tokens 和统一错误/
取消/键盘/无障碍约定；T2 使用同版本 UI 包构建到自己的容器，不共享宿主 realm。
首版不做自定义 React renderer。复杂插件允许受约束自定义区域，不能仿造授权 UI。

| 实际闭环 | 最小范围                                  | 用户与 agent 验收                                                         |
| -------- | ----------------------------------------- | ------------------------------------------------------------------------- |
| 文件整理 | 显式选择目录、预览改名/分类计划、确认提交 | GUI/CLI 同一 plan ID；冲突失败；重试不重复改名；无 overwrite grant 不覆盖 |
| 媒体处理 | 查看媒体、提取音频、一个受控转码 preset   | 两插件共用工具；实际可播放结果；进度、取消、空间不足与更新回归            |
| 日程辅助 | 本地提醒、查看/修改/取消、有限周期任务    | GUI 关闭后执行；时区/休眠恢复策略可见；撤销不触发；不重复通知             |

文件整理“撤销”只覆盖预先记录且目标未被外部修改的动作；不宣称一般文件系统
事务可逆。会议转写、远程日历和 AI 摘要待模型/账号服务作为后续插件接入。

退出：实际实现而非替换 run() 的 E2E；人工键盘/错误/授权/恢复验收；非专业
用户不用终端或手动安装 FFmpeg 就能完成三条任务；未过矩阵版本保持 prototype。

## 10. G8：分发与未来生态

先关闭 P2.7 安全回归，再完成真实市场、Windows 签名、应用 updater、插件/工具
签名分发、SBOM、灰度、失败回滚和支持文档。独立 CLI 与 runtime 同样纳入签名/
兼容/更新验收，不能只签 Desktop 安装包。跨版本断连时提示升级，不悄悄回退到
未授权的直接执行路径。发布者身份与包签名不提供自动权限提升。

建议性能预算须在固定 Windows 设备、数据集和 artifact 上测量：热唤起 p95
不高于 150ms，10k 命令查询 p95 不高于 50ms，内置简单命令冷启动 p95 不高于
1.5s；后台空闲不保持所有插件/WebView 活跃。先记录 CPU/内存/IO 基线再确定
明确内存预算，不把集成测试 timeout 当性能 SLA。

公开 Beta 要求连续四周无未解决 P0/P1、非用户错误执行成功率至少 99.5%、
升级/迁移/回滚演练与诊断隐私验收；插件失败与 Host 崩溃分别统计，未收集数据
不得宣称达到指标。跨平台支持仅开放实际验证的功能。

低代码编辑器以后生成同一 Manifest/commands/React 工程，使用正常安装授权。
模型接入、AI 助手和创作工具通过版本化服务组合；不把编辑器签名、用户生成或
AI 生成当安全豁免。远程调用、开放付费市场和云同步待单独产品与威胁评审。

## 11. 实施与验证记录

新增目录先创建有限测试 fixture 和公开契约，再接真实入口；每项用一个聚焦
Conventional Commit 记录，不提前把父里程碑标为 done。拟新增验证脚本属于
里程碑交付物；当前仍运行已有 docs/生成内容/静态/测试/构建门禁。

设计已接受，G1 实施完成；现有第三方入口拒绝、数据保留策略和成熟度不改变。
G0 已在 2026-10-04 收口；完整根门禁、实际 production artifacts 与前端复核
通过，剩余原生人工复验按维护者要求豁免，见
[验收记录](./validation/p0-market-state.md)。P1.1a/b/c 均已完成，下一项为 G2 生命周期。

2026-10-04，G1 的 a/b/c 已分别完成实施、回归与提交；命令发现/运行规范见
[CLI v1](./cli-contract-v1.md)，同源参考见 [生成文档](./builtin-commands.md)。
三端实际 T1 入口在加载前检查同一 Manifest，外部默认拒绝保持；父项 P1.1 完成。
实际检查范围与本机资源故障记录见 [G1 验收](./validation/g1-command-contract.md)。
G1 完成当时的后续顺序为 G2；当前 Host/client 验证基础已交付，
grant、durable jobs 与独立 CLI 分发仍待 G3，见下方最终范围。

P1.2a 已实施单一生命周期与每插件锁，见 [G2 验收](./validation/g2-runtime.md)。

P1.2b 已实施命令投影、代际租约、在途排空及协作资源 ownership；保持 T1/prototype 范围。

### G2 P1.3a progress (2026-10-04)

P1.2a/P1.2b and P1.3a are implemented and independently validated. The four
headless modules now exist for fixed T1 / Windows validation profiles; Unix,
production cold start, user DB ownership and durable recovery remain future
stages. Native GUI and Node CLI fixture query the same actual Base64 task.
[G2 evidence](./validation/g2-runtime.md) records native identity/profile and
residual boundaries. P1.5a is implemented; G2 Windows/T1 validation scope is complete.

### G2 final scope (2026-10-05)

P1.2a/P1.2b/P1.3a/P1.5a delivered in separate commits. Generated wire/metadata
golden checks, stable errors, redacted diagnostics, actual managed T1 failure/
deadline/output rejection, shared native GUI/Node runId and disconnection policy
are verified. G3 is next; current non-durable tasks have no crash restoration.
Instance change returns INSTANCE_MISMATCH; it never invents an interrupted result
or retries unknown business actions. Prototype labels and SEC open states remain.

2026-10-05: P1.3b implemented and regression-validated in the fixed Windows T1
scope; see [durable jobs](validation/g3-durable-jobs.md). G3 remains pending
P1.4a, P1.4b and P1.5b plus independent security review.

### G3 P1.4a standalone CLI

Windows x64 CLI-only bundle and bounded, authenticated cold-start coordination
are implemented for the fixed T1 inventory. See [standalone CLI evidence](validation/g3-standalone-cli.md).
The package pins relative compiled artifacts and supports explicit GUI-free init,
runtime start/status/stop and granted bundle execution. Source CLI/GUI integration
and operational diagnostics remain P1.4b/P1.5b. Maturity remains prototype;
signed releases, third-party sandbox and independent security review are pending.

### G3 P1.4b shared Host clients

Desktop and source/standalone CLI execute through the same authenticated T1 Host.
`jobs submit/list/status/watch/cancel/lookup` expose durable receipts and bounded
metadata; listings exclude results. Same-user CLI/Desktop sessions can cancel
one another's tasks; validation callers and T0 management roles retain their
separate rejection rules. Lost acknowledgements query the original key, never
create a new effect. The wire client is now exactly 0.2.0; 0.1.0 clients fail the
handshake before business IO. Protocol major and SQLite schema are unchanged.

The source Desktop prototype pins the build-owned Runtime binary and verifies
it before native bootstrap. Its main native window and configured DEV origin
own sessions; credentials/endpoint/executable selectors never enter JavaScript.
Release Desktop distribution remains a later platform gate. Initialization,
grants, cold-start changes and full stop require an actual native confirmation;
revocation is immediate. The GUI shows active task count before full stop.
Closing GUI disconnects foreground work while explicitly granted background
work belongs to Runtime. Native Todo uses async revisions/CAS and no client
persistent store; original local prototype data remains for deliberate import.

See [shared client evidence](validation/g3-shared-clients.md). All scopes stay
prototype. Windows native consent and recovery UI fixes have actual
[acceptance evidence](validation/g3-desktop-acceptance-fixes.md);
independent security review remains pending.

### G3 P1.5b diagnostics and offline recovery

Fixed Windows T1 runs now expose `jobs diagnose <runId>` and a native Desktop
metadata summary/export. Identity, package/dependency lock, grant epoch, state,
sequence, time and stable failure code are allowlisted; inputs, outputs, paths,
credentials and arbitrary exception messages are excluded. Wire client is exactly
0.3.0; older clients fail before business IO. Protocol major and DB schema stay 1/2.

Offline `runtime storage list/create/restore <backup-id>/retry` reserves the same
current-user profile and first native pipe before SQLite IO. Restore/retry require
CLI `--confirm` or actual native confirmation. Logical UUIDs select same-profile
backups; clients cannot choose backup paths. Pending recovery blocks Host startup.
Restoration stages SQLite data, revokes grants, disables cold start, interrupts
unfinished jobs and quarantines original DB/private payloads. Retry continues the
same journal; it never replays business work. Old result metadata remains queryable
through diagnosis, while quarantined outputs expire; new runs retain normal results.
Missing post-backup keys return ACCEPTANCE_UNKNOWN and require explicit review.

See [diagnostic/recovery evidence](validation/g3-diagnostics-recovery.md). Scope remains prototype. Independent
security review, assistive technology and other platform acceptance are pending;
these checks do not authorize production.

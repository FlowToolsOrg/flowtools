# G7 Windows/T1 功能交付

日期：2026-11-04。维护者选择固定 Windows/T1 功能原型，人工验收 waived。
G7 已完成上述范围，产品成熟度保持 prototype。文件、媒体、日程/提醒、
桌面及搜索的完成边界见 [矩阵](./g7-completion-matrix.md)。源代码 `02d416a`
已通过干净完整本机根门禁；最终文档提交的同 SHA 远端结果记录在
[Draft PR #15](https://github.com/FlowToolsOrg/flowtools/pull/15)。

## 用户流程

桌面设置中的“文件整理”选择一个本地 NTFS 目录，预览前缀改名，再经原生
逐文件确认提交。同一 plan ID 可以在 GUI/CLI 查看和提交；日志记录逐项意图、
确认结果及实际身份。中断后显式重试核对原方向，不重复改名。撤销只接受身份、
内容及目标均未被外部修改的动作，所有改名拒绝覆盖。恢复旧库后，只能核对
challenge 作废旧计划，保留文件当前名称、私有定义及最终回执，不重放旧操作。

桌面设置中的“媒体处理”先选择文件和全新输出名，预览后明确授权读取、
创建文件及一个固定工具动作。两个实际内置插件 `plugin-media-inspector` /
`plugin-media-converter` 共用同一个受管工具包，完成信息查看、WAV 提取及
固定 VP9/可选 Opus WebM。普通 job receipt、进度、取消、重新查询和重启后的
结果仍由同一个 Runtime 管理。关闭 GUI 不取消已批准的后台任务。

CLI 示例（须先初始化本地 Runtime profile，构建固定 CLI/native artifacts）：

```powershell
flowtools files plan --directory D:\selected --prefix 整理_ --approve-read
flowtools files preview <plan-id> --revision 1
flowtools files commit <plan-id> --revision 1 --digest <sha256> --approve
flowtools files undo <plan-id> --revision 1 --digest <sha256> --approve
flowtools files recovery
flowtools files discard-restored --challenge <sha256> --approve
flowtools media plan --input D:\input.wav --output D:\output.wav --action audio --approve-read
flowtools media start <plan-id> --digest <sha256> --approve --wait
flowtools media status <run-id>
flowtools media cancel <run-id>
```

`media plan --action inspect` 不指定输出；WebM 使用 `--action webm` 和新 `.webm`
文件。文件/媒体选择仅进入认证本地管理面；插件输入只有 plan ID 和摘要。
GUI/CLI 共用 Rust 生成的协议和单 writer，不接受 executable、argv 或 filters。

## 工具与边界

- 实际程序：PGP 验证的官方 FFmpeg 8.1.3 源码受限构建，程序 SHA-256 为
  `36df0f37b9252cd8ec11a9c87caeda7c537e54ef75f59a5614aefecefc0e644e` /
  `60e8a872c8090a2c09ee1575c8c99b64b54b3c7af7c8da69158dab40c3fa7dfd`。
- 固定归档 `2b26f1bc4731c8db82f2bda708a96354bd31acf1af2cc16058f108d25ccfe415`，
  manifest `198713e499838a7f09258835fcb90b4b4121937377e4fa7135af2eb9998af233`。
  新生成的独立本地 Ed25519 根 quorum 为 2/2，另有一个 publisher key；未使用
  公开 fixture 私钥。私钥仅存 Git 忽略的开发目录内的 DPAPI 文件，不随源码
  或产品提交/打包，普通 build 不读私钥。
- `assets/builtin-media` 的原始签名字节不经过换行或 JSON 格式化。工具准入只为
  精确 archive/manifest/source/license/flavor/layout 匹配提供独立原型 policy；
  `REVIEWED_TOOLS` 仍为空。安装、镜像、accepted lease、current-floor、真实 LPAC、
  SID/capability、程序/Job 排空以及独立 probe 沿用原路径。
- 文件上限仍为本层 512 项/128 文件、单文件 64 MiB、合计 256 MiB；媒体沿用
  原 1 MiB 输入/输出、四活动调用、八尝试、64 保留组、5 秒原生执行及 500ms
  drain。原型可拒绝超限媒体，不声称面向长视频。元数据与加密私有载荷仍有
  原配额；工具及原生输入 GC 未开放，达到保留上限会明确拒绝新任务。
- 发布先持久化意图，在 held directory 下 create-only 写私有 staging 文件，
  flush/哈希校验后再次检查当前 grant、任务及安装元数据，再原生改名到新目标。
  未知写入保留日志/字节并拒绝重放；不会将 `progress=end` 当作完成。
- 本地原型 package 描述符按原 30 天上限签发，截止
  **2026-12-03 14:59:06 UTC**；根按原 365 天上限。到期拒绝工具准入；不绕过
  时钟、签名、撤销或版本 floor。正式续期/更新发行须走新的受控版本流程。

正式分发、Windows 代码签名、完整源码提供方案和独立安全/许可证审核继续
pending，不因原型签名或测试而获得法律/安全批准。一般第三方执行、任意
工具路径、跨平台、市场/updater/GC 与公开 Beta 属于后续范围。G5/G6 的现有
更新/队列/撤销回归继续保留，原型未声称完整生产成熟度。

## 验证

- Core 文件专项 22 项通过，含实际 NTFS 改名、重启重试、撤销、恢复作废、
  trigger 替换、未知回执和完整 128 文件提交/撤销；原配额未提高。
- 媒体发布两项实际文件/SQLite 专项通过：磁盘写入错误注入（Windows 112）、
  发布时撤销、SQL 替换及恢复拒绝。错误注入不等于真实磁盘耗尽演练。
- Rust 原生集成实际执行两个编译后的插件，签名安装/镜像/LPAC、独立 probe、
  WAV/WebM 新文件发布与冲突/输入变化/撤销/取消；三项全部通过，活动原生
  取消与撤销均确认排空后没有输出。
- CLI 与原生桌面协议实际进程联动通过：同一文件 plan ID、拒绝缺失确认、
  提交重试与跨端撤销、真实媒体发布、重启后结果及跨端同一回执。
- 固定 Chromium 的文件/媒体组件四项通过；该交互回归使用受控 transport，
  实际副作用另由上述 native/CLI 测试证明。人工项仅 waived。
- 十五个实际命令 artifacts 的无 React/GUI/source 消费、schema 拒绝、abort、
  maturity/清单和原有插件行为检查通过。完整独立 driver 原 67 项全部通过
  （6/10/8/9/6/5/5/4/4/4/6，无 fail/ignored），来源四项真实拒绝对照与实际
  GPGv 签名验证、精确 inventory、原 source/program pins、fmt/all-target Clippy
  通过。完整 Windows 根门禁结果见下文，最终文档提交的远端结果见上述 PR。

实现提交 `bcce635` 的首次完整门禁通过静态/类型阶段后，在 Rust golden
清单仍为十三项的完整性测试停止。现改由完整十五项 build-owned catalog
生成 fixture，并给纯语义图提供同一工具的 metadata candidate；不增加
安装/执行 authority。对应 Rust roundtrip 与 Runtime client 检查随后复验，
新的完整门禁单独记录，不覆盖首次失败。

`e55140d` 的首次重跑使用仓库内 TEMP，Core 文件/发布测试 22 项返回
`RuntimeBusy`（497 pass / 22 fail）。同一编译二进制在仓库内复现失败，
移至工作区外 `D:/flowtools-g7-test-temp` 后 24 项全部通过；未改变祖先目录
共享/身份保护，具体持柄进程未确定。工作区外的完整重跑随后通过 Core
519+11+13、Runtime 原生 27+6 与 Chromium 117/36，但 Runtime Bun 为
16 pass / 2 fail：新工具自动安装阻断了恢复后权限管理，旧库存 fixture 仍
预期未配置。失败日志保留，不以这些已通过的子项代替整仓成功。

修复将固定 Host 目录图与执行准入分开：只有 build-owned 工具声明用于
语义图；恢复状态保留原 trust/安装字节与 ToolAuthority，原工具任务接收、
持有回执恢复和每个 native effect 仍完整复核。实际恢复 fixture 增加了
权限管理/无工具任务可用、库存与媒体启动继续 `RECOVERY_PENDING` 的断言；
库存 fixture 则核对真实锁定归档、镜像和原共享配额，未放宽原期限/保护。
修复后精确运行当前 `clients.test.ts`，五项实际进程回归全部通过（85.65s），
Runtime lint/严格 types/Clippy 与文档契约通过；完整根门禁再次单独执行。

`caecf9c` 的完整门禁通过 Core 519+11+13（303.62s）、Runtime 27+6/Bun18
及 Chromium117/36 后，在 Desktop 命令参考过期处停止（111 pass / 1 fail）。
现从真实十五项契约重新生成 `builtin-commands.md`。随后的 Desktop 专项发现
原生只读库存测试也仍预期空库存（37 pass / 1 fail）；已改为核对真实锁定包、
镜像和原配额，保留离线不初始化/冷启动及查询前后原字节相同的全部断言。

修复后的完整 Desktop 专项 112 Bun + Node2 + Rust38+6 全部通过；随后按原
顺序完成 11/11 workspace build、Web25/Desktop29 实际产物等值/拒绝及最终
Rust fmt/check/all-target Clippy。上述逐段结果不代替下一次干净完整根门禁。

## 完整本机交付门禁

实现源代码 `02d416a1ea134ba7a350c2c974d67eb8db8230c4` 的
`pwsh -NoProfile -File scripts/check-ci.ps1` 完整通过，exit 0，最终工作区
无漂移。使用固定 native target 与工作区外 TEMP；原共享/身份保护、执行/
排空期限、原生/Node 断言与内容漂移检查全部保留。

- 11/11 workspace lint、types、test、build；Core 519+11+13（单元 295.03s）。
- Runtime 原生 27+6 / Bun18；固定 Chromium 117 项、36 文件。
- Desktop Bun112 + 真实 Node2 + Rust38+6；CLI101 + Node2、实际十五插件。
- Web25 / Desktop29 个真实产物在子进程 opt-in/canary 重建后逐字节等值，
  已知不安全入口/认证开关/canary 拒绝与最终 Rust fmt/check/all-target Clippy 通过。
- 额外执行 `bun run --cwd apps/desktop tauri build --debug --no-bundle`，实际
  完成桌面应用编译/链接（55.16s native build），产物为
  `<CARGO_TARGET_DIR>/debug/desktop.exe`。本机对应
  `D:/code/projects/flowtools/apps/desktop/src-tauri/target/debug/desktop.exe`；
  它是本地未签名 Debug 原型，正式发行和人工实窗验收状态仍按上述约定。

最终状态同步仅更新 Markdown 文档；全部非 Markdown Git blobs 须与上述
完整验证源代码相同，并单独运行文档契约/差异检查。最终 SHA、推送核对和
同 SHA 的 PR/push 两路 Windows 结果维护在上述 Draft PR，不用旧提交绿色替代。

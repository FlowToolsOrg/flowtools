# G6 固定媒体完成投影

日期：2026-10-31。P2.8b3 engineering prototype；专项已过，完整门禁另记。

`packages/runtime-core/src/packages/tools/media/completion.rs` 提供固定转换的
纯完成投影。调用方传入实际退出码、bounded 完整 progress、独立输出
ffprobe JSON 与 T0 观察的文件大小；不读取路径、启动进程或发布用户文件。

仅 WAV 提取和 WebM 转码可产生该投影。要求进程 exit 0、ProgressDecoder
完整结束且未中毒、非空/受限实际文件大小与 ffprobe 大小一致、正的已知
时长及固定 recipe 输出角色：单 PCM s16le audio，或 VP9 video 加可选 Opus
audio。额外 streams、错误格式/编码/缺失必要属性、私有 raw metadata、
重复 JSON key、进度/元数据超量均拒绝。进度 time/size 仍是 advisory，N/A
不转为零或伪造已知值；未把进度字节数当作实际输出大小。

返回值仍为现有 `MediaInspection` 数据，不是 opaque artifact proof、lease、
grant 或提交收据。原生 adapter 必须另行证明所拥有进程/Job 已排空、输出
是本次新建的 held file、ffprobe 读的是同一身份、当前授权/epoch 仍有效，
然后才可进入独立 create-only 文件事务。调用者提供的数值不能证明这些。

六个新增 Core 用例覆盖纯契约拒绝；独立非发布媒体候选保留原六项 inventory，
其中真实 WAV/WebM 与 `-n` 用例接入新投影。即使提供先前有效的输出元数据，
exit 0 但没有本次完整 progress 的覆盖拒绝也不能被投影为转换成功。
现有 source/candidate hashes、LPAC、PID/Job 与全部时间界限保持不变。

实际 Core 媒体专项十七项（含六个新增用例）通过，0.01s；当前 Core inventory
为 431。all-target Clippy `-D warnings` 通过，19.40s。日志
`execution-validation/g6/media-completion-core-20261031.log` 和
`media-completion-clippy-20261031.log`，均 exit 0。

同一显式 `scripts/test-media-candidate.ps1 -BuildRoot ...` driver 的固定 hash
准备、Rust fmt、实际 inventory 恰好六项、all-target Clippy（11.41s）和六项
真实媒体 LPAC 测试全部通过，0 ignored，2.90s。日志
`execution-validation/g6/media-completion-native-20261031.log` / exit 0。
WAV/WebM 的新完成投影读取独立真实 ffprobe 结果，已有文件的 `-n` 覆盖
拒绝仍实际 exit 0，但不能通过本次完整进度检查。source/candidate hashes、
全六项实际测试及其 PID/Job、500ms drain 和其他时限均未改变。

隔离 Rust 工作区首次直接运行完整 CI 契约组为 14 pass / 4 fail：本工作区
没有安装 Node workspace 依赖或构建 SDK，原 lint/Turbo/conformance/browser
依赖不可解析；媒体候选边界契约本身通过。日志
`media-completion-ci-contracts-20261031.log` 保留；随后在依赖已齐全的主
工作区重新运行原用例，结果另记，没有删除或放宽这些用例。

依赖齐全的主工作区随后运行相同十八项原 CI 契约全部通过，356 个断言、
41.33s，日志 `media-completion-ci-contracts-main-20261031.log` / exit 0。
Rust fmt、文档格式与 docs:check 同样通过；当前提交完整门禁仍另记。

本页不将前一 SHA 的完整门禁计为本提交通过；真实发行/审核 profiles、
原生 Runtime 工具 adapter、用户文件发布与 G7 闭环仍 pending。

随后 `24981c1` 完整本机根门禁 exit 0、最终无漂移：11/11 workspace
lint/types/test/build；Core 431+11+9（单元 1281.27s）；Runtime 原生 24+6
（46.55s/2.20s）及 Bun 17（含原 50 路冷启动）；Chromium 109/34；Desktop
106 Bun+Node 1+原生 36+6（3.52s/0.11s）；Web 23/Desktop 25 产物一致、
production refusal 和最终 Rust 检查通过。日志
`execution-validation/g6/root-ci-24981c1-20261031.log`。

远端 ref 已核对为 `24981c126d9875f63c44d9fe3c82f9b26e07550c`，
[PR 41267501533](https://github.com/FlowToolsOrg/flowtools/actions/runs/41267501533) /
[push 41267498969](https://github.com/FlowToolsOrg/flowtools/actions/runs/41267498969)
均已 success；前一步 `4887b40` 的两路远端 CI 也已 success。

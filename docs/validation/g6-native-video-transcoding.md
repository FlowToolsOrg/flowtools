# G6 Core 有界 WebM 转码与独立解码检查

日期：2026-11-01；2026-11-02 续验。P2.8b8，prototype。G5/G6/G7 父项
未完成；普通产品 Host 无工具 authority，production review/pins 仍为空。
G7 剩余人工项保持 waived，不记作 passed。

## 有限行为

`start_media_video` 只接受最多 1MiB 的 T0 字节，固定首个视频轨和可选首个
音轨，输出 VP9/Opus WebM 到有界 stdout；无客户端路径、argv、filters、
shell、网络或可写子进程输出目录。私有参数不修改原 V1 文件 recipes。
`CompletedVideo` 不可由调用者构造/克隆/序列化，只含字节、允许的元数据和
独立解码帧数，不产生用户文件发布权限。

同一已计费 ToolExecute 调用先运行 ffmpeg，再由 T0 将完整捕获的原字节
create-only 写入新的只读 Low 输入，独立 ffprobe 执行 `-count_frames`。
两阶段各持有独立 LPAC/SID/Job，恢复线程前回读实际 token/capabilities/
程序身份与 child/loader 策略。原五秒执行、500ms drain、1MiB stdout、
64KiB progress、四项在途、每任务八次和 64 组/64MiB 私密输入预算保持；
信息检查、WAV 和 WebM 共用计数及完整 Runtime 排空路径。

## 管道完成投影

不可 seek 的 WebM 可能没有 duration。新 `media/streaming.rs` 使用独立
完成投影，保留原 P2.8b3 的正时长要求和所有拒绝回归。新投影要求实际零退出、
完整有界 progress、独立 probe 的精确文件大小、严格 VP9/可选 Opus 角色，
以及每流规范的正整数 `nb_read_frames`，最多一百万。未知 duration 保持
未知，不从进度推算；显式零时长拒绝。ffprobe 非空错误 stderr 即使退出零
也拒绝；raw stderr 不进入持久记录或产品历史。

只新增一个允许字段，先拒绝重复 JSON key，剥离并验证后使用原严格元数据
解析器；未知字段、filename/tags、重复流、非规范/零/缺失帧数仍拒绝。
单独投影不证明文件身份、Job 排空、当前权限或发布。实际 actor 在准备、
resume、运行、提交与收取时复查原任务、完整授权链、epoch 和当前信任。

## 持久与恢复

新 optional/skip-empty `nativeVideo` 使用 `native-video-pipe-v1`；原
`nativeAudio` 格式与 profile 不变。固定 API/记录域选择操作，存储 profile
字符串不能选择操作。原单 writer、32KiB 元数据、exact predecessor 和
trigger 拒绝保持。记录只含固定身份、长度/hash、两个预留 UUID、期限与阶段。
未知 SQL 不覆盖，错误先回收 owned process，Interrupted 保留原输入与工具根。
重启中断，不恢复进程、handle、claim 或输出；纯 Base64 后台重试例外不能
重放 nativeVideo。所有原生媒体的保留 ID 均在 orphan cleanup 前严格验证。

## 验证记录

纯投影六项 **6 pass**（0.01s，462 filtered）；首个实际 Core 原生 WebM
正向 **1 pass**（13.31s，编译 39.30s，499 filtered），一次预算、两套实际
PID/SID/Job、新鲜同字节输入、精确角色/大小/三帧均核对。普通 Core inventory
为 468；加入九项 WebM 后候选 wrapper 总计 501，完整清单另记。

实际 Core 合成输出为 896 字节，按内容 hash create-only 保存，仅供验证。
Playwright 1.59.1 / Chromium 147.0.7727.15 实际播放到 ended：16×16、总解码
三帧，实际回调呈现三帧，时间 0/1/2s；canvas 像素分别 [0,0,0,255]、
[27,27,27,255]、[70,70,70,255]。日志
`execution-validation/g6/native-video-pinned-browser-initial-20261101.log`。
这不是 mock 或仅检查元数据，也不证明 OS 播放器/产品 UI 通过。

显式非发布 driver 分别核对 6/10/8/9、总计 33 项，不进入普通产品构建；
原候选 source/program hashes 和原始六项 LPAC feasibility 测试保持。

首轮完整 driver 原六项 **6 pass**（47.19s）、信息 **10 pass**（121.19s）、
WAV **8 pass**（111.39s），WebM **7 pass / 1 fail**（113.42s），Clippy
18.72s 通过。失败是恢复构造器的精确错误码：测试预期 StoreCorrupt，实际
Host setup 在原 artifact 边界映射为 DependencyArtifactMismatch。加强测试
同时直接核对原严格 retention reader 的 StoreCorrupt，以及 Host 构造器的
精确映射拒绝和两份实际输入保留；产品拒绝路径未改、没有放宽断言为任意错误。
日志 `execution-validation/g6/native-video-full-driver-initial-20261101.log`。
新增真实双流样例验证可选 Opus 编码及每流独立解码；最终 33 项结果另记。

2026-11-01 修正并加入双流样例后，WebM 九项定向 **9 pass / 0 fail /
0 ignored**（127.02s，492 filtered），编译 20.57s；同时精确核对 strict
retention reader/Host setup 两层拒绝、旧输入保留、纯后台重试正向对照和
原视频不重放。双流样例实际得到 VP9 三帧与 Opus 正帧数、48kHz 单声道。
日志 `execution-validation/g6/native-video-nine-focused-20261101.log`。
两项 CI 边界契约 **2 pass / 0 fail**（265 assertions）、严格类型及文档
契约通过。2026-11-02 的完整 33 项 driver 与最终整仓结果继续单独记录。

2026-11-02 首次 33 项 driver 在 all-target Clippy 拒绝测试 AVI 构造器的
手写奇偶条件处终止，未运行后续四组真实测试。改用 Rust 1.96.0 的
`is_multiple_of(2)`，保持完全相同的 RIFF padding；未加 lint 例外。
原日志 `execution-validation/g6/native-video-full-driver-final-20261102.log`
保留，修正后的完整结果另记。

2026-11-02 修正后的完整显式 driver **exit 0**：原六项 **6 pass**（36.03s）、
信息 **10 pass**（85.82s）、WAV **8 pass**（89.34s）、WebM **9 pass**
（98.05s），均 0 fail / 0 ignored。精确 6/10/8/9/33 inventory、格式、实际
探针与 all-target Clippy（5.73s）通过。日志
`execution-validation/g6/native-video-full-driver-final-fixed-20261102.log`。
包含实际 VP9/Opus 双流、两套 PID/SID/Job、新鲜 probe、取消与当前撤销、
未知 SQL/原字节保留、共享工具独立授权和后台恢复不重放。专项不代替最终
干净提交的根门禁或远端结果。

普通 Core 的媒体/完成投影与 capture 共 **26 pass**（0.01s，442 filtered），
包含原 V1 正时长/角色/进度拒绝，普通 Core all-target Clippy 通过（12.45s）。
主工作区实际 Runtime all-target `cargo check --locked` 通过（6.65s），确认
默认 Host 与内部新 API 集成可编译。日志分别为
`execution-validation/g6/native-video-core-media-capture-20261102.log`、
`native-video-core-clippy-20261102.log`、`native-video-host-check-main-20261102.log`。

主工作区全部二十项 CI 契约 **20 pass / 0 fail**（26.41s，626 assertions），
包含更新后的精确候选清单、原始拒绝/失败传播与内容漂移检查；日志
`execution-validation/g6/native-video-ci-contracts-main-20261102.log`。最终干净
提交仍需完整根门禁，不以这些专项代替。

## 剩余范围

用户文件 read/write grant、原生 create-only 输出发布/撤销/恢复、GUI/CLI
媒体工作流、accepted 旧版本 continuation、更新/LKG/GC、签名发行、完整
source offer、法律及独立安全审核仍待完成。此项不关闭 G7。

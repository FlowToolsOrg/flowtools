# G6 Core 有界 WAV 提取与独立检查

日期：2026-11-01。P2.8b7，prototype；当前专项与最终整仓结果分别记录。
普通产品 Host 仍无工具 authority，production review/pins 为空；G5/G6/G7
父项未完成，G7 剩余人工项为 waived，不能记作 passed。

## 当前行为

`RuntimeCore::start_media_audio` 只接受非空且最多 1MiB 的 T0 字节，返回
不可构造/克隆/序列化的 actor handle。仅固定 WAV 提取；无用户路径、argv、
filter、shell、网络 URL 或客户端选择器。`CompletedAudio` 只返回受限字节和
检查后的元数据，没有文件发布权限、executable getter 或延期 grant。

Core 在同一已计费 ToolExecute 调用内依次运行固定 ffmpeg 和独立 ffprobe。
两者都独立创建实际 LPAC/Job、回读 token/SID/capabilities/策略/image identity，
分别执行原五秒和 500ms owned process AND Job drain 界限。每次准备、恢复
线程、运行、结果提交和收取均复查当前全链权限、epoch、期限、任务原行、
当前信任与 held image；第二程序不能借用另一消费方或另一个 pin 的权限。

`images/inspection/profile.rs` 是私有有限 stdio 配置。原 Inspect 参数和
65536 字节聚合预算保持。WAV 的输入仍为只读私有 `input`，仅二进制 stdout
和 progress stderr；子进程没有可写的输出文件/目录，所有 argv 由 Rust 固定。
stdout 硬上限 1MiB、progress 65536 字节，聚合 1114112；超限后 capture
永久拒绝，取消实际 owned Job，不返回部分字节。WAV 每轮最多读 64KiB；
原 Inspect 仍最多 8KiB，没有扩大其输出预算或任何期限。

ffmpeg 完全退出且 Job 排空、有完整有界 progress 后，T0 才把捕获字节
create-only 写入另一个新的 Low 只读输入。独立 ffprobe 读取该实际 held 文件，
再由原 `project_completed_output` 检查 exit/progress、WAV/PCM16 单音轨、
已知正 duration 和精确观察大小。原 V1 文件输出 recipes、完成投影及其拒绝
语义未修改。两程序各用自己的 AppContainer SID/环境/stdio/Job，工具文件
可以共享，私有输入和授权不共享。FFmpeg 的
[pipe](https://ffmpeg.org/ffmpeg-protocols.html#pipe) 与
[progress](https://ffmpeg.org/ffmpeg.html) 只是固定传输方式，不代替 LPAC 或授权。

## 持久阶段与恢复

`runtime_media/audio.rs` 的 private formatVersion 1 /
`native-audio-pipe-v1` 记录原 consumer/archive pin、两项随机 input UUID、
原输入长度/hash、输出长度/hash、deadline 与有限 phase，不保存原始输入、
音频、路径、PID 或 stderr。Preparing 在 OS IO 前；Extracting/Probing 在
相应 resume 前；PreparingProbe 在第二输入/进程创建前，沿用原 jobs.metadata
writer、32KiB 上限、完整 predecessor 及 trigger guards。

与信息检查共享四项在途、每任务八次逻辑调用、profile 64 个输入组/64MiB；
一项 WAV 提取预留两个 UUID。保留 ID 只来自严格读取并核对 actor 原摘要的
实际持久行。未知 SQL 不覆盖，未知排空/准备保留根和私密字节并停止 admission；
错误返回前先尝试排空实际 owned process。普通 root/service 不能提前成功或
释放仍持有的工具。终态提交不清除未知/Interrupted 的根和私密输入。

重启只中断，不重建 handle/claim、进程或字节输出；含任何 native attempt
的任务也不能使用既有纯 Base64 后台重试例外。实际回归使用相同 catalog/
epoch/后台 grant 的两任务对照：无 native attempt 的任务仍可排队，原 WAV
任务保持 Interrupted 且不可 take_run，两份实际输入原字节保留。没有续权、
重绑定 seal、重放或隐式 GC。

## 专项记录

独立 `managed-media-candidate` driver 分别核对原六项、信息检查十项、WAV
八项、总计 24 项，再运行 all-target Clippy 和三组串行回归。普通根 CI 不
执行候选程序，也没有产品 feature/Host selector。实际 source/program hashes、
公开 fixture 签名及生产空 review 边界沿用
[候选记录](./g6-installed-media-candidate.md)。

首个实际正向用例 **1 pass**（12.69s），固定消费方以一次预算完成两次
独立原生进程、同字节 probe、精确输出和正常终态。首次完整 driver 原六项
6 pass（41.93s）、信息十项 10 pass（110.77s）、WAV 7 pass / 1 fail（95.49s）；
输出预算样例使用候选未启用的 PCM_u8，实际为 OutputInvalid，尚未到达预算
路径。没有改候选/来源/decoder 配置或放宽断言。

换为支持的边界 PCM16 后实际先触发 Timeout，保留原失败日志。WAV 管道
每轮读取从 8KiB 调整到有限 64KiB；原五秒和 1MiB 不变。相同边界样例
随后 **1 pass**（10.62s），精确 BudgetExceeded、actual process exited /
Job active=0 且未创建 probe input 均成立。普通 Inspect 读取和预算保持。

修正后完整显式 driver exit 0：原六项 **6 pass**（39.55s）、信息十项
**10 pass**（107.16s）、WAV 八项 **8 pass**（98.54s），0 fail / 0 ignored；
实际 6/10/8/24 inventory、格式、探针与 Clippy（6.00s）通过。日志
`execution-validation/g6/native-audio-full-driver-bounded-read-20261101.log`。
随后后台恢复测试加强为上述相同授权的正向对照，定向 **1 pass**（14.94s）；
最终完整清单和当前整仓门禁仍另记。

最终当前修改树的完整 driver 再次 exit 0：6 pass（43.14s）、10 pass
（111.28s）、8 pass（103.72s），均 0 fail / 0 ignored，包含加强后的后台
正向对照；精确 24 项清单、格式、探针和 all-target Clippy（7.83s）通过。
日志 `execution-validation/g6/native-audio-full-driver-final-20261101.log`。
普通 Core 的 capture 三项 **3 pass**（459 filtered）；Core all-target
Clippy（13.75s）、两项边界契约（261 assertions）、严格 TS 与文档契约通过。
完整根门禁仍须在最终干净提交运行，不以专项代替。

主工作区整合后，二十项 CI 契约 **20 pass / 0 fail**（29.43s，622 assertions），
实际 Runtime all-target `cargo check --locked` 通过（9.81s）；文档和类型检查
通过。当前提交仍须完整根门禁，不把这些检查描述为 G7 办公闭环完成。

八项覆盖一次预算/两个真实 PID、SID 和 Job/新鲜 probe；恢复线程前取消；
probe resume 前和最终提交前撤销；未知 SQL 原字节及实际排空；边界硬配额；
一消费方取消而另一方继续；后台恢复不重放及保留。capture 的独立状态回归
还检查两管道的各自/聚合上限、sticky refusal、原 Inspect stderr 丢弃和单次收取。

## 独立实际解码

真实 Core 输出的 32078 字节合成 WAV 以 create-only 方式保存为忽略的测试
artifact，已有文件必须与本次结果逐字节一致。使用项目已固定的 Playwright
1.59.1 / Chromium 147.0.7727.15 实际 Web Audio 解码：单声道、16000Hz、
16000 帧、1 秒，全部 16000 个 PCM 样本逐项匹配；不是 mock 或元数据推断。
日志 `execution-validation/g6/native-audio-pinned-browser-all-pcm-20261101.log`。
首次脚本误引用未安装的 @playwright/test，改用项目实际声明的 playwright；
随后修正合成样本的参考算法为 Chromium 正/负整数各自缩放及 float32 运算，
保持逐样本精确比较，没有扩大误差。算法参照
[Chromium source](https://chromium.googlesource.com/chromium/src/+/HEAD/media/base/audio_sample_types.h)。
这证明固定 Chromium 的实际解码，不证明 OS 播放器、声卡或产品 UI 已验收。

## 仍未完成

当前只有受限 T0 字节到受限 T0 WAV；无用户文件 read/write grant、GUI/CLI
媒体操作、用户输出 create-only/覆盖发布/撤销/恢复。WebM 原生 adapter、
accepted 旧版本 continuation、更新/LKG/GC、真正签名发行、完整 source offer、
法律与独立安全审核仍 pending，不把 fixture key 或成功子进程作为生产权限。

前一步 `b804353` 完整本机门禁 exit 0、最终无漂移：Core 459+11+10
（1509.20s）、11/11 workspace、Runtime 24+6/Bun 17、Chromium 109/34、
Desktop 108+Node 2+原生 36+6、Web 23/Desktop 25 与最终 Rust 检查通过。
同 SHA 的 [PR 41312363809](https://github.com/FlowToolsOrg/flowtools/actions/runs/41312363809)
及 [push 41312358557](https://github.com/FlowToolsOrg/flowtools/actions/runs/41312358557)
均 success；不替代当前修改树的整仓门禁。

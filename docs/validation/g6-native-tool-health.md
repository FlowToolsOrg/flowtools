# G6 P2.8c3：固定合成媒体健康检查

日期：2026-11-02。当前为隔离工作区已验证的有限原型，Core 两项与原生五项通过。
完整 44 项与整仓结果另记，不能以本文认定 G5/G6/G7 完成。

## 范围与行为

Core 的 T0 `start_tool_health` 只创建固定的一秒 PCM16、16kHz、单声道合成
WAV，包含非静音、有限且可重现的样本。输入不来自用户、路径或 argv。
它复用普通 `start_media_audio`：accepted 任务原 lease、当前 manifest/grant、
全部祖先 epoch/预算、实际签名 ZIP/密封 image 和固定 WAV 转换；随后由第二个
实际 LPAC/Job 的 ffprobe 独立检查同字节输出。

只使用原有一个逻辑调用、两个保留输入、四项在途、每任务八次、64 物理组、
原 stdout/progress 与五秒/500ms 期限。没有额外进程入口或 capability，没有
更改原媒体 recipes、完成条件、jobs.metadata 格式、writer 或恢复策略。
重开仍中断原生尝试，不恢复内存 health handle 或自动执行。

健康成功还要求独立 metadata 明确为一秒 WAV、PCM16/16kHz/单声道，实际
RIFF/fmt/data 结构无歧义，全部 16000 样本与原固定合成信号逐字节相同。
有限 LIST/JUNK 可以位于数据前，超过有限 chunk/大小界限、重复 fmt/data、
额外尾部、未知时长、轨道或大小均拒绝；固定非 seekable pipe 的未知 RIFF/data
长度不等于未知实际观察大小，必须仍满足完整已捕获字节和精确样本。

收取时重复原在途调用的全部当前检查，然后使用普通 current-floor
`check_active`；C1 允许旧在途调用排空的例外不能产生“当前版本健康”观察。
最后再次检查任务原记录、当前完整 claim/授权，并绑定本次固定输入 hash/长度
与原工具 archive pin。撤销、过期、恢复、SQL 改动仍拒绝。

不提供字节、path、PID、seal、argv 或 executable；返回不可由外部构造、
不可序列化/Clone 的 `ToolHealthObservation`，仅可读取有限工具 pin 与观察
时间。它是诊断观察，不激活版本、不赋予后续执行权、不降低 floors、
不替代健康切换事务或 LKG。普通 Host authority/review/pins 仍为空；
没有 IPC/SDK/CLI/GUI selector 或自动健康任务。

## 验证记录

纯投影测试与五个实际候选检查已写入，原 6/10/8/9/6 分组不变，driver 将
额外强制收集 health 5、总数 44；原六项跳过每个子分组，不混淆通过数量。
源码契约单项 1 pass / 261 assertions、严格 TS、Rust 格式检查通过；尚不能
替代 Rust 编译、五个真实进程测试或全量 44 项专项。

当前 `cargo test --locked ... health -- --test-threads=1` **2 pass / 0 fail**
（0.02s，484 filtered，编译 22.72s），Core all-target Clippy 通过（11.86s）。
当前 Core 总数 486。日志 `execution-validation/g6/tool-health-core-initial-20261102.log`。

真实已安装签名 fixture 的五项原生 health **5 pass / 0 fail / 0 ignored**
（71.47s，525 filtered，编译 20.97s），当前独立 wrapper 共 530 项。
涵盖同样本真实 FFmpeg/ffprobe、两套 PID/Job 排空、一次调用、收取后不可重用、
floor 前进不产生当前健康、完成后撤权、取消不影响另一个消费方，以及跨
Core 句柄拒绝。日志 `execution-validation/g6/tool-health-native-five-initial-20261102.log`。

扩展 driver 的聚焦源码契约 **1 pass / 261 assertions**，严格 TS 与格式检查
通过。最初整份 CI 契约在无 Node workspace 依赖的隔离工作区为 16 pass /
4 fail：一个 driver 清单更新尚未同步，另外三个缺少 Turbo/manifest/browser
构建依赖。该结果没有记为通过；修正清单后只运行独立源码契约，整份二十项
仍须在主工作区正常依赖下复验。

完整 driver 随后 **exit 0**：原六项 **6 pass**（40.13s）、信息 **10 pass**
（105.87s）、WAV **8 pass**（98.96s）、WebM **9 pass**（109.01s）、续接
**6 pass**（67.51s）、健康 **5 pass**（70.34s），均 0 fail / 0 ignored。
精确 6/10/8/9/6/5/44 inventory、原实际探针、格式、候选 all-target Clippy
（10.43s）通过；原 source/program hashes 和原六项拒绝/期限/Job 断言保持。
日志 `execution-validation/g6/tool-health-full-driver-initial-20261102.log`。
最终干净提交的完整根门禁、对应远端 CI 与后续健康激活仍须分别记录。
当前 G7 文件授权/事务/撤销、用户媒体发布、工具健康切换、GC/LKG 和正式
工具审查仍未完成。

主工作区整合后实际 Runtime all-target `cargo check --locked` 通过（4.58s）；
全二十项 CI 契约 **20 pass / 0 fail**（25.58s，634 assertions），原完整
失败传播、Node companion、元数据/生成漂移与产物拒绝契约保持。
日志 `execution-validation/g6/tool-health-main-focused-20261102.log`。
该组合命令后续的 tsgo 因相对 typeRoots 参数触发编译器内部 path-not-absolute
panic 而 exit 2；相同严格检查改用绝对 typeRoots 后单独通过，文档契约也通过。
日志 `execution-validation/g6/tool-health-main-type-docs-fixed-20261102.log`。
未通过放宽类型或换用宽松模式规避；原 panic 与此前隔离工作区失败日志保留。
最终干净提交仍须完整根门禁及对应远端检查。

`afd50f41b7994f7dcffd29e40b6181267dadc054` 随后完整本机根门禁 **exit 0**、
最终无漂移：Core 486+11+11（1474.29s）、11/11 workspace、Runtime 24+6
（45.21s/1.87s）及 Bun 17、Chromium 113/35、Desktop 108 Bun+Node 2+
原生 37+6（3.94s/0.16s），Web 23/Desktop 25 一致性与 production refusal、
最终 Rust check/clippy（7.02s/6.36s）通过。日志
`execution-validation/g6/root-ci-afd50f4-20261102.log`。已推送并以 ls-remote
核对相同 SHA；[PR 41414986075](https://github.com/FlowToolsOrg/flowtools/actions/runs/41414986075)
与 [push 41414977562](https://github.com/FlowToolsOrg/flowtools/actions/runs/41414977562)
后续均已核对 success；PR 保持 Draft，健康结果约束的激活仍 pending。

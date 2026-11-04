# 原生媒体阶段与有限进度

日期：2026-11-02。范围：Core T0 的 WAV/WebM 进度观察，G7 仍为
**in-progress / prototype**。

## 行为

`media_audio_progress` / `media_video_progress` 使用原不透明句柄，返回
Converting、Validating 或 Complete，以及原 `MediaProgress` 的有限时间/
大小/结束标记。没有原始管道、stderr、输入、输出字节、路径、PID 或用户
标识；没有新 IPC、客户端执行选项、权限或发布入口。

观察复用最后一次正常 maintenance 已收取的最多 64KiB progress，使用原
严格 decoder，只公布完整记录。未知时间/大小继续未知；既有高水位、坏记录
与溢出拒绝保持。重复观察不收取结果、不启动下一阶段、不消耗额外工具调用。
Host 仍须执行原 maintenance，推进 actual owned process/Job、pipe 和 probe。

`progress=end` 只是工具报告。转换进程与 Job 排空后才进入独立 probe；probe
完成、实际字节投影、当前完整授权及持久提交通过后才显示 Complete。时间/
大小不作为百分比、输入时长或发布成功；WebM 管道的未知时长不被推算。

每次观察仍验证原任务元数据前值、完整委托链/claim/当前权限和实际工具。
拒绝时不返回旧快照，仍活跃的实际进程通过原停止/排空路径处理。未知 SQL
或排空继续停止准入并保留 actor/输入；不补写异常任务记录。完成后的真正
结果收取继续独立检查当前授权。

新增数据只在内存中；原 `jobs.metadata`、profiles、四在途/八次尝试/64 组
输入、stdout 1MiB/progress 64KiB、五秒与 500ms、原 recipes/projections 和
actual LPAC/PID/Job 检查保持。普通 Host 无工具 authority，生产 reviews/pins
为空；观察不是健康激活、后续执行 grant 或文件提交权。

## 验证

- Core capture 五项通过（新增两项、原三项），覆盖分块 CRLF、只公布完整
  记录、未知字段值、结束标记不结束 capture、回退/坏记录/溢出与 stderr 隐私。
- 新增五项实际候选通过（35.26s），覆盖 WAV/WebM 的真实转换与独立验证、
  重复观察后仍可收取、未知 WebM 时长、跨 Core 句柄、撤权/SQL 拒绝及真实
  PID/Job 排空。之后补充单次调用配额的强断言，纳入完整专项复验。
- driver 保留原 6/10/8/9/6/5 分组，额外精确五项，总计 49；原组不能被新项
  隐藏或跳过。对应 CI 契约 1 pass（265 assertions）及严格 TS 通过。
- 初次编译的私有模块导出错误已按原层级 re-export 修正；一次错误的测试
  名过滤因零用例 fatal 退出，改用实际用例名后通过。未放宽可见性或允许
  no-tests，失败日志保留。

当前完整专项、干净本机根门禁和远端 CI 分别记录，未完成的结果不计 passed。
日志位于忽略目录 `execution-validation/g6/`，前缀 `media-progress-`。

G7 用户文件 read/write grant、事务/撤销/恢复、媒体发布、工具版本激活/
GC/LKG、两个媒体插件 GUI/CLI 闭环、正式审核与签名发行仍 pending。
人工项保持 waived；见 [完成矩阵](./g7-completion-matrix.md)。

完整原生 driver 随后 **exit 0**：原 6 pass（25.61s）、信息 10 pass
（66.06s）、WAV 8 pass（59.47s）、WebM 9 pass（71.03s）、续接 6 pass
（43.45s）、健康 5 pass（45.42s）、进度 5 pass（37.18s），均 0 fail /
0 ignored。精确 6/10/8/9/6/5/5/49 inventory、实际探针、固定 source/program
hashes、格式和 all-target Clippy（18.67s）通过。新 WAV 用例将 grant 限为
单次调用，重复进度观察及收取均成功，之后新调用准确返回 BudgetExceeded，
任务仍只有一个 native attempt。没有放宽任何原预算、期限或断言。日志
`execution-validation/g6/media-progress-full-driver-20261102.log`。

随后提交 `69666ce` 的完整本机根门禁 exit 0：11/11 workspace lint/types/test/
build、Core 500+11+11、Runtime 24+6、Chromium 113（35 files）、Desktop
108 Bun + Node 2 + Rust 37+6、生产产物等值/拒绝及最终无漂移通过。对应
[PR 41470321494](https://github.com/FlowToolsOrg/flowtools/actions/runs/41470321494)
与 [push 41470311151](https://github.com/FlowToolsOrg/flowtools/actions/runs/41470311151)
均 SUCCESS；不等同于完整 G7 验收。

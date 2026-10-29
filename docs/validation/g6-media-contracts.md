# G6 P2.8b1 固定媒体 recipes 与结果投影

日期：2026-10-29。范围：T0 Rust 纯契约 / prototype，G6 与 G7 整体仍 in-progress。
此模块没有进程、磁盘、网络、安装/lease 或 broker grant API，不产生可执行身份。

`packages/runtime-core/src/packages/tools/media.rs` 提供三种版本化有限动作：
inspect 使用 ffprobe；extract-audio-wav 使用固定 PCM WAV；transcode-webm
使用固定 VP9/Opus 参数。输入绑定只叫 input，输出名称固定；不接受调用方
路径、URL、shell、argv、filter、codec 或 executable。FFmpeg 禁止 stdin 和
覆盖，输入协议仅 file，输出不复制 metadata/chapters。实际程序仍须由受管
签名工具库存/lease 解析，不以 recipe 自行定位/启动工具。

ffprobe 仅请求格式、时长、大小和固定 stream 字段。最多 64KiB JSON、32 流、
16 个格式 token、32 声道、384kHz、单边 16384/总 64Mi 像素，媒体大小
256MiB、时长七天。复用严格重复 key JSON 解析，再拒绝未知字段、filename、
tags、控制字符、非法 codec/token、重复 index 和越界数值。时长按十进制
精确转毫秒，不通过浮点舍入越过上限；N/A/缺失为明确 unknown。

progress 按有界分块/CRLF 解析，单行 512 字节、合计 64KiB；只投影 output
time、reported bytes 与 end marker。out_time_us 除以 1000 转毫秒；不将
历史 out_time_ms 的名称误当毫秒。N/A 保持 unknown，负初始时间投影零；
时间/大小高水位跨 unknown 记录保留。重复、回退、非法 key/控制字符、超量、
结束后的新内容及未完成记录拒绝；首次错误后 decoder 永久拒绝继续使用。
未知但有界的其他进度字段丢弃，不复制原始文本到日志。

依据 [FFmpeg n8.0 的原始 progress 实现](https://github.com/FFmpeg/FFmpeg/blob/n8.0/fftools/ffmpeg.c)：
out_time_us/out_time_ms 都输出同一微秒值，未确定时间/大小可为 N/A，progress
以 continue/end 划分记录。该源码对照用于解析协议，不是本项目选择/审核了
该发行版本或真实 codec build。

十项纯 Rust 回归覆盖固定 recipe 与任意请求拒绝、正常元数据、私有字段与
重复 key、字节/stream/pixel/整数边界、精确时长、分块进度、N/A/负时间、
跨 unknown 高水位、永久错误状态及输出预算。测试未执行 FFmpeg，也不证明
codec 可用、真实可播放产物、平台隔离、tool sharing 或 G7 办公闭环。

后续 adapter 必须使用真实受管工具与新鲜授权，在 LPAC 内运行并依据实际
字节/时钟/Job exit 强制预算；progress 是不可信建议值。进程成功退出后仍须
验证输出，再通过独立的 create-only 文件发布与恢复事务提交用户目标。
实际发行/许可证/安全审阅、库存/leases/撤销更新，以及 GUI/CLI 两插件共享和
端到端媒体验收仍 pending；没有改变 maturity 或解除生产入口拒绝。

本机 `cargo test --locked -p flowtools-runtime-core packages::tools::media`：
10 passed / 0 failed。Core all-target Clippy warnings-as-errors 与 docs:check
通过；最终整仓及远端验收须针对包含本模块的实际提交单独完成。

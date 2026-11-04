# G6 P2.8b2 真实媒体构建候选与 LPAC 专项

日期：2026-10-29。状态：prototype，最终六项实际 LPAC 专项全部通过。
这是一项显式 developer acceptance，不是生产工具审核、签名发行、工具
安装/lease 或 G7 GUI/CLI 媒体闭环；正常生产 `REVIEWED_TOOLS` 保持为空。

## 构建与来源

`scripts/media-candidate/` 固定 FFmpeg 8.1.2、Opus 1.6.1、libvpx 1.17.0。
不下载或运行系统 FFmpeg，不安装全局工具、不执行下载管道。构建前离线核对
归档 SHA-256、FFmpeg tag 签名和完整 Git tag/commit/tree，之后才展开源码。
现有展开目录须逐字节、完整库存一致；路径穿越、链接、重复/大小写碰撞、
特殊文件与超量解压拒绝。只有同一 build root 下的固定目录会产生构建输出。

FFmpeg Git tag 签名指纹来自[官方验证说明](https://ffmpeg.org/download.html)：
`DD1EC9E8DE085C629B3E1846B18E8928B3948D64`，与 tarball RSA key 不同。
GitHub API 中保留的旧证书已过期；官方站点取回的证书也过期，均未据此放宽
拒绝。更新材料来自 [OpenPGP fingerprint endpoint](https://keys.openpgp.org/vks/v1/by-fingerprint/DD1EC9E8DE085C629B3E1846B18E8928B3948D64)，
仍核对相同 fingerprint、固定证书 SHA-256、有效自签与实际 tag 签名，拒绝
expired/revoked signature。源码验证用自己的公共 keyring，不导入用户 keyring；
这些公开上游 key 绝不进入 Runtime production pins 或 fixture package pins。

已实际核对 FFmpeg 10,135、Opus 482、libvpx 1,258 个源码文件。
FFmpeg archive 同时重算 Git tree；Opus hash 与上游公布值核对；libvpx 为固定
官方仓库 commit 的 TLS archive/hash pin，不声称后两者有本次独立 PGP 签名。
Python 四项 hostile/archive/Git/库存回归及 `-O` 下四项均通过。

`build.sh` 只从这些精确源码构建静态文件。禁用网络、自动检测、共享库、
ffplay、设备输入与默认全功能；明确列举 decoder/demuxer/muxer/protocol/
encoder/filter/bsf。仅启用 file/pipe、WAV、WebM 和固定 recipe 必需能力。
libvpx 仅构建真实 `libvpx.a`，不构建未使用的独立 C++ RTC library，不修改上游。
配置输出为 LGPL 2.1 or later；这不代替 [FFmpeg 许可证审阅](https://ffmpeg.org/legal.html)、
完整 source offer、MinGW/GCC runtime exceptions、Opus/libvpx notices/patents、
漏洞/构建安全与独立法律批准。跨 build root/机器的 bit-reproducibility 尚未证明。

## 显式接受测试

独立非发布 Cargo workspace：
`apps/runtime/test/fixtures/media-candidate/`。它没有产品 binary target，不属于
root/default Runtime build/test。普通 CI 只校验其锁、边界和真实 Cargo 选择，
不会因二进制缺失而静默跳过媒体测试，也不冒充已运行了它们。
开发者必须显式运行 `scripts/test-media-candidate.ps1 -BuildRoot ...`。

准备脚本先核对 tracked source/candidate locks、source receipt、两份精确
program hash 和大小，再原子生成固定 ignored `.generated/bin`。不接受任意
expected hash、程序名、runtime executable path、argv、Host selector 或 fallback。
候选文件缺失使编译失败；变更 bytes/hash 或没有实际收集测试均 fatal。
锁仅是这次 build-only candidate 的身份记录，不是 InstalledInertTool 或执行授权。

六项使用相同真实 LPAC/Job/恢复前 token、SID、capability、继承对象和策略复核：

1. 固定候选 hash/header 拒绝发生在创建子进程前。
2. ffprobe 读取确定性生成 WAV，验证实际时长/大小/PCM/sample rate/channels 投影。
3. 实际 WAV 提取、PCM 字节相同，随后用独立 ffprobe 再检查输出；对不同的已有
   canary 输出拒绝覆盖，原字节保留且没有完整 progress/end。
4. 确定性生成 H.264 I_PCM/AVI，实际转 WebM；检查 VP9/尺寸/时长，再由新的私有
   Job 实际解码并二次转码，不把单一文件后缀或 metadata 当作解码成功。
5. 真实可达 loopback listener 的正向对照之后，恶意 HLS/HTTP 素材拒绝且无连接。
6. 真实编码器启动并写出后取消，原 500ms wait/drain 保持；检查实际 Job 清空、
   private host canary 不变且没有向 host 发布 partial output。

这不是用户媒体的视听验收，也没有 GUI、CLI 普通命令、两个插件共享工具、
持久 leases、逐效果 grants/epochs、输出身份验证/发布、更新/LKG/GC 或恢复。

## 实测发现与修复

首轮实际六项为 3 pass / 3 fail：有效 ffprobe 输出含空 `programs`、
`stream_groups`，旧 RawProbe 因 deny-unknown-fields 拒绝；WebM 缺失
`vp9_superframe`，实际进程返回 unsigned 3115105544 / signed -1179861752。
这与纯契约十项通过同时存在，不能覆盖或删除。

投影现只允许两种确切空数组；非空、null、object、任何 nested metadata 仍拒绝。
新增对应回归后 Core 媒体共十一项通过，真实 ffprobe 与 WAV/PCM 专项也通过。
候选构建补入固定 `vp9_superframe` 后，使用仓库内完整配方重建，所有六项
实际 LPAC 专项通过（2.25s），没有筛选或 ignored 项；同一显式 driver 的 fmt、
实际 inventory 6 与 all-target Clippy warnings-as-errors 均通过。
日志 `execution-validation/g6/media-native-final-20261029.log` / exit 0。
旧 candidate pin 对新构建的准备操作实际拒绝，既有 fixture images 的 hash
保持不变；核对新候选后显式更新 build-only lock 才能准备并运行它。
另有真实编译缺-image 拒绝及实际 inventory 期望 7/收集 6 拒绝；恢复图片后，
最终完整 driver 再次通过六项（2.23s）。非 Windows/x64 显式 compile_error，
不产生空测试成功；源验证脚本要求恰好四项，普通与 `-O` 都保持拒绝逻辑。

还确认 pinned FFmpeg 的 `assert_file_overwrite` 返回 AVERROR_EXIT，而 main
把它映射为 0：`-n` 拒绝时**退出码仍为 0**。实际断言包含该真实退出值、明确
refusal、不同 canary 原字节保留和 progress.finish 拒绝；未假定非零退出。
未来产品执行器必须联合完整 progress、实际进程/Job 退出与新输出身份/内容复核，
不能仅据退出 0 或 end marker 提交用户文件；原 stderr 不进入产品历史。

完整本机/远端验收应绑定最终提交，不能用此专项替代。前一个 `ab73889` 的本机
完整根门禁 exit 0（首次 D 盘不足失败后改 C 盘独立 target 重跑），其
[PR 40868819104](https://github.com/FlowToolsOrg/flowtools/actions/runs/40868819104)
和 [push 40868809341](https://github.com/FlowToolsOrg/flowtools/actions/runs/40868809341)
均 SUCCESS。这些是上一个提交的整仓证据。

后续候选提交 `c24b274` 完整本机根门禁 exit 0：11/11 workspaces 的 lint/types/
test/build、Core 288+11+3、Runtime 原生 24+6/Bun 17（含原 50 路冷启动）、
Chromium 109/34 文件、Desktop 原生 36+6、Web 23/Desktop 25 产物一致及最终
clean-worktree 通过。日志 `execution-validation/g6/root-ci-c24b274-dcache-20261029.log`。
[PR 40879978485](https://github.com/FlowToolsOrg/flowtools/actions/runs/40879978485)
和 [push 40879967308](https://github.com/FlowToolsOrg/flowtools/actions/runs/40879967308)
均 SUCCESS；普通根 CI 仍不执行独立六项真实媒体专项。

首次同 SHA 的完整检查在 standalone bundle 的 Node fs.cp/copyfile 阶段出现
UNKNOWN -4094，未进入该例 50 路启动断言。原失败保留在
`execution-validation/g6/root-ci-c24b274-20261029.log`，随后同一源文件的独立
复制成功，OS 根因未确定。未修改的原专项复验通过（112.74s，含实际 50 路），
日志 `execution-validation/g6/standalone-c24b274-20261029.log`；之后使用已支持
的 D 盘 Cargo cache 完整重跑通过。没有更改 TEMP/TMP、时限、断言或门禁。

# 官方 FFmpeg 来源与受限静态构建

日期：2026-11-03。范围：按维护者要求自行取得官方来源材料，构建并验证
FFmpeg/ffprobe。G7 保持 in-progress / prototype，人工验收继续 waived。

[FFmpeg 官方下载页](https://ffmpeg.org/download.html) 发布源码并链接 Windows
二进制提供方。已下载官方链接的 BtbN 固定发行
`autobuild-2026-11-02-12-51`、独立 checksums.sha256 与 LGPL 8.1.3 ZIP。
ZIP 为 167779962 bytes，SHA-256 为
`d84ba8f0636333a242728908d8cdefa1496893cbc616c706a8379aa4570ced04`。
两份通用 exe 各约 126MiB，超过现有 32MiB program 预算；只作来源数据保留，
没有执行、安装或提高预算。未采用此前提供的其他应用内部二进制。

随后取得官方 `ffmpeg-8.1.3.tar.xz`、detached signature 和 release 公钥。
archive 为 11732036 bytes，SHA-256 为
`7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3`。
实际 GnuPG 验证得到 Good signature / VALIDSIG，精确 RSA 指纹
`FCF986EA15E6E293A5644F10B4322F04D67658D8`。它不同于原 8.1.2 Git tag
签名者；两条旧/新验证路径保留，互不替换。公共 keyring 独立，不改用户
GnuPG、系统信任或 Runtime DSSE pins。

## 构建与可复核材料

`download-official-media.ps1` 只取固定 HTTPS URL/哈希，限制实际流量与期限，
临时文件 create-only。未知已有文件保留并拒绝，最终移动不覆盖。固定锁、
源码签名、公钥、离线验证器与 `build-official.sh` 均纳入代码评审。

源码预检拒绝路径穿越、链接、特殊文件、大小写碰撞和超量展开，实际 10116
文件。已有树须逐文件原字节匹配。构建复用原 8.1.2 候选的固定 Opus/libvpx
源码与开发工具链；仅替换为官方签名的 8.1.3 FFmpeg 源码，原构建配方与
source/program pins 不变。新配方禁用 network/autodetect/shared/设备输入，
保留固定 file/pipe、WAV/WebM、VP9/Opus 能力。

新静态 pair：

| 程序    | bytes    | SHA-256                                                            |
| ------- | -------- | ------------------------------------------------------------------ |
| ffmpeg  | 10781184 | `36df0f37b9252cd8ec11a9c87caeda7c537e54ef75f59a5614aefecefc0e644e` |
| ffprobe | 10566144 | `60e8a872c8090a2c09ee1575c8c99b64b54b3c7af7c8da69158dab40c3fa7dfd` |

实际配置 GPL=0、NONFREE=0、NETWORK=0、LIBVPX=1、LIBOPUS=1。
四份原始许可证材料与 aggregate hash 单独固定。此构建由项目开发者完成，
不能称 FFmpeg 官方 Windows 二进制、独立安全批准或法律审核；完整 source
offer、工具链审阅及 [FFmpeg 法律材料](https://ffmpeg.org/legal.html) 仍须按
真实发行范围处理。跨机器/目录 bit reproducibility 尚未证明。

## 实际验证

新 `native_official.rs` 精确四项，使用真实 Core 模块和现有 test-only authority：

- 精确新旧 program pins、篡改/未知 role 拒绝；生产空 review 仍拒绝安装。
- 两个消费方共享一次实际 signed-fixture 安装，固定健康转换与独立 probe
  均验证真实 PID/Job 已排空，逐样本检查与当前授权保持。
- 实际 WAV 和 VP9 WebM 输出，独立 full-frame probe；未知时长保持未知。
- 独立 probe 恢复前撤权时拒绝并实际排空，不返回旧输出。

四项专项最终 4 pass / 0 fail / 0 ignored（30.06s），all-target Clippy
通过（9.25s）。初次新用例先收取失败再读取诊断，得到零条观察；改为在收取
前核对原有两个 PID/Job，没有放宽产品行为。首次失败日志保留。

四项实际 PowerShell 来源拒绝对照已通过：未知下载原字节保留；源码篡改；
即便测试副本锁的 SHA 改成篡改值，独立 PGP 仍拒绝；程序篡改。每项还核对
没有生成 fixture 输出。实际正向 GPGv 验证在持有并重新哈希签名输入期间
完成，精确程序/通知检查后才写独立 create-only fixture 输出。

离线源码 verifier 实际核对官方签名及全部 10116 文件，四项 hostile/source
回归在普通 Python 与 `-O` 下均通过（含 gzip/xz 两种展开）。下载脚本对
全部已缓存精确材料的复核通过；随后单独 HTTP 重取公钥时遇到 TLS EOF，
同 URL 的 .NET 与 Python 均失败。保留该网络失败，没有关闭 TLS 校验或
将失败写成通过；本次先前已完成的下载、固定哈希与签名材料仍可离线核验。

原 6/10/8/9/6/5/5/4/4 分组保持，加官方来源四项共 61 项。完整 driver、
当前提交根门禁与远端结果分别记录，专项通过不替代整仓通过。

完整 driver 首次在原六项的共享镜像释放正向对照失败：两个 durable retained
均已 false，真实进程/Job 已空，紧接着取得 writer 遇到 Windows error 32。
单项复验仍失败。临时诊断确认最后 held-image Arc strong count 为 1、释放后
本进程没有对应 file handle；诊断延迟后实际 writer 成功。确切外部持有者未
证实，不归因为杀毒软件或 Core 泄漏。最终保留两个消费方/真实 PID/Job/
lease 拒绝与实际 writer 正向断言，将最终取得 writer 限在原 500ms 排空窗口，
仅对 error 32 等待，其他错误直接失败；产品时限和产品代码未改。

随后最终完整 driver **exit 0**：全部 61 项均 0 fail / 0 ignored。原
6/10/8/9/6/5/5/4/4 与新增四项分别为
21.32/58.82/50.05/57.19/35.09/36.45/29.23/48.82/21.21/26.86s；
实际 inventory、原 probe、格式、all-target Clippy（6.46s）与四项来源拒绝
对照均通过。driver 契约专项 1 pass / 298 assertions、严格 TypeScript 与
PowerShell 解析通过。完整日志
`execution-validation/g6/official-8.1.3-full-driver-final-20261103.log`；首次失败与
单项诊断单独保留。当前修改仍需干净提交的根门禁与远端检查。

来源文件与完整构建/失败/成功日志保留在 ignored
`execution-validation/g6/vendor-downloads/`、`official-8.1.3-build/` 和
`official-*20261103.log`。源码和新 pair 现已具备可核验材料；正式发行身份、
独立审核、激活/LKG/GC、文件事务及两个媒体插件的用户输入/输出发布仍须
继续实现，见 [G7 矩阵](./g7-completion-matrix.md)。

# G7 Windows/T1 功能交付

日期：2026-11-04。维护者选择固定 Windows/T1 功能原型，人工验收 waived。
本页记录文件/媒体实际实现；当前整仓及远端结果在完成后补记，不借用旧提交的绿色。

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
  公开 fixture 私钥。私钥仅存开发者仓库外 DPAPI 文件，普通 build 不读私钥。
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
  通过。完整 Windows 根门禁及当前提交远端 CI 继续收尾，结果随后单独记录。

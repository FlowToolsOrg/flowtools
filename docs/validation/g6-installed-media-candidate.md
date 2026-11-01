# G6 实际签名 fixture、密封媒体候选与 LPAC

日期：2026-11-01。状态：prototype；实际六项专项通过，当前完整门禁另记。

P2.8b5 在独立、非发布的 `managed-media-candidate` Cargo workspace 中编译
实际 Core 源文件，使用仅 `cfg(test)` 存在的私有 fixture authority。未复制
或改写 Core 实现，没有产品 feature、Host selector、wire/Tauri command、
executable getter 或生产 review/pin。普通根 CI 不运行此专项；原独立
`media-candidate` 的六项及固定 candidate/source/program hashes 保持。

输入复用原媒体候选的精确 program hashes 和来源检查。四份来源通知
（FFmpeg LGPL、libvpx LICENSE/PATENTS、Opus COPYING）逐份与合并字节均
固定 hash；只生成忽略的测试输入，不表示完整 source offer、正式签名发行、
许可证法律批准或独立安全审核。签名封装使用公开确定性测试 key、fixture
publisher/source/build flavor，绝不能作为真实发布者身份或生产 trust pin。

测试先以实际签名 ZIP 安装和显式 materialize，随后让普通固定 T1 Core
接收任务、固定不可变依赖锁并持有实际文件。调用经过 `invoke_media_tool`
当前授权/任务原记录/签名字节/原 seal 检查，才在测试回调中启动实际安装
目录内的固定程序。路径由 fixture 自己的实际收据推导，没有产品路径接口。

复用原生 feasibility harness 的 suspended/LPAC/token/capability/Job 检查，
只增加固定 ffmpeg/ffprobe 角色与已有静态 recipe 的测试调用。原生回读
owned process 的实际 image path；程序不能换成 fixture 目录内的副本。
保留五秒工具输出、65536 字节聚合 stdout/stderr 与 500ms Job drain 界限。
原正向对照和拒绝选择器仍仅测试可见，未进入 Runtime 配置。

实际六项：

1. 精确真实候选仍被空 production review 拒绝，安装前没有工具池或任务行；
   错 hash/过短 image 先拒绝。
2. 两个固定消费方共享一份实际 signed ZIP/密封文件，以不同 LPAC SID
   执行真实 ffprobe；一方终态不能释放另一方仍持有的文件。
3. 实际安装 FFmpeg 提取 WAV；工具退出/Job 排空后打开新输出，拒绝
   reparse/hardlink/大小异常，独立 ffprobe 与完成投影联合验证。
4. 实际 WebM 转码与已有文件 `-n` 退出零拒绝；没有完整本次进度不能
   冒充成功，原输出字节保持。
5. 实际 artifact IO 后撤销 grant，不进入原生启动回调，保留原任务引用。
6. 当前签名根撤销实际已安装候选，不再进入回调，保留旧文件与根。

本地工作区先完成 `bun install` 与 `bun run build:packages`，再显式运行：

```powershell
$env:CARGO_TARGET_DIR='D:/code/projects/flowtools/apps/desktop/src-tauri/target'
pwsh -NoProfile -File scripts/test-managed-media-candidate.ps1 -BuildRoot <verified-build-root>
```

driver 要求实际 Core 构建输入、原固定候选和通知文件，缺失即失败；构建
原生探针后检查实际 `managed_media::` inventory 恰好六项，再运行 all-target
Clippy 与六项串行检查。不会运行此独立 crate 内为编译私有测试 authority
而同时收集的普通 Core 用例；这些用例仍在根门禁中按原方式完整执行。

driver 为探针与独立 crate 显式指定同一个 target root；采用调用方既有
`CARGO_TARGET_DIR`，未设置时仅使用此 fixture 自己的 target。不会修改
调用方环境，也不会把另一个 target 中的旧探针当作本次构建结果。

依赖契约首次失败，发现 `cargo generate-lockfile` 选择了主仓库之外的
registry 版本。改为沿用根锁并由 Windows metadata 更新本地 package
条目后，全部依赖版本/source/checksum 与主仓库吻合；新增契约 **1 pass /
0 fail**、232 个断言（66ms）。不放宽依赖检查。跨平台 `--offline` metadata
曾因未缓存的非 Windows `cfg_aliases` 下载失败；显式 Windows metadata
随后通过，没有增加包版本。严格 TypeScript 检查也通过。

此专项仍是测试回调中的同步工具调用；
产品原生 adapter、异步撤销/owned call accounting、用户输入选择与独立
输出发布/恢复事务、可播放交付、accepted 旧版本 continuation、更新/GC
及 G5/G6/G7 父项仍未完成。

首次 driver 编译在测试封装层失败：把 Core 放在子模块后，原 `pub(in
crate::packages::...)` 不能经由重导出成为真实 ancestor；另有测试归档字段
误用了 BlobDescriptor。改为在 crate 根 `include!` 实际 Core `lib.rs`，并
保留原归档类型、仅更新实际 size/hash。Core 私有可见性未改变。随后实际
编译通过（28.86s），`managed_media::` inventory 恰好六项；日志
`managed-media-candidate-native-20261101.log` 与
`managed-media-candidate-inventory-root-modules-20261101.log`。首次未运行测试。

随后 driver 的 all-target Clippy 因重复加载同一 feasibility harness/token
模块而失败，仍未运行原生六项。将 harness 整理为唯一的 cfg(test) 模块，
原密封测试和新专项复用同一模块；没有压制该 lint 或改变产品权限。日志
`managed-media-candidate-native-retry-root-modules-20261101.log` 保留。

修正后完整显式 driver exit 0：探针构建、格式、实际 inventory 恰好六项、
all-target Clippy `-D warnings`（17.70s）以及 **6 pass / 0 fail / 0 ignored**
（43.08s）。实际 owned image 回读、不同 LPAC SID、原五秒输出/65536 字节/
500ms Job drain、当前 Core grant/签名拒绝与原字节断言均通过。日志
`execution-validation/g6/managed-media-candidate-native-shared-harness-20261101.log`。
普通 Core 的其余 453 项在此 driver 中明确 filtered，仍由当前整仓根门禁
执行，不将此专项当作它们或产品异步 adapter 的通过证据。

随后原独立 `media-candidate` 显式 driver 的原六项全部通过（3.31s），其
实际 inventory、格式及 all-target Clippy 同样通过；原程序/来源 hashes、
LPAC/token/Job、PID、5s 输出与 500ms drain 界限保持。日志
`execution-validation/g6/managed-media-candidate-original-six-20261101.log` / exit 0。

共享模块兼容回归：原 Core image/恢复 **41 项全部通过**（246.69s），原
实际 owned service Job 排空一项通过（3.25s），Core all-target Clippy
`-D warnings` 通过（21.40s）。日志 `managed-media-candidate-core-images`、
`managed-media-candidate-core-drain`、`managed-media-candidate-core-clippy`，
日期均 `20261101`，均 exit 0；真实退出、权限/字节和全部时限断言保留。

实际源模块/依赖/产品图/严格六项/target 一致性边界契约最后通过，**1 pass /
0 fail**、238 个断言（82ms）。先前新增 image 回读检查的文本匹配因 rustfmt
换行失败，改为允许空白并检查必须先于 ResumeThread，未删除该检查；原
失败日志保留。最新日志 `managed-media-candidate-ci-contract-final-20261101.log`。
完整门禁仍须在最终干净提交上执行；这些专项不替代它。

主工作区初次 `bun test scripts/ci-contracts.test.ts` 还匹配了忽略目录中的
两个同名归档文件，实际 35 pass / 0 fail，但附加的“恰好十九项”包装检查
拒绝。改用 `./scripts/ci-contracts.test.ts` 后只收集当前十九项，其中现有
CSS lint 用例超过原 30s 时限（18 pass / 1 fail）。原样定向复验通过，
15.35s，未改时限/断言；首次超时的 OS/子进程根因尚未确定。日志分别为
`managed-media-candidate-ci-contracts-main-20261101.log`、
`managed-media-candidate-ci-contracts-main-exact-20261101.log` 与
`managed-media-candidate-css-lint-retry-20261101.log`。最终整仓结果另记。

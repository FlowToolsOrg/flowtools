# 开发 Host 严格签名检查耗时

日期：2026-11-02。范围：依赖的开发/测试构建设置，非 G7 功能完成或安全认证。

## 改动

Root、独立 Desktop、独立 managed-media-candidate 三个 Cargo root 只对
`ed25519-dalek` / `curve25519-dalek` 设置 opt-level 3，同时显式保留
`debug-assertions = true` 与 `overflow-checks = true`。原 SHA2 设置保持；
没有全局 dev/test/release 优化或关闭断言，没有 feature、依赖/lock、准入、
review/pins、预算、测试过滤或期限变化。普通产品 Host 仍无工具 authority。

CI 契约按完整 TOML 精确检查三个 profile，只允许这些 package overrides，
不能借此改为全局优化或移除调试/溢出检查；原 Node 两项契约及 5s/30s 保持。

## 测量与检查

相同八项 held-descriptor 严格签名/恢复/过期/撤销/源身份拒绝测试：当前基线
二进制 **8 pass**（6.29s），优化后相同八项 **8 pass**（0.98s）。这是单次
本机对比，包含环境负载差异，不外推为整仓或生产性能。日志
`execution-validation/g6/strict-signature-baseline-20261102.log`、
`strict-signature-optimized-initial-20261102.log`。

实际 verbose rustc 命令确认两个密码库 `opt-level=3` 和
`debug-assertions=on`，没有关闭 overflow 的参数。Cargo 配置明确要求 overflow
检查；此编译器在这些实际选项下，受 black_box 保护的 u8 加法溢出探针按预期
panic/exit 101，标明 attempt to add with overflow，而非关闭断言或静默回绕。
探针为忽略目录里的固定开发验证源码，不进入产品或 Cargo workspace。
日志 `execution-validation/g6/crypto-profile-overflow-20261102.log`。

严格签名边界聚焦 **35 pass / 0 fail**（0.37s）：包括 scalar malleability、
low-order R、错误 key 长度、阈值、弱 key/角色复用、签名编码与包路径拒绝。
这是聚焦选择，不能替代完整 Core 测试。日志
`execution-validation/g6/strict-signature-focused-rejections-20261102.log`。

优化配置下完整实际媒体 driver **exit 0**：原 6 pass（25.20s）、信息 10 pass
（66.82s）、WAV 8 pass（57.93s）、WebM 9 pass（60.81s）、续接 6 pass
（38.40s）、健康 5 pass（41.15s），均 0 fail / 0 ignored。原精确
6/10/8/9/6/5/44 inventory、原探针、格式和 all-target Clippy（20.87s）通过。
各测试组总时长从上一配置的 491.82s 到 290.31s，约减少 41%；准备、编译与
其他系统耗时未计入，不声明总 CI 加速比例。原 source/program hashes、
所有 PID/Job/令牌/时间/拒绝断言保持。日志
`execution-validation/g6/strict-signature-full-driver-20261102.log`。

当前完整根门禁和对应远端 CI 尚须独立运行。没有提升 G5/G6/G7 成熟度，
文件事务/撤销、媒体发布、版本激活与 GC/LKG 等工作仍未完成。

主工作区整合后 Runtime all-target `cargo check --locked` 通过（12.19s）；
完整二十项 CI 契约 **20 pass / 0 fail**（27.40s，636 assertions），包括三个
Cargo root 的完整 profile 精确检查。严格 TS 使用正常绝对 typeRoots，文档
契约通过。日志 `execution-validation/g6/signature-latency-main-focused-20261102.log`。
最终干净提交仍须完整根门禁，不将这些聚焦结果称为整仓或生产通过。

`e4ed1e6903953c782d346d14086c78835d7b9826` 随后完整本机根门禁 **exit 0**、
最终无漂移：Core 486+11+11（455.58s）、11/11 workspace、Runtime 24+6
（47.76s/2.02s）及 Bun 17、Chromium 113/35、Desktop 108 Bun+Node 2+
原生 37+6（4.27s/0.16s），Web 23/Desktop 25 一致性与 production refusal、
最终 Rust check/clippy（18.85s/12.05s）通过。与前一步同套 Core 的 1474.29s
相比，这次本机减少约 69%；不将单次环境对比称为稳定生产性能指标。
日志 `execution-validation/g6/root-ci-e4ed1e6-20261102.log`。已推送并以
ls-remote 核对相同 SHA；[PR 41424481274](https://github.com/FlowToolsOrg/flowtools/actions/runs/41424481274)
与 [push 41424473938](https://github.com/FlowToolsOrg/flowtools/actions/runs/41424473938)
在记录时仍运行，结果 pending；PR 保持 Draft。

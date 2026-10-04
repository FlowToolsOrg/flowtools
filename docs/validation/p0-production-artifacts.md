# P0.3b4 发布产物与跨入口门禁

- 日期：2026-10-04；P0.3b3 已独立提交为 fbbe988 后开始本项。
- 状态：done；本地实际产物与拒绝回归通过，远端干净 Windows checkout 完整门禁及最终无漂移复核通过。
- 范围：生产外部入口停用的构建证据；不是签名、sandbox、grant 或安全审批。

## 可复现命令

```powershell
bun run build
bun run verify:production-entrypoints
```

生产门禁固定检查实际 `apps/web-vite/dist` 与 `apps/desktop/dist`，要求非空
index.html 和 JavaScript。它不 import 或执行 artifact，拒绝 symlink/junction
与构建树外的路径，保留 package-relative 文件身份与实际字节 SHA-256。
默认运行先检查已有生产产物，再依次实际重建两端，不能传参数跳过比较。

重建子进程临时开启精确 opt-in=1，并注入固定 checkout path、`.invalid` preview
URL、两种 synthetic signing-key canary；不改变父进程/注册表，不读取实际密钥
内容作为 probe，实际同名 key 在 child 被 synthetic 值覆盖且不打印。
所有文件与基线逐字节一致才通过；当前 Web 21、Desktop 24，共 45 个文件通过。
危险开发模块、HTML bridge/event 指纹、/@fs、已知 certification switch 语法
与所有 canary 均未出现。完整构建过程保留错误，失败非零退出，不容忍漂移。

字符串文档中的 production-certified 不算授权。TypeScript AST 检查 identifier、
property、element access 中的已知 certified/productionCertified 等 switch；
这是保守的已知语法约束，不证明任意 renamed local/dataflow 安全。普通 SDK、
Web、Desktop 与 CLI 的副作用前拒绝仍由各自真实服务/compiled-entry 回归证明。

## 拒绝回归与 CI

新增 artifact 单元回归 17 项，另有 CI contracts 6 项。覆盖空输出、
缺入口/JS、零字节、八个危险模块/事件/import-map/CDN 指纹、四种伪认证语法、不同类型文件
中的三类 canary、实际字节/稳定 hash/只读行为和 redirected artifact。
旧 CI 的新 post-build-gate 断言实际失败（缺失 gate，索引 -1）；新 artifact
模块缺失也使测试失败。修复后全部通过，没有 --pass-with-no-tests 或重试。
补充的 import-map ID 与 esm.sh CDN 两项回归在补齐 marker 前实际失败，
随后拒绝覆盖扩展，不因只检查 loader 事件而漏掉独立 import-map 注入。

`scripts/check-ci.ps1` 在 root build 后通过 Invoke-QualityCommand fatal 执行
`verify:production-entrypoints`，最终 clean-worktree 检查仍包括这次重建漂移。
root lint 与 Desktop test/types 覆盖新脚本。没有改 Actions triggers、permissions、
cache、SHA、required-check 名称或保护规则；Turbo strict env 不变。

CI 的 root tests 同时保留 SDK 真实模式矩阵、Web 存储恢复拒绝、CLI 固定 compiled
inventory / 真实 run()、Desktop bridge/runner 六构建模式与持久状态不能授权的
入口证据。Web b1 和 Desktop b3 的实际前端/专用原生人工记录独立保留；本项
不重复启动原生应用、不操作用户 DB，也不把模拟 native IPC 当实窗验收。

## 干净检出复核记录

独立检出 `execution-validation/p0b4-clean-ci-20261004` 使用未移动工作分支的
测试快照 00c4221，未复制 node_modules、JS dist 或 Turbo results；所有构建
均 force、tests 保持 uncached，只复用允许的 Cargo native target。
2026-10-04 首轮 check-ci.ps1 在冻结依赖安装、任务/文档/catalog/browser
检查与 package bootstrap、Rust 绑定生成后，因 lint 子进程的
`Committing semi space failed` / exit 134 终止，fatal 传播保持有效。
同轮另一个实际产物复核在 Rust 重建时报 `rustc-LLVM ERROR: out of memory`。
现场 Windows committed bytes 84,222,685,184 / limit 87,440,084,992，
仅约 3 GiB commit 余量；这不是通过记录。已停止并行重建，等待资源释放后
串行复核，不修改断言、门禁、严格环境、timeout 或机器 pagefile 设置。
最新单独 artifact/CI contract 检查为 23 pass / 0 fail，工作流 actionlint
1.7.12 通过；完整干净 gate 与最终无漂移结论仍待验证。
用户随后明确要求停止内存分析、以后再处理。本轮不再分析机器资源或重跑
重型构建，保留实现与失败证据，不将 P0.3b4 或 Phase 0 标记为 done。

## 未验证与残余风险

2026-10-04 补充核对：合并 commit `966878d65ae3ecb599dd9ed660eb38f745a3a79e`
的 [Windows quality](https://github.com/FlowToolsOrg/flowtools/actions/runs/37189304639)
已完成且 conclusion=success；Run all quality gates 与 Verify clean worktree
均成功。该 workflow 调用现有 `check-ci.ps1`，包含 uncached tests、force
build、fatal production artifact gate、Rust fmt/check/clippy 与最终漂移检查。
它补齐本地因资源不足中断的自动化验收，不将那次本地失败改记为通过。
P0.3b4 与 P0.3b 据此关闭；P0.3c 的市场/权限文案仍独立待处理。

SHA 比较是同一机器/构建工具下的 opt-in 不变量，不是跨机器 reproducible
build、publisher provenance 或包签名。已知指纹/语法不认证不存在所有恶意
程序；ordinary service gate、隔离与 package broker 不能只依赖此扫描。
CLI compiled package 与 SDK development subpath 不是本扫描对象；分别由
CLI 48 项 compiled-entry 契约与 SDK 的显式 development guard 覆盖。
所有 maturity 仍 prototype，12 项 SEC 风险保持 open。远端 CI、保护配置与
独立安全 Reviewer 需单独验收，不能由本机成功或人工功能回报推断。

如果新门禁失败，修复泄漏/不确定性并重新构建，不绕过 gate、不删除历史
源码或测试数据；关闭开发 opt-in 不等于撤销用户 grant，后者尚未实现。

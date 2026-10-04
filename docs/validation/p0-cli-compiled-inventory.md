# P0.3b2 CLI compiled inventory 验收

- 日期：2026-10-04；Windows / Bun 1.3.14。
- 范围：固定 T1 清单与编译入口、CLI 失败输出；不是第三方签名安装/隔离。
- 状态：完成；七 workspace 全根 gate 与十二真实插件 smoke 已通过。

## 入口与拒绝证据

生成器在可信构建流程中同步 Web、Desktop 和 CLI 三个排序清单。CLI 将清单
内嵌于 dist；运行时不读取源码 metadata，也不扫描新增目录。未知 ID 在任何
文件访问前查表拒绝，路径、大小写变体、file URL 或布尔 certified/granted
字段不能扩大 inventory。只 import 固定 plugins/dist 下普通编译文件；目录
junction、symlink、身份/版本/maturity/type/run/schema 不一致均失败。
list 仅检查固定产物存在，不执行或认证它们；info/run/help 加载后验证一致性。

旧版 9 项 compiled CLI 回归中 8 项失败：源码 canary 被执行、坏 dist 回退源码、
公共 loadPlugin 接受任意路径、junction 重定向、源码-only list/info/help 假成功。
修复后这 9 项和新增 6 项 metadata/运行契约拒绝回归全部通过，CLI 共 48 tests。
测试将真实 CLI dist 复制到 node_modules/.tmp 的临时唯一项目，所有源码与坏
产物均是可丢弃 fixture；不修改实际内置源文件、真实编译产物或用户数据。
库存 presence fixture 不运行插件、不伪造真实内置成功；正向运行由实际十二
compiled 插件 smoke 与既有 CLI 子进程合约测试负责。

七 workspace lint/types/test/build 强制重跑、零 Turbo 缓存通过：SDK 86、CLI 48、
plugins 41、Web 22、Desktop Bun 25 + Rust 13、Chromium 59。单独十二真实 compiled
smoke 15 项通过；CLI text 的 hello 得到 aGVsbG8=，help 输出实际 schema flags。
更严格的新增目录自报 ID fixture 修改后，CLI 48 项又单独复核通过。
清单生成器最初为三输出连续启动格式化 CLI，Windows 全根测试触发原 30 秒预算；
改用同一进程内安装版本的 formatter API / 仓库配置后，冷检查 5.24 秒、根测试
185ms 通过。未放宽超时、跳过断言或把重试当作验收。构建仍有大 chunk 等性能
提示，未把这些提示作为性能/发布承诺。

## 复现

```powershell
bun run scripts/generate-manifests.ts --check
bun run docs:check
bun run build:packages --force
bun run --cwd packages/cli test
bun run smoke:plugins
bun run lint --force
bun run check-types --force
bun run test --force
bun run build --force
```

根 test 和 build 必须顺序执行。CLI 测试自行构建自己的 dist，不依赖 plugins
workspace 来构建 CLI（避免循环依赖）；实际插件 smoke 先构建所有前置产物。
缺产物时 JSON 命令输出 LOAD_FAILED envelope 到 stdout，并非零退出；text/help
输出稳定失败码和重建提示。原有 generated flags / JSON envelope / text 行为保留。

## 残余风险与恢复

这是默认停用源码入口与 T1 构建一致性，不是完整包准入。可写入可信 compiled
artifact 的对手仍可能在 import 时执行代码；metadata 校验发生在该 T1 import
之后，不消除替换文件或 TOCTOU。T3 OS runner、签名/完整性/发布者、持久 grant
及独立安全评审仍未实现，SEC-001/002 保持 open；Desktop/b4 与 P0.3c 仍待验收。
本项不持久化授权、不删除或迁移插件源码；产物缺失通过重新构建恢复，无法
用 opt-in、认证字段或旧源码降级恢复生产执行。UI 人工验收不等于独立安全批准。

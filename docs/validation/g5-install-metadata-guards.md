# G5/G6：安装元数据写入与 journal 清除保护

日期：2026-10-31。状态：in-progress / prototype。

后续 accepted artifact leases 需要依赖现有安装库存。检查三个旧安装路径时
发现，最终库存 INSERT 的 SQL 触发器可以替换 pending，随后原 DELETE
会将替换内容抹去。a7 已单独保护其消费方 journal，旧插件、raw 工具 ZIP
和密封文件路径仍需要补齐。另一个实际 SQLite 观察表明：BEFORE DELETE
更新同一行后，DELETE 仍会移除此行，RETURNING 返回旧值；因此最终
记录缺失和 affected row count 都不足以证明删除了原 journal。
SQLite [明确警告此类 BEFORE 触发器行为未定义](https://sqlite.org/lang_createtrigger.html)，
实现不依赖它一定返回哪一版内容，而是对额外写入拒绝。

`install/metadata.rs` 使用内部有限固定槽位，复用原 DataStore 单写者
事务，保存原 JSON 字节与格式。每个 upsert/delete 只允许一个实际行变更，
同时比较 affected rows 和含触发器效果的 `total_changes` 差值；额外写入、
替换后恢复和替换后删除都拒绝。清除还绑定实际旧 TEXT 字节，拒绝未知
key、非 pending 槽位删除、过大/非 TEXT 记录、缺失 journal 和异常回读。
调用方原有信任、库存、opaque journal、SQL 前后及原生效果检查继续保留。
依据 [total_changes 计数契约](https://sqlite.org/c3ref/total_changes.html)，计数只
覆盖当前连接，不能替代事务与 writer ownership；这里仍由原 IMMEDIATE
事务排除其他写者。普通表的 upsert/delete 使用原 affected rows 和实际字节
回读共同验证，不把 view/INSTEAD OF 或 REPLACE 行为作为接受途径。

本步骤只覆盖安装/恢复元数据；不创建工具执行身份、grant、lease、GC、
新数据库或客户端选项。失败由原事务回滚；已发布或隔离的文件继续由原
显式 abort/reconciliation 路径保留并恢复管理。

## 验证状态

旧实现加相同新用例的独立工作区实际为 **0 pass / 6 fail**（6.61s），三个
原生安装域均复现库存触发器替换 prepared journal，以及 BEFORE DELETE
替换后仍成功清除的缺口；不是只靠代码推断。日志
`execution-validation/g6/install-metadata-red-retry-20261031.log`，exit 101。

修复后相同六项全部通过（17.90s），其中 commit/abort 的 BEFORE/AFTER
DELETE 四种组合分别检查原 journal、库存、信任和实际保留字节。另五项
真实 SQLite 测试通过（0.09s），覆盖十六个固定槽位、隐藏后恢复、IGNORE、
重新插入、原字节、格式、边界、非 TEXT、未知 key 和已结束事务拒绝。
日志 `install-metadata-green-retry-20261031.log` 与
`install-metadata-slots-20261031.log`，均 exit 0。完整门禁仍须实际完成。

既有四项 SQL trigger 专项随后全部通过（47.98s），涵盖 raw/image 库存与
恢复 receipt/journal/trust 替换；日志 `install-metadata-existing-sql-20261031.log`。
Core all-target Clippy `-D warnings` 通过（13.39s），实际 Rust inventory 已
核对为 406 项，含六项新原生和五项固定槽位用例。Rust fmt、docs:check 与
文档格式通过；不将这些结果计为新提交整仓或远端成功。

随后 `060f235` 完整本机根门禁 exit 0，最终工作区无漂移：11/11 workspace
lint/types/test/build；Core 406+11+9（单元 976.02s），Runtime 原生 24+6/Bun
17（含原 50 路冷启动）、Chromium 109/34、Desktop 106 Bun+Node 1+原生 36+6，
Web 23/Desktop 25 产物一致、production refusal 及最终 Rust 检查通过。
日志 `execution-validation/g6/root-ci-060f235-20261031.log`。远端 ref 已核对为
`060f235d2dace1a45d20ea88b7abb78d361e2e5d`；
[PR 41196464140](https://github.com/FlowToolsOrg/flowtools/actions/runs/41196464140)
与 [push 41196459042](https://github.com/FlowToolsOrg/flowtools/actions/runs/41196459042)
均已完成且结论为 success；这是本 SHA 的远端完整门禁结果。

复现工作区首次编译缺少生成的 `manifest-validation.json`，没有执行测试；
`install-metadata-red-20261031.log` 保留该失败。补齐与当前通过门禁的
同源产物并核对 SHA 后才得到上述六项实际失败。第一次修复试跑还复用了
共享 target 中的旧工作区测试 binary：编译仅 0.48s、仍为旧总数 401，六项
再次失败；原 `install-metadata-green-20261031.log` 保留，不计为修复验证。
随后保持源字节不变、更新当前 crate entry 的 mtime 强制实际重新编译，
日志明确显示修复工作区（34.40s），当前总数为 406，包含新增六项原生和
五项 SQLite 测试。没有清缓存、换断言、删失败或放宽期限。

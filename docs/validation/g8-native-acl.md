# G8 P2.7d：原生命令 ACL 与显式权限

日期：2026-11-04。范围为现有 Windows/T1 Desktop，不开放第三方执行。

## 复现与修复

修复前，实际 Tauri MockRuntime IPC 的无害 callback 对照失败：非主窗口
调用应用命令也能进入处理器。原生 Runtime 命令自身有 label/origin 检查，
但应用命令整体尚未使用 Tauri AppManifest ACL。

build.rs 现使用官方 AppManifest.commands 声明 13 个已有命令并生成权限。
普通主窗口仅授予七个元数据命令和四个 managed Runtime 命令；验证命令仅
由已有固定身份的 runtime-validation 配置显式增加，原 Debug/identity 检查
保留。没有增加 handler、DTO、Host selector 或执行 grant。

普通 capability 不再使用任何插件/core 的 `:default` 集合：

- app 只读版本、event listen/unlisten、资源 close；删除未使用的 path/window/
  webview/image 默认集合。
- opener 仅 HTTP(S) URL，不授予 mailto/tel、文件 reveal；dialog 仅 open/save。
- notification 仅查询/请求权限和 notify，不授予 channel/batch 等管理接口。
- store 保留现用 load/get-store/get/set/delete/save，不授予 clear/reset/reload。
- FS 仅保留文本读取命令与原 deny-default，删除递归 app 目录和 mkdir 授权。
  没有静态路径 allow；用户通过 dialog 显式选择产生的原动态 scope 仍由
  官方插件管理。未新增写入、raw SQL、clipboard 或 shell 权限。

官方生成的 13 个 permission TOML 首次纳入版本控制并强制 LF；后续原生构建
的字节漂移仍由干净工作区门禁发现，不将整个目录忽略。Windows CRLF fixture
同时覆盖该新来源，不能靠 stage 刷新绕过验证。

## 实际回归

- 修复前 ACL 回归失败；修复后 7 项 native bindings 集成全部通过。
- 实际 Tauri ACL 的主窗口/local 正对照进入无害 callback；外部窗口、远程
  来源和清单外命令负对照均在 callback 前拒绝，计数保持 0，无 SQLite/Host/
  用户 profile 或 OS 业务效果。此对照验证 ACL，不冒充元数据 CRUD 业务验收。
- 既有 Runtime 原生 origin/label/initialize 六条拒绝断言不变。为继续独立
  检查原生 guard，该测试只对 external 测试窗口临时授予 control ACL；该
  capability 只存在集成测试中，不写入产品配置。
- 既有 shell IPC 拒绝测试扩展覆盖 reveal、store clear/reset/reload、通知
  channel、原始 FS 读写、clipboard 和 SQL；实际响应必须为 not allowed。
- 两项配置契约拒绝 default 集合、remote/validation/启动等额外授权，并检查
  验证配置仍使用固定身份和单独权限。
- Desktop 38 项单元与 7 项集成、23 项 CI/权限配置契约通过；Desktop
  lint/check-types 与原生全目标 Clippy（-D warnings）通过。

参考：[Tauri permissions](https://v2.tauri.app/security/permissions/)、
[AppManifest](https://docs.rs/tauri-build/latest/tauri_build/struct.AppManifest.html)。

主窗口仍是合作 T1，同 realm 代码可使用其已有能力；store load 等现有接口
不构成逐插件路径隔离。AppManifest/显式权限不代替 Runtime grant、包签名、
独立审核或平台沙箱；SEC-003/004/006/007/011/012 保持 open。生产 profiles
与市场/发行批准不变，P2.7/G8 仍 pending。

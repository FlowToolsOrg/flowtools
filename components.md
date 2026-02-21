# UI Components Roadmap

## 0. 约束与目标

- 目标模块：`工具市场`、`工具详情`、`运行面板`、`设置中心`
- 组件形态：优先 `compound component`（组合式 API）
- 当前阶段：先做 `纯展示组件`（逻辑通过 props/callback 注入，不内置业务状态机）
- 视觉方案：使用 HeroUI 默认 theme/color，不额外自定义品牌主题
- 命名规范：文件夹和文件名全部 `kebab-case`

## 1. 设计原则

- 可组合：每个业务块拆成 `Root + Slot` 子组件，避免单体大组件
- 可替换：尽量让输入/输出通过 props 定义，避免耦合 SDK/runtime
- 可测试：每个组件至少 2 条用例（1 渲染 + 1 交互）
- 可评审：所有新类型定义处加注释 `// TODO(review): waiting code review`

## 2. 模块与组件清单

## 2.1 工具市场 (`tool-market`)

- `tool-market-page`
  - `ToolMarketPage.Root`
  - `ToolMarketPage.Header`
  - `ToolMarketPage.Content`
- `market-toolbar`
  - `MarketToolbar.Root`
  - `MarketToolbar.Search`
  - `MarketToolbar.Filters`
  - `MarketToolbar.Actions`
- `tool-grid`
  - `ToolGrid.Root`
  - `ToolGrid.Item`
- `tool-card`
  - `ToolCard.Root`
  - `ToolCard.Header`
  - `ToolCard.Meta`
  - `ToolCard.Tags`
  - `ToolCard.Actions`
- `market-empty-state`

## 2.2 工具详情 (`tool-detail`)

- `tool-detail-page`
  - `ToolDetailPage.Root`
  - `ToolDetailPage.Header`
  - `ToolDetailPage.Content`
- `tool-summary-card`
- `tool-permission-list`
  - `ToolPermissionList.Root`
  - `ToolPermissionList.Item`
- `tool-version-timeline`
- `tool-related-list`

## 2.3 运行面板 (`run-panel`)

- `run-panel`
  - `RunPanel.Root`
  - `RunPanel.Header`
  - `RunPanel.Content`
  - `RunPanel.Footer`
- `run-input-panel`
- `run-result-panel`
- `run-log-list`
- `run-status-strip`
- `run-history-panel`

## 2.4 设置中心 (`settings-center`)

- `settings-center-page`
  - `SettingsCenterPage.Root`
  - `SettingsCenterPage.Nav`
  - `SettingsCenterPage.Content`
- `settings-group-card`
- `settings-item`
  - `SettingsItem.Root`
  - `SettingsItem.Label`
  - `SettingsItem.Description`
  - `SettingsItem.Control`
- `settings-switch-field`
- `settings-select-field`
- `settings-input-field`

## 3. 数据模型草案（首版）

> 具体类型放到 `packages/ui/src/components/**/types.ts`，并在类型顶部标记 `// TODO(review): waiting code review`

- `tool-entity`
  - `id`
  - `name`
  - `description`
  - `status`
  - `version`
  - `tags`
  - `permissions`
  - `is-installed`
  - `is-pinned`
- `tool-run-record`
  - `id`
  - `started-at`
  - `duration-ms`
  - `status`
  - `summary`
- `setting-entity`
  - `id`
  - `group`
  - `type` (`switch | select | input`)
  - `label`
  - `description`
  - `value`
  - `options` (仅 select)

## 4. 实施顺序（按依赖关系）

1. `settings-center`（依赖最少，先沉淀 field 组件）
2. `tool-card` / `tool-grid`（市场核心展示单元）
3. `tool-market-page` / `market-toolbar` / `market-empty-state`
4. `tool-detail` 全套组件
5. `run-panel` 全套组件
6. `apps/ui-test` 页面集成与回归

## 5. 开发节奏（必须执行）

每个组件组严格按下面流程推进，并在中间频繁 commit：

1. 写完组件（实现）
2. 写该组件测试（至少 1 渲染 + 1 交互）
3. 运行该组件相关测试
4. commit 当前组件组

示例（固定模板）：

1. 写完 `settings-*` -> 测试 `settings-*` -> commit `settings-*`
2. 写完 `tool-card` -> 测试 `tool-card` -> commit `tool-card`
3. 写完 `tool-grid/toolbar` -> 测试 -> commit
4. 写完 `tool-detail-*` -> 测试 -> commit
5. 写完 `run-panel-*` -> 测试 -> commit

## 6. 测试文件组织

- 组件测试与实现同目录放置：
  - `packages/ui/src/components/<component>/<component>.test.tsx`
- 测试命名：
  - `renders ...`
  - `calls ... when ...`
- 集成测试：
  - `apps/ui-test/src/test/*.test.tsx` 仅验证跨组件组合

## 7. 本轮提交要求

- 第一笔提交只包含：`components.md`
- 后续提交按第 5 节节奏执行，不混入无关改动

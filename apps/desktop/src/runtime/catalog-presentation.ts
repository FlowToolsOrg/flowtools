export type CatalogStage = 'available' | 'downloaded' | 'installed' | 'enabled'

/** Persisted legacy installation labels describe metadata, never package admission. */
export function catalogPresentation(
  source: 'react' | 'html',
  stage: CatalogStage,
  hasUi: boolean
) {
  if (source === 'html')
    return {
      stageLabel: stage === 'available' ? '仅目录记录' : '旧配置记录',
      primaryLabel: '不可安装',
      canExecute: false,
    }
  const stages = {
    available: '内置可用',
    downloaded: '配置待保存',
    installed: '配置已保存',
    enabled: '配置已启用',
  }
  const actions = {
    available: '保存配置',
    downloaded: '保存配置',
    installed: hasUi ? '启用并打开' : '启用配置',
    enabled: hasUi ? '打开' : '运行',
  }
  return {
    stageLabel: stages[stage],
    primaryLabel: actions[stage],
    canExecute: true,
  }
}

export const permissionPresentation = [
  ['storage', '当前为宿主命名空间存储；隔离与持久授权尚未实现', '授权未实现'],
  ['network', '内置请求适配；域名与操作授权尚未实现', '授权未实现'],
  ['fs', '宿主原生文件适配；路径授权尚未实现', '授权未实现'],
  ['clipboard', '宿主剪贴板适配；插件级授权尚未实现', '授权未实现'],
  ['native', '宿主原生能力；第三方原始调用保持拒绝', '高风险'],
  ['preload', 'HTML/Legacy 普通桥接与生产执行保持拒绝', '默认拒绝'],
] as const

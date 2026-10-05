import type { ErrorCode } from './bindings'

import { RuntimeClientError } from './client'

const explanations = {
  ACCEPTANCE_UNKNOWN: [
    '任务提交是否完成尚未确认',
    '保留原幂等键并查询回执；不要用新键重试写入。',
  ],
  EXECUTION_INTERRUPTED: [
    '执行进程在完成确认前中断',
    '检查实际数据或副作用，再主动决定是否发起新任务。',
  ],
  RESULT_EXPIRED: [
    '私有任务结果已超过保留时间',
    '任务元数据仍保留；检查原业务数据，不自动重放。',
  ],
  COLD_START_DENIED: [
    '当前策略未允许冷启动 Runtime',
    '通过交互式初始化或主动导入策略配置冷启动；不自动打开 GUI。',
  ],
  REVISION_CONFLICT: [
    '共享数据已被其他客户端修改',
    '读取最新 revision，检查冲突后重新操作。',
  ],
  STORE_BUSY: [
    '当前数据 profile 已有写入者',
    '连接现有 Runtime；不另建或删除数据库。',
  ],
  STORE_CORRUPT: ['数据库完整性检查失败', '保留原数据，从已验证备份主动恢复。'],
  SCHEMA_UNSUPPORTED: [
    '数据库版本不兼容',
    '使用兼容版本或已验证的升级前备份。',
  ],
  STORAGE_FAILED: [
    '共享数据写入或恢复失败',
    '保留原数据，检查权限和可用空间后重试。',
  ],
  PROTOCOL_MISMATCH: [
    '通信协议版本不匹配',
    '安装与 Runtime 匹配的客户端版本。',
  ],
  CLIENT_INCOMPATIBLE: ['客户端版本不兼容', '更新客户端后重新连接。'],
  FRAME_TOO_LARGE: ['请求超过传输大小限制', '减少本次输入内容。'],
  INVALID_REQUEST: [
    '请求格式不符合契约',
    '检查命令参数，不添加身份或执行路径。',
  ],
  SESSION_INVALID: [
    '会话无效或操作不属于当前调用方',
    '重新连接；只取消自己提交的任务。',
  ],
  INSTANCE_MISMATCH: [
    'Runtime 已更换实例',
    '旧任务没有恢复证据，确认状态后再决定下一步。',
  ],
  APPROVAL_REQUIRED: [
    '此操作需要授权',
    '主动导入与当前包和调用方匹配的授权；验证模式只允许纯内置命令。',
  ],
  INTERACTION_REQUIRED: ['此命令需要界面交互', '选择支持 headless 的命令。'],
  CAPABILITY_UNDECLARED: [
    '命令未声明此能力操作',
    '检查命令效果与 operation 声明，声明不能替代授权。',
  ],
  SCOPE_DENIED: ['参数超出授权范围', '使用已批准的资源，或主动申请新的范围。'],
  GRANT_REVOKED: [
    '授权已撤销或过期',
    '查询任务结果；重新批准后主动发起新任务。',
  ],
  BUDGET_EXCEEDED: ['操作超过资源预算', '减少调用次数或使用获准的资源预算。'],
  PLUGIN_NOT_FOUND: [
    '插件或命令不在固定清单中',
    '检查插件 ID 和命令 ID，并构建内置插件。',
  ],
  INPUT_INVALID: ['输入未通过命令校验', '按照命令 schema 修正输入。'],
  JOB_NOT_FOUND: [
    '当前 Runtime 中没有此任务',
    '检查 runId 和 Runtime 实例，不自动重新提交。',
  ],
  IDEMPOTENCY_CONFLICT: [
    '相同幂等键对应的请求发生变化',
    '查询原任务；重连时保留原始键、参数和 deadline。',
  ],
  TIMEOUT: ['任务超过截止时间', '查询终态；确认结果后再主动发起新任务。'],
  ABORTED: ['任务或等待已取消', '查询任务终态，确认是否需要继续。'],
  EXECUTION_FAILED: ['受管执行失败', '检查内置构建产物后查询任务状态。'],
  OUTPUT_INVALID: ['执行输出未通过契约校验', '检查插件输出与资源大小预算。'],
  RUNTIME_BUSY: [
    'Runtime 达到连接或任务上限',
    '释放本次验证会话或启动新的可丢弃 profile。',
  ],
  RUNTIME_DISCONNECTED: [
    '与 Runtime 的连接已断开',
    '重连同一实例并查询 runId，保留原始幂等键。',
  ],
  INVALID_RESPONSE: [
    'Runtime 响应未通过契约校验',
    '核对客户端/Runtime 版本，并查询已接收任务。',
  ],
} satisfies Record<ErrorCode, readonly [string, string]>

/** Exportable allowlist only: never spread exception messages, inputs or results. */
export function describeRuntimeError(error: unknown) {
  const code =
    error instanceof RuntimeClientError ? error.code : 'EXECUTION_FAILED'
  const [summary, action] = explanations[code]
  return {
    formatVersion: 1 as const,
    code,
    summary,
    action,
    acceptanceUnknown:
      error instanceof RuntimeClientError && error.acceptanceUnknown,
  }
}

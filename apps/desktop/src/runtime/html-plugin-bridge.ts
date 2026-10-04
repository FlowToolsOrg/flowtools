import type {
  HtmlPluginBridgeCommand,
  HtmlPluginBridgeRequest,
} from './development-html-plugin-bridge'
import type { PluginRuntimeContextValue } from '@flowtools/sdk/types'

import { ExternalCodeDisabledError } from '@flowtools/sdk'

export type {
  HtmlPluginBridgeCommand,
  HtmlPluginBridgeRequest,
  HtmlPluginBridgeResponse,
} from './development-html-plugin-bridge'

/** Ordinary host API is deny-only, even when the caller claims development. */
export function createHtmlPluginBridgeScript(
  _command: HtmlPluginBridgeCommand
): never {
  throw new ExternalCodeDisabledError()
}

export function injectHtmlPluginBridge(
  _html: string,
  _command: HtmlPluginBridgeCommand,
  _baseUrl?: string
): never {
  throw new ExternalCodeDisabledError()
}

export async function handleHtmlPluginBridgeRequest(
  _context: PluginRuntimeContextValue,
  _request: HtmlPluginBridgeRequest
): Promise<never> {
  throw new ExternalCodeDisabledError()
}

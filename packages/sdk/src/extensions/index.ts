export {
  AppearanceError,
  createAppearanceResolver,
  parseAppearanceOverrides,
  parseAppearanceTheme,
} from './appearance'
export type {
  AppearanceColor,
  AppearanceColorRole,
  AppearanceDefaults,
  AppearanceErrorCode,
  AppearanceMode,
  AppearanceOverrides,
  AppearanceRequest,
  AppearanceShadow,
  AppearanceTheme,
  AppearanceTokenPatch,
  AppearanceTokens,
  ResolvedAppearance,
} from './appearance'
export {
  extensionContributionLimits,
  parseExtensionContributions,
} from './schema'
export { ExtensionContributionRegistry } from './registry'
export { projectPluginContributions } from './projection'
export { ExtensionContributionError } from './types'
export type {
  ExtensionContribution,
  ExtensionContributionDocument,
  ExtensionContributionErrorCode,
  ExtensionContributionKind,
  ExtensionContributionOwner,
  ExtensionContributionSnapshot,
  ExtensionContributionValue,
  RegisteredExtensionContribution,
} from './types'

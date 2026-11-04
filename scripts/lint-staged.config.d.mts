export function stagedCommands(
  files: readonly string[],
  source: boolean
): string[]
declare const config: Record<string, (files: readonly string[]) => string[]>
export default config

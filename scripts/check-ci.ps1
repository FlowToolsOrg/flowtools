$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ci-gates.ps1')

$qualityRepositoryRoot = Split-Path $PSScriptRoot -Parent
Push-Location -LiteralPath $qualityRepositoryRoot
try {
  Assert-CleanWorktree
  Invoke-QualityCommand 'Frozen dependency install' bun @(
    'install', '--frozen-lockfile'
  )
  Invoke-QualityCommand 'Workspace task contracts' bun @(
    'run', 'verify:workspace-tasks'
  )
  Invoke-QualityCommand 'Documentation and threat-model contracts' bun @(
    'run', 'docs:check'
  )
  Invoke-QualityCommand 'Portable plugin catalog contracts' bun @(
    'run', 'verify:plugin-catalog'
  )
  Invoke-QualityCommand 'Pinned Chromium install' bun @(
    'run', '--cwd', 'apps/ui-test', 'test:install-browser'
  )
  Invoke-QualityCommand 'Build package prerequisites' bun @(
    'run', 'build:packages', '--force'
  )
  Invoke-QualityCommand 'Generate host routes and Rust bindings' bun @(
    'run', 'generate:hosts'
  )
  Invoke-QualityCommand 'Generated Runtime contracts' bun @(
    'run', 'verify:runtime-contracts'
  )
  Invoke-QualityCommand 'Lint all workspaces' bun @(
    'run', 'lint', '--force'
  )
  Invoke-QualityCommand 'Type-check all workspaces' bun @(
    'run', 'check-types', '--force'
  )
  Invoke-QualityCommand 'Test all workspaces' bun @('run', 'test')
  Invoke-QualityCommand 'Build all workspaces' bun @(
    'run', 'build', '--force'
  )
  Invoke-QualityCommand 'Production external-entrypoint artifacts' bun @(
    'run', 'verify:production-entrypoints'
  )
  Invoke-QualityCommand 'Rust format' cargo @(
    'fmt', '--manifest-path', 'apps/desktop/src-tauri/Cargo.toml', '--', '--check'
  )
  Invoke-QualityCommand 'Rust locked check' cargo @(
    'check', '--locked', '--manifest-path', 'apps/desktop/src-tauri/Cargo.toml'
  )
  Invoke-QualityCommand 'Rust clippy' cargo @(
    'clippy', '--locked', '--all-targets', '--features', 'codegen',
    '--manifest-path', 'apps/desktop/src-tauri/Cargo.toml', '--', '-D', 'warnings'
  )
  Assert-CleanWorktree
} finally {
  Pop-Location
}

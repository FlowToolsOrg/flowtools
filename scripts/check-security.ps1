$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ci-gates.ps1')
. (Join-Path $PSScriptRoot 'security-tools.ps1')

$securityRoot = Split-Path $PSScriptRoot -Parent
Push-Location -LiteralPath $securityRoot
try {
  $shallow = & git rev-parse --is-shallow-repository
  if ($LASTEXITCODE -ne 0 -or $shallow -cne 'false') {
    throw 'Secret scanning requires a complete Git history (fetch-depth: 0)'
  }
  $gitleaks = Get-SecurityTool gitleaks
  Invoke-QualityCommand 'Full-history secret scan (redacted)' $gitleaks @(
    'git', '--redact=100', '--no-banner', '--ignore-gitleaks-allow',
    '--config', '.gitleaks.toml', '--gitleaks-ignore-path', '.gitleaksignore',
    '--log-opts=--all --full-history', '.'
  )
  Invoke-QualityCommand 'JavaScript dependency audit (all severities)' bun @('audit')
  $audit = Get-SecurityTool cargo-audit
  # All four independent lockfiles, including both explicit native fixtures.
  # No platform/severity filters, advisory ignores, stale DB or offline fallback.
  foreach ($lockfile in @(
    'Cargo.lock',
    'apps/desktop/src-tauri/Cargo.lock',
    'apps/runtime/test/fixtures/media-candidate/Cargo.lock',
    'apps/runtime/test/fixtures/managed-media-candidate/Cargo.lock'
  )) {
    Invoke-QualityCommand -RejectErrorOutput "Rust dependency audit: $lockfile" $audit @(
      'audit', '--file', $lockfile
    )
  }
  Invoke-QualityCommand 'Actual security-scanner refusal controls' pwsh @(
    '-NoProfile', '-File', (Join-Path $PSScriptRoot 'test-security-scanners.ps1')
  )
} finally {
  Pop-Location
}

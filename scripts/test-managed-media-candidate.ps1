param([Parameter(Mandatory)][string]$BuildRoot)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ci-gates.ps1')
$managedRepository = Split-Path $PSScriptRoot -Parent
Push-Location -LiteralPath $managedRepository
try {
  & (Join-Path $PSScriptRoot 'prepare-managed-media-candidate.ps1') -BuildRoot $BuildRoot
  foreach ($managedInput in @('packages/runtime-core/.generated/catalog.json', 'packages/runtime-core/.generated/manifest-validation.json')) {
    if (-not (Test-Path -LiteralPath $managedInput -PathType Leaf)) { throw 'Build-owned Core inputs required; run bun run build:packages first' }
  }
  $managedManifest = 'apps/runtime/test/fixtures/managed-media-candidate/Cargo.toml'
  # Both independent manifests must use the same target root: Fixture::new finds
  # the actual probe beside the test binary, even without a caller target env.
  $managedTargetRoot = if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) { Join-Path $managedRepository 'apps/runtime/test/fixtures/managed-media-candidate/target' } else { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) }
  Invoke-QualityCommand 'Managed candidate isolation probe' cargo @('build', '--locked', '--manifest-path', 'apps/runtime/test/fixtures/isolation-probe/Cargo.toml', '--target-dir', $managedTargetRoot)
  Invoke-QualityCommand 'Managed candidate formatting' cargo @('fmt', '--manifest-path', $managedManifest, '--', '--check')
  # Core test source modules compile for private test authority, but their suites
  # remain in normal Core CI. Keep the original six and native runner ten
  # independently counted; no ignored, missing or unexpected test is acceptance.
  $managedInventory = & cargo test --locked --manifest-path $managedManifest --target-dir $managedTargetRoot --lib managed_media:: -- --list
  if ($LASTEXITCODE -ne 0) { throw "Managed candidate inventory failed: $LASTEXITCODE" }
  $managedCount = @($managedInventory | Where-Object { $_ -match '^managed_media::[^:]+: test$' }).Count
  if ($managedCount -ne 6) { throw "Managed candidate test inventory mismatch: expected 6, collected $managedCount" }
  $nativeCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_inspection::[^:]+: test$' }).Count
  $allManagedCount = @($managedInventory | Where-Object { $_ -match '^managed_media::.*: test$' }).Count
  if ($nativeCount -ne 10 -or $allManagedCount -ne 16) { throw "Native inspection inventory mismatch: expected 10 and total 16, collected $nativeCount and $allManagedCount" }
  Write-Output 'Verified exactly 6 managed real-media tests.'
  Invoke-QualityCommand 'Managed candidate all-target Clippy' cargo @('clippy', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--all-targets', '--', '-D', 'warnings')
  Invoke-QualityCommand 'Six signed installed-image LPAC checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::', '--', '--test-threads=1', '--skip', 'managed_media::native_inspection::')
  Invoke-QualityCommand 'Ten Core-owned native inspection checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_inspection::', '--', '--test-threads=1')
} finally { Pop-Location }

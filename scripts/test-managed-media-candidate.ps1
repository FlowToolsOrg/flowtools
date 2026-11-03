param([Parameter(Mandatory)][string]$BuildRoot, [Parameter(Mandatory)][string]$OfficialBuildRoot)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ci-gates.ps1')
$managedRepository = Split-Path $PSScriptRoot -Parent
Push-Location -LiteralPath $managedRepository
try {
  & (Join-Path $PSScriptRoot 'prepare-managed-media-candidate.ps1') -BuildRoot $BuildRoot
  & (Join-Path $PSScriptRoot 'prepare-official-media-candidate.ps1') -BuildRoot $OfficialBuildRoot
  Invoke-QualityCommand 'Four official source refusal controls' pwsh @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'test-official-media-source.ps1'), '-BuildRoot', $OfficialBuildRoot)
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
  # remain in normal Core CI. Keep original six, inspection ten, audio eight, video nine, live continuation six, fixed health five, progress five, cohort four, queue four, official-source four and explicit queue-discard six
  # independently counted; no ignored, missing or unexpected test is acceptance.
  $managedInventory = & cargo test --locked --manifest-path $managedManifest --target-dir $managedTargetRoot --lib managed_media:: -- --list
  if ($LASTEXITCODE -ne 0) { throw "Managed candidate inventory failed: $LASTEXITCODE" }
  $managedCount = @($managedInventory | Where-Object { $_ -match '^managed_media::[^:]+: test$' }).Count
  if ($managedCount -ne 6) { throw "Managed candidate test inventory mismatch: expected 6, collected $managedCount" }
  $nativeCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_inspection::[^:]+: test$' }).Count
  $audioCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_audio::[^:]+: test$' }).Count
  $videoCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_video::[^:]+: test$' }).Count
  $continuationCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_continuation::[^:]+: test$' }).Count
  $healthCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_health::[^:]+: test$' }).Count
  $progressCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_progress::[^:]+: test$' }).Count
  $cohortCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_cohort::[^:]+: test$' }).Count
  $queueCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_queue::[^:]+: test$' }).Count
  $officialCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_official::[^:]+: test$' }).Count
  $discardCount = @($managedInventory | Where-Object { $_ -match '^managed_media::native_queue_discard::[^:]+: test$' }).Count
  $allManagedCount = @($managedInventory | Where-Object { $_ -match '^managed_media::.*: test$' }).Count
  if ($nativeCount -ne 10 -or $audioCount -ne 8 -or $videoCount -ne 9 -or $continuationCount -ne 6 -or $healthCount -ne 5 -or $progressCount -ne 5 -or $cohortCount -ne 4 -or $queueCount -ne 4 -or $officialCount -ne 4 -or $discardCount -ne 6 -or $allManagedCount -ne 67) { throw "Native media inventory mismatch: expected 10/8/9/6/5/5/4/4/4/6/67, collected $nativeCount/$audioCount/$videoCount/$continuationCount/$healthCount/$progressCount/$cohortCount/$queueCount/$officialCount/$discardCount/$allManagedCount" }
  Write-Output 'Verified exactly 6 managed real-media tests.'
  Invoke-QualityCommand 'Managed candidate all-target Clippy' cargo @('clippy', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--all-targets', '--', '-D', 'warnings')
  Invoke-QualityCommand 'Six signed installed-image LPAC checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::', '--', '--test-threads=1', '--skip', 'managed_media::native_inspection::', '--skip', 'managed_media::native_audio::', '--skip', 'managed_media::native_video::', '--skip', 'managed_media::native_continuation::', '--skip', 'managed_media::native_health::', '--skip', 'managed_media::native_progress::', '--skip', 'managed_media::native_cohort::', '--skip', 'managed_media::native_queue::', '--skip', 'managed_media::native_official::', '--skip', 'managed_media::native_queue_discard::')
  Invoke-QualityCommand 'Ten Core-owned native inspection checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_inspection::', '--', '--test-threads=1')
  Invoke-QualityCommand 'Eight Core-owned bounded WAV checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_audio::', '--', '--test-threads=1')
  Invoke-QualityCommand 'Nine Core-owned bounded WebM checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_video::', '--', '--test-threads=1')
  Invoke-QualityCommand 'Six live media continuation checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_continuation::', '--', '--test-threads=1')
  Invoke-QualityCommand 'Five fixed native health checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_health::', '--', '--test-threads=1')
  Invoke-QualityCommand 'Five current native progress checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_progress::', '--', '--test-threads=1')
  Invoke-QualityCommand 'Four signed native cohort update checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_cohort::', '--', '--test-threads=1')
  Invoke-QualityCommand 'Four signed queued-tool policy checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_queue::', '--', '--test-threads=1')
  Invoke-QualityCommand 'Four official signed-source media checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_official::', '--', '--test-threads=1')
  Invoke-QualityCommand 'Six explicit queued-tool discard checks' cargo @('test', '--locked', '--manifest-path', $managedManifest, '--target-dir', $managedTargetRoot, '--lib', 'managed_media::native_queue_discard::', '--', '--test-threads=1')
} finally { Pop-Location }

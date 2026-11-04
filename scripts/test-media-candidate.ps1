param([Parameter(Mandatory)][string]$BuildRoot)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ci-gates.ps1')
$mediaRepository = Split-Path $PSScriptRoot -Parent
Push-Location -LiteralPath $mediaRepository
try {
  & (Join-Path $PSScriptRoot 'prepare-media-candidate.ps1') -BuildRoot $BuildRoot
  $mediaManifest = 'apps/runtime/test/fixtures/media-candidate/Cargo.toml'
  Invoke-QualityCommand 'Media candidate formatting' cargo @('fmt', '--manifest-path', $mediaManifest, '--', '--check')
  Invoke-QualityCommand 'Media candidate test inventory' bun @('run', 'scripts/check-rust-tests.ts', $mediaManifest, '6')
  Invoke-QualityCommand 'Media candidate all-target Clippy' cargo @('clippy', '--locked', '--manifest-path', $mediaManifest, '--all-targets', '--', '-D', 'warnings')
  Invoke-QualityCommand 'Six real-media LPAC checks' cargo @('test', '--locked', '--manifest-path', $mediaManifest, '--', '--test-threads=1')
} finally { Pop-Location }

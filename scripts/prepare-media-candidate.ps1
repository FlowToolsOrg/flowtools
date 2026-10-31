param([Parameter(Mandatory)][string]$BuildRoot)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or -not [Environment]::Is64BitProcess) { throw 'Windows/x64 validation is required' }
$mediaRepository = Split-Path $PSScriptRoot -Parent
$mediaFixture = Join-Path $mediaRepository 'apps/runtime/test/fixtures/media-candidate'
$mediaLock = Get-Content -LiteralPath (Join-Path $mediaFixture 'candidate-lock.json') -Raw | ConvertFrom-Json
$mediaSourceLockPath = Join-Path $PSScriptRoot 'media-candidate/source-lock.json'
$mediaSourceDigest = (Get-FileHash -LiteralPath $mediaSourceLockPath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($mediaLock.formatVersion -ne 1 -or $mediaLock.maturity -ne 'prototype' -or $mediaLock.target -ne 'windows-x64' -or $mediaLock.buildFlavor -ne 'validation-lgpl-static-v1' -or $mediaLock.sourceLockSha256 -cne $mediaSourceDigest) { throw 'Candidate lock refused' }

function Assert-MediaBuildPath([string]$Path) {
  $mediaCurrent = [IO.Path]::GetFullPath($Path)
  while ($mediaCurrent) {
    if (Test-Path -LiteralPath $mediaCurrent) {
      if (((Get-Item -LiteralPath $mediaCurrent -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Candidate path redirection refused' }
    }
    $mediaCurrent = [IO.Path]::GetDirectoryName($mediaCurrent)
  }
}
$mediaBuild = (Resolve-Path -LiteralPath $BuildRoot -ErrorAction Stop).Path
Assert-MediaBuildPath $mediaBuild
$mediaReceipt = Get-Content -LiteralPath (Join-Path $mediaBuild 'source-verification.json') -Raw | ConvertFrom-Json
if ($mediaReceipt.formatVersion -ne 1 -or $mediaReceipt.maturity -ne 'prototype' -or $mediaReceipt.sourceLockSha256 -cne $mediaSourceDigest) { throw 'Exact source verification receipt required' }

$mediaImages = @{}
foreach ($mediaRole in @('ffmpeg', 'ffprobe')) {
  $mediaInput = Join-Path $mediaBuild "candidate/bin/$mediaRole.exe"
  Assert-MediaBuildPath $mediaInput
  $mediaStream = [IO.File]::Open($mediaInput, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  try {
    if ($mediaStream.Length -lt 1024 -or $mediaStream.Length -gt 33554432) { throw 'Candidate image budget refused' }
    $mediaBytes = [byte[]]::new([int]$mediaStream.Length)
    $mediaStream.ReadExactly($mediaBytes, 0, $mediaBytes.Length)
    if ($mediaBytes[0] -ne 0x4d -or $mediaBytes[1] -ne 0x5a) { throw 'Candidate image header refused' }
    $mediaHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($mediaBytes)).ToLowerInvariant()
    if ($mediaHash -cne $mediaLock.programs.$mediaRole) { throw "Exact $mediaRole candidate hash refused" }
    $mediaImages[$mediaRole] = $mediaBytes
  } finally { $mediaStream.Dispose() }
}

# All input images pass before touching this fixed ignored developer output.
# Atomic replacement also avoids writing through an existing hardlink's inode.
$mediaOutput = Join-Path $mediaFixture '.generated/bin'
Assert-MediaBuildPath $mediaOutput
[IO.Directory]::CreateDirectory($mediaOutput) | Out-Null
Assert-MediaBuildPath $mediaOutput
foreach ($mediaRole in @('ffmpeg', 'ffprobe')) {
  $mediaTarget = Join-Path $mediaOutput "$mediaRole.exe"
  Assert-MediaBuildPath $mediaTarget
  if (Test-Path -LiteralPath $mediaTarget -PathType Container) { throw 'Unexpected candidate output directory' }
  $mediaTemporary = Join-Path $mediaOutput ('.candidate-' + [guid]::NewGuid().ToString('D') + '.tmp')
  $mediaCreated = $false
  try {
    $mediaFile = [IO.File]::Open($mediaTemporary, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    $mediaCreated = $true
    try { $mediaFile.Write($mediaImages[$mediaRole], 0, $mediaImages[$mediaRole].Length); $mediaFile.Flush($true) } finally { $mediaFile.Dispose() }
    [IO.File]::Move($mediaTemporary, $mediaTarget, $true)
    $mediaCreated = $false
  } finally {
    if ($mediaCreated) { Remove-Item -LiteralPath $mediaTemporary -Force -ErrorAction Stop }
  }
}
Write-Output 'Exact build-only media images prepared; no Runtime installation, review profile or grant.'

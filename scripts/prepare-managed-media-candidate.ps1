param([Parameter(Mandatory)][string]$BuildRoot)
$ErrorActionPreference = 'Stop'
# Preserve the original candidate/source/program checks and fixed hashes.
& (Join-Path $PSScriptRoot 'prepare-media-candidate.ps1') -BuildRoot $BuildRoot
$managedRepository = Split-Path $PSScriptRoot -Parent
$managedFixture = Join-Path $managedRepository 'apps/runtime/test/fixtures/managed-media-candidate'
$managedLock = Get-Content -LiteralPath (Join-Path $managedFixture 'notice-lock.json') -Raw | ConvertFrom-Json
if ($managedLock.formatVersion -ne 1 -or $managedLock.maturity -cne 'prototype' -or $managedLock.files.Count -ne 4) { throw 'Managed notice lock refused' }
function Assert-ManagedCandidatePath([string]$Path) {
  $managedCurrent = [IO.Path]::GetFullPath($Path)
  while ($managedCurrent) {
    if (Test-Path -LiteralPath $managedCurrent) {
      if (((Get-Item -LiteralPath $managedCurrent -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Managed candidate path redirection refused' }
    }
    $managedCurrent = [IO.Path]::GetDirectoryName($managedCurrent)
  }
}
$managedBuild = (Resolve-Path -LiteralPath $BuildRoot -ErrorAction Stop).Path
$managedNotices = [IO.MemoryStream]::new()
try {
  foreach ($managedEntry in $managedLock.files) {
    if ($managedEntry.path -notmatch '^sources/[A-Za-z0-9.-]+/(COPYING(?:\.LGPLv2\.1)?|LICENSE|PATENTS)$' -or $managedEntry.sha256 -cnotmatch '^[0-9a-f]{64}$') { throw 'Managed notice entry refused' }
    $managedInput = Join-Path $managedBuild $managedEntry.path
    Assert-ManagedCandidatePath $managedInput
    $managedStream = [IO.File]::Open($managedInput, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
      if ($managedStream.Length -lt 1 -or $managedStream.Length -gt 1048576) { throw 'Managed notice size refused' }
      $managedBytes = [byte[]]::new([int]$managedStream.Length)
      $managedStream.ReadExactly($managedBytes, 0, $managedBytes.Length)
      $managedHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($managedBytes)).ToLowerInvariant()
      if ($managedHash -cne $managedEntry.sha256) { throw 'Exact source notice hash required' }
      $managedNotices.Write([Text.Encoding]::UTF8.GetBytes(('===== ' + $managedEntry.path + " =====`n")))
      $managedNotices.Write($managedBytes)
      $managedNotices.WriteByte(10)
    } finally { $managedStream.Dispose() }
  }
  $managedCombined = $managedNotices.ToArray()
  if ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($managedCombined)).ToLowerInvariant() -cne $managedLock.combinedSha256) { throw 'Exact combined notice bytes required' }
} finally { $managedNotices.Dispose() }

# Fixed ignored test input, never a signed distribution or full source offer.
$managedOutput = Join-Path $managedFixture '.generated'
Assert-ManagedCandidatePath $managedOutput
[IO.Directory]::CreateDirectory($managedOutput) | Out-Null
Assert-ManagedCandidatePath $managedOutput
$managedTarget = Join-Path $managedOutput 'notices.txt'
Assert-ManagedCandidatePath $managedTarget
if (Test-Path -LiteralPath $managedTarget) {
  $managedExisting = [IO.File]::Open($managedTarget, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  try {
    if ($managedExisting.Length -ne $managedCombined.Length -or [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($managedExisting)).ToLowerInvariant() -cne $managedLock.combinedSha256) { throw 'Unknown managed notice output preserved and refused' }
  } finally { $managedExisting.Dispose() }
} else {
  $managedFile = [IO.File]::Open($managedTarget, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
  try { $managedFile.Write($managedCombined); $managedFile.Flush($true) } finally { $managedFile.Dispose() }
}
Write-Output 'Exact notices prepared for unpublished signed-fixture validation; no production review, release signature or legal approval.'

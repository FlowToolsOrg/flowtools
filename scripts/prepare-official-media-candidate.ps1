param([Parameter(Mandatory)][string]$BuildRoot)
$ErrorActionPreference = 'Stop'
$officialRepository = Split-Path $PSScriptRoot -Parent
$officialLock = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'media-candidate/official-8.1.3-lock.json') -Raw | ConvertFrom-Json
if ($officialLock.formatVersion -ne 1 -or $officialLock.maturity -cne 'prototype' -or $officialLock.version -cne '8.1.3' -or $officialLock.signer -cne 'FCF986EA15E6E293A5644F10B4322F04D67658D8') { throw 'Official source lock refused' }
function Assert-OfficialPath([string]$Path) {
  $officialCurrent = [IO.Path]::GetFullPath($Path)
  while ($officialCurrent) {
    if ((Test-Path -LiteralPath $officialCurrent) -and (((Get-Item -LiteralPath $officialCurrent -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) { throw 'Official source path redirection refused' }
    $officialCurrent = [IO.Path]::GetDirectoryName($officialCurrent)
  }
}
function Read-OfficialBytes([string]$Path, [string]$Digest, [long]$Limit) {
  Assert-OfficialPath $Path
  $officialStream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  try {
    if ($officialStream.Length -lt 1 -or $officialStream.Length -gt $Limit) { throw 'Official source size refused' }
    $officialData = [byte[]]::new([int]$officialStream.Length)
    $officialStream.ReadExactly($officialData, 0, $officialData.Length)
    if ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($officialData)).ToLowerInvariant() -cne $Digest) { throw 'Exact official source/program hash required' }
    return ,$officialData
  } finally { $officialStream.Dispose() }
}
$officialBuild = (Resolve-Path -LiteralPath $BuildRoot -ErrorAction Stop).Path
Assert-OfficialPath $officialBuild
$officialArchive = Join-Path $officialBuild 'upstream/ffmpeg-8.1.3.tar.xz'
$officialKey = Join-Path $PSScriptRoot 'media-candidate/ffmpeg-release-key.asc'
$officialSignature = Join-Path $PSScriptRoot 'media-candidate/ffmpeg-8.1.3.tar.xz.asc'
$null = Read-OfficialBytes $officialArchive $officialLock.archiveSha256 33554432
$officialKeyBytes = Read-OfficialBytes $officialKey $officialLock.keySha256 16384
$null = Read-OfficialBytes $officialSignature $officialLock.signatureSha256 16384
$officialGpg = Get-Command gpgv -ErrorAction SilentlyContinue
if ($officialGpg) { $officialGpgPath = $officialGpg.Source } else {
  $officialGit = Get-Command git -ErrorAction Stop
  $officialGpgPath = Join-Path (Split-Path (Split-Path $officialGit.Source -Parent) -Parent) 'usr/bin/gpgv.exe'
}
if (-not (Test-Path -LiteralPath $officialGpgPath -PathType Leaf)) { throw 'GnuPG required for actual official release verification' }
$officialKeyring = Join-Path $officialBuild ('verification-' + [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($officialKeyring) | Out-Null
# gpgv only checks signatures against the explicitly pinned public keyring. It
# neither starts an agent nor fetches keys or modifies the user's GnuPG state.
# Git's MSYS program requires its own drive syntax when called by PowerShell.
$officialMsys = $officialGpgPath.Replace('\', '/').EndsWith('/usr/bin/gpgv.exe', [StringComparison]::OrdinalIgnoreCase)
function Convert-OfficialGpgPath([string]$Path) {
  $officialFull = [IO.Path]::GetFullPath($Path)
  if (-not $officialMsys) { return $officialFull }
  if ($officialFull -notmatch '^[A-Za-z]:[\\/]') { throw 'MSYS GnuPG requires an explicit local drive path' }
  return '/' + $officialFull.Substring(0, 1).ToLowerInvariant() + '/' + $officialFull.Substring(3).Replace('\', '/')
}
$officialGpgHome = Convert-OfficialGpgPath $officialKeyring
$officialPublicRing = Join-Path $officialKeyring 'upstream-public-key.gpg'
$officialArmor = [Text.Encoding]::ASCII.GetString($officialKeyBytes)
$officialEncoded = @([regex]::Matches($officialArmor, '(?m)^[A-Za-z0-9+/]{1,76}={0,2}\r?$') | ForEach-Object { $_.Value.Trim() }) -join ''
$officialPublicBytes = [Convert]::FromBase64String($officialEncoded)
$officialRingFile = [IO.File]::Open($officialPublicRing, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
try { $officialRingFile.Write($officialPublicBytes); $officialRingFile.Flush($true) } finally { $officialRingFile.Dispose() }
$officialGpgKey = Convert-OfficialGpgPath $officialPublicRing
$officialGpgSignature = Convert-OfficialGpgPath $officialSignature
$officialGpgArchive = Convert-OfficialGpgPath $officialArchive
# Hold and rehash the exact three files used by gpgv. A path checked earlier
# cannot be replaced between the independent hash and signature checks.
$officialGuards = [Collections.Generic.List[IO.FileStream]]::new()
try {
  foreach ($officialHeld in @(
    @{ Path = $officialArchive; Digest = $officialLock.archiveSha256; Limit = 33554432 },
    @{ Path = $officialSignature; Digest = $officialLock.signatureSha256; Limit = 16384 },
    @{ Path = $officialPublicRing; Digest = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($officialPublicBytes)).ToLowerInvariant(); Limit = 16384 }
  )) {
    Assert-OfficialPath $officialHeld.Path
    $officialGuard = [IO.File]::Open($officialHeld.Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $officialGuards.Add($officialGuard)
    if ($officialGuard.Length -lt 1 -or $officialGuard.Length -gt $officialHeld.Limit -or [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($officialGuard)).ToLowerInvariant() -cne $officialHeld.Digest) { throw 'Held official signature input changed' }
  }
  $officialStatus = & $officialGpgPath --homedir $officialGpgHome --keyring $officialGpgKey --status-fd 1 $officialGpgSignature $officialGpgArchive
  $officialSignatureExit = $LASTEXITCODE
} finally { foreach ($officialGuard in $officialGuards) { $officialGuard.Dispose() } }
if ($officialSignatureExit -ne 0 -or @($officialStatus | Where-Object { $_ -cmatch '^\[GNUPG:\] VALIDSIG FCF986EA15E6E293A5644F10B4322F04D67658D8 ' }).Count -ne 1) { throw 'Official PGP signature/fingerprint refused' }
$officialImages = @{}
foreach ($officialRole in @('ffmpeg', 'ffprobe')) {
  $officialImages[$officialRole] = Read-OfficialBytes (Join-Path $officialBuild "candidate/bin/$officialRole.exe") $officialLock.programs.$officialRole 33554432
  if ($officialImages[$officialRole][0] -ne 0x4d -or $officialImages[$officialRole][1] -ne 0x5a) { throw 'Official program header refused' }
}
$officialNotices = [IO.MemoryStream]::new()
try {
  foreach ($officialNotice in $officialLock.notices) {
    if ($officialNotice.path -cnotmatch '^candidate/licenses/[a-z-]+\.txt$') { throw 'Official notice path refused' }
    $officialBytes = Read-OfficialBytes (Join-Path $officialBuild $officialNotice.path) $officialNotice.sha256 1048576
    $officialNotices.Write([Text.Encoding]::UTF8.GetBytes(('===== ' + $officialNotice.path + " =====`n")))
    $officialNotices.Write($officialBytes); $officialNotices.WriteByte(10)
  }
  $officialNoticeBytes = $officialNotices.ToArray()
  if ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($officialNoticeBytes)).ToLowerInvariant() -cne $officialLock.combinedNoticesSha256) { throw 'Official notice aggregate refused' }
} finally { $officialNotices.Dispose() }
$officialOutput = Join-Path $officialRepository 'apps/runtime/test/fixtures/managed-media-candidate/.generated/official'
Assert-OfficialPath $officialOutput
[IO.Directory]::CreateDirectory($officialOutput) | Out-Null
$officialImages['notices'] = $officialNoticeBytes
foreach ($officialRole in @('ffmpeg', 'ffprobe', 'notices')) {
  $officialName = if ($officialRole -ceq 'notices') { 'notices.txt' } else { "$officialRole.exe" }
  $officialTarget = Join-Path $officialOutput $officialName
  Assert-OfficialPath $officialTarget
  if (Test-Path -LiteralPath $officialTarget) {
    $officialExpected = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($officialImages[$officialRole])).ToLowerInvariant()
    $null = Read-OfficialBytes $officialTarget $officialExpected 33554432
  } else {
    $officialFile = [IO.File]::Open($officialTarget, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try { $officialFile.Write($officialImages[$officialRole]); $officialFile.Flush($true) } finally { $officialFile.Dispose() }
  }
}
Write-Output 'Official PGP source and exact small static pair verified; original fixture images and production authority unchanged.'

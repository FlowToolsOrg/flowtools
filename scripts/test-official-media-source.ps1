param([Parameter(Mandatory)][string]$BuildRoot)
$ErrorActionPreference = 'Stop'
$sourceRepository = Split-Path $PSScriptRoot -Parent
$sourceBuild = (Resolve-Path -LiteralPath $BuildRoot -ErrorAction Stop).Path
$sourceFixture = Join-Path ([IO.Path]::GetTempPath()) ('flowtools-official-source-' + [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($sourceFixture) | Out-Null
$sourceCompleted = 0
function Invoke-SourceRefusal([string]$Script, [string]$ArgumentName, [string]$ArgumentValue, [string]$Expected) {
  $sourceStart = [Diagnostics.ProcessStartInfo]::new((Get-Command pwsh -ErrorAction Stop).Source)
  $sourceStart.UseShellExecute = $false
  $sourceStart.CreateNoWindow = $true
  $sourceStart.RedirectStandardOutput = $true
  $sourceStart.RedirectStandardError = $true
  foreach ($sourceArgument in @('-NoProfile', '-File', $Script, $ArgumentName, $ArgumentValue)) { $sourceStart.ArgumentList.Add($sourceArgument) }
  $sourceProcess = [Diagnostics.Process]::new(); $sourceProcess.StartInfo = $sourceStart
  try {
    if (-not $sourceProcess.Start()) { throw 'Source refusal process failed to start' }
    $sourceOut = $sourceProcess.StandardOutput.ReadToEndAsync(); $sourceErr = $sourceProcess.StandardError.ReadToEndAsync()
    if (-not $sourceProcess.WaitForExit(15000)) { $sourceProcess.Kill($true); $sourceProcess.WaitForExit(); throw 'Source refusal exceeded its bound' }
    $sourceText = $sourceOut.GetAwaiter().GetResult() + $sourceErr.GetAwaiter().GetResult()
    if ($sourceText.Length -gt 65536 -or $sourceProcess.ExitCode -eq 0 -or -not $sourceText.Contains($Expected)) { throw ('Expected source refusal absent: ' + $sourceText) }
  } finally { $sourceProcess.Dispose() }
}
# An existing unknown download must remain byte-identical and never be replaced.
$sourceUnknown = Join-Path $sourceFixture 'unknown/upstream/ffmpeg-8.1.3.tar.xz'
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($sourceUnknown)) | Out-Null
$sourceForeign = [Text.Encoding]::ASCII.GetBytes('FOREIGN_SOURCE_BYTES')
[IO.File]::WriteAllBytes($sourceUnknown, $sourceForeign)
Invoke-SourceRefusal (Join-Path $PSScriptRoot 'download-official-media.ps1') '-Destination' (Join-Path $sourceFixture 'unknown') 'Unknown existing download preserved and refused'
if ([Convert]::ToHexString([IO.File]::ReadAllBytes($sourceUnknown)) -cne [Convert]::ToHexString($sourceForeign)) { throw 'Unknown download was changed' }
$sourceCompleted++
foreach ($sourceCase in @('archive', 'signature', 'program')) {
  $sourceCaseRoot = Join-Path $sourceFixture $sourceCase
  $sourceScripts = Join-Path $sourceCaseRoot 'scripts'
  $sourceData = Join-Path $sourceScripts 'media-candidate'
  $sourceCaseBuild = Join-Path $sourceCaseRoot 'owned-build'
  [IO.Directory]::CreateDirectory($sourceData) | Out-Null
  [IO.Directory]::CreateDirectory((Join-Path $sourceCaseBuild 'upstream')) | Out-Null
  Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'prepare-official-media-candidate.ps1') -Destination $sourceScripts
  foreach ($sourceName in @('official-8.1.3-lock.json', 'ffmpeg-release-key.asc', 'ffmpeg-8.1.3.tar.xz.asc')) { Copy-Item -LiteralPath (Join-Path $PSScriptRoot "media-candidate/$sourceName") -Destination $sourceData }
  $sourceArchive = Join-Path $sourceCaseBuild 'upstream/ffmpeg-8.1.3.tar.xz'
  Copy-Item -LiteralPath (Join-Path $sourceBuild 'upstream/ffmpeg-8.1.3.tar.xz') -Destination $sourceArchive
  Copy-Item -LiteralPath (Join-Path $sourceBuild 'candidate') -Destination $sourceCaseBuild -Recurse
  $sourceLockPath = Join-Path $sourceData 'official-8.1.3-lock.json'
  if ($sourceCase -in @('archive', 'signature')) {
    $sourceChanged = [IO.File]::ReadAllBytes($sourceArchive); $sourceChanged[$sourceChanged.Length - 1] = $sourceChanged[$sourceChanged.Length - 1] -bxor 1
    [IO.File]::WriteAllBytes($sourceArchive, $sourceChanged)
    if ($sourceCase -ceq 'signature') {
      # Even a matching hash in a substituted fixture lock cannot forge the
      # independently pinned upstream signature over these changed bytes.
      $sourceLock = Get-Content -LiteralPath $sourceLockPath -Raw | ConvertFrom-Json
      $sourceLock.archiveSha256 = (Get-FileHash -LiteralPath $sourceArchive -Algorithm SHA256).Hash.ToLowerInvariant()
      [IO.File]::WriteAllText($sourceLockPath, ($sourceLock | ConvertTo-Json -Depth 8))
    }
  } else {
    $sourceProgram = Join-Path $sourceCaseBuild 'candidate/bin/ffmpeg.exe'
    $sourceChanged = [IO.File]::ReadAllBytes($sourceProgram); $sourceChanged[1024] = $sourceChanged[1024] -bxor 1
    [IO.File]::WriteAllBytes($sourceProgram, $sourceChanged)
  }
  $sourceExpected = if ($sourceCase -ceq 'signature') { 'Official PGP signature/fingerprint refused' } else { 'Exact official source/program hash required' }
  Invoke-SourceRefusal (Join-Path $sourceScripts 'prepare-official-media-candidate.ps1') '-BuildRoot' $sourceCaseBuild $sourceExpected
  if (Test-Path -LiteralPath (Join-Path $sourceCaseRoot 'apps')) { throw 'Invalid source/program material published fixture output' }
  $sourceCompleted++
}
if ($sourceCompleted -ne 4) { throw 'Exactly four actual official-source refusal cases required' }
Write-Output 'Four actual official-source refusal cases passed; inputs and original fixture outputs preserved.'

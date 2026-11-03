param([Parameter(Mandatory)][string]$Destination)
$ErrorActionPreference = 'Stop'
# Developer acquisition only. Neither these paths nor URLs enter Runtime IPC.
$officialDownloads = @(
  @{ Name = 'upstream/ffmpeg-8.1.3.tar.xz'; Url = 'https://ffmpeg.org/releases/ffmpeg-8.1.3.tar.xz'; Sha256 = '7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3'; Limit = 33554432 },
  @{ Name = 'upstream/ffmpeg-8.1.3.tar.xz.asc'; Url = 'https://ffmpeg.org/releases/ffmpeg-8.1.3.tar.xz.asc'; Sha256 = '5f2880c7fea01f8694d30cb9d06362f77241ee67ce5926b157a226b18f08a538'; Limit = 16384 },
  @{ Name = 'upstream/ffmpeg-devel.asc'; Url = 'https://ffmpeg.org/ffmpeg-devel.asc'; Sha256 = '397b3becedcd5a98769967ff1ff8501ddc89f8368b8f766e4701377d7dbaabe5'; Limit = 16384 },
  @{ Name = 'reference/checksums.sha256'; Url = 'https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-11-02-12-51/checksums.sha256'; Sha256 = '8af2e6d4e745f12aa10ccfb146fd28ca52763387ae2e749844474d084a422e26'; Limit = 1048576 },
  @{ Name = 'reference/ffmpeg-n8.1.3-32-g760d10603e-win64-lgpl-8.1.zip'; Url = 'https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-11-02-12-51/ffmpeg-n8.1.3-32-g760d10603e-win64-lgpl-8.1.zip'; Sha256 = 'd84ba8f0636333a242728908d8cdefa1496893cbc616c706a8379aa4570ced04'; Limit = 209715200 }
)
function Assert-OfficialDownloadPath([string]$Path) {
  $downloadCurrent = [IO.Path]::GetFullPath($Path)
  while ($downloadCurrent) {
    if ((Test-Path -LiteralPath $downloadCurrent) -and (((Get-Item -LiteralPath $downloadCurrent -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) { throw 'Download path redirection refused' }
    $downloadCurrent = [IO.Path]::GetDirectoryName($downloadCurrent)
  }
}
$officialDestination = [IO.Path]::GetFullPath($Destination)
Assert-OfficialDownloadPath $officialDestination
[IO.Directory]::CreateDirectory($officialDestination) | Out-Null
$officialHttp = [Net.Http.HttpClient]::new()
$officialHttp.Timeout = [TimeSpan]::FromMinutes(5)
$officialHttp.DefaultRequestHeaders.UserAgent.ParseAdd('FlowTools-Official-Media-Acquisition/1.0')
try {
  foreach ($officialDownload in $officialDownloads) {
    $officialTarget = Join-Path $officialDestination $officialDownload.Name
    Assert-OfficialDownloadPath $officialTarget
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($officialTarget)) | Out-Null
    if (Test-Path -LiteralPath $officialTarget) {
      $officialExisting = Get-Item -LiteralPath $officialTarget -Force
      if ($officialExisting.Length -lt 1 -or $officialExisting.Length -gt $officialDownload.Limit -or (Get-FileHash -LiteralPath $officialTarget -Algorithm SHA256).Hash.ToLowerInvariant() -cne $officialDownload.Sha256) { throw 'Unknown existing download preserved and refused' }
      Write-Output ('Verified existing ' + $officialDownload.Name)
      continue
    }
    $officialPartial = $officialTarget + '.partial-' + [Guid]::NewGuid().ToString('N')
    $officialCancel = [Threading.CancellationTokenSource]::new([TimeSpan]::FromMinutes(5))
    $officialResponse = $null
    try {
      $officialResponse = $officialHttp.GetAsync($officialDownload.Url, [Net.Http.HttpCompletionOption]::ResponseHeadersRead, $officialCancel.Token).GetAwaiter().GetResult()
      $officialResponse.EnsureSuccessStatusCode() | Out-Null
      if ($officialResponse.Content.Headers.ContentLength -gt $officialDownload.Limit) { throw 'Download declared size refused' }
      $officialInput = $officialResponse.Content.ReadAsStream()
      $officialFile = [IO.File]::Open($officialPartial, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
      try {
        $officialBuffer = [byte[]]::new(1048576); $officialTotal = 0L
        while (($officialRead = $officialInput.ReadAsync($officialBuffer, 0, $officialBuffer.Length, $officialCancel.Token).GetAwaiter().GetResult()) -gt 0) {
          $officialTotal += $officialRead
          if ($officialTotal -gt $officialDownload.Limit) { throw 'Download actual size refused' }
          $officialFile.Write($officialBuffer, 0, $officialRead)
        }
        $officialFile.Flush($true)
      } finally { $officialFile.Dispose(); $officialInput.Dispose() }
      if ((Get-FileHash -LiteralPath $officialPartial -Algorithm SHA256).Hash.ToLowerInvariant() -cne $officialDownload.Sha256) { throw 'Download checksum refused; partial preserved' }
      # Both absolute paths are checked inside the one explicitly owned output.
      $officialBoundary = $officialDestination.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
      foreach ($officialPath in @($officialPartial, $officialTarget)) {
        if (-not [IO.Path]::GetFullPath($officialPath).StartsWith($officialBoundary, [StringComparison]::OrdinalIgnoreCase)) { throw 'Download publication escaped destination' }
        Assert-OfficialDownloadPath $officialPath
      }
      [IO.File]::Move($officialPartial, $officialTarget, $false)
      Write-Output ('Downloaded exact ' + $officialDownload.Name)
    } finally { if ($officialResponse) { $officialResponse.Dispose() }; $officialCancel.Dispose() }
  }
  $officialChecksum = Get-Content -LiteralPath (Join-Path $officialDestination 'reference/checksums.sha256')
  if (@($officialChecksum | Where-Object { $_ -cmatch '^d84ba8f0636333a242728908d8cdefa1496893cbc616c706a8379aa4570ced04\s+\*?ffmpeg-n8\.1\.3-32-g760d10603e-win64-lgpl-8\.1\.zip$' }).Count -ne 1) { throw 'Independent release checksum manifest refused' }
} finally { $officialHttp.Dispose() }
Write-Output 'Official signed source and official-linked Windows pair acquired as data; no installation, execution or production grant.'

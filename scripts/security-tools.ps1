$ErrorActionPreference = 'Stop'

# Build/validation tools only. Never bundled or selectable through product IPC.
$SecurityTools = @{
  gitleaks = @{
    Version = '8.30.1'
    Url = 'https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_windows_x64.zip'
    ArchiveSha256 = 'd29144deff3a68aa93ced33dddf84b7fdc26070add4aa0f4513094c8332afc4e'
    Executable = 'gitleaks.exe'
    ExecutableSha256 = '17157e2ee8b76fc8b1d8bee607a250e34b8a8023c8bc81822d4b5ee4d78fcb7c'
  }
  'cargo-audit' = @{
    Version = '0.22.2'
    Url = 'https://github.com/rustsec/rustsec/releases/download/cargo-audit/v0.22.2/cargo-audit-x86_64-pc-windows-msvc-v0.22.2.zip'
    ArchiveSha256 = '0a7316540862c13d954f648917ceacca593747baed6eec180fafa590be2710ab'
    Executable = 'cargo-audit-x86_64-pc-windows-msvc-v0.22.2/cargo-audit.exe'
    ExecutableSha256 = '0157f5ce1ce9fd4fb0a1f7c79af1229771d1f80b6c2613ddb0d9200a8ba73946'
  }
}

function Assert-SecurityDigest {
  param(
    [Parameter(Mandatory)][string]$Path,
    [Parameter(Mandatory)][string]$Expected
  )
  $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actual -cne $Expected) { throw 'Security tool integrity mismatch' }
}

function Get-SecurityTool {
  param([Parameter(Mandatory)][ValidateSet('gitleaks', 'cargo-audit')][string]$Name)
  if (-not $IsWindows -or [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw 'Security scanner bootstrap supports Windows x64 only'
  }
  $pin = $SecurityTools[$Name]
  $directory = Join-Path ([System.IO.Path]::GetTempPath()) "flowtools-security-$Name-$($pin.Version)"
  $executable = Join-Path $directory $pin.Executable
  if (-not (Test-Path -LiteralPath $executable)) {
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    $archive = Join-Path $directory 'download.zip'
    Invoke-WebRequest -Uri $pin.Url -OutFile $archive -TimeoutSec 120
    Assert-SecurityDigest $archive $pin.ArchiveSha256
    Expand-Archive -LiteralPath $archive -DestinationPath $directory -Force
  }
  # Also recheck an existing cache before every execution; never trust PATH/version alone.
  Assert-SecurityDigest $executable $pin.ExecutableSha256
  return $executable
}

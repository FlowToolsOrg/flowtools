$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'security-tools.ps1')

$scannerFixtureRoot = [System.IO.Path]::GetFullPath((Join-Path ([System.IO.Path]::GetTempPath()) ('flowtools-scanner-controls-' + [guid]::NewGuid())))
New-Item -ItemType Directory -Path $scannerFixtureRoot | Out-Null
$repository = Split-Path $PSScriptRoot -Parent
$gitleaks = Get-SecurityTool gitleaks
$audit = Get-SecurityTool cargo-audit

function Invoke-ScannerControl {
  param([string]$Command, [string[]]$Arguments, [int]$ExpectedExit)
  $output = @(& $Command @Arguments 2>&1)
  if ($LASTEXITCODE -ne $ExpectedExit) {
    throw "Scanner control $(Split-Path $Command -Leaf)/$($Arguments[0]) expected exit $ExpectedExit, received $LASTEXITCODE"
  }
  return ($output -join "`n")
}

try {
  Push-Location -LiteralPath $scannerFixtureRoot
  try {
    $integrityControl = Join-Path $scannerFixtureRoot 'scanner-byte-control'
    Set-Content -LiteralPath $integrityControl -Value 'original bytes' -NoNewline
    $expectedDigest = (Get-FileHash -LiteralPath $integrityControl -Algorithm SHA256).Hash.ToLowerInvariant()
    Assert-SecurityDigest $integrityControl $expectedDigest
    Set-Content -LiteralPath $integrityControl -Value 'substituted bytes' -NoNewline
    $substitutionRefused = $false
    try { Assert-SecurityDigest $integrityControl $expectedDigest } catch {
      if ($_.Exception.Message -cne 'Security tool integrity mismatch') { throw }
      $substitutionRefused = $true
    }
    if (-not $substitutionRefused) { throw 'Substituted scanner bytes were accepted' }
    Remove-Item -LiteralPath $integrityControl
    Invoke-ScannerControl git @('init', '--quiet') 0 | Out-Null
    Invoke-ScannerControl git @('config', 'user.name', 'Scanner fixture') 0 | Out-Null
    Invoke-ScannerControl git @('config', 'user.email', 'scanner@example.invalid') 0 | Out-Null
    # Local synthetic credentials only, never printed or sent to a service.
    $credential = 'ghp_' + ([guid]::NewGuid().ToString('N') + 'AB12')
    $path = 'apps/ui-test/fixtures/plugin-webviews'
    New-Item -ItemType Directory -Path $path -Force | Out-Null
    Set-Content -LiteralPath "$path/loopback.key" -Value 'public fixture' -NoNewline
    Invoke-ScannerControl git @('add', '.') 0 | Out-Null
    Invoke-ScannerControl git @('commit', '--quiet', '-m', 'clean control') 0 | Out-Null
    $arguments = @(
      'git', '--redact=100', '--no-banner', '--ignore-gitleaks-allow',
      '--config', (Join-Path $repository '.gitleaks.toml'),
      '--gitleaks-ignore-path', (Join-Path $repository '.gitleaksignore'),
      '--log-opts=--all --full-history', '.'
    )
    Invoke-ScannerControl $gitleaks $arguments 0 | Out-Null
    Set-Content -LiteralPath "$path/loopback.key" -Value "token=$credential" -NoNewline
    Invoke-ScannerControl git @('add', '.') 0 | Out-Null
    Invoke-ScannerControl git @('commit', '--quiet', '-m', 'synthetic secret') 0 | Out-Null
    $denied = Invoke-ScannerControl $gitleaks $arguments 1
    if ($denied -notmatch 'leaks found: 1' -or $denied.Contains($credential)) {
      throw 'New secret at an ignored historical path must refuse without disclosure'
    }
    Set-Content -LiteralPath "$path/loopback.key" -Value 'removed synthetic secret' -NoNewline
    Invoke-ScannerControl git @('add', '.') 0 | Out-Null
    Invoke-ScannerControl git @('commit', '--quiet', '-m', 'remove synthetic secret') 0 | Out-Null
    Invoke-ScannerControl $gitleaks $arguments 1 | Out-Null

    # The real advisory must be observed; a network/parser/tool error cannot pass.
    $lock = Join-Path $scannerFixtureRoot 'Cargo.lock'
    Set-Content -LiteralPath $lock -Value @'
version = 3
[[package]]
name = "time"
version = "0.1.44"
source = "registry+https://github.com/rust-lang/crates.io-index"
'@
    $vulnerable = Invoke-ScannerControl $audit @('audit', '--file', $lock, '--no-fetch', '--json') 1
    if ($vulnerable -notmatch 'RUSTSEC-2020-0071') { throw 'Known vulnerable Rust control was not detected' }
    Set-Content -LiteralPath $lock -Value 'version = 3'
    Invoke-ScannerControl $audit @('audit', '--file', $lock, '--no-fetch') 0 | Out-Null
    Set-Content -LiteralPath $lock -Value 'invalid lockfile'
    Invoke-ScannerControl $audit @('audit', '--file', $lock, '--no-fetch') 2 | Out-Null
    Write-Output 'Security scanner controls passed: integrity, clean, new/history secret, vulnerable/clean/malformed Rust lock.'
  } finally {
    Pop-Location
  }
} finally {
  $expectedParent = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\', '/')
  if ((Split-Path $scannerFixtureRoot -Parent) -cne $expectedParent -or (Split-Path $scannerFixtureRoot -Leaf) -notlike 'flowtools-scanner-controls-*') {
    throw 'Scanner fixture cleanup escaped the temporary directory'
  }
  Remove-Item -LiteralPath $scannerFixtureRoot -Recurse -Force
}

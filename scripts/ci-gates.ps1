$ErrorActionPreference = 'Stop'

function Invoke-QualityCommand {
  param(
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Command,
    [string[]]$CommandArguments = @(),
    [switch]$RejectErrorOutput
  )

  Write-Output "::group::$Name"
  try {
    $qualityErrorOutput = $false
    if ($RejectErrorOutput) {
      # cargo-audit can report failed registry queries while returning zero.
      # Preserve every diagnostic, including informational warnings.
      & $Command @CommandArguments 2>&1 | ForEach-Object {
        $qualityLine = [string]$_
        Write-Output $qualityLine
        $qualityPlainLine = $qualityLine -replace '\x1b\[[0-?]*[ -/]*[@-~]', ''
        if ($qualityPlainLine.TrimStart() -match '^error:') {
          $qualityErrorOutput = $true
        }
      }
    } else {
      & $Command @CommandArguments
    }
    if ($LASTEXITCODE -ne 0) {
      throw "$Name failed with exit code $LASTEXITCODE"
    }
    if ($qualityErrorOutput) {
      throw "$Name reported errors despite exit code 0"
    }
  } finally {
    Write-Output '::endgroup::'
  }
}

function Assert-CleanWorktree {
  $qualityStatus = @(& git status --porcelain --untracked-files=all)
  if ($LASTEXITCODE -ne 0) {
    throw 'Cannot inspect Git worktree'
  }
  if ($qualityStatus.Count -gt 0) {
    $qualityStatus | Write-Output
    throw 'Quality gates changed tracked or untracked repository files'
  }
}

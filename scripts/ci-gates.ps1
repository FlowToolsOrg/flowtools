$ErrorActionPreference = 'Stop'

function Invoke-QualityCommand {
  param(
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][string]$Command,
    [string[]]$CommandArguments = @()
  )

  Write-Output "::group::$Name"
  try {
    & $Command @CommandArguments
    if ($LASTEXITCODE -ne 0) {
      throw "$Name failed with exit code $LASTEXITCODE"
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

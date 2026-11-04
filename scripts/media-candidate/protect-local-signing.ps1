param(
  [Parameter(Mandatory)][ValidateSet('Protect', 'Unprotect')][string]$Mode,
  [Parameter(Mandatory)][string]$Path
)
$ErrorActionPreference = 'Stop'
$signingPath = [IO.Path]::GetFullPath($Path)
foreach ($signingAncestor in @($signingPath, [IO.Path]::GetDirectoryName($signingPath))) {
  $signingCurrent = $signingAncestor
  while ($signingCurrent) {
    if ((Test-Path -LiteralPath $signingCurrent) -and ((Get-Item -LiteralPath $signingCurrent -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Signing path redirection refused' }
    $signingCurrent = [IO.Path]::GetDirectoryName($signingCurrent)
  }
}
$signingEntropy = [Text.Encoding]::UTF8.GetBytes('FlowTools-Windows-T1-local-package-signing-v1')
if ($Mode -ceq 'Protect') {
  $signingText = [Console]::In.ReadToEnd()
  if ($signingText.Length -lt 1 -or $signingText.Length -gt 16384) { throw 'Signing material budget refused' }
  $signingDirectory = [IO.Path]::GetDirectoryName($signingPath)
  [IO.Directory]::CreateDirectory($signingDirectory) | Out-Null
  $signingSid = [Security.Principal.WindowsIdentity]::GetCurrent().User
  $signingAcl = [Security.AccessControl.DirectorySecurity]::new()
  $signingAcl.SetOwner($signingSid)
  $signingAcl.SetAccessRuleProtection($true, $false)
  foreach ($signingOwner in @($signingSid, [Security.Principal.SecurityIdentifier]::new('S-1-5-18'))) {
    $signingAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($signingOwner, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow'))
  }
  Set-Acl -LiteralPath $signingDirectory -AclObject $signingAcl
  $signingPlain = [Text.Encoding]::UTF8.GetBytes($signingText)
  try { $signingCipher = [Security.Cryptography.ProtectedData]::Protect($signingPlain, $signingEntropy, [Security.Cryptography.DataProtectionScope]::CurrentUser) }
  finally { [Array]::Clear($signingPlain) }
  $signingFile = [IO.File]::Open($signingPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
  try { $signingFile.Write($signingCipher); $signingFile.Flush($true) } finally { $signingFile.Dispose() }
} else {
  $signingFile = [IO.File]::Open($signingPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
  try {
    if ($signingFile.Length -lt 1 -or $signingFile.Length -gt 32768) { throw 'Protected signing material budget refused' }
    $signingCipher = [byte[]]::new([int]$signingFile.Length)
    $signingFile.ReadExactly($signingCipher, 0, $signingCipher.Length)
    $signingPlain = [Security.Cryptography.ProtectedData]::Unprotect($signingCipher, $signingEntropy, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    try { [Console]::Out.Write([Text.Encoding]::UTF8.GetString($signingPlain)) } finally { [Array]::Clear($signingPlain) }
  } finally { $signingFile.Dispose() }
}

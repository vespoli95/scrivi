[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$')]
    [string]$Version,

    [Parameter(Mandatory)]
    [string]$ExecutablePath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (!$IsWindows) { throw 'Installer packaging requires Windows.' }
$parsed = [version]$Version.Substring(1)
foreach ($part in @($parsed.Major, $parsed.Minor, $parsed.Build)) {
    if ($part -gt 65535) { throw 'Version components must be at most 65535.' }
}
$compiler = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
if (!(Test-Path -LiteralPath $compiler)) { throw 'Install Inno Setup 6 before packaging.' }
$executable = (Resolve-Path -LiteralPath $ExecutablePath).Path
$output = Join-Path $PSScriptRoot '../../dist'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$output = (Resolve-Path -LiteralPath $output).Path
& $compiler "/DAppVersion=$($Version.Substring(1))" "/DExecutablePath=$executable" `
    "/DOutputPath=$output" (Join-Path $PSScriptRoot 'scrivi.iss')
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }

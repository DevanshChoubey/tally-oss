# Builds the Windows release artifacts: a zip of the binaries and an Inno Setup installer.
#
# Usage: pwsh packaging/windows/package.ps1 -Version 0.1.0 -ReleaseDir target\release -OutDir dist
# The installer step needs Inno Setup's iscc.exe on the PATH (the CI job installs it).
param(
  [Parameter(Mandatory = $true)][string]$Version,
  [Parameter(Mandatory = $true)][string]$ReleaseDir,
  [Parameter(Mandatory = $true)][string]$OutDir
)
$ErrorActionPreference = 'Stop'
$Version = $Version.TrimStart('v')
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
$name = "ledgerly-$Version-windows-x86_64"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
# iscc resolves relative paths against the .iss file's folder, so hand it absolute ones.
$OutDir = (Resolve-Path $OutDir).Path
$stage = Join-Path $OutDir 'stage'
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item (Join-Path $ReleaseDir 'ledgerly-app.exe'), (Join-Path $ReleaseDir 'ledgerly.exe') $stage
Copy-Item (Join-Path $root 'LICENSE-*') $stage
Copy-Item (Join-Path $root 'NOTICE') (Join-Path $stage 'NOTICE.txt')
Copy-Item (Join-Path $root 'crates\ledgerly-app\assets\fonts\OFL.txt') (Join-Path $stage 'FONTS-OFL.txt')
Copy-Item (Join-Path $root 'crates\ledgerly-app\assets\fonts\Inter-LICENSE.txt') (Join-Path $stage 'FONTS-Inter-LICENSE.txt')

$zipDir = Join-Path $OutDir $name
New-Item -ItemType Directory -Force -Path $zipDir | Out-Null
Copy-Item (Join-Path $stage '*') $zipDir -Recurse
Compress-Archive -Path $zipDir -DestinationPath (Join-Path $OutDir "$name.zip") -Force
Remove-Item -Recurse -Force $zipDir

& iscc.exe "/DAppVersion=$Version" "/DSourceDir=$stage" "/O$OutDir" (Join-Path $PSScriptRoot 'ledgerly.iss')
if ($LASTEXITCODE -ne 0) { throw "iscc failed with exit code $LASTEXITCODE" }
Write-Output "package: wrote $name.zip and the installer in $OutDir"

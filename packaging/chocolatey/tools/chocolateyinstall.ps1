# Chocolatey install script template (see ../ledgerly.nuspec). The checksum is the installer's
# line in the release's SHA256SUMS.txt, filled in by scripts/fill-package-manifests.sh.
$ErrorActionPreference = 'Stop'
$packageArgs = @{
  packageName    = 'ledgerly'
  fileType       = 'exe'
  url64bit       = 'https://github.com/DevanshChoubey/tally-oss/releases/download/v@VERSION@/ledgerly-@VERSION@-windows-x86_64-setup.exe'
  checksum64     = '@SHA256@'
  checksumType64 = 'sha256'
  silentArgs     = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-'
  validExitCodes = @(0)
}
Install-ChocolateyPackage @packageArgs

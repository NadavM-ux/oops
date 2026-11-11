# Oops installer for Windows.
# Downloads the latest release from GitHub and installs it for the current user (no admin needed).
# Source: https://github.com/NadavM-ux/oops
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$url = 'https://github.com/NadavM-ux/oops/releases/latest/download/Oops-windows-setup.exe'
$setup = Join-Path $env:TEMP 'Oops-setup.exe'

Write-Host 'Downloading Oops...'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -Uri $url -OutFile $setup -UseBasicParsing

Write-Host 'Installing...'
Get-Process -Name 'Oops' -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Process -FilePath $setup -ArgumentList '/S' -Wait
Remove-Item $setup -ErrorAction SilentlyContinue

$exe = Join-Path $env:LOCALAPPDATA 'Oops\Oops.exe'
if (Test-Path $exe) {
  Start-Process $exe
  Write-Host 'Done. Oops is running: look for its icon next to the clock.'
} else {
  Write-Host 'Installed. Open Oops from the Start menu.'
}

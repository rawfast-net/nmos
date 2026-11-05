# Build Linux binary (Debian 12 / glibc) via Docker on Windows
# Requires Docker Desktop running.
#
#   powershell -ExecutionPolicy Bypass -File .\build-linux.ps1
#
# Output:
#   .\dist\nmos
#   ..\nmos-linux-bin.zip

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $Root

Write-Host "==> Checking Docker daemon..." -ForegroundColor Cyan
try {
    docker info 2>$null | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "docker info failed" }
} catch {
    Write-Host "ERROR: Docker Desktop is not running." -ForegroundColor Red
    Write-Host "Start Docker Desktop and re-run: .\build-linux.ps1" -ForegroundColor Yellow
    exit 1
}

$Dist = Join-Path $Root "dist"
New-Item -ItemType Directory -Force -Path $Dist | Out-Null

Write-Host "==> Docker build (Debian 12 / bookworm)..." -ForegroundColor Cyan
$Image = "nmos-deb12:build"
docker build -t $Image --target builder -f Dockerfile .
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$Cid = docker create $Image
if (-not $Cid) {
    Write-Host "ERROR: docker create failed" -ForegroundColor Red
    exit 1
}
try {
    $OutBin = Join-Path $Dist "nmos"
    docker cp "${Cid}:/app/target/release/nmos" $OutBin
    if ($LASTEXITCODE -ne 0) { throw "docker cp failed" }
} finally {
    docker rm $Cid | Out-Null
}

$InstallSh = @"
#!/usr/bin/env bash
set -euo pipefail
DIR="`$(cd "`$(dirname "`$0")" && pwd)"
install -m 755 "`$DIR/nmos" /usr/local/bin/nmos
echo "Installed: `$(command -v nmos)"
nmos --help | head -n 8
"@
$shPath = Join-Path $Dist "install-bin.sh"
$InstallSh = $InstallSh -replace "`r`n", "`n" -replace "`r", "`n"
[System.IO.File]::WriteAllText($shPath, $InstallSh, (New-Object System.Text.UTF8Encoding $false))

$Zip = Join-Path (Split-Path $Root -Parent) "nmos-linux-bin.zip"
if (Test-Path $Zip) { Remove-Item $Zip -Force }
Compress-Archive -Path (Join-Path $Dist "*") -DestinationPath $Zip -Force

$len = (Get-Item (Join-Path $Dist "nmos")).Length
Write-Host "==> OK: dist\nmos ($len bytes)" -ForegroundColor Green
Write-Host "==> ZIP: $Zip" -ForegroundColor Green
Write-Host ""
Write-Host "On the server:" -ForegroundColor Yellow
Write-Host "  scp dist\nmos root@IP:/usr/local/bin/nmos"
Write-Host "  ssh root@IP 'chmod +x /usr/local/bin/nmos && nmos --claim-cores 4 --claim-ram-gb 8'"
Write-Host "  or: unzip nmos-linux-bin.zip && sudo bash install-bin.sh"

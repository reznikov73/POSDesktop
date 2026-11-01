[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    & cargo wix --version *> $null
    if ($LASTEXITCODE -ne 0) {
        Write-Host 'cargo-wix не установлен.' -ForegroundColor Yellow
        Write-Host '  cargo install cargo-wix'
        Write-Host '  cargo wix init    # один раз, создаёт wix\main.wxs'
        exit 1
    }

    & cargo wix --nocapture --output dist
    if ($LASTEXITCODE -ne 0) { throw 'cargo wix: сборка MSI не удалась' }

    Get-ChildItem (Join-Path $root 'dist') -Filter *.msi | ForEach-Object {
        Write-Host "artifact: $($_.FullName)" -ForegroundColor Green
    }
} finally {
    Pop-Location
}

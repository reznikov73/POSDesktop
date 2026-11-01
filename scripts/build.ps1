[CmdletBinding()]
param([switch]$NoTests)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    if (-not $NoTests) {
        Write-Host '== fmt ==' -ForegroundColor Cyan
        & cargo fmt --all
        if ($LASTEXITCODE -ne 0) { throw 'cargo fmt не удался' }
        & cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) { throw 'cargo fmt: код не отформатирован' }

        Write-Host '== clippy ==' -ForegroundColor Cyan
        & cargo clippy --workspace --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'clippy: предупреждения как ошибки' }

        Write-Host '== tests ==' -ForegroundColor Cyan
        & cargo test --workspace
        if ($LASTEXITCODE -ne 0) { throw 'тесты упали' }
    }

    Write-Host '== build ==' -ForegroundColor Cyan
    & cargo build --release -p posdesktop
    if ($LASTEXITCODE -ne 0) { throw 'сборка упала' }

    $version = ([regex]::Match((Get-Content -Raw -Encoding UTF8 (Join-Path $root 'Cargo.toml')), '(?m)^version = "([^"]+)"')).Groups[1].Value
    $dist = Join-Path $root 'dist'
    New-Item -ItemType Directory -Force $dist | Out-Null
    $source = Join-Path $root 'target\release\posdesktop.exe'
    $target = Join-Path $dist "POSDesktop-$version.exe"
    Copy-Item $source $target -Force
    $mb = [math]::Round((Get-Item $target).Length / 1MB, 2)
    Write-Host "artifact: $target ($mb MB)" -ForegroundColor Green
} finally {
    Pop-Location
}

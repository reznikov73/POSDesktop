[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Write-Host '== POS Desktop setup ==' -ForegroundColor Cyan

function Show-Tool {
    param([string]$Name, [string]$Command)
    $found = Get-Command $Command -ErrorAction SilentlyContinue
    if ($found) {
        Write-Host ("  ok    {0,-12} {1}" -f $Name, $found.Source) -ForegroundColor Green
        return $true
    }
    Write-Host ("  MISS  {0,-12} установите и повторите" -f $Name) -ForegroundColor Yellow
    return $false
}

$hasRust = Show-Tool 'rustc' 'rustc'
$hasCargo = Show-Tool 'cargo' 'cargo'
$hasGit = Show-Tool 'git' 'git'
$hasGh = Show-Tool 'gh' 'gh'

$hasWix = $false
try {
    & cargo wix --version *> $null
    $hasWix = ($LASTEXITCODE -eq 0)
} catch {
    $hasWix = $false
}
if ($hasWix) {
    Write-Host '  ok    cargo-wix' -ForegroundColor Green
} else {
    Write-Host '  MISS  cargo-wix     cargo install cargo-wix' -ForegroundColor Yellow
}

if (-not ($hasRust -and $hasCargo)) {
    throw 'Rust toolchain не найден: установите rustup'
}

Push-Location $root
try {
    if (-not (Test-Path (Join-Path $root '.git'))) {
        & git init -b main
    }

    $userName = & git config user.name
    if (-not $userName) { Write-Host '  ! git user.name не настроен' -ForegroundColor Yellow }
    $userEmail = & git config user.email
    if (-not $userEmail) { Write-Host '  ! git user.email не настроен' -ForegroundColor Yellow }

    if ($hasGh) {
        & gh auth status *> $null
        if ($LASTEXITCODE -ne 0) {
            Write-Host '  gh не авторизован: выполните gh auth login' -ForegroundColor Yellow
        } else {
            & gh repo view reznikov73/POSDesktop *> $null
            if ($LASTEXITCODE -ne 0) {
                Write-Host '  создаю публичный репозиторий reznikov73/POSDesktop'
                & gh repo create reznikov73/POSDesktop --public --source $root --remote origin --push
            } else {
                Write-Host '  репозиторий существует'
            }
        }
    } else {
        Write-Host '  gh не установлен: winget install GitHub.cli, затем gh auth login' -ForegroundColor Yellow
    }

    Write-Host ''
    Write-Host 'Дальше:'
    Write-Host '  cargo test --workspace'
    Write-Host '  cargo run -p posdesktop-d0-spike'
    Write-Host '  .\scripts\build.ps1'
} finally {
    Pop-Location
}

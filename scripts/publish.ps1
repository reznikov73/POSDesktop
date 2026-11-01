[CmdletBinding()]
param([switch]$Private)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

Push-Location $root
try {
    & git remote get-url origin *> $null
    if ($LASTEXITCODE -ne 0) {
        & gh auth status *> $null
        if ($LASTEXITCODE -ne 0) { throw 'gh не авторизован: выполните gh auth login' }
        $visibility = if ($Private) { '--private' } else { '--public' }
        & gh repo create reznikov73/POSDesktop $visibility --source $root --remote origin
        if ($LASTEXITCODE -ne 0) { throw 'не удалось создать репозиторий' }
    }

    & git push -u origin main
    if ($LASTEXITCODE -ne 0) { throw 'push не удался' }

    Write-Host 'опубликовано: https://github.com/reznikov73/POSDesktop' -ForegroundColor Green
} finally {
    Pop-Location
}

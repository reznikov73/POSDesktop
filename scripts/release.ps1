[CmdletBinding()]
param(
    [switch]$NoPush,
    [switch]$NoTests,
    [switch]$Release,
    [ValidateSet('patch', 'minor', 'major')][string]$Bump = 'patch',
    [string]$Message
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

Push-Location $root
try {
    Write-Host '== version ==' -ForegroundColor Cyan
    $version = & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'version.ps1') -Bump $Bump
    if ($LASTEXITCODE -ne 0 -or -not $version) { throw 'не удалось поднять версию' }
    Write-Host "version: $version" -ForegroundColor Green

    $buildArgs = @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'build.ps1'))
    if ($NoTests) { $buildArgs += '-NoTests' }
    & pwsh @buildArgs
    if ($LASTEXITCODE -ne 0) { throw 'сборка не удалась' }

    & pwsh -NoProfile -File (Join-Path $PSScriptRoot 'installer.ps1')
    if ($LASTEXITCODE -ne 0) {
        Write-Host 'installer: MSI не собран (cargo-wix отсутствует)' -ForegroundColor Yellow
    }

    if (-not $Message) { $Message = "chore: build $version" }

    & git add -A
    if ($LASTEXITCODE -ne 0) { throw 'git add не удался' }

    & git diff --cached --quiet
    if ($LASTEXITCODE -eq 0) {
        Write-Host 'git: нет изменений для коммита' -ForegroundColor Yellow
    } else {
        & git commit -m "$Message [v$version]"
        if ($LASTEXITCODE -ne 0) { throw 'git commit не удался' }
        Write-Host "commit: $Message [v$version]" -ForegroundColor Green
    }

    if (-not $NoPush) {
        & git push origin main
        if ($LASTEXITCODE -ne 0) {
            Write-Host 'push не выполнен: проверьте remote и авторизацию (gh auth login)' -ForegroundColor Yellow
        }
    }

    if ($Release) {
        $tag = "v$version"
        & git tag $tag
        & git push origin $tag
        $artifacts = (Get-ChildItem (Join-Path $root 'dist')).FullName
        & gh release create $tag $artifacts --title $tag --notes "POS Desktop $tag"
    }
} finally {
    Pop-Location
}

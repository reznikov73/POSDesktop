[CmdletBinding()]
param(
    [string]$Set,
    [ValidateSet('patch', 'minor', 'major')][string]$Bump = 'patch'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$path = Join-Path $root 'Cargo.toml'
$text = Get-Content -Raw -Encoding UTF8 $path

$match = [regex]::Match($text, '(?m)^version = "(\d+)\.(\d+)\.(\d+)"')
if (-not $match.Success) {
    throw 'не найден version в [workspace.package] Cargo.toml'
}

if ($Set) {
    if ($Set -notmatch '^\d+\.\d+\.\d+$') { throw "некорректная версия: $Set" }
    $new = $Set
} else {
    $major = [int]$match.Groups[1].Value
    $minor = [int]$match.Groups[2].Value
    $patch = [int]$match.Groups[3].Value
    switch ($Bump) {
        'major' { $major++; $minor = 0; $patch = 0 }
        'minor' { $minor++; $patch = 0 }
        default { $patch++ }
    }
    $new = "$major.$minor.$patch"
}

$updated = $text.Substring(0, $match.Index) + "version = `"$new`"" + $text.Substring($match.Index + $match.Length)
Set-Content -Path $path -Value $updated -Encoding utf8 -NoNewline

$wxs = Join-Path $root 'wix\main.wxs'
if (Test-Path $wxs) {
    $wixText = Get-Content -Raw -Encoding UTF8 $wxs
    $wixUpdated = [regex]::Replace($wixText, 'Version="[^"]*"', "Version=`"$new`"")
    if ($wixUpdated -ne $wixText) {
        Set-Content -Path $wxs -Value $wixUpdated -Encoding utf8 -NoNewline
    }
}

Write-Output $new

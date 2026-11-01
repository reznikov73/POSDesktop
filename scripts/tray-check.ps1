# Проверка трея и единственного экземпляра (ТЗ M1).
#
# Запускает оболочку, дергает её сообщениями, которые присылает Проводник,
# и проверяет по логу: показ панели вторым экземпляром, переключение панели из
# меню, открытие логов и выход.
#
# Usage: .\scripts\tray-check.ps1 [-Exe <путь к posdesktop.exe>]
[CmdletBinding()]
param(
    [string]$Exe = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\posdesktop.exe')
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class TrayCheck {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern uint RegisterWindowMessageW(string name);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
}
'@

$CallbackMessage = 0x8001
$WM_RBUTTONUP = 0x0205
$Broadcast = [IntPtr]0xFFFF

if (-not (Test-Path -LiteralPath $Exe)) {
    throw "не найден $Exe — сначала соберите: cargo build --release -p posdesktop"
}

# Каталог логов ищется по тому же правилу, что и в приложении: ближайший Cargo.toml выше exe.
$root = Split-Path -Parent $Exe
for ($i = 0; $i -lt 6; $i++) {
    if (Test-Path -LiteralPath (Join-Path $root 'Cargo.toml')) { break }
    $parent = Split-Path -Parent $root
    if (-not $parent) { break }
    $root = $parent
}
$log = Join-Path $root 'logs\app.log'
$failures = 0

function Wait-Log([string]$pattern, [int]$seconds = 5) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if (Get-Content -LiteralPath $log -Tail 30 -Encoding UTF8 | Select-String -Pattern $pattern) {
            return $true
        }
        Start-Sleep -Milliseconds 200
    }
    return $false
}

function Get-TrayWindow {
    $found = (Get-Content -LiteralPath $log -Tail 40 -Encoding UTF8 |
        Select-String 'трей: иконка добавлена, hwnd=0x([0-9a-f]+)').Matches
    if ($found.Count -eq 0) { return 0 }
    return [Convert]::ToInt64(($found | Select-Object -Last 1).Groups[1].Value, 16)
}

function Open-TrayMenu([int64]$hwnd) {
    [void][TrayCheck]::SetCursorPos(1000, 620)
    Start-Sleep -Milliseconds 200
    [void][TrayCheck]::PostMessage([IntPtr]$hwnd, $CallbackMessage, [IntPtr]::Zero, [IntPtr]$WM_RBUTTONUP)
    Start-Sleep -Milliseconds 700
}

Get-Process posdesktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

$proc = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 3
"pid=$($proc.Id)"

$tray = Get-TrayWindow
if ($tray -eq 0) {
    Stop-Process -Id $proc.Id -Force
    throw 'иконка трея не создана'
}
"иконка трея: hwnd=0x{0:x}" -f $tray

# 1. Второй экземпляр не создаёт окон, а просит показать панель.
$second = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 2
$secondAlive = Get-Process -Id $second.Id -ErrorAction SilentlyContinue
if ($secondAlive) { "ОШИБКА: второй экземпляр не завершился"; $failures++ }
$count = (Get-Process posdesktop -ErrorAction SilentlyContinue | Measure-Object).Count
if ($count -ne 1) { "ОШИБКА: процессов posdesktop $count, ожидался 1"; $failures++ }
if (Wait-Log 'панель открывается \(запрос от второго экземпляра\)') {
    'второй экземпляр показал панель существующей оболочки'
} else {
    'ОШИБКА: существующая оболочка не отреагировала на второй запуск'
    $failures++
}

# 2. Первый пункт меню переключает панель.
Open-TrayMenu $tray
[System.Windows.Forms.SendKeys]::SendWait('{DOWN}{ENTER}')
if (Wait-Log 'панель (закрывается|открывается) \(трей\)') {
    'меню: пункт «Показать / скрыть панели» сработал'
} else {
    'ОШИБКА: пункт меню не переключил панель'
    $failures++
}

# 3. Второй пункт открывает логи.
Open-TrayMenu $tray
[System.Windows.Forms.SendKeys]::SendWait('{DOWN}{DOWN}{ENTER}')
if (Wait-Log 'открываю логи') {
    'меню: пункт «Логи» сработал'
} else {
    'ОШИБКА: пункт «Логи» не сработал'
    $failures++
}

# 4. Третий пункт завершает оболочку.
Open-TrayMenu $tray
[System.Windows.Forms.SendKeys]::SendWait('{DOWN}{DOWN}{DOWN}{ENTER}')
Start-Sleep -Seconds 2
$left = (Get-Process posdesktop -ErrorAction SilentlyContinue | Measure-Object).Count
if ($left -eq 0) {
    'меню: пункт «Выход» завершил оболочку'
} else {
    "ОШИБКА: после «Выход» осталось процессов: $left"
    $failures++
    Get-Process posdesktop -ErrorAction SilentlyContinue | Stop-Process -Force
}

if ($failures -gt 0) {
    throw "проверка трея провалена: ошибок $failures"
}
'OK: трей, второй экземпляр и выход работают'

# Проверка горячих клавиш панелей (ТЗ §5.6).
#
# Запускает оболочку, читает из лога, какие комбинации удалось зарегистрировать,
# нажимает их синтетически и проверяет видимость окон панелей.
#
# Usage: .\scripts\hotkeys-check.ps1 [-Exe <путь к posdesktop.exe>]
[CmdletBinding()]
param(
    [string]$Exe = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\posdesktop.exe')
)

$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class Hotkeys {
    public delegate bool Proc(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] public static extern bool EnumWindows(Proc cb, IntPtr p);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);

    public const byte VK_SHIFT = 0x10;
    public const byte VK_CONTROL = 0x11;
    public const byte VK_MENU = 0x12;
    const uint KEYUP = 0x0002;

    public static IntPtr Find(uint pid, string title) {
        IntPtr found = IntPtr.Zero;
        EnumWindows(delegate(IntPtr h, IntPtr p) {
            uint owner; GetWindowThreadProcessId(h, out owner);
            if (owner != pid) return true;
            var sb = new StringBuilder(256); GetWindowTextW(h, sb, 256);
            if (sb.ToString() == title) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static void Press(bool shift, byte key) {
        if (shift) keybd_event(VK_SHIFT, 0, 0, UIntPtr.Zero);
        keybd_event(VK_CONTROL, 0, 0, UIntPtr.Zero);
        keybd_event(VK_MENU, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(80);
        keybd_event(key, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(80);
        keybd_event(key, 0, KEYUP, UIntPtr.Zero);
        keybd_event(VK_MENU, 0, KEYUP, UIntPtr.Zero);
        keybd_event(VK_CONTROL, 0, KEYUP, UIntPtr.Zero);
        if (shift) keybd_event(VK_SHIFT, 0, KEYUP, UIntPtr.Zero);
        System.Threading.Thread.Sleep(400);
    }
}
'@

if (-not (Test-Path -LiteralPath $Exe)) {
    throw "не найден $Exe — сначала соберите: cargo build --release -p posdesktop"
}

$root = Split-Path -Parent $Exe
for ($i = 0; $i -lt 6; $i++) {
    if (Test-Path -LiteralPath (Join-Path $root 'Cargo.toml')) { break }
    $parent = Split-Path -Parent $root
    if (-not $parent) { break }
    $root = $parent
}
$log = Join-Path $root 'logs\app.log'
$failures = 0

function Read-Log { Get-Content -LiteralPath $log -Tail 60 -Encoding UTF8 }

function Wait-Log([string]$pattern, [int]$seconds = 4) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if ((Read-Log) | Select-String -Pattern $pattern) { return $true }
        Start-Sleep -Milliseconds 200
    }
    return $false
}

# Возвращает $true, если комбинация зарегистрирована с Shift.
function Get-ShiftVariant([string]$label) {
    $lines = Read-Log
    if ($lines | Select-String -Pattern ([regex]::Escape("хоткей $label занят")) -Quiet) { return $true }
    return $false
}

Get-Process posdesktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

$proc = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 3
"pid=$($proc.Id)"

foreach ($line in (Read-Log | Select-String 'хоткей')) { "  лог: $line" }

$statusShift = Get-ShiftVariant 'Ctrl+Alt+3'
$servicesShift = Get-ShiftVariant 'Ctrl+Alt+4'
$allShift = Get-ShiftVariant 'Ctrl+Alt+Esc'

$status = [Hotkeys]::Find([uint32]$proc.Id, 'POS Desktop — статус')
$services = [Hotkeys]::Find([uint32]$proc.Id, 'POS Desktop — сервисы')
if ($status -eq [IntPtr]::Zero -or $services -eq [IntPtr]::Zero) {
    Stop-Process -Id $proc.Id -Force
    throw 'окна статуса и сервисов не найдены'
}

# 1. Статус: Ctrl+Alt+3 скрывает и возвращает панель.
[Hotkeys]::Press($statusShift, 0x33)
if (-not (Wait-Log 'панель статус скрыта')) { 'ОШИБКА: статус не скрылся'; $failures++ }
if ([Hotkeys]::IsWindowVisible($status)) { 'ОШИБКА: окно статуса всё ещё видно'; $failures++ }

[Hotkeys]::Press($statusShift, 0x33)
if (-not (Wait-Log 'панель статус показана')) { 'ОШИБКА: статус не вернулся'; $failures++ }
if (-not [Hotkeys]::IsWindowVisible($status)) { 'ОШИБКА: окно статуса не показалось'; $failures++ }

# 2. Сервисы: Ctrl+Alt+4.
[Hotkeys]::Press($servicesShift, 0x34)
if (-not (Wait-Log 'панель сервисы скрыта')) { 'ОШИБКА: сервисы не скрылись'; $failures++ }
[Hotkeys]::Press($servicesShift, 0x34)
if (-not (Wait-Log 'панель сервисы показана')) { 'ОШИБКА: сервисы не вернулись'; $failures++ }

# 3. Ctrl+Alt+Esc сворачивает всё.
[Hotkeys]::Press($allShift, 0x1B)
if (-not (Wait-Log 'все панели свёрнуты')) { 'ОШИБКА: свернуть всё не сработало'; $failures++ }

Stop-Process -Id $proc.Id -Force

if ($failures -gt 0) {
    throw "проверка хоткеев провалена: ошибок $failures"
}
'OK: горячие клавиши панелей работают'

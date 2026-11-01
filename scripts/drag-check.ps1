# Сквозная проверка перетаскивания плиток (ТЗ §6.1, A10).
#
# Запускает приложение с изолированным APPDATA, перетаскивает плитку weather
# мышью, проверяет привязку и сохранение в layout.json, затем перезапускает
# приложение и убеждается, что позиция восстановлена.
#
# Usage: .\scripts\drag-check.ps1 [-Exe <путь к posdesktop.exe>]
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
public class DragCheck {
    public delegate bool Proc(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] public static extern bool EnumWindows(Proc cb, IntPtr p);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, UIntPtr e);
    public const uint LEFTDOWN = 0x0002, LEFTUP = 0x0004;
    public const uint SWP_NOSIZE = 0x0001, SWP_NOMOVE = 0x0002, SWP_NOACTIVATE = 0x0010;
    static readonly IntPtr Topmost = new IntPtr(-1);

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

    public static void Bring(IntPtr h) {
        SetWindowPos(h, Topmost, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }

    public static void Drag(int fromX, int fromY, int toX, int toY, int steps) {
        SetCursorPos(fromX, fromY);
        System.Threading.Thread.Sleep(150);
        mouse_event(LEFTDOWN, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(100);
        for (int i = 1; i <= steps; i++) {
            SetCursorPos(fromX + (toX - fromX) * i / steps, fromY + (toY - fromY) * i / steps);
            System.Threading.Thread.Sleep(20);
        }
        System.Threading.Thread.Sleep(150);
        mouse_event(LEFTUP, 0, 0, 0, UIntPtr.Zero);
    }
}
'@

function Get-Tile([string]$layout, [string]$id) {
    $screen = (Get-Content -LiteralPath $layout -Raw -Encoding UTF8 | ConvertFrom-Json).monitors.primary
    $screen.objects | Where-Object id -eq $id
}

function Start-App([string]$appData) {
    $saved = $env:APPDATA
    $env:APPDATA = $appData
    try {
        return Start-Process -FilePath $Exe -PassThru -WindowStyle Hidden
    } finally {
        $env:APPDATA = $saved
    }
}

if (-not (Test-Path -LiteralPath $Exe)) {
    throw "не найден $Exe — сначала соберите: cargo build --release -p posdesktop"
}

$root = Join-Path $env:TEMP ('posdesktop-check-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force (Join-Path $root 'POSDesktop') | Out-Null
$layout = Join-Path $root 'POSDesktop\layout.json'
$failures = 0

$proc = Start-App $root
Start-Sleep -Seconds 3
$hwnd = [DragCheck]::Find([uint32]$proc.Id, 'POS Desktop — рабочая область')
if ($hwnd -eq [IntPtr]::Zero) {
    Stop-Process -Id $proc.Id -Force
    throw 'окно рабочей области не найдено'
}
[DragCheck]::Bring($hwnd)
Start-Sleep -Milliseconds 300

$before = Get-Tile $layout 'weather'
"старт: weather=($($before.x), $($before.y))"

$grabX = $before.x + 88
$grabY = $before.y + 44
# Ведём центр плитки в (704, 396): при захвате (88, 44) плитка встанет в (616, 352).
[DragCheck]::Drag($grabX, $grabY, 704, 396, 24)
Start-Sleep -Milliseconds 900

$after = Get-Tile $layout 'weather'
"после перетаскивания: weather=($($after.x), $($after.y))"
if ($after.x -ne 616 -or $after.y -ne 352) {
    "ОШИБКА: ожидалось (616, 352)"
    $failures++
}

Stop-Process -Id $proc.Id -Force
Start-Sleep -Milliseconds 600

$proc2 = Start-App $root
Start-Sleep -Seconds 3
$reloaded = Get-Tile $layout 'weather'
"после перезапуска: weather=($($reloaded.x), $($reloaded.y))"
if ($reloaded.x -ne 616 -or $reloaded.y -ne 352) {
    "ОШИБКА: позиция не восстановлена"
    $failures++
}
Stop-Process -Id $proc2.Id -Force
Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue

if ($failures -gt 0) {
    throw "проверка перетаскивания провалена: ошибок $failures"
}
"OK: перетаскивание с привязкой и сохранение работают"

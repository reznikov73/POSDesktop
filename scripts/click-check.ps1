# Проверка клика по плитке: приложение запускается, плитка не съезжает.
#
# Создаёт временный ярлык в меню «Пуск» (cmd /c exit, свёрнутый), подкладывает
# изолированную раскладку с плиткой «POS Test» и кликает по ней мышью.
#
# Usage: .\scripts\click-check.ps1 [-Exe <путь к posdesktop.exe>]
[CmdletBinding()]
param(
    [string]$Exe = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\posdesktop.exe')
)

$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class ClickCheck {
    public delegate bool Proc(IntPtr h, IntPtr p);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(Proc cb, IntPtr p);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, UIntPtr e);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT point);

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
}
'@

if (-not (Test-Path -LiteralPath $Exe)) {
    throw "не найден $Exe — сначала соберите: cargo build --release -p posdesktop"
}

$exeRoot = Split-Path -Parent $Exe
for ($i = 0; $i -lt 6; $i++) {
    if (Test-Path -LiteralPath (Join-Path $exeRoot 'Cargo.toml')) { break }
    $parent = Split-Path -Parent $exeRoot
    if (-not $parent) { break }
    $exeRoot = $parent
}
$log = Join-Path $exeRoot 'logs\app.log'

$root = Join-Path $env:TEMP ('posdesktop-click-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force (Join-Path $root 'POSDesktop') | Out-Null
$layout = Join-Path $root 'POSDesktop\layout.json'
$failures = 0

# Ярлык-пустышка, чтобы проверить запуск, ничего не открывая.
$programs = Join-Path $env:USERPROFILE 'AppData\Roaming\Microsoft\Windows\Start Menu\Programs'
$shortcut = Join-Path $programs 'POS Test App.lnk'
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($shortcut)
$link.TargetPath = "$env:SystemRoot\System32\cmd.exe"
$link.Arguments = '/c exit'
$link.WindowStyle = 7
$link.Save()

try {
    # Раскладка: первая плитка становится приложением «POS Test».
    $json = & $Exe --layout 2>$null | Out-String
    $data = $json | ConvertFrom-Json
    $tile = $data.monitors.primary.objects[0]
    $tile.type = 'app'
    $tile | Add-Member -NotePropertyName app -NotePropertyValue 'POS Test' -Force
    $tile.PSObject.Properties.Remove('widget')
    $data | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $layout -Encoding UTF8
    "плитка для клика: $($tile.id) в ($($tile.x),$($tile.y))"

    Get-Process posdesktop -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500

    $saved = $env:APPDATA
    $env:APPDATA = $root
    $proc = Start-Process -FilePath $Exe -PassThru -WindowStyle Hidden
    $env:APPDATA = $saved
    Start-Sleep -Seconds 3

    $hwnd = [ClickCheck]::Find([uint32]$proc.Id, 'POS Desktop — рабочая область')
    if ($hwnd -eq [IntPtr]::Zero) { throw 'окно рабочей области не найдено' }
    [void][ClickCheck]::SetWindowPos($hwnd, [IntPtr](-1), 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0010)
    Start-Sleep -Milliseconds 400

    $x = [int]($tile.x + $tile.w / 2)
    $y = [int]($tile.y + $tile.h / 2)
    $point = New-Object ClickCheck+POINT
    $point.X = $x
    $point.Y = $y
    $over = [ClickCheck]::WindowFromPoint($point)
    if ($over -ne $hwnd) { throw "точка клика перекрыта другим окном ($over)" }

    [void][ClickCheck]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 200
    [ClickCheck]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 120
    [ClickCheck]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 2000

    $tail = Get-Content -LiteralPath $log -Tail 12 -Encoding UTF8
    if ($tail | Select-String 'запуск POS Test') {
        'клик: приложение запущено'
    } else {
        'ОШИБКА: запуск не зафиксирован в логе'
        $failures++
    }

    $after = (Get-Content -LiteralPath $layout -Raw -Encoding UTF8 | ConvertFrom-Json).monitors.primary.objects[0]
    "после клика: плитка в ($($after.x),$($after.y))"
    if ($after.x -ne $tile.x -or $after.y -ne $tile.y) {
        'ОШИБКА: клик сдвинул плитку'
        $failures++
    }

    Stop-Process -Id $proc.Id -Force
} finally {
    Remove-Item -LiteralPath $shortcut -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}

if ($failures -gt 0) {
    throw "проверка клика провалена: ошибок $failures"
}
'OK: клик по плитке запускает приложение и не двигает её'

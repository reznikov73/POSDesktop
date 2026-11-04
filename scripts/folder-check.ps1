# Проверка раскрытия папки на месте плитки: клик раскрывает, повторный — закрывает.
[CmdletBinding()]
param(
    [string]$Exe = 'E:\Work\VSC\POSDesktop\target\release\posdesktop.exe'
)

$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class FolderTest {
    public delegate bool Proc(IntPtr h, IntPtr p);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(Proc cb, IntPtr p);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, UIntPtr e);

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

    public static void Click(int x, int y) {
        SetCursorPos(x, y);
        System.Threading.Thread.Sleep(200);
        mouse_event(0x0002, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(90);
        mouse_event(0x0004, 0, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(700);
    }
}
'@

$root = Join-Path $env:TEMP ('posdesktop-folder-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force (Join-Path $root 'POSDesktop') | Out-Null
$layout = Join-Path $root 'POSDesktop\layout.json'
$failures = 0

$word = 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Word.lnk'
$excel = 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Excel.lnk'

$json = & $Exe --layout 2>$null | Out-String
$data = $json | ConvertFrom-Json
$tile = $data.monitors.primary.objects[0]
$tile.type = 'app'
$tile | Add-Member -NotePropertyName app -NotePropertyValue $word -Force
$tile | Add-Member -NotePropertyName apps -NotePropertyValue @($excel) -Force
$tile.PSObject.Properties.Remove('widget')
$data | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $layout -Encoding UTF8

Get-Process posdesktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

$saved = $env:APPDATA
$env:APPDATA = $root
$proc = Start-Process -FilePath $Exe -PassThru -WindowStyle Hidden
$env:APPDATA = $saved
Start-Sleep -Seconds 3

$exeRoot = Split-Path -Parent $Exe
for ($i = 0; $i -lt 6; $i++) {
    if (Test-Path -LiteralPath (Join-Path $exeRoot 'Cargo.toml')) { break }
    $parent = Split-Path -Parent $exeRoot
    if (-not $parent) { break }
    $exeRoot = $parent
}
$log = Join-Path $exeRoot 'logs\app.log'

$workspace = [FolderTest]::Find([uint32]$proc.Id, 'POS Desktop — рабочая область')
if ($workspace -eq [IntPtr]::Zero) { throw 'окно рабочей области не найдено' }
[void][FolderTest]::SetWindowPos($workspace, [IntPtr](-1), 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0010)
Start-Sleep -Milliseconds 400

$x = [int]($tile.x + 44)
$y = [int]($tile.y + 44)
"клик по папке в ($x,$y)"
[FolderTest]::Click($x, $y)

$tail = Get-Content -LiteralPath $log -Tail 15 -Encoding UTF8
if ($tail | Select-String 'папка раскрыта') {
    'папка раскрылась на месте плитки'
} else {
    'ОШИБКА: в логе нет строки о раскрытии'
    $failures++
}

[FolderTest]::Click($x, $y)
if ((Get-Content -LiteralPath $log -Tail 15 -Encoding UTF8) | Select-String 'папка закрыта') {
    'повторный клик закрыл папку'
} else {
    'ОШИБКА: папка не закрылась'
    $failures++
}

Stop-Process -Id $proc.Id -Force
Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue

if ($failures -gt 0) { throw "проверка папки провалена: ошибок $failures" }
'OK: папка раскрывается на месте плитки и закрывается'

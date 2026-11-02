# Проверка окна папки: клик по плитке-папке открывает панель с приложениями.
[CmdletBinding()]
param(
    [string]$Exe = 'E:\Work\VSC\POSDesktop\target\release\posdesktop.exe',
    [string]$Shot = 'C:\Users\rez\AppData\Roaming\AutoClaw-oversea-official\accounts\a13284e151c1988d16d8ec021c6ff7e5d5f84db13f785b1d6897e218f89a4874\projects\Новый проект\shots\folder-window.png'
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class FolderCheck {
    public delegate bool Proc(IntPtr h, IntPtr p);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(Proc cb, IntPtr p);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
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
}
'@

$root = Join-Path $env:TEMP ('posdesktop-folder-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force (Join-Path $root 'POSDesktop') | Out-Null
$layout = Join-Path $root 'POSDesktop\layout.json'
$failures = 0

$excel = 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Excel.lnk'
$word = 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Word.lnk'

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

$workspace = [FolderCheck]::Find([uint32]$proc.Id, 'POS Desktop — рабочая область')
if ($workspace -eq [IntPtr]::Zero) { throw 'окно рабочей области не найдено' }
[void][FolderCheck]::SetWindowPos($workspace, [IntPtr](-1), 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0010)
Start-Sleep -Milliseconds 400

$x = [int]($tile.x + $tile.w / 2)
$y = [int]($tile.y + $tile.h / 2)
[void][FolderCheck]::SetCursorPos($x, $y)
Start-Sleep -Milliseconds 200
[FolderCheck]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 120
[FolderCheck]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 1200

$folder = [FolderCheck]::Find([uint32]$proc.Id, 'POS Desktop — папка')
"окно папки: $folder, видимо: $([FolderCheck]::IsWindowVisible($folder))"
if ($folder -eq [IntPtr]::Zero -or -not [FolderCheck]::IsWindowVisible($folder)) {
    'ОШИБКА: окно папки не открылось'
    $failures++
} else {
    $rect = New-Object FolderCheck+RECT
    [void][FolderCheck]::GetWindowRect($folder, [ref]$rect)
    "папка: ($($rect.Left),$($rect.Top)) $($rect.Right - $rect.Left)x$($rect.Bottom - $rect.Top)"
    [void][FolderCheck]::SetWindowPos($folder, [IntPtr](-1), 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0010)
    Start-Sleep -Milliseconds 400
    $bmp = New-Object System.Drawing.Bitmap ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
    $g.Dispose()
    $bmp.Save($Shot, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    "снимок: $Shot"
}

Stop-Process -Id $proc.Id -Force
Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue

if ($failures -gt 0) { throw 'проверка окна папки провалена' }
'OK: папка открывается окном с приложениями'

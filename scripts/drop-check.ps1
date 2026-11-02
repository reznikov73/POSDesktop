# Проверка приёма ярлыков из Проводника: WM_DROPFILES с настоящим HDROP.
#
# Создаёт временный ярлык, собирает структуру DROPFILES в глобальной памяти и
# посылает рабочей области WM_DROPFILES — так же, как это делает Проводник.
#
# Usage: .\scripts\drop-check.ps1 [-Exe <путь к posdesktop.exe>]
[CmdletBinding()]
param(
    [string]$Exe = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\posdesktop.exe')
)

$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class DropCheck {
    public delegate bool Proc(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] public static extern bool EnumWindows(Proc cb, IntPtr p);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
    [DllImport("kernel32.dll")] public static extern IntPtr GlobalAlloc(uint flags, UIntPtr bytes);
    [DllImport("kernel32.dll")] public static extern IntPtr GlobalLock(IntPtr memory);
    [DllImport("kernel32.dll")] public static extern bool GlobalUnlock(IntPtr memory);

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

    // DROPFILES { DWORD pFiles; POINT pt; BOOL fNC; BOOL fWide; } + список путей UTF-16
    public static IntPtr MakeDropFiles(string path) {
        byte[] text = Encoding.Unicode.GetBytes(path + "\0\0");
        int header = 20;
        int total = header + text.Length;
        IntPtr memory = GlobalAlloc(0x0042, (UIntPtr)total); // GMEM_MOVEABLE | GMEM_ZEROINIT
        if (memory == IntPtr.Zero) throw new InvalidOperationException("GlobalAlloc failed");
        IntPtr pointer = GlobalLock(memory);
        try {
            byte[] buffer = new byte[total];
            BitConverter.GetBytes(header).CopyTo(buffer, 0);   // pFiles
            BitConverter.GetBytes(1).CopyTo(buffer, 16);       // fWide = TRUE
            text.CopyTo(buffer, header);
            Marshal.Copy(buffer, 0, pointer, total);
        } finally {
            GlobalUnlock(memory);
        }
        return memory;
    }
}
'@

if (-not (Test-Path -LiteralPath $Exe)) {
    throw "не найден $Exe — сначала соберите: cargo build --release -p posdesktop"
}

$root = Join-Path $env:TEMP ('posdesktop-drop-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force (Join-Path $root 'POSDesktop') | Out-Null
$layout = Join-Path $root 'POSDesktop\layout.json'
$failures = 0

$programs = Join-Path $env:USERPROFILE 'AppData\Roaming\Microsoft\Windows\Start Menu\Programs'
$shortcut = Join-Path $programs 'POS Drop Test.lnk'
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($shortcut)
$link.TargetPath = "$env:SystemRoot\System32\cmd.exe"
$link.Arguments = '/c exit'
$link.WindowStyle = 7
$link.Save()

try {
    # Раскладка из заводской: первая плитка — точка броска.
    $json = & $Exe --layout 2>$null | Out-String
    $data = $json | ConvertFrom-Json
    $tile = $data.monitors.primary.objects[0]
    $data | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $layout -Encoding UTF8

    Get-Process posdesktop -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 500

    $saved = $env:APPDATA
    $env:APPDATA = $root
    $proc = Start-Process -FilePath $Exe -PassThru -WindowStyle Hidden
    $env:APPDATA = $saved
    Start-Sleep -Seconds 3

    $hwnd = [DropCheck]::Find([uint32]$proc.Id, 'POS Desktop — рабочая область')
    if ($hwnd -eq [IntPtr]::Zero) { throw 'окно рабочей области не найдено' }
    [void][DropCheck]::SetWindowPos($hwnd, [IntPtr](-1), 0, 0, 0, 0, 0x0001 -bor 0x0002 -bor 0x0010)
    Start-Sleep -Milliseconds 400

    $x = [int]($tile.x + $tile.w / 2)
    $y = [int]($tile.y + $tile.h / 2)
    [void][DropCheck]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 200

    $drop = [DropCheck]::MakeDropFiles($shortcut)
    $posted = [DropCheck]::PostMessage($hwnd, 0x0233, $drop, [IntPtr]::Zero)
    "WM_DROPFILES отправлен: $posted"
    Start-Sleep -Milliseconds 1500

    $after = (Get-Content -LiteralPath $layout -Raw -Encoding UTF8 | ConvertFrom-Json).monitors.primary.objects[0]
    "плитка $($after.id): apps = $($after.apps -join ', ')"
    if ($after.apps -and ($after.apps -contains $shortcut)) {
        'ярлык добавлен в плитку'
    } else {
        'ОШИБКА: ярлык не попал в плитку'
        $failures++
    }

    Stop-Process -Id $proc.Id -Force
} finally {
    Remove-Item -LiteralPath $shortcut -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}

if ($failures -gt 0) {
    throw "проверка приёма файлов провалена: ошибок $failures"
}
'OK: ярлык из Проводника добавляется в плитку'

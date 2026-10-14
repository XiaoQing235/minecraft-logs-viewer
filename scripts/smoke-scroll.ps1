$ErrorActionPreference = 'Stop'
if (!$IsWindows) { throw 'This native input check runs on Windows.' }
Set-Location (Split-Path $PSScriptRoot -Parent)
$binary = (Resolve-Path -LiteralPath 'target/release/minecraft-logs-viewer.exe').Path
$fixture = 'build/smoke-scroll/logs'
New-Item -ItemType Directory -Path $fixture -Force | Out-Null
foreach ($name in '01.log', '02.log', '05.log') {
    $text = (1..8 | ForEach-Object { "[12:34:56] [Server thread/INFO]: $name line $_ 你好" }) -join "`n"
    [IO.File]::WriteAllText((Join-Path (Get-Location) "$fixture/$name"), $text + "`n", [Text.UTF8Encoding]::new($false))
}
[IO.File]::WriteAllText((Join-Path (Get-Location) "$fixture/04-empty.log"), '')
$gzipFile = [IO.File]::Create((Join-Path (Get-Location) "$fixture/03.log.gz"))
$gzip = [IO.Compression.GZipStream]::new($gzipFile, [IO.Compression.CompressionLevel]::Optimal)
try {
    $text = ((1..8 | ForEach-Object { "[12:34:56] [Server thread/INFO]: 03.log.gz line $_" }) -join "`n") + "`n"
    $gzip.Write([Text.Encoding]::UTF8.GetBytes($text))
} finally { $gzip.Dispose(); $gzipFile.Dispose() }
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class MlvWheel {
    delegate bool EnumProc(IntPtr window, IntPtr unused);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback, IntPtr unused);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint id);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
    [StructLayout(LayoutKind.Sequential)] struct Point { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr window, ref Point point);
    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr window, uint message, IntPtr value, IntPtr position);
    public static IntPtr Find(int id) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, unused) => {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner != id) return true;
            var title = new StringBuilder(128);
            GetWindowText(window, title, title.Capacity);
            if (title.ToString() != "Minecraft Logs Viewer") return true;
            found = window;
            return false;
        }, IntPtr.Zero);
        return found;
    }
    public static void Wheel(IntPtr window, int ticks) {
        Rect rect;
        if (!GetClientRect(window, out rect)) throw new Exception("Cannot read test window");
        var point = new Point { X = rect.Right * 2 / 3, Y = rect.Bottom / 2 };
        if (!ClientToScreen(window, ref point)) throw new Exception("Cannot position wheel event");
        long position = (long)(ushort)point.X | ((long)(ushort)point.Y << 16);
        for (int i = 0; i < Math.Abs(ticks); i++) {
            int delta = ticks > 0 ? -120 : 120;
            if (!PostMessage(window, 0x020A, new IntPtr(delta << 16), new IntPtr(position)))
                throw new Exception("Cannot post wheel event");
        }
    }
    public static void DragScrollbar(IntPtr window) {
        Rect rect;
        if (!GetClientRect(window, out rect)) throw new Exception("Cannot read test window");
        int x = rect.Right * 1241 / 1280;
        int start = rect.Bottom * 289 / 820;
        int end = rect.Bottom * 525 / 820;
        var down = new IntPtr((long)(ushort)x | ((long)(ushort)start << 16));
        var move = new IntPtr((long)(ushort)x | ((long)(ushort)end << 16));
        if (!PostMessage(window, 0x0201, new IntPtr(1), down)) throw new Exception("Cannot press scrollbar");
        System.Threading.Thread.Sleep(30);
        if (!PostMessage(window, 0x0200, new IntPtr(1), move)) throw new Exception("Cannot drag scrollbar");
        System.Threading.Thread.Sleep(30);
        if (!PostMessage(window, 0x0202, IntPtr.Zero, move)) throw new Exception("Cannot release scrollbar");
    }
}
'@
function Check-Wheel([string]$name, [int]$forward, [int]$backward, [string]$expected) {
    $process = Start-Process -FilePath $binary -ArgumentList $fixture, '--smoke', '--screenshot', "build/$name.bmp" -WindowStyle Hidden -PassThru -RedirectStandardError "build/$name.txt"
    try {
        $clock = [Diagnostics.Stopwatch]::StartNew()
        $window = [IntPtr]::Zero
        while ($window -eq [IntPtr]::Zero -and $clock.ElapsedMilliseconds -lt 2000) {
            $window = [MlvWheel]::Find($process.Id)
            [Threading.Thread]::Sleep(20)
        }
        if ($window -eq [IntPtr]::Zero) { throw 'Test window not found.' }
        [Threading.Thread]::Sleep(450)
        [MlvWheel]::Wheel($window, $forward)
        if ($backward) { [Threading.Thread]::Sleep(500); [MlvWheel]::Wheel($window, -$backward) }
        if (!$process.WaitForExit(6000)) { throw 'Window smoke timed out.' }
        if ($process.ExitCode) { throw "Window smoke failed: $($process.ExitCode)" }
        $result = Get-Content "build/$name.txt" -Raw
        if ($result -notmatch $expected) { throw "Unexpected stream position: $result" }
        Write-Host "$name passed: $($result.Trim())"
    } finally { if (!$process.HasExited) { $process.Kill(); $process.WaitForExit() }; $process.Dispose() }
}
Check-Wheel 'scroll-forward' 4 0 'current=02\.log, top_line=3'
Check-Wheel 'scroll-reverse' 4 2 'current=01\.log, top_line=6'
Check-Wheel 'scroll-multiple' 16 0 'current=05\.log, top_line=8'
$long = ((1..80 | ForEach-Object { "[12:34:56] [Server thread/INFO]: 01.log line $_" }) -join "`n") + "`n"
[IO.File]::WriteAllText((Join-Path (Get-Location) "$fixture/01.log"), $long, [Text.UTF8Encoding]::new($false))
$process = Start-Process -FilePath $binary -ArgumentList $fixture, '--smoke', '--screenshot', 'build/scrollbar-local.bmp' -WindowStyle Hidden -PassThru -RedirectStandardError build/scrollbar-local.txt
try {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $window = [IntPtr]::Zero
    while ($window -eq [IntPtr]::Zero -and $clock.ElapsedMilliseconds -lt 2000) {
        $window = [MlvWheel]::Find($process.Id)
        [Threading.Thread]::Sleep(20)
    }
    if ($window -eq [IntPtr]::Zero) { throw 'Scrollbar test window not found.' }
    [Threading.Thread]::Sleep(450)
    [MlvWheel]::DragScrollbar($window)
    if (!$process.WaitForExit(6000)) { throw 'Scrollbar smoke timed out.' }
    if ($process.ExitCode) { throw 'Scrollbar smoke failed.' }
    $result = Get-Content build/scrollbar-local.txt -Raw
    if ($result -notmatch 'current=01\.log, top_line=(\d+)') { throw "Scrollbar left the current file: $result" }
    $line = [int]$Matches[1]
    if ($line -lt 35 -or $line -gt 70) { throw "Scrollbar did not drag to the expected range: $result" }
    Write-Host "scrollbar-local passed: line $line in 01.log"
} finally { if (!$process.HasExited) { $process.Kill(); $process.WaitForExit() }; $process.Dispose() }
foreach ($mode in 'working', 'executable') {
    $output = "build/autodetect-$mode.txt"
    if ($mode -eq 'working') {
        $process = Start-Process -FilePath $binary -WorkingDirectory $fixture -ArgumentList '--smoke' -WindowStyle Hidden -PassThru -RedirectStandardError $output
    } else {
        $portable = 'build/smoke-portable/logs'
        New-Item -ItemType Directory -Path $portable -Force | Out-Null
        Copy-Item -LiteralPath "$fixture/01.log" -Destination $portable -Force
        Copy-Item -LiteralPath $binary -Destination "$portable/viewer.exe" -Force
        $process = Start-Process -FilePath "$portable/viewer.exe" -ArgumentList '--smoke' -WindowStyle Hidden -PassThru -RedirectStandardError $output
    }
    try {
        if (!$process.WaitForExit(6000)) { throw 'Directory detection smoke timed out.' }
        if ($process.ExitCode) { throw "Directory detection failed: $mode" }
        $result = Get-Content -LiteralPath $output -Raw
        if ($result -notmatch 'current=01\.log') { throw "Directory not detected: $result" }
        Write-Host "autodetect-$mode passed"
    } finally { if (!$process.HasExited) { $process.Kill(); $process.WaitForExit() }; $process.Dispose() }
}

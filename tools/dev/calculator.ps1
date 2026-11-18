# calculator.ps1: drives Calculator for live tests, without touching the owner's settings.
#
#   -Start [-Era win95|win98|win2000] [-Scale 2] [-Release]
#         starts the built Calculator with its data in a scratch folder (-Data), in the look and
#         size given, and waits for its window.
#   -Keys "1 2 + 3 enter"   real key presses, one by one, each only while one of the Calculator's
#                  windows is in front: digits, + - * / . @ % r, ( ) ! # & ^, letters,
#                  enter, esc, del, back, ins, f2-f9, f12, ctrl+m, alt+v ...
#   -Click "x y; x y"   real left clicks at points of its window (1x pixels of the window, as
#                  the core counts them), each only when the Calculator is what lies there.
#   -Hold "x y"    presses the left button there and keeps it down (-Release lets go there).
#   -LetGo "x y"   lets go of the left button at that point.
#   -Look "win95 2"   what the bar does when its look or size changes (scratch settings.txt,
#                  then Bevel95's look-changed message to the Calculator).
#   -Snap <name>   photographs its windows (main, a menu, About), each its own rectangle only,
#                  into <Out>\<name>-<kind>.png.
#   -Stop          closes it (a close message to its main window).
#
# Safety: before any key or click it waits until nobody has touched the keyboard or mouse for
# 8 seconds (the owner may be using the PC), then checks that the Calculator's window is the one
# in front (keys) or under the point (clicks); only its own windows are photographed or closed.
param(
    [switch]$Start, [string]$Era = "win95", [int]$Scale = 2, [switch]$Release,
    [string]$Keys = "", [string]$Click = "", [string]$Hold = "", [string]$LetGo = "", [string]$Look = "", [string]$Snap = "", [switch]$Stop,
    [int]$Idle = 8,
    [string]$Data = (Join-Path $env:TEMP "bevel95-calculator-test"), [string]$Out = (Join-Path $env:TEMP "bevel95-calculator-test\caps")
)
Add-Type -AssemblyName System.Drawing
Add-Type -Namespace Calc -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p, IntPtr after, string c, string t);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessageW(string name);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
[StructLayout(LayoutKind.Sequential)] public struct POINT { public int x, y; }
[StructLayout(LayoutKind.Sequential)] public struct LASTINPUTINFO { public uint cbSize; public uint dwTime; }
[DllImport("user32.dll")] public static extern bool GetLastInputInfo(ref LASTINPUTINFO i);
[DllImport("kernel32.dll")] public static extern uint GetTickCount();
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
[DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
[DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
[DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
'@
[void][Calc.W]::SetProcessDpiAwarenessContext([IntPtr](-4))

$calc = Get-Process calculator -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "*\target\*" } | Select-Object -First 1
$settingsFile = Join-Path $Data "Bevel95\settings.txt"

function Get-CalcWindows {
    $found = @()
    if (-not $calc) { return $found }
    foreach ($class in 'Bevel95Window', 'Bevel95ProgramMenu') {
        $h = [IntPtr]::Zero
        while ($true) {
            $h = [Calc.W]::FindWindowExW([IntPtr]::Zero, $h, $class, [NullString]::Value)
            if ($h -eq [IntPtr]::Zero) { break }
            $owner = 0; [void][Calc.W]::GetWindowThreadProcessId($h, [ref]$owner)
            if ($owner -eq $calc.Id -and [Calc.W]::IsWindowVisible($h) -and -not [Calc.W]::IsIconic($h)) {
                $title = New-Object Text.StringBuilder 100; [void][Calc.W]::GetWindowTextW($h, $title, 100)
                $kind = if ($class -eq 'Bevel95ProgramMenu') { 'menu' } elseif ($title.ToString() -eq 'Calculator') { 'main' } else { 'box' }
                $found += [pscustomobject]@{ Handle = $h; Kind = $kind; Title = $title.ToString() }
            }
        }
    }
    return $found
}
function Get-Window([string]$kind) { Get-CalcWindows | Where-Object Kind -eq $kind | Select-Object -First 1 }
function Get-Scale { [int]((Get-Content $settingsFile | Where-Object { $_ -match '^scale' }) -replace '\D', '') }
# Waits until nobody has used the keyboard or mouse for $Idle seconds (at most 5 minutes).
function Wait-Idle {
    $deadline = (Get-Date).AddMinutes(5)
    while ((Get-Date) -lt $deadline) {
        $info = New-Object Calc.W+LASTINPUTINFO; $info.cbSize = 8
        [void][Calc.W]::GetLastInputInfo([ref]$info)
        $idleMs = [Calc.W]::GetTickCount() - $info.dwTime
        if ($idleMs -ge $Idle * 1000) { return }
        Start-Sleep -Milliseconds 500
    }
    throw "the PC was not idle for $Idle seconds within 5 minutes: nothing sent"
}
# The screen point of (x, y) in the main window; throws unless the Calculator is what lies there.
function Get-Point([string]$where) {
    $w = Get-Window 'main'
    if (-not $w) { throw "the Calculator's window is not showing" }
    $s = Get-Scale
    $r = New-Object Calc.W+RECT; [void][Calc.W]::GetWindowRect($w.Handle, [ref]$r)
    $xy = $where -split '[ ,]+'
    $p = New-Object Calc.W+POINT; $p.x = $r.L + [int]$xy[0] * $s + [int]($s / 2); $p.y = $r.T + [int]$xy[1] * $s + [int]($s / 2)
    if ([Calc.W]::GetAncestor([Calc.W]::WindowFromPoint($p), 2) -ne $w.Handle) { throw "another window covers the Calculator at ${where}: nothing done" }
    return $p
}

if ($Start) {
    if ($calc) { throw "a test Calculator is already running (process $($calc.Id))" }
    $folder = Join-Path $Data "Bevel95"
    New-Item -ItemType Directory -Force $folder | Out-Null
    [IO.File]::WriteAllText($settingsFile, "era = $Era`r`nscale = $Scale`r`n")
    $profile_ = if ($Release) { "release" } else { "debug" }
    # The build folder: wherever cargo puts it (.cargo\config.toml may move it).
    $target = (& (Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe') metadata --format-version 1 --no-deps --manifest-path (Join-Path $PSScriptRoot '..\..\Cargo.toml') | ConvertFrom-Json).target_directory
    $exe = Join-Path $target "$profile_\calculator.exe"
    $before = $env:LOCALAPPDATA
    $env:LOCALAPPDATA = $Data
    try { $calc = Start-Process -FilePath $exe -PassThru } finally { $env:LOCALAPPDATA = $before }
    for ($i = 0; $i -lt 50 -and -not (Get-Window 'main'); $i++) { Start-Sleep -Milliseconds 100 }
    $main = Get-Window 'main'
    if (-not $main) { throw "the Calculator's window did not come up" }
    $r = New-Object Calc.W+RECT; [void][Calc.W]::GetWindowRect($main.Handle, [ref]$r)
    $front = [Calc.W]::GetForegroundWindow() -eq $main.Handle
    "started: process $($calc.Id), window $($r.R - $r.L)x$($r.B - $r.T) at $($r.L),$($r.T), in front: $front, data in $folder"
}

if ($Click -or $Hold -or $LetGo) {
    $old = New-Object Calc.W+POINT; [void][Calc.W]::GetCursorPos([ref]$old)
    $done = 0
    foreach ($where in ($Click -split ';' | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
        Wait-Idle
        $p = Get-Point $where
        [void][Calc.W]::SetCursorPos($p.x, $p.y); Start-Sleep -Milliseconds 60
        [Calc.W]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 60
        [Calc.W]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 100
        $done++
    }
    if ($Hold) {
        Wait-Idle
        $p = Get-Point $Hold
        [void][Calc.W]::SetCursorPos($p.x, $p.y); Start-Sleep -Milliseconds 60
        [Calc.W]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 200
        "holding the left button at $Hold"
    }
    if ($LetGo) {
        # The button is down: the Calculator has the mouse, so only the point is checked.
        $w = Get-Window 'main'; $s = Get-Scale
        $r = New-Object Calc.W+RECT; [void][Calc.W]::GetWindowRect($w.Handle, [ref]$r)
        $xy = $LetGo -split '[ ,]+'
        [void][Calc.W]::SetCursorPos($r.L + [int]$xy[0] * $s + 1, $r.T + [int]$xy[1] * $s + 1); Start-Sleep -Milliseconds 100
        [Calc.W]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 100
        "let go at $LetGo"
    }
    if (-not $Hold) { [void][Calc.W]::SetCursorPos($old.x, $old.y) }
    if ($done) { "$done click(s)" }
}

if ($Keys) {
    $vk = @{ enter = 0x0D; esc = 0x1B; del = 0x2E; back = 0x08; f9 = 0x78; f10 = 0x79; ctrl = 0x11; shift = 0x10; alt = 0x12
             down = 0x28; up = 0x26; left = 0x25; right = 0x27; ins = 0x2D
             f2 = 0x71; f3 = 0x72; f4 = 0x73; f5 = 0x74; f6 = 0x75; f7 = 0x76; f8 = 0x77; f12 = 0x7B
             '+' = 0x6B; '-' = 0x6D; '*' = 0x6A; '/' = 0x6F; '.' = 0x6E }
    # Characters typed with Shift on a US keyboard.
    $shifted = @{ '@' = 0x32; '%' = 0x35; '(' = 0x39; ')' = 0x30; '!' = 0x31; '#' = 0x33; '&' = 0x37; '^' = 0x36 }
    Wait-Idle
    foreach ($k in $Keys -split ' ' | Where-Object { $_ }) {
        # Its main window, its open menu or its About box.
        $front = [Calc.W]::GetForegroundWindow()
        if (-not (Get-CalcWindows | Where-Object { $_.Handle -eq $front })) { throw "the Calculator is not in front: stopped before '$k'" }
        $codes = if ($shifted.ContainsKey($k)) { @(0x10, $shifted[$k]) }
                 elseif ($vk.ContainsKey($k)) { @($vk[$k]) }
                 else { @(($k -split '\+') | ForEach-Object { if ($vk.ContainsKey($_)) { $vk[$_] } else { [byte][char]$_.ToUpper() } }) }
        foreach ($c in $codes) { [Calc.W]::keybd_event([byte]$c, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 30 }
        [array]::Reverse($codes)
        foreach ($c in $codes) { [Calc.W]::keybd_event([byte]$c, 0, 2, [IntPtr]::Zero); Start-Sleep -Milliseconds 30 }
        Start-Sleep -Milliseconds 350
    }
    "keys sent: $Keys"
}

if ($Look) {
    # What the bar does when its look or size changes: the scratch settings.txt, then its message.
    $main = Get-Window 'main'
    if (-not $main) { throw "the Calculator's window is not showing" }
    $parts = $Look -split '[ ,]+'
    [IO.File]::WriteAllText($settingsFile, "era = $($parts[0])`r`nscale = $($parts[1])`r`n")
    $message = [Calc.W]::RegisterWindowMessageW('Bevel95.LookChanged')
    [void][Calc.W]::PostMessageW($main.Handle, $message, [IntPtr]::Zero, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 400
    "look changed to $Look"
}

if ($Snap) {
    New-Item -ItemType Directory -Force $Out | Out-Null
    Start-Sleep -Milliseconds 300
    foreach ($w in Get-CalcWindows) {
        $r = New-Object Calc.W+RECT; [void][Calc.W]::GetWindowRect($w.Handle, [ref]$r)
        $bmp = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
        $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size); $g.Dispose()
        $file = Join-Path $Out "$Snap-$($w.Kind).png"
        $bmp.Save($file); $bmp.Dispose()
        "$($w.Kind) `"$($w.Title)`": $($r.R - $r.L)x$($r.B - $r.T) at $($r.L),$($r.T) -> $file"
    }
}

if ($Stop) {
    $main = Get-Window 'main'
    if ($main) { [void][Calc.W]::PostMessageW($main.Handle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
    for ($i = 0; $i -lt 30 -and $calc -and (Get-Process -Id $calc.Id -ErrorAction SilentlyContinue); $i++) { Start-Sleep -Milliseconds 100 }
    if ($calc -and (Get-Process -Id $calc.Id -ErrorAction SilentlyContinue)) { "the Calculator is still running" } else { "the Calculator is closed" }
}

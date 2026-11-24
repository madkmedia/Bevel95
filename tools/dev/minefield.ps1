# minefield.ps1: drives Minesweeper for live tests, without touching the owner's settings.
#
#   -Start [-Era win95|win98|win2000] [-Scale 2] [-At "100 100"] [-Level beginner] [-Seed 7]
#          [-Fresh] [-Release]
#         starts the built game with its data in a scratch folder (-Data), in the look and size
#         given, its window at -At (screen pixels), and waits for the window. -Seed fixes where
#         the first game's mines go (core test `prints_a_seeds_field` lists them); -Fresh
#         forgets the scratch folder's best times first.
#   -Snap <name>   photographs every window of the game (main window, a menu, a box), each its
#                  own rectangle only, into <Out>\<name>-<kind>.png; prints their sizes.
#   -Click "x y" or "x y; x y; ..." [-Button left|right|middle|both] [-Down] [-Up]
#                  real mouse clicks at points of the game's main window, in 1x pixels of the
#                  window (as the core counts them). -Down / -Up: only press, only let go.
#   -Square "c r; c r; ..."   the same at the middles of squares (column, row from 0).
#   -Hover "x y"   only moves the pointer to a point of the main window, and leaves it there.
#   -BoxClick "x y"   the same in the game's open box (its dialog window).
#   -MenuClick "x y"  the same in the game's open menu.
#   -Keys "f10 down enter"   real key presses, sent only while a window of the game is in front.
#   -Look "win2000 3"   what the bar does when its look or size changes: writes the scratch
#                  settings.txt and posts Bevel95's look-changed message to the game.
#   -Stop          closes the game (a close message to its main window).
#
# Safety: only windows of the game's own process are ever clicked, photographed or closed. A
# click is made only when the game's window is what lies under that point of the screen, and
# keys are sent only while one of its windows is in front (an early run clicked the app in
# front after the game had dropped behind it: Alt+G was another program's hotkey on this PC).
param(
    [switch]$Start, [string]$Era = "win95", [int]$Scale = 2, [string]$At = "120 120", [string]$Level = "beginner",
    [string]$Seed = "", [switch]$Fresh, [switch]$Release, [string]$Snap = "", [string]$Click = "", [string]$Square = "",
    [string]$BoxClick = "", [string]$MenuClick = "", [string]$Hover = "", [string]$Button = "left", [switch]$Down, [switch]$Up,
    [string]$Keys = "", [string]$Look = "", [switch]$Stop,
    [string]$Data = (Join-Path $env:TEMP "bevel95-minefield-test"), [string]$Out = (Join-Path $env:TEMP "bevel95-minefield-test\caps")
)
Add-Type -AssemblyName System.Drawing
. (Join-Path $PSScriptRoot 'lines-pause.ps1')
Add-Type -Namespace Mf -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p, IntPtr after, string c, string t);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessageW(string name);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
[StructLayout(LayoutKind.Sequential)] public struct POINT { public int x, y; }
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
[DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
[DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
[DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
'@
[void][Mf.W]::SetProcessDpiAwarenessContext([IntPtr](-4))

$game = Get-Process minefield -ErrorAction SilentlyContinue | Select-Object -First 1
$settingsFile = Join-Path $Data "Bevel95\settings.txt"

# Every visible window of the game's process with one of Bevel95's window classes.
function Get-GameWindows {
    $found = @()
    if (-not $game) { return $found }
    foreach ($class in 'Bevel95Window', 'Bevel95ProgramMenu') {
        $h = [IntPtr]::Zero
        while ($true) {
            $h = [Mf.W]::FindWindowExW([IntPtr]::Zero, $h, $class, [NullString]::Value)
            if ($h -eq [IntPtr]::Zero) { break }
            $owner = 0; [void][Mf.W]::GetWindowThreadProcessId($h, [ref]$owner)
            if ($owner -eq $game.Id -and [Mf.W]::IsWindowVisible($h) -and -not [Mf.W]::IsIconic($h)) {
                $title = New-Object Text.StringBuilder 100; [void][Mf.W]::GetWindowTextW($h, $title, 100)
                $kind = if ($class -eq 'Bevel95ProgramMenu') { 'menu' } elseif ($title.ToString() -eq 'Minesweeper') { 'main' } else { 'box' }
                $found += [pscustomobject]@{ Handle = $h; Kind = $kind; Title = $title.ToString() }
            }
        }
    }
    return $found
}
function Get-Window([string]$kind) { Get-GameWindows | Where-Object Kind -eq $kind | Select-Object -First 1 }
function Get-Scale { [int]((Get-Content $settingsFile | Where-Object { $_ -match '^scale' }) -replace '\D', '') }

if ($Start) {
    if ($game) { throw "Minesweeper is already running (process $($game.Id))" }
    $folder = Join-Path $Data "Bevel95"
    New-Item -ItemType Directory -Force $folder | Out-Null
    [IO.File]::WriteAllText($settingsFile, "era = $Era`r`nscale = $Scale`r`n")
    $saved = Join-Path $folder "minefield.txt"
    if ($Fresh -and (Test-Path $saved)) { Remove-Item $saved }
    $old = if (Test-Path $saved) { @(Get-Content $saved | Where-Object { $_ -notmatch '^(window|level)\s*=' }) } else { @() }
    [IO.File]::WriteAllLines($saved, @($old) + @("window = $At", "level = $Level"))
    $profile_ = if ($Release) { "release" } else { "debug" }
    # The build folder: wherever cargo puts it (.cargo\config.toml may move it).
    $target = (& (Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe') metadata --format-version 1 --no-deps --manifest-path (Join-Path $PSScriptRoot '..\..\Cargo.toml') | ConvertFrom-Json).target_directory
    $exe = Join-Path $target "$profile_\minefield.exe"
    $before = $env:LOCALAPPDATA
    $env:LOCALAPPDATA = $Data
    if ($Seed) { $env:BEVEL95_MINEFIELD_SEED = $Seed }
    try { $game = Start-Process -FilePath $exe -PassThru } finally { $env:LOCALAPPDATA = $before; $env:BEVEL95_MINEFIELD_SEED = $null }
    for ($i = 0; $i -lt 50 -and -not (Get-Window 'main'); $i++) { Start-Sleep -Milliseconds 100 }
    $main = Get-Window 'main'
    if (-not $main) { throw "the game's window did not come up" }
    $r = New-Object Mf.W+RECT; [void][Mf.W]::GetWindowRect($main.Handle, [ref]$r)
    "started: process $($game.Id), window $($r.R - $r.L)x$($r.B - $r.T) at $($r.L),$($r.T), data in $folder"
}

# Real clicks at points (1x pixels) of one of the game's windows.
function Press([string]$kind, [string]$points) {
    $w = Get-Window $kind
    if (-not $w) { throw "the game has no $kind window open" }
    $s = Get-Scale
    $old = New-Object Mf.W+POINT; [void][Mf.W]::GetCursorPos([ref]$old)
    $flags = switch ($Button) { 'right' { @(0x0008), @(0x0010) } 'middle' { @(0x0020), @(0x0040) } 'both' { @(0x0002, 0x0008), @(0x0010, 0x0004) } default { @(0x0002), @(0x0004) } }
    $done = 0
    foreach ($where in ($points -split ';' | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
        $r = New-Object Mf.W+RECT; [void][Mf.W]::GetWindowRect($w.Handle, [ref]$r)
        $xy = $where -split '[ ,]+'
        $x = $r.L + [int]$xy[0] * $s + [int]($s / 2); $y = $r.T + [int]$xy[1] * $s + [int]($s / 2)
        # Never click anything but the game: the window under the point must be the one meant.
        $target = New-Object Mf.W+POINT; $target.x = $x; $target.y = $y
        $under = [Mf.W]::GetAncestor([Mf.W]::WindowFromPoint($target), 2)
        if ($under -ne $w.Handle) { [void][Mf.W]::SetCursorPos($old.x, $old.y); throw "another window covers the game's $kind window at ${where}: nothing was clicked there ($done clicks made before)" }
        [void][Mf.W]::SetCursorPos($x, $y); Start-Sleep -Milliseconds 60
        if (-not $Up) { foreach ($f in $flags[0]) { [Mf.W]::mouse_event($f, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 40 } }
        if (-not $Down) { foreach ($f in $flags[1]) { [Mf.W]::mouse_event($f, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 40 } }
        Start-Sleep -Milliseconds 60
        $done++
    }
    if (-not $Down) { [void][Mf.W]::SetCursorPos($old.x, $old.y) }
    "$done $Button click(s) in the $kind window"
}
if ($Hover) {
    # Only moves the pointer (and leaves it there): onto a menu title, say.
    $w = Get-Window 'main'
    if (-not $w) { throw "the game's window is not showing" }
    $s = Get-Scale
    $r = New-Object Mf.W+RECT; [void][Mf.W]::GetWindowRect($w.Handle, [ref]$r)
    $xy = $Hover -split '[ ,]+'
    $target = New-Object Mf.W+POINT; $target.x = $r.L + [int]$xy[0] * $s + [int]($s / 2); $target.y = $r.T + [int]$xy[1] * $s + [int]($s / 2)
    if ([Mf.W]::GetAncestor([Mf.W]::WindowFromPoint($target), 2) -ne $w.Handle) { throw "another window covers the game's window at ${Hover}: the pointer was not moved" }
    # In two steps, so that the window sees the pointer move.
    [void][Mf.W]::SetCursorPos($target.x - 2, $target.y); Start-Sleep -Milliseconds 60
    [void][Mf.W]::SetCursorPos($target.x, $target.y); Start-Sleep -Milliseconds 200
    "pointer at $Hover of the main window"
}
if ($Click) { Press 'main' $Click }
if ($Square) {
    # All three editions start their 16x16 squares at (15, 96) in window coordinates.
    $points = ($Square -split ';' | ForEach-Object { $_.Trim() } | Where-Object { $_ } | ForEach-Object { $cr = $_ -split '[ ,]+'; "$(15 + 16 * [int]$cr[0] + 8) $(96 + 16 * [int]$cr[1] + 8)" }) -join '; '
    Press 'main' $points
}
if ($BoxClick) { Press 'box' $BoxClick }
if ($MenuClick) { Press 'menu' $MenuClick }

if ($Keys) {
    $front = [Mf.W]::GetForegroundWindow()
    if (-not (Get-GameWindows | Where-Object { $_.Handle -eq $front })) { throw "no window of the game is in front: no key sent" }
    Start-Sleep -Milliseconds 150
    $vk = @{ tab = 0x09; space = 0x20; enter = 0x0D; esc = 0x1B; down = 0x28; up = 0x26; left = 0x25; right = 0x27; ctrl = 0x11; shift = 0x10; alt = 0x12; back = 0x08; del = 0x2E; home = 0x24; end = 0x23; f1 = 0x70; f2 = 0x71; f4 = 0x73; f10 = 0x79 }
    foreach ($k in $Keys -split ' ') {
        $codes = @(($k -split '\+') | ForEach-Object { if ($vk.ContainsKey($_)) { $vk[$_] } else { [byte][char]$_.ToUpper() } })
        foreach ($c in $codes) { [Mf.W]::keybd_event([byte]$c, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 30 }
        [array]::Reverse($codes)
        foreach ($c in $codes) { [Mf.W]::keybd_event([byte]$c, 0, 2, [IntPtr]::Zero); Start-Sleep -Milliseconds 30 }
        Start-Sleep -Milliseconds 150
    }
    "keys sent: $Keys"
}

if ($Look) {
    # The main window, minimized or not (the game must follow the look then too).
    $mainHandle = [IntPtr]::Zero
    if ($game) {
        $h = [IntPtr]::Zero
        while ($true) {
            $h = [Mf.W]::FindWindowExW([IntPtr]::Zero, $h, 'Bevel95Window', 'Minesweeper')
            if ($h -eq [IntPtr]::Zero) { break }
            $owner = 0; [void][Mf.W]::GetWindowThreadProcessId($h, [ref]$owner)
            if ($owner -eq $game.Id) { $mainHandle = $h; break }
        }
    }
    if ($mainHandle -eq [IntPtr]::Zero) { throw "the game is not running" }
    $parts = $Look -split '[ ,]+'
    [IO.File]::WriteAllText($settingsFile, "era = $($parts[0])`r`nscale = $($parts[1])`r`n")
    $message = [Mf.W]::RegisterWindowMessageW('Bevel95.LookChanged')
    [void][Mf.W]::PostMessageW($mainHandle, $message, [IntPtr]::Zero, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 400
    "look changed to $Look"
}

if ($Snap) {
    New-Item -ItemType Directory -Force $Out | Out-Null
    Start-Sleep -Milliseconds 300
    $n = 0
    foreach ($w in Get-GameWindows) {
        $r = New-Object Mf.W+RECT; [void][Mf.W]::GetWindowRect($w.Handle, [ref]$r)
        $bmp = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
        # Without Bevel95's Monitor Lines, if they are on (lines-pause.ps1).
        Suspend-MonitorLines
        $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size); $g.Dispose()
        Resume-MonitorLines
        $file = Join-Path $Out "$Snap-$($w.Kind).png"
        $bmp.Save($file); $bmp.Dispose()
        "$($w.Kind) `"$($w.Title)`": $($r.R - $r.L)x$($r.B - $r.T) at $($r.L),$($r.T) -> $file"
        $n++
    }
    if ($n -eq 0) { "no window of the game is showing" }
}

if ($Stop) {
    $main = Get-GameWindows | Where-Object Kind -eq 'main' | Select-Object -First 1
    if ($main) { [void][Mf.W]::PostMessageW($main.Handle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
    elseif ($game) { "the game's window is not showing (minimized?): stopping its process"; Stop-Process -Id $game.Id }
    for ($i = 0; $i -lt 30 -and (Get-Process minefield -ErrorAction SilentlyContinue); $i++) { Start-Sleep -Milliseconds 100 }
    if (Get-Process minefield -ErrorAction SilentlyContinue) { "the game is still running" } else { "the game is closed" }
}

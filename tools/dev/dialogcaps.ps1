# dialogcaps.ps1 <output folder>: opens each of the bar's dialog windows by a message to the bar
# (Run, Log Off, Shut Down: Start menu entries 8, 10, 11 of the Windows 98 look; Taskbar
# Properties and its two pages; Bevel95 Settings; Date/Time Properties and its Time Zone page),
# photographs each window exactly (DPI aware, only its own rectangle) and closes it again. For
# proving that a change leaves the windows pixel for pixel as they were.
#
# Safety: a window is found by Bevel95's own class names, never by "whatever is in front";
# only such a window is ever closed, and Ctrl+Tab (a real key press, for the next page) is only
# sent while a Bevel95 window has the focus. (An earlier version closed the window in front
# when a dialog had not come up: that was the Claude app's.)
# -NoKeys leaves out the two Ctrl+Tab presses (the second pages): no real key is sent then.
param([Parameter(Mandatory)][string]$Out, [int[]]$StartEntries = @(8, 10, 11), [switch]$NoKeys)
Add-Type -AssemblyName System.Drawing
Add-Type -Namespace Dc -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string c, string t);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p, IntPtr after, string c, string t);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessageW(string name);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
'@
[void][Dc.W]::SetProcessDpiAwarenessContext([IntPtr](-4))
New-Item -ItemType Directory -Force $Out | Out-Null
$bar = [Dc.W]::FindWindowW('Bevel95Bar', [NullString]::Value)
if ($bar -eq [IntPtr]::Zero) { throw 'no Bevel95 bar running' }
# The bar's messages by their registered names (decisions.md, "Desktop icons"; tools\dev\postbar.ps1).
$WM_BAR_COMMAND = [Dc.W]::RegisterWindowMessageW('Bevel95.Bar.Command')
$WM_START_ACTION = [Dc.W]::RegisterWindowMessageW('Bevel95.Bar.StartAction')
$WM_OPEN_PROPERTIES = [Dc.W]::RegisterWindowMessageW('Bevel95.Bar.OpenProperties')
$classes = @('Bevel95Window', 'Bevel95Dialog', 'Bevel95Properties', 'Bevel95DateTime')

# A window of the bar's own process only: Bevel95's other programs (the Minefield game) use the
# same window class, and must never be photographed as a dialog, let alone closed.
$barProcess = 0; [void][Dc.W]::GetWindowThreadProcessId($bar, [ref]$barProcess)
function Find-Ours {
    foreach ($c in $classes) {
        $h = [IntPtr]::Zero
        while ($true) {
            $h = [Dc.W]::FindWindowExW([IntPtr]::Zero, $h, $c, [NullString]::Value)
            if ($h -eq [IntPtr]::Zero) { break }
            $owner = 0; [void][Dc.W]::GetWindowThreadProcessId($h, [ref]$owner)
            if ($owner -eq $barProcess -and [Dc.W]::IsWindowVisible($h)) { return $h }
        }
    }
    return [IntPtr]::Zero
}
function Snap([string]$name) {
    $h = [IntPtr]::Zero
    for ($i = 0; $i -lt 20 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep -Milliseconds 100; $h = Find-Ours }
    if ($h -eq [IntPtr]::Zero) { Write-Host "  ${name}: no Bevel95 window came up"; return [IntPtr]::Zero }
    Start-Sleep -Milliseconds 500
    $r = New-Object Dc.W+RECT; [void][Dc.W]::GetWindowRect($h, [ref]$r)
    $bmp = New-Object Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
    $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size); $g.Dispose()
    $bmp.Save((Join-Path $Out "$name.png")); $bmp.Dispose()
    Write-Host "  ${name}: $($r.R - $r.L)x$($r.B - $r.T)"
    return $h
}
function CtrlTab([IntPtr]$h) {
    if ($NoKeys) { Write-Host '  (-NoKeys: no key sent)'; return }
    if ($h -eq [IntPtr]::Zero -or [Dc.W]::GetForegroundWindow() -ne $h) { Write-Host '  (not in front: no key sent)'; return }
    [Dc.W]::keybd_event(0x11, 0, 0, [IntPtr]::Zero); [Dc.W]::keybd_event(0x09, 0, 0, [IntPtr]::Zero)
    [Dc.W]::keybd_event(0x09, 0, 2, [IntPtr]::Zero); [Dc.W]::keybd_event(0x11, 0, 2, [IntPtr]::Zero)
}
function Close-Ours {
    for ($i = 0; $i -lt 5; $i++) {
        $h = Find-Ours
        if ($h -eq [IntPtr]::Zero) { return }
        [void][Dc.W]::PostMessageW($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 400
    }
}

Close-Ours
$names = @{ 8 = 'run'; 10 = 'logoff'; 11 = 'shutdown' }
foreach ($e in $StartEntries) {
    [void][Dc.W]::PostMessageW($bar, $WM_START_ACTION, [IntPtr]$e, [IntPtr]::Zero)
    [void](Snap $names[$e])
    Close-Ours
}
[void][Dc.W]::PostMessageW($bar, $WM_OPEN_PROPERTIES, [IntPtr]::Zero, [IntPtr]::Zero)
$h = Snap 'properties-1'
CtrlTab $h; [void](Snap 'properties-2')
Close-Ours
# Bevel95 Settings: the bar menu's command 5 (BarCommand::Settings in core/src/bar_menu.rs).
[void][Dc.W]::PostMessageW($bar, $WM_BAR_COMMAND, [IntPtr]5, [IntPtr]::Zero)
[void](Snap 'settings')
Close-Ours
[void][Dc.W]::PostMessageW($bar, $WM_BAR_COMMAND, [IntPtr]4, [IntPtr]::Zero)
$h = Snap 'datetime-1'
CtrlTab $h; [void](Snap 'datetime-2')
Close-Ours

# Live check of the volume control on the running bar, from a DPI-aware process (messages from a
# DPI-unaware one get their coordinates scaled). Usage: volcheck.ps1 [-Era 0|1|2] (switches the
# look first, if given). Prints the popup's rectangle, the pointer and the screen, then checks
# that a click on the clock and a second click on the speaker each close it.
param([int]$Era = -1)
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern IntPtr SetProcessDpiAwarenessContext(IntPtr v);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessageW(string name);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}
'@
[void][W]::SetProcessDpiAwarenessContext([IntPtr]-4)
# The look by its registered name ("Bevel95.Bar.Theme", decisions.md; it was WM_APP + 7).
$WM_BAR_THEME = [W]::RegisterWindowMessageW('Bevel95.Bar.Theme')
$bar = [W]::FindWindow('Bevel95Bar', [NullString]::Value)
if ($bar -eq [IntPtr]::Zero) { 'no bar'; exit 1 }
if ($Era -ge 0) { [void][W]::PostMessage($bar, $WM_BAR_THEME, [IntPtr]$Era, [IntPtr]0); Start-Sleep -Seconds 2 }
$r = New-Object W+RECT; [void][W]::GetWindowRect($bar, [ref]$r)
$w = $r.R - $r.L; $h = $r.B - $r.T
# Find the speaker: its yellow (#FFFF00) pixels in the right third of the bar.
$bmp = New-Object Drawing.Bitmap $w, $h
$g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size); $g.Dispose()
$xs = @(); $ys = @()
for ($y = 0; $y -lt $h; $y++) { for ($x = [int]($w * 2 / 3); $x -lt $w; $x++) { $c = $bmp.GetPixel($x, $y); if ($c.R -eq 255 -and $c.G -eq 255 -and $c.B -eq 0) { $xs += $x; $ys += $y } } }
$bmp.Dispose()
if (-not $xs) { 'speaker not found'; exit 1 }
$sx = [int](($xs | Measure-Object -Average).Average); $sy = [int](($ys | Measure-Object -Average).Average)
"bar: ($($r.L),$($r.T)) ${w}x$h; speaker at client ($sx,$sy)"
function Click([int]$x, [int]$y) {
  $l = [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF))
  [void][W]::PostMessage($bar, 0x0201, [IntPtr]1, $l); Start-Sleep -Milliseconds 60
  [void][W]::PostMessage($bar, 0x0202, [IntPtr]0, $l)
}
function Popup { [W]::FindWindow('Bevel95Volume', [NullString]::Value) }
Click $sx $sy; Start-Sleep -Milliseconds 1200
$p = Popup
if ($p -eq [IntPtr]::Zero) { 'popup did not open'; exit 1 }
$pr = New-Object W+RECT; [void][W]::GetWindowRect($p, [ref]$pr)
$cur = New-Object W+POINT; [void][W]::GetCursorPos([ref]$cur)
"popup: ($($pr.L),$($pr.T))-($($pr.R),$($pr.B)); pointer ($($cur.X),$($cur.Y)); screen $([W]::GetSystemMetrics(0))x$([W]::GetSystemMetrics(1))"
# A click on the clock (the right end of the bar, left of its edge) closes it.
Click ($w - 12) ([int]($h / 2)); Start-Sleep -Milliseconds 500
"after a click on the clock: " + $(if ([W]::IsWindow($p)) { 'STILL OPEN' } else { 'closed' })
# Open again, then a click on the speaker closes it (and does not reopen it).
Click $sx $sy; Start-Sleep -Milliseconds 1200
$p = Popup
if ($p -eq [IntPtr]::Zero) { 'popup did not reopen'; exit 1 }
Click $sx $sy; Start-Sleep -Milliseconds 1200
"after a second click on the speaker: " + $(if ([W]::IsWindow($p) -or (Popup) -ne [IntPtr]::Zero) { 'STILL OPEN' } else { 'closed' })

# How long the volume control takes to appear after a click on the speaker (posted to the
# running bar from a DPI-aware process). Prints, per run: when the window existed and when
# its pixels first showed on the screen, counted from the button's release.
param([int]$Runs = 3, [switch]$Real)
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class T {
  [DllImport("user32.dll")] public static extern IntPtr SetProcessDpiAwarenessContext(IntPtr v);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetDoubleClickTime();
  [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h);
  [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr dc);
  [DllImport("gdi32.dll")] public static extern uint GetPixel(IntPtr dc, int x, int y);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
'@
[void][T]::SetProcessDpiAwarenessContext([IntPtr]-4)
"double-click time: $([T]::GetDoubleClickTime()) ms"
$bar = [T]::FindWindow('Bevel95Bar', [NullString]::Value)
if ($bar -eq [IntPtr]::Zero) { 'no bar'; exit 1 }
$r = New-Object T+RECT; [void][T]::GetWindowRect($bar, [ref]$r)
$w = $r.R - $r.L; $h = $r.B - $r.T
$bmp = New-Object Drawing.Bitmap $w, $h
$g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size); $g.Dispose()
$xs = @(); $ys = @()
for ($y = 0; $y -lt $h; $y++) { for ($x = [int]($w * 2 / 3); $x -lt $w; $x++) { $c = $bmp.GetPixel($x, $y); if ($c.R -eq 255 -and $c.G -eq 255 -and $c.B -eq 0) { $xs += $x; $ys += $y } } }
$bmp.Dispose()
if (-not $xs) { 'speaker not found'; exit 1 }
$sx = [int](($xs | Measure-Object -Average).Average); $sy = [int](($ys | Measure-Object -Average).Average)
function Post([uint32]$m, [int]$x, [int]$y, [int]$wp) {
  if ($Real) {
    # A real button press or release at that point of the bar (screen coordinates).
    [void][T]::SetCursorPos($r.L + $x, $r.T + $y)
    $flag = switch ($m) { 0x0201 { 0x0002 } 0x0202 { 0x0004 } }
    [T]::mouse_event($flag, 0, 0, 0, [IntPtr]::Zero)
  } else { [void][T]::PostMessage($bar, $m, [IntPtr]$wp, [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF))) }
}
$back = New-Object T+POINT; [void][T]::GetCursorPos([ref]$back)
$dc = [T]::GetDC([IntPtr]::Zero)
for ($run = 1; $run -le $Runs; $run++) {
  Post 0x0201 $sx $sy 1; Start-Sleep -Milliseconds 40
  $clock = [Diagnostics.Stopwatch]::StartNew()
  Post 0x0202 $sx $sy 0
  $p = [IntPtr]::Zero
  while ($p -eq [IntPtr]::Zero -and $clock.ElapsedMilliseconds -lt 5000) { $p = [T]::FindWindow('Bevel95Volume', [NullString]::Value); if ($p -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 2 } }
  $exists = $clock.ElapsedMilliseconds
  if ($p -eq [IntPtr]::Zero) { "run ${run}: no volume control after 5 s"; continue }
  $pr = New-Object T+RECT; [void][T]::GetWindowRect($p, [ref]$pr)
  # Its title text area: dark pixels of "Volume" appear once it has painted.
  $before = @(); $pts = @()
  for ($i = 0; $i -lt 40; $i++) { $pts += ,@(($pr.L + 4 + ($i * 3) % ($pr.R - $pr.L - 8)), ($pr.T + 4 + [int]($i / 2) % ($pr.B - $pr.T - 8))) }
  $first = (& { foreach ($q in $pts) { [T]::GetPixel($dc, $q[0], $q[1]) } }) -join ','
  $painted = $null
  while ($clock.ElapsedMilliseconds -lt 5000) {
    $now = (& { foreach ($q in $pts) { [T]::GetPixel($dc, $q[0], $q[1]) } }) -join ','
    if ($now -ne $first) { $painted = $clock.ElapsedMilliseconds; break }
    Start-Sleep -Milliseconds 2
  }
  "run ${run}: window after $exists ms, pixels changed after $(if ($painted) { "$painted ms" } else { 'no change seen (it may have painted before the first look)' })"
  Start-Sleep -Milliseconds 300
  # Close it with a click on the clock (the bar's right end).
  Post 0x0201 ($w - 12) ([int]($h / 2)) 1; Start-Sleep -Milliseconds 40; Post 0x0202 ($w - 12) ([int]($h / 2)) 0
  Start-Sleep -Milliseconds 700
  if ([T]::IsWindow($p)) { "  (still open)" }
}
[void][T]::ReleaseDC([IntPtr]::Zero, $dc)
if ($Real) { [void][T]::SetCursorPos($back.X, $back.Y) }


# How quickly the running bar answers a message, sampled every 20 ms for a while: shows whether
# its thread is sometimes busy (a busy bar delays timers such as the volume control's).
param([int]$Seconds = 20)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class P {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c, string t);
  [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint m, IntPtr w, IntPtr l, uint flags, uint ms, out IntPtr result);
}
'@
$bar = [P]::FindWindow('Bevel95Bar', [NullString]::Value)
if ($bar -eq [IntPtr]::Zero) { 'no bar'; exit 1 }
$samples = New-Object System.Collections.Generic.List[double]
$slow = @()
$clock = [Diagnostics.Stopwatch]::StartNew()
$end = $Seconds * 1000
while ($clock.ElapsedMilliseconds -lt $end) {
  $t0 = $clock.Elapsed.TotalMilliseconds
  $res = [IntPtr]::Zero
  [void][P]::SendMessageTimeout($bar, 0, [IntPtr]::Zero, [IntPtr]::Zero, 0, 5000, [ref]$res)
  $dt = $clock.Elapsed.TotalMilliseconds - $t0
  $samples.Add($dt)
  if ($dt -gt 30) { $slow += ("{0,7:N0} ms into the run: {1:N0} ms" -f $t0, $dt) }
  Start-Sleep -Milliseconds 20
}
$sorted = $samples | Sort-Object
"samples: $($samples.Count); median {0:N1} ms; 99th percentile {1:N1} ms; slowest {2:N0} ms" -f $sorted[[int]($sorted.Count / 2)], $sorted[[int]($sorted.Count * 0.99)], $sorted[-1]
"answers slower than 30 ms: $($slow.Count)"
$slow | Select-Object -First 40

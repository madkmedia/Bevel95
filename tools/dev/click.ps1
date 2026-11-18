# click.ps1 <x> <y> <button: left|right> [stay]: a real mouse click at screen pixel (x, y)
# (DPI aware), then the pointer goes back where it was unless "stay" is given.
param([int]$x, [int]$y, [string]$button = "left", [string]$stay = "")
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public static class Clk {
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int x, y; }
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
}
"@
[void][Clk]::SetProcessDpiAwarenessContext([IntPtr](-4))
$old = New-Object Clk+POINT
[void][Clk]::GetCursorPos([ref]$old)
[void][Clk]::SetCursorPos($x, $y)
Start-Sleep -Milliseconds 80
if ($button -eq "right") { [Clk]::mouse_event(0x0008, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 60; [Clk]::mouse_event(0x0010, 0, 0, 0, [IntPtr]::Zero) }
else { [Clk]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 60; [Clk]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero) }
Start-Sleep -Milliseconds 80
if ($stay -ne "stay") { [void][Clk]::SetCursorPos($old.x, $old.y) }
"clicked $button at $x,$y"

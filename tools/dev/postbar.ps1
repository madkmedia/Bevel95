# postbar.ps1 <name> [wParam] [lParam]: posts one of the bar's own messages to the running bar, by
# its registered name, from a DPI-aware process. The bar's messages that do or change something
# have been registered by name since the fourth safety review of 2026-11-18 (C3); their names and
# old WM_APP numbers are listed in docs/decisions.md ("Desktop icons", the registered messages).
# The name may be given whole or without "Bevel95.Bar.":
#   tools\dev\postbar.ps1 Command 5        # BarCommand 5 (Bevel95 Settings), core/src/bar_menu.rs
#   tools\dev\postbar.ps1 StartAction 8    # Start menu entry 8 (Run, 98's look)
#   tools\dev\postbar.ps1 OpenProperties   # Taskbar Properties
# Only a window of class Bevel95Bar whose process is bevel95.exe is posted to.
param([Parameter(Mandatory)][string]$Name, [long]$WParam = 0, [long]$LParam = 0)
Add-Type -Namespace Pb -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string c, string t);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessageW(string name);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
'@
[void][Pb.W]::SetProcessDpiAwarenessContext([IntPtr](-4))
$full = if ($Name.StartsWith('Bevel95.')) { $Name } else { "Bevel95.Bar.$Name" }
$message = [Pb.W]::RegisterWindowMessageW($full)
if ($message -eq 0) { throw "the name $full could not be registered" }
$bar = [Pb.W]::FindWindowW('Bevel95Bar', [NullString]::Value)
if ($bar -eq [IntPtr]::Zero) { throw 'no Bevel95 bar running' }
$owner = 0; [void][Pb.W]::GetWindowThreadProcessId($bar, [ref]$owner)
$process = Get-Process -Id $owner -ErrorAction SilentlyContinue
if (-not $process -or $process.ProcessName -ne 'bevel95') { throw 'the Bevel95Bar window is not bevel95.exe''s' }
if (-not [Pb.W]::PostMessageW($bar, $message, [IntPtr]$WParam, [IntPtr]$LParam)) { throw "posting $full failed" }
Write-Host "posted $full ($WParam, $LParam) to the bar"

# testicon.ps1 <log> [seconds]: shows a notification icon ("Bevel95 test icon") for a while and
# writes every mouse event it gets to <log>. For testing Bevel95's notification area without
# touching anyone's programs.
param([string]$log, [int]$seconds = 120)
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$icon = New-Object System.Windows.Forms.NotifyIcon
$icon.Icon = [System.Drawing.SystemIcons]::Information
$icon.Text = "Bevel95 test icon"
$write = { param($what) Add-Content -Path $log -Value ("{0:HH:mm:ss.fff} {1}" -f (Get-Date), $what) }
$icon.add_MouseDown({ param($s, $e) & $write "down $($e.Button)" })
$icon.add_MouseUp({ param($s, $e) & $write "up $($e.Button)" })
$icon.add_MouseClick({ param($s, $e) & $write "click $($e.Button)" })
$icon.add_MouseDoubleClick({ param($s, $e) & $write "double $($e.Button)" })
$icon.add_MouseMove({ param($s, $e) & $write "move" })
$icon.Visible = $true
& $write "shown"
$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = $seconds * 1000
$timer.add_Tick({ $icon.Visible = $false; $icon.Dispose(); & $write "gone"; [System.Windows.Forms.Application]::Exit() })
$timer.Start()
[System.Windows.Forms.Application]::Run()

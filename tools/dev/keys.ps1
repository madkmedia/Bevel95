# keys.ps1 "tab tab space ctrl+tab alt+t down space enter": real key presses (keybd_event),
# 150 ms apart, to whatever window has the focus.
param([string]$seq)
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public static class Kb { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra); }
"@
$vk = @{ tab = 0x09; space = 0x20; enter = 0x0D; esc = 0x1B; down = 0x28; up = 0x26; left = 0x25; right = 0x27; ctrl = 0x11; shift = 0x10; alt = 0x12 }
foreach ($k in $seq -split ' ') {
    $parts = $k -split '\+'
    $codes = @($parts | ForEach-Object { if ($vk.ContainsKey($_)) { $vk[$_] } else { [byte][char]$_.ToUpper() } })
    foreach ($c in $codes) { [Kb]::keybd_event([byte]$c, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 30 }
    [array]::Reverse($codes)
    foreach ($c in $codes) { [Kb]::keybd_event([byte]$c, 0, 2, [IntPtr]::Zero); Start-Sleep -Milliseconds 30 }
    Start-Sleep -Milliseconds 150
}
"keys sent: $seq"

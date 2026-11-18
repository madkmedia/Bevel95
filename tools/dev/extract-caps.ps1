# Pulls IMG:<name>:<base64>; segments out of a saved javascript_tool result and writes
# <name>.png files into private\reference\refs\v86 (exact emulator canvas pixels).
param([Parameter(Mandatory)] [string]$Result, [string]$OutDir = (Join-Path $(if ($env:BEVEL95_PRIVATE) { $env:BEVEL95_PRIVATE } else { "$PSScriptRoot\..\..\private" }) "reference\refs\v86"))
New-Item -ItemType Directory -Force $OutDir | Out-Null
$text = [System.IO.File]::ReadAllText($Result)
$found = [regex]::Matches($text, 'IMG:([\w.-]+):([A-Za-z0-9+/=]+);')
foreach ($m in $found) {
    $name = $m.Groups[1].Value
    $bytes = [Convert]::FromBase64String($m.Groups[2].Value)
    $path = Join-Path $OutDir "$name.png"
    [System.IO.File]::WriteAllBytes($path, $bytes)
    Add-Type -AssemblyName System.Drawing
    $img = [System.Drawing.Image]::FromFile($path)
    "$name.png  $($img.Width)x$($img.Height)  $($bytes.Length) bytes"
    $img.Dispose()
}
if ($found.Count -eq 0) { "no IMG segments found" }

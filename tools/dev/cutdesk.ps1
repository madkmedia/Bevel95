# Cuts a desktop icon out of a Reference PC capture: its colors from the capture (the PC's
# shell draws a file's icon through its own image list, which can round colors), which pixels
# are see-through from the icon in Windows' file (a PNG written by tools\dev\icons.ps1). Checks
# that every see-through pixel shows the desktop color in the capture, and says how many visible
# pixels differ from the file. Writes a PNG with transparency (for tools\dev\icon2rust.ps1).
#
#   powershell -NoProfile -File tools\dev\cutdesk.ps1 <capture> <x> <y> <desktop hex> <file icon png> <out png>
param(
    [Parameter(Mandatory)][string]$Capture, [Parameter(Mandatory)][int]$X, [Parameter(Mandatory)][int]$Y,
    [Parameter(Mandatory)][string]$Desk, [Parameter(Mandatory)][string]$Mask, [Parameter(Mandatory)][string]$Out
)
Add-Type -AssemblyName System.Drawing
$c = [Drawing.Bitmap]::FromFile((Resolve-Path $Capture).Path)
$m = [Drawing.Bitmap]::FromFile((Resolve-Path $Mask).Path)
$d = [Convert]::ToInt32($Desk, 16)
$w = $m.Width; $h = $m.Height
$o = New-Object Drawing.Bitmap($w, $h, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
$badClear = 0; $recolored = 0; $deskInside = 0
for ($j = 0; $j -lt $h; $j++) { for ($i = 0; $i -lt $w; $i++) {
    $cap = $c.GetPixel($X + $i, $Y + $j).ToArgb() -band 0xFFFFFF
    $f = $m.GetPixel($i, $j)
    if ($f.A -eq 0) {
        if ($cap -ne $d) { $badClear++ }
        $o.SetPixel($i, $j, [Drawing.Color]::FromArgb(0, 0, 0, 0))
    } else {
        if ($cap -ne ($f.ToArgb() -band 0xFFFFFF)) { $recolored++ }
        if ($cap -eq $d) { $deskInside++ }
        $o.SetPixel($i, $j, [Drawing.Color]::FromArgb(0xFF000000 -bor $cap))
    }
} }
$o.Save($Out, [Drawing.Imaging.ImageFormat]::Png)
"{0}: see-through but not desktop-colored {1}; visible and colored differently from the file {2}; visible in the desktop color {3}" -f (Split-Path $Out -Leaf), $badClear, $recolored, $deskInside
$o.Dispose(); $m.Dispose(); $c.Dispose()

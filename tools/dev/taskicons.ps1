# taskicons.ps1 <capture.png> [barTop]: finds the task buttons on a classic taskbar capture
# (800 wide) and prints each button's left edge, width and whether it is pressed, plus where
# its 16x16 icon sits (2px bevel + 2px padding in; pressed buttons one row lower).
param([string]$Path, [int]$BarTop = 572, [string]$Face = "C0C0C0")
Add-Type -AssemblyName System.Drawing
$b = [Drawing.Bitmap]::FromFile($Path)
$y = $BarTop + 4   # the buttons' top edge
$px = { param($x, $yy) $b.GetPixel($x, $yy).ToArgb() -band 0xFFFFFF }
$x = 56
$found = @()
while ($x -lt $b.Width - 70) {
    $c = & $px $x $y
    $left = & $px ($x - 1) $y
    # a button starts where its top edge (white when raised, black when pressed) begins after face gray
    if (($c -eq 0xFFFFFF -or $c -eq 0x000000 -or $c -eq 0x404040) -and $left -eq [Convert]::ToInt32($Face, 16)) {
        $start = $x
        while ($x -lt $b.Width -and (& $px $x $y) -eq $c) { $x++ }
        $w = $x - $start + 1
        $pressed = $c -ne 0xFFFFFF
        $found += [pscustomobject]@{ X = $start; W = $w; Pressed = $pressed; IconX = $start + 4; IconY = $BarTop + 7 + [int]$pressed }
        continue
    }
    $x++
}
$b.Dispose()
$found | Format-Table -AutoSize | Out-String


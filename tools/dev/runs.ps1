# runs.ps1 <in.png> <x> <w> <y> [y...]: prints each given row from x to x+w-1 as runs of equal
# color ("x0-x1 RRGGBB"), for reading exact positions without counting letters (see pixels.ps1).
# -Skip RRGGBB leaves runs of that color out (the background). -Column: the numbers after <w>
# are columns and <x> <w> a range of rows (runs read downwards).
param([Parameter(Mandatory, Position = 0)][string]$in, [Parameter(Mandatory, Position = 1)][int]$x,
    [Parameter(Mandatory, Position = 2)][int]$w,
    [Parameter(Mandatory, Position = 3, ValueFromRemainingArguments)][int[]]$rows, [string]$Skip = '', [switch]$Column)
Add-Type -AssemblyName System.Drawing
$src = [Drawing.Bitmap]::FromFile((Resolve-Path $in).Path)
foreach ($y in $rows) {
    $runs = @(); $start = $x; $last = $null
    for ($i = $x; $i -le $x + $w; $i++) {
        $inside = if ($Column) { $i -lt $x + $w -and $i -lt $src.Height } else { $i -lt $x + $w -and $i -lt $src.Width }
        $hex = if ($inside) { $c = if ($Column) { $src.GetPixel($y, $i) } else { $src.GetPixel($i, $y) }; '{0:X2}{1:X2}{2:X2}' -f $c.R, $c.G, $c.B } else { $null }
        if ($hex -ne $last) {
            if ($null -ne $last -and $last -ne $Skip) { $runs += ('{0}-{1} {2}' -f $start, ($i - 1), $last) }
            $start = $i; $last = $hex
        }
    }
    '{0,4}: {1}' -f $y, ($runs -join ' | ')
}
$src.Dispose()

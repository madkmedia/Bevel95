# colors.ps1 <in.png> <x> <y> <w> <h>: the distinct colors in a region, most frequent first, with
# how many pixels have each and the first pixel that has it.
param([Parameter(Mandatory)][string]$in, [Parameter(Mandatory)][int]$x, [Parameter(Mandatory)][int]$y,
    [Parameter(Mandatory)][int]$w, [Parameter(Mandatory)][int]$h)
Add-Type -AssemblyName System.Drawing
$src = [Drawing.Bitmap]::FromFile((Resolve-Path $in).Path)
$count = @{}; $first = @{}
for ($j = $y; $j -lt $y + $h; $j++) {
    for ($i = $x; $i -lt $x + $w; $i++) {
        $c = $src.GetPixel($i, $j); $hex = '{0:X2}{1:X2}{2:X2}' -f $c.R, $c.G, $c.B
        if ($count.ContainsKey($hex)) { $count[$hex]++ } else { $count[$hex] = 1; $first[$hex] = "$i,$j" }
    }
}
$src.Dispose()
$count.GetEnumerator() | Sort-Object Value -Descending | ForEach-Object { '{0}  {1,6}  first at {2}' -f $_.Key, $_.Value, $first[$_.Key] }

# pixels.ps1 <in.png> <x> <y> <w> <h>: prints the region as hex colors, one row per line, with a legend
# (each distinct color gets a letter) so layouts can be read exactly.
param([string]$in, [int]$x, [int]$y, [int]$w, [int]$h)
Add-Type -AssemblyName System.Drawing
$src = [Drawing.Bitmap]::FromFile($in)
$letters = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'
$map = [ordered]@{}
$rows = @()
for ($j = 0; $j -lt $h; $j++) {
    $line = ''
    for ($i = 0; $i -lt $w; $i++) {
        $c = $src.GetPixel($x + $i, $y + $j)
        $hex = '{0:X2}{1:X2}{2:X2}' -f $c.R, $c.G, $c.B
        if (-not $map.Contains($hex)) { $map[$hex] = $letters[[Math]::Min($map.Count, $letters.Length - 1)] }
        $line += $map[$hex]
    }
    $rows += ('{0,4} {1}' -f ($y + $j), $line)
}
$src.Dispose()
$hdr = ''; for ($i = 0; $i -lt $w; $i++) { $hdr += [string](($x + $i) % 10) }
'     ' + $hdr
$rows
''
$map.GetEnumerator() | ForEach-Object { '{0} = #{1}' -f $_.Value, $_.Key }

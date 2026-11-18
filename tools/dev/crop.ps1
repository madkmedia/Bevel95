# crop.ps1 <in.png> <out.png> <x> <y> <w> <h> [scale]: cuts a region and enlarges it (nearest neighbour).
param([string]$in, [string]$out, [int]$x, [int]$y, [int]$w, [int]$h, [int]$scale = 4)
Add-Type -AssemblyName System.Drawing
$src = [Drawing.Bitmap]::FromFile($in)
$bmp = New-Object Drawing.Bitmap -ArgumentList ($w * $scale), ($h * $scale)
for ($j = 0; $j -lt $h; $j++) {
    for ($i = 0; $i -lt $w; $i++) {
        $c = $src.GetPixel($x + $i, $y + $j)
        for ($dy = 0; $dy -lt $scale; $dy++) { for ($dx = 0; $dx -lt $scale; $dx++) { $bmp.SetPixel($i * $scale + $dx, $j * $scale + $dy, $c) } }
    }
}
$src.Dispose()
$bmp.Save($out, [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
"wrote $out"

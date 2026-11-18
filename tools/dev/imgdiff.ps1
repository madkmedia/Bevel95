# imgdiff.ps1 <folder A> <folder B>: compares the PNGs of the same name in two folders pixel for
# pixel and prints, per picture, how many pixels differ and the box around them (1x pixels are
# not assumed: the pictures are compared as captured).
param([Parameter(Mandatory)][string]$A, [Parameter(Mandatory)][string]$B)
Add-Type -AssemblyName System.Drawing
foreach ($f in Get-ChildItem $A -Filter *.png) {
    $other = Join-Path $B $f.Name
    if (-not (Test-Path $other)) { "{0,-16} missing in B" -f $f.Name; continue }
    $x = [Drawing.Bitmap]::FromFile($f.FullName); $y = [Drawing.Bitmap]::FromFile((Resolve-Path $other).Path)
    if ($x.Width -ne $y.Width -or $x.Height -ne $y.Height) {
        "{0,-16} size differs: {1}x{2} vs {3}x{4}" -f $f.Name, $x.Width, $x.Height, $y.Width, $y.Height
    } else {
        $rect = New-Object Drawing.Rectangle 0, 0, $x.Width, $x.Height
        $dx = $x.LockBits($rect, 'ReadOnly', 'Format32bppArgb'); $dy = $y.LockBits($rect, 'ReadOnly', 'Format32bppArgb')
        $n = $dx.Stride * $x.Height
        $bx = New-Object byte[] $n; $by = New-Object byte[] $n
        [Runtime.InteropServices.Marshal]::Copy($dx.Scan0, $bx, 0, $n); [Runtime.InteropServices.Marshal]::Copy($dy.Scan0, $by, 0, $n)
        $x.UnlockBits($dx); $y.UnlockBits($dy)
        $diff = 0; $l = [int]::MaxValue; $t = [int]::MaxValue; $r = -1; $bo = -1
        for ($i = 0; $i -lt $n; $i += 4) {
            if ($bx[$i] -ne $by[$i] -or $bx[$i + 1] -ne $by[$i + 1] -or $bx[$i + 2] -ne $by[$i + 2]) {
                $diff++; $px = ($i % $dx.Stride) / 4; $py = [math]::Floor($i / $dx.Stride)
                $l = [math]::Min($l, $px); $t = [math]::Min($t, $py); $r = [math]::Max($r, $px); $bo = [math]::Max($bo, $py)
            }
        }
        if ($diff -eq 0) { "{0,-16} identical" -f $f.Name } else { "{0,-16} {1} pixels differ, in x {2}..{3}, y {4}..{5}" -f $f.Name, $diff, $l, $r, $t, $bo }
    }
    $x.Dispose(); $y.Dispose()
}

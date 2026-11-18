# Turns 16x16 pictures (or a PNG of any square size, such as 32x32 desktop icons) into `Rows`
# constants for bevel-art (art/src, palette letters, '.' = see-through); give each a line in its
# module's ENTRIES, and the core looks it up (ClassicIcon::art). Windows' pictures go in
# art/ only.
# Each source is NAME=PATH (a PNG written by tools\dev\icons.ps1, transparency kept) or
# NAME=PATH@X,W,DX (a W-pixel-wide slice of a strip bitmap starting at X, magenta FF00FF
# see-through, placed DX pixels from the left of a 16x16 picture).
#
#   powershell -NoProfile -File tools\dev\icon2rust.ps1 -Out snippet.rs NAME=file.png NAME2=strip.bmp@0,9,3
param([Parameter(Mandatory)][string]$Out, [Parameter(ValueFromRemainingArguments)][string[]]$Sources)

Add-Type -AssemblyName System.Drawing
$text = New-Object Text.StringBuilder
foreach ($s in $Sources) {
    $name, $spec = $s -split '=', 2
    $path, $slice = $spec -split '@', 2
    $b = [Drawing.Bitmap]::FromFile((Resolve-Path $path).Path)
    $n = if ($slice) { 16 } else { $b.Width }
    $grid = New-Object 'string[,]' $n, $n
    if ($slice) {
        $x0, $w, $dx = $slice -split ',' | ForEach-Object { [int]$_ }
        for ($y = 0; $y -lt 16; $y++) { for ($i = 0; $i -lt $w; $i++) {
            $c = $b.GetPixel($x0 + $i, $y).ToArgb() -band 0xFFFFFF
            if ($c -ne 0xFF00FF) { $grid[$y, ($dx + $i)] = '{0:X6}' -f $c }
        } }
    } else {
        for ($y = 0; $y -lt $n; $y++) { for ($x = 0; $x -lt $n; $x++) {
            $c = $b.GetPixel($x, $y)
            if ($c.A -ne 0) { $grid[$y, $x] = '{0:X6}' -f ($c.ToArgb() -band 0xFFFFFF) }
        } }
    }
    $b.Dispose()
    $palette = [ordered]@{}
    # Letters for the colors: a-z, then A-Z, then digits (enough for a 256-color icon's few dozen).
    $letters = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!#$%&()*+,-/:;<=>?@[]^_{|}~'
    $rows = for ($y = 0; $y -lt $n; $y++) {
        $row = ''
        for ($x = 0; $x -lt $n; $x++) {
            $c = $grid[$y, $x]
            if (-not $c) { $row += '.'; continue }
            if (-not $palette.Contains($c)) { $palette[$c] = $letters[$palette.Count] }
            $row += $palette[$c]
        }
        $row
    }
    $pal = ($palette.GetEnumerator() | ForEach-Object { "(b'$($_.Value)', 0x$($_.Key))" }) -join ', '
    [void]$text.AppendLine("#[rustfmt::skip]")
    [void]$text.AppendLine("const ${name}: Rows = Rows {")
    [void]$text.AppendLine("    w: $n,")
    [void]$text.AppendLine("    h: $n,")
    [void]$text.AppendLine("    palette: &[$pal],")
    [void]$text.AppendLine("    rows: &[")
    foreach ($r in $rows) { [void]$text.AppendLine("        `"$r`",") }
    [void]$text.AppendLine("    ],")
    [void]$text.AppendLine("};")
    [void]$text.AppendLine()
}
[IO.File]::WriteAllText($Out, $text.ToString())
"wrote $($Sources.Count) pictures to $Out"

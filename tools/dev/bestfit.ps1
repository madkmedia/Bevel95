# bestfit.ps1: writes core\src\best_fit.rs, Windows' own "best fit" from Unicode to code page
# 1252 (Windows-1252, what the glyph atlases hold), asked of Windows itself. Windows 95 read
# window titles through this conversion: many characters became a near one (U+0141 L, U+0107 c,
# U+2011 -), the rest "?". For each character U+0100..U+FFFF (not the surrogates) it calls
# WideCharToMultiByte(1252, 0, ...) and keeps the pairs whose result is one byte, is not "?",
# and is not the character itself (those are in the atlas anyway). The byte is written as the
# Unicode character it stands for in code page 1252, the one the atlas has a glyph for.
# Run from the repository root: powershell -NoProfile -ExecutionPolicy Bypass -File tools\dev\bestfit.ps1
Add-Type -Namespace Bf -Name W -MemberDefinition @'
[DllImport("kernel32.dll")] public static extern int WideCharToMultiByte(uint cp, uint flags, ushort[] w, int wn,
    byte[] mb, int mbn, IntPtr def, out int usedDefault);
[DllImport("kernel32.dll")] public static extern int MultiByteToWideChar(uint cp, uint flags, byte[] mb, int mbn,
    [Out] ushort[] w, int wn);
'@
# (ushort arrays, not char arrays: P/Invoke would convert a char array to ANSI on the way in.)
$pairs = New-Object System.Collections.Generic.List[string]
$one = New-Object uint16[] 1
$bytes = New-Object byte[] 8
$back = New-Object uint16[] 2
foreach ($code in 0x100..0xFFFF) {
    if ($code -ge 0xD800 -and $code -le 0xDFFF) { continue }
    $one[0] = [uint16]$code
    $used = 0
    $n = [Bf.W]::WideCharToMultiByte(1252, 0, $one, 1, $bytes, 8, [IntPtr]::Zero, [ref]$used)
    if ($n -ne 1 -or $used -ne 0 -or $bytes[0] -eq 0x3F) { continue }
    $m = [Bf.W]::MultiByteToWideChar(1252, 0, $bytes, 1, $back, 2)
    if ($m -ne 1) { continue }
    $to = [int]$back[0]
    if ($to -eq $code) { continue }
    $pairs.Add(('(0x{0:X4}, 0x{1:X4})' -f $code, $to))
}
$lines = New-Object System.Text.StringBuilder
$head = @'
//! Windows' "best fit" from Unicode to code page 1252, the characters the glyph atlases hold
//! (`bevel-winkit`'s `font::gdi::charset_1252`). Windows 95 read window titles through this conversion,
//! so a character it could not show became a near one where Windows had one (U+0141 as "L",
//! U+0107 as "c", a non-breaking hyphen as "-"), and "?" otherwise. Generated from Windows
//! itself by `tools/dev/bestfit.ps1` (WideCharToMultiByte(1252, 0, ...) for U+0100..U+FFFF):
//! do not edit by hand. Each pair is (character, the character of code page 1252 it becomes),
//! sorted by the first.

/// Windows' best-fit character for `ch` in code page 1252, if it has one.
pub fn best_fit(ch: char) -> Option<char> {
    let code = u16::try_from(u32::from(ch)).ok()?;
    let at = BEST_FIT.binary_search_by_key(&code, |&(from, _)| from).ok()?;
    char::from_u32(u32::from(BEST_FIT[at].1))
}

#[rustfmt::skip]
const BEST_FIT: [(u16, u16); COUNT] = [

'@
[void]$lines.Append($head.Replace('COUNT', [string]$pairs.Count))
for ($i = 0; $i -lt $pairs.Count; $i += 6) {
    $row = $pairs.GetRange($i, [Math]::Min(6, $pairs.Count - $i)) -join ', '
    [void]$lines.Append("    $row,`n")
}
[void]$lines.Append("];`n")
$text = $lines.ToString().Replace("`r`n", "`n")
$out = Join-Path (Get-Location) 'core\src\best_fit.rs'
[IO.File]::WriteAllText($out, $text)
"wrote $out ($($pairs.Count) pairs)"

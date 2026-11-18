# no-test-build.ps1: dot-sourced by install.ps1 and release.ps1. A program built by `cargo test`
# (its dev-dependencies switch on winkit's feature `test-scratch`) keeps everything in a scratch
# folder under %TEMP% instead of %LOCALAPPDATA%\Bevel95, and `cargo test --release` puts such
# programs at target\release\<name>.exe, where a release build is looked for (tried 2026-11-18,
# the third safety review's suspected 1: `cargo build --release` afterwards puts the plain ones
# back). So every program is checked for the tests' scratch code before it is installed or
# packed; the mark is the scratch folder's name, which only that code holds.

$script:TestBuildMark = [Text.Encoding]::UTF8.GetBytes('bevel95-tests-localappdata')

function Test-HoldsTestBuildMark([string]$file) {
    $bytes = [IO.File]::ReadAllBytes($file)
    $mark = $script:TestBuildMark
    $i = [Array]::IndexOf($bytes, $mark[0])
    while ($i -ge 0 -and $i -le $bytes.Length - $mark.Length) {
        $same = $true
        for ($j = 1; $j -lt $mark.Length; $j++) { if ($bytes[$i + $j] -ne $mark[$j]) { $same = $false; break } }
        if ($same) { return $true }
        $i = [Array]::IndexOf($bytes, $mark[0], $i + 1)
    }
    return $false
}

# Stops (throws) with a plain message if any of `files` is a test build.
function Assert-NoTestBuild([string[]]$files, [string]$what) {
    $tests = @($files | Where-Object { Test-HoldsTestBuildMark $_ } | ForEach-Object { Split-Path -Leaf $_ })
    if ($tests.Count -gt 0) {
        $is = if ($tests.Count -eq 1) { 'is a test build' } else { 'are test builds' }
        throw "Nothing was $what`: $($tests -join ', ') $is (made by cargo test, which keeps its files in a scratch folder instead of %LOCALAPPDATA%\Bevel95). Run cargo build --release and try again."
    }
}

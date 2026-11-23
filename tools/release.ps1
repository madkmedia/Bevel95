# release.ps1: makes the one file a stranger downloads (docs/decisions.md, "Bevel95 Setup").
#   - builds every program in release mode
#   - appends the five companions to the bar's own program, each with its SHA-256 fingerprint
#     (core/src/package.rs says how; core/examples/pack.rs does it and reads the result back)
#   - writes it to private\dist\<build>\Bevel95-<version>.exe and prints its size and SHA-256
# Nothing is installed, started or published, and nothing that is already in private\ is changed:
# if that file exists, the script stops (move it away first).
#
# Run it (from anywhere):
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools\release.ps1

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$manifest = Join-Path $root "Cargo.toml"
# cargo takes .cargo\config.toml (the statically linked C runtime) from the folder it is started
# in, not from the manifest's folder: start it in the project's root, wherever this was run from.
Set-Location -LiteralPath $root

$meta = & $cargo metadata --format-version 1 --no-deps --manifest-path $manifest | ConvertFrom-Json
$version = ($meta.packages | Where-Object { $_.name -eq "bevel95" }).version
$build = (& git -C $root rev-parse --short=7 HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or !$build) { $build = "unknown" }
if (& git -C $root status --porcelain --untracked-files=no) {
    Write-Warning "The working tree has changes that are not committed: the programs will say build $build, but hold more than that commit."
}
$dist = Join-Path $root "private\dist\$build"
$out = Join-Path $dist "Bevel95-$version.exe"
if (Test-Path -LiteralPath $out) { throw "$out exists already; move it away first (nothing was changed)." }

& $cargo build --workspace --release --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { throw "The build failed; no release was made." }
$release = Join-Path $meta.target_directory "release"
$companions = @("bevel-help.exe", "folders.exe", "minefield.exe", "solitaire.exe", "calculator.exe")
$files = @("bevel95.exe") + $companions | ForEach-Object { Join-Path $release $_ }
foreach ($file in $files) { if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw "The build did not produce $file." } }
# Never a program built by cargo test (the third safety review of 2026-11-18, suspected 1); the
# packing checks again (core/examples/pack.rs).
. (Join-Path $PSScriptRoot "no-test-build.ps1")
Assert-NoTestBuild $files "packed"

New-Item -ItemType Directory -Force $dist | Out-Null
& $cargo run --quiet --release --manifest-path $manifest -p bevel-core --example pack -- $out @files
if ($LASTEXITCODE -ne 0) { throw "The one file could not be made." }

$item = Get-Item -LiteralPath $out
$hash = (Get-FileHash -LiteralPath $out -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host ""
Write-Host "Made $($item.FullName)"
Write-Host ("  {0:N0} bytes ({1:N1} MB)" -f $item.Length, ($item.Length / 1MB))
Write-Host "  SHA-256 $hash"

# The portable zip beside it (the owner, 2026-11-23: "Yes, add the zip"): unzipped, exactly the
# folder Setup's Portable choice makes (docs/decisions.md, "Bevel95 Setup"): bevel95.exe (the bar as
# a plain program), programs\ with the five companions, and settings.txt with Bevel95's mark
# (Bevel95's defaults, as Setup writes it: `pack --settings`). Made with .NET's own zip classes, the
# entries in a fixed order and dated with the commit's time, so the same commit gives the same zip.
$zip = Join-Path $dist "Bevel95-$version-portable.zip"
if (Test-Path -LiteralPath $zip) { throw "$zip exists already; move it away first (the one file was made)." }
$settings = Join-Path $dist "settings.txt.portable"
& $cargo run --quiet --release --manifest-path $manifest -p bevel-core --example pack -- --settings $settings
if ($LASTEXITCODE -ne 0) { throw "The portable settings.txt could not be made." }
$time = [long](& git -C $root log -1 --format=%ct HEAD)
$when = [DateTimeOffset]::FromUnixTimeSeconds($time)
$entries = @(, @("bevel95.exe", (Join-Path $release "bevel95.exe")))
foreach ($c in $companions) { $entries += , @("programs/$c", (Join-Path $release $c)) }
$entries += , @("settings.txt", $settings)
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$stream = [IO.File]::Open($zip, [IO.FileMode]::CreateNew)
try {
    $archive = New-Object IO.Compression.ZipArchive($stream, [IO.Compression.ZipArchiveMode]::Create)
    foreach ($e in $entries) {
        $entry = $archive.CreateEntry($e[0], [IO.Compression.CompressionLevel]::Optimal)
        $entry.LastWriteTime = $when
        $w = $entry.Open()
        $bytes = [IO.File]::ReadAllBytes($e[1])
        $w.Write($bytes, 0, $bytes.Length)
        $w.Dispose()
    }
    $archive.Dispose()
} finally { $stream.Dispose() }
Remove-Item -LiteralPath $settings

# The zip checks its own work: each program equals the one file's part of the same name (by the
# fingerprints `pack --check` prints), the bar's stamp says this version and commit, settings.txt
# holds Bevel95's mark, and nothing else is in it.
$parts = @{}
$stampVersion = ''; $stampBuild = ''
foreach ($line in (& $cargo run --quiet --release --manifest-path $manifest -p bevel-core --example pack -- --check $out)) {
    if ($line -match '^\s+(\S+)\s+\d+ bytes\s+sha256 ([0-9a-f]{64})') { $parts[$Matches[1]] = $Matches[2] }
    if ($line -match 'build stamp: version (\S+) build (\S+)') { $stampVersion = $Matches[1]; $stampBuild = $Matches[2] }
}
if ($stampVersion -ne $version -or $stampBuild -ne $build) { throw "The bar's stamp says '$stampVersion $stampBuild', not '$version $build'." }
$check = [IO.Compression.ZipFile]::OpenRead($zip)
try {
    $names = @($check.Entries | ForEach-Object FullName)
    $want = @($entries | ForEach-Object { $_[0] })
    if (($names -join '|') -ne ($want -join '|')) { throw "The zip holds $($names -join ', ') instead of $($want -join ', ')." }
    $sha = [Security.Cryptography.SHA256]::Create()
    foreach ($entry in $check.Entries) {
        $s = $entry.Open(); $ms = New-Object IO.MemoryStream; $s.CopyTo($ms); $s.Dispose()
        $bytes = $ms.ToArray()
        if ($entry.FullName -eq 'settings.txt') {
            if (-not [Text.Encoding]::UTF8.GetString($bytes).StartsWith('# Bevel95 settings')) { throw "The zip's settings.txt has no mark." }
            continue
        }
        $digest = -join ($sha.ComputeHash($bytes) | ForEach-Object { $_.ToString('x2') })
        if ($parts[$entry.FullName] -ne $digest) { throw "The zip's $($entry.FullName) is not the one file's part." }
    }
} finally { $check.Dispose() }
$zipItem = Get-Item -LiteralPath $zip
Write-Host ""
Write-Host "Made $($zipItem.FullName)"
Write-Host ("  {0:N0} bytes ({1:N1} MB); {2} files, each the one file's own; dated {3:u}" -f $zipItem.Length, ($zipItem.Length / 1MB), $entries.Count, $when.UtcDateTime)
Write-Host "  SHA-256 $((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant())"

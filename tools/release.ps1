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

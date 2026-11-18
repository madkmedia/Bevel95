# install.ps1: builds Bevel95 and installs it for everyday use.
#   - copies the program to %LOCALAPPDATA%\Bevel95\bevel95.exe (next to settings.txt and the log),
#     and its bundled applications in the programs subfolder
#   - puts two shortcuts on the desktop: "Bevel95" (start it) and
#     "Restore Windows taskbar" (the emergency button: bevel95 --restore)
# If Bevel95 is running, it is closed first (a running program can't be replaced; closing
# brings the Windows taskbar back meanwhile) and started again afterwards.
#
# Run it (from anywhere):
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools\install.ps1

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
$manifest = Join-Path $root "Cargo.toml"
# cargo takes .cargo\config.toml (the statically linked C runtime) from the folder it is started
# in, not from the manifest's folder: start it in the project's root, wherever this was run from.
Set-Location -LiteralPath $root

& $cargo build --workspace --release --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { Write-Host "The build failed; nothing was installed."; exit 1 }
# The build folder may be moved by .cargo\config.toml; ask cargo where it is.
$target = (& $cargo metadata --format-version 1 --no-deps --manifest-path $manifest | ConvertFrom-Json).target_directory
$release = Join-Path $target "release"
$built = Join-Path $release "bevel95.exe"

$dir = Join-Path $env:LOCALAPPDATA "Bevel95"
$installed = Join-Path $dir "bevel95.exe"
$programDir = Join-Path $dir "programs"
$programs = @("bevel-help.exe", "folders.exe", "minefield.exe", "solitaire.exe", "calculator.exe")
foreach ($file in @("bevel95.exe") + $programs) {
    if (!(Test-Path -LiteralPath (Join-Path $release $file) -PathType Leaf)) { throw "Build did not produce $file." }
}
# Never a program built by cargo test (the third safety review of 2026-11-18, suspected 1).
. (Join-Path $PSScriptRoot "no-test-build.ps1")
Assert-NoTestBuild (@("bevel95.exe") + $programs | ForEach-Object { Join-Path $release $_ }) "installed"
# One installed copy. A program started from a packaged (MSIX) app, such as the Claude desktop
# app, does not always write where it says: a folder it creates at the top of %LOCALAPPDATA%
# lands in that app's private area (Packages\<the app's package>\LocalCache\Local), and from then on everything it writes at the top of
# that folder stays there, hidden from the desktop shortcut, which keeps starting the old
# program in the real folder (found 2026-11-12; docs/decisions.md, "One installed copy").
function Get-PrivateCopy {
    Get-ChildItem -LiteralPath (Join-Path $env:LOCALAPPDATA "Packages") -Directory -ErrorAction SilentlyContinue |
        ForEach-Object { Join-Path $_.FullName "LocalCache\Local\Bevel95" } |
        Where-Object { Test-Path -LiteralPath $_ }
}
$private = @(Get-PrivateCopy)
if ($private) {
    throw "Nothing was installed: a private copy of the installed folder hides the real one ($($private -join ', ')). See docs/decisions.md, 'One installed copy'."
}
New-Item -ItemType Directory -Force $dir | Out-Null
$private = @(Get-PrivateCopy)
if ($private) {
    # The real folder did not exist, and the one just made is private. Take it away again.
    foreach ($p in $private) { if (!(Get-ChildItem -LiteralPath $p -Force)) { Remove-Item -LiteralPath $p } }
    throw "Nothing was installed: $dir does not exist yet and cannot be made from inside the app this script was started from. Open PowerShell from the Start menu, run: mkdir `"$dir`" and then run this script again."
}
if ((Get-Item -LiteralPath $dir).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Installation folder is a reparse point: $dir" }
New-Item -ItemType Directory -Force $programDir | Out-Null
if ((Get-Item -LiteralPath $programDir).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Programs folder is a reparse point: $programDir" }

# Stage in the same directory before replacing each executable. A failed copy or
# a locked program leaves the existing executable intact.
function Install-Program([string]$source, [string]$destination) {
    $existing = Get-Item -LiteralPath $destination -ErrorAction SilentlyContinue
    if ($existing) {
        if ($existing.PSIsContainer -or ($existing.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw "Unexpected target: $destination" }
        if ($existing.Length -eq (Get-Item -LiteralPath $source).Length -and
            (Get-FileHash -LiteralPath $destination).Hash -eq (Get-FileHash -LiteralPath $source).Hash) { return }
    }
    $stage = Join-Path (Split-Path $destination -Parent) ([IO.Path]::GetRandomFileName())
    try {
        [IO.File]::Copy($source, $stage)
        if ($existing) { [IO.File]::Replace($stage, $destination, [NullString]::Value) }
        else { [IO.File]::Move($stage, $destination) }
    } finally {
        if ([IO.File]::Exists($stage)) { [IO.File]::Delete($stage) }
    }
}

# Leave a running legacy game and its media available until the next update.
# All three strings identify the project's older binaries across release builds.
function Remove-LegacyGame([string]$destination) {
    $path = Join-Path $destination "minefield.exe"
    $old = Get-Item -LiteralPath $path -ErrorAction SilentlyContinue
    if (!$old -or $old.PSIsContainer -or ($old.Attributes -band [IO.FileAttributes]::ReparsePoint)) { return $true }
    try {
        $bytes = [IO.File]::ReadAllBytes($path)
        if ($bytes.Length -lt 2 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) { return $true }
        $binary = [Text.Encoding]::ASCII.GetString($bytes)
        foreach ($marker in 'BEVEL95_MINEFIELD_SEED', 'minefield.txt', 'Bevel95Window') {
            if (!$binary.Contains($marker)) { return $true }
        }
        if (Get-Process -Name minefield -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $path }) {
            Write-Warning "The older Minesweeper is running. Run the installer again after closing the game to remove its old files."
            return $false
        }
        Remove-Item -LiteralPath $path
    } catch {
        Write-Warning "Could not retire the older Minesweeper at $path ($($_.Exception.Message))"
        return $false
    }
    return $true
}

# Match the bytes previously installed. Keep other files, including modified
# sounds and books, and never traverse directory junctions in these paths.
function Remove-LegacyMedia([string]$destination) {
    $base = Get-Item -LiteralPath $destination -ErrorAction Stop
    if ($base.Attributes -band [IO.FileAttributes]::ReparsePoint) { return }
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        foreach ($entry in Get-Content -LiteralPath (Join-Path $PSScriptRoot "legacy-files.txt")) {
            $name, $size, $digest = $entry -split ' '
            $parts = $name.Split('/')
            $folders = @()
            $path = $base.FullName
            for ($i = 0; $i -lt $parts.Count - 1; $i++) {
                $path = Join-Path $path $parts[$i]
                $folder = Get-Item -LiteralPath $path -ErrorAction SilentlyContinue
                if (!$folder -or !$folder.PSIsContainer -or ($folder.Attributes -band [IO.FileAttributes]::ReparsePoint)) { break }
                $folders = @($folder) + $folders
            }
            if ($folders.Count -ne $parts.Count - 1) { continue }
            $path = Join-Path $path $parts[-1]
            $item = Get-Item -LiteralPath $path -ErrorAction SilentlyContinue
            if (!$item -or $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or $item.Length -ne [int]$size) { continue }
            try {
                $bytes = [IO.File]::ReadAllBytes($item.FullName)
                if ($bytes.Length -ne [int]$size) { continue }
                if ($item.Extension -eq '.chw') {
                    # HTML Help timestamps in the generated Win98 keyword index.
                    [Array]::Clear($bytes, 16, 4)
                    [Array]::Clear($bytes, 358, 8)
                }
                if ([BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-', '') -ne $digest) { continue }
                Remove-Item -LiteralPath $item.FullName -ErrorAction Stop
                foreach ($folder in $folders) {
                    if (!$folder.GetFileSystemInfos().Length) { $folder.Delete($false) }
                }
            } catch { Write-Warning "Could not clean old file $name ($($_.Exception.Message))" }
        }
    } finally { $sha.Dispose() }
}
foreach ($program in $programs) {
    Install-Program (Join-Path $release $program) (Join-Path $programDir $program)
}
function Get-InstalledBar { Get-Process bevel95 -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $installed } }
$wasRunning = $null -ne (Get-InstalledBar)
try {
    if ($wasRunning) {
        Write-Host "Closing the running Bevel95 (the Windows taskbar comes back meanwhile)..."
        Start-Process -FilePath $built -ArgumentList "--restore" -WindowStyle Hidden -Wait
        $deadline = (Get-Date).AddSeconds(10)
        while ((Get-InstalledBar) -and ((Get-Date) -lt $deadline)) {
            Start-Sleep -Milliseconds 100
        }
        # Its windows are gone but it still runs: if it holds the mark "finishing file jobs", it is
        # finishing a copy or move of your files (docs/decisions.md, "File jobs outlive their
        # window"). It is never ended by force; wait for it (Windows' taskbar is back meanwhile), at
        # most 30 minutes. Without the mark it is not finishing anything: say so and stop.
        function Test-Finishing {
            foreach ($p in @(Get-InstalledBar)) {
                $mark = $null
                if ([Threading.EventWaitHandle]::TryOpenExisting("Local\Bevel95.FinishingFileJobs.$($p.Id)", [ref]$mark)) {
                    $mark.Dispose()
                    return $true
                }
            }
            return $false
        }
        if ((Get-InstalledBar) -and !(Test-Finishing)) {
            throw "The old Bevel95 (process $(@(Get-InstalledBar)[0].Id)) did not end within 10 seconds and is not finishing a copy or move: bevel95.exe was not replaced. End it in Task Manager and run this script again."
        }
        $deadline = (Get-Date).AddMinutes(30)
        $said = [DateTime]::MinValue
        while ((Get-InstalledBar) -and ((Get-Date) -lt $deadline)) {
            if (((Get-Date) - $said).TotalSeconds -ge 15) {
                Write-Host "The old Bevel95 is still finishing a copy or move of files (Windows' progress window shows it); waiting for it to end..."
                $said = Get-Date
            }
            Start-Sleep -Milliseconds 500
        }
        if (Get-InstalledBar) { throw "The old Bevel95 is still finishing a file job after 30 minutes: bevel95.exe was not replaced. Run this script again once it has ended." }
    }
    Install-Program $built $installed
} finally {
    # Explorer itself starts it (start.ps1), as it does for the desktop shortcut: the bar is
    # then an ordinary program of the owner's, not one of whatever ran this script (the Claude
    # app's own programs get the private area described above).
    if ($wasRunning -and (Test-Path -LiteralPath $installed)) { & (Join-Path $PSScriptRoot "start.ps1") }
}
if (Remove-LegacyGame $dir) { Remove-LegacyMedia $dir }
Remove-LegacyMedia $release

# [IO.File]::Replace over a program that is running leaves the old copy beside it under a name
# like "folders.exe~RF1669db9a.TMP" (Windows' backup of the replaced file, still in use then).
# Such leftovers of Bevel95's own six programs go at the next install; one still in use stays.
function Remove-ReplaceLeftovers([string]$folder) {
    $names = @("bevel95.exe") + $programs
    $pattern = '^(' + (($names | ForEach-Object { [regex]::Escape($_) }) -join '|') + ')~RF[0-9a-f]+\.TMP$'
    Get-ChildItem -LiteralPath $folder -File -Force -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match $pattern -and !($_.Attributes -band [IO.FileAttributes]::ReparsePoint) } |
        ForEach-Object {
            try { [IO.File]::Delete($_.FullName); Write-Host "Removed an old copy left by an earlier install: $($_.Name)" }
            catch { Write-Host "An old copy left by an earlier install is still in use and stays until the next install: $($_.Name)" }
        }
}
Remove-ReplaceLeftovers $dir
Remove-ReplaceLeftovers $programDir

$desktop = [Environment]::GetFolderPath("Desktop")
$shell = New-Object -ComObject WScript.Shell
function Set-Shortcut([string]$name, [string]$arguments, [string]$description) {
    $link = $shell.CreateShortcut((Join-Path $desktop "$name.lnk"))
    $link.TargetPath = $installed
    $link.Arguments = $arguments
    $link.WorkingDirectory = $dir
    $link.Description = $description
    $link.Save()
}
Set-Shortcut "Bevel95" "" "Start the Windows 95-style taskbar"
Set-Shortcut "Restore Windows taskbar" "--restore" "Close Bevel95 and bring back the Windows taskbar"

foreach ($file in @($installed) + @($programs | ForEach-Object { Join-Path $programDir $_ })) {
    $item = Get-Item -LiteralPath $file
    Write-Host ("Installed {0} ({1:N0} bytes, built {2:yyyy-MM-dd HH:mm})" -f $item.FullName, $item.Length, $item.LastWriteTime)
}

# start.ps1: starts the installed Bevel95 the way the desktop shortcut does.
#
# Explorer itself is asked to start it (through the desktop's own shell object), so the bar is
# an ordinary program of the owner's, whoever ran this script. A program that a tool of the
# Claude desktop app starts directly is one of the app's own, also when it breaks away from
# the app's job: what it creates new at the top of %LOCALAPPDATA% lands in the app's private
# area. Started by Explorer (this way, or with "explorer.exe <file>") it writes the real
# place (each measured with a throwaway folder, 2026-11-12; docs/decisions.md, "One
# installed copy").
#
# Run it (from anywhere):
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools\start.ps1

$exe = Join-Path $env:LOCALAPPDATA "Bevel95\bevel95.exe"
if (!(Test-Path -LiteralPath $exe -PathType Leaf)) {
    Write-Host "Bevel95 is not installed ($exe)."
    exit 1
}
try {
    # ShellWindows; its desktop entry (SWC_DESKTOP = 8, SWFO_NEEDDISPATCH = 1) lives in Explorer.
    $windows = [Activator]::CreateInstance([type]::GetTypeFromCLSID('9BA05972-F6A8-11CF-A442-00A0C90A8F39'))
    $hwnd = 0
    $desktop = $windows.FindWindowSW([ref]0, [ref]$null, 8, [ref]$hwnd, 1)
    if ($null -eq $desktop) { throw "Explorer's desktop was not found" }
    $desktop.Document.Application.ShellExecute($exe, '', (Split-Path $exe -Parent), 'open', 1)
} catch {
    Write-Warning "Could not ask Explorer to start Bevel95 ($($_.Exception.Message)); starting it directly."
    Start-Process -FilePath $exe
}

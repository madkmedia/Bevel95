# lines-pause.ps1: dot-sourced by the helpers here that photograph the screen itself
# (calculator.ps1, dialogcaps.ps1, minefield.ps1, volcheck.ps1, voltiming.ps1). Their copies of the
# screen would show Bevel95's Monitor Lines while the person has them on, and then compare wrong
# for no visible reason. Suspend-MonitorLines asks the running bar to take the lines away for a
# moment (the bar's registered message Bevel95.Bar.PauseMonitorLines; nothing is saved, and the
# bar brings them back by itself after at most a minute); Resume-MonitorLines ends the pause.
# Both do nothing when no bar runs or the lines are off, so a helper works the same then.
if (-not ('LinesPause.W' -as [type])) {
    Add-Type -Namespace LinesPause -Name W -MemberDefinition @'
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string c, string t);
[DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessageW(string name);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern IntPtr SendMessageTimeoutW(IntPtr h, uint m, IntPtr w, IntPtr l, uint flags, uint ms, out IntPtr result);
'@
}

# Sends the pause (1) or its end (0) to the bar (only a Bevel95Bar window of bevel95.exe); true if
# the bar said the lines were showing or paused.
function Send-LinesPause([int]$On) {
    $bar = [LinesPause.W]::FindWindowW('Bevel95Bar', [NullString]::Value)
    if ($bar -eq [IntPtr]::Zero) { return $false }
    $owner = 0; [void][LinesPause.W]::GetWindowThreadProcessId($bar, [ref]$owner)
    $process = Get-Process -Id $owner -ErrorAction SilentlyContinue
    if (-not $process -or $process.ProcessName -ne 'bevel95') { return $false }
    $message = [LinesPause.W]::RegisterWindowMessageW('Bevel95.Bar.PauseMonitorLines')
    if ($message -eq 0) { return $false }
    $answer = [IntPtr]::Zero
    # SMTO_ABORTIFHUNG, at most 2 s: a bar that does not answer is not waited for.
    $sent = [LinesPause.W]::SendMessageTimeoutW($bar, $message, [IntPtr]$On, [IntPtr]0, 2, 2000, [ref]$answer)
    return ($sent -ne [IntPtr]::Zero -and $answer -ne [IntPtr]::Zero)
}

# Before a photograph: the lines away, and a moment for the screen to be drawn without them.
function Suspend-MonitorLines {
    if (Send-LinesPause 1) { Start-Sleep -Milliseconds 150 }
}

# After it: the lines back (nothing happens if there was no pause).
function Resume-MonitorLines {
    [void](Send-LinesPause 0)
}

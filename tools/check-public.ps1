# check-public.ps1: the check before a push. This folder is the public project; only what lies
# outside private\ and CLAUDE.local.md (both ignored by git) is public. The script changes
# nothing, contacts nothing and never pushes.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools\check-public.ps1
#
# What it checks:
#   1. No tracked file lies under private\, and CLAUDE.local.md is not tracked.
#   2. Every tracked file, as it is in the working folder, against every kind in the private word
#      list (export-private-words.txt in the private folder: BEVEL95_PRIVATE, default this
#      checkout's untracked private\; kind|regex per line). A hit names the kind, the file and the
#      line, never the text itself. Without the list it stops: the list is never tracked, so that
#      it is never published.
#   3. The commits not yet pushed (those that no remote branch known here holds): every author
#      and committer must be the public identity, and no message may hold a private word or a
#      note about an AI helper (Co-Authored-By, "Generated with").
# Exit code 0 when all is clean, 1 when anything was found.
param(
    [string]$Name = 'madkmedia',
    [string]$Email = '358349870+madkmedia@users.noreply.github.com'
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Split-Path -Parent $PSScriptRoot)).Path
$private = if ($env:BEVEL95_PRIVATE) { $env:BEVEL95_PRIVATE } else { Join-Path $root 'private' }
$failed = 0

# The private word list.
$wordsFile = Join-Path $private 'export-private-words.txt'
if (-not (Test-Path -LiteralPath $wordsFile -PathType Leaf)) {
    throw "Stopped: the private word list $wordsFile is missing, so nothing could be checked. It lists the private words (kind|regex per line; docs/decisions.md, 'The public copy')."
}
$kinds = @(Get-Content -LiteralPath $wordsFile | Where-Object { $_.Trim() -and -not $_.StartsWith('#') } | ForEach-Object {
    $kind, $regex = $_ -split '\|', 2
    if (-not $regex) { throw "Stopped: a line of $wordsFile has no 'kind|regex' shape." }
    [pscustomobject]@{ Kind = $kind.Trim(); Regex = [regex]::new($regex, 'IgnoreCase') }
})
if ($kinds.Count -eq 0) { throw "Stopped: $wordsFile lists nothing." }

# 1. What is tracked.
$tracked = @(git -C $root -c core.quotepath=off ls-files)
if ($LASTEXITCODE -ne 0) { throw "Stopped: git could not list the tracked files." }
$wrong = @($tracked | Where-Object { $_ -like 'private/*' -or $_ -ieq 'CLAUDE.local.md' })
foreach ($w in $wrong) { Write-Host "  PRIVATE FILE TRACKED: $w" -ForegroundColor Red; $failed++ }

# 2. The tracked files' contents (and their paths).
$latin = [Text.Encoding]::GetEncoding(28591)
function Scan([string]$text, [string]$where) {
    $n = 0
    foreach ($k in $kinds) {
        # The public identity's own no-reply address is public (it is on every commit).
        $found = @($k.Regex.Matches($text) | Where-Object { $_.Value -ine $Email })
        if ($found.Count -eq 0) { continue }
        $n += $found.Count
        $lines = @($found | ForEach-Object { ($text.Substring(0, $_.Index) -split "`n").Count } | Select-Object -Unique -First 10)
        Write-Host ("  PRIVATE: {0} x{1} in {2}, line(s) {3}" -f $k.Kind, $found.Count, $where, ($lines -join ', ')) -ForegroundColor Red
    }
    $n
}
$scanned = 0
foreach ($rel in $tracked) {
    $path = Join-Path $root ($rel -replace '/', '\')
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
    $failed += Scan $latin.GetString([IO.File]::ReadAllBytes($path)) $rel
    $failed += Scan $rel "the name of $rel"
    $scanned++
}
Write-Host "Scanned $scanned tracked files against $($kinds.Count) patterns."

# 3. The commits not yet pushed.
git -C $root rev-parse --verify --quiet HEAD | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "No commit yet: no author to check."
} else {
    $range = @('HEAD', '--not', '--remotes')
    $count = [int](git -C $root rev-list --count @range)
    $people = @(git -C $root log --format="%an <%ae>%n%cn <%ce>" @range | Sort-Object -Unique)
    Write-Host "Commits not yet pushed: $count. Their authors and committers:"
    $public = "$Name <$Email>"
    foreach ($p in $people) {
        if ($p -ceq $public) { Write-Host "  $p" }
        else { Write-Host "  NOT THE PUBLIC IDENTITY: one author or committer (not shown here: see git log)" -ForegroundColor Red; $failed++ }
    }
    foreach ($hash in @(git -C $root rev-list @range)) {
        $message = (git -C $root log -1 --format=%B $hash) -join "`n"
        $failed += Scan $message "the message of $($hash.Substring(0, 7))"
        if ($message -match '(?im)^\s*co-authored-by:|generated with') {
            Write-Host "  AI NOTE in the message of $($hash.Substring(0, 7))" -ForegroundColor Red; $failed++
        }
    }
}

if ($failed) {
    Write-Host "Found $failed problem(s): DO NOT PUSH. Repair them first." -ForegroundColor Red
    exit 1
}
Write-Host "Clean: nothing private found, every unpushed commit is the public identity's. Nothing was pushed."

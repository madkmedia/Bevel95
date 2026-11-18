# vm.ps1: the reference PCs. Real Windows 95, 98 and 2000, installed from the owner's discs,
# run in QEMU without a window; everything goes through QEMU's control port (QMP).
# All of it lives in private\reference\ (never in git).
#
#   . tools\vm.ps1                  # load the functions
#   Start-Vm win98                  # boot it; changes are thrown away when it stops
#   Start-Vm win98 -Keep            # ... changes are kept (to adjust the installed PC)
#   Send-Keys win98 ctrl-esc        # QEMU key names: a, shift-a, ret, spc, tab, esc, alt-f4, meta_l
#   Send-Text win98 "notepad`n"     # type text (`n = Enter)
#   Save-Screen win98 C:\x.png      # the exact screen contents as PNG
#   Move-MouseBy win98 10 -5        # relative pointer moves (no PC has an absolute tablet now)
#   Click-Mouse win98
#   Stop-Vm win98

# The PCs, discs and QEMU are private material, never part of the public project: they are found
# in the folder reference\ of the private folder: BEVEL95_PRIVATE, by default this checkout's
# untracked private folder.
$VmRoot = Join-Path $(if ($env:BEVEL95_PRIVATE) { $env:BEVEL95_PRIVATE } else { Join-Path (Split-Path $PSScriptRoot -Parent) 'private' }) 'reference'
$Qemu = Join-Path $VmRoot 'qemu\qemu-system-i386.exe'
$QemuImg = Join-Path $VmRoot 'qemu\qemu-img.exe'
foreach ($needed in @($Qemu, (Join-Path $VmRoot 'vm'), (Join-Path $VmRoot 'iso'))) {
    if (-not (Test-Path -LiteralPath $needed)) {
        Write-Warning "vm.ps1: $needed is missing. The Reference PCs (QEMU in qemu\, the disks in vm\, the owner's discs in iso\) are private material and not part of the public project; set BEVEL95_PRIVATE to the folder whose reference\ holds them."
    }
}

# Disc file names start with these patterns (private\reference\iso). Ports are for the control connection.
# Windows 95 stops with "While initializing device IOS: Windows protection error" on CPUs that run
# its timing loop faster than about 350 million times a second (the famous fast-CPU bug). Running
# one instruction per translation block slows the emulated CPU to an old Pentium's pace, which
# avoids it (-icount would too, but runs about 75 times slower than real time here).
# Start 95 without -FloppyDir: with a floppy inserted the IOS error came back (twice in a row).
# 95's disc is the 95C one (OSR 2.5): the 1995 disc is an upgrade edition that needs Windows 3.x.
# 98's CPU has no local APIC: after 98's own restarts the APIC came back switched off, and QEMU
# then keeps the old interrupt controller's timer from the CPU, so DOS-time code waited forever.
# 2000 runs on QEMU's ISA-only PC (isapc: no PCI, so no IDE bus-master DMA): on fast virtual
# hardware Windows 2000's IDE driver races with its own DMA interrupts (VirtualBox's manual:
# "installation reboots, usually during component registration"); here setup died at "Completing
# configuration of Distributed Transaction Coordinator" every time on the normal PC, whatever the
# CPU, clock or network card. With plain IDE transfers, QEMU's -win2k-hack covers the race.
# (An AMD PCscsi disk instead hung SeaBIOS.) Bus = 'scsi' is kept as an option. Its CPU has no
# APIC either: with one, setup picks an APIC HAL that never gets a timer on the ISA PC.
# Graphics: QEMU's standard VGA ('std') with a display driver made for emulators, because the
# drivers on the Windows discs reach High Color (16-bit) at most in QEMU: VMDisp9x for 95/98
# (open source), VBEMP for 2000 (bearwindows; free for non-commercial use), as v86's and the
# Computernewb wiki's guides do. They only show the picture: Windows' own drawing code (95/98's
# DIB engine, 2000's framebuf.dll) still draws every pixel. The disks keep a snapshot
# "highcolor-cirrus" of the earlier Cirrus/High Color state (qemu-img snapshot -a to go back,
# with Vga = 'cirrus').
$VmConfig = @{
    win95   = @{ Port = 4495; Mem = 64;  Cpu = 'pentium';  Disk = '512M'; Iso = 'win95_osr2*';  Tablet = $false; Bus = 'ide'; Machine = 'pc'; Vga = 'std'; Flags = @('-accel', 'tcg,one-insn-per-tb=on') }
    win98   = @{ Port = 4498; Mem = 128; Cpu = 'pentium2,-apic'; Disk = '1G'; Iso = 'win98_first*'; Tablet = $false; Bus = 'ide'; Machine = 'pc'; Vga = 'std'; Flags = @() }
    win2000 = @{ Port = 4420; Mem = 256; Cpu = 'pentium3,-apic'; Disk = '3G'; Iso = 'win2000*';   Tablet = $false; Bus = 'ide'; Machine = 'isapc'; Vga = 'std'; Flags = @('-win2k-hack') }
}

function Get-VmIso([string]$Pattern) {
    (Get-ChildItem (Join-Path $VmRoot "iso\$Pattern.iso") | Select-Object -First 1).FullName
}

function Start-Vm {
    param(
        [Parameter(Mandatory)][string]$Name,
        [string]$Cd,             # disc image in the CD drive (default: none)
        [string]$FloppyDir,      # a folder shown to the PC as floppy A: (read-only)
        [switch]$BootCd,         # boot from the CD once, then from the hard disk
        [switch]$Keep,           # keep changes to the hard disk
        [switch]$Nic,            # a network card (Realtek 8139) that reaches nothing outside
        [switch]$Sound,          # a Sound Blaster 16 (port 220, IRQ 5, DMA 1/5) that plays nothing
        [string]$Clock = 'localtime',  # or a date like 2003-07-15T12:00:00 (2000's setup needs one)
        [string[]]$Extra = @()   # more QEMU options
    )
    $c = $VmConfig[$Name]
    $disk = Join-Path $VmRoot "vm\$Name.qcow2"
    if (-not (Test-Path $disk)) { & $QemuImg create -q -f qcow2 $disk $c.Disk }
    $net = if ($Nic) { @('-netdev', 'user,id=n0,restrict=on', '-device', 'rtl8139,netdev=n0') } else { @('-nic', 'none') }
    $a = @('-name', $Name, '-M', $c.Machine, '-cpu', $c.Cpu, '-m', $c.Mem, '-rtc', "base=$Clock",
        '-vga', $c.Vga, '-display', 'none') + $net + @(
        '-qmp', "tcp:127.0.0.1:$($c.Port),server=on,wait=off")
    if ($c.Bus -eq 'scsi') {
        # the drives are named hd0 / cd0; the CD boots first only with -BootCd
        $cdBoot = if ($BootCd) { 0 } else { 2 }
        $a += '-device', 'am53c974,id=scsi0',
            '-drive', "file=$disk,format=qcow2,if=none,id=hd0", '-device', 'scsi-hd,bus=scsi0.0,scsi-id=0,drive=hd0,bootindex=1'
        $a += if ($Cd) { '-drive', "file=$Cd,format=raw,if=none,id=cd0,media=cdrom,readonly=on" } else { '-drive', 'if=none,id=cd0,media=cdrom' }
        $a += '-device', "scsi-cd,bus=scsi0.0,scsi-id=2,drive=cd0,bootindex=$cdBoot"
    } else {
        $a += '-drive', "file=$disk,format=qcow2,if=ide,index=0,media=disk"
        if ($Cd) { $a += '-drive', "file=$Cd,format=raw,if=ide,index=2,media=cdrom,readonly=on" }
        else { $a += '-drive', 'if=ide,index=2,media=cdrom' }
        if ($BootCd) { $a += '-boot', 'once=d' }
    }
    if ($FloppyDir) { $a += '-drive', "if=floppy,index=0,format=raw,readonly=on,file=fat:floppy:12:$FloppyDir" }
    if ($c.Tablet) { $a += '-usb', '-device', 'usb-tablet' }
    if ($Sound) { $a += '-audiodev', 'none,id=snd0', '-device', 'sb16,audiodev=snd0' }
    if (-not $Keep) { $a += '-snapshot' }
    $a += $c.Flags
    $a += $Extra
    $quoted = $a | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }
    $log = Join-Path $VmRoot "vm\$Name.log"
    $p = Start-Process $Qemu -ArgumentList $quoted -WindowStyle Hidden -RedirectStandardError $log -PassThru
    # wait until the control port answers
    for ($i = 0; $i -lt 50; $i++) {
        Start-Sleep -Milliseconds 200
        if ($p.HasExited) { throw "QEMU exited: $(Get-Content $log -Raw)" }
        try { $t = New-Object Net.Sockets.TcpClient; $t.Connect('127.0.0.1', $c.Port); $t.Close(); return $p } catch { }
    }
    throw "QEMU's control port $($c.Port) did not answer"
}

# Sends QMP commands (hashtables or JSON strings) over one connection; returns the answers.
function Invoke-Qmp {
    param([Parameter(Mandatory)][string]$Name, [Parameter(Mandatory)][object[]]$Commands, [int]$DelayMs = 0)
    $client = New-Object Net.Sockets.TcpClient
    $client.Connect('127.0.0.1', $VmConfig[$Name].Port)
    try {
        $stream = $client.GetStream()
        $reader = New-Object IO.StreamReader($stream)
        $writer = New-Object IO.StreamWriter($stream)
        $writer.AutoFlush = $true
        $writer.NewLine = "`n"
        [void]$reader.ReadLine()
        $answers = @()
        foreach ($cmd in @(@{ execute = 'qmp_capabilities' }) + $Commands) {
            $json = if ($cmd -is [string]) { $cmd } else { $cmd | ConvertTo-Json -Depth 10 -Compress }
            $writer.WriteLine($json)
            do { $line = $reader.ReadLine() } until ($null -eq $line -or $line -match '^\{"(return|error)"')
            if ($null -eq $line) { break }
            $answers += ($line | ConvertFrom-Json)
            if ($DelayMs) { Start-Sleep -Milliseconds $DelayMs }
        }
        $answers | Select-Object -Skip 1 | Where-Object { $_.error } | ForEach-Object { Write-Warning "QMP: $($_.error.desc)" }
        $answers | Select-Object -Skip 1
    } finally { $client.Close() }
}

function Invoke-Hmp([string]$Name, [string]$CommandLine) {
    (Invoke-Qmp $Name @(@{ execute = 'human-monitor-command'; arguments = @{ 'command-line' = $CommandLine } })).return
}

function Send-Keys {
    param([Parameter(Mandatory)][string]$Name, [Parameter(Mandatory, ValueFromRemainingArguments)][object[]]$Keys)
    $flat = @($Keys | ForEach-Object { $_ })   # an array passed positionally arrives nested
    # "ctrl-alt-delete" = those keys held together; 200 ms presses (the slowed-down 95 misses 100 ms ones)
    $cmds = foreach ($k in $flat) {
        $codes = @($k -split '-' | ForEach-Object { @{ type = 'qcode'; data = $_ } })
        @{ execute = 'send-key'; arguments = @{ keys = $codes; 'hold-time' = 200 } }
    }
    [void](Invoke-Qmp $Name @($cmds) -DelayMs 280)
}

$KeyOf = @{
    ' ' = 'spc'; "`n" = 'ret'; "`t" = 'tab'; '-' = 'minus'; '_' = 'shift-minus'; '=' = 'equal'; '+' = 'shift-equal'
    '\' = 'backslash'; '|' = 'shift-backslash'; '/' = 'slash'; '?' = 'shift-slash'; '.' = 'dot'; '>' = 'shift-dot'
    ',' = 'comma'; '<' = 'shift-comma'; ';' = 'semicolon'; ':' = 'shift-semicolon'; "'" = 'apostrophe'
    '"' = 'shift-apostrophe'; '[' = 'bracket_left'; '{' = 'shift-bracket_left'; ']' = 'bracket_right'
    '}' = 'shift-bracket_right'; '`' = 'grave_accent'; '~' = 'shift-grave_accent'; '!' = 'shift-1'; '@' = 'shift-2'
    '#' = 'shift-3'; '$' = 'shift-4'; '%' = 'shift-5'; '^' = 'shift-6'; '&' = 'shift-7'; '*' = 'shift-8'
    '(' = 'shift-9'; ')' = 'shift-0'
}

function Send-Text([string]$Name, [string]$Text) {
    $keys = foreach ($ch in $Text.ToCharArray()) {
        $s = [string]$ch
        if ($KeyOf.ContainsKey($s)) { $KeyOf[$s] }
        elseif ($s -cmatch '^[a-z0-9]$') { $s }
        elseif ($s -cmatch '^[A-Z]$') { 'shift-' + $s.ToLower() }
        else { throw "no key for '$s'" }
    }
    if ($keys) { Send-Keys $Name @($keys) }
}

function Save-Screen([string]$Name, [string]$Path) {
    [void](Invoke-Qmp $Name @(@{ execute = 'screendump'; arguments = @{ filename = $Path; format = 'png' } }))
}

# Waits until the screen pixel at (X, Y) has the color Rgb (0xRRGGBB); $false after TimeoutSec.
function Wait-Pixel([string]$Name, [int]$X, [int]$Y, [int]$Rgb, [double]$TimeoutSec = 30) {
    Add-Type -AssemblyName System.Drawing
    $f = Join-Path $env:TEMP "bevel95-vm-$Name-wait.png"
    $end = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $end) {
        try {
            Save-Screen $Name $f
            $b = New-Object Drawing.Bitmap($f)
            $hit = $X -lt $b.Width -and $Y -lt $b.Height -and ($b.GetPixel($X, $Y).ToArgb() -band 0xFFFFFF) -eq $Rgb
            $b.Dispose()
            if ($hit) { Remove-Item $f; return $true }
        } catch { }
        Start-Sleep -Milliseconds 250
    }
    Remove-Item $f -ErrorAction SilentlyContinue
    $false
}

# Absolute pointer position (USB tablet, Windows 2000); the screen size maps to QEMU's 0..32767.
function Move-Mouse([string]$Name, [int]$X, [int]$Y, [int]$Width = 800, [int]$Height = 600) {
    $ax = [int](($X * 32768 + 16384) / $Width); $ay = [int](($Y * 32768 + 16384) / $Height)
    [void](Invoke-Qmp $Name @(@{ execute = 'input-send-event'; arguments = @{ events = @(
        @{ type = 'abs'; data = @{ axis = 'x'; value = $ax } }, @{ type = 'abs'; data = @{ axis = 'y'; value = $ay } }) } }))
}

# Relative pointer moves (PS/2 mouse, Windows 95/98), in small steps so Windows does not speed them up.
function Move-MouseBy([string]$Name, [int]$Dx, [int]$Dy, [int]$Step = 4) {
    $cmds = @()
    while ($Dx -ne 0 -or $Dy -ne 0) {
        $sx = [Math]::Max(-$Step, [Math]::Min($Step, $Dx)); $sy = [Math]::Max(-$Step, [Math]::Min($Step, $Dy))
        $cmds += @{ execute = 'input-send-event'; arguments = @{ events = @(
            @{ type = 'rel'; data = @{ axis = 'x'; value = $sx } }, @{ type = 'rel'; data = @{ axis = 'y'; value = $sy } }) } }
        $Dx -= $sx; $Dy -= $sy
    }
    if ($cmds) { [void](Invoke-Qmp $Name $cmds -DelayMs 15) }
}

function Click-Mouse([string]$Name, [string]$Button = 'left', [switch]$Double) {
    $down = @{ execute = 'input-send-event'; arguments = @{ events = @(@{ type = 'btn'; data = @{ down = $true; button = $Button } }) } }
    $up = @{ execute = 'input-send-event'; arguments = @{ events = @(@{ type = 'btn'; data = @{ down = $false; button = $Button } }) } }
    $cmds = if ($Double) { @($down, $up, $down, $up) } else { @($down, $up) }
    [void](Invoke-Qmp $Name $cmds -DelayMs 60)
}

# Puts another disc into the CD drive (or empties it with no path).
function Set-Cd([string]$Name, [string]$Iso) {
    # QEMU's monitor reads backslashes as escapes, so the path goes with forward slashes
    $drive = if ($VmConfig[$Name].Bus -eq 'scsi') { 'cd0' } else { 'ide1-cd0' }
    if ($Iso) { Invoke-Hmp $Name "change $drive $($Iso -replace '\\', '/') raw read-only" } else { Invoke-Hmp $Name "eject -f $drive" }
}

function Stop-Vm([string]$Name) {
    try { [void](Invoke-Qmp $Name @(@{ execute = 'quit' })) } catch { }
}

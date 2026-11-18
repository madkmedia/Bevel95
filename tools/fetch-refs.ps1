# fetch-refs.ps1: downloads the real Windows 95 / 98 / 2000 screenshots that the drawings are
# checked against (docs/theme-spec.md, "Reference screenshots") into private\reference\refs (kept out of git;
# BEVEL95_PRIVATE names another folder for private),
# and makes color-corrected copies of the Toasty Tech ones.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools\fetch-refs.ps1 [folder]
#
# Then compare: cargo compare-refs (it reads private\reference\refs unless BEVEL95_REFS names another folder).

# The folder for private material (BEVEL95_PRIVATE, default: this checkout's untracked private).
param([string]$dir = (Join-Path $(if ($env:BEVEL95_PRIVATE) { $env:BEVEL95_PRIVATE } else { Join-Path (Split-Path $PSScriptRoot -Parent) "private" }) "reference\refs"))

$files = [ordered]@{
    "win95-rtm-desktop-startmenu-programs-open-taskbar-640x480.png"            = "https://guidebookgallery.org/pics/gui/desktop/full/win95.png"
    "win95-rtm-taskbar-3tasks-1active-clock-640x28.png"                        = "https://guidebookgallery.org/pics/gui/system/managers/running/win95.png"
    "win95-rtm-startmenu-programs-accessories-multimedia-cutout-624x292.png"   = "https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win95-1-2.png"
    "win95-rtm-startmenu-nohighlight-cutout-164x261.png"                       = "https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win95-1-1.png"
    "win95-startmenu-4level-cascade-640x480-shiftedcolors.png"                 = "http://toastytech.com/guis/win95startmenu.png"
    "win95-desktop-contextmenu-new-submenu-640x480-shiftedcolors.png"          = "http://toastytech.com/guis/win95desktop2.png"
    "win95-taskbar-contextmenu-640x480-shiftedcolors.png"                      = "http://toastytech.com/guis/win95tile.png"
    "win95-startbutton-contextmenu-640x480-shiftedcolors.png"                  = "http://toastytech.com/guis/win95startexplore.png"
    "win95-taskbar-4tasks-1active-640x480-shiftedcolors.png"                   = "http://toastytech.com/guis/win95desktop3.png"
    "win2000pro-desktop-startmenu-open-taskbar-800x600.png"                    = "https://guidebookgallery.org/pics/gui/desktop/full/win2000pro.png"
    "win2000pro-taskbar-quicklaunch-3tasks-1active-tray-800x28.png"            = "https://guidebookgallery.org/pics/gui/system/managers/running/win2000pro.png"
    "win2000pro-startmenu-programs-accessories-cutout-441x302.png"             = "https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win2000pro-1-2.png"
    "win98-desktop-startmenu-favorites-channels-taskbar-640x480.png"           = "https://guidebookgallery.org/pics/gui/desktop/full/win98.png"
    "win98-taskbar-quicklaunch-3tasks-1active-tray-640x28.png"                 = "https://guidebookgallery.org/pics/gui/system/managers/running/win98.png"
    "win98-startmenu-programs-accessories-communications-cutout-564x405.png"   = "https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win98-1-2.png"
    "win98-startmenu-nohighlight-cutout-168x405.png"                           = "https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win98-1-1.png"
    "win98se-startmenu-nohighlight-cutout-194x364.png"                         = "https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win98se-1-1.png"
    "dialogs\win95-run-dialog-347x163.png"                                     = "https://guidebookgallery.org/pics/gui/system/features/run/win95.png"
    "dialogs\win95-shutdown-dialog-347x222.png"                                = "https://guidebookgallery.org/pics/gui/startupshutdown/shutdownwindow/win95.png"
    "dialogs\win2000-run-dialog-347x179.png"                                   = "https://guidebookgallery.org/pics/gui/system/features/run/win2000pro.png"
    "dialogs\win2000-shutdown-dialog-417x269.png"                              = "https://guidebookgallery.org/pics/gui/startupshutdown/shutdownwindow/win2000pro.png"
    "dialogs\win2000-logoff-dialog-288x123.png"                                = "https://guidebookgallery.org/pics/gui/startupshutdown/logout/win2000pro.png"
    "dialogs\win98-logoff-dialog-288x123.png"                                  = "https://guidebookgallery.org/pics/gui/startupshutdown/logout/win98.png"
    "dialogs\win98-run-dialog-347x163.png"                                     = "https://guidebookgallery.org/pics/gui/system/features/run/win98.png"
    "dialogs\win98-shutdown-dialog-323x192.png"                                = "https://guidebookgallery.org/pics/gui/startupshutdown/shutdownwindow/win98.png"
    "dialogs\win95-run-dialog-typed-text-ok-enabled-347x163-shiftedcolors.png" = "http://toastytech.com/guis/win95rununc.png"
    "dialogs\win95-shutdown-dialog-dimmed-desktop-640x480-shiftedcolors.png"   = "http://toastytech.com/guis/win95shutdown.png"
}

# Toasty Tech's captures have every system color shifted by a fixed amount; these swap them back.
$swap = @{
    "00787F" = "008080"; "BFB8BF" = "C0C0C0"; "FFF8FF" = "FFFFFF"; "7F787F" = "808080"; "DFD8DF" = "DFDFDF"
    "00007F" = "000080"; "FFF800" = "FFFF00"; "7F7800" = "808000"; "007800" = "008000"; "00F8FF" = "00FFFF"
    "00F800" = "00FF00"; "7F0000" = "800000"; "7F007F" = "800080"
}

New-Item -ItemType Directory -Force (Join-Path $dir "dialogs") | Out-Null
foreach ($name in $files.Keys) {
    $out = Join-Path $dir $name
    if (Test-Path $out) { continue }
    Write-Host "downloading $name"
    Invoke-WebRequest -Uri $files[$name] -OutFile $out -UseBasicParsing
}

Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System.Collections.Generic; using System.Drawing; using System.Drawing.Imaging;
public static class Bevel95Recolor {
    public static void Run(string src, string dst, Dictionary<int, int> map) {
        using (var b = new Bitmap(src)) using (var o = new Bitmap(b.Width, b.Height, PixelFormat.Format24bppRgb)) {
            for (int y = 0; y < b.Height; y++) for (int x = 0; x < b.Width; x++) {
                int c = b.GetPixel(x, y).ToArgb() & 0xFFFFFF, m;
                o.SetPixel(x, y, Color.FromArgb(unchecked((int)0xFF000000) | (map.TryGetValue(c, out m) ? m : c)));
            }
            o.Save(dst, ImageFormat.Png);
        }
    }
}
'@
$map = New-Object 'System.Collections.Generic.Dictionary[int,int]'
foreach ($k in $swap.Keys) { $map[[Convert]::ToInt32($k, 16)] = [Convert]::ToInt32($swap[$k], 16) }
foreach ($name in $files.Keys | Where-Object { $_ -like "*-shiftedcolors.png" }) {
    $fixed = Join-Path $dir ($name -replace "-shiftedcolors\.png$", "-fixedcolors.png")
    if (-not (Test-Path $fixed)) {
        Write-Host "restoring the colors of $name"
        [Bevel95Recolor]::Run((Join-Path $dir $name), $fixed, $map)
    }
}
Write-Host "Done. Now: cargo compare-refs  (for a folder other than private\reference\refs, set BEVEL95_REFS to it first)"

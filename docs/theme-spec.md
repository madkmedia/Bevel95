# Theme spec: every number, and where it came from

All sizes are in 1× pixels (Windows 95 pixels). The code version of this table is
[`core/src/theme.rs`](../core/src/theme.rs) (`WIN95`, `WIN98` and `WIN2000`). The tables
below give 95 and 2000; Windows 98 is 95 except for what [its own section](#windows-98) lists.

**Jump to:** [Sources](#sources) · [Colors](#colors) · [Bevels](#bevels) · [Bar](#the-bar) ·
[Start button](#start-button) · [Task buttons](#task-buttons) ·
[**Notification area and clock**](#notification-area-and-clock) ·
[Text](#text) · [Start menu](#start-menu) · [Cascading menus](#cascading-menus) ·
[Windows' own menus](#windows-own-menus) · [Submenu placement](#submenu-placement) ·
[**Window menu, scroll arrows**](#window-menu-and-scroll-arrows) ·
[Dialogs](#dialogs) · [**Program windows**](#program-windows) ·
[**Taskbar Properties**](#taskbar-properties) ·
[Pictures](#pictures-banner-icons-flag) · [**Windows 98**](#windows-98) ·
[**Reference screenshots**](#reference-screenshots) ·
[**Discrepancies**](#discrepancies) · [**Flagged for your eyes**](#flagged-for-your-eyes)

## Sources

| Name used below | What | Version read |
|---|---|---|
| **Screenshot** | Real Windows 95, 98 and 2000 screenshots, measured pixel by pixel. Each one is listed under [Reference screenshots](#reference-screenshots) (S1–S17, D1–D10). These win over everything else. | downloaded 2026-11-06 and 2026-11-07 |
| **RetroBar** | [dremin/RetroBar](https://github.com/dremin/RetroBar) (Apache-2.0): `RetroBar/Themes/System.xaml` (base theme), `Themes/Windows 95-98.xaml`, `Themes/Windows 2000.xaml`, `Controls/TaskList.xaml.cs`, `Converters/TaskButtonWidthConverter.cs`, `Resources/start9x.png` (the flag, since 2026-11-18 cut from the Reference PCs instead and found identical) | commit `d7e14690` (2026-11-01) |
| **98.css** | [jdan/98.css](https://github.com/jdan/98.css) (MIT): `style.css` | commit `b1d7a907` (2025-09-07) |
| **WPF** | RetroBar's exact text settings rendered with WPF on this PC, to see which pixel rows its labels land on (RetroBar places text through font metrics, not pixel numbers). See [the appendix](#appendix-the-wpf-text-measurement). | Windows 11 26200 |
| **GDI** | The real fonts measured on this PC: `MS Sans Serif` is the bitmap font `SSERIFE.FON` (96 DPI); `Tahoma` is TrueType. Both drawn without anti-aliasing. | Windows 11 26200 |
| **v86** | Real Windows run in the [v86 emulator](https://copy.sh/v86/) in the browser, its screen read pixel for pixel (exact values, not screenshots of screenshots): "Windows 95" (it has Internet Explorer 4's taskbar, 1px taller than standard; 256 colors), Windows 98 (first edition; 16 colors, so `#DFDFDF` shows as light gray) and Windows 2000 (full color). For what no screenshot shows: the window menu, the scroll arrows, shortcut texts, the check mark, the Taskbar Properties window, the Start menu with small icons. The window menu and scroll arrow numbers are locked in tests (`menus_measure_like_windows` in core/tests/golden.rs, layout.rs's tests). The Taskbar Properties window and the small-icon Start menus were saved as exact PNG files (the emulator's canvas, through the browser) and are compared by `cargo compare-refs` like the screenshots: put them in `v86/` in the reference folder (they are Microsoft's pictures, so they stay out of the repository too). | read 2026-11-07 |
| **Reference PC** | Real Windows installed from the owner's discs and run in QEMU (`tools/vm.ps1`; see decisions.md, "Reference PCs"): Windows 95 "C" (OSR 2.5, the plain 95 shell), Windows 98 first edition and Windows 2000 Professional SP4, all at 800×600 True Color, captured as exact PNGs (including the mouse pointer, which is masked). For the notification area: icons, the clock at different times of day, tooltips, and the speaker, volume control and speaker menu of all three (each PC with a Sound Blaster 16 added). For task buttons: Notepad, a folder, My Computer, My Documents, the Recycle Bin and Control Panel, raised and pressed. Compared by `cargo compare-refs` from `vm/` in the reference folder (Microsoft's pictures: kept out of the repository). | captured 2026-11-08 |
| **derived** | Computed from the values above. | |
| **PROVISIONAL** | In no source. My best guess, to be checked against a real screenshot. | |

**How the screenshots are used.** `cargo compare-refs` (see [core/tests/reference.rs](../core/tests/reference.rs))
draws the same state as each screenshot with our code and compares them pixel for pixel; only
the apps' own icons, a mouse pointer and a tooltip are left out. **All 42 cases match with 0
differing pixels**: the three taskbars; the Windows 95 Start menu alone and with Programs >
Accessories > Multimedia open; the Windows 98 Start menu alone, with Favorites > Channels open
and with Programs > Accessories > Communications open; the Windows 2000 Start menu alone and
with Programs > Accessories open; Windows 95's taskbar, desktop and Start-button right-click
menus; the eight dialogs; and from the v86 emulator, 19 states of the Taskbar Properties
window (Windows 95 and 98: each check box's effect on the preview and the second page; Windows
2000: the same, the focused tab and the Advanced page) and the Start menus of Windows 98 and
2000 with small icons. **From the reference PCs, 69 more**: the notification area of Windows 95
(no icons; the speaker, normal and muted), 98 (one, two and three icons, the speaker among
them) and 2000 (no icons; the speaker, normal and muted) at different times of day, Windows
98's tooltips (the clock's date, an icon's), the volume control of Windows 2000 in six states
and of Windows 95 in five (opened from the Run box and from the speaker; half, full and no
volume; muted; focus on the slider or on Mute) and of Windows 98 in two (from the speaker; then
muted), the speaker menus of Windows 95 and 98, each where its Windows opened it, and the
taskbars of all three with Notepad and folder windows open (20 states: five or six windows
opened one after another, each pressed as it opened, then none pressed; see [Classic icons for
today's windows](#classic-icons-for-todays-windows)), and the Start menu's Settings and Find
(Search) submenus of all three with their icons (23 states: opened by the pointer, then each
entry selected; see [Cascading menus](#cascading-menus)). Later additions (Date/Time
Properties, Start > Documents, [program windows](#program-windows) and their menus) brought
the count to **190 cases, all with 0 differing pixels** (2026-11-09).

## Colors

The names in `code` are RetroBar's resource keys. All of them were confirmed by the screenshots.

| Color | WIN95 | WIN2000 | Source | 98.css says |
|---|---|---|---|---|
| Face (bar and buttons) | `#C0C0C0` | `#D4D0C8` | RetroBar `ButtonFace`; Screenshot | `--surface: #c0c0c0` (agrees) |
| Light (inner top/left of raised buttons; top row of the bar; menus' outer top/left) | `#DFDFDF` | `#D4D0C8` | RetroBar `ButtonLight`; Screenshot S1–S4 (settles the old flagged item 2) | `--button-face: #dfdfdf` (agrees) |
| Highlight (outer top/left; second row of the bar) | `#FFFFFF` | `#FFFFFF` | RetroBar `ButtonHighlight`; **Reference PC** (`vm/<pc>-truecolor-tasks-1`: the bar's second row, the Start button's outer top and left) | `--button-highlight: #ffffff` (agrees) |
| Inner highlight (line across the top inside the active button) | `#FFFFFF` | `#FFFFFF` | RetroBar `ButtonInnerHighlight`; Screenshot S2, S11 | n/a |
| Shadow (inner bottom/right) | `#808080` | `#808080` | RetroBar `ButtonShadow`; **Reference PC** (the Start button's inner bottom and right, `vm/<pc>-truecolor-tasks-1`) | `--button-shadow: #808080` (agrees) |
| Dark shadow (outer bottom/right) | `#000000` | `#404040` | RetroBar `ButtonDarkShadow`; Screenshot | `--window-frame: #0a0a0a` (**differs**) |
| Text | `#000000` | `#000000` | RetroBar `ButtonForeground`; **Reference PC** (the "Start" label, `vm/<pc>-truecolor-tasks-1`) | `--text-color: #222222` (**differs**) |
| Flashing button face; menu selection | `#000080` | `#0A246A` | RetroBar `ButtonFlashingFace`; Screenshot (menu selections) | `--dialog-blue: #000080` (agrees) |
| Flashing button text; selected menu text | `#FFFFFF` | `#FFFFFF` | RetroBar `ButtonFlashingForeground`; **Reference PC** for the selected menu text (Start > Documents selected, `vm/<pc>-truecolor-documents-0`); no capture shows a flashing button | title text `white` (agrees) |
| Clock text | `#000000` | `#000000` | RetroBar `ClockForeground`; **Reference PC** (the clock, `vm/<pc>-truecolor-tasks-1`) | n/a |
| Desktop (Bevel95's own desktop) | `#008080` (Windows 98 too) | `#3A6EA5` | the Reference PCs' desktops as first installed (compare-refs cases `vm-*-desktop-color`) | n/a |
| Start menu banner | a picture: `#808080` with light gray lettering | a picture: dithered black-to-blue gradient, white lettering | Screenshot (see [Pictures](#pictures-banner-icons-flag)) | n/a |

## Bevels

Buttons and menus have a 2px bevel: an outer ring and an inner ring. In every ring the bottom
and right lines own the bottom-left and top-right corner pixels (all screenshots agree;
RetroBar and 98.css do the same).

| Bevel | Outer top/left | Outer bottom/right | Inner top/left | Inner bottom/right | Source |
|---|---|---|---|---|---|
| Raised (Start, task buttons) | Highlight | Dark shadow | Light | Shadow | RetroBar; Screenshot S2, S11 |
| Pressed (Start while its menu is open, the active task) | Dark shadow | Highlight | Shadow | Light | RetroBar; Screenshot S2, S4 |
| Menu window (Start menu, every menu) | Light | Dark shadow | Highlight | Shadow | Screenshot S1–S4, S7 |
| Clock well (1px only) | Shadow | Highlight | n/a | n/a | RetroBar `Tray`; Screenshot S2 |

## The bar

| Value | WIN95 | WIN2000 | Source |
|---|---|---|---|
| Height | 28 | 28 | RetroBar `TaskbarHeight`; Screenshot S2, S11 |
| Top edge | row 0 Light, row 1 Highlight | same | RetroBar; Screenshot |
| Gap above and below the button row | 2 | 2 | RetroBar; Screenshot |
| Button row (Start, tasks, clock well) | y = 4, height 22 | same | derived; Screenshot |

## Start button

| Value | WIN95 | WIN2000 | Source |
|---|---|---|---|
| Gap from the screen's left edge | 2 | 2 | RetroBar; Screenshot |
| Padding inside the bevel (left/right, top/bottom) | 2, 1 | 2, 1 | **Reference PC**: the flag at (6, 579) in every capture of the three, the button at (2, 576) (first RetroBar) |
| Flag | 16×16 | 16×16 | **Reference PC**: cut from the Start buttons, at (6, 579) on the 800×600 screens, in [`core/src/flag.rs`](../core/src/flag.rs) (first RetroBar's `start9x.png`); matches S2 and S11 pixel for pixel |
| Gap between flag and label | 2 | 2 | RetroBar; Screenshot |
| Label | "Start", **bold** | "Start", **bold** | RetroBar; Screenshot |
| Space after the label | 1 | 0 | RetroBar; Screenshot |
| Pressed: contents move | 1 right, 1 down | same | RetroBar; Screenshot S4, S10 |
| **Pressed while the Start menu is open: dotted focus rectangle** | **yes**, 3px in from every edge; every other pixel of its outline (the first one after each corner) swapped with its partner in the 16-color palette, which turns face into black and black into face | **no** | Screenshot S1, S3, S4 (95), S13 (98) and S10 (2000, none) |
| **Resulting size** | **54 × 22** at x = 2 | **55 × 22** at x = 2 | derived; Screenshot S2 (95: x 2–55), S11 (2000: x 2–56) |

## Task buttons

| Value | WIN95 | WIN2000 | Source |
|---|---|---|---|
| Gap between Start and the first button | 4 | 4 | RetroBar; Screenshot S2 |
| Width while there is room | 160 | 160 | RetroBar; Screenshot S2, S11 |
| Gap after each button | 3 | 3 | RetroBar; Screenshot |
| Narrowest before scrolling | **22**: just the icon | 22 | **v86** (Windows 98: 14 icon-only buttons of 22 on a 640 screen; RetroBar says 45: [D10](#discrepancies)) |
| Shrinking | Each slot (button + gap) is 163 while there is room, else every slot gets the same rounded-down share; the pixels left over stay empty at the end. Below 25 per slot: [scroll arrows](#window-menu-and-scroll-arrows). | same | **v86** (Windows 98 and 95: 9 buttons of 94 in 846; RetroBar spreads the leftover pixels: [D11](#discrepancies)) |
| Padding inside the bevel | 2, 1 | 2, 1 | RetroBar; Screenshot |
| Icon, then gap | 16, **2** | 16, **2** | **Screenshot S2, S11** (RetroBar says 3: [D5](#discrepancies)) |
| A window without an icon | its title starts where the icon would be | same | Screenshot S11 ("NetMeeting") |
| Space after the label | 1 | 1 | RetroBar |
| Long titles | cut and end in `...`; a pressed button's bold MS Sans Serif label is measured as GDI measures it, its 1px overhang included, so it is cut one letter sooner ("Untitled - Note..." where the letters alone would allow "Untitled - Notep...") | same (Tahoma's bold has no overhang) | Windows 95's DrawText (see flagged item 5); **Reference PC** (98 taskbar, 4 and 5 windows) |
| **Active** button | pressed bevel; inside it a 2×2 checkerboard (Highlight where x+y is odd, Face elsewhere, counted from the inside top-left of the **first** button: Windows lines the pattern up with its task window, so a pressed button an odd number of pixels further right has it the other way round); a 1px Inner-highlight line across the top; contents 1px lower, not sideways; **bold** label | same | RetroBar; Screenshot S2 and S11 confirm the phase for the first button and the 1px-down-only move (settles old flagged items 3 and 4); **Reference PC** (95, 98 and 2000 taskbars with the second and fourth buttons pressed) shows it follows the first button |
| **Flashing** button | raised bevel, face = flashing face, text = flashing text, regular weight | same | RetroBar `TaskButtonFlashing` |
| First button | x = 60 | x = 61 | derived; Screenshot S2 (95) |
| **Dragged** button (Bevel95's own: no Windows before 7 dragged task buttons) | its look from before the press (raised, or the Active look for the window in front, its checkerboard moving with it); the pressed point stays under the pointer, 1x pixel for 1x pixel; it stops at the row's ends; drawn over the others; it takes the place its middle is over (the line between two places is the middle of the gap between them) | same | ours (PROVISIONAL), `tasks::drag_step` |
| Sliding buttons | a neighbour whose place changes, the dragged button at its release, every button after Esc: **100 ms**, quick at first and gentle at the end (quadratic ease-out: three quarters of the way at half the time), whole 1x pixels | same | ours (PROVISIONAL), `tasks::SLIDE_MS` |
| Repaint while dragging or sliding | the task area only: at each move of the pointer and on a 15 ms timer (Windows' timers come about every 16 ms; measured live: 396 steps in 6.5 s), stopped at rest | same | ours, `bar::MOTION_MS` |

## Notification area and clock

The sunken well on the right: the programs' notification icons, then the clock. Windows 98's
numbers apply to 95 (its well without icons matches them); 2000 has its own.

| Value | WIN95 (and 98) | WIN2000 | Source |
|---|---|---|---|
| Gap to the screen's right edge and to the last task slot | 2 | 2 | RetroBar; Screenshot S2 |
| Border | 1px sunken | same | RetroBar; Screenshot |
| Padding inside the border | 2 | 2 | **Reference PC**: the first icon starts 3 right of the well's left edge in 95 and 98 (1 border + 2), 4 in 2000 (and the 1 in below), `vm/<pc>-truecolor-tray-volume` (first RetroBar) |
| **The clock's part** | **as wide as the widest time of day + 10** (46 + 10 = 56 for "12:00 AM"), the current time **centered** in it, rounded left: it keeps its width all day | **widest + 19** (46 + 19 = 65), centered | **Reference PC**: three times of day each (95: "12:33 AM"; 98: "12:05 AM", "3:15 PM", "7:27 AM"; 2000: "12:22 AM", "8:19 AM", "8:20 AM"); Screenshot S2, S11, S14. Explains the old "9 / 8" and "13 / 13" margins, which only held for those screenshots' times ([D6](#discrepancies)) |
| Where the clock's part starts without icons | 1 in from the padding | flush with the padding | Reference PC; Screenshot S2 |
| Notification icons | 16×16, centered vertically (7 below the bar's top) | same | Screenshot S11, S14 |
| First icon | flush with the padding | 1 in | Reference PC; Screenshot S11, S14 |
| From one icon to the next | **17** | **22** (PROVISIONAL: only one icon seen) | Reference PC (98, two icons) |
| The clock's part after the last icon's step | 2 | 0 | Reference PC; Screenshot S11, S14 |
| Without a clock (Show Clock off) | the icons and the step after the last; no icons and no clock: no well | same | PROVISIONAL |
| New icons | join the **right** end, next to the clock | same (PROVISIONAL) | Reference PC (98: the display settings icon, switched on after Task Scheduler's, came right of it) |
| **Size for "1:49 PM"** without icons | **63 × 22** at x = 575 on a 640px screen | | Screenshot S2 |

**Tooltips** (the clock's date, an icon's text), the same in every look: a 1px Window Frame
border (`#000000`), Info Background (`#FFFFE1`) inside, Info Text (`#000000`) in the bar's font;
the text 2px in from the border at the sides and 1px at the top and bottom, so one line is
17 tall and its text width + 6 wide (Screenshot S10 and S13, whose wrapped lines count their
trailing space; Reference PC). **Where**: at the pointer's x, normally 20 below it; with no room
below (always, over the bar) its bottom edge sits on the top edge of what it describes (an
icon's top; for the clock, 2 rows below the well's top), and it is moved left to end 1px inside
the screen's right edge. Moving the pointer within the clock does not move it (Reference PC,
98). It appears after a double-click time's rest (a fifth of that when another just showed)
and goes after ten.

**The speaker** (Windows' volume icon, shown in its place): one 16×16 picture in 95, 98 and
2000, and the muted one with a red "no" sign (Reference PCs; identical to Screenshot S11; in 95
and 98 its light gray pixels look like the bar's gray). Its tooltip: "Volume". Its right-click
menu, the same in all three: **Open Volume Controls** (bold) and Adjust Audio Properties. Where
the menu opens: Windows 95 puts it on the screen's bottom edge, over the taskbar, as if clicked
at the pointer's x there (its right edge at the pointer near the screen's right edge); Windows
98 and 2000 open it at the pointer, flipped up and left (Reference PCs).

**The volume control** ("sndvol32 /t"), measured on Windows 2000, 95 and 98 (Reference PCs),
from its top-left corner:

| Value | All three | Source |
|---|---|---|
| Window | 81 × 136, raised window edge (outer light and dark shadow, inner white and shadow) | Reference PC |
| Font | **MS Sans Serif** 8pt, regular, even in 2000 (the old dialog font "MS Shell Dlg", not Tahoma) | Reference PC: "M" 9 wide, its underline too |
| "Volume" | centered across the window, its text cell 10 down | Reference PC |
| Wedge | 11 × 66 at (11, 34): white top edge and slope (one column right every 6 rows), shadow right edge | Reference PC |
| Slider | focus outline (30 × 81) at (27, 27); groove 4 × 65 at (37, 35), sunken; thumb 21 × 11 at x 29, raised; the thumb's top travels 54 rows, top = loudest | Reference PC: Home, half, End |
| Mute | 13 × 13 sunken box at (9, 117), white inside, standard check mark 3 in; label at (27, 117), access key M | Reference PC |
| Focus rectangles | slider: dots one pixel after the corner, inverting (white XOR); Mute: dots on the corner, XORed with the face color (black on gray; they gray out the access key's underline where they cross it) | Reference PC |
| Keyboard cues | opened with the mouse: no focus rectangle and no underline until a key is pressed (2000); 95 and 98 always show them | Reference PCs |
| Where it opens | 2000: at the pointer, flipped up and/or left to stay on the screen. 95 and 98: on the screen's bottom edge (over the taskbar), right of the pointer if it fits there, else left of it | Reference PCs: 2000's corner at the pointer; 95's right edge at the pointer (733), 98's left edge at it (710) |

95 and 98 show the same window in their colors, and while it is open their taskbar shows a
**"Volume Control" button** (2000's does not; Bevel95 shows none yet). Esc does not close 98's
(Bevel95's closes in every look, and so does a click on the bar, which never takes the focus).
Behavior that no picture fixes: an arrow key moves 2%, a page (a click beside the thumb, Page
Up/Down) 20%, a wheel notch 6%; Windows 2000's own slider moved more finely.

### Network and battery icons (Bevel95's own, from each Windows' files)

Windows 11 hands out neither icon, so the bar shows its own (`core::status`), leftmost in the
well. Every picture and word below was read out of the Windows files on the reference PCs'
disks (`tools/dev/icons.ps1`, the files in `private/reference/refs/files`; 95's Dial-Up Networking from its
CD, as it was not installed). None of these Windows knew Wi-Fi: the connection icon stands for
whatever connection the PC uses.

| Item | 95 | 98 | 2000 |
|---|---|---|---|
| Connection pictures | RNAAPP.EXE 110 (idle), 111, 112, 113 (both screens lit), 114 (red X) | RNAAPP.EXE 110 to 114 (different pixels from 95's) | NETSHELL.DLL 193 (idle), 192, 191, 190 (both lit), 195 (one computer, red X: "Network cable unplugged") |
| Connection tooltip | "Connected to %s" (RNAAPP string 204) | same | name, then "Speed: %s", "Sent: %s packets", "Received: %s packets" (NETSHELL 2202 to 2206) on separate lines; offline on a cable "Network cable unplugged" (2207) |
| Connection menu | **Status**, Disconnect (RNAAPP menu 100; Disconnect grayed: Bevel95 does not hang up) | same | **Status**, Open Network and Dial-up Connections (NETSHELL menu 15100) |
| Battery pictures | SYSTRAY.EXE 306 (plug), 200 (charging), 300/301/302 (high, medium, low), 304 (unknown) | POWERCFG.CPL 200 (plug), 202 8-bit (plug and battery: charging); batteries from BATMETER.DLL's 16-pixel strip (bitmap 301) | as 98, but charging is STOBJECT.DLL 200 (95's picture) |
| Battery tooltip | "85% remaining", "85% (charging)", "AC power", "Battery remaining: unknown" (SYSTRAY strings) | "1:30 hours (85%) remaining", "15 min (20%) remaining", "85% remaining", " (charging)", "On AC power", "Unknown remaining" (SYSTRAY 1034 to 1050) | the same words (STOBJECT 158 to 163) |
| Battery menu | Adjust Power Properties, Open Battery Meter (95's three check items left out) | Adjust Power Properties, Open Power Meter | same |

What they do: a click on the connection opens Windows 11's own Wi-Fi list
(`ms-availablenetworks:`), a double-click and Status its network status page, Open Network and
Dial-up Connections the Network Connections folder; a double-click on the battery and Open Power
Meter open Windows' battery page, Adjust Power Properties the Power Options panel. The battery
icon shows only on a PC with a battery. High above 66%, medium above 33% (Windows' own battery
flags use these limits).

Bevel95's own choices, PROVISIONAL (no picture or text settles them):
- **Which screen means what**: the back computer lights while sending, the front one while
  receiving (the resource order idle, back, front, both suggests send before receive).
- **When the screens light**: the originals lit at every packet; today's PCs exchange a few
  packets almost every quarter second in the background, so a screen lights only for 16 or more
  packets in 200 ms (`status::ACTIVITY`).
- **"Not connected"** (95 and 98 offline): the dial-up icon simply went away then; Bevel95 keeps
  it (red X) so the Wi-Fi list is always one click away.
- **The 98/2000 batteries' place in their square**: they come from a strip, not from icons;
  centered (3 pixels in). The plug in the same strip is exactly POWERCFG.CPL's icon.
- The connection's name: Windows 11's ("Wi-Fi", "Ethernet"), with the Wi-Fi network's name in
  brackets, where 2000 said "Local Area Connection".

**The battery's percentage beside its icon** (Bevel95's own option, off unless switched on in
Bevel95 Settings' Taskbar page; no classic Windows had it). All numbers are Bevel95's own,
PROVISIONAL, the same in the three looks (`layout::NUMBER_EXTRA`, `NUMBER_PAD`):

| Value | 95, 98 and 2000 |
|---|---|
| Text | the percentage with the sign ("87%", "100%" on AC power); none while Windows does not know it, and none without a battery icon |
| Font, color, baseline | the clock's (`colors.clock_text`, `text.clock_baseline`), in True Color and 16 Colors alike |
| The number's part | as wide as "100%" + 10 (95 and 98: 26 + 10 = 36; 2000: 29 + 10 = 39), so the area keeps its width as the number changes |
| Where the part starts | the icons' gap (95/98: 2, 2000: 0) after the battery icon's step |
| The number in its part | 5 in from the part's left edge, left-aligned |
| What comes after the part | the next icon starts at the part's end; everything right of it, and the clock, keeps its place: the well grows to the left by the gap and the part (95/98: 38, 2000: 39) |
| Pointer and clicks | the part and the gap before it belong to the battery icon (its tooltip, click, double-click, right-click) |

## Quick Launch

Measured on the Reference PCs' True Color captures (`vm/<pc>-truecolor-tasks-<n>`,
`-ql-default-0`, `-ql-kbd-focus-<icon>`, `-ql-pressed-<icon>`, `-ql-hover-<icon>-tooltip`;
the 256- and 16-color captures give the same geometry) and GUIdebook's S14 and S11. 800×600,
the bar from y 572; L = the x of the etched line after Start. Code: `core::quick_launch`
(`layout`), `paint::paint_quick_launch`, `tooltip::place_quick_launch`.

| What | 98 (and the 95 look, by 98's rule) | 2000 |
|---|---|---|
| L (the line after Start) | 58: Start's right edge (`start.right()`) + 2 | 60: + 3 (Start is one wider) |
| Etched lines | shadow column at x, highlight at x+1, the buttons' rows (y 576..597) | same |
| Grip | L+4, 3×18 from y 578: highlight top row and left column, shadow bottom row and right column, face inside | same |
| Buttons | 23×22, x = L+11+23k, y 576..597, touching; the 16×16 icon at +3,+3 | same |
| Under the pointer (or keyboard focus) | a 1px raised edge: highlight top and left, shadow bottom and right (corners as `Framebuffer::frame`) | same |
| Held down | the edge inverted, the icon at +4,+4 | same |
| After the last button | 4 face columns, the second line at L+15+23n, its grip 4 after it | same |
| First task button | L+26+23n (176 with 98's four icons, 155 with 2000's three) | same |
| Task area's right end | 7 before the well (the line 4 before it: x 696/697 with the well at 700) | 6 before the well, no line |
| Tooltip | at the pointer's x, its bottom row on the button's top row - 1 (y 559..575) | one pixel left of the pointer, its bottom row 16 below the bar's top (572..588); 2000's Internet Explorer has a second line (its shortcut's description), growing upward to 559 |
| Tooltip words | "Launch Internet Explorer Browser", "Launch Outlook Express", "Show Desktop" (the shortcuts' file names); Windows Explorer's (Bevel95's own) "Windows Explorer"; the person's shortcuts their file name without the extension | the same, Internet Explorer's with "Finds and displays information and Web sites on the Internet." |

The pictures: True Color cut from the captures (98 cells x 72, 95, 118; 2000 x 74, 97, 120; y
579), 16 colors the files' 4-bit images (0 pixels differ from the 256- and 16-color captures).
Comparison cases `vm-<pc>-quick-launch-<n>` (the whole bar from x 0 to the well, with the windows
of `tasks-<n>`; 98's View Channels cell masked: Bevel95 leaves it out), `-hot-<icon>`,
`-pressed-<icon>` (the pointer masked), `win98-taskbar-quick-launch`, `win2000-taskbar-quick-launch`.

Not measured, Bevel95's own: Windows Explorer's place (after the look's own icons); at most 10
icons; the 95 look's band (95 had none: 98's rule on 95's bar). The bar's menu, the Toolbars
submenu, the band's menu and the icon's menu are as captured (`captures.md`, 2 and 3); 2000's band
menu with Adjust Date/Time and Task Manager... as its capture shows. The menu on an icon is
shortened to Open, Delete (and Properties for the person's shortcuts) in this first pass.
PROVISIONAL: the words "Show Quick Launch" in Bevel95 Settings.

**Quick Launch off** (98 and 2000; `ql-off-0`, `-off-1`, `-off-6`): the line after Start and its
grip stay, the task buttons start at L+11 (98 x 69, 2000 x 71); the task area ends as with the
band (98: 7 before the well and its line 4 before it; 2000: 6 before it). Six windows: 98 buttons
98 wide on a step of 101, 2000 101 on 104. The 95 look's bar has no bands. Cases
`vm-<pc>-quick-launch-off-<n>`.

## Text

| Value | WIN95 | WIN2000 | Source |
|---|---|---|---|
| Font | MS Sans Serif (the bitmap font) | Tahoma | CLAUDE.md; RetroBar |
| Size | 11px per em (8pt at 96 DPI) | 11px | RetroBar; every screenshot's text matches GDI's glyphs exactly |
| Cell height, baseline | 13, 11 | 13, 11 | GDI |
| Bold | GDI's simulated bold of the bitmap font, exactly what Windows 95 did | Tahoma Bold | GDI; Screenshot |
| One glyph differs from today's font | none | Windows 2000's Tahoma bends the tail of `y` one row higher (row 11: `.#...`); the bar uses that shape | Screenshot S12 ("Accessibility", "System Tools", "Synchronize"); applied in [`core/src/text.rs`](../core/src/text.rs) |
| Labels (Start, tasks): baseline row in the 16px content box | 12 | 12 | Screenshot S2, S11 |
| Clock: baseline row in the 16px content box | **13** (1px lower than the labels) | 13 | **Screenshot** S1, S2 (95), S11 (2000) ([D7](#discrepancies)) |
| Characters outside Windows-1252 | Windows' best-fit character in code page 1252 where it has one (452 of U+0100..U+FFFF: U+0141 as `L`, U+2011 as `-`, U+202F as a no-break space), else `?`. Bevel95's own: the round dots U+25CF, U+2B24, U+26AB as the bullet U+2022; invisible marks (U+200B..U+200F, U+202A..U+202E, U+2060..U+2064, U+FE00..U+FE0F, U+FEFF) nothing, 0 wide | same | what Windows 95 showed (it read titles through that conversion); table from WideCharToMultiByte(1252, 0) by `tools/dev/bestfit.ps1`, [`core/src/best_fit.rs`](../core/src/best_fit.rs) |

## Start menu

Measured on Screenshot S1, S3 and S4 (95) and S10 and S12 (2000). The code is
`start_menu` in [`core/src/theme.rs`](../core/src/theme.rs) (`large`) and `MenuStyle::Start` in
[`core/src/paint.rs`](../core/src/paint.rs).

| Value | WIN95 | WIN2000 |
|---|---|---|
| Frame | 3: the menu bevel above, then 1px of face | same |
| Banner | 21px wide, from the frame down to the frame; its picture sits at the bottom, the rest is the picture's top color | same, other picture |
| Items | 32 tall, from the banner's right edge to the frame | same |
| Icon (32×32) | 6px into the item, top of the item | **2px** into the item |
| Label | 6px after the icon (44 into the item); its 13-row cell centered, **an odd spare row below** | **9px** after the icon (43 into the item); **an odd spare row above** |
| Selection | the whole item, selection color; label and arrow white | same |
| Submenu arrow | 4×7 triangle; its right edge 6px from the item's; top (item height − 9) / 2 | right edge **5px** from the item's; top (item height − **7**) / 2 |
| Separator | 9 tall; shadow line on row 2, highlight on row 3; full item width | **8** tall; lines on rows 3 and 4; **2px** short at both ends. The one under Windows Update is 1 row taller (an extra row above the line). |
| Access keys | always underlined | underlined only when the menu was opened from the keyboard (or once a key is used) |
| Width | 3 + 21 + 44 + longest label + **31** + 3 | 3 + 21 + 43 + longest label + **25** + 3 |
| Position | left edge = the Start button's; bottom edge directly on top of the button (over the bar's top 4 rows) | same |
| Entries | Programs, Documents, Settings, Find, Help, Run..., ―, Shut Down... | Windows Update, ―, Programs, Documents, Settings, Search, Help, Run..., ―, Log Off *name*..., Shut Down... |

## Cascading menus

The menus that open from the Start menu (Programs and its folders, Documents, Settings,
Find/Search), with 16×16 icons. A task button's list of windows uses the same look. Measured
on Screenshot S3 (95) and S12 (2000); Settings and Find on the Reference PCs (which have no
network, so Find has no "Computer..."), 23 compare-refs cases (`vm-<era>-settings-*`,
`-find-*`, `-search-*`), icons included.

| Value | WIN95 | WIN2000 |
|---|---|---|
| Frame | 3 | 3 |
| Items | **22** tall | **20** tall |
| Selection | the item minus **1 row at the top and bottom** (20 rows), selection color | the whole item |
| Icon (16×16) | 6px into the item, centered (3 rows down) | 2px into the item, centered (2 rows down) |
| Label | 6px after the icon (28 into the item); cell centered, spare row below | 9px after the icon (27 into the item); spare row above |
| Submenu arrow | as in the Start menu | as in the Start menu |
| Width | 3 + 28 + longest label + **25** + 3, the label measured as its raw text: an access key's "&" counts as a letter (6px), so Settings and Find are 6px wider than their labels need | 3 + 27 + longest label + **21** + 3 |
| Settings | Control Panel, Printers, Taskbar... | Control Panel, ―, Network and Dial-up Connections, Printers, Taskbar & Start Menu... |
| Find (2000: Search) | Files or Folders... | For Files or Folders..., On the **I**nternet... (For People... left out: no counterpart today) |
| Longest label | 300px (longer window titles end in "...") | same (our choice: no screenshot has one) |
| Too many entries for the screen | more columns to the right, each as wide as the widest entry | same (PROVISIONAL: no screenshot shows it) |
| Empty folder | one grayed "(Empty)" entry | same (PROVISIONAL: from memory) |
| Folder icon | the classic "program group" icon | the 2000 one |

**Documents** (read on the Reference PCs after opening zebra, apple and mango, in that order,
from Explorer; five compare-refs cases `<era>-documents*`, 0 differing pixels, the documents'
own icons masked): at most the 15 most recent documents, named as Explorer names them (known
extensions hidden by default). Windows 95: by name, ignoring case; no other entry. Windows 98:
"My Documents", a line, then the documents in the order they were first opened (oldest
first); with none, nothing under the line. Windows 2000: "My Documents", a line, then by name;
with none, a grayed "(Empty)"; the menu has one face row more under its last entry than
2000's other menus. "My Documents"' icon is the shell's own (`icons::my_documents`), a few
colors off the window's picture. Documents opened from the Run box did not appear on 98 (only
those opened from Explorer); Bevel95 lists what Windows 11 records, less its app links (screen
clips, Game Bar), Control Panel pages and folders, which these Windows never listed.

## Windows' own menus

Text menus as Windows itself draws them: the bar's right-click menu with its Bevel95 submenu (and
in it the Theme and Size submenus), and a task button's right-click menu. Windows 95 measured on Screenshot S6, S7 and S8
(the taskbar's, the desktop's and the Start button's right-click menus) and on **v86** (the
window menu of Windows 95 and 98, Windows 98 Notepad's Edit menu); Windows 98 measured the same.
**Windows 2000** measured on the **Reference PC** (its taskbar's right-click menu and the
speaker's): labels, items, arrows, separators. Its texts are not compared pixel for pixel,
because that Windows 2000's Tahoma differs from today's in a few letters' widths
([D16](#discrepancies)); what depends on them is PROVISIONAL.

| Value | WIN95 (and 98) | WIN2000 |
|---|---|---|
| Frame | 3 | 3 |
| Items | **17** tall | 17 (Reference PC) |
| Label | 21px into the item; text cell 1 row below the item's top | **17**; 1 row below (Reference PC) |
| Space after the longest label | 21 (holds the arrow) | 21 (PROVISIONAL: fits the taskbar menu with today's Tahoma) |
| Shortcut text ("Alt+F4", after a tab in the label) | starts **8** after the longest label; **19** follow the widest shortcut text, measured in the regular font even when drawn bold (v86: Windows 95's window menu 21 + 43 + 8 + 30 + 19 = 121 wide; 98 Notepad's Edit menu 21 + 55 + 8 + 28 + 19 = 131) | same |
| Default entry | **bold**, label and shortcut text; widths measured in its own font (Screenshot S8: "Open"; v86: "Close") | same |
| Access-key underline under bold text | **1px longer** than the character: GDI's overhang for the bold it makes up from MS Sans Serif (Screenshot S8, v86); Tahoma has a real bold, so none (today's GDI no longer adds it either: [GDI test](../platform/winkit/src/font.rs)) | none |
| Bold label's width | the same overhang counts: a menu whose widest label is bold is **1px wider** (Reference PCs: Windows 95's and 98's speaker menus, 174 wide) | none (real bold) |
| Selection | the whole item | same |
| Grayed-out entry | **etched**: a white copy 1px down-right under the gray, the label's underline and a picture too (Reference PCs in True Color: Minesweeper's window menu on 95, Calculator's Edit > Paste and window menu on 98). With 16 or 256 colors Windows drew them plain gray (v86: Windows 98 Notepad's Cut, Copy, Paste, Delete), which is what Bevel95 drew before | etched (Reference PC: Cascade Windows, Tile...) |
| Grayed-out and highlighted | gray on the selection color (v86: "Restore" in the window menu); the arrow keys highlight grayed entries too | same |
| Submenu arrow | right edge 6px from the item's; top (17 − 9) / 2 = 4 | same (Reference PC: Toolbars) |
| Separator | 9 tall; lines on rows 3 and 4; full width in 95 (Reference PC: Minesweeper's Game menu); in **98, 1 short of each end** (Reference PC: Calculator's window menu) | lines **1 short of each end** (Reference PC) |
| Window menu's shortcut column (with pictures) | as if labels came 17 after 21 (v86, 98) | kept at 17 + 21 = 38 (PROVISIONAL) |
| Access keys | always underlined | only when opened from the keyboard |
| Check mark | 7×7, its left edge 6px into the item, centered vertically (v86: Windows 98 Notepad's "Word Wrap"; Reference PC: Minesweeper's Game menu on 95) | **4px** into the item (Reference PC: Minesweeper's Game menu) |
| Option bullet | 5×5, centered in the first 21px (PROVISIONAL shape) | same |
| Position | at the click, flipped up and/or left to stay on the screen | |

## Window menu and scroll arrows

**Right-click on a task button** (Windows' own menu style, above). For a button with one window:
that window's own menu, as Windows 95, 98 and 2000 show it (v86); for a button with several
windows (grouping came with Windows XP): XP's group menu, drawn the same way.

| Value | WIN95 | WIN98 | WIN2000 |
|---|---|---|---|
| Entries | &Restore, &Move, &Size, Mi&nimize, Ma&ximize, separator, &Close Alt+F4 | same | same |
| Grayed out (Windows' own rule) | Restore unless minimized or maximized; Move when maximized; Size unless a normal window with a sizing border; Minimize/Maximize without that box or when already so; Close for windows that forbid it | same | same |
| Bold (default) | Restore for a minimized window, else Close (as RetroBar and Windows XP) | same | same |
| Pictures | **none** (v86) | Restore, Minimize, Maximize, Close glyphs (v86): Restore 10×9 at (4, 4), Minimize 7×2 at (5, 11), Maximize 10×10 at (4, 3), Close 9×9 at (5, 4), from the item's top-left; in the label's color, gray when grayed out | as 98 (PROVISIONAL) |
| Labels in a menu with pictures | — | start **17** into the item, but the shortcut column leaves room as if they started 17 after the usual 21 (v86: 144 wide, "Alt+F4" 89 into the item) | as 98 (PROVISIONAL) |
| Group menu | &Cascade, Tile &Horizontally, Tile &Vertically, separator, Mi&nimize Group, separator, &Close Group (PROVISIONAL: Windows XP's entries from memory) | same | same |

**Scroll arrows**, when the buttons do not fit even at 22 wide (Windows 98, v86):

| Value | All looks | Source |
|---|---|---|
| Where | flush with the right end of the task area (98: 4px before the tray's etched line, which our bar does not have) | v86 (98 and 95) |
| Size | 16 wide; the row's height split in two: up 11, down 11 (rows of 23, as in the emulated 95: 11 and 12) | v86 |
| Buttons per row | as many 25px slots as fit before the arrows; rows are paged, the first always full | v86 (98: 14 of 22 on 640; 95: 33 on 1024) |
| Look | Windows' scroll-arrow button (DrawFrameControl): outer light and dark shadow, inner white and shadow (not a push button, whose white is outside); a 3×2 black triangle: up tip at (7, 4), base (6–8, 5); down base (6–8, 5), tip (7, 6) | v86 (98); Windows 11's DrawFrameControl draws the same at 16×11 |
| Grayed at the first or last row | no: Windows 98 drew both arrows black on the first row | v86 |
| Pushed | sunken (shadow and white outside, dark shadow and light inside), arrow 1px down-right | DrawFrameControl (PROVISIONAL: not seen on 98) |
| Clicking | acts as the button goes down, like Windows' up-down control; another window coming to the front shows its button's row | PROVISIONAL (not seen on 98) |

## Submenu placement

| Value | WIN95 | WIN2000 | Source |
|---|---|---|---|
| Horizontal | 6px over the parent menu's right edge (both 3px frames overlap) | same | Screenshot S3, S6, S12 (every cascade) |
| Vertical | the submenu's first item level with the entry (its frame 3px above) | same | Screenshot |
| No room below | opens upward (bottom level with the entry's bottom); if that does not fit either, moves up to end at the screen's bottom edge | moves up just enough, ending 1px above the screen's bottom edge | Screenshot S6 (95, "New"), S3 (95, Accessories), S12 (2000, Accessories) |
| No room to the right | opens to the left, overlapping the same 6px | same | PROVISIONAL |

## Dialogs

Run, Shut Down Windows and (Windows 2000) Log Off Windows, measured on the dialog
screenshots D1–D5 below; all five match them with 0 differing pixels (Windows 98's dialogs
are in [its own section](#windows-98)). The code is [`core/src/dialog.rs`](../core/src/dialog.rs).

| Value | WIN95 | WIN2000 |
|---|---|---|
| Frame | 3: Light/Dark, Highlight/Shadow, then 1px face (the menu bevel) | same colors of 2000 |
| Title bar | 18 tall at (3, 3), solid `#000080` | Gradient `#0A246A` → `#A6CAF0` up to the "?" button or 2px before a lone "X", with the end color under the buttons. Bevel95 uses 32 fractional bits for the per-column step to match the captured colors |
| Title | bold, white, 2px in from the bar's corner | Tahoma Bold |
| Title buttons | 16×14 raised, 5px from the top; "X" 21px from the dialog's right edge, "?" 18px left of it; glyphs in black | same |
| Push buttons | 75 or 78 × 23, raised; the default one has a 1px black frame around it; label centered (spare pixel to the right, text cell 1 row above center); disabled: the label etched (white 1px down-right under gray); focus: dotted rectangle 4px inside | same |
| Pushed button | a 1px shadow frame, label 1px down-right (PROVISIONAL: no screenshot) | same |
| Text field (Run) | sunken 2px (Shadow/Highlight, Dark/Light), white; text and the 1×13 caret 1px in | text 4px in |
| Drop-down button | 16×17 inside the field's right end, window bevel, 7×4 arrow 4px in and 6px down | same |
| Option buttons (95 Shut Down) | 12×12 pictures (on/off), label 17px right; the focus rectangle runs 1px around the label (2px more below) | (none) |
| Choice list (2000 Shut Down) | (none) | the chosen entry in the selection color while it has the focus, text 1px in; the open list is PROVISIONAL (white, 1px black frame, 13-row entries) |
| Underlines and focus rectangles | always | only after the keyboard is used |
| Run | 347×163; icon (14, 40); text (57, 40); "Open:" (14, 85); field (57, 82, 275×21); OK, Cancel, Browse... at y = 124, x = 96, 177, 258 | 347×179; same, buttons at y = 136 |
| Shut Down | 347×222; text (60, 40); options at (61, 65 + 21·i); Yes, No, Help at y = 183, x = 87, 171, 255 (78 wide) | 417×269; banner (3, 22, 411×77); icon (23, 110); question and explanation at x = 69; list (69, 132, 260×21); OK, Cancel, Help at y = 232, x = 167, 248, 329 |
| Log Off | (Windows 95 logs off from Shut Down) | 288×123; icon (23, 40); text (77, 46); Yes, No at y = 84, x = 77 and 152 (65 wide) |
| Position | Run: lower left, 12px from the screen's edge and above the bar (PROVISIONAL); Shut Down and Log Off: centered on the screen (Toasty Tech) | same |
| Shut Down dims the screen | every other 1× pixel black (where x + y is odd), the bar included | same |

## Program windows

A program's own window: frame, title bar with the program's picture and the Minimize, Maximize
and Close buttons, and the menu bar. Measured on the **Reference PCs** in True Color: the real
Minesweeper on Windows 95, 98 and 2000 and Calculator on Windows 98 (measured first: 98's
Games were installed later the same day), each in front, behind another window, with a menu
title chosen by the keyboard and with a menu open. The geometry is the same in all three;
colors and the way a title shows differ. The window chrome and menu comparisons include
the 146×129 Windows 2000 system menu, which matches with 0 differing pixels. The code is
[`core/src/window.rs`](../core/src/window.rs); the dialogs' frames and title bars use it too.

| Value | WIN95 | WIN98 | WIN2000 |
|---|---|---|---|
| Frame | 3: Light/Dark shadow, Highlight/Shadow, 1px face | same | same, 2000's colors |
| Title bar | 18 rows at (3, 3); one face row below it | same | same |
| Menu bar | 18 rows at y = 22, from x = 3; one face row below; the contents start at (3, 41). A window is its contents + (6, 44) | same | same |
| Program's picture | 16×16 at the title bar's corner + (2, 1) | same | same |
| Title | bold, 20px after the title bar's left end (2px after the picture's part), 2 rows down | same | Tahoma Bold |
| Buttons | 16×14, 5 rows from the window's top: Close 21 from the right edge, Maximize 18 left of it, Minimize against Maximize. Raised like the dialogs' "X" | same | same |
| Glyphs (from the button's corner) | Minimize 6×2 at (4, 9); Maximize a 9×9 box with a 2-row top at (3, 2); Close the dialogs' "X" | same | same |
| Maximize of a window that cannot grow | grayed out: gray glyph over a white copy 1px down-right | same | same |
| Title bar, in front | `#000080`, title white | gradient `#000080` → `#1084D0`, starting after the picture's part (18px) and ending 2px before Minimize ([the rule](#windows-98)) | Gradient `#0A246A` → `#A6CAF0` over the same span, using the dialog painter's per-column step |
| Title bar, behind | `#808080`, title `#C0C0C0` | gradient `#808080` → `#B5B5B5`, title `#C0C0C0` | gradient `#808080` → `#C0C0C0`, title `#D4D0C8` |
| Menu titles | side by side from x = 3, each its label + 12 wide (6 each side), 18 tall; label cell 2 rows down | same | same |
| Title chosen (Alt, arrows) or its menu open | filled with the selection color, text white | sunken 1px frame (Shadow/Highlight), text 1px down-right | as 98 |
| Pointer on a title | nothing | nothing | raised 1px frame (Highlight/Shadow) |
| Titles of a window behind | black | `#808080` | `#808080` |
| Access keys underlined | always | always | only after Alt is used |
| Drop-down menu | [Windows' own menu](#windows-own-menus); its corner at (title's left edge, 40): right under the bar | same | same |
| Window menu (Alt+Space, the picture) | Corner at (3, 21), under the title bar's left end | Same | Same position. The 146×129 menu uses 20px rows for items with system bitmaps, 17px plain rows and a 9px separator |
| Buttons held down | sunken, glyph 1px down-right (PROVISIONAL: not captured) | same | same |

## Minesweeper

In Explorer, Minesweeper uses the 16×16 and 32×32 mine from Windows 95's WINMINE.
Bevel95 uses its own silver beveled B at those sizes and the viewer uses Windows 95's Help
icon. For these three executables, the 48×48 frame
enlarges the 16×16 pixels threefold. The 64×64 and 256×256 frames enlarge the 32×32
artwork by two and eight respectively.

The True Color captures in `private/reference/refs/vm/{win95,win98,win2000}-truecolor-mine-*.png`
show the board at 1×. The client begins at (3, 41) inside the window. In the table,
W and H count squares. All other measurements are in pixels.

| Measurement | Windows 95 and 98 | Windows 2000 |
|---|---|---|
| Whole window after a level change | (16W + 30) × (16H + 111) | (16W + 26) × (16H + 107) |
| Beginner window | 160×239 at launch, 158×239 when switching back to Beginner, both with an 8×8 field | 170×251 with a 9×9 field |
| Intermediate / Expert window | 286×367 / 510×367 | 282×363 / 506×363 |
| Board surround | Light gray with a white top and left edge and a dark gray bottom and right edge | Light gray even though the surrounding window uses the Windows 2000 face color |
| Counter panel | At client (9, 9), (16W + 6) × 37, with a 2px sunken bevel | Same |
| Counter wells | 41×25 at client (16, 15) and (16W − 35, 15). Three 13×23 digits fill each black display | Same |
| Face button | 26×26 centered in the panel, including its 1px border | Same footprint |
| Field frame | At client (9, 52), (16W + 6) × (16H + 6), with a 3px sunken bevel | Same |
| First square | 16×16 at client (12, 55), or window (15, 96) | Same |
| Raised square | 1px white top and left, gray and black bottom and right | 2px white top and left, 2px gray bottom and right, diagonal corners |
| Custom Field dialog | 237×136, opens at window (27, 67) | 201×163 at window (3, 41) |
| Scores dialog | **Best Times**, 330×179 at window (27, 67), with the group labeled **Fastest Mine Sweepers** | **Fastest Mine Sweepers**, 261×147 at window (3, 87) |

The shared sizes are Intermediate (16×16 with 40 mines) and Expert (30×16 with 99).
The captures also show a held square, an open number, a flag, a question mark, a loss,
the Game and Help menus, and the Custom Field and scores dialogs. The geometry and mouse
hit testing use the same rectangles. See [board.rs](../core/src/minefield/board.rs).

The tile, counter and face strips come from each edition's WINMINE bitmap resources
410/411, 420/421 and 430/431. Windows 95 and 98 share their color strips. Windows 2000
has different color faces and tiles. All three share the digits and monochrome artwork.
The Color menu switches the board to the matching monochrome strips.

The Custom Field and scores dialogs match the six reference windows pixel for pixel.
Windows 2000 uses MS Sans Serif 8pt for their controls and Tahoma Bold for the titles.
The new-record name prompt has a **Congratulations** title on Windows 95 and 98 and
opens without a title bar on Windows 2000.

The reference suite matches 55 game, menu and dialog captures with zero differing pixels
in the compared regions. The game checks replay held squares, opened
squares, flags, question marks and losses. They omit guest mouse pointers, registration
text and Windows 95/98 resource-heap values. Each About memory line is checked using the
guest's measured value.

## Solitaire

Measurements are at 1× in the True Color reference guests. The dealt hands and the
opening card back can differ from game to game.

| Measurement | Windows 95 and 98 | Windows 2000 |
|---|---|---|
| Initial window | 597×434 | 593×430 |
| Card face | 71×96 | 71×96 |
| Initial play area including the 17px status bar | 585×384 | 585×384 |
| Options | 342×217 | 207×189 |
| Select Card Back | 351×259 | 273×207 |

In the initial layout the deck starts at (11, 5) within the play area. The seven
columns begin at y = 107 and are 82 pixels apart. In a column, the next covered card
starts 3 pixels below the last one and the next face-up card starts 15 pixels below.
In Draw Three, each card in a packet starts 14 pixels to the right and 1 pixel down.
The four suit stacks line up with the rightmost four columns.

The native opening window matched in all three looks at 1× after substituting the
seeded deal's card faces and back into each reference screenshot using the original
CARDS.DLL artwork. The first draw matched throughout the window above the changing
status strip. A 2× capture also reproduced the 1× pixels in 2×2 blocks.

The Deck picker has two rows of six backs. All 36 thumbnails in the three native
dialog windows matched the reference guests. The Options dialog calls the Vegas balance
setting **Keep score** in Windows 95/98 and **Cumulative Score** in Windows 2000.
The 95/98 Help menu has **Help Topics** and **About Solitaire**. Windows 2000 shows
**Contents**, **Search for Help on...**, **How to Use Help** and **About Solitaire**.
The complete Windows 95 opening topic picker matched all 434×428 pixels. Windows
98's 525×450 Solitaire Help opening window matched the guest across its title,
toolbar, Contents tree, splitter and frame. The authored welcome text differed in
5,116 pixels and its scrollbar in another 127 pixels.
The Windows 2000 opening comparison differed in 2,736 article-text pixels. The frame,
toolbar and navigation matched across the full 550×450 window.

The 52 Solitaire reference comparisons cover 36 game and dialog views, four Windows
98 keyboard states, three Help opening windows, two Windows 95 articles and seven
sheets of original artwork. Those sheets include all 52 faces in normal and selected
colors, the 12 backs, three empty-pile markers and 14 animation frames. The four
comparisons with authored Help text account for differences in words and wrapping.

## Calculator

Measured at 1× on the three Reference PCs in True Color (`private/probes/calc/measurements.txt`,
the captures `private/reference/refs/vm/<pc>-truecolor-calc-*.png`). Positions are in the client area, which
starts at (3, 41) in the window, under the menu bar.

| Standard view | Windows 95 | Windows 98 | Windows 2000 |
|---|---|---|---|
| Window (client) | 276×272 (270×228) | 260×252 (254×208) | 260×252 (254×208) |
| Display (sunken 2 px, white) | (11, 20) 249×26 | (8, 0) 239×26 | (8, 0) 239×26 |
| Display text ends | 8 left of the display's edge | 2 left | 2 left, after a space (numbers only) |
| Memory box ("M" pen) | (11, 52) 36×26 (+13, +7) | (12, 39) 27×26 (+10, +6) | as 98 |
| Buttons | 36×28; columns 11, 63, 103, 143, 183, 223; rows 86, 120, 154, 188 | 36×29; columns 8, 54, 93, 132, 171, 210; rows 73, 106, 138, 171 | as 98 |
| Top row | Back 104, CE 157, C 210; 49×28 at y 52 | Backspace 54 (63 wide), CE 120 (62), C 185 (62); 29 tall at y 37 | as 98 |
| Captions | bold 10-point, cut from the capture (bevel-art's `calculator.rs`, looked up by `captions95.rs`) | MS Sans Serif 8, centered | MS Sans Serif 8 (not Tahoma), centered |
| Raised edges (outer, inner) | white / 3D light, shadow / black | 3D light / white, shadow / black | as 98 in 2000's colors |
| Pushed in | black / shadow, 3D light / white; caption 1 right, 2 down | shadow / black, 3D light / white; caption still | as 98 |

Caption colors: 95 digits, +/- and . blue (0000FF), / * - + = red, sqrt % 1/x navy (000080),
MC MR MS M+ red, Back CE C maroon (800000). 98 and 2000: digits, +/-, ., sqrt, %, 1/x blue, the
rest red. Windows 95 draws an etched line (808080 over FFFFFF) across the client at y 5 and 6.
Windows 98 paints face color over the bottom-left 4×5 pixels of M+ until a button is first
pushed (by the mouse or a key's flash): the rule that fits all of 98's captures.
No button ever shows a focus rectangle. A key flashes its button pushed in (about 150 ms after
the key it is down, by 250 ms up); the display changes as it comes up (Bevel95: 4 ticks of
50 ms).

Menus: the option bullet of 98's and 2000's View menu is 6×6 (`.####.` over four full rows),
centered in the check column; Windows 2000's Calculator underlines the access key of a grayed-out
entry with every other pixel (Edit > Paste with an empty clipboard; the window menu and Solitaire's
menus underline solid). View: 95 **Scientific**, **Standard**
(check mark); 98 **Standard**, **Scientific** (bullet); 2000 adds a separator and **D&igit
grouping** (its access key is the i). About Calculator is the shell's About box 33 right of and 74
below the window's corner; 95's credit wraps onto two lines (Developed for Microsoft / by Kraig
Brockschmidt), 98 and 2000 pass none.

The 64 Calculator comparisons (`BEVEL95_CASE=calculator`) all match to 0 pixels; masked are
the guests' mouse pointers (11×19, 2000's 17×25), Windows 95's key-flash capture's 8 button
(caught mid-repaint), and About's registration and resource lines as in Minesweeper's.

| Scientific view | Windows 95 | Windows 98 | Windows 2000 |
|---|---|---|---|
| Window (client) | 501×340 (495×296) | 480×309 (474×265) | as 98 |
| Display | (246, 16) 237×26; text 5 from its edge, 1 row lower than Standard | (216, 0) 249×26 | (8, 0) 458×26 |
| Groups (etched, no title) | bases (11, 55) 259×28, angles (276, 55) 207×28, Inv/Hyp (11, 88) 127×27 | (8, 29) 242×27, (251, 29) 215×27, (8, 61) 123×27 | (8, 29) 212×27, (221, 29) 245×27, (8, 61) 126×27 |
| Option buttons (circle, label pen +17) | y 63: Hex 21, Dec 82, Oct 144, Bin 202; Deg 286, Rad 352, Grad 421 (Dword Word Byte the same) | y 35: 13, 75, 136, 195; Degrees 258, Radians 324, Gradients 393 | y 35: 13, 63, 112, 162; Degrees 226, Radians 307, Grads 388; Qword 226, Dword 286, Word 346, Byte 406 |
| Check boxes (label pen +18) | y 95: Inv 21, Hyp 81 | y 68: 12, 74 | as 98 |
| Parentheses box, memory box | (143, 88) 36×27, (195, 88) 36×27 ("M" pen +12, +6) | (140, 62) 27×26, (189, 62) 27×26 | as 98 |
| Buttons | 36×28; columns 11, 63, 103, 143, 195, 247, 287, 327, 367, 407, 447; rows 122, 156, 190, 224, 258; Back/CE/C 49 wide at 328, 381, 434, y 88 | 36×29; columns 8, 57, 96, 135, 185, 234, 273, 312, 351, 390, 429; rows 98, 130, 163, 195, 228; Backspace/CE/C 65 wide at 266, 333, 401, y 62 | as 98 |

The columns: Sta Ave Sum s Dat | F-E dms sin cos tan | ( Exp x^y x^3 x^2 | ) ln log n! 1/x | MC
MR MS M+ PI (pi) | 7 4 1 0 A | 8 5 2 +/- B | 9 6 3 . C | / * - + D | Mod Or Lsh = E | And Xor Not
Int F. Colors: 95 Sta–Dat, PI, A–F navy, F-E…1/x purple (800080), Mod…Int red; 98 and 2000 F-E…1/x
magenta (FF00FF), Sta–Dat, pi, A–F blue, Mod…Int red, a grayed caption 808080 without etching. The
"(=n" text is centered in its box (pen y +6). 95's Scientific captions are cut from its capture like
the Standard ones. While an error shows, 95 and 98 draw the option buttons and check boxes switched
off (face inside, the dot in shadow, labels etched). Windows 98 paints over Dat's bottom-left corner
as over M+'s.

Statistics Box: 225×165 at screen (120, 130); list (8, 27) 210×56 (sunken 2 px, 13-pixel lines,
text pen +2, 95 +5; the chosen line in the menu highlight), RET LOAD CD CAD 42×23 at x 9, 63, 117,
171, y 116 (RET framed as the default), "n=N" centered at y 146 (one pixel left of center). Its
picture: 95 and 98 one, 2000 another (`stats.rs`); 2000 has "?" beside the X.

## Help viewer

Windows 95 uses a WinHelp topic picker with Contents, Index and Find. Choosing a topic opens
a separate article view. Windows 98 and 2000 use the HTML Help layout, with a toolbar across
the top, navigation tabs on the left and a topic on the right. Bevel95 draws both interfaces
from the app-supplied catalog. Its game articles have authored text, so their line breaks
and scrolling can differ from the original documents.

| Edition | Minesweeper Help opens to | Game topics |
|---|---|---|
| Windows 95 | WinHelp's topic picker | How to play Minesweeper, Strategy and Tips |
| Windows 98 | Welcome to Help, with a flat contents list | The object of Minesweeper, Playing Minesweeper, Strategies and tips |
| Windows 2000 | Minesweeper overview, with a Minesweeper book in the contents tree | Overview, Play Minesweeper, Customize the playing field, Strategies and tips |

On Windows 2000, **Search for Help on...** opens the keyword index. **Using Help** opens
a separate authored book about navigating the viewer. Windows 98's **About this Help book**
link describes the Bevel95 guide. Web Help uses locally written support topics for the
old online destinations.

At 1× the Windows 95 picker is 434×428. The two games use the same WinHelp article
window definition. Its default width is 360/1024 of the screen width, with a minimum of
200 pixels at 1×. The viewer wraps the text at that width and grows down from the
window's top, leaving five pixels above the bottom of the work area. When more height
is needed, the viewer adds a vertical scrollbar and wraps the text in the remaining width.

On the 800×600 reference desktop, original Solitaire opened **How to play Solitaire**
at 281×426 and its scoring article at 281×218. Narrowing the original window to 200 pixels
made the instructions overflow. The top stayed at y = 59 and the window grew to 508 pixels,
with the taskbar starting at y = 572. At the original 281-pixel width, Bevel95's authored
Solitaire topics fit at 377 and 407 pixels respectively. Minesweeper's two articles fit
at 362 and 343 pixels, matching the original window heights.

The Windows 98 and 2000 viewers open at 525×450 and
550×450 respectively. Their navigation splitters start at x = 224 and x = 254. The shared
viewer uses a 7px splitter for Windows 98 and a 4px splitter for Windows 2000.

Calculator's books (`*-truecolor-calc-help-*`) showed three more things. Only the first
Contents entry opens at the start (98's two top-level books: the first open and chosen, the
second closed). A row wider than the tree brings a 16-pixel horizontal scroll bar across the
bottom of the tree's inside (98: arrows at x 0 and 193 of the 209-pixel inside, 2000: 0 and
208 of 224); the scrolled extent is the widest shown row's selection mark (indent 19 per
level from 19, the label plus 4), 241 on 98 and 306 on 2000; the thumb is the page's share
of the track rounded as MulDiv does, 153 of 177 and 141 of 192; its checker starts one pixel
later than the vertical bars'; the left arrow's tip is at x 5 of the button, the right arrow's
back at x 6, both 7 tall centered at y 7. WinHelp's picker with a book chosen reads "Click a
book, and then click Open. Or click another tab, such as Index." and its first button
**Open** (Print stays enabled).

The Find dialog is 357×119 in both HTML Help editions. Windows 98 waits in Find until it
closes. Windows 2000 lets the reader use the viewer with Find open. From Contents, Print
opens a scope chooser measuring 294×162 on Windows 98 and 305×175 on Windows 2000.
On Windows 2000 an index entry with more than one matching page opens a 477×236
**Topics Found** chooser. Its entire window matched the guest capture in a native test.

The Minesweeper reference suite covers 36 Help states across the three editions (4 Windows 95,
16 Windows 98 and 16 Windows 2000) in addition to the 55 game, menu and dialog captures.
The Help comparisons report changed article-text pixels separately. They compare
the paper around the text, the frame, toolbar, tabs, lists and divider against captures.
For HTML Help it checks the article heading and the first three words of its
authored opening paragraph against the expected glyphs at the measured text positions.

Live 1× captures from the game's Help menu matched the full Windows 95 Contents picker.
The Index comparison differed by the 13 pixels of its blinking insertion caret. The initial
Windows 98 and 2000 windows matched outside the article and its scrollbar. The Windows 2000
scrollbar also matched. Its Web Help directory matched across the entire 550×450 window.
In both looks, Hide and Show keep the right edge of the window fixed.

## Taskbar Properties

Read from Windows 95 (with Internet Explorer 4), 98 and 2000 in the [v86 emulator](#sources);
all 19 captured states match with 0 differing pixels (`cargo compare-refs`, cases
`*-properties-*`). Windows 95 with Internet Explorer 4 and Windows 98 show **the very same
window**, pixel for pixel (only the mouse pointer differs); Windows 95's own 1995 version was
not found anywhere, so WIN95 shows this one (flagged item 19). The code is
[`core/src/properties.rs`](../core/src/properties.rs).

| Value | WIN95 / WIN98 | WIN2000 |
|---|---|---|
| Window | 344×400, frame and title bar as the dialogs above, "?" and "X"; title "Taskbar Properties" | same size; "Taskbar and Start Menu Properties" |
| Where it opens | 2px from the screen's left edge, its bottom 4px above the bar | same |
| Tabs | from x 11 on a row at y 31, 18 tall; each as wide as its label + 6 on each side, label at x + 6, y + 3; raised (Highlight/Dark outside, Light/Shadow inside, top corners cut off by one pixel); the chosen tab 2px larger on every side, running into the page; labels "Taskbar Options", "Start Menu Programs" | labels "General", "Advanced" (Tahoma) |
| Focused tab | dots 1px inside the tab's usual place, on pixels where x + y is odd (Windows 2000; assumed for 95 and 98) | same |
| Page | frame (9, 49, 326×312), raised like a button (Highlight/Dark outside, Light/Shadow inside) | same |
| OK, Cancel, Apply | 75×23 at y 367, x 98, 179, 260; Apply grayed until something changes (and stays enabled after a change is undone, as in Windows); "&Apply" | same |
| Grayed Apply | etched (as the dialogs' grayed buttons in full color); the emulator, in 16 and 256 colors, draws it flat gray ([D13](#discrepancies)) | etched |
| Check boxes | 13×13: Shadow/Highlight outside, Dark/Light inside, white within (gray while held down: PROVISIONAL); Windows 95's 7×7 check mark 3px in; label 18px right, its text cell level with the box's top; focus dots 1px around the label (2px more below) | same |
| Taskbar Options check boxes | (34, 226), (34, 257), (34, 288), (34, 319): "Always on &top", "A&uto hide", "Show &small icons in Start menu", "Show &clock" | (34, 226), (34, 252), (34, 276), (34, 301), (34, 325); the fifth, "Use &Personalized Menus", is grayed out (Bevel95 does not do it) |
| Preview | a 1px sunken frame (34, 74, 276×132), teal inside, put together from five pictures cut from the emulator: the sample window (62×117 at the frame's inside + (212, 0)), the Start menu with large or small icons (152×99 or 115×99 at + (2, 0)), the taskbar with or without its clock (269×29 at + (2, 100)). The taskbar is drawn over the window when it is always on top, under it when not; Auto hide replaces the taskbar with its top 4 rows (white, then face; Shadow and black on the right) under a dotted cyan outline of where it was | the same, with Windows 2000's own five pictures |
| Group boxes (Start Menu Programs) | etched frames (24, 68, 296×135) "Customize Start menu" and (24, 215, 296×101) "Documents menu"; label 9px in, its text cell 6 rows above the line, the line left out from 2px before the label to 1px after it | one group (25, 68, 296×181) |
| Start Menu Programs page | icons (43, 92) and (43, 238); texts at x 91, y 92 and 227; "&Add..." (48, 166, 78×23), "&Remove..." (132, 166, 80×23), "A&dvanced..." (217, 166, 78×23), "&Clear" (217, 280, 78×23) | icons (42, 79) and (42, 186); texts at (85, 79) and (87, 186), "Web sites,  click Clear." with two spaces; buttons 80×23 at x 229: "A&dd..." y 80, "&Remove..." 105, "Ad&vanced..." 131, "Re-&sort" 157, "&Clear" 217 |
| Start Menu Settings (2000) | (none) | label "Start Menu Se&ttings" at (24, 256); a sunken white list (24, 272, 297×73) of 16-row entries, flat 13×13 check boxes (2px black frame) 2px in, labels 21px in; a 16px scroll bar (arrow buttons as the menus' bevel, 7×4 arrows; track a checkerboard of white and face; thumb as long as Windows' MulDiv gives: 19 for 4 of 8 rows). Display Favorites and Display Logoff work; the other six are grayed out. The selection while the list has the focus: PROVISIONAL |
| Bevel95 Settings (ours: a second window of the same kind, titled "Bevel95 Settings", with the tabs "General", "Taskbar", "Desktop" and "Programs"; the Taskbar page is a group "Windows key" (24, 68, 296×68; option buttons at (36, 84 + 21·i), "Opens &Find" ("Opens Sea&rch" in the 2000 look) and "Opens the Start &menu") and 8 below it a group "Buttons" (24, 144, 296×69; the check box "&Group similar taskbar buttons" at (36, 161), one line of text at (36, 183)); the Desktop page is one group (24, 68, 296×88) with its check box at (36, 85) and three lines of text from (36, 107); the Programs page is one group "The Programs menu shows" (24, 68, 296×283) with three option buttons at (36, 84 + 21·i) and a tree in a sunken frame (36, 147, 272×196): 12 rows, a 16px scroll bar at its right while more rows show than fit; the tree itself is Windows' own, see "Trees") | "General": groups "Look" (24, 68, 296×89; option buttons at (36, 84 + 21·i)), "Size" (24, 165, 296×68; 1× to 4× at x 36, 96, 156, 216, y 181), "Startup" (24, 241, 296×47; the check box at (36, 258)), "Clock" (24, 296, 296×55; two lines of text from (36, 311), its button 23 tall at y 312, 12 in from the group's right edge), built from the parts above. It opens one 22px cascade step right of and above Taskbar Properties' place (PROVISIONAL) | same |

What each option does in Bevel95:

| Option | Effect |
|---|---|
| Always on top | on: the bar is a topmost window. Off: other windows can cover it; maximized windows still stop above it (Windows XP's behavior as remembered: PROVISIONAL); the Windows key brings it to the front |
| Auto hide | the bar gives its strip back (windows can use the whole screen) and slides down until 2 rows show (Windows 2000 in the emulator: its top edge, 2px). It slides up while the pointer is on that strip (or a menu of the bar is open, or the Windows key is pressed) and slides away 0.5 s after the pointer leaves, in steps of a sixth of its height every 15 ms (PROVISIONAL timings) |
| Show small icons in Start menu | the Start menu with 16×16 icons and no banner (below) |
| Show clock | off: no clock and no well (Windows leaves an empty notification area out) |
| Add... | Windows' own Create Shortcut wizard (`rundll32 appwiz.cpl,NewLinkHere`), into your Start menu's Programs folder |
| Remove..., Advanced... | your Start menu's Programs and Start Menu folders in File Explorer (Windows 95's Remove dialog is not rebuilt) |
| Re-sort | nothing: the Programs menu is always sorted here |
| Clear | `SHAddToRecentDocs` with nothing, Windows' documented way to empty the recent documents list: the Documents menu, and on Windows 11 also File Explorer's Recent files |
| Display Favorites, Display Logoff (2000) | add Favorites under Programs, show "Log Off <user>..." |

The Start menu with small icons, from the emulator (cases `win98-start-menu-small`,
`win2000-start-menu-small`, 0 differing pixels):

| Value | WIN95 | WIN98 | WIN2000 |
|---|---|---|---|
| Banner | none | none | none |
| Rows | 22 (PROVISIONAL: as its cascading menus; the emulator's 95 has Internet Explorer 4's menus) | 20 | 20 |
| Icon | 16×16, 6px in (PROVISIONAL) | 2px in | 2px in |
| Label | 6px after the icon (PROVISIONAL) | 9px after the icon (its cascading menus use 5 and 2) | 9px after the icon |
| Right of the longest label | 25 (PROVISIONAL; the Reference PC's menu is 6 wider: `vm/own-pictures/win95-truecolor-start-small`, private/todo.md) | 21 | 21 |
| Submenu arrow | 6px from the edge (PROVISIONAL; the Reference PC: its right end 9px from the edge) | 5px | 5px |
| Under the last entry | 0 | 0 (the large menu has 1) | 0 |
| First separator | as the large menu | as the large menu | 1 extra row above it, as the large menu |
| Icons | the Reference PC's (`vm/own-pictures/win95-truecolor-start-small`: rows 22, icons 6px in and labels as here, exact) | its 16-color versions | its own, all from the Reference PC (`vm/own-pictures/win2000-truecolor-start-small`) |

## Trees

Windows' tree view, read on the Reference PCs (2026-11-11) from the tree of the Programs menu
in "Remove Shortcuts/Folders" (Taskbar Properties > Start Menu Programs > **Remove...**; in
Windows 2000: Advanced > **Remove...**): captures `vm/win95-truecolor-remove-tree-1..2.png`,
`vm/win98-truecolor-remove-tree-1..3.png`, `vm/win2000-truecolor-remove-tree-1..2.png`. In all
three the tree's inside begins at (55, 294) of the screen. `core::tree` draws it; Bevel95
Settings' list of programs is such a tree with a check box added to each row. Positions are
from the tree's left edge and a row's top.

| Value | WIN95 | WIN98 | WIN2000 |
|---|---|---|---|
| Row | 16 tall | same | same |
| Step for each folder a row is in | 19 | same | same |
| Outermost row | no lines, no box ("Start Menu"); its picture at 0. (First read as having lines and a box: that capture, `remove-tree-2`, was scrolled by one row) | same | same |
| Plus/minus box | 9×9, frame `#808080`, inside white, sign `#000000` (5 long; the plus 5 tall); its middle 8 right of the row's lines cell (under the picture of the folder above) and 8 down | same | same |
| Lines | dots `#808080` on every other pixel. Down: on even rows, from the row's top to its middle, on to its bottom if a row of the same level follows; through the rows of an open folder for each level that goes on below. Across: at the row's middle, from the line 10 to the right (6 dots; the box covers the first three) | same | same |
| Picture | 16×16, right after the lines (19 further in than the box's cell) | same | same |
| Label | begins 19 after the picture's left edge; text 2 further, 1 down | same | same |
| Selected, with the focus | label filled with the highlight color (text width + 4, the row's height), text in the highlight's text color; around it a frame of single pixels, alternately black and white XOR the highlight color (`#FFFF7F` on `#000080`, `#F5DB95` on `#0A246A`): black where screen x + y is even | same | same |
| Selected, without the focus (only in a tree that keeps its selection in sight then: that of Remove Shortcuts/Folders does) | the same rectangle filled with the face color `#C0C0C0`, text in the usual text color, no frame (`vm/*-truecolor-remove-focus-1.png`) | same | same, its face color `#D4D0C8` |
| Folder picture | its Programs menu's program group | a 16-color one (frame `#808000`, body `#C0C0C0`) that its menus do not use | 98's, pixel for pixel |
| "Start Menu" picture | a yellow closed folder (dithered `#FFFF00` and `#C0C0C0`, frame `#808080`) | a gray closed folder (frame `#808000`, body `#C0C0C0`) | 98's, pixel for pixel |
| Shortcut arrow (on every program's picture in the tree of the Start menu) | 7×7 in the picture's lower left corner (rows 9 to 15): a white square with a black arrow, its left and top edge `#808080`, its right and bottom edge black | same | same |
| Check box (ours in a tree of programs; each from a list the Windows had) | 11×11: frame `#808080` of 1, inside white, the check mark 2 in (Add/Remove Programs, Windows Setup: `vm/win95-truecolor-windows-setup-list.png`); 3 in and 3 down in its place of 16 (PROVISIONAL) | 13×13: black frame of 2, inside white, the mark 3 in; 2 in and 2 down in its place of 16 (the tree of Folder Options, View: `vm/win98-truecolor-folder-options-view.png`) | as 98 (its Start Menu Settings list) |
| Half-ticked box | inside `#C0C0C0`, mark black (Windows Setup's "shaded" box; 98's is the same) | the flat box with a gray inside (PROVISIONAL, by analogy) | as 98 |

compare-refs, 0 differing pixels each: the trees inside the 16 whole-window cases of the next
section, and the check boxes in `vm-win98-tree-check-boxes` and `vm-win95-list-check-boxes`.

### Remove Shortcuts/Folders

The window the trees were read from, the same in Windows 95, 98 and 2000 but for each look's
colors, font and pictures. `core::remove` draws it. On the Reference PCs it stands at (35, 218)
of the screen, Taskbar Properties at (2, 168). Positions are in the window.

| Value | All three |
|---|---|
| Window | 324×350; title "Remove Shortcuts/Folders" with the "?" and close buttons; 33 right of and 50 below Taskbar Properties' top-left corner |
| Text | "To remove an item from the Start menu, select the item and" / "click Remove." at (18, 38), the second line 13 lower; the T is the tree's access key |
| Tree | sunken frame (18, 74, 288×224), inside 284×220: 13 rows in full and 12px of a 14th. It begins with "Start Menu", under it "Programs" (open, and selected when the window opens), the folders closed |
| Scroll bar | 16 wide at the inside's right edge, once more than 13 rows show; a page is 13 rows |
| Remove | (150, 308) 75×23, "Remove" with R underlined; grayed out while "Start Menu" or "Programs" is selected |
| Close | (231, 308) 75×23 |
| Framed button | Close while the tree has the focus; else the button that has the focus (with its focus rectangle) |

Captures: `vm/<pc>-truecolor-remove-tree-*.png` (as opened; folders opened with Down and Right)
and `vm/<pc>-truecolor-remove-focus-0..2.png` (Accessories selected, then Tab, then Tab again).
16 compare-refs cases of the whole window, 0 differing pixels each: `vm-<pc>-remove-opened`,
`-accessories` (and 98's `-games`), `-focus-tree`, `-focus-remove`, `-focus-close`. Masked: each
PC's own program pictures, without the shortcut arrow on them.

## Find (programs)

Quick Search since 2026-11-16. Bevel95's own window (decisions.md, "Find (programs)"): no classic Windows had it, so every
value here is ours and PROVISIONAL, chosen in the manner of the dialogs above and built from
their parts. The code is [`core/src/find.rs`](../core/src/find.rs).

| Value | WIN95 / WIN98 | WIN2000 |
|---|---|---|
| Window | 330×264, the dialogs' frame and title bar with the look's small Find picture (as a program's window has its picture) and "X" only; title "Quick Search" (until 2026-11-16 "Find: Programs") | the same title (until 2026-11-16 "Search: Programs"), the small Search picture |
| Where it opens | as Run: 12 right of the screen's left edge, its bottom 12 above the bar | same |
| "&Named:" | at (9, 33) (4 below the field's top; Run's "Open:" sits 3 below) | same |
| Field | Run's: (58, 29, 263×21), sunken, white; text 1 in from its inside, caret as Run's | text 4 in, as Run's |
| List | sunken frame (9, 57, 312×181), 7 below the field, as wide as the field reaches; inside white | same |
| Header | 17 rows at the top of the list's inside; two raised cells (Highlight/Dark outside, Light/Shadow inside, face within): "Name" 150 wide, "In Folder" the rest; labels 6 in, centered in height. Since 2026-11-18 the line between them can be dragged (it takes the mouse 8 left to 7 right of In Folder's first pixel, with the header's double-bar pointer, and shows an inverted 1-pixel line from the header's top to the list's bottom while dragged, as a folder window's Details); Name never narrower than 38 (its picture's 22 and 16), In Folder never narrower than 40; a double-click fits Name to the longest name found plus 22 before and 6 after; the width is remembered (`quick-search.txt`) | same |
| Rows | 10 rows of 16 under the header; the picture 2 in, the name 22 in (cut with "..." 6 before the column's end), the folder 6 into its column; the selected name on the menus' highlight color with 2 to its left and right | same |
| Scroll bar | `core::scroll`'s, 16 wide at the right of the list's inside over its whole height (header included) once more than 10 programs are found; the header and rows then end at it | same |
| Status bar | the bottom 20 rows inside the frame (3, 241, 324×20): 2 rows of face, then one panel with a 1px sunken edge (Shadow, Highlight), its text 3 in, centered in height: "N item(s) found" (until 2026-11-16 "N program(s) found"; "Looking for programs..." until the first reading) | same |
## Pictures: banner, icons, flag

| Picture | Where it comes from |
|---|---|
| "Windows95", "Windows98", "Windows 2000 Professional" banners (21×122, 21×105, 21×225) | the Reference PCs' Start menus (`vm/win95-truecolor-documents-0`, `win98-truecolor-accessories`, `win2000-truecolor-documents-0`, and every capture with the Start menu open), pixel for pixel; the same as bitmap 157 of each PC's EXPLORER.EXE. First cut from Screenshots S4 and S1, S13 and S15, S12 and S10; found identical on 2026-11-18. [`core/src/banner.rs`](../core/src/banner.rs) |
| Start menu icons (32×32), 95, 98 and 2000 | the same Reference PC captures (first Screenshots S3, S4, S10, S12, S13; found identical on 2026-11-18), but 98's and 2000's Log Off, still from S13 and S10 (the PCs' Start menus have no Log Off). [`core/src/icons.rs`](../core/src/icons.rs) |
| Program group icons (16×16), 95, 98 and 2000 | Reference PC: 95's and 2000's from `vm/<pc>-truecolor-explorer-programs-menu`; 98's is the 16-color image of icon 20 in its SHELL32.DLL (first Screenshots S3, S12, S15) |
| Start menu icons with small icons (16×16), 95, 98 and 2000 | 95's and 2000's: the Reference PCs' Start menus with small icons (`vm/own-pictures/<pc>-truecolor-start-small`, 2026-11-18); 98's: the 16-color images of the same icons in its files. First the v86 emulator's Start menus with small icons ([Taskbar Properties](#taskbar-properties)) |
| Taskbar Properties: the preview's five pieces per design, the pictures of its second page | as raw RGB in [`art/assets/`](../art/assets/) (`ie4-*`: Windows 95 with Internet Explorer 4 and 98; `w2k-*`: Windows 2000). The Start menus large and small, the taskbar, Customize and Documents: the Reference PCs' EXPLORER.EXE (bitmaps 150, 149, 151; icons 107, 108), 2000's with its 3-D grays drawn as D4D0C8; the window, the taskbar without clock and 2000's Clear: found identical in the PCs' Taskbar Properties (`vm/own-pictures`, 2026-11-18); `w95-*`: plain Windows 95's own Start menus and taskbars, from its PC. The preview's Start button is Windows' own picture, not the real button: in Windows 2000 it has more space between the flag and "Start" (seen side by side on the 2000 PC, owner's question 2026-11-09) |
| Dialog icons (Run, the computer of Shut Down, the key of Log Off) | Reference PC: `vm/<pc>-truecolor-runlist-after-esc`, `vm/<pc>-truecolor-shutdown`; the key is the middle 32×32 of the 48×48 256-color image of icon 45 in 2000's SHELL32.DLL (first Screenshots D1–D5) |
| Windows 2000 Shut Down banner (411×77, 145 colors) | Reference PC, `vm/win2000-truecolor-shutdown` (first Screenshot D4), as raw RGB in [`art/assets/`](../art/assets/) |
| Which pixels are see-through | where a capture shows the icon both selected and not: the pixels that change with the selection color; a file's icon says it itself. Otherwise (the pictures still from screenshots): the background color reachable from the icon's edge |
| Start flag | the Reference PCs' Start buttons: identical in every True Color capture of all three (first taken from RetroBar's `start9x.png`) |
| Program and document icons | today's Windows icons, read from the shell (they are yours, like the task buttons'), without the shortcut arrow Windows 95's menu never showed |
| Settings and Find (Search) icons (16×16), 95, 98 and 2000 | the Reference PCs' menus, each entry captured selected and not ([`tools/dev/cutmenu.ps1`](../tools/dev/cutmenu.ps1)): 0 mismatches for all. Control Panel's is its window's picture there too; 98's Taskbar and Files or Folders are 95's, pixel for pixel |
| Task-button icons of Notepad and Explorer's windows (16×16), 95, 98 and 2000 | the reference PCs' taskbars: see [Classic icons for today's windows](#classic-icons-for-todays-windows) |

## Classic icons for today's windows

At the owner's request, Notepad's and Explorer's windows show the old pictures on their task
buttons instead of today's ([`core/src/window_icons.rs`](../core/src/window_icons.rs)). Which
window gets which:

| Today's window | Picture | 95 | 98, 2000 |
|---|---|---|---|
| Notepad (any notepad.exe) | Notepad | the spiral notepad | the blue notepad (2000's is 98's, pixel for pixel) |
| Explorer: Documents (by its display name or full path) | My Documents | the folder window's (95 had no My Documents) | My Documents |
| Explorer: This PC, Home | My Computer | its own | its own |
| Explorer: Recycle Bin | Recycle Bin, empty | its own | its own |
| Explorer: Control Panel and its pages (titles without " - File Explorer") | Control Panel | its own | its own |
| Explorer: any other folder (Downloads included) | the folder window's: an **open** folder | 16 colors | 98's 0x9F/0xCF palette, 2000's 0x9C/0xCE one |
| A button with several Explorer windows | the folder window's | | |

How they were taken: on each reference PC, My Computer, the Recycle Bin, Notepad, Control Panel,
a folder (`C:\WINDOWS`, `C:\WINDOWS\TEMP`, `C:\WINNT\Temp`) and, on 98 and 2000, My Documents
were opened one after another, the taskbar captured after each (the newest window's button
pressed) and once more with none pressed. Each icon was cut from its raised button (face gray
reachable from the edge = see-through) and checked against its pressed button, where every
see-through pixel shows the checkerboard and every other pixel is unchanged: 0 mismatches for
all 17. The 20 captures are compare-refs cases (`vm-<era>-tasks-<n>`), whole buttons, labels
included.

What the captures also showed: the pressed buttons' checkerboard follows the first button (see
[Task buttons](#task-buttons)); pressed bold labels are cut one letter sooner (same table); 98's
and 2000's Quick Launch band ends a few pixels before the notification area, so their task areas
are 517 and 566 pixels wide in these captures (the only widths that give the buttons the
measured widths with 4, 5 and 6 windows), and the cases draw our bar with those task areas
(drawn without Quick Launch; the same captures with the band are compared whole in [Quick Launch](#quick-launch)); Windows 2000 SP4's Tahoma fits one letter more before "..." in the
91-pixel buttons ("My Docume...", "Untitled - N..." against today's "My Docum...", "Untitled -
..."), so those two label ends are masked: see [D16](#discrepancies) and [flagged
item](#flagged-for-your-eyes) 25.

## Date/Time Properties

Read from the reference PCs (True Color captures `private/reference/refs/vm/<era>-truecolor-datetime-*.png`,
`-cal<day>`, `-hands-*`, `-sweep-m*`; three compare-refs cases, 0 differing pixels outside the
hands). The same window in all three looks, 404×341; 95 and 98 identical, 2000 with its colors,
Tahoma and four differences (below). Windows placed it at its default spot for a new window
(98: 0,0; 95: 44,44 then 66,66; 2000: 110,110 then 132,132); Bevel95 puts it at 44,44.

| What | Value |
| --- | --- |
| Page frame, buttons, tabs | page (9,49) 386×253; OK/Cancel/Apply 75×23 at x 158/239/320, y 308; tabs as the Properties window's, but the chosen tab's text stays 1px lower (95/98; 2000 as the Properties window) and the chosen tab also covers the page frame's inner line |
| Groups | Date (19,60) 183×197, Time (211,60) 170×197, labels "&Date", "&Time" |
| Month | drop-down list (30,79) 75×21, button (87,81) 16×17; focused: highlight (34,83) 51×13 with the dotted rectangle around it (dots light where (x+y) on the screen is odd, the rest black) |
| Year | field (115,79) 63×21, text 1px in (2000: 4px); arrows (178,79) and (178,89), 16×10 each, 3×2 arrows |
| Calendar | (30,111) 162×130; inside split in 7 columns (left = 32 + col×158/7); rows 18 high; weekday band 18 rows of ButtonShadow with ButtonFace letters 5px into each column; days centered in their column; the chosen day's box is as wide as two digits (starting where two digits would start) and ends with the day's digits; focused, its edge becomes the dotted rectangle |
| Clock face | 12 hour marks 4×4 (lit edge 00FFFF, face 008080, shadow 000000) and 48 minute dimples 3×3 (shadow top left, highlight bottom right), positions in `core::datetime` |
| Hands | measured on 181 captures of Windows 98 (`private/reference/refs/vm/hands98`: every minute position at 6 o'clock, every hour position with the minute hand far from it, every second at 11:48). Each of the hour and minute hands is three copies of one teal shape: a ButtonShadow copy 2px down and right, a white copy 2px up and left, then the teal shape; the minute hand is drawn first, the hour hand over it. Minute hand about 48 long and 3 wide on each side at the middle, hour hand about 40.75 and 4, both with a tail of about 10. The hour hand moves every 12 minutes. Second hand: a 1px line that inverts what is under it, from the pixel right and below the middle (always drawn) to an end point left out, taken from a table of 60 measured end points (Windows' rounding is irregular, no formula fits); where the ideal line runs exactly between two pixels, the step is taken early when the line goes up and late when it goes down. That second hand is exact on all 60 captures. **PROVISIONAL**: the hour and minute shapes are a fitted model (`core::datetime`, `MINUTE_HAND`, `HOUR_HAND`), about 135 pixels per capture still differ along their edges (95: 170, 98: 84, 2000: 121 on the three compare-refs captures, so the three Windows draw the same hands). Windows' own fill rule was not found: a horizontal hand is not symmetric about its axis and its middle row reaches the tip, which no triangle filled at pixel centers does |
| Time | field (256,222) 77×20 (2000: (252,222) 86×20); arrows 16×9 one pixel right of it; each part in a slot two digits wide 6px apart, the colon's dot 2px after a slot, AM/PM 2px after the seconds; a number ends 1px short of its slot's end but never starts before it (two digits fill the slot) |
| Time, keyboard (98) | read on the 98 PC, eight compare-refs cases (`win98-time-*`, 0 differing pixels). Alt+T or Tab into the field chooses the hour; Tab and Shift+Tab go through hour, minutes, seconds, AM/PM before leaving it; Left and Right do nothing. A chosen number is highlighted only as wide as its digits; a chosen AM/PM from the seconds' slot to 1px before the field's inner edge, with a dotted rectangle (every other pixel inverted, by screen position) and no caret. A typed digit replaces the number, a second one is added only if the result is still a time (hour 1–12, else 0–59), a third is ignored; a typed hour keeps AM or PM, except 12 which is always noon (12 PM). After a digit or an arrow the number shows unpadded, not highlighted. A 2px caret inverts the last two columns of the chosen number's slot, over the full text height, blinking. The year field ignores typed digits (only its arrows change it). The face follows the typed time within a second (Bevel95: at once). 95 and 2000 not checked yet: they get 98's behavior (flagged) |
| Bottom line | "Current time zone:" at (19,272); the zone's name at x 110 (2000: 115) |
| Title bar (98) | the shading's span ends 5px past the title buttons' edge (this window's capture is the first at a known position) |

### Time Zone page

Read on all three reference PCs (`private/reference/refs/vm/<era>-truecolor-timezone-*.png`: the page, the
list dropped down page by page, a zone without daylight saving time, the box with the focus,
2000's map at nine zones); 26 compare-refs cases `<era>-timezone-*`, 0 differing pixels. The page
is laid out alike in all three looks, with each look's colors and font.

| What | Value |
| --- | --- |
| Zone field | drop-down list (21,58) 362×21, button (365,60) 16×17; with the focus the entry is highlighted at (25,62) 338×13 with the dotted rectangle around it, as the month's |
| Dropped list | (21,79) 362×197: a 1px black frame, 15 rows of 13px from (22,80), each entry 2px in, the chosen one highlighted with the dotted rectangle over it (every other pixel inverted); a 16px scroll bar (thumb as Windows' MulDiv rounds it). Opens with the chosen entry as its first row. Keys move the choice at once, also while it is open; Esc keeps the moved choice (95, 98 seen) |
| Lists | Windows 95 OSR 2.5: 52 entries; 98 first edition: 62; 2000 SP4: 74, each with its own wording. Each entry stands for a zone of today's Windows (`core::timezone`); where the places' offset changed since, the entry follows today's time of the place it names (flagged: Midway/Samoa keeps -11 as UTC-11; 98's Colombo is today's Sri Lanka +5:30; 2000's Almaty/Novosibirsk is Novosibirsk +7) |
| Map | (22,85) 360×188 sunken frame, inside 356×184 in two colors: sea 00007F, land 007F00 (`art/assets/w98-world.bits`, 1 bit per pixel). 95's differs from 98's in 40 pixels (`w95-world.bits`). 95 and 98 never move it; 2000 shows 98's map turned: column x shows column (x + 338 + trunc(offset_minutes × 356 / 1440)) mod 356 of it, the offset from the entry's own label (measured at nine zones) |
| Daylight saving box | check box (21,278), label "Automatically adjust clock for &daylight saving changes" 18px right of it. For a zone without daylight saving time: grayed out with its check kept (95, 98), left out (2000) |
| Tabs (95, 98) | with Time Zone chosen, both tab labels sit 1px below the tabs' usual top (with Date & Time chosen: 2 and 3); the chosen tab covers the page frame's inner line from its own left edge (2000: 2px further right, as for the first tab) |

### Run's list

Read on all three reference PCs on 2026-11-13 (`private/reference/refs/vm/<era>-truecolor-runlist-*.png`: Run
opened from Start, commands typed so the list held 6 to 26 entries, the list dropped by F4,
Alt+Down and the button); 11 compare-refs cases `<era>-runlist-*`, 0 differing pixels (the
field's inside is masked: Windows shows the command selected there). The list is the Time Zone
page's (`core::droplist`), placed at the field.

| What | Value |
| --- | --- |
| Place | as wide as the field (275), its frame directly under the field (window y 103), or directly above it (its bottom on the field's top, y 82) when it would reach past the **screen's** bottom edge (not the taskbar's top: 98 moved up showed it over its taskbar). The Run box itself sat at (5,405) on 95 and 98 and (5,389) on 2000, 4px above the taskbar, so there its list opened above once it had 7 or more (95, 98) or 8 or more (2000) rows |
| Rows | one per command up to **10** (95, 98) or **23** (2000), 13px each, 2px frame in all (`dialog::run_list_rows`) |
| Scroll bar | 16px, always shown: with more commands than rows as the Time Zone list's (thumb by the same MulDiv rule: 98, 15 in 10 rows: 65px; 2000, 26 in 23: 236px); with all of them shown grayed out, no thumb, both arrows grayed (the arrow in ButtonShadow over a ButtonHighlight copy 1px down and right) |
| Highlight | Highlight color, white text, no dotted rectangle (the field has the focus) |
| Opening | the first command that **starts with** the field's text, letters of either case alike, is highlighted and becomes the first row as far as the list allows (field "c:\\" opened on "c:\\windows" above "c:\\"); with an empty field or no match nothing is highlighted and the list starts at its first command. The field keeps its text |
| Button | opens and closes the list as it goes down, drawn pushed in only while held (raised while the list is open). F4, Alt+Down and Alt+Up open and close it |
| Keys while open | Up/Down move the highlight (from none: the first command), the field shows that command, the list scrolls only to keep it in view; Enter and Esc close only the list (Run stays open, nothing runs) |
| Keys while closed | Up/Down step through the commands from the one shown, without opening the list |
| Mouse while open | the row under the pointer is highlighted, the field unchanged; a click on a row puts it in the field and closes the list; a click anywhere else in the Run box (even on Cancel) only closes the list |
| Typing while open | goes into the field; nothing is highlighted, and the list scrolls to the first command starting with the text (95; 2000 with no match went back to its first command) |
| Wheel | open list: scrolls, highlight unchanged (98, 2000). Closed: 2000 steps to the next command; 95 had no wheel at all |
| Not built | 98's and 2000's list slides open (an animation); 98 completes typed text inline and 2000 shows a separate suggestion list under the field (Internet Explorer's AutoComplete); Windows put the last command, selected, in the field when Run opened |

## Windows 98

Windows 98 kept Windows 95's bar, colors, font, bevels, task buttons, clock and its own
menus; its Start menu and dialogs are new (they came with Internet Explorer 4, which 2000
also built on). Measured on S13–S16 and D6, D9, D10; all seven comparisons match with 0
differing pixels. "First edition": Windows 98 Second Edition changed the Start menu (see
[D8](#discrepancies)). The code is `WIN98` in [`core/src/theme.rs`](../core/src/theme.rs).

| Value | WIN98 | Source |
|---|---|---|
| Start menu items | **36** tall; icon **5px** into the item, centered (2 rows down); label **2px** after the icon (39 into the item), cell centered with an odd spare row **above** | S13, S15, S16 |
| Start menu width | 3 + 21 + 39 + longest label + **20** + 3 (168 for "Windows Update") | S13, S15, S16 |
| Cascading menu items | **20** tall, the whole item selected; icon 5px in; label 2px after it (23 into the item) | S13, S15 |
| Cascading menu width | 3 + 23 + longest label + **16** + 3, but only + **3** when no entry has a submenu (no room kept for arrows) | S13 (Favorites 124, Channels 142), S15 (Programs, Accessories, Communications 153) |
| Below the last entry | **1 extra row** of face color inside the frame, in the Start menu (the banner runs through it) and every cascade | S13, S15, S16 |
| Submenu arrow | right edge **4px** from the item's; top (item height − 7) / 2 | S15 |
| Separator | 8 tall; lines on rows 3 and 4; 2px short at both ends; both separators alike | S13, S15 |
| Banner | "Windows98", white letters on a dark-to-light blue dithered 16-color picture (21×105), `#000080` above it | S13 and S15, identical |
| Submenu placement | **5px** over the parent's right edge; first item level with the entry | S13, S15 |
| No room below a submenu | moves up just enough (PROVISIONAL: as 2000, whose Start menu is the same code) | |
| Entries | Windows Update, ―, Programs, Favorites, Documents, Settings, Find, Help, Run..., ―, Log Off *name*..., Shut Down... (access keys: F**a**vorites, **L**og Off; always underlined, as in 95) | S13, S15 |
| Favorites | your Favorites folder (folders first, then links), folders with 98's plain yellow folder icon | S13 |
| Settings, Find submenus | Control Panel, Printers, Taskbar & Start Menu..., Folder Options..., ―, Windows Update... (no access key) / Files or Folders..., On the Internet... (98's Active Desktop above the separator and its People are left out: no counterpart today) | Reference PC (no network, so no Find > Computer...) |
| Icon squares | every Start menu icon (32×32 and 16×16) is drawn on a square of **`#BFBFBF`**, **`#00007F`** when selected: one step off the menu's colors (`icon_cell` in theme.rs) | Reference PC, and S13 (earlier comparisons left icons out, so it went unseen) |
| Task button titles | cut short with "..." only where they would pass the bevel's inner edge (3px later than 95's rule) | S14 ("Drive Converter (FAT32)" fills its 121px button) |
| Title bars | gradient `#000080` → `#1084D0`, up to the "?" button or 2px before a lone "X" or the Minimize button; see below | D6, D9, D10, Reference PC |
| Run | as 95 (347×163, same positions and icon), text "Type the name of a program, folder, document, or Internet / resource, and Windows will open it for you." | D9 |
| Shut Down | 323×192; computer icon (14, 40); "What do you want the computer to do?" (60, 40); options S**t**and by, **S**hut down, **R**estart, Restart in **M**S-DOS mode at (61, y) with y = 65, 84, 104, 123 (12 dialog units, rounded); OK, Cancel, Help at y = 158, x = 62, 146, 230 (78 wide); starts on Shut down | D10 |
| Log Off | as 2000's (288×123, same key picture), in 98's colors and font | D6 |

**Windows 98's title bar gradient.** Unlike 2000's GDI gradient, it steps every 2 pixels in
8.8 fixed point, over a width rounded up to a multiple of 16 and centered on the part that
shows. With *visible* = the columns from the gradient's left end (the caption's, or 18 further
right when the window has an icon) to its right end: `width = visible rounded up to 16`,
`origin = left − (width − visible) / 2`, and column *x* gets
`from + ((at · (((to − from) << 8) / width)) >> 8)` with `at = (x − origin)` rounded down to an
even number. It depends only on the window, not on where the window is on the screen. Found by
fitting six windows of the Reference PC at known places (dialogs with "?" and "X", with "X"
alone, a program's window in front and behind) and it reproduces D6, D9 and D10 exactly as
well. The rule before this one (2026-11-07) rounded to multiples of 8 *screen* pixels and
needed the window's position; it fit D6 and D10 but was a shade off on D9 and on every
Reference PC window. One exception: a message box's gradient runs right up to its grayed "X"
(Reference PC: Run's "cannot find" box); drawn for Solitaire's "Deal Again?" and the desktop's
extension warning (`window::paint_frame`, a disabled lone "X").

**Pictures.** The 32×32 Start menu icons were first cut from S13, the only full-color 98
screenshot; since 2026-11-18 they are the Reference PC's (`vm/win98-truecolor-accessories` and
20 more captures), found identical, but Log Off (still S13's: the PC's Start menu has none).
Windows Update is the same picture as 2000's, so 2000's is used. The Programs folder icon was
cut from a 16-color screenshot (S15), so 98's is the 16-color one: the 16-color image of icon
20 in the Reference PC's SHELL32.DLL; in True Color the PC draws the 256-color image, which
differs (not changed: [decisions.md](decisions.md), "Every picture from the owner's own
Windows"). The Run icon is 95's; the Log Off key is 2000's (its four holes now see-through).

## Desktop icons
Read off the Reference PCs' desktops at 800x600 True Color (`private/reference/refs/vm/<era>-truecolor-desk-*`),
the same in all three looks unless said:
- Grid: one icon every 75 pixels down and across, in columns from the top left of the work
  area (800x572: 7 per column).
- Picture: 32x32, 21 pixels from the cell's left, 2 from its top.
- Label box: 38 pixels below the cell's top; as wide as the widest line + 4 and 13 pixels per
  line + 2; centered under the picture ((75 - width) / 2, rounded down); each line centered in
  it; the text's line box starts at the box's top (MS Sans Serif's letters 2 pixels lower,
  Tahoma's 3).
- Wrapping: at spaces, lines at most 71 pixels wide (98: "My Documents" 71 fits, "brown fox
  jum..." 74 does not; 2000: Tahoma's "My Documents" 70 fits, "Connect to the" 72 does not).
  A longer word: "..." at the end (95, 98, and 2000 when not in full); 2000 in full breaks it
  between letters.
- Lines: 95 always all; 98 and 2000 two, the second being the rest of the name shortened with
  "..." to 71, except on the focus icon while the desktop has the keyboard.
- Text: white; unselected on the desktop color. Selected (desktop in front): the selection
  color behind it (000080; 2000 0A246A) and white text; the picture selected (95: the average
  with 000080 per channel, rounded down; 98 and 2000: their own captured pictures).
- Focus frame: on the label box, every pixel of its edge whose screen x + y is odd, inverted
  (FF7F7F on teal, FFFF7F on navy, F5DB95 on 2000's blue). 2000 only after a key.
- Another window in front: no selection and no focus frame shown.
- The desktop's menu: Windows' popup menu style of each look (rows as the other menus);
  98's and 2000's New submenu has 19-pixel rows with a 16x16 picture 6 pixels from the row's
  left and the text at 28 (not done yet).

Moving them (`private/reference/refs/vm/<era>-truecolor-move-*`, 2026-11-12; the same in all three unless
said):
- A drop: the icon moves by exactly the pointer's way, to the pixel (98: pressed at 30,15 and
  let go 153,97 further, its cell lies at 153,97; 95 and 2000 the same). No grid. A drop onto
  another icon is a file operation in Windows (98 asked "create a shortcut instead?" for the
  Recycle Bin dropped on My Documents); Bevel95 puts the icon back.
- Drag starts once the pointer is more than 2 pixels away (SM_CXDRAG 4, centered).
- While dragged the icon stays where it was, as it showed (selected, focus frame). The dragged
  picture follows the pointer at the same offset:
  - 95, one icon: the plain picture on every pixel whose screen x + y is even, the desktop
    showing through on the others; the label's words solid black, no box.
  - 98, one icon: the plain picture half over what is under it, and the words in black half over
    it (on teal the text is 004040, 606060 becomes 307070). Not exact: 98 halves its image
    list's colors (EFEFEF gave 70B0B0, 003030 gave 005F5F); Bevel95 halves the picture's own
    colors (rounded down). Not compared.
  - 2000, each icon: as it shows selected (picture, label on 0A246A, white words) at about
    47% over what is under it (0A246A on 3A6EA5 gave 244B89, white 97B2CF); Bevel95 uses 120/255,
    rounded. Not exact either, not compared.
  - 95 and 98, several icons: no pictures, only for each icon a dotted box (DrawFocusRect)
    48x36 at the cell's top, 13 from its left, and a two-pixel dotted line under it across the
    label's text (from the box's left + 2, the label's width - 4) at the label box's top +
    its height / 2 (98: rows 220-221 for a one-line label at 213).
- Selection rectangle: DrawFocusRect between the press and the pointer (the pointer's own row
  and column outside it: pressed at 150,60, pointer at 30,260, dots from 30,60 to 149,259).
  Every icon whose picture or label it touches is selected while it is drawn; the others lose
  the selection (with Ctrl they keep it). The focus stays.
- Ctrl+click adds or takes away one icon and moves the focus there. Shift+click selects every
  icon touching the rectangle around the anchor icon and the clicked one (98: from My
  Briefcase at 0,375 to My Computer at 350,210 selected Set Up and Connect between, not My
  Documents or the Recycle Bin above, nor Online Services below).
- Enter opens every selected icon (95: four windows, 98: four, 2000: two). Ctrl+A selects all.
- Arrows (98, icons lying anywhere): the nearest icon further that way; a step to the side
  counts about twice (Up from 0,225 went to 0,0, not to 200,175).
- Line up Icons: each icon to the grid cell nearest its cell's corner (98: 153,97 to 150,75;
  95 and 2000: 219,139 to 225,150); if taken, the next free cell to its right (98: 30,108 to
  75,75 although 0,150 below was free). Its access key: 95 "Lin&e up Icons", 98 and 2000
  "Line &Up Icons".
- Auto Arrange: off at the start in all three. Switched on, the icons are packed into columns
  from the top left in the order they lie: by the column their cell's left edge is in, then from
  the top (95: icons at x 0, 0, 0, 200, 200, 250 came in that order). A drop with it on goes in
  where it lands and the rest close up (98). The submenu ticks it with the menu's check mark.
- A new item from the desktop menu's New lies with its picture's top-left at the point the menu
  was opened (all three; 98: right-click at 400,250, folder picture at 400,250). Another new
  item (a file made elsewhere) takes the first free cell, column by column (98: the first
  column was full, it went to 75,0).

## The menu on an icon
Read off the Reference PCs at 800x600 True Color (`private/reference/refs/vm/<era>-truecolor-item-*`,
2026-11-12; the same in all three unless said):
- The menus: Windows' popup menu style of each look, rows as its other menus. Opened by Shift+F10
  (or the menu key) at the focus icon's picture middle (98: My Computer's picture at 21,2, menu
  at 37,18); with nothing selected, the desktop's menu at the work area's top left (2000: 0,0).
  Open bold. 2000 underlined access keys only when opened from the keyboard.
- Entries (ours in bold here are the ones kept): file and shortcut **Open**, Print, Open With
  (98 "Open &with...", Quick View), - Send To (95 "Se&nd To", 98 "Send &To") - **Cu&t, &Copy** -
  **Create &Shortcut, &Delete, Rena&me** - **P&roperties**; folder **Open, &Explore** (2000
  "E&xplore"), Find (2000 Search, Sharing), then as a file; My Computer **Open, Explore**, Find
  (2000 Search, Manage, Map/Disconnect Network Drive) - **Create Shortcut**, Rename -
  **Properties**; My Documents (98, 2000) **Open, Explore** (98 Remove from Desktop, 2000 Search,
  Delete) - **Create Shortcut**, Rename - **Properties**; Recycle Bin **Open, Explore, Empty
  Recycle &Bin** (grayed while empty, etched; 2000's key underline dotted on the menu's even
  columns) - (Paste while files are on the clipboard) - **Create Shortcut** - **Properties**.
  Several selected (the Recycle Bin and a folder): 95/98 Open, Explore, Find - Paste - Create
  Shortcut - Properties; 2000 Open, Explore, Empty Recycle Bin - Paste - Create Shortcut -
  Properties.
- 2000's bold Open: "O" 8 pixels wide, round (rows `..####..`, `.##..##.`, 4x `##....##`,
  `.##..##.`, `..####..`), advance one more than today's Tahoma Bold; "e" with its last two rows
  `##..##`, `.####.`.
- The desktop's menu: Paste and Paste Shortcut (95/98 "&Paste", "Paste &Shortcut") between
  separators right above New, grayed (etched) while the clipboard holds no files; Undo
  (98/2000 "Undo Rename\tCtrl+Z") below them after a rename, left out.
- Question windows (window pixels, frame included; centered on the screen; Yes default and
  focused, a black frame and the focus dots; No; the title bar's X enabled):
  - one item (Confirm File Delete, also emptying one item): 419x133; picture 32x32 at 18,38;
    text from 65,38 (its 13-pixel row), wrapping at x 400; Yes 239,96 80x23, No 326,96 80x23.
  - a folder (Confirm Folder Delete): 419x152; text from 60,38; buttons at y 115.
  - several (Confirm Multiple File Delete, also emptying several): 419x135; text from 65,45;
    Yes 224,96 87x23, No 318,96 87x23.
  - Pictures: a file, a folder, several sheets going into the Recycle Bin, and for emptying a
    folder with a red "!" (`art/assets/<era>-delete-icons.rgb`, cut from the captures over the
    face color).
- Rename box (98 "Test text": label box 90..134 at 116; box 88..138 at 112..128): a 1-pixel black
  frame, white inside; left x = the label box's left - margin + 2, width = the text + 2 x margin
  + 2, margin 4 (MS Sans Serif) or 5 (Tahoma: "Test text" 85..140 around a 44-pixel text); top =
  the label box's top - 4, the text's row 2 pixels lower, height 13 + 4 for one line; 95 and 98
  with several lines: top - 6, the text 3 lower, 13 per line + 8 (2000 keeps + 4). The text
  centered, all of it selected at first (000080 / 0A246A, white). Caret black, 13 tall, 2 pixels
  wide in 95 and 98, 1 in 2000. The picture shows plain (not selected) meanwhile.
- Cut picture (a text file's, selected or not, the label unchanged): 95/98 each channel half
  the picture's and half the desktop's, rounded down (FFFFFF on 008080 7FBFBF, C0C0C0 60A0A0);
  2000 each in 5 bits: (p >> 4) + (d >> 4), widened by v << 3 | v >> 2 (FFFFFF on 3A6EA5
  94ADCE, 000000 183152, C6C6C6 7B94B5).
- Create Shortcut: "Shortcut to Test text" (the shown name), selected, in the next free cell.

## The extension warning
Read off the Reference PCs at 800x600 True Color (`private/reference/refs/vm/<era>-truecolor-renext-*`,
2026-11-12, `private/probes/rename-ext/r.ps1`; extensions shown: 95 My Computer > View > Options >
View, "Hide MS-DOS file extensions for file types that are registered"; 98 Start > Settings >
Folder Options > View and 2000 `control folders` > View, "Hide file extensions for known file
types"; the same in all three unless said):
- A USER message box (MB_YESNO | MB_ICONEXCLAMATION), title "Rename", the words "If you change
  a filename extension, the file may become unusable." / empty line / "Are you sure you want to
  change it?". 95 and 98 389x126, 2000 398x126 (Tahoma is wider). Window pixels, frame included:
  the 32x32 warning picture at 14,33 (the one Solitaire's "Deal Again?" box shows); the text's
  13-pixel rows from 62,33 (2000 65,33), 13 per line; Yes 116,89 (2000 120,89) and No 81 to its
  right, both 75x23; Yes focused and default; the title bar's X grayed (98's and 2000's gradient
  runs up to it). 2000 underlined Y and N, the name having been typed.
- Place: x = (screen width - window width + 7) / 2, y = (screen height - window height + 17) / 2,
  rounded down (209,245 and 204,245 on 800x600; Solitaire's 185x119 box at 311,249 fits the same
  rule).
- When: a file whose old extension is a registered type, renamed to another extension or to
  none (".txt" to ".dat", ".txt" to nothing). Not: an old extension that is no registered type
  (".dat" to nothing or to ".txt"; on 98 also after giving ".dat" an empty registry key, and
  after pointing it at a type that does not exist or exists with nothing in it), an extension
  added to a name without one, only the case changed (".txt" to ".TXT"), a folder ("x.txt" to
  "x.dat"), a shortcut ("Shortcut to a.txt" to "s.dat": the hidden ".lnk" stays).
- Meanwhile the rename box stays, the typed name in it without selection or caret.
- After No the box is open again, all selected: 95 and 98 with the name typed, 2000 with the old
  name. 95/98's box for "a.dat" typed over the two-line "New Text Document.txt": while the
  warning is open top = label box top - 3 (333 for a label row at 336), the text 2 lower,
  13 + 8 tall; after No 13 + 4 tall, top = label box top - 1, the text 2 lower. 2000's box after
  No (two lines again) stood 4 pixels lower than a freshly opened one (not copied). Esc then
  closes it with the old name; Yes renames.
- Shift+Del, captured for later (`*-renext-shiftdel-*`, answered No): file "Confirm File Delete"
  ("Are you sure you want to delete 'a.TXT'?"), folder "Confirm Folder Delete" ("Are you sure you
  want to remove the folder 'x.dat' and all its contents?", 2000 wrapped after "its"), two items
  "Confirm Multiple File Delete" ("Are you sure you want to delete these 2 items?"); all with the
  deleted-for-good picture (sheets with a red "!") and the delete windows' layouts, Yes focused.

## Folder windows (Windows 95)

Measured on the Windows 95 Reference PC at 800×600 True Color (`private/reference/refs/vm/win95-truecolor-folder-*.png`,
40 captures taken with `private/probes/folders/f.ps1`; pixels read with `runs.ps1`, `winrect.ps1`,
`menucorner.ps1` there). Compared by `cargo compare-refs` (`vm-win95-folder-*`, 22 cases, 0 pixels).

- **Frame**: a sizable window's, 4 pixels (DFDFDF/FFFFFF top-left, 808080/000000 bottom-right, then
  2 rows of face); caption 4..21, menu bar titles as every program's (moved in by the wider frame),
  the list's sunken edge from y 42 (808080 and 000000 outside, FFFFFF and DFDFDF inside), its white
  inside from (6, 44). Maximized: no frame (as Solitaire).
- **Icons**: the desktop's 75-pixel cells, the first cell's corner at the list's inside corner, the
  picture 21 and 2 into the cell, labels as on the desktop but black on white; rows from the left,
  as many columns as fit the list's inside (beside a vertical scroll bar when the icons need one:
  `t26` has 6 in 478 − 16). Selected: the desktop's look (picture averaged with 000080, label white
  on 000080); another window in front: no selection and no focus frame shown (`mc-inactive-sel`). A
  new window has the focus frame on its first icon, nothing selected.
- **Resizing**: the icons stay where they are (`pf-narrow`: 265 → 175 wide kept three columns);
  scroll bars come. The scroll bars' whole is the icons' extent plus 2 pixels right and down (the
  thumbs of `pf-narrow` and `t26` match only so). Left arrow: its triangle one pixel further left
  than the right arrow's mirror.
- **Status bar**: 19 rows above the frame's bottom 4 (2 face, then 17-row parts); the first part
  144 wide from the frame's inside, a 2-pixel gap, the second to the inside right edge; parts sunken
  1 pixel (808080 top-left, FFFFFF bottom-right); text 4 in from the part's outer left edge, 2 below
  its top. The size grip: three diagonals of one white and two 808080 pixels in a 12×12 square at
  (w − 17, h − 17), the corner cleared to face first; text stops 2 (a part's) or 3 (menu help)
  pixels before it. While a menu entry is highlighted: its help text alone, no parts, 3 in from the
  frame's inside.
- **Words** (shell32's string table, read from the PC's SHELL32.DLL): "%s object(s)", "%s object(s)
  (plus %s hidden)", "%s object(s) selected", "Free Space: %s,  Capacity: %s" (two spaces), sizes
  "%s bytes" under 1000, else three digits and KB/MB/GB right after ("939KB", "2.63MB"); whether
  the last digit is cut off or rounded is not settled (Bevel95 cuts it off). Nothing selected: the
  count and the size of every file, hidden ones too (C: "5 object(s) (plus 13 hidden)", "939KB");
  a file selected: its size; a folder: nothing; a drive in My Computer: its free space.
- **First size**: icons in a near square (columns = √n rounded up, rows as needed): width 40 + 75 ×
  columns, height 69 + 75 × rows, at least 210 × 201, at most 520 × 412 (65% and 412/600 of the
  screen). Measured: 0, 1, 2 items 210×201; 3, 4 210×219; 5 265×219; 7 265×294; 10 340×294; 17
  415×369; 26 490×412 (scroll bar). A folder of 96 items opened in List view at 520×412 (List view
  is not built; Bevel95 shows large icons).
- **Place**: new windows cascade 22 pixels a step down and right, 0 to 132 then 0 again (the first
  after boot at 44); a folder's window opens again where it was closed, at its size (Windows 95
  remembered it; `cursors-moved-reopened`).
- **Behaviour**: a double-click on a folder opens a new window; on a folder whose window is open,
  that window comes forward as it was (`mc-dblclick-c-open`); Backspace opens the parent (or brings
  it forward: `c-backspace`); Backspace in My Computer showed a message box "You are at the top level
  and cannot go up another level. To see the desktop, minimize any open windows." (`mc-backspace`;
  not built).
- **Menus** (rows compared with Windows' fuller menus): File with nothing selected (My Computer):
  Create Shortcut, Delete, Rename, Properties (grayed), Close; a drive: Open (bold), Explore, Find...,
  Format..., Create Shortcut, Delete and Rename grayed, Properties, Close; a folder: Open, Explore,
  Find..., Send To, New, ...; a file: Open, Print, Send To, New, ...; Edit: Undo, Cut, Copy, Paste,
  Paste Shortcut, Select All, Invert Selection (shortcuts Ctrl+Z/X/C/V/A); View: Toolbar, Status Bar,
  Large/Small Icons, List, Details, Arrange Icons, Line up Icons, Refresh, Options...; Help: Help
  Topics, About Windows 95; the empty area: View, Arrange Icons, Line up Icons, Paste, Paste
  Shortcut, (New,) Properties. File menu at (frame + title.x, title.bottom), as every menu bar's.
  The New menu's kinds of file (Text Document and so on) were drawn 2 pixels further left and 1
  higher than its other rows (not copied; that row is left out of its comparison).
- **Pictures**: shell32.dll's 16-color icons, exact: 3½" floppy (group 7), 5¼" (6), removable (8),
  hard disk (9), network (10), CD-ROM (12), RAM disk (13), Control Panel (36 = 137), Printers
  (38 = 138); title bars: My Computer's and the open folder's 16×16 (as on task buttons), a drive
  window its drive's 16×16. Gray folders in some captures are the virtual PC's drawing.

## Folder windows (Windows 98 and 2000)

Measured on the Windows 98 and 2000 Reference PCs at 800×600 True Color with the web view off
(`private/reference/refs/vm/win98-truecolor-folder-*`, 51 captures, `win2000-truecolor-folder-*`, 31; taken with
`private/probes/folders/f.ps1`, pixels read with `private/probes/folders2/runs.ps1`, `bbox.ps1`,
`split.ps1`). Compared by `cargo compare-refs` (`vm-win98-folder-x-*`, `vm-win2000-folder-x-*`, 26
cases, 0 pixels). Window pixels from its top-left corner, frame f = 4 (0 maximized).

- **Rebar**: an etched rectangle from (f, f + 19) to the frame's inside right, 92 rows (98) or 74
  (2000): outer 808080 top-left / FFFFFF bottom-right, inner the other way round. Bands from x f +
  2: menu band y f + 21, 22 rows; toolbar band 2 rows lower, 40 rows (98) or 22 (2000); address band
  2 rows lower, 22 rows. Between bands an etched line (808080 from x f + 2, FFFFFF from f + 1, both
  to the right edge less 3). Each band's gripper: a raised 3-pixel bar at x f + 4 from 2 rows below
  the band's top to 2 above its bottom. The list's sunken edge 3 rows below the rebar (98: y 118,
  2000: 100), x from f as in 95.
- **Throbber**: 38 × 22 at the menu band's right end (the rebar's inside right − 38), black with the
  logo at rest (cut), after an etched vertical line (808080, FFFFFF) whose highlight goes one row
  further down into the band's line.
- **Menu titles** (flat buttons): from x f + 11, each its name's width + 16 wide, 19 high one row
  below the band's top; name 8 in, 3 down. Under the pointer raised (FFFFFF top-left, 808080
  bottom-right), while its menu is open sunken, the name 1 down and right. 98 always underlines
  access keys here; 2000 once a key has been pressed. Menus open at (title x, title bottom).
- **Toolbar** from x f + 11. 98: buttons 54 × 40, Back and Forward 53 + 14 for the arrow;
  separators 8 wide, their etched line at +3/+4, 2 rows in from the band's top and bottom. Picture
  20×20 at +17 (Back, Forward +18), 3 down; name centered in 54, its cell 24 down; arrow a 5×3
  triangle at the arrow part's +4, 19 down, black even when grayed. 2000: 22 high, Back 50 wide
  (picture at +4, 3 down; "Back" 23 in, 4 down) + 13 for the arrow, Forward 24 + 13, Up and Delete
  24; the arrow at +4, 10 down, etched when grayed. Under the pointer: the picture in color and a
  1-pixel raised edge (on both parts of a split button); pressed: sunken, picture and name 1 down and
  right; a split button pressed has both parts sunken (98: the button's edge two columns wider and
  the arrow's left edge drawn over it; 2000: the arrow moves 1 down and right too). Grayed: the
  etched picture, the name etched.
  Undo (2026-11-16): 98 between two separators after Paste (button x 381 of a window at 0, 0,
  `win98-truecolor-folder-mycomputer`, `-hot-undo`), 2000 the 24-wide button right after Delete
  (x 528 of a window at 110, 110). Never seen grayed: plain in every capture, also in fresh windows.
  98 cuts the toolbar 4 pixels before the band's end (its Properties showed only the "P"'s stem
  in a window 512 wide).
- **Address band**: "Address" at x f + 11, 5 down (2000: 4; "A&ddress" underlined in 2000 once a
  key was pressed); the combo box from the name's end + 4 to the rebar's inside right − 4 (2000:
  − 49, then Go: a flat button 43 × 22 from the combo's end + 2, picture at +3/+3, "Go" at +23/+4),
  2-pixel sunken edge, white; its picture 16×16 at the inside's +1/+1 (98's had its bottom row
  clipped: drawn at +1/+2 it lost the last row), the text at +21/+3; the arrow button 16 wide at
  the inside's right, pushed in while the list is open. Clicked: the whole text selected (a
  selection-colored band 13 rows high under the text).
- **Address list**: a 1-pixel black frame from the combo's left edge below it; rows 16 high from the
  frame's top + 1; picture at the frame's + 4 + 10 per level, name 20 to its right, 2 down. 98's
  list was as wide as 532 pixels and reached past the window (Bevel95: the combo's width, inside the
  window, scrolled to the highlighted row); 2000's highlights the place shown: its name on the
  selection color (w + 2 wide, 15 rows, from one row down) with a dotted focus frame, its picture
  tinted (not copied: drawn plain).
- **Status bar**: 20 rows: 2 face rows, then parts 18 high; 98: 240 | the rest | 218, 2000: the
  rest | 76 | 153, 2 pixels apart. Words 2 in and 1 down (2000: 1 and 1); the zone part: My
  Computer's picture at +2 (2000 +1), "My Computer" at +22 (2000 +21), 1 down. Sizes 98 "1.02MB",
  2000 "59.2 KB"; 2000 with one file selected: "Type: Application Size: 59.2 KB".
- **Wording**: titles: My Computer, Desktop, a drive's name (98 " (C:)" with a leading space when it
  has no label, 2000 "Local Disk (C:)", "Compact Disc (D:)"), a folder's name. Address: 98 a folder's
  path ("C:\WINDOWS", "C:\"), 2000 its name ("WINNT", "Local Disk (C:)"). My Computer: 98 drives,
  Printers, Control Panel (Dial-Up Networking, Scheduled Tasks: no counterpart); 2000 drives,
  Control Panel.
- **Behaviour**: a double-click on a folder shows it in the same window; Back grayed at the start,
  Forward at the end; Backspace = Up and forgets Forward; Up from a drive: My Computer; from My
  Computer: grayed in 98, the Desktop in 2000. After going somewhere, 2000 shows the focus frame on
  the first icon (a new window shows none). The little arrows list up to 9 places, the nearest
  first. Typing into 98's address bar completed the text inline (not copied).
- **First size**: 98 by its icons: 31 + 75 per column × 117 + 75 per row (5 items 256 × 267, 8
  items 256 × 342); 2000 680 × 556 at 800×600, cut to the work area where it opened; new windows
  cascade 22 a step (2000: 110, 132). Flagged: 98's smallest and largest new window.
- **Menus' help texts** (read off the status bar): 98 File's "Open"/"Explore" (the verbs' own),
  "Creates shortcuts to the selected items." ... "Closes the window."; titles "Contains Edit
  commands", "Contains commands for manipulating the view", "Contains commands for browsing to
  various pages."; Go "Goes back one step.", "Goes forward one step.", "Goes up one level."; 2000
  "Contains edit commands.", "Goes to the previous page."; View's Refresh 98 "Refresh the contents
  of the current page.", 2000 "Refreshes ...". Access keys: 98 "&Status Bar", 2000 "Status &Bar",
  "G&o To".

## Explorer's tree

Measured on the three Reference PCs at 800×600 True Color (`private/reference/refs/vm/<era>-truecolor-tree-*.png`,
43 captures taken with `private/probes/tree/t.ps1`; 98 with "as Web Page" off, 2000 with classic
folders, 95 and 98 in Large Icons). Compared by `cargo compare-refs` (`vm-*-tree-*`, 15 cases, 0
pixels). Window pixels from its top-left corner, frame f = 4.

- **The window**: 95 and 98 600 × 411 (95 at 44, 44: the cascade's first place; 98 at 0, 0), 2000
  680 × 458 at 110, 110. Title "Exploring - <place>" (95, 98; 98's " (C:)" gives two spaces),
  2000 the place's name; the Explorer's picture (explorer.exe 101: 95 its 16-color image, 98 and
  2000 their 256-color one) in the title bar and on the task button (2000 only while the tree
  shows). 98's Explorer started without a status bar (Bevel95: the folder windows' one setting).
- **The panes** start where the list would (95: f + 38; 98 and 2000: the list's top). 95: "All
  Folders" a 1-pixel sunken box (808080 top-left, FFFFFF bottom-right) 192 × 19, its text 4 in and
  2 down; one face row; the tree in a 2-pixel sunken edge to the status bar; the bar between the
  panes 3 wide; "Contents of '<place>'" over the icons the same way. 98 and 2000: the bar's frame
  (sunken outside, raised inside, 1 pixel each) 196 (2000: 200) wide; its caption 20 rows from the
  frame's +2, an etched line under it (808080 at +22, FFFFFF at +23), the tree from +24 without an
  edge; caption text 98 "All Folders" at +13/+6 (from the frame), 2000 "Folders" at +8/+5; the
  close button 20 × 17 at (w − 25, 4) (2000: (w − 23, 3)), flat, raised under the pointer, sunken
  pressed, its "X" 8 × 7 at +6/+5 (pressed +1/+1); the bar between the panes 4 wide. The panes'
  widths stay at every window size (192/196/200 at 600 wide and maximized at 800).
- **Rows**: Windows' tree view (see "Trees"): rows of 16 from the tree's inside, the Desktop at 0
  without lines or box; labels cut at the edge, the tree scrolls across (its whole: the widest
  label's box); scroll bars as the list's. Selected with the keyboard: label on the selection color
  with the focus rectangle (2000: no rectangle until a key was pressed); without the keyboard
  nothing shows; a selected folder shows the open folder. 95's 16 × 16 folders are shell32's groups
  4 and 5 (yellow on a gray checkerboard; some folders came out plain gray: the virtual PC's
  drawing), Desktop explorer.exe 103, Printers shell32 38; 98's and 2000's are the address bar's.
- **What opens first**: 95 and 98 the Windows drive, opened, the tree down to it; 2000 My Documents.
  Opening a row scrolls as little as needed for its last row to show, the row in sight
  (`win95-…-tree-windows`). The splitter drag: 95 a 4-pixel band of every other pixel inverted
  following the pointer (98 drew a 1-pixel dotted line; Bevel95 the band in every look); let go,
  the pane takes the width; the icons keep their places (a scroll bar comes).
- **Status bar** with the tree: 95 "939KB (Disk free space: 284MB)" in its right part, 2000 "1
  object(s) (Disk free space: 2.05 GB)" in its first.
- **2000's Folders button**: 66 × 22 after Up's separator (Bevel95 leaves Search out, so it stands
  where Search stood), its picture 17 × 17 at +4/+3 (gray, in color under the pointer or while on),
  "Folders" at +23/+4; while the tree shows: sunken with a checkerboard of white and face color
  inside a face border of 1 (none under the pointer), the picture and name 1 down and right.
- **Menus**: on a row Explore (bold), Open, Find... | Send To | Cut, Copy | Create Shortcut, Delete,
  Rename | Properties (a drive: Explore, Open, Find..., Format..., Create Shortcut, Properties);
  98's View > Explorer Bar: Search, Favorites, History, Channels, All Folders | None (the one in
  effect with a dot); 2000's: Search, Favorites, History, Folders | Tip of the Day.
- **A drive without a disk**: 95 a message box titled as the window, 189 × 126 at the screen's
  middle: the error picture at 14, 33, "A:\ is not accessible." / "" / "The device is not ready."
  from 62, 33, Retry (default) at 17, 89 and Cancel at 98, 89, each 75 × 23. 2000 "Insert disk", 347 ×
  137: the drive's picture at about 14, 26, "Please insert a disk into drive A:." at 108, 33,
  Cancel (default) at 257, 98. After Cancel the tree shows the place shown before.
## Folder views (Small Icons, List, Details)

Measured 2026-11-14 on the three Reference PCs (320 captures `private/reference/refs/vm/<era>-truecolor-view-*`,
`private/probes/views/v.ps1`; every capture's test folder and window in `private/probes/views/<era>-notes.txt`).
Checked by 18 compare-refs cases `vm-<era>-view-*` (0 differing pixels). Coordinates are from the
list's inside corner unless said.

| | 95 | 98 | 2000 |
|---|---|---|---|
| Row height (every small view) | 17 | 17 | 17 |
| 16x16 picture | row top, at the left (Details: 2 in) | same | same; Details 1 lower |
| Label box | after the picture, text + 4 wide (Details: from 18 in, text + 8), the whole row | same | same |
| Text | 2 in; 2 down (Small Icons: 0 down) | same | same |
| Small Icons grid | 112 across | widest text + 28 | widest text + 28 |
| List column | widest text + 28 (112 when the window opened in List) | widest text + 28 | widest text + 28 |
| Details header | 17 high: FFFFFF, DFDFDF, face, 808080, 000000 | same | 17 high: FFFFFF, face, 808080, 404040; rows from 19 |
| Details default widths | Name 120, Size 60, Type 120, Modified 120 | same | Name 120, Size 96, Type 120, Modified 120 |
| My Computer's columns | Name 120, Type 150, Total Size 90, Free Space 90 | same | same |
| Header text | 6 in (numbers: 6 from the right), pressed: 1 down and right | same | same; the sort arrow 9 after the text |
| Sort arrow | none | none | etched triangle 8 x 7, 5 below the header's top; up (white base) ascending, down (gray base) descending |
| Cut names | "..." to fit the Name column less 22 | same | same |
| Divider drag | an inverted 1-pixel line from the header's top to the list's bottom at the new edge; columns move at the release | same | same |
| Double-click on a divider | Name: widest text + 24 (209 for the test name) | same | 218 (Tahoma) |
| Sizes | KB rounded up, commas: "0KB", "1,600KB" | same | "2,328 KB" |
| Dates | M/D/YY h:mm AM | same | M/D/YYYY h:mm AM |
| Selected | label 000080 / white; picture half 000080 | same | 0A246A; picture in 5-bit steps (7B8CAD for white) |
| Window without the keyboard | no selection | no selection | selection in the face color D4D0C8, black text, plain picture |
| Header click | sorts ascending (Modified newest first), again: exactly reversed, folders last; the selection goes, the focus stays | same | Modified oldest first |
| Line up Icons, Auto Arrange | grayed in List and Details | same | same |
| Opening view | Large Icons up to 77 items (78 and 79 not tried), List from 80; remembered per folder; Explorer: its own last view (List at first) | the same window: always Large Icons; a new window on 120 items: List; Explorer as 95 | always Large Icons; per folder, Explorer too |

- Small Icons kept each item's grid place (column and row) when 95 and 98 switched from Large Icons
  (95's three columns stayed three); 2000 put them into rows as wide as the window. 95 painted a long
  label over the next cell's item (later items are painted first). 98 once showed Small Icons 112
  apart after Back (`rule-v1-small-before-beta`): not settled, Bevel95 uses the widest label.
- The Views button: 98's is a split button 53 + 14 wide like Back (picture 18 in, "Views" under it,
  the arrow 4 in and 19 down), the button cycles Large, Small, List, Details; 2000's one drop-down 31
  wide (picture 4 in, the arrow 22 in and 10 down) whose press opens the menu at once.
- The empty area's menu starts with View and a line under it (95's My Computer too).
- Not measured: a 16x16 shortcut arrow (Bevel95 shrinks the look's 32x32 one); 2000's Thumbnails
  (left out); the menu titles 2000 grays in a window without the keyboard (not built); one pixel at
  the lower left of 98's hot Views arrow (masked in its case).
## The web view panel (Windows 98 and 2000 folder windows)

Measured on the Reference PCs, 2026-11-15 (`private/reference/refs/vm/<era>-truecolor-web-*`, notes in
`private/probes/web/<era>-notes.txt`). "Inside" is the area within the list's sunken edge; positions are
from its top-left pixel. No font smoothing anywhere (every text pixel 000000, links 0000FF, 98's
"Warning" FF0000).

| | Windows 98 | Windows 2000 |
|---|---|---|
| Panel width | round(30% of the inside); none under 400 (450 on the warning page); never more than 480, Bevel95's own limit, not measured (30% of 1600, the widest common screen of 98's time; GitHub issue 2: 768 on a 2560-wide window) | 200; none under 400 |
| Without the panel | a 32-row white band above the icons, the title at (18, 3) | nothing |
| Kept for the scroll bar | 16 columns at the panel's right (line and wrapping use width - 16) | the same |
| Corner picture | wvleft.bmp 182 × 237 at (0, 0) | wvleft.bmp 127 × 56 at (0, 0) |
| Place picture (32 × 32) | at (15, 15) | at (12, 11) |
| Title | Verdana 16pt bold (21 px), cell top 52, lines 25 apart, left 15 | Tahoma 13pt bold (17 px), cell top 49, left 12 |
| Line | wvline.gif (168 × 1: FF3300 40, FFCC00 47, 66CC33 42, 0099FF 39) stretched to width - 16, rounded to the nearest column; 30 rows below the title's last cell top | 6699CC, 2 rows, width - 16; 23 rows below the title's cell top |
| Text | Verdana 8pt (11 px) and bold, left 15, 23 rows below the line, lines 13 apart, wraps 15 from the kept column | Tahoma 8pt, left 12, 17 rows below the line, lines 13 apart, paragraphs 12 more, wraps at width - 16 |
| One file | **name.ext** / type / blank / Modified: / date / blank / Size: N bytes (under 1000) or the list's KB | **name** / type / Modified: date / Size: 9.29 KB (none for 0 bytes) / Attributes: (normal) |
| Several | N items selected. / blank / Total File Size: N bytes / blank / names (16 at most) | N items selected. / Total File Size: 2.28 MB / names (not wrapped) |
| Nothing selected | Select an item to view its description. | the same, then "See also:" and links 17 rows apart (My Documents, My Network Places, My Computer) |
| My Computer | the prompt; a drive: bold name, type, Capacity / Used / Free with 12 × 12 legend boxes (C0C0C0, FFFFFF) at 15, text at 34, a 100 × 50 pie (depth 8) 25 rows below the last line | the prompt, a note (FFFFE1, border D3D3D3, text at 13) "Displays the files and folders on your computer", See also with Network and Dial-up Connections; a drive: legend boxes (D4D0C8, FFFFFF) at 28, text at 46, a 120 × 59 pie (depth 9) centered in a 120-row box 26 rows below the last line |
| A drive's own folder | no pie | the pie with nothing selected |
| Pie colors | used part and side DFDFDF, free FFFFFF, outline 000000 | used part and side D4D0C8 |
| Warning page | Warning (bold red) / blank / Modifying the contents of this folder may cause your programs to stop working correctly. / blank / To view the contents of this folder, click **Show Files** (bold link). wvlogo.gif 314 × 331 at the inside's bottom right | This folder contains files that keep your system working properly. There is no need to modify its contents. / blank / To view the contents of this folder, click: Show Files (plain link). wvlogo.gif 299 × 303, its last row one below the inside's |
| Links | underline 12 rows below the cell top, under the spaces between words too; status bar ":Show Files" | the same; status bar the link's description |

Capacities: GB with two decimals, MB whole, a space before the unit ("0.99 GB", "249 MB"), cut
off rather than rounded. Not settled: where 2000 put the first column of icons beside the panel
(8, 16 or 21 pixels in three captures), and 98's shorter label cut beside the panel ("rather lon...").

## Explorer pointers, clicks and messages (2026-11-16)

Measured on the three Reference PCs at 800 × 600 (`private/probes/measure17`, captures
`private/reference/refs/vm/<era>-<depth>-m17-*`; the PCs' clocks run slow, times converted to Windows' own).

| | Windows 95 | Windows 98 | Windows 2000 |
|---|---|---|---|
| Details column line: pointer | comctl32's divider (cursor 106: 2-pixel bar, 2-row shaft, 18 × 18, hot spot the bar's right column and the shaft's lower row) | the same | the same (with 2000's pointer shadow) |
| Column line: where | 8 left to 7 right of the next column's first pixel (16) | the same (press tests) | the same within ±1 |
| Column line: drag | 1-pixel inverted line, columns move on release | the same | live resizing |
| Bar between the panes: pointer | its own split pointer (1-pixel bar 15 high, 3-row shafts; `kit::SPLIT95`) | IDC_SIZEWE | IDC_SIZEWE |
| Bar: where | the 3-pixel bar only | the bar (4) and the tree pane's 2 edge columns | the same as 98 |
| Bar: drag | 4-pixel checkered inverted band | 1-pixel dotted inverted line | live |
| Second click on another item | (not tried) | only selects it | only selects it |
| Slow second click on the selected name | renames (any gap past the double-click time), box about 0.65 s after | the same, about 0.5 s after | the same, about 0.5 s after |
| First click on Name, window sorted by name | order kept, no arrow | order kept, no arrow | order kept, the up arrow already shown |
| Address that cannot be opened | (no address bar) | message titled with the typed text, 494 × 119 for `C:\nothere\nofile` | "Address Bar", 485 × 119 |
| Submenu, pointer onto another parent entry | (not measured) | lit at once, submenu closed about 0.4 s later | (not measured) |

The address message is USER's message box: the error picture at 14, 33, text from 62 in lines
of 13 (moved down to center on the picture when shorter), the box at most 5/8 of the screen wide
(text then wraps), 16 to the right of the widest line, OK (75 × 23) centered 17 under the taller of
picture and text, 14 under it to the bottom. Windows 98 toolbar pictures: SHDOCVW.DLL 4.72.3110
(Internet Explorer 4.01 SP1) has a 16-color set (resources 261/262, 272/273) drawn only in 16-color
mode and a 256-color set (265/266, 276/277) drawn at 256 colors, High Color and True Color; Bevel95's
pictures are the 256-color set.

## Dragging files (2026-11-16)

Measured on the three Reference PCs by a measuring helper (`private/probes/drag18`: `summary.txt`,
`<era>-notes.txt`, the pointers cut into `pointers\`; captures `private/reference/refs/vm/<era>-truecolor-drag-*`;
a second drive from a vvfat disk; times converted to Windows' own: 95 0.892, 98 0.331, 2000 0.840 of
real time). Built in `core::drag`, `winkit::dragdrop`, `winkit::undo`.

| | Windows 95 | Windows 98 | Windows 2000 |
|---|---|---|---|
| Left drag, one drive / another | move / copy | move / copy | move / copy |
| A program (.exe) | shortcut | shortcut | moved like any file |
| Ctrl / Shift / Ctrl+Shift | copy / move / shortcut, the mark changes at once | the same | the same |
| Pointer: move | the arrow, no mark | the same pixels | its own arrow (a row longer) |
| Pointer: copy, shortcut | arrow + 11 × 11 box (808080 left and top, black right and bottom, white inside; a 5 × 5 plus, or the shortcut arrow) at (+11, +18) | the same | the box at (+11, +11) |
| "No" sign | (not cut) | black ring and slash, white outline, 18 × 18 | the same with outline, 20 × 20; hot spot not measured (Bevel95: the middle) |
| Dragged picture, one item | picture as a checkerboard, name in solid black | picture and name 50 % over what is under (black → 7F7F7F on white) | picture selected, about 50 % (black → 878787), the name's box as at the start |
| Several items | dotted outlines (48 × 36 round each picture, a line per name; small views a 16 × 16 square) | the same | the real pictures |
| Over another program | (not tried) | no picture, OLE's pointer only | (not tried) |
| Drop place | a folder, a program, the Recycle Bin, My Computer (refusing: "no" sign) drawn selected only while the pointer is on it; the empty area not marked | the same; a tree row's label selected, the old selection without fill | the same |
| Tree row opening under a drag | (not measured) | at a move after a short rest (not while still, 4.8 s) | after about 1 s still (0.7-1.7 s) |
| Right-drag menu | Move Here, Copy Here, Create Shortcut(s) Here, line, Cancel (M, C, S); bold = what a left drag does | the same | Copy Here first |
| Drop on the Recycle Bin | no question | Delete's own question ("send 'a1' to the Recycle Bin?") | no question |
| Esc; the other button | cancel at once | the same | the same |
| Drop in the same window | (not tried) | Large Icons: the icons move there, remembered | (not tried) |
| Edit's Undo | "Undo Move/Copy/Rename/Delete" (U), no Ctrl+Z; status "Undo Move of 'a3'" | the same; nothing to undo: "Undo  Ctrl+Z" grayed, "Undoes the previous file operation." | "Undo Move  Ctrl+Z" |
| Undo depth; Undo Copy | several steps; asks "Are you sure you want to delete 'a2'?" (delete picture) | the same | the same |
| Undo on the desktop's menu | "Undo Delete" after Paste Shortcut | the same | "Undo Rename  Ctrl+Z" |
| Undo that cannot be done | (not measured) | "Error Renaming File", "Cannot rename file:  File system error (1026)." | "Error Renaming File or Folder", "Cannot rename file: Cannot read from the source file or disk." |
| Toolbar Undo | (no toolbar) | between Paste and Delete | after Delete, before Views |
| A shortcut made by a drag | (not measured) | not undoable | (not measured) |

Not measured: the "no" sign's hot spot, tree scrolling at the edges (Bevel95: one row every
150 ms within 16 pixels of the edge, PROVISIONAL), drives in My Computer as drop places, Undo of a
move or copy that cannot be done (Bevel95 uses the rename message's pattern), the undo depth's limit
(Bevel95: 10).
## 16 Colors (2026-11-16)

Measured on the three Reference PCs in 16 colors (95 and 98 at 800×600 through their own display
settings; 2000 at 640×480 with its built-in VGA driver, its own driver having no 16-color mode):
`private/probes/colors16/survey.txt` and the notes beside it, captures `private/reference/refs/vm/<pc>-16color-*`.

- **The emulator's values.** QEMU shows the 16 colors with its own values: 000000, A80057 (maroon),
  00A857 (green), A8A857 (olive), 0000A8 (navy), A857A8 (purple), 57A8A8 (teal), C0C7C8 (silver),
  87888F (gray), the other seven as Windows has them. Bevel95 draws Windows' own values (what a
  real monitor showed); `cargo compare-refs` translates the captures (`sixteen_cases::read_capture`).
- **The 16 colors**, in Windows' (VGA) order, index 0 to 15: 000000 800000 008000 808000 000080
  800080 008080 C0C0C0 808080 FF0000 00FF00 FFFF00 0000FF FF00FF 00FFFF FFFFFF.
- **The looks' colors** (`theme::WIN95_16`, `WIN98_16`, `WIN2000_16`):

  | | True Color | 16 colors | source |
  |---|---|---|---|
  | 95, 98: 3D light | DFDFDF | C0C0C0 | all their captures |
  | 98: active / inactive caption | 000080 → 1084D0 / 808080 → B5B5B5 | 000080 / 808080, plain | Run, Calculator, Minesweeper |
  | 98: Start menu icon squares | BFBFBF / 00007F | the menu's C0C0C0 / 000080 | Start menu |
  | 2000: face and light | D4D0C8 | C0C0C0 | every window |
  | 2000: dark shadow | 404040 | 000000 | every window |
  | 2000: selection | 0A246A | 000080 | menus, Shut Down's list |
  | 2000: desktop | 3A6EA5 | 008080 | desktop |
  | 2000: captions | 0A246A → A6CAF0 / 808080 → C0C0C0 | 000080 / 808080, plain | Run, Shut Down |
  | tooltip | FFFFE1 | 95, 98: FFFFFF with FFFF00 dots; 2000: FFFFFF | 98's and 2000's clock tooltip |
  | grayed label | gray etched with white | 95, 98: plain gray; 2000: etched | Taskbar Properties' Apply (95, 98), Run's OK (2000) |

- **Dotted areas** (95 and 98, `colors16::fill`): a 4×4 ordered (Bayer) pattern of two of the 16. The
  tooltip FFFFE1 is FFFFFF with 2 of 16 pixels FFFF00, at (x mod 4, y mod 4) = (0, 2) and (2, 0)
  counted from the window's corner (98's clock tooltip). Only this one area was measured; any other
  color outside the 16 is filled plain with its table color.
- **Pictures without a 16-color drawing** (`colors16::picture`): never dotted, each pixel one of the 16.
  2000: the nearest color (RGB distance), right for all 32,768 colors tested at 5 bits per channel
  (`private/probes/colors16/win2000/map-16color-5bit.txt`). 95 and 98 (identical): the measured table of
  every color at 6 bits per channel (`core/assets/vga-9x.txt`, from `win98/map-16color-6bit.txt`; each
  channel rounded to the nearest of its 64 steps). Its rough shape: black when the brightest channel
  is below 65; grays 65..160 gray, 161..223 silver, 224.. white; a strongly colored pixel takes the
  channels above half of the brightest, dark (80) or bright (FF). Within one step of a border the
  table and single patches can disagree (TESTTC's 3A6EA5, 4080C0, C08040, 40C080). A pixel below half
  alpha is see-through (a 16-color icon has a 1-bit mask).
- **Selected icon** (all three): every other pixel of the picture navy 000080, the others its own,
  counted from the picture's own corner, which is navy (My Computer on the three desktops).
- **Own drawings**: 98's and 2000's 16-color drawings come from the PCs' own files (4-bit icons of
  SHELL32, EXPLORER, MYDOCS; 98's 16-color toolbar from its screens; 2000's BROWSEUI.DLL strips
  274/275): art/src/colors16.rs (the drawings) and core/src/colors16_art.rs (which picture each belongs to), the match report in 	mp/probes/colors16/match/report.txt.
- **98's web view**: no watercolor picture; the stretched wvline.gif each pixel by 98's rule:
  FF3300 red, FFCC00 yellow, 66CC33 lime, 0099FF aqua (m/win98-16color-folder-mycomputer, row 227).
- **Focus rectangle**: inverting works on the palette number, i → 15 − i (navy → yellow, teal → red,
  silver → gray). The desktop's selected label: yellow on every other pixel, navy between (95, 2000).
  2000's Shut Down list: yellow and black alternating, from its corner.
- **Windows 2000's Shut Down banner** in 16 colors is a drawing of its own (no shadowed swoosh, a
  plain white ground): cut from `vm/win2000-16color-shutdown` (411×77 at 114, 92).
- Pictures that are 16-color drawings in every mode stay as they are: all of 95's icons (its shell
  has only 4-bit icons), the three Start banners, Minesweeper's and Solitaire's pictures and card
  backs, the Calculator.

## 256 Colors (2026-11-20)

Measured on the three Reference PCs at 256 colors, 800×600, each restarted in 256 colors
(`private/probes/colors256/survey.md`, captures `private/reference/refs/vm/<pc>-256color-*`, the
pictures cut by `private/probes/colors256/cut256.ps1`, its report `cut256-report.txt`).

- **The emulator's values.** 95 and 98: Windows' own. 2000: each channel at 6 bits, an odd step 3
  higher (v → (v div 4) × 4, + 3 when v div 4 is odd): D4D0C8 shows D7D0C8, 3A6EA5 386FA7, C0DCC0
  C0DFC0, the halftone 66 / 99 / CC as 67 / 98 / CF; 80 and FF as they are.
- **The looks' colors** (`theme::WIN95_256`, `WIN98_256`, `WIN2000_256`): each look's 16-color theme
  and

  | | 256 colors | 16 colors | source |
  |---|---|---|---|
  | 95, 98: grayed label | gray etched with white (as True Color) | plain gray | Taskbar Properties' Apply, Run's OK |
  | 2000: face and light | D4D0C8 (kept) | C0C0C0 | every window |
  | 2000: desktop | 3A6EA5 (kept) | 008080 | desktop |
  | 2000: dark shadow, selection | 000000, 000080 | the same | Run, the message boxes |
  | 2000: captions | 000080 / 808080, plain | the same | Run, My Computer |
  | 2000: tooltip, infotip FFFFE1 | white, yellow at (0, 0) and (2, 2) of every 4×4 from the area's corner | white | the web view's infotip (`-folder-mycomputer-2`); the clock tooltip not caught: PROVISIONAL |
  | 98: tooltip FFFFE1 | white, yellow at (1, 1) and (3, 3) from the tooltip's corner | white, yellow at (0, 2) and (2, 0) | `win98-256color-tooltip-clock`; 95's: PROVISIONAL, the same |
  | 95: Help topic paper FFFFE2 | white, yellow at (0, 1) and (2, 3) from the topic area's corner | (16 Colors keeps FFFFE2: not built, though its PC dotted it the same) | `win95-256color-calc-help-topic` |
  | 2000: focus rectangle | on 3A6EA5 C0DCC0, on D4D0C8 000000, else as 16 colors | by i → 15 − i | `-desktop-2`, `-msgbox-not-found` |
  | 98: web view line | True Color's (FF3300, FFCC00, 66CC33, 0099FF) | each pixel by 98's rule | `-folder-mc-sel-drive` row 227 |
  | 98: pie and legend | DFDFDF → C0C0C0 (as 16 colors) | | the same capture |
  | 2000: web view line | 6699CC (True Color's) | nearest | `-folder-mycomputer-2` |
  | 2000: infotip edge | D7D7D7 (the browser's gray) | silver | the same |

- **Pictures cut from the captures**: 98's watercolor (182×237 at the panel's corner, 6,120 of
  `win98-256color-folder-mc-sel-drive`: 28,988 pixels shown, 14,146 filled, mostly the 48 columns
  right of the 134 the panel showed, nearly all white: PROVISIONAL); 2000's corner picture (127×56
  at 6,102 of `win2000-256color-folder-mycomputer-2`: 6,196 shown, 916 filled under the folder's
  picture and the title: PROVISIONAL); the throbbers (38×22: 98's at 468,25, 2000's at 636,25);
  2000's Minesweeper title-bar picture (16×16 at 85,84 of `-mine-new`).
- **As 16 colors**: every icon and toolbar picture's 16-color drawing (hot ones too: 98's hot Cut
  compared, 0 pixels), 2000's Go button (line art, its face D4D0C8), the Start menus and banners,
  Run, Shut Down (2000's own 16-color banner), Calculator, Minesweeper (98), Date/Time, Taskbar
  Properties (its preview's face and desktop kept on 2000), Quick Launch.
- **PROVISIONAL**: the HTML Help pages (each pixel by the 16-color rule; the PCs dithered them in the
  browser's halftone colors by a rule not found), the filled pixels of the two web view pictures,
  grayed menu entries etched on 95 and 98, the tooltips named above, 2000's throbber's dark gray
  171717 (as shown; Windows' own value lies between 141414 and 171717).
- **Comparison cases** (`core/tests/twofiftysix_cases`), masks and why: 98's web panel: the pie,
  the folder's picture in the panel (the PC's), and the panel's bottom (98 gave it a horizontal
  scroll bar there and a shorter thumb; Bevel95's panel has none, a layout matter of True Color as
  well); 2000's web panel: the folder's picture; the folder toolbars: what Bevel95 leaves out, as in
  the True Color cases; 98's hot Cut: the pointer; Run: inside the command line; the bars: the
  clock's time; 2000's Taskbar Properties: "Use Personalized Menus" (alive on its PC, always grayed
  in Bevel95); 95's Taskbar Properties: "Show clock" (as in 16 colors); 2000's Shut Down: the
  question's underline; 95's Help topic: compared down to the five steps (the shared book's lines
  reflow below), its glossary link "operators" and step 5's "=." (CALC.HLP's own text).
## Reference screenshots

None of these files are in the repository (they are Microsoft's pictures).
[`tools/fetch-refs.ps1`](../tools/fetch-refs.ps1) downloads them all into `private\reference\refs` (kept
out of git), which `cargo compare-refs` reads (`BEVEL95_REFS` can name another folder). The Toasty Tech files have every system color shifted by a fixed amount
(for example `#BFB8BF` for `#C0C0C0`); the script also writes copies with the colors swapped
back (`-shiftedcolors.png` → `-fixedcolors.png`), which is what the comparison reads.

| # | File (as the comparison expects it) | Source | Shows |
|---|---|---|---|
| S1 | `win95-rtm-desktop-startmenu-programs-open-taskbar-640x480.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/desktop/full/win95.png) | Windows 95 desktop: Start menu with Programs open, pressed Start button, clock |
| S2 | `win95-rtm-taskbar-3tasks-1active-clock-640x28.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/managers/running/win95.png) | Windows 95 taskbar: Control Panel active, Dial-Up Networking, Calculator, "1:49 PM" |
| S3 | `win95-rtm-startmenu-programs-accessories-multimedia-cutout-624x292.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win95-1-2.png) | Start > Programs > Accessories > Multimedia (screen offset 2, 188) |
| S4 | `win95-rtm-startmenu-nohighlight-cutout-164x261.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win95-1-1.png) | The Start menu alone and the pressed Start button (offset 2, 217) |
| S5 | `win95-startmenu-4level-cascade-640x480-shiftedcolors.png` | [Toasty Tech](http://toastytech.com/guis/win95startmenu.png) | Four-level cascade |
| S6 | `win95-desktop-contextmenu-new-submenu-640x480-shiftedcolors.png` | [Toasty Tech](http://toastytech.com/guis/win95desktop2.png) | Desktop right-click menu with "New" open |
| S7 | `win95-taskbar-contextmenu-640x480-shiftedcolors.png` | [Toasty Tech](http://toastytech.com/guis/win95tile.png) | Taskbar right-click menu |
| S8 | `win95-startbutton-contextmenu-640x480-shiftedcolors.png` | [Toasty Tech](http://toastytech.com/guis/win95startexplore.png) | Start button right-click menu (bold default item) |
| S9 | `win95-taskbar-4tasks-1active-640x480-shiftedcolors.png` | [Toasty Tech](http://toastytech.com/guis/win95desktop3.png) | Four shrunk task buttons |
| S10 | `win2000pro-desktop-startmenu-open-taskbar-800x600.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/desktop/full/win2000pro.png) | Windows 2000 desktop, Start menu with Windows Update selected (a tooltip covers part of it) |
| S11 | `win2000pro-taskbar-quicklaunch-3tasks-1active-tray-800x28.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/managers/running/win2000pro.png) | Windows 2000 taskbar: Control Panel active, Calculator, NetMeeting, "1:11 PM" |
| S12 | `win2000pro-startmenu-programs-accessories-cutout-441x302.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win2000pro-1-2.png) | Start > Programs > Accessories (offset 2, 297 on 800×600) |
| S13 | `win98-desktop-startmenu-favorites-channels-taskbar-640x480.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/desktop/full/win98.png) | Windows 98 desktop: Start > Favorites > Channels open (a tooltip covers part of it), pressed Start button with its focus rectangle |
| S14 | `win98-taskbar-quicklaunch-3tasks-1active-tray-640x28.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/managers/running/win98.png) | Windows 98 taskbar: Quick Launch, Scheduled Tasks, Drive Converter (no icon), Calculator active, "3:15 PM" |
| S15 | `win98-startmenu-programs-accessories-communications-cutout-564x405.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win98-1-2.png) | Start > Programs > Accessories > Communications (16 colors; offset 2, 73) |
| S16 | `win98-startmenu-nohighlight-cutout-168x405.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win98-1-1.png) | The Windows 98 Start menu alone (16 colors; offset 2, 73) |
| S17 | `win98se-startmenu-nohighlight-cutout-194x364.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/managers/applicationmanager/win98se-1-1.png) | Windows 98 Second Edition's Start menu (16 colors, slightly off: `#C3C3C3` for `#C0C0C0`): only for [D8](#discrepancies), not compared |

Dialogs (in the `dialogs` subfolder):

| # | File | Source | Shows |
|---|---|---|---|
| D1 | `win95-run-dialog-347x163.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/features/run/win95.png) | Windows 95 Run, empty |
| D2 | `win95-shutdown-dialog-347x222.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/startupshutdown/shutdownwindow/win95.png) | Windows 95 Shut Down Windows |
| D3 | `win2000-run-dialog-347x179.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/features/run/win2000pro.png) | Windows 2000 Run |
| D4 | `win2000-shutdown-dialog-417x269.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/startupshutdown/shutdownwindow/win2000pro.png) | Windows 2000 Shut Down Windows |
| D5 | `win2000-logoff-dialog-288x123.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/startupshutdown/logout/win2000pro.png) | Windows 2000 Log Off Windows |
| D6 | `win98-logoff-dialog-288x123.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/startupshutdown/logout/win98.png) | Windows 98 Log Off Windows (also: the focus rectangle on a button) |
| D7 | `win95-run-dialog-typed-text-ok-enabled-347x163-shiftedcolors.png` | [Toasty Tech](http://toastytech.com/guis/win95rununc.png) | Run with text typed |
| D8 | `win95-shutdown-dialog-dimmed-desktop-640x480-shiftedcolors.png` | [Toasty Tech](http://toastytech.com/guis/win95shutdown.png) | Shut Down over the dimmed screen: where it sits, how the screen dims |
| D9 | `win98-run-dialog-347x163.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/system/features/run/win98.png) | Windows 98 Run, empty, caret blinked off |
| D10 | `win98-shutdown-dialog-323x192.png` | [GUIdebook](https://guidebookgallery.org/pics/gui/startupshutdown/shutdownwindow/win98.png) | Windows 98 Shut Down Windows, Stand by chosen |

Rejected: Wikipedia's "Windows 95 taskbar screenshot" (reduced to 16 colors), first-run
screenshots with off colors, and 15/16-bit captures (colors like `#D6D6CE` instead of
`#D4D0C8`). The preliminary *Windows Interface Guidelines for Software Design* (Feb 1995) gives
the bevel rules used above but no pixel sizes for menus or the taskbar.

## Discrepancies

Where the sources disagree. The screenshots win; RetroBar's value was used where no
screenshot shows the thing.

| # | What | RetroBar | 98.css | Screenshot (used) |
|---|---|---|---|---|
| D1 | Outer bottom/right line of buttons | `#000000` | `#0a0a0a` | `#000000` |
| D2 | Text color | `#000000` | `#222222` | `#000000` |
| D3 | Bottom/right line of the clock well | `#FFFFFF` | `#dfdfdf` | `#FFFFFF` |
| D4 | Font | TrueType "Microsoft Sans Serif" | a web conversion | the real bitmap font, drawn by GDI: matches the screenshots glyph for glyph |
| D5 | Gap between a task button's icon and its label | 3 | n/a | **2** |
| D6 | Space left / right of the clock text | 95: 8 / 8; 2000: 13 / 10 | n/a | **neither: a fixed-width clock part (widest time + 10, or + 19 in 2000) with the time centered**, which gave 9 / 8 and 13 / 13 for the screenshots' times (Reference PC, three times of day) |
| D7 | Windows 95 clock text height | baseline 12 (centered, no nudge) | n/a | **13**, 1px lower than the labels |
| D10 | Narrowest task button | 45, then 82 with scrolling | n/a | **22** (v86: just the icon, then the scroll arrows) |
| D11 | Pixels left over when buttons share a crowded row | spread one each over evenly spaced buttons | n/a | **all buttons equally wide**, the rest stays empty (v86) |

Between the screenshots themselves:

| # | What | Disagreement | Used |
|---|---|---|---|
| D8 | Windows 98's Start menu | 98 (first edition: S13, S15, S16, and Toasty Tech's `win98start.png`) has **36**-pixel items and a spare row at the bottom; 98 Second Edition (S17, and its 16-color cascade cut-out) has **32**-pixel items like 95 and no spare row | first edition: the look most screenshots show |
| D9 | Windows 98's title bar gradient | settled 2026-11-09: the first rule reproduced Log Off (D6) and Shut Down (D10) but was a shade off on Run (D9); the rule fitted to the Reference PC's windows reproduces all three | the new rule ([Windows 98](#windows-98)); D9's title bar is compared whole |
| D12 | The scroll arrows' triangles | the emulated "Windows 95" (its own drawing code, Internet Explorer 4's taskbar) puts the up arrow of a 16×11 button 1px higher than Windows 98 does | Windows 98's (Windows 11 draws them the same way) |
| D13 | A grayed-out push button's label | the emulator's Windows 95 (256 colors) and 98 (16 colors) draw a grayed Apply flat gray; GUIdebook's full-color Run dialogs (95 RTM, 98, 2000) draw grayed OK etched | etched (full color is what Bevel95 shows); compare-refs leaves those labels out of the emulator cases |
| D14 | Windows 2000's Search access key | the GUIdebook Start menus show no underlines; the emulator's keyboard-opened menu underlines the **c** ("Sear&ch") | Sear&ch (was Sea&rch) |
| D15 | Windows 98's Start menu with small icons vs its cascading menus | icon 2px in and label 9px after it, arrows 5px from the edge (the emulator's Start menu) vs 5 and 2, arrows 4px (GUIdebook's cascades) | each as measured: `start_small` and `small` in theme.rs |
| D16 | Windows 2000's Tahoma | GUIdebook's screenshots (S10–S12) match today's Tahoma letter for letter (but "y"); the Reference PC's Windows 2000 SP4 draws a few letters a pixel narrower or wider (bold "O" and space one wider; "Task " one narrower), while its clock's digits match | today's (with S10–S12's "y"); the Reference PC's menus are measured for their geometry only |
| D17 | Windows 98's notification area, one icon vs two | Screenshot S14 alone fits an 18-pixel step with the clock's text 9 after it | **17** from the Reference PC's two icons, with the clock's part fixed and centered (fits S14 too) |

## Flagged for your eyes

Things no screenshot settles. Compare the golden images (open [`docs/preview.html`](preview.html))
with your memory of the real thing.

1. **Windows' own menus in the 2000 look**: labels, items, arrows and separators are now
   measured on the Reference PC; the space after the labels and the window menu's pictures and
   shortcut column are still 95's and 98's.
2. **The option bullet's shape** (the Theme and Size submenus); the check mark is now measured.
3. **Cut-off titles** end in three periods `...` (Windows 95's DrawText).
4. **Very long Programs folders** continue in more columns; Windows 95 may have drawn a line
   between columns.
5. **"(Empty)"** in an empty folder.
6. **The focus rectangle's colors** are those of a 16-color screen (black dots), as in every
   Windows 95 screenshot found; on a true-color screen Windows 95 would have drawn dark gray.
7. **Windows 2000's Start flag**: the 95 flag is used (it matches the 2000 screenshot too).
8. **The task-button list of windows** uses the cascading-menu look (Windows 95 had no such
   list; Windows XP's was a plain menu with icons).
9. **The notification area**: the step between several icons in the 2000 look (22; only one
   icon seen), whether 95 and 2000 also add new icons on the right (98 does), and the well
   without a clock. Icons that programs
   draw as light symbols for Windows 11's dark taskbar are drawn dark (their colors inverted;
   RetroBar inverts Windows' own system icons the same way): a modern compromise, not a
   classic look.
10. **Where Run opens** (lower left), a **pushed** dialog button, and Windows 2000's **open
    list** of Shut Down choices: no screenshot shows them.
11. **The bar's right-click menu** shows "Ctrl+Win+T" next to "Switch back to Windows"
    again, spaced the way the emulated Windows 95 and 98 space shortcut texts.
12. **Windows 98's menus open at once**; the real ones slid open (an animation no screenshot
    can show).
13. **Windows 98 without Quick Launch**: every 98 screenshot has Quick Launch, and with it an
    etched line between the task buttons and the clock. **Settled 2026-11-20** by the captures
    `<pc>-truecolor-ql-off-<n>`: without Quick Launch 98 and 2000 keep the line after Start and one
    grip, the task buttons start at L+11 (69, 71), the task area ends 7 (98, its line 4 before the
    well) or 6 (2000) before the well; Bevel95 draws that since then (`quick_launch::task_band`;
    the 95 look keeps 95's bar). See [Quick Launch](#quick-launch).
14. **Narrow menus without submenus in Windows 2000** (98 has them): 2000 runs the same Start
    menu code, but no 2000 screenshot shows such a menu, so 2000 keeps the arrow room.
15. **Windows 98's Run box with text in it** (where the text starts) and **where a Windows 98
    submenu goes when it does not fit below**: no screenshot shows them (95's and 2000's rules
    are used).
16. **The group menu** of a button with several windows: Windows XP's entries (Cascade, Tile,
    Minimize Group, Close Group) in the classic look; 95, 98 and 2000 never grouped buttons.
17. **A pushed scroll arrow**, and the bar **showing the row of the window that comes to the
    front**: what Windows' own drawing code does, not seen on a real Windows 98.
18. **The narrowest task button (22, just the icon) in the 95 and 2000 looks**: seen on
    Windows 98 and on the emulated 95, whose taskbar came with Internet Explorer 4; the
    original Windows 95 and Windows 2000 were not checked.
19. **Windows 95's Taskbar Properties** is the version Windows 95 got with Internet Explorer
    4 (identical to 98's) but its preview, which is plain Windows 95's since 2026-11-18 (its
    Reference PC: Help, Run and Shut Down, its own taskbar). The PC's window has the same frame
    and places; its words differ ("Show Clock", the second page's sentences: private/todo.md).
20. **Windows 95's Start menu with small icons**: laid out like its cascading menus (22-pixel
    rows); the emulator's 95 has Internet Explorer 4's Start menu, so its numbers (20-pixel
    rows, as 98's cascades) were not used. Its 16×16 icons are the emulator's.
21. **Windows 2000's small Favorites and Log Off icons, and its large Favorites icon**: its own
    since 2026-11-18 (the Reference PC with Display Favorites and Display Logoff; until then 98's).
22. **Always on top switched off**: other windows can cover the bar, but maximized windows
    still stop above it (Windows XP's behavior as remembered; not checked on 95). **Auto hide**:
    the 2-row strip is Windows 2000's; the slide's speed and the half-second wait are guesses.
23. **The Properties window's held-down check box** (gray inside), **held-down option button**,
    **pushed scroll arrow** and **the selected row of the Start Menu Settings list**: no
    capture shows them.
24. **Bevel95 Settings** (Look, Size, Startup, Clock; a Taskbar page; a Desktop page; a Programs page) is ours,
    in Taskbar Properties' style: its title, its tabs ("General", "Taskbar", "Desktop", "Programs") and
    where it opens (a cascade step from Taskbar Properties' place) are our choices. No classic
    Windows had such a window.
25. **Classic icons for today's windows**: which picture goes where is our choice (File
    Explorer's Home as My Computer, a button of several Explorer windows as the folder, Control
    Panel recognized by titles without " - File Explorer", which is read in English). In the
    2000 look the narrowest buttons cut long titles one letter sooner than the real 2000, whose
    Tahoma is a little narrower in a few letters ([D16](#discrepancies)).
26. **The volume control opens at once** (the owner's wish): Windows 95 to 2000 waited a
    double-click time first. A double-click shows it for a moment before the mixer opens.
27. **Settings and Find as on a PC without a network** (the Reference PCs): no Find >
    Computer... (it opened Windows' Network folder); 98's Active Desktop submenu and 98's and
    2000's People are left out (nothing today to open).
28. **Classic names for Explorer's folder windows** (the owner's wish): "My Computer" for This
    PC and File Explorer's Home, "My Documents" in every look (Windows 95 itself had no My
    Documents folder), "Control Panel" for "All Control Panel Items", and every folder's name
    without " - File Explorer" (read in English, as for the pictures).
29. **Date/Time Properties sets Bevel95's own clock** (the owner's choice), never the PC's: the
    same window, but OK/Apply change only how far the bar's clock runs from the PC's time.
    Its clock hands are approximate for now, its month list was not seen on a reference PC.
30. **The classic desktop** is, so far, each look's desktop color and nothing else (exact on
    the Reference PCs). Its right-click menu ends with ours, one entry "Bevel95" (since 2026-11-16; its submenu:
    Bevel95 Settings, Quick Search, Switch to Windows desktop): Windows' own desktop menu came with the icons. Unlike the real desktop it does
    not take the focus when clicked.
31. **The list of programs** on Bevel95 Settings' Programs page is Windows' own tree of the
    Programs menu, measured on the Reference PCs ("Trees"), with one thing of ours: a check
    box in each row. No tree of Windows 95, 98 or 2000 had both lines and boxes. Each box is
    one its Windows had in a list; where it stands in the row is our choice for Windows 95's
    (3 in, 3 down), and the half-ticked flat box of 98 and 2000 (gray inside) is by analogy
    with Windows Setup's shaded box.
32. **Windows 98's and 2000's folder windows**: 98's smallest and largest new window are ours
    (at least three columns, at most 64% × 461/600 of the screen; only 5 and 8 items were seen);
    the address list stays inside the window (Windows' reached past it); 2000's tinted picture
    of the highlighted list row is drawn plain; the throbber does not turn while a folder is read.

## Appendix: the WPF text measurement

Before the screenshots were found, RetroBar's text positions were measured by rendering its
exact settings with WPF: a capital "H" put in a 16px box with RetroBar's font, size, margin,
centering and aliased rendering. The screenshots have since replaced these numbers where they
differ (the 95 clock is 1 row lower than this table).

| Label | WPF font | "H" covers rows | Baseline row |
|---|---|---|---|
| 95 task / Start label (nudged up 1) | Microsoft Sans Serif | 3–10 | 11 |
| 95 clock (no nudge) | Microsoft Sans Serif | 4–11 | 12 |
| 2000 task / Start label | Tahoma | 4–11 | 12 |
| 2000 clock | Tahoma | 5–12 | 13 |

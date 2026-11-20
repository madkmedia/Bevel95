# Decisions and history

Why Bevel95 is built the way it is, feature by feature, in the order the work was done. This
used to be the second half of CLAUDE.md; CLAUDE.md now keeps the rules and points here (the
review of 2026-11-09: every session paid for the whole history up front). Read the section of
the area you are about to change. Newer sections win where they differ from older ones.

## The original plan (AI-written; the decisions below override it where they differ)

The plan the project started from, kept as written. Its milestones 0 to 3 are done; its
"out of scope" list was later partly taken up (the notification area, tooltips) with the
owner's go-ahead.

### Theme spec (milestone 0, do this first)
- Clone https://github.com/dremin/RetroBar and read the theme resource
  dictionaries under its Themes folder for the 95-98 and 2000 looks. Clone
  https://github.com/jdan/98.css. Extract every taskbar/Start-menu-relevant
  number: bar height, button height/width rules, bevel widths and the exact
  colors of each bevel line, face colors, text colors, Start button
  geometry, clock well, gaps/margins, menu metrics.
- Produce `core/theme.rs` with two consts: `WIN95` and `WIN2000`. Also write
  `docs/theme-spec.md` listing each value with its source, and a
  "Discrepancies" section where RetroBar and 98.css disagree. Prefer
  RetroBar's values when they conflict and flag them for me.
- Headless test harness: render fixed states (empty bar; 3 groups, one
  active, one flashing; Start pressed; Start menu open with 10 items) to PNG
  under `tests/golden/`. Tests fail on any pixel diff. Golden images are
  regenerated only on explicit request.

### Milestone 1: the bar exists and is safe
- WS_POPUP window, WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE;
  WM_MOUSEACTIVATE returns MA_NOACTIVATE (clicking us must not steal focus).
- Reserve the bottom edge via SHAppBarMessage (ABM_NEW/QUERYPOS/SETPOS,
  ABM_REMOVE on exit). Primary monitor only in v1.
- Hide Explorer's taskbar: FindWindowW "Shell_TrayWnd" and
  "Shell_SecondaryTrayWnd" → ShowWindow(SW_HIDE). Re-hide if it reappears.
- ALWAYS restore it: on normal exit, on WM_ENDSESSION, in a panic hook, and
  via a `--restore` CLI flag. Losing the taskbar after a crash is the worst
  failure mode; test it deliberately.
- Right-click menu on empty bar (drawn by core, 95 menu style):
  "Switch to Windows taskbar", "Restart", "Exit". Shift+click Exit restarts.
- Global hotkey (RegisterHotKey, Ctrl+Win+T) toggles: show Explorer's
  taskbar + hide ours + ABM_REMOVE, and back.
- Clock (SetTimer, GetLocalTime), 95 style, in the right well.

### Milestone 2: windows and grouping
- Initial list: EnumWindows filtered by IsWindowVisible, no owner or
  WS_EX_APPWINDOW, not WS_EX_TOOLWINDOW, not cloaked (DwmGetWindowAttribute
  DWMWA_CLOAKED), non-empty title.
- Live updates: RegisterShellHookWindow + RegisterWindowMessageW("SHELLHOOK")
  (CREATED, DESTROYED, WINDOWACTIVATED, RUDEAPPACTIVATED, REDRAW, FLASH).
  Add a 2-second re-enumeration as a safety net.
- Group key v1: full exe path (GetWindowThreadProcessId → OpenProcess →
  QueryFullProcessImageNameW); fallback window class. Note in docs that UWP
  apps under ApplicationFrameHost will lump together; v2 uses AppUserModelID.
- One button per group, always combined. Left-click: single window →
  activate (ShowWindow SW_RESTORE + SetForegroundWindow), or minimize if it
  is already foreground; multiple → popup list of window titles (core-drawn,
  XP-style vertical list) to pick from. Middle-click → launch a new instance.
  Active group drawn sunken; flashing group highlighted.
- Buttons shrink as in 95 when the bar fills; titles elided.

### Milestone 3: Start
- Start button with embedded flag; pressed state while menu is open.
- Start menu (core-drawn): cascading "Programs" built from
  SHGetKnownFolderPath(FOLDERID_Programs) + FOLDERID_CommonPrograms merged;
  items are .lnk/.exe launched via ShellExecuteW; icons via SHGetFileInfoW
  on the .lnk path (no COM, no shortcut resolution needed). Plus "Run..."
  (our own tiny dialog → ShellExecuteW) and "Shut Down..." (shutdown.exe).
- Windows key: WH_KEYBOARD_LL, swallow bare LWIN press/release, open menu.
- Keyboard nav in menus (arrows, Enter, Esc, letter accelerators).

### Explicitly out of scope for v1 (do not start without asking)
Notification area/tray, multi-monitor, pinned items/Quick Launch, taskbar
on other edges, tooltips, Vista+ themes, Linux backend (keep core clean for
it, but write nothing).


## Milestone 1 decisions
- Hiding Explorer's taskbar also switches it to auto-hide (ABM_SETSTATE), or
  it keeps reserving its strip of the screen; RetroBar's ManagedShell does the
  same. That is the user's own setting, so its original value is written to
  `%LOCALAPPDATA%\Bevel95\windows-taskbar-state.txt` before changing it, and
  every exit path (normal exit, WM_QUERYENDSESSION, panic hook, watchdog,
  `--restore`) restores it from that file and deletes the file. From
  WM_QUERYENDSESSION until a WM_ENDSESSION(FALSE) the 100 ms tick leaves it alone
  (`App::ending`), or it would switch auto-hide back on before the session ends
  (found by the code review of 2026-11-08; `cargo selftest` checks it). If the file
  cannot be written, the setting is not changed. **The record is certain before the
  setting changes** (found by the safety review of 2026-11-18, B2: the file was written in
  place, so a power cut could leave it empty while Windows had kept auto-hide on; the next
  start took the empty file as "recorded", and `restore` deleted it without putting anything
  back; and a portable copy, whose record is in its own folder, did not see the installed
  copy's and took auto-hide, switched on by Bevel95, for the original):
  - `hide` writes the record the safe way (`state_file::write_whole`: flushed, then put in
    place with MOVEFILE_WRITE_THROUGH) and reads it back; only then is auto-hide switched on.
  - **A record that cannot be understood** (empty, cut short, not a number, unreadable) is not
    "recorded": the original is not known. `hide` then leaves the setting alone for that run
    (logged once; Windows' taskbar keeps its strip under the bar, as when the file cannot be
    written), and `restore` switches auto-hide **off** (Windows' default; the other bits as they
    are) and removes the record, so the next start records afresh. Chosen because it can never
    leave auto-hide on by Bevel95's doing; the price is that someone whose own setting was
    auto-hide on finds it off after such a power cut.
  - **Two copies**: `hide` and `restore` look in this copy's own data folder and, for a portable
    copy, also in the installed folder (`explorer::places`); a record understood in either counts,
    the oldest when both have one. A portable copy only reads the installed one's record: it
    writes and removes only its own. So a portable copy started after the installed one ended hard
    restores the true original, and the installed copy's `--restore` (the desktop shortcut)
    restores from its own record as always. **Not covered**: a portable copy that ended hard,
    followed by the installed copy: the installed copy cannot know where a portable folder is, and
    a portable copy writes nothing outside its folder; that portable copy's own next start, its
    watchdog or its own `--restore` puts it back.
  - **The installed copy's record decides once for a portable copy** (found by the second safety
    review of 2026-11-18, B2): a portable copy may not remove the installed copy's record, so it
    used that old record at the end of every run: a person who had since switched auto-hide on
    in Windows' settings found it off again after every portable run. Now, when a portable copy
    has restored from it, it keeps a note in its own folder (`windows-taskbar-used.txt`: the
    record's time, in 100-ns steps, and its content); a record matching the note no longer counts
    for that copy (`explorer::counted`), so its next run records the state the person has now.
    A record the installed copy writes later (another time) counts again. The note is in Remove's
    list (it never exists in the installed folder) and README's. Test:
    `explorer::tests::the_installed_copys_record_decides_once_for_a_portable_copy` (fails before:
    the old record was "recorded" at every run).
  - **`hide` and `restore` take turns** (the second safety review of 2026-11-18, suspected 1, not
    proven, closed anyway): a panic hook's `restore` on a worker thread while the main thread was
    inside `hide`, past its check of `panic::happened`, could remove the record and put the
    setting back, after which `hide` switched auto-hide on with no record left. Both now take one
    lock (`explorer::RECORD_LOCK`), waiting at most a second (`RECORD_WAIT`): `restore` then goes
    on without it as before, `hide` does nothing that time. `hide` checks `panic::happened` again
    once it holds the lock. A panic inside `hide` on its own thread runs the hook's `restore`
    there: it sees this thread holds the lock and goes on at once, so the hook can never hang.
    **The lock is one for all of Bevel95's processes** (the third safety review of 2026-11-18,
    C2: it was a `std::sync::Mutex`, so an old bar's watchdog restoring 100 ms after its bar's
    window went, or the "Restore Windows taskbar" shortcut's `--restore`, could put the setting
    back and remove the record in the middle of a new bar's `hide`, leaving auto-hide on with no
    record): a named mutex, `Local\Bevel95 taskbar record`, as the Undo list's lock is, taken by
    the bar's `hide` and `restore`, the panic hook's, the watchdog's and `--restore`'s, with the
    same limit and the same rule for the panic hook's own thread. A program that ended holding it
    leaves it abandoned, and the next taker gets it (the record is written whole). Tests:
    `explorer::tests::hide_and_restore_take_turns_and_never_hang_the_panic_hook` (two
    threads: the restore comes after the hide; a hide that never ends lets the restore go after
    the limit; the same thread at once; a lock left by a panic; on a lock name of the test's own),
    `the_record_lock_is_shared_with_other_processes` (Windows PowerShell holds the name for 1.5 s:
    this process waits its limit, then gets it once PowerShell lets go).
  - **Every way that gives the screen back runs whatever is known of the data folder** (the fifth
    safety review of 2026-11-18, A1). `--restore`: always all of it (decided without a message;
    not known: the record read beside the program and in the installed folder, and what was used
    retired as in every mode, nothing else written and nothing logged; "Bevel95 Setup", where a
    copy keeps its data). **A restore, in every mode, retires what it restored from** (the sixth
    safety review of 2026-11-18, B1: a not-known `--restore` kept the record, so a later run took
    it for a live original and switched back an auto-hide the person had set since): the record
    beside the program is Bevel95's own file by its name and is removed; the installed copy's
    record used by a program that is not the installed copy is noted beside the program, as a
    portable copy notes it (`state_file::write_whole_even_if_not_known`, for this one file). With
    no program folder known (`current_exe` failed) nothing is retired. Test:
    `explorer::tests::a_restore_puts_the_setting_back_whatever_is_known_of_the_data_folder`. The watchdog,
    `--watchdog-launch` and `--after`: told (installed, or portable with the flag), never "not
    known". The bar's Exit, its WM_QUERYENDSESSION and the panic hook: only in a bar that runs,
    which has decided its folder. Setup's closing of a bar and Remove run the installed copy's
    `--restore`, which decides "installed" without reading. A start that cannot know (the bar,
    `--setup`, `--uninstall`, a companion) shows the message and ends before it has hidden or
    closed anything. Tests: `explorer::tests::a_restore_puts_the_setting_back_whatever_is_known_of_the_data_folder`,
    `tests::a_restore_is_never_stopped_by_the_data_folder` (main.rs).
  - Tests (the record's functions, without a taskbar): `explorer::tests` (written whole and read
    back; five records that cannot be understood, each "leave alone" and "switch off, remove";
    two copies; one place for the installed copy). The bar's ordinary start and exit after the
    install showed the log's "saved ..." and "restored ..." lines. Explorer's windows are shown and
  hidden with posted calls (ShowWindowAsync, SWP_ASYNCWINDOWPOS), so a hung Explorer
  cannot hang the bar, the watchdog or the panic hook.
- A watchdog process restores after hard kills and crashes, and performs
  restarts (the bar exits with code 95; the watchdog restores, then starts a
  new bar), so two bars never overlap. It is started via a short-lived
  launcher, so it is not the bar's child (Task Manager's "End task" on a
  process group would kill both). Since 2026-11-18 a bar that ends while a
  copy or move of its own runs gives the screen back and its place (the mutex)
  at once and keeps its process, without a window, until the job is done
  ("File jobs outlive their window"); since the second review of that day the
  watchdog takes the bar's window gone for "ended", restores and starts a
  Restart's new bar at once, and a new bar may run beside the old process.
- **A panic on any thread ends the bar** (found by the safety review of 2026-11-18, suspected
  2, traced in the code): the panic hook runs for a panic on any thread and gave the screen back
  (appbar removed, Windows' taskbar restored, the record removed), but a panic on a worker thread
  (a file job, the Start menu's icons, the apps list, the desktop's thread) ends only that
  thread. The bar ran on: within 100 ms its tick saw Windows' taskbar and hid it again
  (recording the restored state, which was right), but its appbar stayed removed, so maximized
  windows ran under the bar until the next change of screen. Now (`winkit::panic`, the hook every
  Bevel95 program installs on its main thread): the hook logs the panic, the bar gives the screen
  back as before and posts WM_CLOSE to its own window, and from then on `explorer::hide` and the
  tick do nothing (`panic::happened`). The bar ends the ordinary way (`on_destroy` restores once
  more, `finish_file_jobs` lets a running copy finish), with exit code 0: the watchdog restores
  again and does not start a new bar (a bar that restarted after every panic could loop). A panic
  on the main thread inside a window procedure aborts the process (Rust does not unwind out of
  `extern "system"`), elsewhere it unwinds out of `main`: either way the process ends and the
  watchdog restores.
  - **The other programs** (folders.exe, Minesweeper, Solitaire, Calculator, the Help viewer)
    install the same hook with `panic::close_windows_after`: a panic on a worker thread (a
    folder's or the tree's reader, an address, a file job; the Help viewer's reader of its
    caller's requests; the Help owner threads in each game) is logged and closes the program's
    visible windows of Bevel95's class as if their Close button were clicked (each saves what it
    saves on close), and the program ends as usual: folders.exe stays without a window while a
    file job runs. Windows' own windows of the program (a copy's progress window, Properties) and
    folders.exe's hidden hand-over window are left alone. Before, nothing was logged and the
    windows stayed, waiting for work that never came.
  - Test: `platform/winkit/tests/panic_rule.rs` (its own process, a scratch log): a panic on the
    main thread closes nothing; on a worker thread the visible window of Bevel95's class gets
    WM_CLOSE and goes, a hidden one and one of another class stay; the log has both lines. The
    owner's bar was never made to panic.
- The Explorer taskbar is re-hidden whenever it reappears (100 ms check, as
  ManagedShell does). The same tick follows the clock, screen size/DPI
  (WM_DPICHANGED is unreliable for tool windows on Windows 11, per
  ManagedShell) and fullscreen apps (the bar steps behind a window covering
  the whole main screen).
- The right-click menu window takes the focus while open (so Esc and clicking
  elsewhere work) and gives it back after; the bar itself never takes it.
- Settings: `%LOCALAPPDATA%\Bevel95\settings.txt` (`era`, `scale`), created
  with comments on first run. Log: `bevel95.log` in the same folder.
- **No private text in the log** (2026-11-17, before Bevel95 goes public: people attach the log to
  their reports). No log line holds text from outside Bevel95: no window titles, file or folder
  names, notification icons' tooltips (a Wi-Fi network's name), what was typed into Run, program,
  document or desktop item names, paths under the user's folders, the network's name. A line says
  what a report needs instead: a window's class and its program's file name
  (`window_list::describe`), a count, a kind (`winkit::log::path_kind`: "a folder", "a .txt file";
  `typed_kind`: "a command", "a web address", "a path to a .docx file"), Bevel95's own folder as
  `%LOCALAPPDATA%\Bevel95\settings.txt` (a portable copy's as "the portable folder\..."). Bevel95's
  own names and settings stay. The one exception is `bevel95 --list-windows`, which the person runs
  on purpose: its first line says the lines below hold window titles and program names. Guarded by
  `platform/winkit/tests/private_log.rs`, which reads every log call of the bar, winkit and the
  programs for the patterns taken out (a test of the sources, not of types: a log call takes a
  `&str`, and wrapping every call in a type would be a large change for the same guard). The
  first line of every start says the version and build ("Groundwork for sharing" below).
- **Window resources in the log** (2026-11-13, after a morning when Windows' shared pool for
  windows, the desktop heap, ran out: another program's doing by Windows' System log; Explorer
  crashed and Bevel95 was refused windows). The bar's tick calls
  `winkit::log::watch_gui_resources`: every ten seconds it reads the bar's own USER and GDI
  object counts (GetGuiResources, with Windows' own peaks; windows-sys feature
  `Win32_System_Threading`, already on), writes one line "window resources: ..." every ten
  minutes (the first at start), and a plain WARNING line the first time either count reaches
  2,000 and again at 5,000 (`core::gui_watch`: thresholds and wording, tested). Every
  `log_win32("CreateWindowExW...")` that failed with error 8 (not enough memory) adds both
  counts and the likely cause: Windows' shared pool, unless Bevel95's own count is near the
  10,000 Windows allows a program. The folder windows' program (olders.exe, 2026-11-13) writes
  the same line from its hidden window's timer; the games, Calculator and the Help viewer do not yet.
- Painting copies the upscaled picture 1:1 with SetDIBitsToDevice (no GDI
  scaling), which is equivalent to the DIB section + BitBlt in the plan.
- `cargo selftest` runs the real exe and checks the taskbar comes back after
  hotkey toggles, Restart, a hard kill, and `--restore`. It briefly hides the
  owner's taskbar, so it is opt-in, never part of `cargo test`.
- **The tests never write the real data folder** (found by the second safety review of
  2026-11-18, C3): every log line of a test went to `%LOCALAPPDATA%\Bevel95\bevel95.log`, the
  bar's own log (counting towards its 1 MB rotation), and on a contributor's PC `cargo test` made
  that folder; started from the Claude app it was made in the app's private area, which
  `tools\install.ps1` then refuses. Now `log::installed_dir` in a test build puts a scratch folder
  (`%TEMP%\bevel95-tests-localappdata`) in place of %LOCALAPPDATA% as Windows knows it
  (FOLDERID_LocalAppData); a test that points LOCALAPPDATA elsewhere itself (`panic_rule.rs`)
  keeps that. A test build is: winkit's own unit tests (`cfg(test)`), and everything built by
  `cargo test` with winkit's feature `test-scratch`, which every crate that uses winkit switches
  on in its `[dev-dependencies]` (winkit itself for its integration tests, by a dev-dependency on
  itself). Cargo switches dev-dependencies' features on only for a build that includes tests, so
  `cargo build --release` (install.ps1, release.ps1) never has it: a released program gained no
  switch, and its data folder is found as before. Chosen over `[env]` in `.cargo\config.toml`
  (it would also reach `cargo run`, the builds and the public copy's config, and moving
  LOCALAPPDATA for rustc and the linker is not wanted). The bar that `cargo selftest` starts is a
  test build too: it keeps its record in the scratch folder, where the test now looks
  (`bevel_winkit::log::installed_dir`). Guard: `platform/winkit/tests/real_data_untouched.rs`
  (a test build's folder is not the real one; after its own log line the real folder was not
  made and its logs do not hold the line; fails before).
  - **A test build is never installed or packed** (the third safety review of 2026-11-18,
    suspected 1, tried in a build folder of its own, `private/probes/review3/release-exp`):
    `cargo test --release` does put test builds of all six programs at
    `target\release\<name>.exe`, where a release build is looked for (each held the tests'
    scratch code); a `cargo build --release` afterwards rebuilt all six and put plain ones back
    (none held it; the same sizes as the installed ones). So the scripts' own build already gave
    plain programs, but a program copied by hand after a test run would not be one. Now
    `tools\install.ps1` and `tools\release.ps1` check every built program for that code's mark
    (the scratch folder's name, which only `log::tests_base` holds; `tools/no-test-build.ps1`)
    after the build and stop with a plain message ("Nothing was installed: ... is a test build
    (made by cargo test, ...). Run cargo build --release and try again."), and the packing
    (`core/examples/pack.rs`) refuses one too. Tried: the check on the six test builds stopped,
    the packing refused and wrote nothing, the installed programs passed.
- **The tests show no windows** (2026-11-18, the owner: "Why the many bursts of cmd windows
  periodically on my system?"). A test binary of a windowed program (the bar's, the programs')
  has no console of its own, so each console program its tests started opened a window of its
  own (a Windows Terminal window on Windows 11), and `cargo test --workspace` ran many times a
  day. The starters: icacls in four tests (`log.rs`, `install.rs`), a copy of cmd.exe running
  `ping` for up to 30 s twice (`install.rs`, Remove's wait), PowerShell holding a lock
  (`explorer.rs`), cmd for a junction (`copy.rs`); git in winkit's build script is covered too
  (it inherits cargo's console, so it showed nothing, but the rule leaves no exception). Rule: every
  process a test, an example or a build script starts goes through `bevel_winkit::quiet`
  (`quiet::command`, CREATE_NO_WINDOW; a CreateProcessW of a test's own names
  `quiet::NO_WINDOW` in its flags; the build script includes the same file by `#[path]`). A
  program with windows of its own shows them on a private desktop (Help's tests) or its test is
  `#[ignore]`d with the reason (`cargo selftest`). Guard: `platform/winkit/tests/quiet_starts.rs`
  reads the test code of every crate (`tests`, `examples`, build scripts, each source file from
  its test module on) and fails on `Command::new(`, ShellExecute, WinExec or a CreateProcess
  without `quiet::NO_WINDOW`. Counted with `private/probes/quiet-tests/run.ps1` (a watcher of the
  top-level windows shown during one `cargo test --workspace`, by class and program, never by
  title): before, 10 windows of consoles (7 Windows Terminal windows, 3 of pseudo-consoles);
  after, none. Two
  tests still make windows of their own process, as they need visible ones (`jobs_rule.rs`,
  `panic_rule.rs`: three 1x1 tool windows at -32000, never activated, seen by no one). The
  programs themselves start no console program with a window: Setup's self-removal runs
  PowerShell with CREATE_NO_WINDOW, Shut Down runs shutdown.exe by ShellExecute with SW_HIDE, and
  everything else they start has windows of its own (Bevel95's programs, Control Panel,
  rundll32's wizard, the person's own programs and what they type into Run, which keeps
  Windows' own way).

## Milestone 2 decisions
- Grouping logic lives in `core::tasks` (`TaskList`): button order is the order
  windows first appeared (at startup: least recently used first) and stays
  stable; a button sits where its oldest window is.
- Button label: one window = its title (95 style); several = "N AppName" (XP
  style). AppName is the exe's version-info FileDescription, else its file name.
- **Characters the fonts lack** (2026-11-11; until then every one was drawn "?", so an editor's
  unsaved file, whose title begins with U+25CF, a black circle, showed as "? ..."). The
  glyph atlases held Windows-1252 only (now `font::charset_1252`; other European alphabets
  since 2026-11-19, below), as Windows 95's window titles could.
  Windows 95 read titles through Windows' "best fit" conversion to that code page, which
  turns many characters into a near one and the rest into "?". `core::text`'s `Font::glyph`,
  the one place every width and drawing goes through (labels, menus, tooltips, text fields),
  now does the same for a character the font does not have:
  - Windows' best-fit character, if the font has it: a table of 452 pairs asked of Windows
    itself (`tools/dev/bestfit.ps1`: WideCharToMultiByte(1252, 0, ...) for U+0100..U+FFFF,
    the pairs whose answer is one byte other than "?"), committed as sorted const data
    (`core/src/best_fit.rs`) and looked up by binary search. U+0141 L, U+0107 c, U+2011 -,
    U+2009 a space, U+202F a no-break space, U+FF01 !.
  - **Bevel95's own two additions**, which Windows 95 did not make (its best fit has "?" for
    them): the round dots U+25CF, U+2B24 and U+26AB are drawn as the font's bullet U+2022,
    and invisible marks (U+200B..U+200F, U+202A..U+202E, U+2060..U+2064, U+FE00..U+FE0F,
    U+FEFF) draw nothing and are 0 wide. Today's titles carry these where 1995's had none.
  - Everything else stays "?". A character is still one glyph, so counting by characters (the
    "..." of a shortened label, the access key's underline, the caret in a text field) is
    unchanged. No golden picture changed.
- **Letters of other European alphabets** (2026-11-19, the first reports after the release: a
  98-look Start menu of a Russian Windows showed every Cyrillic letter as "?"). The atlases of
  every program (`winkit::font::render`) now hold, besides code page 1252, the letters of
  Central European (1250), Cyrillic (1251), Greek (1253), Turkish (1254) and Baltic (1257)
  Windows: 250 characters (`font::OTHER_CODE_PAGES`, each code page's bytes 0x80..0xFF that
  1252 lacks, asked of Windows' own code page tables; each letter listed once, under the first
  code page that has it, so Baltic letters that 1250 has too come from 1250's font).
  - **Nothing of 1252 changes.** Its letters are drawn exactly as before: the same face asked
    the same way (`ANSI_CHARSET`, by its em). The other letters are added beside them, set on
    the same baseline and cut to the same cell; a test checks that `render` holds
    `render_1252`'s glyphs unchanged. The font fixtures hold 1252 only (`render_1252`,
    `cargo regen-fonts`), and no golden picture or comparison case changed.
  - **Each look's own face.** 2000: Tahoma asked in each code page's character set (Tahoma has
    them all). 95 and 98: what a Russian, Greek, Polish, Turkish or Baltic Windows 95/98 had,
    MS Sans Serif in that code page: the bitmap files `SSERIFEE.FON` (1250), `SSERIFER.FON`
    (1251), `SSERIFEG.FON` (1253), `SSERIFET.FON` (1254) and `SSEE1257.FON` (1257), all in
    Windows 11's Fonts folder but registered only for a Windows of that language. On an English
    Windows 11, asking for MS Sans Serif in those character sets gives Arial (TrueType, a cell
    of 14). So Bevel95 loads the code page's file for its own process alone
    (`AddFontResourceExW` with `FR_PRIVATE`; nothing in Windows changes) **before** it first
    asks for the face in that code page, and asks the way a bitmap font is sized: by its whole
    cell, the 13 of the 1252 font, bitmap fonts first (`OUT_RASTER_PRECIS`). Loaded only after
    a failed ask, as first built, the next ask in the same process got other letters under the
    same name (in the bar a TrueType font 14 px tall; `tests/first_atlas.rs` guards it). Where
    a PC has no such file, Windows' substitute is taken, drawn without smoothing at the same
    size and cut to the cell, and the log says once which face came instead; where nothing can
    be drawn, the letters stay "?". A limit (the review before 0.1.1, not tried): on a Windows
    of another language, that language's file (`SSERIFER.FON` on a Russian one) is the
    registered "MS Sans Serif", not `SSERIFE.FON`, and the 1252 font asked with
    `ANSI_CHARSET` may then not be the one expected; `load_privately` could load `SSERIFE.FON`
    first for the 1252 letters too. Another (cosmetic): `text::ms_sans_serif` takes the
    platform's font only if `render` ran before its first use, so a program that draws a
    dialog template or a Help control before rendering its look's font keeps the 1252-only
    fixture for that run (other alphabets' letters show as "?").
  - **Not compared with an original.** No Reference PC runs a Russian, Greek, Central European,
    Turkish or Baltic Windows, so these letters' pixels are those of the bitmap files above as
    today's GDI draws them (95, 98) and today's Tahoma (2000), not checked against captures.
  - **The windows that use MS Sans Serif whatever the look** (classic dialog templates, Help's
    controls: `core::text::ms_sans_serif`) get the same atlas from the platform
    (`text::use_platform_ms_sans_serif`, set by the first `font::render` of a program); the
    headless tests keep the fixture.
  - **Typed and compared names.** Typed letters reach every text field as they are (WM_CHAR,
    one UTF-16 unit each; these letters are all in the Basic Multilingual Plane). Quick Search
    compares by `core::text::fold`: lowercase letter by letter in every alphabet, Turkish İ and
    ı both as "i", Greek final sigma as sigma. The Programs menu's merging of the two Start Menu
    folders and the address bar's drive names compare by `text::same_name` (each letter's
    one-letter uppercase, as Windows compares file names), no longer by ASCII case only.
  - **Still "?"**: Chinese, Japanese, Korean, Hebrew, Arabic, Thai and the rest; combining
    marks are not composed; right-to-left text is not laid out. Characters outside these lists
    keep Windows 95's best fit to 1252 (`best_fit`).
  - **Cost**: the two fonts of a look take about 6.5 ms (MS Sans Serif) and 8 ms (Tahoma) in a
    release build instead of 2.5 and 3.2 (the first in a process about 14 ms with the files'
    loading); a few milliseconds per program start.
- ApplicationFrameHost.exe windows (Settings, Calculator, ...) are grouped by
  title instead of lumped together; AppUserModelID grouping is still v2.
- Icons: WM_GETICON (small2, small, big; 50 ms SendMessageTimeout) → class
  icon → SHGetFileInfoW. Pixels come from drawing the HICON with DrawIconEx on
  black and on white and solving for alpha (instead of GetIconInfo/GetDIBits:
  works the same for masked and alpha icons of any size).
- Live updates: shell hook (SHELLHOOK message) plus a full re-read every 2 s
  and whenever the foreground window changes (checked on the 100 ms tick).
  HSHELL_FLASH and HSHELL_RUDEAPPACTIVATED are not in windows-sys; they are
  defined locally exactly as WinUser.h does (… | HSHELL_HIGHBIT).
- Clicks act on mouse-up over the same button, with the pressed look while
  held (mouse capture). Middle-click: ShellExecuteW on the program path.
- Scroll arrows and the right-click window menu came later (see "Start with
  Windows, scroll arrows, window menu" below).
- `bevel95 --list-windows` writes what the bar would show to the log.

## Milestone 3 decisions
- Pixel exactness is checked against real screenshots (GUIdebook, Toasty Tech;
  listed with URLs in `docs/theme-spec.md`). They are Microsoft's pictures, so
  they stay out of the repo: `tools/fetch-refs.ps1` downloads them (and fixes
  Toasty Tech's shifted palette), `cargo compare-refs` (opt-in, folder in
  `BEVEL95_REFS`) draws the same states and fails on any differing pixel;
  only the apps' own icons, a mouse pointer and a tooltip are masked. All 13
  cases match with 0 differences. Screenshots win over RetroBar where they
  differ (task icon gap 2, clock margins 9/8 and 13/13, 95 clock baseline 13).
- Three menu styles in `core::paint` (`MenuStyle`): Windows' own menus
  (`Popup`), the Start menu (`Start`) and its cascades (`Cascade`, also used for
  a task button's window list), each with measured metrics per era; 2000's
  `Popup` values are provisional (no screenshot). Long menus continue in more
  columns. (The bar menu's "Ctrl+Win+T" was dropped here and came back once
  shortcut-text spacing was measured, see below.)
- Pictures cut from the screenshots: both Start menu banners, the Start menu
  icons, program-group icons, dialog icons (`core::banner`, `core::icons`) and
  the 2000 Shut Down banner (`art/assets/*.rgb`, raw RGB); since 2026-11-18 cut
  from the Reference PCs instead, found identical ("Every picture from the
  owner's own Windows"). Today's Tahoma
  differs from 2000's in one glyph row ('y'); `Font::with_period_glyphs` puts
  the old row back.
- The Start menu opens on mouse-down on Start (a second click closes it), is
  placed with its bottom edge on the Start button, and shows Start pressed
  with Windows 95's dotted focus rectangle (2000 has none). The Programs tree
  is re-read on each open. Item icons come from the shell's small image list
  via `IImageList::GetIcon` (vtable call; `SHGetImageList`) with the thread
  DPI-unaware: real 16×16 pictures without the shortcut arrow. Cached by path.
- Windows key: `WH_KEYBOARD_LL` (only while our bar shows), on a thread of its
  own with its own message loop (Windows silently drops a low-level hook that
  keeps the keyboard waiting about a second; the bar's thread can be that busy).
  A lone Win press has its release held back; the bar then sends VK 0xE8 and the
  release (SendInput), so Windows sees a combination and keeps its own menu shut.
  Ctrl+Esc is swallowed (not with Shift: Task Manager; not with Alt). A Win press
  while the key reads as up starts afresh (its release may have gone to the lock
  screen). Menus opened this way get the focus through `util::take_focus`
  (AttachThreadInput fallback).
- Dialogs are core-drawn (`core::dialog`): Run, Shut Down (dims the screen
  with a checkerboard screenshot window first), 2000's Log Off. Actions:
  `shutdown.exe /s /t 0` and `/r /t 0`, `ExitWindowsEx(EWX_LOGOFF)`,
  `SetSuspendState`. Run splits "program args" at the longest existing path,
  keeps `run-history.txt` (Up/Down), and Browse uses Windows' file picker.
  Settings/Find/Help/Windows Update open today's equivalents.
- **Run's list of earlier commands** (2026-11-13, the owner: "The dropdown for Run doesn't
  work"; the button was drawn but a click there landed on the field, whose rectangle contains
  it: the button now comes first in the layout). Measured on the three reference PCs
  (theme-spec.md, "Run's list"). `core::droplist` is the Time Zone page's list made shared
  (`DropList` place and painting, `Shown` first row and highlight, `find_prefix`); the Time Zone
  page now paints through it (its 26 compare-refs cases unchanged). `DialogWindow` keeps the
  open list (`list`, `run_list`, `open_list`, `close_list`): the button opens it on mouse-down
  (`Press::Grab`, so a drag onto a row and letting go there takes it, as Windows did), F4 and
  Alt+Up/Down toggle it, Enter/F4 close it taking the highlighted command, Esc and any other
  click close it as it is, losing the front closes it. Windows showed the list in a window of
  its own, past the Run box's edges; the control kit does the same with one window: a form
  can say it has a list outside its rectangle (`Form::drop_rect`, `paint_drop`), and the host
  (`winkit::kit`, `reshape`) then makes the window big enough for both and shapes it with a
  window region to the form and the list (nothing else of it is there: clicks beside the list
  go to the windows behind), keeping the form where it was; mouse coordinates are counted from
  the form's corner (`Host::origin`). The list opens above the field when it would pass the
  screen's bottom edge (`DialogWindow::screen_bottom`, from the bar's monitor). Decided
  differently from the originals: the wheel scrolls the open list in the 95 look too (95 had
  no wheel; the Time Zone list already scrolls there), and with no command yet the list is one
  empty row (not seen). Not built: the slide-open animation, Internet Explorer's AutoComplete
  (98 inline, 2000 a suggestion list), the last command preselected when Run opens (todo).
- The bar's right-click menu has Theme (Windows 95 / Windows 2000) and Size
  submenus. Both switch live (a theme change re-renders the font atlases) and
  are written back into settings.txt with `config::with_setting`, which keeps
  every other line and comment.

## Windows 98 look (added after milestone 3, on the owner's request)
- A third era, `Era::Win98` / `WIN98` (`era = win98`), between 95 and 2000 in
  the Theme menu. It is 95's theme except the Start menu, dialogs and one task
  button rule; `docs/theme-spec.md` has a "Windows 98" section. Windows 98 *first
  edition* (98 SE's Start menu has 32px items; noted as discrepancy D8).
- New theme fields: `StartMenuMetrics::bottom_pad` (98: one face row under the
  last entry) and `IconItems::pad_right_no_arrows` (98: menus without submenus
  are narrower). `Era::always_shows_keys()` replaces `era == Win95` checks.
- Start menu: Windows Update, Programs, Favorites (`FOLDERID_Favorites`, read
  like Programs; `popup::Tree` picks the folder tree), Documents, Settings,
  Find, Help, Run, Log Off, Shut Down. Settings adds Folder Options and Windows
  Update (`Target::FolderOptions`, `Target::WindowsUpdate`).
- Dialogs: 98's Shut Down (Stand by / Shut down / Restart / MS-DOS mode, OK
  Cancel Help), Run with 98's text, Log Off as 2000's. 98's title bar gradient
  depends on the dialog's screen position modulo 8 (`Dialog::screen_x`).
- Icons: 98's 32×32 Start icons from GUIdebook's one full-color 98 screenshot
  (since 2026-11-18 the Reference PC's, found identical, Log Off too; Windows Update is 98's own
  since then, not 2000's);
  its Programs folder icon only exists in 16-color shots. `cargo compare-refs`
  maps our colors to 16 colors for 16-color screenshots (`Case::vga`).
- Not done: 98's menu slide animation; 98's Active Desktop and Find > People.

## Start with Windows, scroll arrows, window menu (owner's go-ahead)
- **Start with Windows** (now a check box in Bevel95 Settings; it started in the bar menu,
  then sat on a page of the Properties window): the owner chose the registry Run
  entry over a Startup-folder shortcut. This is the one registry write, an
  owner-approved exception to "never registry edits": value `Bevel95` under
  HKCU\...\CurrentVersion\Run = the quoted exe path (`autostart.rs`). It counts
  as on only if it points at this exe and Task Manager has not disabled it
  (StartupApproved\Run, odd first byte); switching on clears that note,
  switching off removes both. Only values named `Bevel95` are touched.
- **New reference source "v86"**: real Windows 95 (with IE4's taskbar, 256
  colors) and Windows 98 (16 colors) run in the copy.sh/v86 emulator in the
  in-app browser, their canvas read with getImageData (exact pixels; the pane's
  screenshots are only previews). Not scriptable, so the numbers are locked in
  tests (`menus_measure_like_windows`, layout.rs tests) and in theme-spec.md, not
  in compare-refs. Pixels stay out of the repo, like the screenshots.
- **Task layout follows Windows 98, not RetroBar** (theme-spec D10, D11): all
  buttons equally wide (floored share, leftover empty), down to 22 (icon only,
  `min_width`); then scroll arrows (`TaskMetrics::scroll_w` 16) flush with the
  task area's right end, row height split 11/11, rows of as many 25px slots as
  fit (`layout::TaskScroll`, `BarLayout::first_task`). Arrows act on mouse-down;
  the row follows the window that comes to the front.
- **Window menu** (`core::window_menu`): fixed entries like RetroBar's, grayed
  by Windows' own rule from the window's style (`window_list::window_state`),
  Restore bold when minimized else Close; carried out as WM_SYSCOMMAND posted to
  the window, after SetForegroundWindow while our menu still has the focus
  (except Minimize). 98/2000 add pictures (`MenuGlyph`), and their labels start
  at 17 while the shortcut column leaves room as if they started at 21 + 17
  (measured quirk). Several windows: Windows XP's group menu (CascadeWindows,
  TileWindows, SC_MINIMIZE/SC_CLOSE to each).
- **Windows' own menus, measured** (`PopupMetrics`): shortcut text after a tab
  (`MenuEntry::parts`) starts 8 after the longest label, 19 after the widest
  shortcut (regular font); bold default entries (`MenuEntry::bold`); grayed
  entries plain gray in 95/98, etched in 2000 (`etched_disabled`, provisional),
  highlightable by keys and pointer (`Menu::selectable`); real check mark at
  x 6. Bold MS Sans Serif underlines access keys 1px longer (`Font::overhang`
  from GDI `tmOverhang`, stored in the bold fixture); compare-refs case
  `win95-start-button-menu` (S8) confirms bold + overhang.
- Testing note: messages posted to the bar from a DPI-unaware process get
  their coordinates scaled by Windows (×1.5 here); post from a DPI-aware
  process. Menus opened by simulated clicks cannot take the focus and close at
  once; use real input (`tools/dev/click.ps1`) for those.

## Properties window (owner's go-ahead)
- The bar menu's **Properties** (and Start > Settings > Taskbar...) opens a core-drawn
  Taskbar Properties window (`core::properties`, `properties_window.rs`), 344×400 in
  every look, read from Windows 95 (IE4), 98 and 2000 in the v86 emulator. 95 and 98
  show the identical IE4 window; 95's 1995 original was never found, so WIN95 uses it
  too (flagged; since 2026-11-18 the 95 look's preview is plain 95's from its Reference PC, whose
  window has the same frame and places: "The last pictures from the owner's own Windows"). Pages: Windows' two ("Taskbar Options" / "Start Menu Programs";
  2000: "General" / "Advanced"). Until 2026-11-11 it had a third page of our own,
  "Bevel95" (Look, Size, Start with Windows, which had moved there from the bar menu):
  that is now the Bevel95 Settings window (its section below). OK / Cancel / Apply as in Windows
  (Apply stays on after any change); `bar::apply_properties` saves and applies.
- The preview is put together from five pictures per design cut from the emulator
  (`art/assets/ie4-*.rgb`, `w2k-*.rgb`), in Windows' order: window and taskbar
  (taskbar last when always on top), Start menu, or Auto hide's strip + cyan dots.
- Options: **always_on_top** off = HWND_NOTOPMOST, space still reserved (XP-style,
  flagged); **auto_hide** = no appbar reservation, the bar slides to a 2-row strip
  and back (tick + WM_MOUSEMOVE; SLIDE timer); **small_icons** = `MenuStyle::StartSmall`
  (`start_small` metrics, 16×16 icons from the emulator); **show_clock** off = empty
  clock text, so no well. 2000's Start Menu Settings: Display Favorites and Display
  Logoff work (`config::StartOptions`, `start::entries(era, opts)`); the other six and
  Personalized Menus are grayed out. New settings.txt keys: `always_on_top`,
  `auto_hide`, `small_icons`, `show_clock`, `favorites`, `log_off`.
- Page buttons: Add... = Windows' Create Shortcut wizard (`rundll32 appwiz.cpl,NewLinkHere`)
  into the Programs folder (until 2026-11-11 it did nothing: see "Add..." under "What the
  Programs menu shows");
  Remove... opens Windows' own "Remove Shortcuts/Folders" window (since 2026-11-11: see
  "What the Programs menu shows"; before that it opened the Programs folder in Explorer);
  Advanced... opens the Start Menu folder in Explorer; Re-sort does nothing; Clear =
  `SHAddToRecentDocs(SHARD_PIDL, NULL)` (also clears Explorer's Recent; the checklist warns).
- v86 captures as exact PNGs: a javascript_tool result over the size limit is saved
  to a file by the harness, so `pngOf` base64 strings padded past the limit come back
  as files (`tools/dev/extract-caps.ps1` decodes them). The pane must be visible
  (the canvas only redraws on screenshots). compare-refs reads them from `v86/` in
  the reference folder and skips those cases if missing; 98's captures are mapped
  to standard VGA colors first (its emulator palette is off). Real key presses for
  live tests: `tools/dev/keys.ps1`.

## Notification area (owner's go-ahead; the plan had it out of scope)
- **How icons arrive** (`tray.rs`, RetroBar/ManagedShell's method): a hidden "Shell_TrayWnd"
  (with a "TrayNotifyWnd" child) marked by the window property `Bevel95.StandIn`, kept first
  in FindWindow order (100 ms tick), placed over the bar and the well. Programs are asked with
  a "TaskbarCreated" broadcast at start, and once more 3 s later (while Windows' taskbar settles
  after being hidden it comes first for a moment, and slow programs answered it: seen with
  a chat program and a web browser). Every message is also forwarded to Windows' taskbar (SendMessageTimeout,
  2 s; WM_USER+372 posted), so its toasts keep working and switching back shows the same icons;
  the bar, the watchdog and `--restore` broadcast "TaskbarCreated" after restoring.
  `explorer::main_tray()` skips the marked window, and `restore()` makes Windows' taskbar
  topmost again. `--restore` ends a bar that does not close in 3 s (its stand-in would keep
  taking the taskbar's messages). The owner accepted the trade-off: no injection or hooks, but
  messages meant for Windows' taskbar pass through Bevel95.
- Message layouts as ManagedShell declares them (SHELLTRAYDATA, 32-bit handles,
  sign-extended; WINNOTIFYICONIDENTIFIER for Shell_NotifyIconGetRect, answered with our
  icons' screen rects). Icons are read at once (programs destroy the HICON) at their own size
  and averaged down to 16×16 (`Icon::downscaled`); light single-color symbols (white glyphs
  for Windows 11's dark taskbar) are inverted (`darken_if_light`). Clicks as Explorer sends
  them (version 4: pointer in wParam, (msg, uID) in lParam; NIN_SELECT / WM_CONTEXTMENU after
  the up for version ≥ 3), AllowSetForegroundWindow on the press, no mouse capture.
  Double-clicks are detected by us. Icons whose window is gone are pruned every 2 s.
- On this Windows 11 (25H2) Explorer hands out only its volume (7820ae73) and "Safely Remove
  Hardware" (7820ae78) icons; network and battery do not arrive. Windows' volume icon is drawn
  as the classic speaker (`icons::speaker`, muted from Core Audio); its clicks never reach
  Explorer: click = our volume control (at once since 2026-11-08, see below), double-click =
  `sndvol.exe`, right-click = menu "Open Volume Controls" (bold, sndvol) / "Adjust Audio
  Properties" (mmsys.cpl); tooltip "Volume".
- **Volume control** (`core::volume`, `volume_popup.rs`, `audio.rs`): one layout in 95, 98 and
  2000, each look's colors, exact (Reference PCs); MS Sans Serif in every look
  (`App::shell_font`). Real volume through Core Audio's IAudioEndpointVolume via vtables
  (windows-sys features Win32_Media, Win32_Media_Audio; CoInitializeEx at start). It takes the
  focus like our menus; polls every 250 ms for changes made elsewhere (and closes if another
  window comes to the front). The bar never takes the focus, so a click on the bar closes it
  explicitly (a click on the speaker while it is open only closes it). Steps (2% / 20% / wheel
  6%) are our choice. Where it opens (`volume::place`): 95 and 98 on the screen's bottom edge
  (TrackPopupMenu's rule at the pointer's x there), 2000 at the pointer. The speaker menu
  (`volume::speaker_menu`, `place_menu`): 95 on the bottom edge, 98 and 2000 at the pointer.
  Measuring it showed that a menu whose widest label is bold is 1px wider in MS Sans Serif (the
  bold font's overhang counts in `popup_columns`). Not done: 95/98's "Volume Control" taskbar
  button while it is open (private/todo.md).
- Layout (theme-spec "Notification area and clock"): the clock's part has a fixed width
  (widest time of day + 10, or + 19 in 2000; `widest_time` formats all 1440 minutes) with the
  time centered; icons 17 apart in 95/98 (22 provisional in 2000); new icons join the right end
  (98). Tooltips (`core::tooltip`, `tip.rs`) are exact (98 Reference PC): placement on top of
  the icon, or 2 rows below the well's top for the clock (long date).
- Windows 2000's own menus are now measured (labels 17 in, 1 down, separators inset 1); its
  SP4 Tahoma differs from today's in a few widths, so its menus are not compare-refs cases.
- Live testing without touching the owner's programs: `tools/dev/testicon.ps1` (a WinForms
  NotifyIcon that logs its mouse events) + messages sent to the bar from a DPI-aware
  PowerShell (`FindWindow(class, [NullString]::Value)`: PowerShell turns `$null` into "").
  Injected input never reaches elevated programs (a remote-desktop program's tray runs elevated: a menu opened
  by a test stays until a real click).

## Classic icons, quicker volume control and Start menu (owner's request, 2026-11-08)
- **Classic pictures for today's windows** (`core::window_icons`): Notepad (exe notepad.exe)
  and Explorer's `CabinetWClass` windows get the old icons instead of their own. Explorer's
  folder windows are titled "<place> - File Explorer" (English Windows); the place is matched
  against `Places` (read once by `window_list::places()`: Documents' display name and path,
  This PC and Windows 11's Home via item lists, Recycle Bin, Control Panel's two views):
  Documents = My Documents (98/2000; 95 had none, so the folder), This PC and Home = My
  Computer, Recycle Bin (empty), Control Panel; every other folder (Downloads included, as the
  owner asked) = the folder window's picture. Explorer windows without the suffix are Control
  Panel pages. Other Explorer windows (Run box, copy progress) keep their icons. A button with
  several Explorer windows shows the folder (`ClassicWindow::for_group`). `WindowInfo::classic`
  carries the kind; `App::window_icon` / `group_icon` use it at paint time, so it follows title
  changes. windows-sys feature `Win32_UI_Shell_Common` (item lists) was added for this.
- The 16 pictures were cut from the reference PCs' taskbars (each window opened, its button
  captured raised and pressed; `tools/dev/taskicons.ps1` finds buttons, `tools/dev/cuttask.ps1`
  cuts an icon and proves its see-through pixels against the pressed button's checkerboard):
  captures `private/reference/refs/vm/<era>-truecolor-tasks-<n>.png`, 20 compare-refs cases
  (`task_icon_cases`). 2000's Notepad is 98's pixel for pixel. A folder window shows the OPEN
  folder. 98 uses its 0x9F/0xCF palette, 2000 the 0x9C/0xCE one (real differences).
- Found while matching them: (1) the pressed task button's checkerboard is lined up with the
  task area (the first button), not with each button: a pressed button an odd number of pixels
  further right has it the other way round (`Framebuffer::checker` takes an origin; 19 golden
  images changed). (2) Pressed (bold MS Sans Serif) labels are elided as GDI measures them,
  overhang included: one letter sooner at the boundary ("Untitled - Note..."). (3) In the
  captures, 98's and 2000's Quick Launch band ends a few pixels before the notification area:
  their task areas are 517 and 566 wide (the only widths giving the measured button widths with
  4 to 6 windows); the cases draw our bar with those task areas. (4) 2000 SP4's Tahoma fits one
  letter more before "..." in 91-px buttons; those two label ends are masked (todo item 5).
- **Volume control at once**: the speaker no longer waits a double-click time before opening it
  (Windows did, to tell a click from a double-click; the owner found the wait slow). A second
  click within the double-click time of the opening one is a double-click: the volume control
  closes and Windows' mixer opens (`speaker_double_click`). Measured: the window appears 15 ms
  after the release (was 520 ms), fully drawn within 60 ms. The bar answered every message within
  8 ms over 30 s (`tools/dev/busyprobe.ps1`; `tools/dev/voltiming.ps1` times the opening).
- **Start menu icons read ahead**: `start_menu::read_icons_ahead` reads every Programs and
  Favorites icon on a thread of its own, 3 s after start, at background priority (COM
  initialized there; the shell's image list is per thread, the cache `FILE_ICONS` is shared).
  The owner's 258 icons take about 11 s; the first Programs menu then opens at once (it took
  0.5 to 2 s, logged as "a submenu of N entries took N ms").
- Test helpers live in `tools/dev/` (they used to be in a session's temporary folder): click,
  keys, testicon, extract-caps, volcheck, voltiming, busyprobe, taskicons, cuttask, cutmenu,
  pixels, crop; bestfit writes `core/src/best_fit.rs` from Windows (see "Characters the fonts
  lack" under milestone 2). Driving the reference PCs: Windows 95/98's Run box does not open "::{CLSID}" names, and
  95's splits a path at its first space; open My Computer and the like from the desktop by
  typing their first letter (click an empty desktop spot first; on 2000, Win+D), then Enter.

## Settings and Find pictures, classic folder names (owner's request, 2026-11-08)
- **Settings and Find submenus as the reference PCs show them** (`start::fixed_submenu`, `None` =
  separator; `fixed_menu` adds the icons, `fixed_target` maps an index to what it opens). The PCs
  have no network, so Find has no "Computer..." (dropped; it opened the Network folder). 98:
  Control Panel, Printers, Taskbar & Start Menu..., Folder Options..., separator, Windows
  Update... (no access key). 2000: Control Panel, separator, Network and Dial-up Connections,
  Printers, Taskbar & Start Menu...; Search's "On the &Internet...". Left out for lack of a
  counterpart: 98's Active Desktop submenu, 98's and 2000's People.
- **Files or Folders opens File Explorer's Home** (2026-11-11, the owner: "Find > Files or
  Folders does nothing"). It ran ShellExecute of `search-ms:`: on this Windows 11 that opens
  nothing and reports success (42), so the log had no line. Tried at idle moments from outside
  the bar (`private/probes/find/filesearch*.ps1`, File Explorer's windows and tabs listed before
  and after, only those that came closed again, no titles read): `search-ms:` and
  `explorer.exe search-ms:`, `shell:SearchHomeFolder` (directly and through explorer.exe),
  Search Home's CLSID 9343812e-..., `explorer.exe /e,search-ms:`, Shell.Application's
  `FindFiles()` (Windows' own "Find: All Files" call) and search-ms: addresses without a
  `query=` part: none opened anything. A search-ms: address with `query=` does open a search
  window (`search-ms:query=&crumb=location:C%3A%5CWindows%5CWeb`: "Search Results in Web"),
  but an empty query lists every file under its place, which for a whole drive would be a long
  search of everything. So the entry opens File Explorer's Home (`shell:::{f874310e-...}` by
  ShellExecute, which opened it as a new window here), whose search box ("Search Home") is
  ready for a name; a log line says so each time. Not done: putting the keyboard in that
  search box (Windows 95's Find started in its Named box). The owner confirmed Home as the
  answer on 2026-11-11 ("Yes, good enough").
- **Icons** (`icons::target_icon`): cut from the reference PCs with `tools/dev/cutmenu.ps1`, each
  entry captured selected and not (`private/reference/refs/vm/<era>-truecolor-{settings,find,search}-*.png`).
  Control Panel's is its window's picture (`window_icons`); 98's Taskbar and Files or Folders are
  95's. 23 compare-refs cases (`submenu_cases`): our rows against the capture's rows of the same
  entries, the whole menu where the entries are the same.
- Found while matching: (1) Windows 98 draws every Start menu icon (32×32 and 16×16) on a square
  of BFBFBF, 00007F when selected (`StartMenuMetrics::icon_cell`); GUIdebook's full-color 98
  screenshot has it too, but earlier cases masked icon cells. 4 golden images changed. (2) Windows
  95 sizes its cascading menus by the labels' raw text, the "&" counted (6 px), so Settings and
  Find are 6 px wider (`cascade_counts_ampersands`). (3) Windows 2000 showed the access keys in
  all these captures (the keyboard had been used).
- **Classic names** (`window_icons::classic_title`, applied in `window_list::window_info`):
  Explorer's folder windows lose " - File Explorer"; This PC and Home → "My Computer", Documents →
  "My Documents" (every look), "All Control Panel Items" → "Control Panel" (Control Panel opened
  as control.exe has no suffix at all; opened as a folder it has one); a full path stays a path.
  English Windows only, like the pictures. Live check: own Explorer windows +
  `bevel95 --list-windows`, printing only those names.
- Capturing Start submenus on the PCs: Ctrl+Esc, the entry's letter (2000 SP4's Start menu starts
  with "Set Program Access and Defaults", so "s" selects that first: press it again, then Right),
  then Down for each entry. For a submenu with nothing selected (98, 2000): Esc until all is
  closed, Ctrl+Esc again with the pointer resting on the entry, nudge the pointer 1-3 px, wait.

## Fullscreen apps, administrator windows, screens (owner's reports, 2026-11-08)
- **Each look stays pure**: the owner rejected borrowing Windows 98's My Documents picture for the
  95 look ("Keep it pure"). Never mix one Windows' pictures or behavior into another's look; where
  a Windows had nothing (95's My Documents), use what it did have (the folder).
- **Fullscreen apps** (`fullscreen.rs`, RetroBar's rules from ManagedShell's FullScreenHelper): a
  window counts if Windows reported it (shell hook codes 53/54, HSHELL_FULLSCREENENTER/EXIT, not
  in windows-sys; ITaskbarList2::MarkFullscreenWindow arrives the same way) or if it fills the main
  screen (a framed window by its client area, others by their outline). Never counted: hidden,
  minimized, our own, cloaked, `NonRudeHWND` (the desktop), taskbars, Windows' immersive panels
  without WS_EX_WINDOWEDGE, layered overlays (transparent or no-activate), and (Bevel95's own rule)
  maximized windows with a title bar: with Auto hide they cover the screen, and browsers' client
  area fills it too. The bar stays behind while the window in front is on another screen or
  topmost. Live-tested with a borderless window of our own (Windows reported it; the bar stepped
  behind and back). The Auto hide case cannot be faked (Windows keeps maximized windows out of the
  reserved strip), so it is on the owner's checklist.
- **Administrator windows** (Task Manager): for an ordinary program Windows refuses ShowWindow /
  ShowWindowAsync (returns 0, access denied) and a posted SC_MAXIMIZE, but lets SC_MINIMIZE,
  SC_RESTORE and SC_CLOSE through (measured on an own Task Manager window; SC_MOVE and SC_SIZE not
  tried: if accepted they start a move loop that injected keys cannot end).
  `window_list::show_async` falls back to those commands; refused window-menu commands are logged.
- **Screens**: everything uses the main screen (MonitorFromPoint(0,0)), re-read on every 100 ms
  tick; all of Windows' taskbars (Shell_SecondaryTrayWnd too) stay hidden; the one bar lists the
  windows of every screen, as Windows 98 and 2000 did. Not yet tried with a real second screen
  (owner's checklist, on a laptop).
- The owner confirmed the Windows key opens Bevel95's Start menu.
- `cargo fmt` would reflow about 20 committed files (they predate `rustfmt.toml`'s width); run
  rustfmt only on new files, or do the reflow as a commit of its own.

## Date/Time Properties (owner's choice A, 2026-11-08)
- A double-click on the clock (two clicks within the double-click time, `bar::clock_double_click`)
  opens `datetime_window.rs`, drawn by `core::datetime`. It sets **Bevel95's own clock**: OK/Apply
  store `clock_offset` (seconds ahead of the PC, settings.txt); `bar::clock_now(offset)` feeds the
  bar's clock and its date tooltip. The PC's clock is never set (no admin prompt, no time-sync
  fight). The owner agreed to one bar on the main screen for several screens (98/2000's way).
- The way back is on our own page (owner's "keep going" on Claude's recommendation; since
  2026-11-11 that page is the Bevel95 Settings window): its Clock group, two lines saying how
  far off it is (`properties::clock_lines`,
  rounded to its largest unit) and **Use the PC's Time** (`Part::UsePcTime`, grayed while the
  offset is 0; takes effect on OK/Apply via `Values::clock_offset`). Date/Time Properties'
  Apply updates an open Properties window (`properties_window::clock_offset_changed`). The
  page's groups moved up (8 apart) to make room; 3 golden images changed.
- Right-clicking the clock gives the bar menu with "Adjust Date/Time" and a separator first
  (`bar_menu::menu(clock)`, as Windows 95 to 2000 did; no access key, like our other bar menu
  entries). WM_BAR_COMMAND's wParam is now `BarCommand::code()`, not a menu index.
- The Date & Time page is pixel-exact on all three reference PCs except the hands (masked in the
  three compare-refs cases; `BEVEL95_HANDS=1` unmasks them to measure). Hands, from 181 captures
  of every position on 98 (`private/reference/refs/vm/hands98`; analysis in `private/probes/hands_probe.rs`):
  each hand = gray copy (+2,+2), white copy (-2,-2), teal shape; minute hand first, hour hand on
  top. The second hand is exact: a table of 60 measured end points (`SECOND_TIPS`) and
  Windows' tie rule (`line_pixels`). Hour and minute shapes are a fitted model (~135 px per
  capture off; 95 and 2000 draw the same hands); Windows' fill rule was not found. Time zone name from GetTimeZoneInformation (windows-sys
  feature `Win32_System_Time` added; TIME_ZONE_ID_DAYLIGHT defined locally).
- Driving the reference PCs' Date/Time Properties: open with the Run box `control timedate.cpl`;
  in the time field Up/Down change the part with the caret, Tab moves hour > minutes > seconds
  (Right does not), typed digits replace with checking; while the time is being set the face
  stops; quick key bursts can drop a few presses (read the field back). A capture right after
  the hour changed may still show the old hands: capture a moment later. Captures can catch a
  control mid-redraw (98's "PM" once came out 3px low with leftovers above): retake, never fit
  to such a picture.
- **Time Zone page** (`core::timezone`, `platform/win/src/zones.rs`): each look's own zone list
  (95: 52, 98: 62, 2000: 74 entries, read from the PCs), each entry naming the zone of today's
  Windows (registry key name) that keeps its places' time. The chosen zone only changes
  Bevel95's clock (owner's choice A): settings `clock_zone` (pc or a key from the lists) and
  `clock_dst`; `zones::now()` converts UTC with SystemTimeToTzSpecificLocalTimeEx (daylight
  saving off: DaylightDate zeroed, DynamicDaylightTimeDisabled); `bar::real_now()` is that
  zone's time, `clock_offset` on top. Apply sets the zone first, then the offset only if the
  date or time was changed. The PC's own zone is stored as `pc`. The map is 1-bit assets cut
  from the PCs; 2000 turns 98's map by the label's offset (formula in `timezone::map_shift`).
  The box grays (95/98) or disappears (2000) for zones without daylight saving time this year
  (`zones::has_dst`, GetTimeZoneInformationForYear). 26 compare-refs cases. Live tests that
  apply a zone: back up settings.txt first and restore it, then Restart the bar.
- Typing into the time (`TimeEntry`, `DateTime::type_digit`, `type_am_pm`, `select_time_part`;
  caret blink from GetCaretBlinkTime in `datetime_window.rs`) follows 98, checked by eight
  compare-refs cases `win98-time-*`; 98's year field ignores digits. 95 and 2000 unchecked.
- Windows 98's title bar shading: span ends 5px past the title buttons' edge (fits this
  window at a known position and GUIdebook's Shut Down / Log Off; GUIdebook's Run still masked).
## Start > Documents and quicker menus (owner's reports, 2026-11-09)
- **Documents** (`start::documents_menu`, `documents_entry`, `order_documents`;
  `start_menu::recent_documents(era)`): only shortcuts to files, read from the .lnk header
  (LinkInfo flag 0x2 at 0x14, no directory attribute at 0x18), so Windows 11's app links
  (ms-screenclip, ms-gamingoverlay), Control Panel pages and folders are left out (as on the
  PCs: 98 and 2000 never listed folders; 2000's Open dialog also saved the file's folder in
  Recent, for its History place, and the menu hid it); shortcuts whose file is gone are left
  out too (owner's choice; `known_gone`: the LinkInfo path, checked with GetFileAttributesW
  only on the Windows drive, so a sleeping disk never holds the menu up); the 15 most
  recent by modification time; names via SHGetFileInfoW(SHGFI_DISPLAYNAME |
  SHGFI_USEFILEATTRIBUTES) on the shortcut's own name (follows the user's hide-extensions
  setting; the owner shows extensions). Order and top entries as the reference PCs showed:
  95 by name; 98 "My Documents" + line + creation order; 2000 "My Documents" + line + by name,
  "(Empty)" when none, plus one face row at the bottom (`Menu::extra_bottom`). "My Documents"
  opens FOLDERID_Documents; its icon is the shell's (`icons::my_documents`, cut from the PCs).
- Driving the PCs' Explorer by keyboard is fragile: the Run box does not add documents to
  Recent (98); after Notepad closes, 98's Explorer list loses the keyboard focus (open a fresh
  `explorer c:\` window, Tab once, then type the name); Alt+F4 on a bare desktop opens Shut Down
  and later keys can confirm it (98 rebooted once; harmless in snapshot mode).
- **Menus vanish at the click** (`popup::vanish`): the chosen entry's menus get an empty window
  region (gone at once, the Start button repainted up) but keep the focus while the program or
  folder starts, so its window may still come to the front; then they are destroyed. Real-click
  test: the menu was gone 12 ms after the key, about 220 ms before Control Panel appeared.
## Network and battery icons, one-file install, Stand by (owner's go-ahead, 2026-11-09)
- **Own notification icons** (`core::status`, `platform/win/src/status.rs`): the bar puts two
  icons of its own first in the well, keys `bar::NETWORK_KEY` / `POWER_KEY` (negative "window
  handles", never in `tray`'s list; `paint_bar` chains them before the programs' icons).
  Pictures, tooltip words and menus come from each Windows' own files, copied out of the PCs'
  disks with 7-Zip (`private/tools/7zip` reads qcow2 and FAT/NTFS directly; 95's Dial-Up Networking
  from its CD's cabinets) into `private/reference/refs/files` (never committed), dumped with
  `tools/dev/icons.ps1` and turned into rows (bevel-art's `Rows`, since 2026-11-17) with `tools/dev/icon2rust.ps1`.
  Table and PROVISIONAL points in theme-spec "Network and battery icons". The owner's idea:
  the dial-up picture stands for any connection (no classic Windows had Wi-Fi).
- Connection: `GetBestInterface` towards 8.8.8.8 (routing table only), `GetIfEntry2` counters
  every 200 ms (`NETWORK_TICKS`); a screen lights for 16+ packets per look (`status::ACTIVITY`:
  this PC's background traffic is 0 to 26 packets per quarter second, which would light them
  nearly always; the owner remembered them dark when idle). Offline: `GetIfTable2` for the main
  adapter's name and whether it is a cable. Wi-Fi network name from the Network List Manager
  (vtables: INetworkListManager::GetNetworkConnections, GetAdapterId, GetNetwork, GetName),
  not wlanapi (Windows 11 24H2 gates SSIDs behind the location permission). windows-sys
  features `Win32_NetworkManagement_IpHelper` and `Win32_NetworkManagement_Ndis` added.
- Battery: `GetSystemPowerStatus` every 2 s; the icon only with a battery (flag not 128/255).
- Clicks: network click = `ms-availablenetworks:` (Windows 11's Wi-Fi flyout; live-tested),
  double-click / Status = `ms-settings:network-status`, Open Network and Dial-up Connections =
  `ncpa.cpl`; battery double-click / Open Power Meter = `ms-settings:batterysaver`, Adjust
  Power Properties = `powercfg.cpl`. The battery menu opens where that look's speaker menu does
  (same program in the originals), the network menu at the pointer.
- `bevel95 --list-status` logs what both icons would show (the battery test on a laptop).
- **One-file install** (owner's idea; `install.rs`): on a PC's first run (no settings.txt) a
  plain MessageBox asks to install: Yes copies the exe to %LOCALAPPDATA%\Bevel95, writes the
  default settings (so the installed copy does not ask again), makes the two desktop shortcuts
  if missing (IShellLinkW + IPersistFile vtables, `audio::method`/`release` shared) and starts
  the installed copy. Tested with LOCALAPPDATA pointed at a scratch folder.
- **Stand by**: SetSuspendState needs SeShutdownPrivilege enabled (present but disabled for
  every user; `whoami /priv`); `dialogs::enable_shutdown_privilege` enables it first. Not
  tried (it would sleep the owner's PC); on the owner's checklist.
## Reference PCs (owner's go-ahead)
- Real Windows, installed from the owner's discs (ISO images in `private/reference/iso`; the
  product keys are in the file names, typed into setup and written nowhere
  else), runs in QEMU 11.1 (`private/reference/qemu`: the official Windows build unpacked with
  7-Zip, not installed, so nothing is added to the owner's Windows). No window:
  `tools/vm.ps1` drives each PC through QEMU's control port (`Start-Vm`,
  `Send-Keys`, `Send-Text`, `Save-Screen` = exact PNG of the screen, without the
  hardware mouse pointer, `Wait-Pixel`, `Set-Cd`, `Stop-Vm`). Disks are
  `private/reference/vm/*.qcow2`; `Start-Vm` without `-Keep` runs with `-snapshot`, so
  reference runs never change them.
- Discs used: Windows 95 "C" (OSR 2.5, full OEM; the Windows inside is from
  August 1996, and its Internet Explorer 4.01 step never ran, so it has the plain
  95 shell), Windows 98 first edition (OEM), Windows 2000 Professional SP4 (OEM).
  The owner's 1995 retail disc is an upgrade edition whose setup wants Windows
  3.x; that check is not worked around. 98 SE is kept for later.
- All three run 800×600 True Color (32 bit), so captures are exact (gray
  #C0C0C0; desktop #008080, 2000's #3A6EA5). The drivers on the Windows discs reach
  High Color at most in QEMU, so the PCs use QEMU's standard VGA with drivers made
  for emulators, as v86's and the Computernewb wiki's guides do: VMDisp9x for 95/98
  (open source, MIT), VBEMP for 2000 (bearwindows, free for non-commercial use),
  from `private/reference/vm/drivers` (copies of `private/downloads`; VMDisp9x matched GitHub's
  checksum, VBEMP's site publishes none).
  They only show the picture: Windows' own code (the 9x DIB engine, 2000's
  framebuf.dll) draws every pixel. Each disk keeps a snapshot `highcolor-cirrus`
  of the earlier Cirrus/16-bit state (`qemu-img snapshot -a`, with `Vga = 'cirrus'`):
  exact references for a possible later High Color mode. True Color captures are in
  `private/reference/refs/vm` (desktops, Shut Down dialogs: their dimming is a black checkerboard; the
  notification area, tooltips, the speaker, volume control and speaker menu of all three,
  2000's menus). A snapshot `truecolor` keeps each disk's state before the notification-area
  work: since then 98 shows its display settings icon, and all three have a Sound Blaster 16
  driver with its volume icon (start them with `Start-Vm <pc> -Sound`: `-audiodev none` plus
  `-device sb16`). 98 also keeps `before-sound`.
- Sound: 2000's Add New Hardware wizard detected the card ("Sound Blaster 16 or AWE32 or
  compatible (WDM)"); the volume icon appeared after a restart. 95 got it through its wizard
  too. On 98 the wizard's Plug and Play search blanks the screen, which VMDisp9x does not
  survive (striped text screen; it looked frozen, Ctrl+Alt+Del showed nothing). Done with the
  display first switched to "Standard Display Adapter (VGA)" (Device Manager > Update Driver >
  list > Show all hardware > Standard display types), then the wizard with detection (Yes: 98
  does not offer the SB16 in its manual list), then VMDisp9x back (JHRobotics > QEMU STD VGA
  PCI Adapter, files from A:, `-FloppyDir private\reference\vm\drivers\vmdisp9x`), then 800×600 True Color
  in Display Properties (applies without a restart). `sndvol32 /t` (Run) opens the volume
  control directly. 95's and 98's take a few seconds to open the first time.
  2000's Shut Down box remembers the last choice (Restart once chosen); on `isapc` it ends at
  "It is now safe to turn off your computer" (then `Stop-Vm`). Its Administrator logs on with
  an empty password.
- Quirks, each found the hard way:
  - 95: the fast-CPU bug ("While initializing device IOS: Windows protection
    error", ESDI_506.PDR's timing loop) hits at random boots. ESDI_506.PDR is
    renamed ESDI_506.PD_ (disk in MS-DOS compatibility mode, no CD inside
    Windows). `-accel tcg,one-insn-per-tb=on` (kept) did not avoid it alone;
    `-icount` ran about 75 times slower than real time. With a floppy inserted
    the IOS error came back (twice in a row), so start 95 without one. After a
    failed start Windows picks Safe Mode: shut down from there, then start again.
    (The usual cure is a patch tool, patcher9x or FIX95CPU; not applied.)
  - 98: the same bug in NDIS.VXD, so the Dial-Up Adapter is removed (no
    networking, hence no logon prompt). Its CPU has no local APIC: after 98's own
    restarts the APIC came back switched off and QEMU withheld the timer from
    DOS-time code (frozen boot menus, endless waits). Its Restart can still hang
    at "Windows is now restarting...": shut down and start instead. 98 never set
    up the PCI bus (QEMU's PnP BIOS is misdetected, a known issue), so a new
    display driver goes onto the old "Cirrus Logic" entry (Device Manager,
    Update Driver, list, Have Disk); 95 the same through Advanced Properties.
  - 2000: a display change reverts unless confirmed within 15 seconds.
  - 2000: on the normal PC, GUI setup always died at "Completing configuration of
    Distributed Transaction Coordinator" (Windows 2000's IDE driver racing its DMA
    interrupts on fast virtual hardware; the CPU model, clock date and network
    card made no difference; an AMD PCscsi disk hung SeaBIOS). It runs on
    `isapc` (no PCI, so no bus-master DMA) with `-win2k-hack` and a CPU without
    APIC (with one, setup picks an APIC HAL that never gets a timer on isapc).
    Unattended setup from `private/reference/vm/win2000-floppy/WINNT.SIF` on a vvfat floppy;
    AutoPartition does nothing on an empty disk (press Enter twice).
  - Installing 95 and 98: boot the 98 CD (its "Boot from CD-ROM" menu times out
    in 5 s; `Wait-Pixel` waits for the menu's highlight bar). 95's setup folder is
    copied to C:\WIN95 from a second CD drive (swapping the boot CD makes drive
    A: vanish). 95's ScanDisk refuses the 98 boot disk's DOS 7.1 (Esc goes on).
  - Keys are held 200 ms (the slowed 95 misses 100 ms presses). The Run box
    autocompletes from its history: type the whole command, then Delete, Enter.
    Ctrl+Shift+Tab does not arrive (repeat Ctrl+Tab); Enter in a tree view presses
    the dialog's default button (Tab to the Properties button instead).
  - Relative pointer moves can overshoot (95 was 6 px off; one long move on 98 hit
    the corner). For an exact spot: push the pointer into the bottom-right corner
    (799, 599), then `Move-MouseBy` back with `-Step 2`, and read where the tip
    landed from a capture (VMDisp9x draws the pointer into the picture).

- **Windows 98's optional parts, 2026-11-09** (the owner: "make sure we have all of the standard
  apps and as much as it will let us install from the disc", "be careful"). Snapshot
  `before-all-parts` first (`qemu-img snapshot -a before-all-parts` on win98.qcow2 goes back).
  Then `Start-Vm win98 -Keep -Sound -Cd <its disc>`, Run `control appwiz.cpl,,2` (Add/Remove
  Programs, Windows Setup page), by keyboard: Space on a component switches between none and all
  of it (a shaded box first goes empty, the next Space fills it; the line "x of y components
  selected" says where it stands), Alt+D opens its Details. Installed in full: Accessibility,
  Accessories (Games, Paint, WordPad, Imaging, screen savers...), Desktop Themes, Multilanguage
  Support, Multimedia, Online Services, System Tools; Internet Tools without Web-Based Enterprise
  Mgmt; Communications got HyperTerminal and Microsoft Chat added. **Left out on purpose**:
  Dial-Up Server, Direct Cable Connection, Virtual Private Networking (each adds a network
  adapter, and with one NDIS.VXD crashes this PC at start: the quirk above), WebTV for Windows
  (new hardware drivers and a hardware search, which the display driver does not survive) and
  WBEM (a background service, no window). 61.7 MB copied from the disc without a question; to
  "restart now?" the answer was No, then Start > Shut Down (98 switched the PC off itself),
  started again: "Updating System Settings" ran twice for about two minutes, then the desktop,
  800×600 True Color, the speaker icon there. New on the desktop: My Briefcase (older 98 desktop
  captures show six icons, later ones seven). Start > Programs > Accessories now has
  Accessibility, Communications, Entertainment, Games (FreeCell, Hearts, Minesweeper, Solitaire)
  and System Tools. Captured: the real Minesweeper in the 17 states of 95's set
  (`win98-truecolor-mine-*`) and the Accessories and Games menus (`win98-truecolor-accessories*`).
  Nine comparisons against it were added (its window in front, behind, a title chosen, a menu
  open; its Game, Help and window menus): 0 differing pixels, 190 cases in all. Windows 95 is
  still to do the same way (todo).

## Review fixes (2026-11-09)
- An outside review (the owner's go-ahead for all of it): `bar::with_app` logs a skipped
  handler when the shared state is busy, with the caller's file and line (`#[track_caller]`),
  the first 20 and every 100th. None were seen in the first minutes after the change.
- The Windows key's held-back release is sent by the keyboard hook's own thread (the hook posts
  itself `WM_RELEASE_WIN` and its message loop calls `SendInput`), no longer by the bar, so a busy
  bar cannot leave the key logically down (a letter typed then became a Win+letter shortcut).
  Not testable with injected keys; on the owner's checklist.
- Shell_NotifyIcon messages (WM_COPYDATA kind 1) are passed on to Windows' taskbar by a thread
  of their own ("tray forwarder", copies of the data, in order, at most 500 waiting, then
  dropped with one log line); our answer to the program never depended on Explorer's. Other
  messages to the stand-in (appbar questions, WM_USER ones) still wait for Explorer's answer,
  which they return. Programs still wait for Bevel95's own answer, as they wait for Explorer's
  without Bevel95. Logged once: "icon messages reach Windows' taskbar".
- CLAUDE.md was split: rules and pointers there, this file for the history (about 15,000
  tokens per session before).
- Next, in this order: the shared control kit in `core` with the three dialog windows ported
  onto it, then a setup window (install or portable, update, remove). No code signing.
## The control kit (2026-11-09, the review's first move)
- `core::kit`: `Form` (what a core-drawn window with controls says about itself: size, layout,
  paint, parts, focus, tab stops, Enter/Esc buttons, and its own key, character, access-key,
  Space, press/release, drag, hover, wheel, paste and caret rules, each event offered to it
  first) and `Controller` (the dialog manager's shared rules: Tab/Shift+Tab, Enter, Esc, access
  keys shown from the first key, Alt+F4, a held button drawn pushed in while the pointer is on
  it and carried out when let go on it, grabbed thumbs, the title bar's rows 3..21 moving the
  window). Its output is `Effect`s (Activate, Capture, Release, MoveWindow, Close, Paste).
- The three windows are kit forms in the core: `dialog::DialogWindow` (Run keeps its history
  and the character widths for clicks in the command), `properties::PropertiesWindow`,
  `datetime::DateTimeWindow`; their old Windows-side key and mouse code moved there unchanged in
  behavior, now tested without Windows.
- `platform/win/src/kit.rs`: one window class (`Bevel95Window`, `kit_proc`) for all of them;
  each window kind implements `Hosted` (its form, `activate` for buttons, its own timers,
  `closed`). Each window's state lives in the kit's list, not in the bar's `App` (the review's
  third point): fonts are copied from the bar when it opens (`retheme` after a look change).
  While a window handles a message its state is lifted out of the list; Win32 calls that can
  call us back (capture, the move loop, buttons' actions, paste) run after it is back.
  `kit::with` / `update` / `close` / `hwnd` find a window by its kind.
- Proof: `tools/dev/dialogcaps.ps1` photographs Run, Log Off, Shut Down, the three Properties
  pages and both Date/Time pages by messages to the bar (only Bevel95's own windows are ever
  closed or sent keys; an early version closed the Claude app's window when a dialog had not
  come up, and was fixed), `tools/dev/imgdiff.ps1` compares: identical before and after, apart
  from the running clock. Real-input checks of typing, Tab, Esc, arrows, Space, a title-bar drag
  and a click on Cancel. The three window files went from 2,166 lines to 740 (plus the host).
- Drawing helpers stay where they were (`dialog.rs`: frame, title bar, push button, well,
  drop-down, focus dots; `properties.rs`: check box, tab, scroll arrow, now the one group box
  that Date/Time uses too). A public drawing module waits for the first separate program.
- The new skip log showed the bar's paint skipped a few times right on the minute (retried by
  `WM_REPAINT`, so harmless); its cause is not known yet (todo).
## Bevel95 Setup (2026-11-09, the owner's design: "some options when first installing")
- Bevel95's file and desktop shortcuts use its silver beveled B logo. Minesweeper's file
  uses the Windows 95 mine and the shared viewer uses the Windows 95 Help icon.
  Each executable carries its own icon, built by
  `tools/build/program_icon.rs`. The game's title bar and Start menu entry use the artwork
  for the current Windows look.
- `core::setup` (a kit form, drawn from the 95 look's parts: Shut Down's option buttons,
  Properties' check box, push buttons; no classic Windows had it, PROVISIONAL) and
  `platform/win/src/install.rs` (`setup(forced)`, before the bar starts; `kit::open_with` with
  its own fonts and `kit::run_until_closed`, since the bar's loop is not running yet).
- When it shows: a copy that is neither the installed one (`%LOCALAPPDATA%\Bevel95\bevel95.exe`)
  nor portable (a settings.txt next to it). First run: Install (copy, default settings, the
  missing desktop shortcuts, optionally Start with Windows for the installed path:
  `autostart::set_on_for`) or Portable (settings.txt next to the file; `log::data_dir` then uses
  that folder for everything). Installed elsewhere: Update (newer/older by file date; closes a
  running bar with `--restore`, replaces the file with retries, starts it), Just run this copy
  (since 2026-11-17 only a copy as built, see "Two gaps" below), or Remove. Remove checks that the companion programs can be removed, closes the installed
  bar, clears Start with Windows, then removes the installed programs, state and matching
  shortcuts. The state is a fixed list of data files (`DATA_FILES`; programs.txt and
  programs.old.txt joined it on 2026-11-11, a test now checks it against every data file
  named in the source). It removes directories when empty. When invoked from the installed executable
  (`bevel95 --setup`), a helper waits for that executable to exit before removing it.
  A bundle matching the installed files starts the installed copy. A PC set up before Setup
  existed (settings.txt but no installed program) is not asked; `BEVEL95_NO_SETUP` (set by
  `cargo selftest`) skips it.
- `log::installed_dir` no longer creates the folder; `data_dir` creates it when it returns it.
  A first run logs nothing until it has a place, so Cancel and Portable leave nothing in
  AppData. The bar's DPI call tolerates Setup having set it already.
- Tested with LOCALAPPDATA pointed at scratch folders: Install, Portable, another newer copy,
  the installed copy's `--setup`, Remove up to its question (answered No). Update and Remove's
  actions were not run here: both close the running bar, which was the owner's.
- **The one file (2026-11-17, the owner: "I like the single exe unpacking installer idea.
  Registry key for installed app is fine.")**. Since Bevel95 became six programs, a copy for
  another PC was the bar plus a `programs` folder; a stranger now downloads ONE file.
  - **The design: the released bar carries the other five, appended to its own file**
    (`core::package`). Windows runs the front part, the bar exactly as built, and ignores what
    follows; after the five programs come an index (`file <name> <offset> <length> <sha-256>`
    per program, the bar first at offset 0) and a 32-byte trailer (`Bevel95 package\0`, the
    index's offset and length). Chosen over a separate small Setup program because Setup
    already lives in the bar (window, shortcuts, Run entry, Remove): a second program would have
    had to carry all of that, or a copy of it. Nothing changes for development: the bar as built
    is no package, `tools\install.ps1` installs as before, and a copy as built still takes the
    companions it finds beside it. The installed copy is the plain bar again (the front part
    only), so an installed copy looks exactly as before: `%LOCALAPPDATA%\Bevel95\bevel95.exe`
    with `programs\`. No compression (it would need a crate or a hand-written decoder): 0.1.0's
    one file is 14.5 MB.
  - **Made by `tools\release.ps1`**: a release build, then `core/examples/pack.rs` (the same
    `package::pack` and `unpack` the tests use; it reads its result back before writing), into
    `private\dist\<build>\Bevel95-<version>.exe`; it prints the size and SHA-256 and stops rather
    than replace a file already there. The name has no "setup" or "install" in it.
  - **Fingerprints**: Setup reads its own file; `package::unpack` checks every program against
    its SHA-256 (in plain Rust, `package::sha256`, known answers in the tests) before anything
    is shown or written. A damaged or cut-short download says so ("Download it again. Nothing
    was changed.") and stops. Names are only `bevel95.exe` and `programs/<a-z0-9->.exe`.
  - **Nothing half-done** (`Bundle`, `stage_all`, `Staged`): every file is first written beside
    its place under a temporary name (`<file>.<pid>.tmp`) and read back; only when all are
    written are they renamed into place. A failure while writing removes every temporary file
    and leaves the old copy as it was. Update writes first, then closes the running bar, then
    renames. Nothing goes to a temporary folder.
  - **A running companion** cannot be replaced (its file is in use): it keeps its old version,
    as `tools\install.ps1` does, and Setup now says so in a message naming each one ("...
    which were open and were left as they were: Solitaire. Close them and run this Setup again
    to update them too."). Better was considered (renaming the running file away and putting
    the new one in its place), but it leaves an old file to clean up later; not built.
  - **Newer or older**: a downloaded file's date is the day of the download, so every program
    now carries a **build stamp** in its file (`bevel_winkit::STAMP`: `\0Bevel95 build stamp
    <version> <commit> <commit time>\0`, the commit's time from git in `winkit/build.rs`).
    Setup compares the stamps' times; a copy built before the stamp (or two builds of one
    commit) falls back to the files' dates as before.
  - **Portable** from the one file unpacks `bevel95.exe` and `programs\` beside it with
    settings.txt (the companions find a portable folder by its `bevel95.exe`), and starts that
    bar. A copy as built is portable where it is, as before.
  - **The installed-apps entry** (the owner's second approved registry entry):
    `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Bevel95` with DisplayName and
    Publisher (both `about::NAME`; the publisher PROVISIONAL, the owner decides),
    DisplayVersion (`about::VERSION`), DisplayIcon (the installed bar, its silver B),
    InstallLocation, UninstallString (`"<installed bevel95.exe>" --uninstall`), EstimatedSize
    (the six files, in KB), NoModify and NoRepair. Written by Install and by Update (version and
    size refreshed), never by Portable or `tools\install.ps1`; Remove takes it away when its
    InstallLocation is the folder being removed. `bevel95 --uninstall` is Remove with its
    question, as Windows' Uninstall button runs it. The registry helpers are `autostart.rs`'s.
  - **The questions** (`core::setup`, second page, only on the Install path): the look (three
    option buttons in a "Look" group), "Show the classic desktop" (ticked), "The Windows key
    opens" (Quick Search, chosen; or The Start menu), "Start Bevel95 when Windows starts" (not
    ticked, as before). The owner's defaults are `config::Config::default()`'s. The first page's
    OK reads **Next >** while Install is chosen; the second has **< Back**, **Install**,
    **Cancel**, Back and Install touching as in Windows' wizards. Choosing a look draws the
    window in it at once (`SetupHost::activate`, `kit::retheme_with`), and Back keeps the
    answers. The answers go into settings.txt (`Answers::settings`, `config::with_setting`);
    an update asks nothing and keeps the settings. Setup over an installed copy is drawn in its
    look and colors; a first run in 95, True Color. Access keys: 9**5**, 9**8**, **2**000, the
    others by letter; digits count as access keys here. PROVISIONAL: the page's words and layout.
  - **Testing without the owner's installation**: `BEVEL95_SETUP_TEST=<a name with "test">`
    puts Start with Windows under `HKCU\Software\<name>\Run`, the entry under
    `...\<name>\Uninstall\Bevel95`, and the desktop shortcuts in a `Desktop` folder beside the
    installed one; with `LOCALAPPDATA` pointed at a scratch folder outside the real one nothing
    of the owner's is touched. Any other value stops the program before it writes anything.
    `private\probes\installer\live.ps1` starts Setup so, posts keys to its window only, answers its
    message boxes, photographs it with PrintWindow and lists the files (with their fingerprints),
    settings, the scratch key and the log. A second bar cannot run beside the owner's (the bar's
    mutex): the scratch bar Setup starts writes its start line and exits, which shows that it
    started.
  - Proof: `cargo test --workspace` 466 passed, 0 failed, 6 ignored; clippy clean. Unit tests:
    `package` (SHA-256 known answers, pack and unpack, a damaged or cut file, names, the stamp),
    `setup` (the two pages, Tab order, looks by key and click, Back, the answers in settings.txt,
    an update in 2000's look), `install` (the one file into the installed layout, a running
    companion left, a failure leaving everything as it was and no temporary file, a damaged file,
    the entry's values, newer by stamp). Golden pictures new: `<look>-setup-first-run`,
    `-setup-questions`, `-setup-update` and `<look>-16-` of each (18); none changed.
    `cargo compare-refs`: the full run after the install passed, no differing pixel (no case was added or changed;
    the one skipped for a missing capture stays skipped).
  - Tried on the scratch place (`private\probes\installer`, two releases 6366821 and 09cb554 from
    `tools\release.ps1`, 14.5 MB each): Install with answers 98 / no desktop / Start menu / Start
    with Windows (six files with the release's fingerprints, settings.txt with the answers, the
    shortcuts, the scratch Run value and entry, the scratch bar started and stopped at the mutex,
    no `.tmp`, the real entry and Run value untouched); the same file again (started the installed
    one, no window); the newer file while the scratch Solitaire ran (five updated, Solitaire left
    and named in the message); the newer file again after closing it (Solitaire updated); the older
    file (offered "Replace ... with this older copy", in 95, 98 and 2000, True Color and 16 Colors,
    cancelled); `bevel95.exe --uninstall` (question, Yes: folder, shortcuts, Run value and entry
    gone); Portable from the newer file on an empty place (bevel95.exe, programs\, settings.txt
    beside it, its bar started; no installed folder). Photographs in `private\probes\installer\shots`.
    Not tried: a real first run on a clean PC, Windows' own list of installed apps and its
    Uninstall button, SmartScreen on a downloaded file, the real Install, Update or Remove of the
    owner's copy.
- **The one file never runs as a bar "from where it is"** (an outside review of 2026-11-18 found
  it: a failed Install answered "It runs from where it is" and ran this copy, which for the one
  file is a bar without its programs). Now, for the one file (`Bundle::packed`), every way out of
  Setup that would run this copy as the bar instead says what failed and ends
  (`install::not_started_here`): a failed Install's copy ("... could not copy itself to <folder>
  (<error>). Nothing was changed."), Setup's window that could not be shown ("Bevel95 Setup could
  not be shown. Nothing was changed."), the installed copy that could not be started when the
  one file is identical to it ("could not start its installed copy ... Nothing was changed."),
  and after a successful Install, Update or Portable whose bar could not be started ("... was
  installed (updated) in <folder> but could not be started. Start it with the Bevel95 shortcut
  on your desktop."; Portable: "Start bevel95.exe in this folder."). Portable's failure message
  now also says "Nothing was changed." (true: everything written is thrown away). "Just run this
  copy" stays offered only to a copy as built and runs only one (`RunThisCopy` refuses a packed
  bundle too). Two more ways were found and closed: **a portable folder** (Bevel95's settings.txt
  beside the file) and **a PC set up before Setup existed** let the one file run as the bar
  without asking Setup at all, with or without its programs beside it; now that happens only
  when all its programs are beside it (`runs_without_setup`: `is_one_file` reads only the file's
  last 32 bytes, `programs_beside` as the running bar finds them), else Setup shows (it can unpack
  them). A copy as built, with its programs beside it, keeps today's behaviour in every case
  ("It runs from where it is."). All messages PROVISIONAL wording. Test:
  `install::tests::the_one_file_never_runs_as_a_bar_after_a_failed_setup` (a scratch folder under
  `private\probes\codex2\unit` whose writing is denied by icacls; the one file: false, "Nothing was
  changed.", nothing written; a copy as built: true, as before; `runs_without_setup` for the one
  file alone, with its programs, and a copy as built). Tried live in Setup's test mode
  (`private/probes/codex2/setup/live.ps1`, LOCALAPPDATA and the registry key scratch, a one file made
  by `core/examples/pack.rs` from this build, keys posted to Setup's own window): the scratch
  installed folder's writing denied, Install: the message (photographed `shots/failed-install.png`),
  OK, the process ended, nothing written, no registry value, no `.tmp` left; the one file alone in
  a scratch portable folder: Setup's first page showed (before: a bar), Esc, nothing written. Not
  tried live: the start failures (Windows refusing to start the installed bar), Setup's window not
  showing.
- **Two gaps an outside review found (2026-11-17)**, both real, both closed:
  - **"Just run this copy" from the one file started a bar without its programs** (its five
    companions are only inside the file). Now **the one file does not offer it**: over an installed
    copy it shows Update (or "Replace ... with this older copy") and Remove
    (`SetupKind::OtherCopy { newer, can_run }`, `can_run` false for a packed copy:
    `install::other_copy`). A copy as built, with its programs beside it, keeps the third choice.
    Golden pictures new: `<look>-setup-update-one-file` and `<look>-16-setup-update-one-file` (6);
    no Setup picture changed.
  - **A download that stopped early installed the bar alone and said it worked**: a cut file has no
    trailer, so `package::entries` calls it "no package" (that stays so) and it read as a copy as
    built with nothing beside it. Now **a copy without all its programs sets nothing up**
    (`Bundle::complete`, `install::refused`): a copy that is not the installed one and lacks any of
    the five companions (from the package, or beside a copy as built) shows one message before any
    question (`setup::INCOMPLETE`, PROVISIONAL: "This copy of Bevel95 is incomplete: its other
    programs are missing. A download that stopped early looks like this. Download it again. Nothing
    was changed.") and ends: no install, no unpacking, no bar. The installed copy's own `--setup`
    is never refused; a portable copy already set up never reaches Setup. Decided beside the brief:
    a lone copy of the installed bar elsewhere (bar bytes equal to the installed one's) still starts
    the installed copy, as before, since that check comes first and changes nothing. An old package
    without Solitaire would now be refused too (all five are needed).
  - Proof: the tests `setup::tests::the_one_file_offers_update_or_remove_only` and
    `install::tests::setup_refuses_a_cut_one_file` (cuts in the trailer, the index, a program and at
    the bar's end; a package of four; a copy as built with 0 to 4 programs beside it; the installed
    copy) failed against the old behaviour and pass now. `cargo test --workspace` 476 passed, 0
    failed, 6 ignored; clippy clean. Tried in the test mode (`private\probes\setup-gaps\live.ps1`, a copy
    of the installer probe with its own scratch place): a file of this build with its last 4 KB cut
    off on an empty place (the message; nothing written: no folder, no log, no shortcut, no scratch
    key); 0f2ef9e's one file installed there; this build's one file over it (two choices, photographed;
    Down, Down, J: Remove chosen, J did nothing; Esc); the cut file over it (the message; only the
    log line "this copy is incomplete" was added). Photographs in `private\probes\setup-gaps\shots`.
- **The real Remove and Install, tried once on the owner's own copy (2026-11-17**, the owner: "You can
  totally wipe and install using our installer if you want."; for this test only). The release was
  6e87411 (Parts A and B of that day): `private\dist\6e87411\Bevel95-0.1.0.exe`, 15,234,737 bytes,
  SHA-256 d8c590e0...0c3e43. Everything of Bevel95 was started through Explorer's own process (the
  desktop's ShellExecute, as `tools\start.ps1`), nothing from a tool; Setup's windows were driven by
  posted messages and photographed alone (`private\probes\real-install\shots`); no private copy existed
  after any step.
  1. Saved first: the whole installed folder to `private\backup\install-20261117-real` (17 files, each
     compared). Noted: 2000 look, scale 2, True Color, classic desktop on, grouping on, Windows key
     Quick Search (the default), Programs page "classic", no `desktop.txt` (no icon moved), no Run
     value and no StartupApproved note (Start with Windows off), no installed-apps entry (it was
     installed by `tools\install.ps1`), both desktop shortcuts, the bar and the desktop showing.
  2. `tools\release.ps1` from 6e87411.
  3. `bevel95.exe --uninstall` (the installed copy): its question, Yes; "Close this message to finish
     removing Bevel95."; OK. Gone: the six programs, the `programs` folder, every data file, both
     shortcuts; no Run value, no entry; the bar and its watchdog ended; Windows' taskbar showing
     (explorer's Shell_TrayWnd visible). **Left behind: the folder with one file, `bevel95.log`
     (97 bytes), the watchdog's line "the bar ended (exit code 0); Windows taskbar restored",
     written 2 ms after Remove deleted the log** (23:03:06.291), so the folder was not empty and
     stayed. A fault of Setup, not repaired here (todo.md, "Housekeeping": Remove should wait for
     the watchdog).
  4. The one file through Explorer: first page (Install chosen), Next >, Windows 2000 (the window
     turned 2000), classic desktop ticked, Quick Search, Start with Windows not ticked (as before),
     Install. The six programs match the release's fingerprints; settings.txt has the answers;
     both shortcuts point at the installed bar; the entry holds DisplayName and Publisher Bevel95,
     DisplayVersion 0.1.0, DisplayIcon `...\bevel95.exe,0`, InstallLocation, UninstallString
     `"...\Bevel95\bevel95.exe" --uninstall`, NoModify 1, NoRepair 1, EstimatedSize 14877 (the six
     files' KB, measured the same); no Run value; the log's start clean (no failure line). **The
     bar's parent is Setup** (which then ended), not explorer.exe: Setup starts the installed bar
     itself. It is still an ordinary program of the owner's (Setup was Explorer's child; no private
     copy). Step 6's start through `tools\start.ps1` gave explorer.exe again.
  5. The same file again: no window; "this copy is the installed one; starting that"; that bar met
     the running one's mutex and exited. The entry's UninstallString is the command step 3 ran.
  6. The bar closed by the "Restore Windows taskbar" shortcut (opened through Explorer); from the
     backup back: settings.txt, programs.txt, task-order.txt, folders.txt, calculator.txt,
     minefield.txt, solitaire.txt, run-history.txt, windows-taskbar-state.txt (each compared); the
     old logs stay in the backup. Started with `tools\start.ps1`: parent explorer.exe; the log's
     start line, the bar's place and the classic icons' places are the same as before; settings.txt
     byte for byte the saved one; still no `desktop.txt` and no Run value.
  7. `tools\install.ps1` over it: every program already equal to the build (nothing replaced), the
     bar restarted (parent explorer.exe), the entry kept with its values.
  - Bevel95's bar was away from 23:03:06 (Remove closed it) to 23:05:18 (Install started it): 2 min
    12 s; again for a few seconds in steps 6 and 7.
  - Not tried: Update on the owner's copy; Windows' own list of installed apps and its Uninstall
    button (the owner's to look at); an install into a missing folder on the real place (Remove had
    left the folder); SmartScreen.
- **Only Bevel95's own settings.txt makes a folder portable (2026-11-18**, found by the safety
  review of 2026-11-18, A3). Any file named settings.txt beside bevel95.exe meant "portable,
  already set up": a download into a folder holding another program's settings.txt skipped Setup
  and the incomplete-copy check, and the bar read that file, replaced it with its defaults if it
  was not UTF-8, or added its own lines to it at the first save.
  - **The mark**: a line that starts with `# Bevel95 settings` (case and leading spaces aside;
    `log::SETTINGS_MARK`). It is the first line of `config::DEFAULT_FILE` since settings.txt
    first existed (every version of config.rs in git has it), and `with_setting` keeps every
    line it does not change, so every settings.txt Bevel95 has written counts, also one saved by
    Notepad with a byte-order mark or Windows' line ends. A file that cannot be read as UTF-8 is
    not marked. Someone who deletes that line from a portable copy's settings.txt gets Setup's
    first page again (README says to keep it).
  - **Where a copy keeps its data is decided once, at a program's start, and then told** (the
    fourth safety review of 2026-11-18, B1, B2, C2; it replaces two rules of the third review's
    that still read settings.txt at the moment of asking: a "not portable" answer was never kept,
    so a companion started while the file was held used the installed folder and then saved its
    whole picture (best times, window places, the Undo list) into the portable one once the file
    was free; every log line and save waited a second while it was held; and a portable copy
    started then ran as the installed copy or offered Update). Every program's first step, before
    anything is read or logged (`log::decide_at_start`; the companions' `log::program_start`),
    decides its data folder for the whole run (`log::Home`: the installed folder, a portable
    folder, or not known); nothing asks again (`log::home`, `data_dir`). How it decides
    (`log::home_for`):
    - the installed copy's programs (`%LOCALAPPDATA%\Bevel95\` and its `programs\`): the installed
      folder, without reading anything;
    - **told**: a program started by a portable copy's program carries the plain flag
      `--portable-copy` (`log::PORTABLE_FLAG`, no path), taken out of its arguments first of all:
      its data is beside the copy's bevel95.exe (the folder above `programs\`, or its own folder
      for a flat copy), without reading settings.txt. The bar's internal jobs (`--after`,
      `--watchdog-launch`, `--watchdog`) without the flag are an installed or not-portable bar's:
      the installed folder, without reading. An installed copy passes no flag;
    - **a start by the person** (a double click, a shortcut, Windows' start, Run, a folder window)
      reads the mark: Bevel95's own settings.txt beside the program, then (for a program in
      `programs\` whose folder above holds bevel95.exe) beside the copy's bevel95.exe, makes the
      copy portable; none, or another program's, means the installed folder. **Only one that
      another program holds just now is not known** (a sharing or lock violation, error 32 or
      33, still so after the short wait: `log::settings_owner`, tried every 50 ms for up to a
      second; the fifth safety review of 2026-11-18, B2: every error but "not found" counted, so
      a settings.txt that could never be read, access denied or damaged, made the copy unable to
      start for good, with a message blaming a program that did not exist). Any other failure to
      read it (`log::Owner::Unreadable`, at once, no wait) counts as another program's file: not
      portable, the installed folder, Setup shows as on a first run; Setup's **Portable** refuses
      with `setup::SETTINGS_UNREADABLE` (PROVISIONAL: "This folder holds a settings.txt that
      cannot be read, so Bevel95 cannot tell whether it is its own and cannot keep its settings
      here. Put Bevel95 into a folder of its own and start it there, or choose Install. Nothing
      was changed."); Install works and never touches it. **A companion started by the person
      never runs as the installed copy beside a settings.txt that could not be read** (the sixth
      safety review of 2026-11-18, C2: a portable copy's file that could not be read for a moment,
      a waking USB disk, a share reconnecting, made a game or a folder window keep its data in the
      installed folder for that run, without a word): a folder window, game, the Calculator or
      Help started by the person (not told by a bar), whose settings.txt beside it or beside the
      copy's bevel95.exe is there but could not be read, for whatever reason, shows the same
      `setup::SETTINGS_UNREADABLE` message and ends, writing nothing (`log::companion_home`,
      `program_start`: decided "not known", so nothing is written, the log included). The bar is
      not affected (Setup shows), nor a start told by its bar, nor the installed copy (neither
      reads the file). Test: `log::tests::a_companion_beside_a_settings_file_that_cannot_be_read_does_not_run`
      (a real access denied with icacls on a scratch file). Tests:
      `log::tests::only_a_file_in_use_just_now_is_not_known` (by error code; a real lock
      violation; a real access denied made with icacls on a scratch file),
      `install::tests::portable_refuses_another_programs_settings` (now also access denied).
      A held one is **not known**: the program does not guess and does not run. It shows one message
      (`setup::SETTINGS_IN_USE_AT_START`, PROVISIONAL: "This folder holds a settings.txt that
      another program has open just now, so Bevel95 cannot tell whether it is its own. Close that
      program and start Bevel95 again. Nothing was changed.": `SETTINGS_IN_USE` without its "or
      choose Install", since no Setup window shows here) and ends, before Setup compares anything
      with an installed copy, writing nothing anywhere (`log::log` and `state_file::write_whole`
      write nothing while it is not known; `data_dir` creates no folder). **`--restore` is the
      exception: it gives Windows' taskbar back whatever is known** (the fifth safety review of
      2026-11-18, A1: it showed the message and then only asked a running bar to close, so a hung
      bar, or a taskbar left hidden with no bar running, stayed so). It decides without a message
      (`log::decide_quietly`) and always does all of `restore_command`: asks a running bar to
      close, ends one that does not close within 3 s, un-hides Windows' taskbar, broadcasts
      TaskbarCreated. Not known, it reads the record beside the program and in the installed
      folder, puts the setting back by the usual rules, retires what it used (the record beside the
      program removed, the installed copy's noted as used beside the program: the sixth safety
      review of 2026-11-18, B1, "Milestone 1 decisions"), writes no log and no other file
      (`explorer::places_for`, `restore_plan`'s `writes`), and shows no message (the taskbar is
      back; nothing waits on the person).
    - **Setup's Portable** tells its own run (`log::became_portable`, by Bevel95's own deed, not
      by reading) once it has written the folder's settings.txt, and starts the bar it unpacked
      with the flag.
    - **Every place one Bevel95 program starts another, and the flag** (`log::flag_for`: only a
      portable run, only to one of the copy's own six programs, in its folder or `programs\`; any
      other program gets nothing): the bar's Restart, watchdog and the watchdog's new bar
      (`watchdog::spawn`, flagged as before); the Start menu, Quick Search, a task button's
      middle click and the bar's Explorer command (`window_list::launch`); the classic desktop's
      folder windows (`desktop::open_in_folder_window`) and its icons opened (an own program's
      exe); Explore (`file_ops::explore`, from the desktop and the folder windows, which also
      starts folders.exe); Help (`help::Help`, from the games, the Calculator and the bar); a
      folder window opening an item or what was typed into its address bar; Run (a path to one of
      the copy's programs). Not flagged: Setup starting the installed copy (`install::start`) and
      `--restore` of the installed copy (Update, Remove): the installed copy's. Windows' own
      programs and shortcuts (.lnk) to the copy's programs: a shortcut cannot carry it; such a
      start is the person's and reads the mark.
    - Tests: `log::tests::a_decision_is_not_asked_twice` (portable at the start, the file held
      afterwards: still portable, 20 asks in under 100 ms; held at the start: not known, still so
      once freed; another program's at the start, Bevel95's later: still installed),
      `a_told_start_never_opens_settings_txt` (a reader that fails the test if called),
      `the_flag_goes_only_to_the_copys_own_programs`, and on a real program of a copy:
      `programs/help/tests/data_folder.rs` (its `bevel-help.exe` copied into a scratch portable
      copy, started on a private desktop with %LOCALAPPDATA% pointed at a scratch folder: held and
      unflagged: the message, nothing written anywhere while it shows and after; held and
      flagged: no wait, its log beside the copy's bevel95.exe; free: by the mark; no settings.txt:
      the installed folder). Tried live (`private/probes/review4/portable/live.ps1`, the released
      viewer in a scratch copy, settings.txt held by a PowerShell handle without sharing): free
      and unflagged 325 ms, log beside the copy; held and unflagged the message after 1.4 s,
      nothing written; held and flagged 72 ms, log beside the copy; no settings.txt: the scratch
      installed folder.
  - **Bevel95 never writes a settings.txt that is not provably its own** (the third safety review
    of 2026-11-18, A1): every write of settings.txt (the bar's creation of a missing one, every
    `save_setting`, the clock's offset and zone, Taskbar Properties, Setup's Install answers and
    Portable's new file) goes through `state_file::change_settings`, which writes only a file
    that is missing (the defaults) or holds the mark. One without it is kept byte for byte and
    logged once per run ("... holds no "# Bevel95 settings" line, so it is not Bevel95's own: it is
    kept as it is and never written; a setting changed holds for this run only"); the setting the
    person changed holds for that run. Before, only the folder's classification checked the mark.
  - **In the installed folder settings.txt is always Bevel95's own** (the fourth safety review of
    2026-11-18, C1: a person who deleted its comment lines by hand lost every later change, without
    a word, and Setup's Install reported answers it had not written). The mark rule exists for a
    folder that may hold another program's file; `%LOCALAPPDATA%\Bevel95` is Bevel95's own, so
    there a settings.txt is saved whatever its first line is, and the mark is put back as its
    first line when it is missing (logged once: "... had lost its first line ...; it is Bevel95's
    own folder, so the line is put back and the settings are saved"). In a portable folder the
    rule stays, but a refused save is no longer silent: once per run the bar says so in the
    look's own message window (`bar::show_settings_notice`, at its next tick; title Bevel95;
    PROVISIONAL: "Bevel95 could not save your settings: the first line "# Bevel95 settings" is
    missing from settings.txt next to the program, so Bevel95 cannot tell that the file is its own
    and does not write it. Put that line back as the first line of the file to save settings
    again. Until then a change holds only until Bevel95 ends."; `state_file::is_not_ours` tells
    the refusal). Setup's Install, whose answers could still fail to be written (an unreadable or
    read-only file), then logs "the answers were NOT written" and says so in a message instead of
    listing them as written. Test: `state_file::tests::the_installed_folders_settings_are_saved_without_the_mark`
    (the installed folder's tidied file saved with the mark first and the rest kept; a portable
    folder's refused and told apart). Not tried live: the notice on the owner's bar (it would need
    their settings.txt without its mark); the message window is the one the desktop's bad-name
    message uses.
  - Tests: `log::tests::a_settings_file_held_for_a_moment_is_read_and_one_held_longer_is_not_known`
    (held 300 ms: read and found marked; another program's: foreign; held longer: not known, not
    portable and not installed),
    `state_file::tests::a_settings_file_without_the_mark_is_never_written` (a file without the
    mark byte for byte after two changes, logged once; fails before: `change` wrote into it),
    `bar::tests::an_unreadable_settings_file_is_never_replaced` (now also a readable file without
    the mark), `install::tests::portable_refuses_another_programs_settings` (now also a held file:
    `SETTINGS_IN_USE`, nothing written), `watchdog::tests::a_portable_copys_own_starts_are_flagged`,
    `log::tests::the_portable_flag_is_taken_out_of_the_arguments`.
  - **A foreign settings.txt** (anything of that name without the mark, or one that cannot be
    read for a lasting reason: the fifth review of 2026-11-18, B2, whose Portable message names
    that cause instead, `SETTINGS_UNREADABLE`): the copy is not
    portable; Setup shows as on a first run; **Portable** refuses before it writes anything
    (`holds_foreign_settings`, the message `setup::FOREIGN_SETTINGS`, PROVISIONAL: "This folder
    already holds another program's settings.txt, so Bevel95 cannot keep its own settings here.
    Put Bevel95 into a folder of its own and start it there, or choose Install. Nothing was
    changed.") and Setup ends; **Install** works as usual and never touches that file.
  - Tests: `log::tests::only_bevel95s_own_settings_make_a_folder_portable` (five foreign files
    not portable and only read; five of Bevel95's portable: all ten were "portable" by the old
    rule), `install::tests::portable_refuses_another_programs_settings` (Setup's own
    `portable_here` with the one file's programs: refused, the message, their file byte for byte
    and the folder as it was; the message box is a stand-in in the tests). Not tried live: Setup's
    window in a folder with a foreign settings.txt (its test mode was not run for this).
- **Remove waits for the bar and its watchdog (2026-11-17**, repairing step 3 above). After closing
  the bar (`close_running_bar`, which waits only for the bar's window), Remove now waits until no
  other process runs the installed `bevel95.exe` (`install::wait_for_copies`: a process snapshot,
  each `bevel95.exe` opened and its file asked on the open handle, so a reused process number is
  not mistaken; Setup's own process is left out, since `--uninstall` runs the installed file
  itself), at most 5 seconds (`REMOVE_WAIT`), and only then deletes. The log says how long it
  waited; if the time runs out it removes anyway, says how many copies still run, and keeps
  `bevel95.log` so that line can be read (the folder would stay in that case regardless, as the
  watchdog will still write). (Changed 2026-11-18: a copy still running after the 5 s is a bar
  finishing a file job, so nothing is removed and Setup says so: "File jobs outlive their
  window".) New windows-sys feature: `Win32_System_Diagnostics_ToolHelp`.
  - Proof: `install::tests::remove_waits_for_other_copies` (a copy of cmd.exe in a scratch folder
    stands for bevel95.exe: no wait when none runs, a copy ending after about a second is waited
    for, one that does not end stops the wait at its limit and is counted, the excepted process
    and a same-named file elsewhere are not waited for). Setup's Remove tried in the test mode
    (`BEVEL95_SETUP_TEST`, LOCALAPPDATA in a scratch folder of the session): a copy as built
    installed there, then `bevel95.exe --uninstall`, Yes, OK: the folder, shortcuts and scratch
    entry gone. **Untried against a running bar**: a second bar cannot run beside the owner's, so
    the race of step 3 itself was not repeated; the owner's checklist for a clean PC has "the
    folder is gone after Remove".
- **The same version started again says so** (2026-11-18, the owner after the release: the one
  file downloaded again and started while that build was installed and running showed nothing;
  the log said "this copy is the installed one; starting that", then the started bar "another
  Bevel95 bar is already running; this one exits"). Now, when the one file (or a copy as built) is
  the very build that is installed and the bar running is the installed copy's
  (`install::running_bar_is`: the bar's window, its program file the installed `bevel95.exe`, as
  `close_running_bar` already asked), Setup shows the look's own message box
  (`confirm::show_message`: the installed copy's look and colors, in the middle of the main
  screen, title "Bevel95", OK) with "Bevel95 <version> is already installed and running."
  (`setup::already_running`) and ends; nothing else happens. Installed but not running: it starts
  the installed copy, as before, with no message. The installed copy started a second time (the
  desktop shortcut) never reaches Setup (`here_installed`) and stays silent; `--setup` skips the
  comparison as before; `--restore`, `--after`, `--uninstall` and the other switches never reach
  this code. Windows' own message box only if the look's font cannot be had. PROVISIONAL: the
  words.

## Programs: the shared Windows side and program windows (2026-11-09, the owner: "do minesweeper")

- The review's second and third moves, done for the first program: every program is its own exe
  in the workspace (`programs/<name>/`), and each owns its state per window. What they share:
  `core` (drawing, the control kit) and the new library `platform/winkit` (crate `bevel-winkit`):
  the GDI font atlases (`font.rs`), the control kit's window host (`kit.rs`), a program's menus
  (`menu.rs`), the look Bevel95 is set to (`look.rs`), the log and the small helpers. The bar's
  crate keeps only what is the bar's; its `kit.rs` is a thin wrapper that hands the bar's fonts to
  the host. Proof that nothing changed for the bar: `tools/dev/dialogcaps.ps1` before and after,
  identical apart from the running clock.
- The host provides several kinds of window (`kit::Kind`): `Tool` (the bar's dialogs, as before: no
  taskbar button, always on top), `Program` (a taskbar button, a minimize box, Windows' own
  minimize and restore, its icons made from pixels at run time) and `Dialog` (a program's dialog:
  above its owner, which is switched off while it is open and comes back to the front after).
  `ResizableProgram` gives the Help viewer native sizing and maximize/restore while its
  form paints the classic frame and handles the changing client dimensions.
  New events reach a `Form`: the right and middle buttons, a grabbed part being dropped, losing
  the mouse, the window coming to the front or going behind, Alt alone or F10 for the menu bar.
- A program's menus (`menu::track`) are shown like Windows' TrackPopupMenu: a modal loop that
  returns what was chosen, or how the menu ended (Esc, the arrow keys to the next title, the
  pointer on another title, a click elsewhere, which is handed back to be carried out). They
  are painted by the core's menu painter, so they are the same menus the bar shows.
- A program follows the bar's look and size: it reads `settings.txt` when it starts
  (`look::load`), and the bar posts the registered message `Bevel95.LookChanged` to every
  `Bevel95Window` of other programs when either changes (`kit::tell_programs_look_changed`).
  Without the bar running, a program still starts, in the look last set.
- **A program's first step** is `log::program_start` (the bar's: `log::decide_at_start`): where
  this run keeps its data is decided once and kept (the fourth safety review of 2026-11-18, B1);
  a program a portable copy's program starts is told so by `--portable-copy`, which every place
  that starts one of the copy's programs adds (`log::flag_for`; the list is in "Bevel95 Setup",
  where a copy keeps its data). A program that cannot know (its copy's settings.txt held by
  another program at a start by the person) says so and ends, writing nothing.
- `core::window`: the frame, title bar and menu bar of a program's window, measured on the
  reference PCs in True Color (theme-spec, "Program windows"): the real Minesweeper on Windows
  95 and 2000 and Calculator on 98. The owner asked why Calculator when the real target could be
  looked at: Windows 98's Games were never ticked at its install (an optional part on every
  retail disc). What Bevel95 needs of it, the window around a program, is the same for every
  program, and Calculator showed it. The owner wants every optional part installed on the
  reference PCs all the same, for the programs to come (todo). The dialogs' frames and title
  bars now come from the same code.
- Measuring those windows in True Color corrected four things the bar already drew, all backed by
  0-pixel comparisons: **Windows 98's title bar gradient** (the old rule depended on the window's
  place on the screen and was a shade off on GUIdebook's Run; the new one fits six windows of the
  reference PC and all three GUIdebook dialogs), **grayed-out menu entries** in the 95 and 98
  looks (etched like 2000's; the emulator's 16 and 256 colors had shown them plain gray, and
  Bevel95 draws True Color), **Windows 98's menu separators** (1px short at both ends) and
  **Windows 2000's check mark** (2px further left). Twelve golden images changed with them.
- **The edges flashed while moving between a menu bar's menus** (2026-11-18, the owner, 2000 look
  at size 2: "when I flip through win exp menu items, the edges of that window ui flash a bit";
  "It's like the outermost stroke of the window"). Measured before anything was changed
  (`private/probes/menuflash`: a scratch copy of folders.exe on a test folder of its own, keys
  posted to its own windows only, the screen around the window photographed about 36 times a
  second during each change, each frame compared with the frames before and after the change):
  moving from one menu to the next (Right), some frames showed the window's outer edge (two
  screen pixels at size 2, a few more on the left and top) in Windows' own system colors
  (E3E3E3, A0A0A0, 696969, and the inactive sizing border F4F7FC) instead of the look's frame:
  in 3 of 5 changes in the 2000 look's Windows Explorer, 3 of 7 in the 98 look's, 8 of 12 in the
  95 look's plain folder window. Moving through one menu's entries (Down) showed none: the
  window's activation does not change then. **The cause**: a menu takes the focus while it is open
  (`menu::track`), so between two menus the folder window becomes active and inactive again;
  each time Windows' default handling of WM_NCACTIVATE painted today's sizing frame over the
  window's outer pixels (a window with a sizing frame, WS_THICKFRAME, though `kit` gives it no
  frame area), until the form painted again. **The repair** (`kit::kit_proc`): for a window
  with a sizing frame, WM_NCACTIVATE goes to the default handling with lParam -1, which keeps
  its answer and its activation work but paints nothing (Windows' documented meaning of -1); the
  form draws its own active or inactive title bar as before. After it: 0 frames with pixels of
  neither picture in every run (2000 Explorer: 6 changes of menu and 6 of entry; 98 Explorer: 6;
  95 and 2000 plain folder windows: 6 each). The other kit windows with a sizing frame (the Help
  viewer) are repaired by the same line; Minesweeper, Solitaire and the Calculator have no
  sizing frame (their windows were never painted over) and were not measured. A limit (the
  review before 0.1.1): Microsoft documents lParam as unused while a visual style is active for
  the window, so the repair rests on how today's Windows was measured to behave; check it again
  after a Windows feature update.

## Minesweeper

- **Reference versions.** The first release on 2026-11-09 used the name Minefield and its own
  game artwork. On 2026-11-10 the owner requested fidelity to the Minesweeper shipped with
  Windows 95, 98 and 2000. True Color captures of all three editions are under
  `private/reference/refs/vm/*-truecolor-mine-*.png`. Each look uses its edition's board, including the
  narrower Beginner field on Windows 95 and 98. The measurements are in
  [the Minesweeper specification](theme-spec.md#minesweeper).
- **Board and input.** At 1× each square is 16×16. The face, mine count and timer sit in a
  recessed panel above the field. Holding the left button depresses a square. Releasing opens
  the square under the pointer. The first square is safe and can show a number. The timer
  reads 001 immediately after the first opening and stops at 999. Right-click cycles the flag
  and optional question mark. Pressing both buttons on an open number previews the neighboring
  squares. Releasing opens those squares when enough flags surround the number.
- **Mine placement.** A new field uses the original Microsoft C random-number sequence.
  If the first click hits a mine, the game moves it to an empty square. Custom fields start
  at 8×8 on Windows 95/98 or 9×9 on Windows 2000 and can grow to 30×24 within the available
  screen space. A field W squares wide and H squares high allows 10 to (W − 1) × (H − 1) mines.
  On a maximally dense board, if the original relocation scan finds no free interior square,
  Bevel95 also checks the last row and column so the first click remains safe.
- **Menus and dialogs.** The host tracks Game and Help with the shared menu painter.
  `core/src/minefield/form.rs` supplies the entries for the selected Windows edition. Custom
  Field takes height, width and mines. Best Times stores a name and time for each of the
  three standard levels. A fresh score reads “999 seconds” and “Anonymous.” The box runs above
  its owner and returns focus to the board on close.
- **Sound.** The Windows 2000 Game > Sound option plays the original clock tick, win cue
  and explosion. The executable embeds the three unsigned 8-bit mono PCM streams from
  WINMINE at 22,050 Hz. The core reconstructs each WAVE header and tests the complete stream
  against the hash of the original resource. Installed and portable copies use the same bytes.
- **Help and About.** Minesweeper supplies its own topic catalog to the shared Help
  viewer. The game instructions are written for Bevel95. The original HLP and CHM files
  stay in the reference cache. About uses the host's memory total, anonymous registration
  and “N/A” for the Windows 95/98 resource-heap gauges.
- **Launching and settings.** The Start menu locates `programs/minefield.exe` in the bundle
  and lists Minesweeper under Accessories > Games. It reads the bar's look
  and scale and responds to changes while open. The settings and scores use the existing
  `minefield.txt` file. Changing difficulty keeps the window's top-left corner in place
  when the new board fits the screen. Reopening restores its saved position. A second
  launch brings the existing window forward.
  `tools/install.ps1` leaves a running game open, so close it before installing to pick up
  the new executable. Setup installs companion programs under `programs/` in the destination.
  Updating an older flat installation migrates the known game and retires the old Help
  books and WAVs that match the previous installation. Settings and scores stay at the
  root. A portable bundle uses that same layout.
- **Shared window host.** The program uses `Hosted::closing` to save its window position and
  `Form::pointer_left` to clear the hover border on Windows 2000's menu bar. The game and
  the bar's dialogs share the `Bevel95Window` class, so the live test helpers also check
  the owning process before sending input or capturing a window.
- **Testing.** The tracked PNGs in `core/tests/golden/` record rendered scenes. To compare the
  rendered games with the original Windows captures in `private/reference/refs/vm/`, run
  `$env:BEVEL95_CASE = 'minesweeper'` followed by `cargo compare-refs` in PowerShell.
  Clear the filter with `Remove-Item Env:BEVEL95_CASE` to run the full reference suite.
  `tools/dev/minefield.ps1` can run the built game with a scratch settings directory, send
  clicks and keystrokes to its own windows and photograph one window at a time.
  `BEVEL95_MINEFIELD_SEED` fixes the board for repeatable play. An earlier live test found
  that another application on the owner's PC registers Alt+G globally. F10 or Alt, then G
  reaches the Game menu.

## Solitaire

- **Installing and starting.** Start > Programs > Accessories > Games finds
  `programs/solitaire.exe` in the installed or portable bundle. The game uses the bar's
  selected look and size. Its options and explicit card-back choice go in `solitaire.txt`
  beside `settings.txt`.
- **Play.** Seven columns build down in alternating colors. The four suit stacks run
  from ace to king. Moving a sequence keeps its order, and turning up the next covered
  card takes a separate click. Options chooses Draw One or Draw Three, Standard, Vegas or
  no scoring, a timed game, the status bar and outline dragging. The Deck dialog offers
  twelve designs. A double-click on an eligible card sends it to its suit stack.
  Windows 2000 also moves every eligible card with a right-click or Ctrl+A. At a win,
  all three editions send the completed suits bouncing across the table, king first.
- **Scoring.** Standard awards 5 points for turning up a column card or moving a card
  from the turned-over deck pile to a column, and 10 for a card sent to a suit stack.
  Taking one back costs 15. Vegas charges $52 for each deal and pays $5 per card on
  a suit stack. Draw One allows one pass through a Vegas deck, Draw Three allows
  three. The optional cumulative balance lasts while Solitaire is open.
- **Help.** Solitaire sends its own catalog to `bevel-help.exe`. Windows 95 has two
  WinHelp topics. Windows 98 has a welcome page and three entries in Contents. Windows
  2000 starts with an overview and has separate articles for play, options and scoring.
  Its **Search for Help on...** command opens the index and **How to Use Help** opens the
  viewer manual. The contents, keywords and targets follow the original books. The
  instructions were written for Bevel95. [Solitaire catalogs](../core/src/solitaire/help.rs)
- **Checks.** Native tests opened Solitaire's Help from its own menu in all three looks,
  played and undid moves, dealt again and reopened the saved options and deck back.
  The Windows 98 and 2000 Help commands reused the existing viewer window. To play a
  repeatable deal in a test, set `BEVEL95_SOLITAIRE_SEED` before launching the game.
  A native Windows 2000-look test won seed 725 in 72 mouse gestures, let the last card
  bounce off the table and accepted **Deal Again?** to start a fresh hand.
  To run the Solitaire reference comparisons, set `$env:BEVEL95_CASE = 'solitaire'`
  in PowerShell and run `cargo compare-refs`.

## Calculator (2026-11-11, the owner asked for it; Standard view first)

- **Built like the games**: `programs/calculator` (`calculator.exe`, a thin Windows side: the
  clipboard, the About box's place, calculator.txt) and `core::calculator`: `number` (Bevel95's
  own decimal type), `engine` (what each Windows computes and shows, key by key), `form` (the
  window, a kit form), `captions95` (95's captions as pixels), `art` (CALC.EXE's 16- and
  32-pixel icon, the same bytes in all three), `dialogs` (About), `settings`. It is in
  `start_menu::OWN_PROGRAMS` under **Accessories** (`OwnProgram::folder`: where Windows kept
  it; the games stay in Games), so also in the choice "only Bevel95's own classic programs", in
  Setup's companions and `install.ps1`. Several can be open,
  as with the real one. It opens where Windows' cascade puts a new window (asked as Solitaire
  asks).
- **Today's Calculator stays** in the Programs menu and Find. `core::find` ranks Bevel95's own
  program before another of the same name and rank (`Program::own`, set by the bar from
  `start_menu::is_own_program`), so `calc` and Enter start Bevel95's.
- **Arithmetic.** Windows 95: the C runtime's double, 13 digits shown and typed, each + − × ÷
  result rounded to 15 significant digits (an inference: it is the one rule that gives 0 for
  both 0.1 + 0.2 − 0.3 and 2 sqrt × = − 2, as the PC showed, and fits every other sequence).
  Windows 98 and 2000 ("accurate to at least 32 digits"): `Decimal`, 48 significant digits
  kept, rounded half up after every operation, an exponent that does not run out (the PCs
  squared to 10^132571 without an error). 98 shows 33 significant digits and shortens the
  exponent form to fit 37 characters; 2000 shows 32 and lets the exponent form run off the
  display's left edge. Whole numbers switch to the exponent form past 13 (95), 33 (98) or 32
  (2000) digits.
- **Rules** (`engine`, from their CalcEngine's behaviour): no precedence; an operator after an
  operator replaces it; `=` repeats the last operation with the same right-hand number; after
  `2 + 3 =`, `4 =` gives 4 on 95 (a typed number ends the repetition) and 8 on 98 and 2000
  (the typed number is lost, the held one comes back); `%` is that percent of the left-hand
  number (0 after C); +/- while typing keeps typing; Backspace only takes back typed digits;
  95's `.` starting a number does not show `0.` until the next digit; while an error shows
  only C and CE act. Error texts per Windows (95 `Cannot divide by zero.`, `Result of function
  is undefined.`, `Result is too large.`; 98 `Error: Positive Infinity.`, `Invalid input for
  function.`; 2000 `Cannot divide by zero.`, `Invalid input for function.`).
- **Every measured sequence is a test**: the three behaviour tables of the Reference PCs are
  `core/tests/fixtures/calculator/behaviour-<pc>.txt` (40, 43 and 43 sequences, 356,
  382 and 382 keys, the memory ones included; the rows where the PC lagged a key behind are kept
  as comments), run
  key by key by `engine`'s tests. Not matched, and listed there (`KNOWN`): 98's and 2000's
  `2 sqrt × = − 2 =` (−2.97…e-38 and 1.157…e-37 on the PCs, 0 here: their square root was a
  few digits off past the 32nd).
- **Keys** (the Help's list for the Standard view): digits, `. ,`, `+ - * /`, Enter and `=`,
  `@` sqrt, `%`, `r` 1/x, F9 +/-, Del CE, Esc C, Backspace, Ctrl+M/R/P/L for MS/MR/M+/MC,
  Ctrl+C and Ctrl+V. A key flashes its button pushed in for 4 ticks of 50 ms and is done as the
  button comes up (the PCs still showed the old number while it was down); another key, a
  click or a menu does a pending one first. The character Windows makes of Ctrl+M (a carriage
  return) is no `=`. Paste types the clipboard's characters as keys, with the Help's
  `:c :m :p :q :r` sequences.
- **Menus**: Edit (Copy, Paste; Paste grayed without text on the clipboard), View (95
  Scientific/Standard with a check mark, 98 and 2000 Standard/Scientific with a bullet, 2000's
  D&igit grouping, which works and is remembered), Help (Help Topics, About Calculator).
  Scientific and Help Topics were grayed until the view and the book existed (both are there
  now: 2026-11-12). Two shared fixes came with them, measured on these
  menus: the option bullet is 6×6 (it was a provisional 5×5; the bar's Theme and Size menus
  changed, six golden pictures), and Calculator's own menus on Windows 2000 underline a grayed
  entry's access key with every other pixel (`Menu::dotted_disabled_keys`; the window menu and
  Solitaire's Game menu on the same PC underline solid, so the rule is Calculator's only).
- **Windows 95's captions are pixels**: its bold 10-point face is not today's MS Sans Serif
  (C and E are narrower; the digits agree), so the 27 captions were cut from the fresh capture.
  98's and 2000's are MS Sans Serif 8 (2000's Calculator did not use Tahoma), centered.
- **Remembered** in `calculator.txt` as WIN.INI's `[SciCalc]` had it: `layout` and `UseSep`.
  Setup's Remove deletes it (`DATA_FILES`), README's "Files it uses" lists it.
- **Proof**: unit tests (number, engine with the three tables, form, settings, find); 64
  compare-refs cases (`BEVEL95_CASE=calculator`: fresh, typed and negative numbers, the longest,
  memory, each error, buttons held, let go off and clicked, the key flash, each menu open,
  Digit grouping, About), all 0 pixels, masks only for the guests' pointer, 95's flash caught
  mid-repaint and About's registration lines; nine new golden pictures (`*-calculator`,
  `-menu`, `-about`). Live (`tools/dev/calculator.ps1`: a scratch data folder, an 8-second
  idle gate before any input, keys only while the Calculator is in front, clicks only where it
  lies): keys, clicks, a button held and let go off it, the View menu, About, Digit grouping
  remembered, a look change (98 to 95 at 2x: 0.25 stayed). The full compare-refs: 417 cases,
  0 differing pixels. Not tried by Claude: Copy and Paste (the owner's clipboard held other data), the
  Start menu entry and Find (the owner's bar; owner's checklist).
- **Scientific view (2026-11-12).** Measured on the three Reference PCs the way the Standard
  view was (a helper worker typed 86 sequences plus extra probes per PC, the display read after
  every key, with the memory, parentheses and Inv/Hyp marks): the tables are
  `core/tests/fixtures/calculator/behaviour-sci-<pc>.txt`, run key by key by `engine`'s tests.
  - **Precedence** (their CalcEngine's table): Or Xor < And < + − < × ÷ Mod Lsh < x^y, left to
    right within a level; an operator replacing another is undone and done again (2 + 3 × − 4 =
    gives 1) while the display keeps the number; "=" does everything of the innermost open
    level and repeats the last operation done (2 + 3 × 4 = = gives 26). The Standard view keeps
    no precedence.
  - **Parentheses**: at most 25 (more are ignored, silently); the display shows one "(" per open
    level until a number comes, the box "(=n"; ")" without "(" does nothing; "=" leaves them
    open; C closes them, CE does not. 98 and 2000 drop a number being typed at "(" (5 ( 3 gives
    3); 95 goes on typing it (53).
  - **Bases**: 95 keeps a 32-bit whole number (more is "Result is too large.") and the word size
    only cuts what is shown (back in Dword the digits are there; Dec shows it signed). 98 and
    2000 cut the number itself to the word size and show Dec unsigned; 2000 has Qword (its
    default, F12). A digit is taken only while the number still fits (Oct Dword: ten 7s). A
    number with a fraction becomes whole; 98's and 2000's Lsh/Rsh in Dec multiply and divide
    (−5 Rsh 1 = −2.5), 95 shifts 32 bits (1 Lsh 40 = 256, the count taken mod 32); And with a
    fraction gives 0 on 98 and 2000 (Or and Xor assumed the same: not measured).
  - **Grayed buttons** (98, 2000): A–F outside Hex, digits a base lacks, F-E dms Exp sin cos tan
    pi outside Dec, Ave Sum s Dat while the Statistics Box is closed; their keys do nothing ("."
    stays blue in Hex but its key does nothing; F-E's key still switches F-E). 95 grays nothing:
    those keys mostly do nothing, sin in Hex computes and cuts, Ave/Sum/s/Dat end the typed
    number.
  - **Functions**: `number.rs` has exp, ln, sin/cos (reduced in degrees and grads exactly: sin
    180° is 0, tan 90° invalid), atan, x^y, factorial with Γ for numbers that are not whole
    (Stirling's series past 100), in 48 digits; 95 uses doubles (its trigonometry rounded to 15
    places). Inv and Hyp come off after a function that used them (Hyp only after sin cos tan;
    x^y's Inv at "="); 2000 leaves them on when the function fails; C clears both. x^y beyond
    10^9999 is "Invalid input for function." on 98/2000 (where between 10^434 and 10^99999 the
    real limit lies was not found). 95 has no factorial of a number that is not whole ("Result is
    too large.").
  - **Exp and F-E**: Exp starts the exponent ("1.e+0"; 95 "1.e+000", from 1 when nothing was
    typed, an exponent up to 307); +/- flips its sign; 98 and 2000 take four digits. F-E ends a
    number being typed and shows the exponent form ("1.23e+2", "5.e-1"; zero "0.e+0", 95 "0.");
    C switches it off.
  - **Small numbers** on 98 and 2000 go to the exponent form once zeros and digits after the
    point pass the digits shown by one (1e-12 plain, 6.98e-31 not).
  - **Errors**: 98 says "Error: Negative Infinity." for −1/0, and all three "Result of function
    is undefined." for 0/0 and x Mod 0 except 95 ("Cannot divide by zero."); 95 "Result of
    function is undefined." where 98/2000 say "Invalid input for function." for tan 90°, log 0,
    ln −2, sqrt −4. While an error shows, 95 and 98 switch the option buttons and check boxes off
    (drawn grayed); 2000 does not.
  - **View switching**: 98 and 2000 start again (display 0., pending operation, parentheses and
    the base gone; the memory stays; Inv stays on although its box shows unticked: 2 sin then
    fails); 95 keeps everything (the number, the operation, the parentheses unseen in Standard, a
    Hex number shown as 0. until Dec is chosen). While an error shows, the view does not change.
    The view is remembered (`layout` in calculator.txt) and the window keeps its corner.
  - **Statistics Box** (`stats.rs`, its own window with a taskbar button, 225×165 at screen
    (120,130) every time; 2000 adds "?"): Dat appends and chooses the new line, LOAD copies the
    chosen one, CD takes it out (the next one chosen), CAD empties, RET gives the Calculator the
    keyboard; its X and View > Standard close it and the numbers go. Ave, Sum, s (n − 1) and with
    Inv the mean and sum of squares and s over n; with one number s is 0.
  - **Typing on 98/2000** is 32 characters with the point (31 digits after "0."), also in the
    Standard view (a change: before, 32 digits).
  - **Not matched** (`KNOWN_SCIENTIFIC`, todo): the leftovers after subtracting the shown digits
    of sqrt 2, 1/3, pi, e (98's and 2000's sqrt 2 was off by about 1e-38 and 4e-38, pi right to
    46 digits, e to 44, 1/3 a fraction); 95's equivalents and its x87 sine of 1e20° and
    36000001°; one repaint quirk of F4 after Radians on 95 and 98. The two open points of the
    Standard view: 98's −1/0 and 0/0 texts are in; the leftover of 2 sqrt × = − 2 = stays
    unmatched (it comes from their square root's last digits, which Bevel95 computes right).
  - **Proof**: the three tables (98, 98 and 102 sequences; 1,608, 1,608 and 1,748 rows; 29
    known rows); 93 new compare-refs cases (`BEVEL95_CASE=calculator-sci`: every base, word size
    and angle, Inv/Hyp, parentheses and their limit, memory, errors, Exp typing, F-E, Digit
    grouping, each menu, the Statistics Box over the Calculator), all 0 pixels, no masks; the
    full run 534 cases, 0 differing pixels. Golden: `*-calculator-scientific`,
    `*-calculator-statistics` new, `*-calculator-menu` changed. Live (`tools/dev/calculator.ps1`,
    which now also sends ( ) ! # & ^, Ins, F2–F12): the view by Alt+V, precedence, Hex and Inv
    by keys, the Statistics Box (Ctrl+S, Enter back, Ins, Ctrl+A, LOAD and CD by Alt+L/Alt+C),
    View > Standard closing it, the view remembered across a restart, in the 95 and 98 looks.
- **Help (2026-11-12).** Help > Help Topics opens each look's own book in the shared viewer
  (`core::calculator::help`, the games' way: the program's `HelpViewer`, closed with the window
  and at a look change). Read from the originals (CALC.HLP decoded by a small reader in
  `private/probes/calc-help`, calc.chm's unpacked files): **95** WinHelp's picker, CALC.HLP's nine
  titled topics in its order, CALC.CNT's Contents (two books, seven pages), the 118 keywords;
  **98** HTML Help, calc.chm's ten topics, its two books and ten pages, 55 keywords and the 16
  under "numbers" and "values", the Windows 98 Welcome page and its Legal Information link;
  **2000** one book "Calculator" with the overview and eleven topics (the original's
  page icons), each topic's Related Topics within the book, 87 index lines. The Help menu had
  only Help Topics in all three (nothing else to add); CALC.EXE has no F1 accelerator.
  - **Original text restored (2026-11-13).** Each edition's catalog contains its book's
    wording, including the right-click instructions, keyboard tables, notes and examples.
    The Windows 98 table uses the older labels Back, PI and Rad. Windows 95 prints BACKSP
    for the Backspace key, Windows 98 prints Backspace and Windows 2000 prints BACKSPACE.
    The original Help binaries stay in the private reference cache. Calculator sends the
    catalog to the shared Help viewer, which lays out table rows and columns at the current width.
  - **Known differences in behavior.** Calculator still lacks right-click What's This?.
    Its decimal engine keeps 48 significant digits. The 98 and 2000 books describe rational
    fractions, and the 95 and 98 books describe pasting `:e` in Hex. The shared viewer displays
    the words of inline glossary links. Glossary popups and links into other Windows Help
    books remain open. The index still flattens subentries and omits two empty 2000 headings.
  - **Verification.** Tests compare every article's text with a checksum taken from the decoded
    original, including its table cells in reading order. They also check the Contents and
    index, topic reachability, the viewer wire format and the Calculator keys named in each book.
  - **The viewers, three changes** (each look's PC showed them): HTML Help opens with only the
    first Contents book open (98's Calculator has two at the top; the games' books have one or
    none); the Contents tree gets Windows' horizontal scroll bar when a row is wider than the
    tree (extent: the widest shown row's selection mark, 241 in 209 on 98, 306 in 224 on 2000,
    thumbs 153 and 141 as measured; arrows 5 pixels, the track a page, the thumb drags);
    WinHelp's picker, with a book chosen, says "Click a book, and then click Open." and its
    button reads Open (Close when open), Print prints the book's pages; WinHelp's index shows a
    keyword of several topics once and opens its first topic (its Topics Found box is not drawn:
    todo).
  - **Proof**: five compare-refs cases (`BEVEL95_CASE=calculator-help`): 95's picker and its
    keyboard topic, 98's opening window and its keyboard topic, 2000's opening window, all 0
    pixels with the books' own wording checked at the measured places (masks: 95's Related
    Topics button and rule, which the viewer writes as links; 98's keyboard table, which
    reflows in the viewer); the full run on 2026-11-12 had 539 cases, 0 differing pixels. Golden pictures: none new (the games' books have none) and none
    changed (the `*-calculator-menu` pictures show the View menu). Live (`tools/dev/calculator.ps1`
    with a scratch data folder, release build): Alt+H, Enter in each look opened the right
    window at the right place (95: 434x428 picker; 98: 525x450 at 50,0; 2000: 550x450 at
    230,30), photographed, and closing the Calculator closed it.

## Bevel95 Settings (2026-11-11, the owner: "put 'Bevel95 Settings' in the right click menu")
- The owner's rule for where a setting lives: what the original Windows had stays where it
  was (Taskbar Properties, Date/Time Properties); everything that is Bevel95's own goes into
  one place, **Bevel95 Settings**, opened from the bar's right-click menu. Its entry sits just
  above Properties (`BarCommand::Settings`, code 5; new commands join the end of
  `BarCommand::ALL`, so the codes that scripts send stay what they were). Three more pages
  have come since: Desktop, Programs and Taskbar (their sections below).
- One form, two windows: `properties::Sheet` (`Taskbar`: Windows' two pages; `Bevel95`: the
  former third page, now its first tab, "General": Look, Size, Startup, Clock; a second tab,
  "Desktop", came with the desktop, see the next section). Taskbar
  Properties so has exactly Windows' tabs again, and `cargo compare-refs` no longer masks a
  third tab: the whole row of tabs is compared with the captures. On the Windows side
  `properties_window::SheetHost<const OWN: bool>` gives the window host two kinds
  (`PropertiesHost`, `SettingsHost`) for the one form.
- Both windows can be open at once and both hold all the values, so each applies only its own
  (`Sheet::merge`: its settings as it shows them, the other window's as they are in effect
  now); otherwise the window opened first would put back what the other one changed. Both
  follow the bar's look and size whenever those change (`properties_window::look_changed`,
  called by `set_era` and `set_size`, so also for the bar menu's Theme and Size entries, which
  an open Properties window used to ignore); Bevel95 Settings then shows the new look or size
  as the one chosen. A window goes back to its opening place only when its look or size
  changed, no longer after every Apply.
- PROVISIONAL (ours; no classic Windows had such a window): the title "Bevel95 Settings", the
  tab "General", and where it opens (Taskbar Properties' place, one 22px cascade step right
  and up, so that neither window opens exactly over the other).
- Theme and Size stayed in the bar's menu (the owner has not decided otherwise). Since 2026-11-16
  they and Bevel95 Settings are in the menu's one "Bevel95" entry ("One "Bevel95" entry and Quick
  Search" below).

## The desktop, first step (2026-11-11, the owner: "go ahead on desktop")
- What the owner asked for: a basic desktop in the original systems' default colors, which a
  right-click can switch off; icons brought over from the real desktop come later. So the
  first step is one plain window (`platform/win/src/desktop.rs`, class `Bevel95Desktop`) in the
  look's desktop color (`Colors::desktop`: 008080 in 95 and 98, 3A6EA5 in 2000; three
  compare-refs cases `vm-*-desktop-color` check them against the reference PCs' desktops).
- Nothing of Windows is changed, so nothing needs restoring: Windows' desktop, its picture and
  the owner's icons stay underneath, and when our window goes (switched off, Exit, a crash,
  Ctrl+Win+T) they simply show again. That is why the watchdog, the panic hook and `--restore`
  know nothing about the desktop.
- Where it sits: directly above Windows' desktop window, under everything else, the way
  ManagedShell places Cairo's desktop (`WindowHelper.ShowWindowDesktop`: after the window just
  above the desktop). Measured on this Windows 11 (25H2): "Progman" holds the icons
  (SHELLDLL_DefView) and is the bottom window of all; when it does not hold them, the "WorkerW"
  that does is used. Windows puts every minimized window right above Progman, and "Show
  desktop" raises Progman over the other windows, so the place is checked on the bar's 100 ms
  tick (`desktop::tick`) and ours is put back directly above it (which also makes Win+D show
  the classic desktop). Only `place` may change its place: `WM_WINDOWPOSCHANGING` adds
  SWP_NOZORDER for everyone else, and if it ever became a topmost window it is put back. (Since
  "Win+D and the classic desktop" below, ours is owned by Windows' desktop window and moves with
  it; only a move into the topmost windows is refused.)
- It never takes the focus (WS_EX_NOACTIVATE, MA_NOACTIVATE), so clicking it cannot bring it
  forward; the price is that a click on it does not deactivate the window in front (todo). For
  the same reason a click does not reach an open menu as "a click elsewhere": the desktop
  closes our menus and the volume control itself, as the bar does. It carries the window
  property `NonRudeHWND`, as Windows' desktop does, so nothing takes it for a fullscreen app
  (our own check leaves out our process anyway), and is excluded from Peek.
- It covers all screens (the virtual screen), as the desktop color did on Windows 98's and
  2000's several screens, while the bar stays on the main one. Not tried with a second screen.
- Painting is one GDI fill of the color (kept in an atomic, so painting never waits for the
  bar's state). It is the one window not drawn through the 1x framebuffer: one flat color has
  no pixels to scale. The icons will be drawn the usual way.
- Its right-click menu (`core::desktop`, `popup::Kind::Desktop`) holds only what works: Switch
  to Windows desktop (the `desktop` setting goes to `no` and is saved; wording after the bar's
  "Switch back to Windows") and Bevel95 Settings (opens its Desktop page). Windows' own
  desktop menu comes with the icons. Commands reach the bar as `WM_DESKTOP_COMMAND`. (Since 2026-11-16
  both are in the menu's one "Bevel95" entry, with Quick Search.)
- The setting: `desktop = yes|no` in settings.txt (`Config::desktop`, default yes because the
  owner asked for the desktop; what a release should default to is in todo.md), shown as "Show
  the classic desktop" on Bevel95 Settings' new Desktop page (`Page::Desktop`,
  `Values::desktop`; it belongs to `Sheet::Bevel95`). The desktop shows while it is switched on
  and our bar is showing (`bar::sync_desktop`); the desktop's own menu switching it off
  updates an open Bevel95 Settings (`properties_window::desktop_changed`).
- PROVISIONAL (ours): the menu's two entries and their wording, the Desktop page's group and
  its three lines of text.
- Proof so far: unit tests, 9 golden images (the page, the desktop with its menu, and Bevel95
  Settings' General page now with two tabs), the three color cases, a full `cargo
  compare-refs` with no pixel differing anywhere. Live on the running bar (2026-11-11, the
  2000 look at 2x, one 2560x1440 screen):
  - Read back without touching anything (`private/probes/desktop-live.ps1`): the window is the
    bar's, covers the virtual screen, sits directly above Progman with nothing else below it,
    is not topmost, never the window in front, carries NonRudeHWND, and the screen shows
    3A6EA5 where it is in front.
  - With real clicks, at a moment the PC was idle (`private/probes/desktop-menu-live.ps1`): a
    right-click brings the menu up at the pointer, with the keyboard; a click on the desktop
    beside it closes it; Switch to Windows desktop takes the window away, Windows' own desktop
    is under the pointer again and settings.txt says `desktop = no`; Bevel95 Settings opens on
    the Desktop page with the box empty, ticking it and OK bring the desktop back to its place.
  - What the test taught: a menu that closes at a click on the desktop leaves the bar as the
    window in front (Windows activates a destroyed window's owner), where real Windows leaves
    the desktop in front; either way the window that was in front stays inactive, as it did in
    95. And a settings.txt from before the desktop has no `desktop` line (that means on); the
    first change writes one.
- **Properties can open Windows' own Display settings** (2026-11-19, a first report after the
  release: a person looked for the screen resolution under the desktop's Properties, where
  Windows 95 to XP had it, and got Bevel95 Settings). One setting, off by default:
  `windows_display_settings = yes|no` (`Config::display_settings`, `Values::display_settings`,
  `Sheet::Bevel95`), shown on the Desktop page in a second group, "Properties on the desktop's
  menu", as the check box "Opens Windows' display settings" (PROVISIONAL words; the brief's
  "Properties on the desktop opens Windows' display settings" was too wide for the page's
  296-pixel group in one line, so its first part became the group's title). On, Properties on the
  empty desktop's menu starts `ms-settings:display` the way the Start menu's Settings entries
  start Windows' pages (`window_list::launch`, ShellExecute); off, Bevel95 Settings' Desktop page
  as before. The Bevel95 submenu's own "Bevel95 Settings" always opens the page, so the setting
  can always be switched back; Quick Search's things to do did not change. The Desktop page's
  three golden pictures were regenerated for the new group, and only they. The real Display
  Properties (patterns, wallpapers, colors) are a wish of the owner's for later, not started.
  - A hidden window does land between the two desktops now and then: a read-only watcher
    (`private/probes/zwatch.ps1`) caught the "IME" helper window of a program that had just
    loaded GDI+ (the test's own PowerShell) there for 44 ms, until the tick put ours back.
    Harmless, and the reason `tick` logs the first three times and then only the 10th, 100th,
    1000th...: a window that kept coming back (another desktop tool pinning itself to the
    bottom) would show in the log as a count that keeps growing.
  - The color follows the look (`private/probes/desktop-look-msg.ps1`, which sends the bar the
    Theme menu's own message and no input, because the owner was back at the PC): in the 98
    look the screen showed 008080, 15 ms after the message for the 2000 look it showed 3A6EA5,
    and 9 ms after the message back, 008080 again, in the same window. The same through
    Bevel95 Settings' Look and Apply (`desktop-look-live.ps1`, real clicks) was not run.
  - Not tried live: Win+D and a second screen (the owner's checklist).

## What the Programs menu shows (2026-11-11, the owner: "do the programs page next")
- What the owner asked for: a place to choose exactly what the Programs menu shows ("only old
  things, or apps on their machine, or check each one they want"), and, before that, the apps
  that were missing from it. It came in steps; this section grows with them.
- **Step 1, Windows' own list of apps** (`platform/win/src/apps.rs`). The Programs menu read
  only the shortcut files of the two Start Menu folders, as Windows 95 to 2000 did, so
  Windows' Store-style apps never showed: they have no shortcut file. They are now read from
  Windows' own list of apps (its "AppsFolder", what Start's "All apps" shows): the list's
  entries whose ID has a packaged app's shape, `<package family>!<app>` with the family ending
  in "_" and 13 letters and digits (`programs::is_store_app`). On the owner's PC that is 37
  of the list's 272 entries (Calculator, Notepad, Paint, Terminal, Settings...); the other
  235 are classic programs, already found through their shortcut files, folders and all.
  - Where they go: into Programs itself, among its own entries, where a program without a
    folder of its own put its shortcut. A shortcut file of the same name there wins (it is
    added first). Not done: today's Calculator, Notepad and Paint in their old place under
    Accessories (todo).
  - What starts one: `shell:AppsFolder\<ID>` handed to ShellExecuteW like a file
    (`programs::APP_PREFIX`; the bar's ordinary `launch`). Tried with that very call
    (`private/probes/app-launch.ps1`: Calculator came up within 100 ms).
  - Its icon: an app has no file to ask the shell about, so `start_menu::icon_index` turns
    the name into an item list (SHParseDisplayName) and asks with SHGFI_PIDL; from there on
    it is the same small icon list as for files. All 37 came back with something to show
    (`--list-programs` counts blank ones: none).
  - Reading the list takes 350 to 560 ms here, so the Start menu never waits for it: it shows
    the list as last read (`apps::cached`) and `apps::refresh` reads it again on a thread of
    its own (at the bar's start; and when the Start menu opens, unless the last reading is
    younger than 30 s). A newly installed app so shows the second time the Start menu opens;
    a new shortcut file still shows at once. The log gets a line when the count changes.
  - The COM calls go through hand-counted places in the method tables, like the image
    list's and Core Audio's (windows-sys has no COM interfaces): IShellFolder::EnumObjects
    (5th), IEnumIDList::Next (4th), Release (3rd). No new windows-sys feature was needed.
  - Seen live (`private/probes/programs-menu-live.ps1`: one real click on Start, the key P): the
    apps stand in the real menu with their pictures. With them the owner's Programs menu has
    five columns at 2x and no longer fits beside the Start menu, so `layout::place_submenu`
    keeps it on the screen from the left edge, over the Start menu (and the entry that comes
    up under the resting pointer is selected). That is what the choice of what to show is for.
- **Step 2, the choice** (`programs::Shown`; `programs = everything|classic|ticked` in
  settings.txt, default `everything`, which is also what a file without the line means).
  `start_menu::programs(shown)` builds the menu each time the Start menu opens:
  - **everything**: the whole tree as before (`all_programs`: the two Start Menu folders,
    Windows' Store-style apps, Bevel95's own programs).
  - **classic**: only Bevel95's own programs, in their Accessories > Games. The owner said
    "only old things"; today that is Minesweeper and Solitaire, and it grows with every
    classic program Bevel95 gets. Not in it: today's Notepad, Paint or Calculator (they are
    not old; whether they should stand in for the old ones is the owner's to decide, todo).
  - **ticked**: the whole tree with only the programs that programs.txt lists
    (`Folder::retain`; a folder left with nothing in it goes). Without that file, all of them.
- **programs.txt** (in the data folder, beside settings.txt, which holds one-line settings
  only): one ticked program on each line, named by its place in the menu, the folders above
  it and its own name with "\" between (`programs::place`; no file or folder name can hold a
  "\"). The place rather than the shortcut's path: it is what the owner sees, it is the same
  for a shortcut in either Start Menu folder, and Bevel95's own programs keep it wherever
  Bevel95 is installed. Lines starting with "#" are comments, so a place starting with "#" is
  written with a "\" before it. `Ticked` compares without regard to case, and to spaces at
  either end: a line loses those when it is read, and **some programs' names end in a space**
  (the shortcut file is "Name .lnk"). The owner's own first list had one such program among
  its 12, and that program was missing from the menu until `--list-programs`, which now
  counts what each choice shows, said "12 listed, 11 shown".
  - It is a list of what is wanted, not of what is hidden ("check each one they want"): a
    program installed later has no tick and does not show. Windows adds apps of its own with
    its updates, and a menu put together by hand should stay as it was put together.
  - Written by Bevel95 Settings on OK or Apply while "ticked" is chosen: when a box was
    flipped, when the choice is new, or when there is no file yet (`save_ticks`). With another
    choice the file is left alone, so the ticks are there again the next time.
- **The page** (`Page::Programs`, Bevel95 Settings' third tab, "Programs"; ours): a group
  "The Programs menu shows" with three option buttons (Everything on this PC / Only Bevel95's
  own classic programs / Only the programs ticked here:) and under them one list of every
  program found, laid out as the menu has them: "Programs" first, each folder followed by
  what is in it, one step further in (`programs::rows`; a folder with no program anywhere in
  it has nothing to tick and gets no row). Each row: a box, the picture, the name.
  - What the boxes show follows the choice (`programs::tick`): with everything, all ticked;
    with the classic programs, Bevel95's own; with the third choice, the owner's ticks.
  - A folder's box (and "Programs", for everything) shows what its programs add up to: ticked,
    empty, or a tick on gray when only some are (`Tick::Mixed`). Flipping it ticks all of them
    unless all were (`programs::toggle`), so one click on "Programs" empties every box.
  - Keys and mouse as in Windows' lists: a click on a box flips it, elsewhere on the row
    selects it; arrows, Page Up/Down, Home, End; Space flips the selected row; a letter goes
    to the next row of that letter (in the list a plain letter is no access key); the wheel
    turns three rows a notch; the scroll bar's buttons, track and thumb. A held scroll button
    scrolls on after a quarter of a second, at every 50 ms tick (`LIST_TIMER`, Bevel95
    Settings' own timer; the window host had no repeat for held buttons).
  - **A tree since the same day** (the owner: "the classic +/- ... it's currently a pain to
    scroll through the massive list"). The folders open and close as in Windows' trees
    (`Row::open`; `programs::visible` are the rows that show). It starts with "Programs" open
    and its folders closed, as short as the menu's first column (about 150 rows instead of
    374 on the owner's PC: 65 folders, 87 programs). A click on a folder's plus/minus box, a double-click on its picture or
    name, Right and Left (Right on an open folder goes in, Left on a row goes to its folder),
    "+", "-" and "*" (the folder with every folder in it) do what they do in Windows. A
    selection inside a folder that closes goes to the folder. Everything right of the check
    box selects the row (Windows: only the name itself).
  - **Each choice shows what it is about** (`programs::reveal`, when the rows are read and when
    a choice is made). The classic programs: the folders that hold Bevel95's own open, all
    others close, the first of those programs scrolls into view (the owner: "auto expand just
    those so a person can see what subset is classic right away"). The ticked ones: the
    folders with some programs ticked and some not open; a folder with all or none ticked says
    so with its box and stays as it is. Everything: nothing changes.
  - **A box flipped while another choice is made starts from that choice** (the owner: "use it
    as a base to add more"): what the choice shows becomes the ticked programs
    (`programs::adopt`), the choice becomes "Only the programs ticked here", then the box
    flips. So the list is never grayed and always takes clicks: choose the classic programs,
    tick Calculator, and the menu has the three. Choosing a preset alone leaves the ticks made
    before untouched until a box is flipped. (Windows' own Performance Options work this way:
    presets, and "Custom" as soon as a box changes.)
  - The rows are read when the page is first shown, on that same timer's first tick, not when
    the window opens (the window says "Looking for programs..." until then): finding every
    program takes about 20 ms once the Start menu's pictures are read ahead (374 rows on the
    owner's PC), but 2.5 to 3 s when they are not yet (the first seconds after the bar
    starts). Windows' list of apps is taken as last read. After a change of look they are read
    again, for the look's pictures of Bevel95's own programs, and the ticks made in the window
    stay (`Properties::refresh_programs`).
  - Windows 2000's Start Menu Settings list shares the scroll bar code now (`ScrollBar`:
    thumb, parts, dragging, painting; its golden pictures and compare-refs cases did not
    change by a pixel).
- **The tree's look is Windows' own** (`core::tree`), read on the Reference PCs the same day:
  the "Remove Shortcuts/Folders" window (Taskbar Properties > Start Menu Programs > Remove...;
  2000: Advanced > Remove...) is a tree of the very Programs menu. Captures
  `vm/*-truecolor-remove-tree-*.png`; the measures are in theme-spec.md ("Trees").
  - The three Windows agree in every measure (rows of 16, steps of 19, the 9×9 plus/minus box
    under the picture of the folder above, dotted lines on even rows, the label 19 after the
    picture, the selection with its two-colored focus rectangle) and in how the tree begins:
    with an outermost row, "Start Menu", that has neither lines nor a box (our first row,
    "Programs", is drawn that way in every look). They differ in their pictures: 98's and
    2000's trees show a 16-color folder picture that their menus do not (`icons::tree_group`,
    cut from the capture), 95's tree has its menu's; "Start Menu" is a yellow folder in 95
    and a gray one in 98 and 2000 (`icons::tree_start_menu`).
  - **Corrected the same day**: the first reading had Windows 95's tree begin with "Programs",
    with lines and a minus box of its own (`TreeStyle::lines_at_root`, and the 95 look's
    Programs page drawn that way for two hours). That capture
    (`win95-truecolor-remove-tree-2.png`) was scrolled by one row: opening Accessories had
    pushed "Start Menu" out of sight, and the scroll bar's thumb (111 of the track, 9 down)
    says so: 22 rows, the first one above the tree. 95's first capture shows "Start Menu".
    Lesson: before reading a list's first row as its first, read its scroll bar.
  - Windows' trees of the Programs menu had no check boxes. Ours stand where Windows' trees
    put theirs, between the lines and the picture, 16 wide. Each look's box is one that
    Windows had: 98's and 2000's flat box of 13 (98: the tree of Folder Options, View; 2000:
    Start Menu Settings); for 95, whose trees had none, the box of 11 with a gray frame from
    its one list with boxes (Add/Remove Programs, Windows Setup; captures
    `vm/win95-truecolor-windows-setup-list.png` and 98's). That list also settles the half
    ticked box: Windows Setup's "shaded" box is the tick on C0C0C0. The first version had
    guessed the sunken dialog check box for 95 and 98: no list of theirs used it.
  - compare-refs, all with 0 differing pixels: the trees as part of the whole windows they
    were read from (the 16 `vm-*-remove-*` cases of "Remove Shortcuts/Folders" below) and the
    two kinds of check box where the captures have them (`vm-win98-tree-check-boxes`,
    `vm-win95-list-check-boxes`).
  - PROVISIONAL (ours): the page and its wording; a check box and a tree's lines in one row
    (each measured, their combination ours); the box of 11 standing 3 in and 3 down in its
    place of 16; a half-ticked box in the flat style (98, 2000: gray inside, by analogy).
- **Accessories keeps its old name** (the owner, the same day). The folder is still called
  "Accessories" on the disk; today's Windows only labels it "Windows Accessories" through its
  desktop.ini. `start_menu::read_into` shows a Programs folder of that name under it, and
  Bevel95's own programs go into it (`ACCESSORIES`), so there is one Accessories with Games in
  it, as in 95, 98 and 2000, beside whatever Windows keeps there (on the owner's PC three
  programs and System Tools). A programs.txt written the first day names the folder by
  Windows' label: `Ticked::with_folder_renamed` carries those ticks over when the file is
  read (the owner's own list: 12 before, 12 after).
- The choice is one of `Values` (`programs`), Bevel95 Settings' own (`Sheet::merge`); the
  rows are the window's (`ProgramList`), not a value: only the window that has the list puts
  ticks into effect.
- Proof: unit tests for the rules (`programs`), the page (`properties`: choices, ticks,
  scrolling, finding, clicks and keys) and the settings line; 6 new golden pictures (the page
  as it opens and with ticks, per look; the 6 older Bevel95 Settings pictures changed by the
  third tab only). Live: the owner used the page before any test of mine could (2026-11-11,
  14:02 to 14:05: opened twice, chose the classic programs, then ticked 12 of 298), which the
  log shows working: list read in 23 ms, both choices saved. `--list-programs` on the same
  data: 298 programs with everything, 2 with the classic ones, 12 with the ticked ones after
  the fix above. `private/probes/programs-page-live.ps1` is the real-input test of the page. Its
  harmless part was run at an idle moment (`-LookOnly`: the page opened by a message and
  Ctrl+Tab, photographed, turned with the wheel, closed with Esc): the real 374 rows with
  their pictures, the owner's 12 ticks adding up to a half-ticked "Programs", six rows
  further after two notches, nothing changed on disk. Its other parts (each choice put into
  effect and the real Programs menu photographed) were not run: they start from "everything"
  and put that back, and by then the owner's own list was in use. After the tree: the unit
  tests rewritten for it (folders opening and closing, the choices revealing, a box flipped
  from a preset), 9 golden pictures of the page (as it opens, the classic choice, every folder
  open with ticks; per look), the six reference cases above, `--list-programs` on the owner's
  data (one Accessories folder, 12 of 12 ticks found), and the same harmless look at the live
  page: the owner's tree with its folders closed, real pictures, the half-ticked "Programs".
  Not tried live by Claude: clicks on the plus/minus boxes and the tree's keys (the owner's
  checklist has them).
- **Remove Shortcuts/Folders** (the owner, the same day, on seeing the capture the tree was
  read from: "what I thought of was already an original feature ... perhaps we should just
  put it there since it's essentially a faithful feature"; then "OK build it then").
  Taskbar Properties' **Remove...** (second page) opens Windows' own window instead of a
  File Explorer folder: `core::remove` (`RemoveWindow`, a form of the control kit),
  `platform/win/src/remove_window.rs` (`RemoveHost`). The Programs page stays: this window is
  the old way in, that page the full one.
  - **The window**, the same in all three Windows (theme-spec.md, "Remove Shortcuts/Folders"):
    324×350, 33 right of and 50 below Taskbar Properties' corner; two lines of text; the tree
    in a sunken frame, 13 rows and part of a 14th, a scroll bar once more rows show; Remove
    and Close. The tree is "Start Menu", under it "Programs" with what Bevel95's Programs
    menu shows now, Programs open and selected, its folders closed. Each program's picture
    has the shortcut arrow in its lower left corner (`icons::SHORTCUT_ARROW`, 7×7, the same in
    95, 98 and 2000), which the Start menu itself never showed.
  - **The focus**, read on the Reference PCs with `private/probes/remove/focus-probe.ps1`
    (captures `vm/*-truecolor-remove-focus-0..2.png`): Remove is grayed out while "Start Menu"
    or "Programs" is selected; Close is the framed button while the tree has the focus, and
    the focused button is the framed one otherwise. Unlike a plain tree, this one keeps its
    selection in sight while a button has the focus: on the window's gray, in the usual text
    color, without the focus rectangle (`TreeRow::focused`). Bevel95 Settings' tree is not
    changed by this: its selection shows only with the focus, as before.
  - **Remove hides, it does not delete** (agreed with the owner before building). Windows sent
    the shortcut file to the Recycle Bin, after asking. Here the selected program, or every
    program in the selected folder, leaves what Bevel95's Programs menu shows
    (`programs::hide`, `start_menu::hide_programs`): what the menu shows now becomes the ticked
    programs without these (programs.txt), and the menu shows "only the ticked ones" from then
    on (`bar::set_programs_shown`; a menu that showed everything or the classic programs
    switches to that choice). Nothing on the disk is touched, so there is no question first;
    Bevel95 Settings' Programs page lists the program again, unticked. An open Bevel95
    Settings follows at once (`Properties::programs_removed`), so its OK does not put the
    program back. In the tree the row goes, a folder left empty goes with it, and the
    selection passes to the row that came after.
  - **A list ticked by hand is never lost to one click** (`start_menu::save_ticked_programs`).
    Remove while the menu shows everything or the classic programs starts a new list from
    that choice, as a box flipped on the Programs page does, and programs.txt is replaced at
    once, with no Cancel. So whenever a list is written while the menu showed another choice,
    and it differs from the one on the disk, that one is first kept as programs.old.txt (one
    copy; Bevel95 never reads it; README, "Files it uses"). The owner asked whether it "just
    sits there": yes, and it stays so. It is written only when a hand-ticked list is replaced
    by one started from another choice, there is ever only one, and nothing reads it; Setup's
    Remove deletes it with programs.txt (both were missing from its list of data files until
    2026-11-11, so the data folder stayed behind; `install::DATA_FILES`, whose test checks
    every data file named in the source against it and README). Found by the live test: the owner
    had chosen the classic programs at 15:08 with 12 hand ticks on the disk, and the test's
    Remove wrote an empty list over them (the test put both files back byte for byte).
  - Taskbar Properties waits while the window is open (`kit::open_dialog`: a dialog of the
    window host, owned by it and like it above every window). A change of look or size closes
    the window (its pictures belong to the old look); closing Taskbar Properties from outside
    closes it first.
  - The scroll bar is the one of Bevel95 Settings' list and 2000's Start Menu Settings
    (`core::scroll::ScrollBar`, moved out of `properties`); the tree's keys and mouse are
    shared with the Programs page (`programs::Cursor`).
  - Proof: 16 compare-refs cases of the whole window against the Reference PCs, 0 differing
    pixels each: as it opens (3), with folders opened by keyboard and the scroll bar there
    (4: 95 scrolled by a row, 98 twice, 2000), and the three focus states per Windows (9).
    Only the PCs' own program pictures are masked, not their arrows. Unit tests for the tree,
    Remove, clicks, keys and scrolling (`remove`), for hiding (`programs`); 3 new golden
    pictures (`*-remove-shortcuts`), and the 3 Windows 95 pictures of the Programs page changed
    with the correction above. The full compare-refs run: 353 cases, none with a differing
    pixel.
  - Live (`private/probes/remove-live.ps1 <folder> -Remove`, real keys on Bevel95's own windows at
    idle moments, 2026-11-11, the 98 look at size 2, the owner's menu showing the classic
    programs): Taskbar Properties by a message to the bar, Ctrl+Tab, Alt+R: the window opened
    648×700, 66 right of and 100 below Taskbar Properties' corner, owned by it and above every
    window, with the keyboard, Taskbar Properties disabled. Down, Right, End and Right to the
    last program, Tab (the selection on gray, Remove framed), Alt+R: one program taken out,
    programs.txt listing the one left, the owner's 12 hand ticks kept as programs.old.txt, the
    next row selected, the window still open. Esc closed it, Taskbar Properties had the
    keyboard again, Esc closed that. The test then put settings.txt and programs.txt back
    byte for byte, removed the safety copy it had caused and restarted the bar to read the
    setting again. Its first run (before the safety copy existed) removed the Games folder
    instead of one program, because End stopped on a closed folder: the tree went down to
    "Start Menu" and "Programs", Remove grayed out and the focus returned to the tree, as it
    should. Not tried live by Claude: the mouse on this window (clicks on rows and boxes,
    the scroll bar, a click on the waiting Taskbar Properties) and a change of look while
    it is open; the owner's checklist has them.
- **Add...** (2026-11-11, the owner: the button did nothing). Taskbar Properties' Add... starts
  Windows' own Create Shortcut wizard in your Programs folder, as Windows 95 to 2000 did
  (`platform/win/src/add_shortcut.rs`).
  - **Why it did nothing**: it ran `rundll32.exe appwiz.cpl,NewLinkHere "<Programs>\New
    Shortcut.lnk"`. The wizard opens only when the file it is given already exists (an empty
    placeholder: Explorer's own New > Shortcut does this, `HKCR\.lnk\ShellNew\Command` is
    `rundll32.exe appwiz.cpl,NewLinkHere %1`) and when the path comes without quotation marks
    (the rest of the command line is the path, spaces and all). Either wrong, rundll32 ends
    without a word.
  - **Now**: an empty "New Shortcut.lnk" is made (made new, never over a file: else "New
    Shortcut (2).lnk" and so on), and rundll32 started on it with CreateProcessW, keeping its
    process. A thread waits for it and posts the bar `WM_SHORTCUT_WIZARD_ENDED`. Then the
    placeholder, if still there and 0 bytes, is removed (a file with anything in it never),
    and the files of the menu's kinds (lnk, url, exe, appref-ms, pif) directly in the Programs
    folder are compared with those before (`start_menu::program_files`). Several wizards may be
    open at once; each has its own placeholder and list.
  - **What the wizard does with the placeholder**, tried on this PC in a scratch folder
    (`private/probes/add-wizard.ps1`, messages only, to the wizard it started itself, at idle
    moments): on Cancel it removes the placeholder itself and nothing is left; on Finish
    (target `notepad.exe` put in by WM_SETTEXT, Next and Finish by PSM_PRESSBUTTON) it writes
    a new file under the chosen name ("notepad.exe.lnk", 1372 bytes) and removes the
    placeholder. Bevel95's own removal is so only a guard. Today's wizard is an Aero wizard
    (class NativeHWNDHost) and has no "Select Program Folder" step: the shortcut always goes
    into the folder of the placeholder, Programs itself.
  - **What it made joins the menu whatever the menu shows** (`start_menu::show_programs`, the
    mirror of `hide_programs`; `programs::unhide`, the mirror of `hide`). The new file's place
    in the menu is found by its path (`start_menu::places_of`). With everything it shows
    anyway, and with the ticked ones while there is no programs.txt yet (all count as ticked):
    nothing is written. With the ticked ones, its tick is added. With the classic programs,
    what they show becomes the ticked programs with the new one (programs.txt, with Remove's
    safety copy of a hand-ticked list), and the menu shows only the ticked ones from then on.
    Windows wrote the shortcut and its menu showed it; the owner asked that it show here too.
  - Open windows follow: Bevel95 Settings (and Taskbar Properties, for the choice) reads its
    list again if it was read, keeping its ticks, and with the ticked choice in effect shows
    that choice and the new program ticked (`Properties::programs_added`), so its OK does not
    take it out again. An open Remove Shortcuts/Folders window (it can be opened while the
    wizard runs) reads its tree again, folders and selection kept (`RemoveWindow::set_rows`).
  - No new windows-sys feature (CreateProcessW and WaitForSingleObject are in
    `Win32_System_Threading`, GetSystemDirectoryW in `Win32_System_SystemInformation`).
  - Proof: unit tests of `unhide`, `programs_added` and `set_rows`; the two scratch-folder runs
    above. Not tried live by Claude: Add... in the bar itself with the owner's real Programs
    folder (it would make a real shortcut there); the owner's checklist has it.

## Fixes from the owner's daily use (2026-11-11)
- **A button's list closes at a second click** (as Windows XP's grouped button did). The press
  closed the list as a click outside it, then the release opened it again. Now the press
  remembers whether that button's list was open (`popup::window_list_open`; the list's windows
  are compared with the button's, one shared window is enough) and the click ends there, the
  way the Start button and the speaker already did. What a click on a task button does is
  decided in the core (`tasks::click`, `ClickState`), with tests. Live-tested with a test
  program of our own (`private/probes/bugs-live`: two windows with a red icon, found on the bar by
  its pixels, real clicks at idle moments): before the fix the second click left the list
  open, after it four clicks went open, closed, open, closed.
- **A minimized window's button always brings it back.** A click minimized the window when
  `app.active` (the foreground window as the tick last read it) was that window. But a
  minimized window can still be Windows' foreground window, when nothing else took its place,
  and then the click "minimized" it again and nothing happened. Reproduced with the test
  program: a window minimized with SW_SHOWMINIMIZED stays the foreground window; its button did
  nothing on the old build. Now `tasks::click` gets `ClickState::minimized` and a minimized
  window is never "in front", and both the window in front and IsIconic are read at the click
  itself (not one tick old). With the fix the same three clicks restored, minimized, restored.
  The list of a group's windows always brings the chosen window forward and the window menu's
  Restore sends SC_RESTORE, so neither had the mistake.
- **Evidence for the next time**: half a second after a click brought a window forward (its
  button or its list), the tick checks that it is in front and not minimized; if not, the log
  says "a click's window is not in front 500 ms later" with its window class, its program
  file and the class of what is in front, never a title (`bar::check_activated`).
- Not done: handing the activation to the window's last active popup (GetLastActivePopup)
  when the listed window is disabled by a dialog. Windows keeps an owned dialog above its
  owner anyway, so nothing showed that it matters; the new log line would show it if it does.
- **Windows' Wi-Fi list closes at a press on the bar** (`core::flyout`, `bar::close_flyout`).
  Found live (`private/probes/bugs-live`: `barclick.ps1`, `flyouts.ps1`, read-only watchers and
  real clicks at idle moments on the bar and on our own test window): a click on the network
  icon makes ShellHost.exe's window of class "ControlCenterWindow" the window in front about
  470 ms later (it is visible only while the list is open). While it is in front Windows
  refuses SetForegroundWindow to the bar's process, so a task button's click left the list
  open and its window behind (log: "SetForegroundWindow refused" and the new "not in front"
  line). Even with the input joined, another process of ours was refused; the bar itself, at
  the press, gets the front with its input joined to the flyout's ("Joined" in the log).
  - Now every press on the bar (left, right, middle) and on the classic desktop first asks
    whether one of Windows' flyouts is in front: ShellHost's "ControlCenterWindow" (the Wi-Fi
    list, quick settings) or a "Windows.UI.Core.CoreWindow" of ShellHost or
    ShellExperienceHost (the notification center, Safely Remove Hardware). If so the bar takes
    the front (`util::bring_forward`, the fallback `take_focus` always had), Windows closes
    the flyout, and the click goes on: a task button's window comes forward. The bar then stays
    the window in front, as Windows' own taskbar does after a click on it. The desktop asks the
    bar to take the front, since it must never come forward itself.
  - A press on the network icon while its list was open only closes it
    (`flyout::icon_press_acts`), as on Windows' own taskbar.
  - Live, on the fixed build: list open, a click on the test window's button: the list closed
    (ControlCenterWindow hidden) and the test window was in front; list open, a click on the
    network icon: closed and stayed closed.
  - `window_list::activate` now falls back the same way when SetForegroundWindow is refused,
    and logs which way it went with the window's class (it logged the title before).
  - What a list opened by another process without the front looks like: ShellExecute of
    "ms-availablenetworks:" from a process that did not get the last input opens the list
    without making it the window in front, and then nothing closes it (clicks on other
    windows, other windows coming forward) until it is in front once. The bar always opens it
    at a click, so this does not happen there.
  - Not done: the popups of programs' own notification icons. They are the programs' windows
    and Windows hands them the front at our press (AllowSetForegroundWindow); whether one stays
    open after a click on the bar was not seen.
- **Windows' real desktop flashing (Start, Win+D)**: found and fixed later the same day for
  Win+D (section "Win+D and the classic desktop" below; the owner found the Windows key fine
  by then). What was measured first
  (2026-11-11, desktop switched on for the test and settings.txt put back byte for byte after):
  - `private/probes/bugs-live/zwatch2.ps1`, a read-only watcher looking every 5 to 9 ms at whether
    the classic desktop exists, is visible, is above Windows' desktop and what is between, and
    what is in front. Start opened and closed by real clicks on the Start button: nothing
    changed. What Windows sees for a lone Windows key rebuilt from outside (Windows key down,
    the bar's own WM_START_KEY, 0xE8 and the release, `winkey-sim.ps1`, with our test window in
    front): our Start menu opened; two hidden helper windows of another process came between
    the two desktops for about 90 ms, under ours, so nothing showed. The real Windows key
    cannot be sent (the hook ignores injected keys) and Win+D cannot be tried without
    minimizing the owner's windows. Raising Windows' desktop window from outside to imitate
    Show desktop did nothing: it keeps itself at the bottom.
  - Found on the way: Windows announces the classic desktop as a fullscreen window
    (HSHELL_FULLSCREENENTER with class Bevel95Desktop, every time it appears, about 250 ms
    later), although it carries `NonRudeHWND`. The bar leaves it out of its own fullscreen
    rule, but Windows itself treats a fullscreen window specially; whether that is behind the
    flash is not known.
  - The log could not tell: until now its one line for "out of place" ("another window had
    come between it and Windows' desktop") also stood for Windows' desktop coming up over it.
    In the minutes before the owner switched the desktop off it was put back at least seven
    times within 9 seconds.
- **What changed**, so that the next flash leaves evidence and the known part is faster:
  - `desktop::check` replaces `tick`'s test: the window must be visible, not minimized,
    directly above Windows' desktop and as large as the screens. If it is not, the log says
    why, with its own count per reason (each logged the first three times, then the 10th,
    100th, ...): hidden or minimized (it is shown again with SW_SHOWNOACTIVATE, never taking
    the focus), "Windows' desktop had come up over it" (Windows' desktop showed), another
    window between (a hidden helper by its class, a program's window only as such), or the
    screens changed; and whether the tick or a Windows event found it, and if the tick came
    more than 300 ms after the one before (the bar was busy).
  - It no longer waits for the tick: window events (SetWinEventHook, out of context, so
    nothing runs in another program; windows-sys feature `Win32_UI_Accessibility` added) for
    another window coming to the front, the order of windows changing, and our window hidden
    or minimized run the same check at once. The hooks go with the window.
  - The shell's fullscreen lines now name the window's class.
  - Kept: it never takes the focus, nothing needs restoring after a crash, it is not topmost
    (put back if it becomes so), it stays under every other window.

## Find (programs) (2026-11-11, the owner: "just to start maybe it can just search programs")
- Since 2026-11-16 the window is **Quick Search** and also finds things to do: see "One "Bevel95"
  entry and Quick Search" below. What follows is its history as first built.
- What the owner asked for, their last functional blocker: a small window, opened by the
  Windows key, in which a few letters typed and Enter start a program ("not pixel perfect";
  "default should be windows key with bevel option to assign that to start instead"). Their
  sketch: a window titled Find, a "Named:" field, a list with a header, a status bar.
- **The window is Bevel95's own** (no classic Windows had one): `core::find` (`FindWindow`, a
  form of the control kit) and `platform/win/src/find_window.rs` (`FindHost`). PROVISIONAL in
  its wording and numbers, matched to no capture, built from the kit's parts in the look's
  style (theme-spec.md, "Find (programs)"): the title "Find: Programs" in the 95 and 98 looks,
  "Search: Programs" in the 2000 look (each look's word, as its Start menu has it), with the
  look's small Find or Search picture; Close only; movable, not resizable. "&Named:" with the
  Run dialog's field; a list with a header of two plain raised cells, "Name" and "In Folder"
  (the columns of Windows' Find results; no sorting), each row a 16×16 picture, the name and
  its folder ("Programs", "Programs\Accessories", ...); 10 rows, `core::scroll`'s scroll bar
  when there are more; a status bar "N program(s) found" (Windows' Find wrote "N file(s)
  found"). 330×264 at 1×; it opens where Run opens (12 right of the screen's edge, 12 above
  the bar).
- **It is not above every window**, unlike the bar's other windows (a kit `Tool` window is
  topmost; Find is set back right after it opens): another program brought forward covers it,
  and the Windows key brings it back. It has no taskbar button.
- **What it lists** (`find::programs`; since 2026-11-18 only the programs among them unless
  `quick_search = everything`: "Quick Search lists only programs by default" below): every program the Programs menu could show with
  "everything" chosen (`start_menu::all_programs`: both Start Menu folders' shortcuts,
  Windows' Store-style apps, Bevel95's own programs). The owner's choice of what the menu
  shows (classic, ticked, programs.txt) does not narrow it. One row per program; the same name
  in the same folder shows once.
- **How it finds** (`find::find`, unit-tested): case-insensitive; the typed text is split at
  spaces and every word must be in the name; rows rank by (a) the name begins with the text,
  (b) a word of the name begins with it (a word starts after any mark that is no letter or
  digit: "MS-DOS Prompt" has the words MS, DOS, Prompt), (c) the rest; A to Z within a rank,
  then by folder. Nothing typed: every program, A to Z. After every change of the text the
  first row is selected.
- **Keys and mouse**: the text field keeps the focus (Tab stays in it). Up, Down, Page Up and
  Page Down move the selection (a page is 9 rows, as Windows' lists turn one row less than
  they show); Left, Right, Home, End, Delete, Backspace and Ctrl+V edit the text. Enter starts
  the selected program and closes the window; Esc closes it. A click on a row selects it, a
  double-click starts it, the wheel turns three rows a notch, the scroll bar works as in the
  Remove Shortcuts/Folders window (a held button scrolls on after a quarter of a second).
  Reopening shows an empty field.
- **Starting** is exactly the Programs menu's way: `window_list::launch` of the entry's target,
  while the window, already off the screen (an empty window region, as the menus `vanish`),
  still has the focus, so the program may come to the front; then the window closes.
- **Speed**: the window appears at once with the list as last read (`PROGRAMS` in
  find_window.rs, about 300 programs here); each opening reads it again on a thread of its own
  ("find programs") and Windows' list of apps too (`apps::refresh`), and an open window takes
  the new list when it comes (`WM_FIND_PROGRAMS_READ`: text, pictures and selected program
  stay). Only the very first opening reads the list before showing (the Start menu does that at
  each opening). Pictures are fetched only for the rows that show (`wanted_icons`), from the
  Start menu's store of them (`start_menu::file_icon`, read ahead after the bar starts), on the
  window's 50 ms timer and before it opens. The log says "opened Find: N programs (ready in N
  ms)": 4 to 8 ms here, 33 ms the first time.
- **The Windows key** (`windows_key = find|start` in settings.txt, `config::WindowsKey`,
  default `find`, which a settings file without the line also means). With `find` a lone press
  opens Find; if Find is the window in front it closes; if Find is open but behind, it comes
  forward. An open menu of ours (and the volume control) closes first. With `start` the key
  opens the Start menu as before. Ctrl+Esc always opens the Start menu. The keyboard hook is
  untouched: only what the bar does with `WM_START_KEY` changed (wParam is the Windows key, 0
  for Ctrl+Esc, as before).
- **Bevel95 Settings, General page**: a group "Windows key" with "Opens &Find" ("Opens
  Sea&rch" in the 2000 look) and "Opens the Start &menu", applied with OK and Apply like the
  page's other settings (`Values::windows_key`, Bevel95 Settings' own in `Sheet::merge`). The
  page had no room below its four groups, so the least disruptive place was beside Look: Look
  narrowed to 140 wide (its labels need 90), the new group 148 wide 8 to its right, the same
  height; nothing else on the page moved. The three golden pictures `*-bevel95-settings`
  changed by that. (Later the same day the group moved, unchanged in wording, to a page of its
  own, Taskbar, and Look got its old width back: "Task buttons: grouping and dragging" below.)
- **A way in by mouse**: Start > Find (Search) ends with Bevel95's own entry "&Programs...",
  with the look's small Programs picture (`Target::FindPrograms`; the bar opens Find once the
  menu is gone). Live (`private/probes/find/submenu-live.ps1`, the 2000 look): the Start menu by
  the bar's own Ctrl+Esc message, the keys C (Search) and P: Find opened in front; Esc closed
  it (the bar was then the window in front). No golden picture shows the Find submenu, so none changed. The compare-refs
  cases of the Find and Search submenus compare the captured rows only (`ours_after`); before,
  Windows 95's case also compared the whole menu.
- **A bar command for scripts**: `BarCommand::Find`, code 6, at the end of `BarCommand::ALL`.
- PROVISIONAL (ours): everything about the window's look and wording, its place, the Windows
  key group's wording and place, the submenu entry's wording.
- Proof: unit tests (`find`: listing, ranking, typing, keys, paging, scrolling, clicks, the
  pictures wanted and kept on a new reading; `properties`: the new group; `config`: the
  setting), three new golden pictures (`*-find-programs`, "p" typed), the window photographed
  live in the 98 look at 2× and compared with its golden picture where neither depends on the
  programs found (title bar and frame, "Named:", the Name header cell, the right edge: 0
  differing pixels; `private/probes/find/regions.ps1`). Live (`private/probes/find/find-live.ps1`, at
  idle moments, real keys only while Find was the window in front): Find by the bar command,
  "mines" typed, Minesweeper (Programs\Accessories\Games) the one row, selected; Esc closed it;
  then again with Enter: Find closed, a new Minesweeper started in front and was closed again.
  The bar's handling of the key, by its own message (`winkey-msg.ps1`, no key pressed): the
  Windows key opened Find, again closed it; Ctrl+Esc opened the Start menu; the Windows key
  then closed the menu and opened Find; again closed it. Not tried by Claude: the real Windows
  key (the hook ignores injected keys) and the mouse on the window (owner's checklist).

## Task buttons: grouping and dragging (2026-11-11, the owner's requests)
- **A switch for grouping.** Windows 95, 98 and 2000 never grouped: one button per window.
  Windows XP brought grouping, with the check box "Group similar taskbar buttons" on its
  taskbar page. Bevel95 grouped always, in every look (`core::tasks`); now it is a setting,
  `group = yes|no` in settings.txt (`Config::group`, a yes/no flag like the others; default
  yes, which a settings file without the line also means: what the bar did before).
  - Off (`TaskList::group` false): every window is a button of its own, with its own title and
    picture, in the order the windows appeared; buttons shrink and scroll arrows come as
    before. The button's key is the program's key plus the window's id, so nothing else
    changed: a click is `tasks::click` with one window (forward, or minimized if it is in
    front and not minimized), so no list of windows ever opens, and the right-click menu is the
    window's own. Switched on again, the same windows share their program's button, in the
    same order (both in the core's tests).
  - It is Bevel95's own setting, so it lives in Bevel95 Settings, on a new second page
    **Taskbar** (`Page::OwnTaskbar`; tabs General, Taskbar, Desktop, Programs): the group
    "Windows key", moved there from the General page unchanged in wording, and a group
    "Buttons" with "&Group similar taskbar buttons" (Windows XP's wording) and one line, "Off:
    every window has a button of its own." Placed in the General page's manner
    (theme-spec.md). It applies with OK and Apply (`Values::group`, Bevel95 Settings' own in
    `Sheet::merge`), and the bar follows at once (`apply_properties` sets the task list's
    switch and reads the windows again).
  - The General page went back to its layout from before the Windows key group (Look 296
    wide): below the row of tabs its three golden pictures are pixel-identical to commit
    abadbdc's (every differing pixel lies in y 31..48, the tabs). The Desktop and the three
    Programs pictures differ from before only in y 29..49 (the new tab). Three new golden
    pictures `*-bevel95-settings-taskbar`.
  - PROVISIONAL (ours): the tab's name, the group "Buttons" and its line of text.
- **Dragging a task button into a new place** is Bevel95's own extra: Windows 95, 98, 2000
  and XP could not; Windows got it with Windows 7. So how it looks and its timing are ours
  (PROVISIONAL), modelled on Windows 7's taskbar and browser tabs. It works with grouping on
  (the program's button moves, all its windows with it) and off (a single window's button
  moves).
  - **Rebuilt 2026-11-12** (the owner: the first version "snapped after you drag some random
    amount"). In the first version the button stayed in its slot, pushed in, and jumped to a
    neighbour's slot once the *pointer* passed that neighbour's middle: how far that was
    depended on where on the button you had pressed. Now:
  - Starting is unchanged: a press that moves sideways further than Windows' drag threshold
    (SM_CXDRAG at 96 DPI, 4, times the bar's scale; up and down do not count) becomes a drag;
    below it everything is as before (the click acts on mouse-up over the same button).
  - From then on the button **follows the pointer**: the point of it that was pressed stays
    under the pointer, 1x pixel for 1x pixel (`ButtonPress::grab`), and it stops at the row's
    ends. It is drawn over the others, in the look it had before the press (raised, or sunken
    and checkered if it is the window in front; its checkerboard moves with it), not pushed in.
  - It **takes the place its middle is over**; the line between two places is the middle of
    the gap between them (`tasks::drag_step`). So a neighbour makes room once the dragged
    button covers more than half of it, the same distance wherever it was pressed. The brief
    said "when the dragged button's middle passes the neighbour's middle"; taken literally
    that is a whole button's width, the neighbour fully covered before it moves, so the
    halfway line was chosen (as browser tabs and Windows 7 do), with the owner's look to
    decide.
  - The others **slide**: a button whose place changes moves there from where it is drawn in
    100 ms (`tasks::SLIDE_MS`), quick at first and gentle at the end (quadratic ease-out),
    in whole 1x pixels, then scaled as everything is. A slide that is turned round (the
    pointer goes back) starts again from where the button is, so nothing jumps. A quick move
    past several buttons slides them all at once. At the release the dragged button slides
    from where it is into its place and the new order stays; it is no click. Esc, or losing
    the mouse capture, puts the order from the press back and every button slides there.
    `tasks::Motion` does this frame by frame: it is told which button (by its key) is in which
    place and where the dragged one is, and remembers where each was drawn.
  - **Repainting**: the bar's 100 ms tick is too coarse, so while a drag or a slide lasts, and
    only then, a timer of 15 ms (`MOTION`) repaints the task area, besides every move of the
    pointer; it stops once everything is at rest (the log then says how many frames and timer
    steps it took). Windows rounds the timer to its own pace: measured 396 steps in 6.5 s,
    about 16 ms, one frame of a 60 Hz screen. A cleaner way (waiting for the screen's frame,
    DwmFlush, on a thread of its own) was not needed. The bar's picture is made whole at 1x as
    always (quick) and only the task area reaches the screen. A task button now fills its own
    face before its edges, so one drawn over another hides it (no golden picture changed).
  - **Windows coming and going during a drag**: the pressed button is found again by its
    windows whenever the buttons are rebuilt; if they all closed, the press ends (logged). If
    the places change (a button more or less, so another width), nothing slides that moment:
    every button is put at its place at once, and the dragged one takes the place its middle is
    over then (`drag_move` again with the last pointer position). A new window still joins at
    the end; a changed title moves nothing. The row shown does not follow a window coming to
    the front while a button is held.
  - The rules are in the core (`tasks::ButtonPress`, `tasks::drag_step`, `tasks::Motion`,
    `TaskList::move_button`, next to `tasks::click`) with tests: start, follow, the row's
    ends, each swap in both directions with its slide, a quick move over several, a slide
    turned round, release, Esc or the mouse lost, windows closing, opening and retitled
    mid-drag, grouping on and off. Golden pictures `*-drag` (new; no other picture changed):
    a drag in progress in each look, the dragged button over the active one, which is halfway
    through its slide. Only the row shown counts (with scroll arrows: no scrolling while
    dragging).
  - For live tests the bar answers `WM_TASK_AT` (WM_APP + 16, sent with a point in the bar
    window): the handle of the first window of the button there, or 0. It only answers, so a
    test can make sure the button under the pointer is its own before it presses
    (`private/probes/drag/smooth.ps1`).
  - How the order is kept: the task list's windows are put in the order of the buttons, each
    button's windows together (`move_button`), so the buttons come out in the new order at
    every reading of the windows; new windows still join at the end. The pressed button is
    found again by its windows whenever the buttons are rebuilt; if they all closed, the press
    ends (logged).
  - **Esc** puts the button back (the order from the press). The bar never has the keyboard
    focus, so the keyboard hook (`keys.rs`) takes Esc while a drag lasts (`keys::set_dragging`),
    typed or sent by a program, swallows its press and release and tells the bar
    (`WM_DRAG_ESC`). Losing the mouse capture (another window took it) puts it back too.
  - **Kept across a restart of the bar**: `task-order.txt` in the data folder (the time
    Windows started, then the windows' handles), written at each drop and when the bar closes,
    read when it starts; ignored after Windows restarted (handles are new then). Setup's
    Remove deletes it with the other data files. The log says "task buttons: a drag moved a
    button from place N to M of K" (places only, no titles) and how many windows the kept
    order had.
  - Live (2026-11-11, at idle moments, real input only on our own buttons:
    `private/probes/drag/drag.ps1` with three copies of a test program, `DragA/B/C.exe`, each with
    a button of its own and a solid icon of its own color, found on the bar by those colors):
    A dragged right past B: the order became B A C while dragging and stayed after the
    release, and A did not come forward (no click); dragged back left with Esc pressed before
    the release: B A C again, log "Esc ended a drag"; a press with a 2-pixel move: A came to
    the front (a click). After two restarts of the bar the order was the one kept. What went
    wrong on the way, in the test script, not the bar: its first version found the buttons by
    the average place of every pixel of a color, and stray red pixels in another program's
    icon put that average onto the owner's neighbouring button: two drags moved one of the
    owner's buttons (places only: the second was undone with drags of our own buttons; which
    button the first one moved, from place 7 to 9, the log cannot tell), and one of the
    owner's windows was brought forward by a click. The script
    now finds a solid run of columns of the color.
  - Live after the rebuild (2026-11-12, bar started through Explorer, at idle moments of 8 s,
    `private/probes/drag/smooth.ps1` with DragA/B/C: before the press it asks the bar
    (`WM_TASK_AT`) whose button is under every point across the bar, does nothing unless the
    three are ours and side by side and the press point is A's, and photographs only our
    three buttons). A slow drag (2 screen pixels every 16 ms) of A two places right: A drawn
    over B, then B already sliding left as A passed half of it, ABC became BCA, nothing came
    to the front. The order survived a restart of the bar (the install). A two places left
    with Esc before the release: everything slid back to BCA (caught mid-slide), log "Esc
    ended a drag". A two places left released: ABC. A released 30 pixels short of its place:
    caught sliding into it. Every drag line in the log was within places 8 to 10 of 10, our
    three; no button of the owner's was pressed. Not tried live: grouping off, a window
    closing mid-drag (both in the core's tests), the Windows 98 and 2000 looks (golden
    pictures), a drag with scroll arrows showing.

## Win+D and the classic desktop (2026-11-11, the owner: "brief flash for Win+D")
- **What happened**: Show desktop (Win+D) raises Windows' desktop window (Progman) above every
  window, ours included; neither of the window events the desktop listened to came, so the bar's
  100 ms tick put ours back (the owner's log, three times: "Windows' desktop had come up over
  it"). The gap was the flash. Measured before the fix (`private/probes/desktop-flash/flash.ps1`,
  with the classic desktop on, 2000 look, one Win+D pair at an idle moment, with the owner's
  yes for this fix): the classic desktop showed 20 ms after the press, then Windows' desktop
  lay above it for 97 ms (window order looked at about every millisecond) and the screen
  showed Windows' own desktop in 4 pixel rounds in a row (one round every 27 ms: GetPixel on
  the screen is that slow).
- **The way chosen, the structural one**: the classic desktop is now **owned by Windows'
  desktop window** (Progman, or the WorkerW that holds the icons). An owned window always
  stays above its owner and Windows moves it with it in the same step. First tried with a
  probe window of our own owned by Progman (`owned.ps1`, a Win+D pair): it stayed above
  Progman the whole time, right above it while the desktop showed, visible, never hidden,
  minimized or cloaked; so Show desktop leaves an owned window alone and carries it up. (A
  WinForms Form ignores an owner of another process; a raw window takes it.) The second way
  of the brief, catching an event at once, was not needed: an event comes after the fact, so
  it could only shorten the gap, never close it. A cover inside Progman, as its child, was
  not tried either: Explorer arranges its own child windows.
- **Its own thread**: owning a window of another process joins the input of the two threads,
  so the desktop window lives on a thread of its own ("classic desktop", `desktop::run`): it
  makes the window, listens to the window events and runs `check` there. The bar only posts:
  the color (InvalidateRect), `WM_CHECK` on its tick, `WM_GO` to hide it. A press on the
  desktop is posted to the bar (`WM_DESKTOP_PRESS`: Windows' flyouts, our menus and the volume
  control close), the right button's release too (`WM_DESKTOP_MENU` with the pointer's
  place: the bar opens the menu). The bar's reading of the taskbar windows skips the desktop
  before asking any window for its title (`desktop::is_ours`), so it never waits for that
  thread.
- **Moving with its owner**: until now `WM_WINDOWPOSCHANGING` refused every change of place
  that did not come from `place`. That would also refuse the move Windows makes with the
  owner, so now only a move into the topmost windows is refused; `check` puts anything else
  right, as before.
- **When Explorer restarts**, Windows destroys the owned window with Progman. The window's
  `WM_DESTROY` notices that it was not `hide` ("desktop: its window went with Windows'
  desktop window (Explorer restarted?) ...") and the bar's tick makes it again once a Windows
  desktop window is there again (while there is none: one line, "waiting for it"). Not tried
  live (it would restart the owner's Explorer).
- **Measured after the fix**, three Win+D pairs at idle moments: Windows' desktop above the
  classic one for 0 ms (never, in 1388, 1377 and 202 looks), no pixel round showing Windows'
  own desktop while the desktop was shown (the classic color from 19 to 20 ms after each
  press), and after each pair the same 20 windows visible and none newly minimized. After the
  second press of each pair a few pixel rounds (about 100 ms) show neither the classic color
  nor what was there before: the owner's windows coming back with Windows' animation; the
  same rounds are in the measurement from before the fix, and Windows' desktop was never
  above ours then.
- Kept: it never takes the focus, nothing needs restoring after a crash, it is not topmost,
  it stays under every other window, Win+D shows the classic desktop. The right-click menu,
  checked by posting a right-click and then a left press to the desktop window (no real input:
  every spot of the desktop was covered by the owner's windows): the menu opened, then closed.
  The logging of `check` stays.

## Desktop icons (2026-11-11, the owner's plan, step 1 of 3)
- What the owner asked for: icons on the classic desktop, classic style, "replacements for
  things we have like recycle bin"; no Network Neighborhood. Step 1 (this section): the look's
  own icons and everything on the owner's desktop, selecting and opening them, the keys, staying
  current, the desktop's real right-click menu. Step 3 (moving icons, Auto Arrange, several
  selected) and step 2 (the menu on an icon, built after it) are this section's last two bullets.
- **Which icons** (`core::desktop_icons`): the look's own first, in its desktop's order (the
  Reference PCs): 95 My Computer, Recycle Bin; 98 My Computer, My Documents, Recycle Bin; 2000 My
  Documents, My Computer, Recycle Bin. No Inbox, Internet, Briefcase or Network icons. Then the
  owner's Desktop folder and the public one (`platform/win/src/desktop_items.rs`), without hidden
  and system files; Windows' own This PC and Recycle Bin are not files, so they never show twice.
  Names are Explorer's (SHGetFileInfo's display name), folders come first, then Arrange Icons'
  order (by name at the start; since step 3 remembered in desktop.txt).
- **Pictures** (`core::desktop_art`): which pixels are see-through comes from the PCs' own files
  (explorer.exe for My Computer, mydocs.dll, shell32.dll: dumped by `tools/dev/icons.ps1`). 95
  draws them exactly (0 differing pixels against its desktop), so its pictures are the files'.
  98 and 2000 draw through the shell's image list, which changes nearly every color slightly, and
  blend a selected icon in a way that is no plain average (C0 and CF both become 60), so their
  plain and selected pictures are cut from the PCs' desktops (`tools/dev/cutdesk.ps1`: colors from
  the capture, see-through pixels from the file, checked against each other). 95 selects by the
  average with 000080, per channel, rounded down; the owner's own pictures use that rule in every
  look. The shortcut arrow is shell32's 32x32 overlay (2000's gray is 848484). The owner's items
  get today's 32x32 picture from the shell's large image list at 96 DPI, without its arrow
  (`start_menu::large_file_icon`), folders the look's own folder.
- **Measured** (numbers in theme-spec.md, "Desktop icons"): the 75-pixel grid, the picture's place,
  the label box, wrapping at 71 pixels, two lines at most in 98 and 2000 except on the focus icon
  while the desktop has the keyboard, a too-long word shortened (95, 98) or broken (2000, in
  full only). Found on the way: when another window is in front, the desktop shows no selection
  and no focus frame at all (`*-desk-sel-inactive`); Windows 2000 draws no focus frame until a key
  has been pressed (`keys_used`; its `-desk-full-sel` capture, taken after mouse clicks only).
- **Painting**: the desktop window keeps a 1x framebuffer of all screens and paints again only the
  icons whose state changed (`Desktop::update` says which rectangle), scaled by whole numbers in
  WM_PAINT. Icons go on the main screen's work area (without the bar), as Windows did.
- **Staying current**: `desktop_items` looks at both folders and SHQueryRecycleBin once a second
  on its own thread; pictures are read only for new or changed files. A selection stays on its
  item (by path) when the list changes.
- **Taking the keyboard without coming forward**: a click calls SetForegroundWindow on the
  desktop's own thread. Activation normally moves a window to the top; a CBT hook on that thread
  (WH_CBT, thread-local, in our own process: no injection) sees HCBT_ACTIVATE first, and
  WM_WINDOWPOSCHANGING then adds SWP_NOZORDER for that one move. The window keeps
  WS_EX_NOACTIVATE and MA_NOACTIVATE, so Windows never activates it by itself (for example when
  the last window closes). The move Windows makes with its owner at Win+D is untouched (it comes
  without HCBT_ACTIVATE). Without the hook the desktop simply never takes the keyboard (logged).
  Live (`private/probes/desktop-icons/focus-live.ps1`, one real click on a free spot of the desktop
  at an idle moment, the 2000 look at size 2): the desktop was in front 30 ms after the click,
  the same 33 visible windows above it before and after, directly above Windows' desktop again
  within 200 ms (a hidden "IME" helper window was between for a moment, as logged before); Down
  and End then moved the selection; the window that was in front got the front back.
- **Opening** (default command, on a thread of its own with COM): My Computer
  `shell:::{20D04FE0-...}`, Recycle Bin `shell:RecycleBinFolder`, My Documents the Documents
  folder, the rest by its path. A double-click is Windows' double-click time and distance on the
  same icon. Live: a double-click posted to the window opened a new This PC window (closed
  again by the test); Home, Down, Enter opened it too. Messages to the desktop must come from a
  DPI-aware process, or Windows rescales the point (found when the first try missed).
- **The menu** (`core::desktop`): each look's own, limited to what works (brief point 7): 95
  Arrange Icons, New, Properties; 98 Arrange Icons, Refresh, New, Properties (separators as on
  the PC); 2000 Arrange Icons and Refresh together. Access keys as captured (98's "Re&fresh",
  "by Si&ze"; 95's and 2000's "Ne&w"). Properties opens Bevel95 Settings' Desktop page (or, when that page says so since
  2026-11-19, Windows' own Display settings: "The desktop, first step"). New >
  Folder and Text Document make the item in the owner's Desktop folder under Windows 11's own
  name ("New folder", "New Text Document.txt", then "(2)"...) and select it once it shows; New >
  Shortcut starts the Create Shortcut wizard on a placeholder there (`add_shortcut::start_on_desktop`).
  A right-click on an icon selects it and opens the menu on it (step 2, below).
- **Proof**: unit tests (order, arranging, grid, labels, clicks, keys, the selection following
  its item, the menus, new names); 3 new golden pictures (`*-desktop-icons`), 3 changed
  (`*-desktop-menu`); 26 compare-refs cases (`vm-*-desk-*`: 20 of the icons with their labels in
  every captured state, 6 of the menus' rows), 0 differing pixels. The captures:
  `private/reference/refs/vm/<era>-truecolor-desk-*.png` (fresh, each icon selected, none, inactive, the menus
  and submenus, long names focused and not, full Recycle Bin), taken with
  `private/probes/desktop-icons/d.ps1` (snapshot mode). Windows 95's Recycle Bin did not turn full
  after a delete on its PC, so its full picture is the file's (exact for 95, as its empty one).
- **Step 3, moving icons (2026-11-12, the owner: "I can open apps there but can't move the
  icons"; on remembering Arrange Icons: "the state should persist like a machine")**. Measured
  first on all three Reference PCs (`private/probes/desktop-move/m.ps1`, captures
  `private/reference/refs/vm/<era>-truecolor-move-*`, numbers in theme-spec.md "Desktop icons"):
  - **Free, not a grid**: all three leave a dropped icon exactly where it was dropped. Line up
    Icons, Auto Arrange and Arrange Icons are what put icons on the grid. Auto Arrange was off
    at the start in all three, so it starts off here. Enter opened every selected icon in all
    three, so it does here.
  - **The model** (`core::desktop_icons`): each item's place is its cell's top-left from the
    work area's corner in the look's own 1x pixels, kept by item id (a path, or `::my-computer`
    and so on). `places` is `None` until the owner moves something (brief point 5): until then
    the icons stand in columns in Arrange Icons' order and flow as before when items come and
    go. The first drop (or Line up, or a New item at the menu's point) freezes every icon where
    it stands. Then a new item takes the first free cell column by column (as 98 put a file made
    elsewhere), a New item from the menu lies at the menu's point (all three), an item that goes
    loses its place, the look's own icons keep theirs (95 hides My Documents; its place waits).
  - **Remembered** in `desktop.txt` in the data folder, written by the bar's desktop thread
    whenever something changed (`take_changed`): `arrange = name|type|size|date`,
    `auto_arrange = yes|no`, then `x y id` per icon. Why there and not in settings.txt: Windows
    kept these with the desktop's own view state (the shell's stream in the registry), not with
    the display options; they change by the dozen with every drag, which settings.txt (written
    by the bar's thread and by Bevel95 Settings) should not; and deleting one file puts the
    desktop back into columns. Setup's Remove deletes it, README lists it.
  - **Looks and sizes**: places are in 1x pixels, so a change of look or size keeps each icon in
    the same cell of the grid. A place that does not fit a smaller area (a larger size, a smaller
    screen) is shown in the first free cell instead, without forgetting the place, so the icon
    comes back when the area is large again: no icon is out of reach.
  - **Dragging** (`press`, `drag_to`, `release`, `cancel`): a press on a selected icon keeps
    the selection until the release (so several can be dragged); a drag starts past 2 pixels; the
    drop moves every selected icon by the pointer's way, clamped into the area; a drop with the
    pointer on another icon (Windows would move, copy or delete files) or outside the area does
    nothing, nor does Esc or losing the mouse. Each look's dragged picture as measured (95
    checkerboard, 98 half and half, 2000 see-through; several icons in 95 and 98 as dotted
    outlines); 98's and 2000's blends are near, not exact (theme-spec), and moving pictures are
    not compare-refs cases.
  - **Selecting**: the dotted rectangle (DrawFocusRect between press and pointer, selecting what
    it touches as it goes, Ctrl adds), Ctrl+click, Shift+click (Windows' icon view selects the
    icons in the rectangle around the anchor and the clicked icon, not a range of the list:
    measured on 98), Ctrl+A. Arrows go to the nearest icon that way (a step to the side counts
    twice, as 98's choice of a far icon straight up showed); Home and End the first and last in
    the order icons lie.
  - **The menu**: Line up Icons right below Arrange Icons in each look's own place and wording
    (95 "Lin&e up Icons", read off its menu; the earlier compare case had guessed "Line &up");
    the Arrange Icons submenu gains a separator and Auto Arrange with the menu's check mark while
    on (`arrange_menu(auto)`; the bar reads `desktop::auto_arrange()`).
  - **Painting**: `Desktop::update` compares every item's look and cell, the rectangle and the
    dragged pictures before and after and returns only the rectangles that changed (merged where
    they overlap). On the Windows side the window takes the mouse at a press (SetCapture) and
    repaints a drag or rectangle on a 15 ms timer that runs only while one lasts (the pointer's
    latest place is taken in at each step; the log counts the frames), like the task buttons'
    MOTION timer.
  - **For live tests**: `WM_ICON_AT` (which icon is at a point: a number made from its id,
    FNV-1a, `desktop::id_code`) and `WM_ICON_PLACE` (where the icon with that number is, or
    whether it is selected), so a test presses only its own icons
    (`private/probes/desktop-move/live.ps1`).
  - A Desktop folder that cannot be read for a moment is no longer taken for an empty desktop
    (the icons would have lost their places): `desktop_items` tries again.
  - Proof: unit tests (drop, drop on an icon or outside, Esc, several icons, the rectangle and
    its pixels, Shift+click, Ctrl+A, arrows, Line up, Auto Arrange, Arrange Icons, new items, the
    columns before any move, remembering across runs, looks and a smaller area, repainting, the
    dragged pictures); golden pictures `*-desktop-select`, `*-desktop-drag`,
    `*-desktop-drag-several` (new) and `*-desktop-menu` (Line Up Icons); compare-refs cases
    `vm-*-desk-arrange-auto` (the tick) and `vm-*-move-*` (a crooked drop and Line up on the
    three PCs), and the menus' Line Up rows now compared in `vm-*-desk-menu`; full compare-refs:
    546 cases, 0 differing pixels.
  - Live (2026-11-12): installed, the bar started by Explorer read no desktop.txt and showed the
    columns as before. `live.ps1 -Phase make` made three test files, which showed in the
    columns (their places answered by `WM_ICON_PLACE`); before the idle-gated drag ran, the
    owner chose Exit on the bar, so no drag was tried live. The test files were removed,
    desktop.txt was never written, settings.txt unchanged. Not tried live: everything that
    moves an icon (the owner's checklist; `live.ps1 -Phase move|check|lineup|auto|clean` is
    ready). Run on 2026-11-12 evening (the owner had started the bar again; the script's `LP`
    helper was renamed, PowerShell took it for Out-Printer): `make`, `move` (test 1 dragged by
    1200,150 window pixels landed exactly there; tests 2 and 3 moved together; test 2 dropped onto
    test 1 went back; a rectangle around test 1 selected only it), `check` (desktop.txt had the
    three places), `lineup` (each test icon onto the nearest grid cell), `auto` on and off again,
    `clean`. desktop.txt did not exist before; it was removed after and the bar restarted, so the
    owner's icons stand in columns as before.
- **Step 2, the menu on an icon (2026-11-12, the owner: "Step 2 on desktop with icon editing is
  fine. I assume the original systems had a verification window?")**. Measured first on all three
  Reference PCs (`private/probes/desktop-menu/k.ps1`, captures `private/reference/refs/vm/<era>-truecolor-item-*`,
  numbers in theme-spec.md "The menu on an icon"):
  - **The menus** (`core::desktop::item_menu`): each look's own order, groups and access keys
    (2000: "E&xplore"), limited to what works here: Open (bold), Explore (folders and the look's
    own icons), Empty Recycle Bin (grayed while empty), Cut, Copy, Create Shortcut, Delete,
    Rename, Properties. Left out, not grayed: Print, Open With, Quick View, Find/Search, Manage,
    Map/Disconnect Network Drive, Sharing, Send To, Remove from Desktop, Paste into a folder or
    the Recycle Bin, other programs' entries. The look's own icons are never cut, copied, deleted
    or renamed (2000's My Documents had Delete and Rename, all had Rename for My Computer: left
    out, as the brief said). Several selected: the entries every one of them has, no Rename
    (Windows' own mix for the Recycle Bin with a folder had Find or Empty Recycle Bin and Paste;
    ours is the plain intersection). The desktop's menu gained Paste and Paste Shortcut in each
    look's place, grayed while the clipboard holds no files.
  - **Windows 2000's Explorer menus** drew their bold entry in a bold whose "O" is round and a
    pixel wider and whose "e" closes at the bottom right, where today's Tahoma Bold (which 2000's
    titles and window menus match) differs: `Font::explorer_bold`, only for menus marked
    `Menu::explorer_bold`. Its grayed access-key underline is dotted (as Calculator's), on the
    menu's even columns (the rule now for both).
  - **The questions** (`core::delete_confirm`, `confirm_window.rs`): today's Windows asks nothing
    when a file goes to the Recycle Bin, so Bevel95 asks with each look's own window: Confirm File
    Delete ("Are you sure you want to send 'x' to the Recycle Bin?"), Confirm Folder Delete
    ("...remove the folder 'x' and move all its contents to the Recycle Bin?"), Confirm Multiple
    File Delete ("...send these N items..."), and before emptying the Recycle Bin "Are you sure
    you want to delete 'x'?" / "...these N items?" (one item's name read from its `$I` record).
    Three window layouts, as Explorer's three dialog templates, the same in 95, 98 and 2000; the
    pictures cut from the PCs (`art/assets/<era>-delete-icons.rgb`). Yes had the focus on all
    three PCs, so it has here; Esc, No and the X answer no. Only Yes hands the work on.
    A second question while one is open (another Delete, an Undo Copy) closes the first, whose
    action goes with it, and only then stores its own (`winkit::confirm::take_over`); the other
    way round, the first one's closing took the new action and the new question's Yes did
    nothing (found by the safety review of 2026-11-18, C1; test
    `a_second_question_keeps_its_own_action`, failed the old way).
  - **Windows' file machinery** (`file_ops.rs`, every call on a thread of its own):
    SHFileOperation for Delete (FOF_ALLOWUNDO, so to the Recycle Bin with Restore; with
    FOF_NOCONFIRMATION after our question, and FOF_WANTNUKEWARNING so Windows still asks if a file
    cannot go to the Recycle Bin), Rename (FO_RENAME; tried in a scratch folder first: a name with
    ":" and a taken name each got Windows' own message window and nothing changed), Paste
    (FO_COPY, or FO_MOVE after Cut; a copy of a file from the Desktop folder itself gets Windows'
    "- Copy" name through FOF_RENAMEONCOLLISION; any other taken name gets Windows' own
    question). Cut and Copy put CF_HDROP and "Preferred DropEffect" on the clipboard, as Explorer
    does, so Explorer pastes them and the desktop pastes what Explorer copied. Create Shortcut and
    Paste Shortcut write "Shortcut to <name>.lnk" (Windows 95 to 2000's name; never over a file:
    "(2)" and so on) with the ShellLink object, the look's own icons by item list (SetIDList), a
    shortcut's shortcut as a copy of it; the new one is selected once it shows. Properties:
    SHObjectProperties, SHMultiFileProperties for several files of one folder, the shell object's
    own for the look's icons. Explore: Explorer with /e. Empty Recycle Bin: SHEmptyRecycleBin
    without its second question. Rename adds the old extension back when the desktop did not show
    it (Explorer hid it, or a shortcut's .lnk). No new windows-sys feature.
  - **Renaming** (`Desktop::begin_rename` and friends): the label becomes Windows' edit box,
    measured: a black frame, white inside, the edit control's margin 4 pixels (MS Sans Serif) or
    5 (Tahoma), 1 pixel of white above and below one line (95 and 98 give several lines 3), the
    text centered, all of it selected at first, the caret 2 pixels wide in 95 and 98 and 1 in
    2000, blinking at Windows' rate; the picture shows plain while renaming. Enter, a click
    elsewhere or the desktop losing the keyboard take the name; Esc, the old name or an empty one
    change nothing. **A name that cannot be a file's name** (found by the safety review of
    2026-11-18, "suspected 3"): the box accepted every character and the new path was the folder
    joined with what was typed, so "..\x" or "C:\x" named another folder. Tried first on scratch
    test items with Windows' machinery as Bevel95 calls it (`private/probes/drops/rename.ps1`;
    "C:\x" tried as a whole path into a scratch folder, not at the drive's root): all three of
    "..\x", a whole path and "a/b" were refused (0x73, another folder) and the item stayed, but for
    "a/b" Windows first made an empty folder "a". Now a name holding any of `\ / : * ? " < > |`,
    or only dots and spaces (`core::rename_warning::bad_name`, tested), is refused before Windows
    sees it, in the desktop's and the folder windows' rename box (also New Folder's and a new
    shortcut's, which are the same box) and again in `file_ops::rename`: the look's message
    (`confirm::bad_name`: title "Error Renaming File" in 95 and 98, "Error Renaming File or
    Folder" in 2000, as measured for a rename that cannot be done; text "A filename cannot contain
    any of the following characters: \ / : * ? " < > |", PROVISIONAL, not measured: 2000 showed
    it in a balloon), and once it has gone the box opens again as after the extension warning's
    No. Tried live in a scratch folder window (`drop.ps1 -Phase badname`, posted keys to its own
    window): all three names gave the message (photographed), the box came back, nothing changed
    on disk. The desktop's box: the same code path, not tried live (its items are the owner's). A renamed item keeps its place, selection and focus (`Desktop::renamed`), and
    in columns it moves to its place in the order with the next reading.
  - **Cut icons** fade as each look drew them (measured on a text file's picture): 95 and 98 half
    picture, half desktop color, rounded down; 2000 the same in 5 bits per channel (its image
    list's 16-bit colors), selected or not. They stay faded until something else goes onto the
    clipboard (AddClipboardFormatListener on the desktop window).
  - **Keys** (`core::desktop::shortcut`): Del, F2, Ctrl+X, Ctrl+C, Ctrl+V, Alt+Enter (nothing
    selected: the desktop's Properties), Shift+F10 and the menu key (at the focus icon's middle,
    as the PCs opened it; nothing selected: the desktop's menu at the top left). Shift+Del, which
    deleted for good in Windows, goes to the Recycle Bin like Del (Bevel95 never deletes the
    owner's files for good).
  - **For live tests**: `WM_TEST_COMMAND` carries out a command on one icon, selected alone,
    as a key would, but only on items named "Bevel95 test ..." (anything else is refused and
    logged); `WM_ICON_PLACE` also answers cut (lParam 2) and renaming (3).
  - **The private messages are registered by name** (the third safety review of 2026-11-18, C4:
    they were fixed `WM_APP + n` numbers, and the desktop window is an ordinary top-level window,
    so another program's stray broadcast in that range reached it: `WM_APP + 11` with wParam 0
    pasted the clipboard's files into the Desktop folder, `WM_APP + 5` rearranged every icon).
    Every message between Bevel95's own threads and programs for the classic desktop and the
    folder windows is now `winkit::util::Private`: registered with RegisterWindowMessageW the
    first time it is used (a number from 0xC000 up that only a program asking by the same name
    gets); the old numbers do nothing. Tried live on the owner's desktop, questions only
    (`private/probes/review3/desktop.ps1`): IconPlace and IconAt by their names answered (My
    Computer found, and found again at its place), the old `WM_APP + 6`/`+ 7` answered 0, the test
    command on a non-test icon was refused by its name and did nothing by `WM_APP + 11`; in a
    scratch copy of folders.exe (`private/probes/review3/tree.ps1 -Named`) the tree's test commands,
    rows and menu worked by the names, and by the old numbers nothing happened. The names, for
    adapting older probes under private\probes\ (which still post the old numbers and now do nothing):
    - to the desktop window (class `Bevel95Desktop`): `Bevel95.Desktop.TestCommand` (was
      `WM_APP + 11`), `.IconAt` (+ 6), `.IconPlace` (+ 7), `.Arrange` (+ 5), `.Check` (+ 1),
      `.Go` (+ 2), `.Look` (+ 3), `.Items` (+ 4), `.Rename` (+ 8), `.Cut` (+ 9), `.Renamed`
      (+ 10), `.RenameAnswer` (+ 17);
    - from the desktop to the bar: `Bevel95.Desktop.Command` (the bar's `WM_DESKTOP_COMMAND`,
      was + 10), `.Press` (+ 14), `.Menu` (+ 15), `.ItemMenu` (+ 30), `.RenameAsk` (+ 31);
    - to folders.exe's hidden window (class `Bevel95Folders`): `Bevel95.Folders.TestCommand`
      (was + 11), `.IconAt` (+ 6), `.IconPlace` (+ 7), `.Items` (+ 1), `.RenameAnswer` (+ 2),
      `.TreeRows` (+ 12), `.Unreadable` (+ 13), `.Pictures` (+ 14).
    - **the bar's own messages** (window class `Bevel95Bar`; the fourth safety review of
      2026-11-18, C3: they were still fixed numbers, and a stray broadcast of `WM_APP + 3` with a
      command's code could close or restart the bar, `+ 4`, `+ 7` or `+ 40` change and save the
      size, the look or the colors, `+ 5` open Run or Shut Down): `Bevel95.Bar.AppBar`
      (WM_APP + 1, the callback SHAppBarMessage is given), `Bevel95.Bar.Command` (WM_APP + 3),
      `Bevel95.Bar.Size` (WM_APP + 4), `Bevel95.Bar.StartAction` (WM_APP + 5),
      `Bevel95.Bar.StartKey` (WM_APP + 6, the keyboard hook's), `Bevel95.Bar.Theme` (WM_APP + 7),
      `Bevel95.Bar.OpenProperties` (WM_APP + 8), `Bevel95.Bar.ShortcutWizardEnded` (WM_APP + 11),
      `Bevel95.Bar.DragEsc` (WM_APP + 13, the keyboard hook's), `Bevel95.Bar.Colors` (WM_APP + 40),
      `Bevel95.Bar.OwnIcon` (WM_APP + 41, the callback of Bevel95's icon in Windows' notification
      area). wParam and lParam are as before. The bar registers them all as it starts
      (`bar::register_messages`), so the keyboard hook never asks Windows for a number. Left as
      numbers, since they only paint, read or answer: `WM_REPAINT` (+ 2), `WM_VOLUME_CHANGED`
      (+ 9), Quick Search's `WM_FIND_PROGRAMS_READ` (+ 12), `WM_TASK_AT` (+ 16, a live test's
      question), and the keyboard hook's own `WM_RELEASE_WIN`, a thread message no broadcast
      reaches. Everything of the project that posts them changed with them: the bar's menus, Quick
      Search, the Start menu, the keyboard hook, Add..., `cargo selftest` (`selftest.rs` registers
      `Bevel95.Bar.Command`), `tools\dev\dialogcaps.ps1` and `volcheck.ps1`. **`tools\dev\postbar.ps1
      <name> [wParam] [lParam]`** posts one by its name (whole or without `Bevel95.Bar.`) from a
      DPI-aware process, to the `Bevel95Bar` window of bevel95.exe only: `postbar.ps1 Command 5`
      opens Bevel95 Settings. The old probes under private\probes\ post the old numbers and now do nothing.
      Test: `bar::tests::the_bars_messages_are_registered_by_name` (each name its own and listed
      here with its old number; only the five above are left as `WM_APP + n` in the bar's files).
    - **A name Windows refuses** (its atom table full) is remembered as failed (`NO_MESSAGE`,
      which no message matches) and logged once; the guards do not ask again at every message (the
      fourth review, C5: each message to the desktop, the bar or a folder window asked again and
      wrote a log line per guard). Test: `util::tests::a_name_that_cannot_be_registered_is_remembered_as_failed`.
    The WM_COPYDATA test kinds (`TEST_DROP`, `TEST_PROPERTIES`) already took test items only.
  - **A test command acts on test items only, on every path** (the same review, C4): the
    desktop's `WM_TEST_COMMAND` gives an icon's own commands only on a test icon, and Paste, Paste
    Shortcut, Undo and Refresh with or without one; Empty Recycle Bin (which it used to show the
    question of), Arrange Icons, Line Up, Auto Arrange, New and switching the desktop off are
    refused (`desktop::test_command_allowed`). Its command goes to the bar under a test's ticket
    (`remember_test_targets`), and the bar carries it out inside `file_ops::for_a_test`: Paste and
    Paste Shortcut take the clipboard's files only when every one is a test's own (named
    "Bevel95 test ..." in a folder named so; they took whatever was there, a person's files too),
    and Undo takes back the last step only when it is all of test items (`Step::test_only`,
    checked under the list's lock; else the step stays). folders.exe's test commands run inside
    the same scope, also what a menu they opened then carries out. Tests:
    `desktop::tests::a_tests_command_acts_on_test_items_only`,
    `file_ops::tests::a_tests_paste_takes_only_test_items`,
    `undo::tests::a_tests_undo_takes_back_only_a_step_of_test_items`,
    `drag::tests::only_a_step_of_test_items_is_a_tests`,
    `desktop::tests::the_desktops_messages_are_registered_by_name`, folders'
    `the_folder_windows_messages_are_registered_by_name`.
  - Proof: unit tests (each kind's menu in each look, several selected, the keys, the questions'
    words and layouts, renaming, the rename box, a renamed item's place, the cut pictures, the
    new file name, the Recycle Bin record); golden pictures `*-desktop-item-menu`,
    `*-desktop-rename`, `*-desktop-cut`, `*-desktop-confirm-delete` (new), `*-desktop-menu`
    (Paste rows); compare-refs: 48 new cases `vm-*-item-*` (22 menus, 17 question windows, 6
    rename boxes, 3 cut pictures) and the desktop menus' grayed Paste rows: 594 cases, 0 differing
    pixels.
  - Live (2026-11-12 evening, the 95 look at size 2, `private/probes/desktop-menu/live.ps1`, only on
    "Bevel95 test ..." items made for it): a posted right-click on test A opened its menu
    (photographed: Open bold, Cut, Copy, Create Shortcut, Delete, Rename, Properties), Esc closed
    it; Copy put test A on the clipboard as a file, Paste made "Bevel95 test A - Copy.txt"; Cut
    marked test B (Preferred DropEffect 2) and its picture faded; Create Shortcut made "Shortcut to
    Bevel95 test A.txt.lnk", selected (this PC shows extensions, so the name has ".txt"); Rename
    of test B to "Bevel95 test C.txt" by posted characters and Enter kept its place; Properties
    opened Windows' window of test A (closed again by the test); Delete of the test folder showed
    Confirm Folder Delete, No left it, Yes sent it to the Recycle Bin; four test items selected
    by Ctrl+clicks and a posted Del showed Confirm Multiple File Delete, Yes sent them to the
    Recycle Bin; Empty Recycle Bin showed "Are you sure you want to delete these 9 items?" and was
    answered No (nothing emptied). Afterwards the five test items were taken out of the Recycle
    Bin by their own `$I`/`$R` records and nothing of the owner's was touched (the Recycle Bin
    held the owner's 4 items before and after). The test's posted right-click gave the classic
    desktop the keyboard, as a real click on it would (the window that was in front became
    inactive, nothing else changed). The clipboard's earlier picture was put back after the
    Cut/Copy tests. Not tried live: Paste of a cut file (a move), Paste Shortcut, Explore, Open,
    the keys F2/Ctrl+X/Ctrl+C/Ctrl+V/Alt+Enter/Shift+F10 by real keys (the owner's checklist), a
    click inside the rename box, the Recycle Bin's and My Computer's Properties, Create Shortcut
    on the look's own icons, the 98 and 2000 looks live.
- **The extension warning (2026-11-12, the owner: "Might as well match reference systems
  there")**. Today's Windows warns about a changed extension in File Explorer only (its file
  machinery renames without asking), so the desktop asks itself, as the Reference PCs did.
  Measured first on all three (`private/probes/rename-ext/r.ps1`, captures
  `private/reference/refs/vm/<era>-truecolor-renext-*`, numbers in theme-spec.md "The extension warning"):
  - **What each Windows showed**: the same in 95, 98 and 2000, a plain message box titled
    "Rename" with the exclamation picture, two sentences, Yes (focused, default) and No, the X
    grayed (so Esc and Alt+F4 do nothing). `core::rename_warning`: the rule, the words and the
    form on the control kit (`RenameWarning`), its place by USER's own centering rule
    (`place`); `confirm_window::RenameWarningHost` shows it on the bar's thread, next to the
    delete questions.
  - **When** (`rename_warning::warns`): a file, not a folder, whose old extension is a known
    type gets another one or none; case aside. Not when the old extension was no known type
    (98: also not after giving ".dat" an empty registry key, or one naming a missing or empty
    type), not for an extension added to a name without one, not for a shortcut (its ".lnk"
    stays). The names compared are the real file names before and after (`new_file_name`
    keeps an extension the desktop did not show), so a hidden extension never asks: that is
    the name the owner sees and types. "Known" on today's Windows is `file_ops::known_type`:
    Windows has a program identifier for it (AssocQueryString, ASSOCSTR_PROGID with
    ASSOCF_INIT_IGNOREUNKNOWN: ".txt", ".log", ".png" yes; ".dat", ".rs" and unknown ones no;
    tried on this PC). No new windows-sys feature.
  - **The rename box waits under it** (`Desktop::rename_asking`, `is_asking`): Enter, a click
    elsewhere or losing the keyboard look at the name first (`rename_result`); if it warns, the
    box keeps the typed name without selection or caret and takes no keys or clicks, and the
    desktop posts `WM_DESKTOP_RENAME_ASK` to the bar. The answer comes back as
    `WM_RENAME_ANSWER`: Yes renames as before; No opens the box again, all selected
    (`rename_declined`), with the name typed in 95 and 98 and the old name in 2000
    (`keeps_typed_name`), and gives the desktop the keyboard back. The warning closed without
    an answer (the bar exiting) counts as No.
  - **The box's place**, measured on "New Text Document.txt" typed over with "a.dat": 95 and 98
    keep the several-line box's 3 pixels of white when a two-line name is typed down to one
    line (3 pixels lower than a several-line box), and after No the one line is a plain
    one-line box 3 pixels lower than a fresh one. Windows 2000's box after No stood 4 pixels
    lower than a fresh one; that move of its edit control is not copied (its compare case is
    left out, the reason in `reference.rs`).
  - Proof: unit tests (every case the PCs showed, extensions found as PathFindExtension does,
    the buttons and keys, the size and place, the box waiting and opening again, known types on
    this PC); golden pictures `*-desktop-rename-extension` (new); compare-refs 7 new cases
    `vm-*-renext-warning` (the whole window, all three) and `vm-win95/98-renext-box-changed`,
    `-after-no`, 0 differing pixels.
  - Live (2026-11-12 night, the 95 look at size 2, `private/probes/rename-ext/live.ps1`, on a test
    file "Bevel95 test E.txt" made for it, at idle moments with an 8-second gate):
    `WM_TEST_COMMAND` Rename, the new name and Enter posted: the warning opened (photographed:
    `private/probes/rename-ext/live/warning.png`), nothing renamed meanwhile; Tab and Enter (No):
    the warning closed, the file unchanged, the box open again; Esc closed it. Again with
    Enter on Yes: the file became "Bevel95 test E.dat". The test file was then deleted (it never
    went to the Recycle Bin, which held the owner's 4 items before and after). Not tried live:
    real keys and clicks, a click elsewhere or another window taking the keyboard as the way to
    end, the 98 and 2000 looks.
  - Shift+Del was captured on the same runs (file, folder, two items: `*-renext-shiftdel-*`),
    for the owner's open question; nothing built.
## One installed copy (2026-11-12, the owner: "the desktop links open a version without many settings")
- What the owner saw: after Exit, the desktop shortcut started a bar without the Desktop and
  Programs pages and with old settings, although every install of the day before had been
  reported as done.
- The cause, measured on this PC: a program started from the Claude desktop app (every tool
  call, every worker, `tools\install.ps1`, a bar restarted by it, the launcher's Run button)
  does not always write where it says. A folder it creates at the top of `%LOCALAPPDATA%`
  lands in the app's private area, `%LOCALAPPDATA%\Packages\<the app's package>\LocalCache\Local`.
  Where a folder exists in both places, such a program sees the two merged with the private
  file winning, and every new file at the top of that folder goes to the private one. A
  subfolder that exists only in the real folder is written for real (a probe file in
  `Bevel95\programs` landed in the real folder, one at the top of `Bevel95` in the private
  one). `GetCurrentPackageFullName` says the shell is in no package, so it cannot be asked.
- So from 2026-11-11 about 12:30 on, each install put `bevel95.exe`, and the bar put
  settings.txt, programs.txt and the log, into the private `Bevel95`, while the real folder
  kept the 12:30 program (its `programs\` did get each new companion). A bar restarted by the
  install or the launcher was the new one; a bar started by Explorer (the desktop shortcut,
  and it would have been the same for "Start with Windows") was the old one with its old
  settings and its own log. Every "installed and tried live" of that day was true only for
  bars started from the app. The earlier note that `explorer.exe <exe>` "does not work here"
  was this.
- The repair: both copies saved in `private\backup\install-20261112` (`private\`, `real-before\`),
  the private `Bevel95` taken away (the owner: "feel free to wipe the old cached version"),
  `tools\install.ps1` run again, which then wrote the real folder, and the owner's current
  settings.txt, programs.txt, task-order.txt, calculator.txt, minefield.txt and log put into
  it. The bar started through Explorer is the new one: its parent is explorer.exe, its image
  in memory has the new build's size, and its start is in the one log.
- From now on:
  - `tools\install.ps1` installs nothing when a private `Bevel95` exists, and says so. If the
    real folder does not exist (after Setup's Remove), it cannot be made from inside the app:
    the script says how to make it from a PowerShell of the owner's own.
  - The bar is always started through Explorer (`install.ps1`'s restart, the launcher in
    CLAUDE.md), so it is an ordinary program of the owner's and not the app's child. A bar
    started from the app could again get private copies of things it creates new under
    `%LOCALAPPDATA%` or `%APPDATA%` (a shortcut made by Add...), and may end with the app.
    Which ways of starting are which was measured the same evening, each with a throwaway
    folder made at the top of `%LOCALAPPDATA%` by the started program: started directly by a
    tool: private; the same with CREATE_BREAKAWAY_FROM_JOB (as the watchdog is started):
    still private, so it does not go by the job; started with `explorer.exe <file>`: real;
    started by Explorer's own process through the desktop's shell object (ShellWindows,
    FindWindowSW for the desktop, `.Document.Application.ShellExecute`): real, and the main
    explorer.exe is the parent. `tools\start.ps1` is the last way, and `install.ps1` restarts
    the bar with it. `IsProcessInJob` tells nothing here: a bar started by Explorer is in a
    job too.
  - "Installed" is checked as the owner starts it: the bar's parent process and the log, not
    the file seen from a tool, which may be the private one.
- **Old copies beside a running program** (2026-11-17): `install.ps1` puts each program in place
  with `[IO.File]::Replace`; when that program is running, Windows keeps the old file as its
  backup under a name like `folders.exe~RF1669db9a.TMP` (one from 19:13 that day lay in the
  installed `programs` folder). Each install now deletes, in the installed folder and its
  `programs` folder, files whose name is one of the six programs' (`bevel95.exe`, `bevel-help.exe`,
  `folders.exe`, `minefield.exe`, `solitaire.exe`, `calculator.exe`) followed by `~RF`, hex digits
  and `.TMP`; one still in use stays, with a plain line instead of an error. The leftover of 19:13
  went at the next install. Setup (`install.rs`) cannot leave such files: it writes a
  `<name>.<pid>.tmp` beside the program and renames it over the old one, which fails for a running
  program (Setup then says it was left, and deletes its temporary file). Setup's Remove is not
  stopped by such a leftover (it opens only the companions by their own names), but cannot take the
  `programs` folder away while one lies in it (the folder is removed only when empty, and the
  leftover stays): only a developer's install made them, so it was left so.
- Left as it is: the build folder `%LOCALAPPDATA%\cargo-target\bevel95` was first made from
  the app and is physically in the private area. Nothing but the build uses it. (Since
  2026-11-18 the build goes to cargo's own `target` folder in the checkout, and that old folder
  is no longer used.)
- Not examined: whether the registry is private in the same way (the Run entry of "Start
  with Windows" is written by the bar itself, which now is an ordinary program).

## Folder windows (2026-11-13, the owner: "I think your order is probably fine")
- What was agreed: a first Explorer, Bevel95 drawing the window and Windows doing the work
  underneath, in four steps: (1) Windows 95's plain folder windows, one window per folder (this
  section), (2) 98's and 2000's window with toolbar and address bar, (3) the two-pane Explorer
  with the tree, (4) the web view panel. Part A of the job (47 captures of all three Explorers,
  `private/probes/explorer/for-the-owner.txt`) found: 95 opens a new window for every folder, 98 and
  2000 the same window; the Explorer entry is Start > Programs > Windows Explorer in 95 and 98,
  Programs > Accessories in 2000.
- **Measured first** on the Windows 95 PC (theme-spec.md, "Folder windows": 40 captures
  `win95-truecolor-folder-*`, the menu help texts and status bar words read from its SHELL32.DLL
  and EXPLORER.EXE string tables, `private/probes/folders/strings.ps1`).
- **Where it lives: a program of its own** (`programs/folders`, `folders.exe`), not windows of the
  bar: a folder window reads disks (a slow network drive, a CD without a disk), loads the
  shell's icons and runs Windows' file machinery; a hang or crash there must never take the bar
  or the desktop down, and the program can be ended without touching the bar. One process holds
  every folder window: started again (`folders.exe <folder>`, as the desktop does), it hands the
  folder to the running one through its hidden message-only window (class `Bevel95Folders`,
  WM_COPYDATA) and ends; a named mutex settles two starts at once. It ends with its last window.
  - **Programs of two copies never serve each other** (the fifth safety review of 2026-11-18, B1:
    the hub was found by a class and a mutex every copy shared, so an installed copy's folder open
    could land in a portable copy's folder program, which kept that window's places and its Undo
    steps in the other copy's folder, and the reverse). Every name by which one Bevel95 program
    finds another of its own kind carries a short fingerprint of the copy's data folder
    (`winkit::copy`: FNV-1a, 64 bits, of the path in lower case, `/` as `\`, no trailing `\`, no
    `\\?\`; `copy::name(base)` is `base.<16 hex digits>`). **One copy has one fingerprint, however
    its folder is spelled** (the sixth safety review of 2026-11-18, C1: of the path as spelled, a
    mapped drive and its network name, a `subst` drive or a junction made one copy two, each with
    its own hub, folders.txt picture and Undo lock): `copy::fingerprint` is of the folder's final
    path as Windows resolves it (the folder opened and asked for its final name,
    `std::fs::canonicalize`); when that cannot be had, of the spelled path with short names spelled
    out, as before, and the log says so once. The installed folder's final path is its usual
    spelling, so its names did not change. Test: `copy::tests::one_copy_through_a_junction_has_one_fingerprint`
    (a junction made in a scratch folder under `private\probes\review6\` and removed again). A program
    of an older build still running during an update keeps its old names until it ends (below):
    nothing is built for it.
    The names, each with its kind:
    - finds a program of its kind, **now per copy**: the folder program's start mutex
      (`Local\Bevel95.Folders.<copy>`, was `Bevel95.Folders`), its hub's place
      (`Local\Bevel95.Folders.Hub.<copy>`), its hidden windows: the classes `Bevel95Folders` and
      `Bevel95FoldersSecond` stay (with no window name, as before, so the live-test scripts that
      look for them still find them), and a start hands over only to one whose process holds the
      mark `Local\Bevel95.Folders.<copy>.<pid>` (`copy::mark`, `copy::is_of_this_copy`);
      Minesweeper's "already running" (a window of class `Bevel95Window` named Minesweeper,
      now only one whose process holds `Local\Bevel95.Minesweeper.<copy>.<pid>`); Solitaire's
      start lock and window marks (`Local\Bevel95.Solitaire.<copy>...`, which held a hash of the
      program's and the data folder's paths before; now the same rule);
    - a lock that guards a file: the Undo list's (`Local\Bevel95 undo list.<copy>`, now per copy,
      since each copy has its own undo.txt); Solitaire's settings lock (`Global\Bevel95.Solitaire.
      Settings.<hash of the folder>`, already per file, kept);
    - **one for the session on purpose** (one screen): the bar's place `Local\Bevel95.Bar` and its
      class `Bevel95Bar` (`--restore`, Setup and the watchdog find it), the lock of Windows'
      taskbar record `Local\Bevel95 taskbar record`;
    - per process, not per copy: the bar's restart mark and the mark "finishing file jobs" (each
      with the process number);
    - no program is found by them: the registered messages (`Bevel95.LookChanged` reaches every
      program's windows, and each reads its own copy's settings.txt again, so another copy's bar
      changes nothing), the drag format `Bevel95 drag` (a drop is carried out, and its Undo step
      kept, by the program it is dropped on, as a drop from Explorer is). The Help viewer is
      started by its caller and talks through its own pipe: nothing to find. The Calculator has no
      "already running".
    Tests: `copy::tests::one_copy_one_name_two_copies_two_names` (case, a trailing backslash,
    slashes, `\\?\`, a real short name of a scratch folder; two folders two names),
    `copy::tests::a_mark_is_seen_for_its_process_and_kind`. Tried live
    (`private/probes/review5/b1/live.ps1`, two scratch portable copies of folders.exe; `installed.ps1`,
    the installed layout in a scratch %LOCALAPPDATA%; `private/probes/review5/games/live.ps1` with
    `tools\dev\minefield.ps1`): an open through each copy opened in that copy's program, a second
    open through the same copy was handed over (A: test folders one and three, B: two and four,
    each in its own folders.txt); the installed layout's second start handed over while a
    portable start beside it ran on its own; a second Minesweeper of the same data folder brought
    the first forward and ended, one of another folder ran as a game of its own. **A program of
    an older build still running after the install** (started before it) has the old names: until
    it ends, a new start does not find it and opens a program of its own.
  - **Nothing is read on the program's thread when a window is asked for** (the fourth safety
    review of 2026-11-18, B3: a folder on a drive that did not answer held the only thread, and
    the next starts waited a minute each). The hub's thread, which every window and every
    hand-over shares, lists no folder, asks no drive and resolves no name when it is asked to
    open or show a place. Moved off it, into a thread per asking (`ask`, `read_facts`, `Facts`):
    whether a start's or a hand-over's argument is a folder (`shows_of`'s `is_dir`), Explorer's
    start (My Documents in 2000), the title's names (`display_name` of the place and of every
    place above it for 98's and 2000's chain, a drive's kind and label: My Computer's whole
    drive list was read for one title), the web view's drive space and My Documents check, the
    tree's way down (My Documents and the two Desktop folders), and the first size's count (the
    folder's listing, the Desktop's, My Computer's drives). The titles are made from what was read
    for each look (`title_from`, pure; a look change makes them again from it). A new window opens
    as soon as that is known (a few milliseconds on a local disk), or after **500 ms** (`PATIENCE`)
    with what is known without asking (the known folders and names read before; the folder's own
    name, a drive's letter; the first size of an empty folder), and its title, pictures, address
    and web view follow when the reading comes. **A start's or a hand-over's argument has the same
    patience** (the sixth safety review of 2026-11-18, C3: one whose reading never came, a drive
    that does not answer or a reading thread that panicked, kept the program for ever without a
    window, and so blocked Setup's update of folders.exe): after 500 ms its window opens with the
    place it names, its drive not asked (`known_of`), and shows what a folder window shows for a
    place that cannot be read (empty; a drive's root also the look's drive message); an argument
    the reading finds is no folder still opens nothing when the answer comes within the 500 ms. A
    reading thread that ends without an answer (a panic in it: `Answer`, handed over on unwind)
    counts as "not read" at once (`not_read`); after a panic nothing new opens (the panic hook
    closes the windows so the program ends). An argument that names no place at all opens nothing.
    Tried live (`private/probes/review6/c3/live.ps1`, a scratch copy asked for a folder, a drive and
    an Explore on a letter that does not exist): each ended in 35 to 142 ms without a window; a
    drive that never answers and a reading thread's panic were not tried live (the test covers them).
    A navigation (98, 2000) shows what is known at once and takes the place's reading
    when it comes, while the window still shows that place; "a place of the history is gone" is
    logged by that thread. Not moved, being a person's actions in a window already shown: the
    places menu's and the address list's titles (`title_of`), what is typed into the address bar
    (`go_typed`), the drive question (`ask_drive`), the tree's My Documents row. Tests:
    `tests::a_title_is_made_from_what_is_read_of_the_place` (a drive's name in 98 and 2000, My
    Documents, the provisional titles, My Computer's count per look).
  - **A late reading goes only to the place it was read for** (the fifth safety review of
    2026-11-18, C2: a window that opened after the 500 ms took its first place's reading even
    after it had gone elsewhere, showing that drive's space and losing its title). The window's
    id when it opened early is kept with the asking (`Asked::opened`), and the reading is applied
    with it (`on_reading`, `apply_facts` now always with an id): a navigation gives the window a
    new id, so a reading for the place it left is dropped. Test:
    `tests::a_late_reading_goes_only_to_the_place_it_was_read_for`.
  - **Only a navigation starts the view afresh; a late reading changes only what it read** (an
    outside review of 2026-11-18 found it: `apply_facts` called `present`, which called
    `show_place`, which threw the items shown away; the reader tells only of changes, so a window
    whose place took longer than the 500 ms to read, a slow or network drive, My Computer with a
    sleeping disk, the first folder after starting, went blank and stayed so until Refresh).
    The code says it by its shape: `show_new_place` (the core's `FolderWindow::show_place`: no
    items, nothing selected, scrolled to the top, the web view's place afresh) and
    `show_what_is_known` (the core's `update_place` and `set_web_known`: the title, the window's
    and the address bar's pictures, the address text, Back, Forward and Up and 98's chain of
    places, the web view's kind, picture and drive space; the items, the selection, the scroll,
    the view's kind and order, a rename box, the address list, the system folder's warning and
    whether its files were shown stay; a typed address stays too, Esc then gives the new address,
    `address::Field::place_known`). Every caller, each one kind on purpose: `create_window` (a new
    window) and `navigate` (98, 2000 and every Explorer going to a place in the same window): a
    new place; `apply_facts` (a reading that came after the window opened or went there, from
    `prepared_ready` for both the early window and the navigation's reading): only what is known;
    `look_changed`: a new place drawn in the new look, its items put back at once by the caller
    (as before; the selection and scroll start afresh at a change of look). Tests:
    `folder::tests::a_late_reading_keeps_the_items_selection_and_scroll` (core, three looks) and
    `tests::a_late_reading_keeps_the_files_shown` (folders.exe: 95's Explorer, 98, 2000 with and
    without the tree). For live tries a test folder's window can make test folders' readings come
    1.5 s late (`TEST_SLOW_READING`, 255, only test folders, only in that program's run); the log
    says "a place's reading came after its items: N items shown, M after it". Tried live
    (`private/probes/codex2/late/live.ps1`, a scratch copy with its own settings.txt, 12 and 9 test
    files): with the old way built in (`old-*.png`), in all three looks the files were shown
    before the reading and 0 after it, for a navigation and for a handed-over folder whose window
    opened after 644 to 652 ms; with the repair 12 of 12 and 9 of 9 after it (`new-*.png`).
  - **The program does not end while a folder is still to open** (the fifth safety review of
    2026-11-18, C1: a folder handed over while the program, without windows, waited for a copy to
    finish, or while its last window closed, was taken, answered and never opened). Once its last
    window has closed, the program first asks for every folder handed over whose message is still
    queued (`open_pending`), then waits for any folder handed over or being read
    (`waiting_to_open`: `PENDING_OPEN` or `ASKED`; `after_last_window`), also while it waits for
    file jobs and right after they end; only with nothing left does it end. An asking counts only
    until its window has opened (`still_to_open`), which is at most its patience: a folder whose
    drive never answers no longer keeps the program (the sixth safety review of 2026-11-18, C3,
    above). Tests: `tests::a_folder_still_to_open_keeps_the_program`,
    `tests::a_place_whose_reading_never_comes_does_not_keep_the_program`. Not tried live (both are races of a
    moment); the folder windows' ordinary opens and hand-overs were tried again after it
    (`private/probes/review5/b1/live.ps1`).
  - **A start hands over to any program that answers; two never overwrite each other's
    folders.txt** (the third safety review of 2026-11-18, C5, and the fourth, B3). A start tries
    the program holding **the hub's place** (`Local\Bevel95.Folders.Hub.<copy>`, a mutex the first
    program owns for its life, `take_hub_place`; one left by a program that ended is taken over)
    first, then every second program already running (`running_hubs`: hidden windows of class
    `Bevel95Folders`, then `Bevel95FoldersSecond`), round after round (`hand_over_to`): each try
    waits at most 2 s, 300 ms while others follow in the round (a suspended holder kept every later
    start 2 s, tried live), and none for a program Windows calls hung (`IsHungAppWindow`,
    SMTO_ABORTIFHUNG). Only when none takes the folder within **10 s** (`HAND_OVER_PATIENCE`; it
    was 60 s, and every later start waited the 60 s again for the same program) does the start
    become a second program itself (logged: "no running folder windows' program took the folder
    in ... ms; this one opens it as a second program, without saving folders.txt, and later starts
    can hand over to it"); another start just making its window is waited for 2 s, as before. A
    second program never saves folders.txt (logged once); only the holder does. Test:
    `tests::a_start_hands_over_to_any_program_that_answers` (a simulated clock: the holder at once;
    a hung holder and a second that answers within 0.7 s; a holder busy for 4 s waited for; none
    answering: a program of its own after the 10 s, the second one asked too; a start making its
    window). Tried live (`private/probes/review4/handover/live.ps1`, a scratch portable copy, test
    folders only, its first program suspended): the first open after it 10.1 s (a second program),
    the next two 0.39 s and 0.37 s, in that second program; the first program resumed, all four
    windows closed, both ended, the owner's folders untouched. Before, each of them waited 60 s.
- **The control kit holds several windows of one kind** now (`winkit::kit`: `open_another`,
  `windows`, `scope`): a window handling a message is "the" window of its kind for every call by
  kind (`with`, `update`, `close`, menus); `scope(hwnd, ...)` does the same for work coming from
  elsewhere (a folder reader's new items). One-window kinds behave as before.
- **Shared, not copied** (commit 1): the desktop's file commands (`winkit::file_ops`: Paste and the
  shortcuts take their folder; each program sets its log word and its after-change hook), the
  questions (`winkit::confirm`: told the look, the window kind and the work area), reading an item
  (`winkit::shell`), HICON pixels (`winkit::icon`), COM calls (`winkit::com`) and the Create
  Shortcut wizard (`winkit::shortcut_wizard`). The bar keeps thin wrappers; its golden pictures,
  tests and comparison cases did not change. The icon view itself is the desktop's
  (`desktop_icons::Desktop` with `Host::Window`: rows instead of columns, white with black labels,
  no moving yet, icons kept where they were first put; `keep_order` for My Computer), its menus the
  desktop's (`desktop::item_menu`, `new_menu`, `arrange_menu`; `ItemKind::Drive` and `Place` for My
  Computer's items) and its pictures the look's (`desktop_art::Art::Drive`, `ControlPanel`,
  `Printers`, from `core::folder::art`). Solitaire's sizable frame became `window::paint_sizable_frame`
  (Solitaire's pictures unchanged).
- **The window** (`core::folder`): `FolderWindow` (a kit form), `menus` (File, Edit, View, the empty
  area's menu, help texts), `status` (words, sizes, first size, cascade), `saved` (folders.txt).
  Menus hold only what works; left out (todo): Find, Format, Print, Send To, Quick View, Undo,
  Toolbar, Small Icons, List, Details, Line up Icons, Options, the Help menu, the empty area's View.
  Program menus can now open submenus (`winkit::menu::track_cascade`: on the pointer resting 400
  ms, a click, Right or Enter; the parent shown still beside it), and show each entry's help text
  in the status bar.
- **Opening**: a folder or drive opens a new window here; one already open comes forward as it was;
  Backspace opens (or brings forward) the parent, a drive's parent is My Computer; Control Panel
  and Printers open Windows' own; a file opens with its program (ShellExecute's default verb). A
  shortcut to a folder opens Windows' Explorer (todo). Backspace in My Computer does nothing yet
  (Windows 95's message box, measured, is not built).
- **My Computer** (`read_my_computer`): every drive letter with the look's picture of its kind (A:
  and B: removable = 3½" floppy), named as Windows 95 named them: "3½ Floppy (A:)", "<label>
  (C:)", "(D:)" without a label; then Control Panel and Printers. A drive's free space in the
  status bar. Windows 95's Dial-Up Networking and Network Neighborhood: no counterpart.
- **Entry points**: in the Windows 95 look the desktop's My Computer and every folder on it open
  here (`desktop::open_in_folder_window`; if `folders.exe` is missing, Windows' Explorer as before);
  the item menu's Open too. The 98 and 2000 looks are unchanged; `folders.exe` started in them
  opens Windows' Explorer instead, and its windows close when the look leaves Windows 95.
- **Task buttons**: each window is a program window with the folder's name as its title and the
  classic 16×16 picture (My Computer, the open folder, a drive's) as its icon, so the bar shows
  them as any program.
- **State persists** (`folders.txt`, `core::folder::saved`): whether the status bar shows, and per
  folder (lower-case path or `::my-computer`) the window's place and size, maximized, Arrange
  Icons' order and Auto Arrange, written when its window closes (at most 200 folders). Setup's
  Remove deletes it; README lists it.
- **Resources**: every window, icon, timer and reader thread a folder window makes goes with it.
  Live: 30 folder windows opened and closed twice, the program's counts with one window were
  USER 17, GDI 46 before and after both rounds (137/136 with 31 open), `private/probes/folders/live.ps1
  -Phase leak`. The program logs its window resources every ten minutes as the bar does.
- **Live tests** go through a copy in `private/probes/folders/live` (its own settings.txt: the 95
  look at size 2, so its log and folders.txt stay there) on scratch folders of test files; the
  hidden window answers `WM_ICON_AT` and `WM_ICON_PLACE` (wParam: the folder window) for tests.
- **Step 2, Windows 98's and 2000's window (2026-11-13, the owner: "Proceed")**: the same program
  draws each look's own window; the "Entry points" bullet above is replaced by the last bullet
  here.
  - **Measured first** on both Reference PCs with the web view off (98: Folder Options > Custom >
    same window, web content only where chosen; 2000: "Use Windows classic folders"). 82 captures
    (51 of 98, 31 of 2000) `<era>-truecolor-folder-*` (each window plain, every toolbar button under the
    pointer and pressed, menus, the address list, Back's list, Up, Backspace, Alt+Left, a typed
    address, the Desktop as a place, new windows), the status bar's help text of every menu entry
    read off strip by strip (`private/probes/folders/seq/<era>/`). Numbers in theme-spec.md "Folder
    windows (Windows 98 and 2000)". Found on the way: 98's Classic style also turns on "own window
    per folder" (95's behaviour), so the captures use Custom; 2000 shows the address of a folder by
    its name, not its path (98: the path); 2000's address bar sometimes keeps the previous place's
    picture (a glitch of its own, masked, not copied); 98 names a drive without label " (C:)" with
    a space in front, in My Computer, the title bar and the list; IE's tooltips ("Back to My
    Computer") came under the pointer (not built, todo).
  - **The core** (`core::folder`): `explorer` lays out and paints the rebar (an etched rectangle
    one row under the title bar; bands 22, 40 (98) or 22 (2000), 22 rows; grippers; the throbber
    at rest at the menu band's end), the flat toolbar (pictures gray, colored under the pointer,
    etched while grayed; 98 names under the pictures, 2000 only Back's beside it; the split
    buttons' arrows and their pressed look, measured pixel by pixel), the address combo box and its
    list, and the three-part status bar (98: 240 | rest | 218 with the zone; 2000: rest | 76 |
    153). `history` keeps the places for Back and Forward and says what Up goes to (98 grays Up in
    My Computer, 2000 goes to the Desktop). `address` has the wording, the typed place, the edit
    field (caret, selection, Esc gives the address back) and the list's entries. `explorer_art`
    holds the pictures cut from the captures (two RGBA sheets in `art/assets`, cut by
    `private/probes/folders2/cut.ps1` from `spec98.txt`/`spec2000.txt`): colors from the captures (the
    shell's image lists change them slightly), see-through pixels from the PCs' own icon files;
    kinds the PCs had no example of (removable, network, RAM drives) are the files' own.
  - **Only what works** (as the desktop's menus): 98's toolbar Back, Forward, Up | Cut, Copy, Paste
    | Delete, Properties; 2000's Back, Forward, Up | Delete. Menus File, Edit, View and 98's Go
    (2000: View > Go To): Back, Forward, Up One Level, My Computer. Left out, not grayed: Undo,
    Views, Search, Folders, History, Move To, Copy To, the Favorites, Tools and Help menus, the
    other views, toolbar customizing, Work Offline, the Go menu's web pages (todo). Cut, Copy and
    Delete stay usable with nothing selected, as on the PCs (they do nothing then).
  - **The same window**: a double-click on a folder, Up, Backspace, Back, Forward, the lists of
    places, 98's File menu's places, the address bar and its list change the window in place
    (`navigate`): the old reader stops, a new one (with a new id, so a late answer of the old one
    is dropped) reads the new place; title, task button picture, address and buttons follow.
    Several folders selected and opened: each in a window of its own. Opened from outside (the
    desktop), a folder gets a new window, or the window showing it comes forward. The Desktop is a
    place (the look's own icons, then the owner's and the public Desktop folder). An address that
    is no folder is handed to Windows as Run would (a program, a file, a web address), and the
    address shown comes back.
  - **First size**: 98 sized a new window by its icons (31 + 75 per column, 117 + 75 per row;
    measured on 5 and 8 items) - its smallest and largest were not measured (Bevel95: at least three
    columns, at most 64% and 461/600 of the screen); 2000 opened every folder at 680 × 556 of 800 ×
    600, cut to the work area. Flagged in theme-spec.
  - **A change of look** turns open windows into the new look's (frame, menus, toolbar, pictures,
    names), keeping their items; in 95 they go on showing the place they show.
  - **Resources**: live, the scratch copy in each look: 30 windows opened and closed left the
    counts where they were (USER 16 → 15, GDI 46 → 46); 400 folder changes in one window: GDI 46 →
    49 after the first 200 (the window's 32×32 icon is made at the first change), then unchanged.
  - **Entry points**: the desktop's My Computer, My Documents (98 and 2000: its folder) and its
    folders open here in all three looks (`desktop::open_item`); if `folders.exe` is missing,
    Windows' Explorer as before. Explore still opens Windows' Explorer (step 3).
  - **Live tests**: `private/probes/folders2/live.ps1` (its own scratch copy in
    `private/probes/folders2/live`, settings.txt with the look given): `WM_TEST_COMMAND` now also goes
    into a test folder (101), Back (102), Forward (103), opens the address list (212) and Back's
    list (213).
- **Step 3, Windows Explorer with the tree of all folders (2026-11-13, the owner: "Yes do step
  3")**. The same program, the same window: `FolderWindow` with `tree_shown` and `exploring`.
  - **Measured first** (theme-spec.md, "Explorer's tree"): 43 new captures `<era>-truecolor-tree-*`
    on the three Reference PCs (`private/probes/tree/t.ps1`, snapshot mode, each PC stopped
    afterwards; 98 with "as Web Page" off, 2000 with classic folders, Large Icons in 95 and 98 so
    the icons match ours). Found: the entry opens a 600 x 411 window (2000: 680 x 458) on C: (95,
    98; the tree opened there) or My Documents (2000); 95 starts with the keyboard on the icons,
    98 and 2000 on the tree; the tree's selection shows only while it has the keyboard; a key
    moves the selection at once and the icons follow after a pause; Tab and F6 go tree -> icons
    (-> 98's and 2000's address bar) -> tree; Backspace in the tree goes to the folder above; a
    letter to the next row starting with it; opening a row scrolls as little as needed for its
    last row; labels are cut at the pane's edge and the tree scrolls across (no "..."); 98's bar
    keeps "Exploring - " in the title when closed, 2000 shows the Explorer picture only while the
    tree shows; the menu on a row has Explore in bold; 95's 16 x 16 folders really are the yellow
    and gray checkerboard (shell32 groups 4 and 5); a drive without a disk: 95 "A:\ is not
    accessible." with Retry/Cancel, 2000 its "Insert disk" box (98's PC had an unformatted disk
    in A: and asked to format it: not copied). Explorer added the drive's free space to the status
    bar (95 after the size, 2000 after the count).
  - **The core**: `folder::pane` (the `Tree` of rows read so far, `wanted`/`set_rows`/`unreadable`,
    `follow` a chain of ids down to the place shown, the keys, each look's `layout` and painting
    with `crate::tree::paint_row`, the row menu's entries), `folder::tree_art` (95's tree pictures,
    each Explorer picture from the PCs' explorer.exe: 95's 16-color image, 98's and 2000's
    256-color one, the images the PCs drew), `disk_box` (the not-ready / Insert disk boxes). The
    folder window lays out its panes from the tree pane's width (95 192, 98 196, 2000 200 at
    every window size; the bar between 3 or 4 wide), keeps the list's focus dots on the list's own
    corner, and gives up its focus frame while the tree has the keyboard.
  - **What the tree holds** (`tree_rows` in folders.exe): the Desktop; under it each look's own
    items in its order (95 My Computer, Recycle Bin; 98 My Computer, My Documents, Recycle Bin;
    2000 My Documents, My Computer, Recycle Bin), then the desktop's folders; under My Computer the
    drives (named as My Computer names them), Control Panel and Printers (95, 98); folders by name.
    No network places, Internet Explorer, Dial-Up Networking or Scheduled Tasks (no counterpart).
    Control Panel, Printers and the Recycle Bin open Windows' own when clicked (Enter, a
    double-click), not when keys pass over them; the tree then shows the place shown again.
  - **Reading off the window's thread**: each folder's rows on a "tree reader" thread (names as
    Explorer shows them, a plus box when the folder holds a folder: read until the first one),
    handed back by `WM_TREE_ROWS`; a 100 ms timer on the program's hidden window asks for the rows
    an open folder still needs, lets the icons follow a key-moved selection after 500 ms (Bevel95's
    pause, PROVISIONAL), and reads rows again after a file command on a row (1.5 s and 4 s later)
    or when the icons' reader sees the folders of the place shown change. A drive that takes
    disks keeps its plus without being asked (as 95 did).
  - **The menu on a row** (and Del, Ctrl+X/C/V, Shift+F10 while the tree has the keyboard): Explore,
    Open, Cut, Copy, Paste (with files on the clipboard), Create Shortcut (next to the folder; a
    drive's or a place's on the desktop), Delete (the look's question; folders only), Properties,
    all through `winkit::file_ops` and `confirm` as everywhere else. Left out (todo): Find..., Send
    To, Format..., Sharing, Rename (a label edit in the tree is not built), the Desktop row's menu.
  - **What a menu, a question, a rename box or a drag acts on is remembered by what it is, never by
    where it is** (the third safety review of 2026-11-18, A2): the menu on a row kept the row's
    index (`menu_row`); rows read while it was open (a slow drive, the re-read after a paste, a
    folder gaining a subfolder) moved the rows below, and Cut, Paste or Delete then acted on
    another folder. Reproduced live before the repair (`private/probes/review3/tree.ps1 -Copy old`,
    a scratch copy: the menu on "Bevel95 test c" open, "Bevel95 test a" opened meanwhile, its
    Properties chosen: "Bevel95 test a1 Properties" opened); after it, "Bevel95 test c
    Properties". Now `menu_row` holds the row's id (`FolderWindow::menu_on_row`, `menu_node`);
    a row gone when the menu closes gets no command at all (logged). Every such place was gone
    through, in the folder windows, their tree and the classic desktop:
    - the tree's menu row: an index, now the id; the tree's selection after a folder above could
      not be read (`Tree::unreadable`): kept its index, now kept by id (it went to the folder only
      when the index fell off the end); a drag resting on a row (`Hover`, which row opens): an
      index, now the id (it only opened a row);
    - the menu on icons and the menu bar's menus in a folder window: the commands took the
      selection when carried out, and items read while the menu was open could change it (a new
      item from New, Paste or Create Shortcut is selected as it shows); now the selection's ids are
      taken when the menu opens and put back before the command (`Desktop::select_ids`), refused
      when any of them is gone. That holds only for **a command that acts on the selection**
      (Open, Explore, Cut, Copy, Create Shortcut, Delete, Rename, the items' Properties, Empty
      Recycle Bin, Invert Selection: `menus::Command::acts_on_selection`); every other command (the
      views, Refresh, Up, Back, Close, Paste into the folder shown, New, Undo, Select All, the
      folder's Properties) runs whatever happened to the selection (the fourth safety review of
      2026-11-18, C4: in a busy folder they were dropped when a selected file was renamed or went
      while the menu was open; `Command::runs_after_menu`, test
      `menus::tests::only_a_command_on_the_selection_needs_the_selection_it_was_opened_on`);
    - a drag of a window's own icons (`OwnOut`) and of the desktop's (`OUT`): indices, which a
      refresh during the drag reorders, so the drop moved other icons (the review's suspected 3);
      now ids (`Desktop::ids_of`, `move_ids`);
    - the desktop's shared `TARGETS` slot (the review's suspected 2): a key and a menu on icons
      wrote one slot that the bar read when it handled the message, so a later key or click could
      replace what an earlier one acted on; now each selection is kept under a ticket of its own
      that travels with the message (`desktop::remember_targets`, `targets_of`; the last 16 are
      kept): `WM_DESKTOP_COMMAND`'s lParam carries the ticket and the keyboard flag, the menu on
      icons' wParam its ticket and lParam the point (16 bits each, signed), and the bar's popup
      keeps the ticket for the menu's life (`Kind::DesktopItem(kinds, ticket)`);
    - the desktop's double-click (`last_press`): the first press's icon by index, now by id;
    - already by id, unchanged: the selection, focus and anchor across a refresh
      (`replace_items`), the rename box (`rename.id`, and the rename acts only if the box's item is
      the one it began on), the icon to rename (`RENAME_REQUEST`), a new item to select
      (`PENDING_SELECT`, `select`), the cut icons, the re-reads after a tree command, Delete's and
      the extension warning's questions (they hold the items' paths). A drop's place
      (`DropAt::Row`, `DropAt::Item`, `drop_target`) is turned into a path in the same call as it
      is found, before any menu.
    Tests: `folder::tests::the_tree_rows_menu_acts_on_its_row_though_rows_arrive_while_it_is_open`,
    `pane::tests::the_selection_stays_on_its_row_when_a_folder_above_cannot_be_read`,
    `desktop_icons::tests::menus_and_drags_keep_their_items_by_id_when_items_arrive`,
    `desktop::tests::each_command_keeps_the_icons_it_was_given`. The tree's test menu (223) now
    opens only on a test folder's row.
  - **Entry points**: `folders.exe` without an argument is Windows Explorer (the Programs menu's
    entry: `start_menu::OWN_PROGRAMS` "Windows Explorer", in Programs for 95 and 98 and under
    Accessories for 2000, the look's Explorer picture; Find lists it first among equal matches);
    `folders.exe --explore <place>` an Explorer window (`file_ops::explore` starts it for folders,
    drives, My Computer and My Documents; Windows' Explorer for the rest or when folders.exe is
    missing). Explore inside a folder window opens Bevel95's Explorer directly. Every Explore and
    the entry open a new window (as in Windows); `window_key` keeps them apart from folders' own
    windows. 98: View > Explorer Bar > All Folders / None; 2000: the Folders button (`Tool::Folders`,
    66 wide, pushed in on a checkerboard while the tree shows, cut from the captures) and View >
    Explorer Bar > Folders; the bar's X in both. Windows 11's own File Explorer entry stays.
  - **95 too goes to folders in the same window** when it has the tree (95's Explorer did; its
    folder windows still open one window per folder).
  - **State persists** (folders.txt): `explorer x y w h max` (one place for all Explorer windows,
    written when one closes), `tree_width` (only after the bar between the panes was dragged; the
    look's own width otherwise), `tree <folder>` (98 and 2000: a folder's own window closed with
    the tree showing opens with it again). Setup's Remove and README's "Files it uses" already
    covered folders.txt.
  - **A few thousand icons**: a folder of 3000 files held the window up for about 30 s once its
    items came (`Desktop::relayout` searched every icon for every free cell); now a set of taken
    cells: 3000 items in 0.2 s (debug, `three_thousand_items_take_no_time`). Live, release build,
    3000 empty files: the window answered every 100 ms probe within 45 ms while the folder was read
    (reading the 3000 pictures takes about 13 s on the reader's thread; the window shows them when
    they are all read). A folder of more than 300 items is looked at less often (up to every 10 s).
  - **Resources**: live in each look (`private/probes/tree/live.ps1 -Phase leak`, scratch copy, release
    build): 30 Explorer windows opened and closed twice, then 300 open/close of a row with 30 rows:
    USER 17 -> 16, GDI 10 -> 10 in all three looks (31 windows: 146/100, 2000 166/101).
  - **Live tests** (`private/probes/tree/live.ps1`, its own scratch copy and settings in
    `private/probes/tree/live`): `--explore` on test folders, the tree's test commands
    (`WM_TEST_COMMAND` 220 open/close the selected row, 221/222 down/up as a key, 223 its menu, 224
    show/hide the tree, 225 Explore the first test folder; only in test folders' windows), keys
    posted to its own windows.
- **Small Icons, List and Details, and big folders that appear at once (2026-11-14, the owner: "Go
  ahead with your rec")**. The web view panel stays for later.
  - **Measured first** on the three Reference PCs by a measuring worker (snapshot mode, each PC
    stopped): 320 captures `<era>-truecolor-view-*`, the notes `private/probes/views/<era>-notes.txt`,
    numbers in theme-spec.md "Folder views". Found: every small view uses 17-pixel rows; only the
    name is highlighted in Details; 95 and 98 hide the selection of a window without the keyboard,
    2000 shows it gray; 2000's Details rows start 2 pixels under its header and it alone draws a
    sort arrow and sorts Modified oldest first; a second header click reverses exactly (folders
    last) in all three; Line up Icons and Auto Arrange are grayed in List and Details; Small Icons
    keep dropped places, List and Details move nothing; a band starting right of a name in Details
    selected nothing.
  - **Which view a window opens in** (`saved::opening_view`): the folder's own remembered view;
    else 95 Large Icons up to 79 items and List from 80 (77 and 80 measured), 98 Large Icons in the
    same window and the 95 rule in a new window, 2000 always Large Icons. 95's and 98's Windows
    Explorer has one view of its own (the one it was closed in; List at first) kept from folder to
    folder; 2000's Explorer uses each folder's own.
  - **The core** (`desktop_icons::views`, a child module of the icon view): `ViewMode` on the same
    `Desktop`, so selection, the keys, the menus on icons, renaming and every file command work in
    each view unchanged; each view's cells, pictures (16x16: the look's own for folders and drives,
    the shell's small picture for files), label boxes, hit tests by arithmetic in List and Details,
    the keys (List: Left/Right by a column, Page Down to the last column shown; Details: Page Down to
    the last whole row, then a page; Small Icons: a page down the column), painting of only what
    shows, Details' columns and header (`paint_header`, `header_hit`), sizes (`kb_text`) and dates
    (`date_text`: the platform gives local file times). `FolderWindow`: the view, header clicks,
    dividers dragged (the inverted line) and double-clicked, scrolling by rows (Details) and columns
    (List), `view_state`/`apply_view_state`.
  - **Menus**: View's four views with the dot (`menus::view_choices`), the empty area's View
    submenu first with a line under it, 98's Views button (next view; its arrow the menu) and 2000's
    (one drop-down), pictures cut from the captures into the explorer sheets. Left out: Thumbnails,
    Line up Icons, Customize this Folder, Choose Columns, as Web Page.
  - **State persists** (folders.txt): `view <mode> <order> forward|reversed <widths|-> <folder>` for
    each folder shown (at most 200), written whenever the view, the order or a width changes and when
    a window leaves a folder; `explorer_view = <mode>` for 95's and 98's Explorer. Setup's Remove and
    README's "Files it uses" already covered folders.txt; the owner's checklists (private/checklists.md) say what it now holds.
  - **Big folders**: the reader names every item from its kind (`shell::kind_of`: one SHGetFileInfo
    with SHGFI_USEFILEATTRIBUTES per extension says whether Explorer shows it, the type's name and
    its picture's place; read-only and system folders are asked for their own names) and posts them
    at once; then it asks each file for its own picture, those on the window's screen first
    (`WANTED`, published by the 100 ms timer), 24 at a time (`WM_PICTURES`, `Desktop::set_pictures`).
    Pictures are read once per place in the shell's image list (`shell::pictures_at`, `ShellPicture`
    shared by `Arc`), the store holding at most 2048 and dropping the least used quarter beyond it;
    no window resources are kept (each HICON is destroyed after its pixels are read). The view
    measures each label once (`label_sizes`), sorts with keys made once, and paints only what shows.
  - **Timings** (live, release build, the 2000 look at size 2, `private/probes/views/live.ps1 -Phase big`):
    3000 files named in 13-23 ms, shown 45-72 ms after the window asked (was about 13 s), the first
    screen's pictures 62-80 ms after the start, every picture about 6 s; 10000 files shown 73-157 ms
    after asking, the first screen's pictures within 108 ms; a header sort answered within 12 ms;
    20 Page Downs within 12 ms each; the window answered every probe within 11 ms except once 78 ms
    (10000 files in Large Icons, the labels measured the first time).
  - **Resources** (`live.ps1 -Phase leak`): the 10000-file folder opened and closed five times and
    200 changes of view with 20 folder changes left the counts where they were (2000: USER 19, GDI
    67 before and after each round, 70 after the view changes: the window's 32x32 icon made at its
    first folder change, as before; 98: 19/65 after each round, 68 after them; 95: 19/65 after each
    round and after 200 view changes).
  - **Proof**: unit tests (each view's layout, header clicks, column drag and fit, the keys, 10000
    items scrolled, painted and sorted, the opening view, folders.txt), golden pictures
    `<era>-folder-details`, `-folder-list`, `-folder-small` (new; 98's and 2000's explorer pictures
    changed by the Views button only), compare-refs cases `vm-<era>-view-*`.
  - **Live tests**: `private/probes/views/live.ps1` (its own scratch copy and settings in
    `private/probes/views/live`): `WM_TEST_COMMAND` 230 + view and 240 + header column (test folders
    only); phases views, commands (Copy and Paste, Rename typed in List and Details, checked on
    disk), big, leak, clean. Run in all three looks: views, commands and leak in each, big in 2000.
    Not tried live: real clicks and keys, dragging a divider, the Views button and menus by the mouse,
    the owner's own folders.

- **Step 4, the web view panel (2026-11-15, the owner: "Yes")**: the panel Windows 98 and 2000 showed
  at the left of a folder's contents, drawn by Bevel95 itself (no web page, no browser part).
  - **Measured first** by a measuring helper on both Reference PCs (snapshot mode, each PC stopped):
    106 captures `<era>-truecolor-web-*`, the notes `private/probes/web/<era>-notes.txt`, numbers in
    theme-spec.md "The web view panel". The templates the real panel came from were read out of the
    PCs' disks for the wording and the rules (98: `Windows\Web\folder.htt`, `mycomp.htt`, the
    Windows folder's own `folder.htt`; 2000: `webvw.dll`'s resources and strings; Microsoft's files,
    in `private/reference/refs/files/<era>/htt` and `webvw`, never committed). Found: 98's panel is 30% of the
    contents (rounded) and goes under 400 pixels, leaving a 32-row band with the title above the
    icons; 2000's is 200 wide and goes under 400 with nothing in its place; both keep 16 columns
    for a scroll bar; no font smoothing in either. 98's "as Web Page" is kept per folder, 2000's
    switch is global (Folder Options); 98 forgets "Show Files", 2000 remembers it per folder.
  - **The core** (`folder::web`): `Web` (on, the place's kind and 32×32 picture, the system folder's
    `Guard`, files shown, a drive's space, the scroll, attributes), `panel_width`, the text of each
    look (98 after `folder.htt`/`mycomp.htt`: `<br>` lines; 2000 after webvw: paragraphs 12 apart,
    "See also:" links, the yellow note), word wrapping as Internet Explorer did (a no-break space
    keeps "6,695 bytes" together), the pie chart (shell32's 3-D pie with its triangle-area rule),
    the warning page with the gears (`logo_picture`) and 98's band. `FolderWindow` puts the icons
    beside the panel (`whole_inside` / `inside`), follows the selection at every paint (the focused
    item first, as `SelectedItems` listed them; at most 101 looked at), hides the icons while the
    warning shows, and gives the links (`Part::WebLink`) and the panel's scroll bar
    (`Part::WebScroll`). Pictures cut from the PCs' webvw.dll by `private/probes/web/cut.ps1` into
    `art/assets/web/*.pic` (palette and runs; on white, as drawn).
  - **Fonts**: 98's text is Verdana 8pt and its bold, its title Verdana 16pt bold (21 px); 2000's
    text is the look's Tahoma, its title Tahoma 13pt bold (17 px): rendered by GDI at 96 DPI without
    smoothing like the other atlases (`winkit::font::WEB_FONTS`, fixtures in `core/tests/fixtures`).
    Each matches the captures exactly except two of today's Verdana glyphs, corrected as Tahoma's
    were (`text::PERIOD_GLYPHS`): bold 21 px "v" (its strokes meet a row later) and 11 px "/".
  - **Windows 2000's Folder Options** (older::options, Tools > Folder Options..., the only entry of
    2000's Tools menu here; FolderOptionsHost in folders.exe, owned by the folder window): its General
    page with Web View and Browse Folders only (Active Desktop, Click items as follows and the other
    tabs left out, not grayed; the page shorter by those groups), the pictures that follow the choice
    (cut from the captures by 	mp/probes/web/cutfo.ps1), Restore Defaults, OK, Cancel, Apply (grayed
    until something changes). Its text is MS Sans Serif, not Tahoma (the captures). Apply and OK keep
    the choice (classic_folders, own_window) and every open 2000 window follows at once, as 2000's
    did. "Open each folder in its own window" makes a double-click (Open) on a folder open a window of
    its own (95's way; Explorer windows with the tree still go in place). The Tools title changed the
    2000 folder windows' menu bar: their golden pictures were drawn again (only the title differs); the
    comparisons already masked 2000's titles Bevel95 lacks, where Tools now stands.
  - **Switches**: 98's View > as Web Page (and the Views button's and the empty area's View menu),
    per folder, `web_off <folder>` in folders.txt; 2000's global "Use Windows classic folders",
    `classic_folders = yes`. On by default in both. 98 opens a folder with the web view at its
    largest first size (512 × 461 of 800 × 600), as it did.
  - **System folders** (`guard_of`): the Windows folder, System32 and (98) System hide their files
    behind the warning and "Show Files"; Program Files too in 2000, while 98 showed its files with a
    warning above the prompt; Program Files (x86) counts as Program Files. 2000's choice is kept per
    folder (`shown_files <folder>`). A folder named "Bevel95 test guard..." stands in for one in the
    live tests. The page is built as measured and asked as a question in todo.md.
  - **Links**: 2000's My Documents and My Computer go there in the same window; My Network Places,
    Network and Dial-up Connections and Add/Remove Programs open Windows' own. Pointing at one puts
    its words in the status bar (98: ":Show Files"). Not built: the hand pointer and the tooltip.
  - **Left out**: the preview of a selected picture (98 160 × 160, 2000 120 × 120: Internet
    Explorer's thumbnail control; nothing in the project reads a picture file's pixels), the panel's
    horizontal scroll bar (wider content is cut at the edge), 98's and 2000's other templates
    (Control Panel, Printers, the Recycle Bin open Windows' own windows here).
  - **Speed and resources** (`private/probes/web/live.ps1`, scratch copy, release build): 3000 files
    named in 67-75 ms and shown 115-125 ms after asking with the panel, the same with it off on the
    same day (67-71 / 116-119 ms; the earlier 13-23 / 45-72 ms were another day's disk); Select All
    of 3000 answered at once. 30 windows opened and closed twice: USER 19 -> 17, GDI 47 -> 47; 300
    selection changes and 60 folder changes: GDI 50 (the window's 32×32 icon made at its first
    folder change, as before), in both looks.
  - **Proof**: unit tests (`folder::web`: the widths, each look's wording and lines for nothing, a
    file and several selected), golden pictures `<era>-folder-web`, `-folder-web-drive`,
    `-folder-web-warning` (new; no earlier picture changed), compare-refs `vm-<era>-web-*` (12 cases;
    masked: the PCs' file pictures, what Bevel95 leaves out, the pie's GDI rounding, and 2000's icons
    beside the panel, whose first column the captures put 8, 16 or 21 pixels from it).

- **Real mouse, real keys (2026-11-15, the owner: "the right click in explore mode a drive or folder
  is funky. Doesn't respond and stays up")**.
  - **The cause**: the control kit captures the mouse from a right or middle button's press to its
    release (`core::kit::Controller::button`) and let it go only *after* what the release did. A
    right-click's menu runs its own message loop inside that (`winkit::menu::track`), so the folder
    window still held the mouse: the menu never saw the pointer (no entry lit up), a click on it went
    to the window under it, and a click elsewhere did not take the front from the menu, so nothing
    closed it (Esc did). The title bar's menu let go itself (`track_window`), which is why only it
    worked. Now the capture goes first; this also covers every other program's right-click menu.
  - **The lesson**: every earlier live test drove the Explorer by posted messages and test messages,
    which never go through the mouse capture, the window in front or the pointer's real place. Menus
    and anything held by the mouse must be tried with real input (`private/probes/mouse/m.ps1`: a scratch
    copy, real clicks, drags, the wheel and keys, each refused unless the window under the pointer or
    in front is the scratch copy's, an 8 s idle gate in which only our own input does not count;
    `sheet.ps1` puts several photographs side by side).
  - **Found by the same pass and fixed**: a right press gives the keyboard to the pane pressed (so the
    item right-clicked shows selected under its menu in 95 and 98, as Windows' list and tree views
    did); a menu bar's menu opened by a click no longer shows its first entry highlighted (only by a
    key), and a click on File then Right without moving the mouse no longer comes back to File (the
    move Windows sends when a menu window appears under a still pointer is not the pointer moving);
    Details' header buttons had no rectangle for the kit, so a real click never sorted; the address
    box scrolls to keep the caret in sight. What the pass found working, and what is still wrong or
    not tried, is in todo.md ("Folder windows").
  - **Seen once, not explained**: during one test a test text file of ours was opened by the program
    (its parent was the scratch copy) between two single clicks 1.2 s apart on it; repeating the same
    clicks on a folder did nothing. The owner may have touched the test window at that moment.

- **What the owner found, and the faults left (2026-11-16)**. Measured first by a measuring helper on
  the three Reference PCs (snapshot mode, each stopped; `private/probes/measure17/summary.txt` and
  `<era>-notes.txt`, captures `private/reference/refs/vm/<era>-<depth>-m17-*`). Note: the PCs' clocks run slow
  (98 at about a third of real time), so every time there was converted to Windows' own.
  - **Pointers** (the owner: dragging the column lines and the bar between the panes works, but no
    two-sided arrow showed, so the spot could not be found). A form now says which pointer it wants
    where (`core::kit::Pointer`, `Form::pointer`; the window host answers WM_SETCURSOR with it,
    `winkit::pointer`). Measured and built: over a Details column line every Windows showed the header
    control's own pointer (a double bar crossed by a two-sided arrow; today's comctl32 still has it,
    cursor 106, loaded from there) from 8 pixels left to 7 right of the next column's first pixel
    (16, at 1×; a press there takes the line, so `header_hit` uses the same zone: it was 7 wide);
    over the bar between the panes 98 and 2000 showed the plain two-sided arrow (IDC_SIZEWE) over the
    bar and the tree pane's two edge columns (6 wide; the press zone too), 95 its own split pointer
    over the 3-pixel bar only: not among today's pointers, so it is drawn from its measured pixels
    (`kit::SPLIT95`, its white outline guessed: the PC showed it over white). The address bar and the
    rename box show the I-beam, a web view link the hand (IDC_HAND: Internet Explorer's hand was the
    same shape). The window's edges and corners already showed Windows' sizing arrows (the host's hit
    test). Pointers are drawn as big as Windows draws its own (SM_CXCURSOR), not by the look's size.
    Tried with real input in the 98 look at size 2: every pointer at the measured pixels, a column
    line and the pane bar dragged.
  - **A single click never opens** (the file opened once between two clicks): Windows makes a
    double-click of two presses close in time and place whatever they land on; Explorer opened only
    when both landed on the same item (the PCs: a quick click on another item only selects it). The
    window keeps the item its last press landed on (`first_press`); a double-click whose first press
    landed elsewhere, or never came to this window, is a plain press. The cause of the one opening
    seen was not found (it needed two presses within 500 ms and 4 pixels); this rule closes every
    path by which a press not on that item could open it.
  - **Renaming by a slow second click** (`slow_rename`): a press on the name (not the picture) of the
    one item selected, while the icons have the keyboard, renames once Windows' double-click time has
    gone by without a double-click or a drag (the PCs: any later second click; the box came about
    one double-click time after it). The program's 100 ms timer carries it out as F2.
  - **The first click on Name** in a window sorted by name keeps the order on all three PCs (2000's
    arrow already shows before it): Bevel95 did so already; nothing changed.
  - **2000's Folder Options**: Restore Defaults makes Apply usable even when nothing differs (as the
    PC). The title bar's "?" still does nothing: only three of its pop-up texts were read off the PC
    (in the notes), so the rest would be guesses (todo).
  - **An address that cannot be opened**: the typed text stays in the address bar until Windows says
    whether it opened; if not, the look's message (`disk_box::DiskBox::address`: 98 titled with the
    typed text, 2000 "Address Bar", each with its own words; USER's message box: error picture, text
    from 62 wrapped to at most 5/8 of the screen, OK centered; 494 and 485 wide as measured), then the
    typed text selected with the keyboard in the field. Something that opened puts the place's
    address back, as before. Windows treated a single word as a web address (the Internet Connection
    Wizard): here it gets the same message.
  - **A typed name means the folder shown** (found by the second safety review of 2026-11-18, C2):
    a relative name ("Logs", "..", "notes\a.txt", "\Windows" from the drive's root) was looked up in
    the program's working folder, which was Bevel95's own data folder (the desktop shortcut's
    "Start in"; System32 when started by the Run entry): "programs" opened Bevel95's own programs
    folder, looking like the expected one. Now `address::typed_place` takes the folder the window
    shows (the Desktop's: the desktop folder) and joins the name to it ("." and ".." taken away,
    never above the root); with My Computer shown it stays relative and is never taken for a
    folder (`go_typed` takes only a whole path). A file found so is handed to Windows by its whole
    path, anything else as typed (a program on the PATH, an address), with the folder shown (else
    the person's own folder) as the folder it starts in. What Windows' own address bar did with a
    relative name was not measured on the Reference PCs (none was started for it): the rule is the
    owner's brief. Anything with a colon (an address, `shell:`) is left as typed. Test:
    `address::tests::a_relative_name_is_looked_up_in_the_folder_shown`.
  - **Run** (`dialogs::run_command`): a relative name is resolved as Windows' own Run does: in
    the person's own folder (FOLDERID_Profile, where Windows' Run starts programs), then the PATH
    and the registered programs (ShellExecute), which gets the person's folder as the folder to
    start in. Test: `dialogs::tests::run_finds_a_relative_name_in_the_persons_folder`.
  - **Every Bevel95 program makes the person's own folder its working folder when it starts**
    (`shell::start_in_profile`, first in the `main` of the bar and of folders.exe, Minesweeper,
    Solitaire, Calculator and the Help viewer): a program started from the Start menu, a folder
    window or Run without a shortcut of its own never gets %LOCALAPPDATA%\Bevel95 as its folder,
    and nothing relative can resolve into it. A shortcut's own "Start in" still wins (Windows
    applies it to the program the shortcut starts). Checked first that nothing relies on the
    working folder being Bevel95's: every data file comes from `log::data_dir` (LOCALAPPDATA or
    the program's own file's folder); companions, the Help viewer and the watchdog are found from
    `current_exe`; Setup's paths are whole and its starts pass their folder (`current_dir`); no
    `fs`, `Path::new` or `Command::new` outside the tests takes a relative name; Rust's
    `Command` does not look in the working folder. Two names handed to ShellExecute without a
    path, `shutdown.exe` (Shut Down) and `explorer.exe` (Explore of a place Bevel95 does not show),
    which ShellExecute would also look for in the working folder, now go by their whole paths
    under %SystemRoot%. comctl32.dll for the header divider's pointer is loaded as a picture file
    only (no code of it runs) and stays as it was.
  - **Every program or panel of Windows that Bevel95 starts goes by its whole path** (the third
    safety review of 2026-11-18, C1: the other bare names were left, and with the person's own
    folder as the working folder, a file named `control.exe` or `ncpa.cpl` there would have run
    instead of Windows' own). `shell::system_program(name)`: Windows' system folder
    (GetSystemDirectoryW; %SystemRoot%\System32 if that fails) joined with the name. Now so:
    Control Panel (`control.exe`) and Network (`ncpa.cpl`) in Start > Settings, Network
    Connections and Power Options (`ncpa.cpl`, `powercfg.cpl`: `status::connections`,
    `power_options`), the volume control's mixer and sound settings (`SndVol.exe`, `mmsys.cpl`),
    the web view's Add/Remove Programs (`appwiz.cpl`), and the Create Shortcut wizard's panel
    (`rundll32.exe <system>\appwiz.cpl,NewLinkHere`; tried by hand: the wizard opened, Cancel left
    nothing). The `ms-settings:` and `shell:` places and web addresses are no files and stay. What
    the person types into Run or an address bar keeps Windows' own rules. Test:
    `platform/win/tests/whole_paths.rs` (the bar's, winkit's and the programs' sources: no
    `launch`, `shell_open`, `ShellExecuteW` or `Command::new` of a bare name, no constant naming
    one, no rundll32 panel by its bare name; the checker finds each of the old lines).
  - **Submenus**: with a submenu open, the pointer onto another entry of the parent lights it at once;
    resting there 400 ms the submenu closes and that entry's own submenu opens after the menu delay
    (98: lit at once, closed about 0.4 s later; `winkit::menu`, `Parent`, `parent_switch`); back into
    the submenu before that, nothing changes.
  - **Status bar words** of the menus on the icons and on the empty area, entry by entry as the
    shell's (`menus::item_help`; the empty area: 95 names what each submenu holds, 98 and 2000 show
    nothing for the submenus and Refresh); the menu on a tree row blanks the status bar in 95 and 98
    and leaves it as it is in 2000.
  - **Cut names**: Large Icons already showed a name whole on the focused icon while the icons had
    the keyboard (98, 2000; 95 never cuts), as measured. Not changed: with the keyboard in the address
    bar 98 and 2000 cut it again (98 hid the selection, 2000 kept it gray); Bevel95 keeps showing it
    whole there (the focus frame a golden picture and its capture show must stay: one flag drives
    both). Details keeps "..." when selected on all three (as built).
    Not built: the little boxes with the whole name (98: in place over a cut Large Icons label after
    about 1 s; 2000: an infotip with name, type and size 22 pixels under the pointer after about 1 s;
    the tree's clipped labels in place at once), 98's Small Icons and List spacing by the longest name.
  - **The window drawn inactive while in front** (seen once): not reproduced. The program's 100 ms
    timer now looks for it; if it happens the log says "a window in front was drawn inactive" and the
    window is drawn active.
  - **Windows 98's toolbar pictures** (the owner compared them with copy.sh's Windows 98): Bevel95's
    are 98's own 256-color set (SHDOCVW.DLL 4.72.3110, Internet Explorer 4.01 SP1, 98 first edition),
    which 98 drew at 256 colors, High Color and True Color alike; copy.sh's picture is 98 running in 16
    colors, which drew the separate 16-color set (halved, its Cut matches the 16-color capture pixel
    for pixel). Nothing changed; the question is in todo.md, both sets cut in
    `private/probes/measure17/toolbar98`.

## Dragging files (2026-11-16, the owner: "Fine to work on remaining faults and dragging files")
- **Measured first** on the three Reference PCs by a measuring helper (snapshot mode, each PC
  stopped): theme-spec.md "Dragging files", `private/probes/drag18`, captures
  `private/reference/refs/vm/<era>-truecolor-drag-*`. The rules: within one drive a move, to another drive a copy,
  a program (.exe) a shortcut in 95 and 98 (2000 moved it); Ctrl copies, Shift moves, Ctrl+Shift
  makes a shortcut, the mark changing at once; the right button's menu at the drop (Move Here, Copy
  Here, Create Shortcut(s) Here, Cancel; 2000 Copy Here first; the left drag's choice bold); Esc and
  the other button cancel; only 98 asked before a drop on the Recycle Bin; Undo for moves, copies,
  renames and deletes, several steps back, Undo Copy asking first.
- **One way for everything: Windows' drag and drop (OLE)** (`winkit::dragdrop`, hand-written COM
  objects as `winkit::com` does: windows-sys has no interfaces). A drag out of a Bevel95 window gives
  Windows the shell's own data object of its files (SHCreateDataObject on their item lists, as
  Explorer gives them), so Windows' Explorer and every program that takes files take them; a drop
  target on every folder window and on the desktop reads the dropped file names (CF_HDROP) from
  anything: a folder window, the desktop (the bar's program), Windows' Explorer, a program. Between
  Bevel95's own two programs it is the same way. A drag of anything but files (a web page's text or
  picture) is refused. New windows-sys feature: `Win32_System_Com_StructuredStorage` (STGMEDIUM) in
  `bevel-winkit`.
- **Who does the work**: the window a drop lands on, with Windows' file machinery
  (`file_ops::drop_files`: FO_MOVE, FO_COPY with FOF_ALLOWUNDO, its progress window and its own
  questions, a copy into the same folder getting Windows' "- Copy" name; shortcuts as Create
  Shortcut makes them). A move it carried out is reported to the source as "nothing", so no source
  program deletes anything afterwards (the shell's "optimized move"). Bevel95's own drags offer
  move, copy and shortcut; another program offering only a copy gets a copy.
- **A drop does only what the dragging program allows** (found by the safety review of
  2026-11-18, A1: the handlers carried out the look's wish from the keys and drives and used the
  source's offer only for the pointer and the answer, so a file dragged from a browser's list of
  downloads, which offers only a copy, was moved on the same drive and binned on the Recycle Bin).
  The effect carried out is the one the pointer shows, decided once before anything is done, for
  DragEnter, DragOver and Drop alike (`dragdrop::drop_effect`, the rule `core::drag::narrow`, pure
  and tested with every wish and offer): the look's wish (`effect_for`) if offered; else a copy if
  offered, else a move if offered, else the "no" sign; a shortcut only where one was wished for.
  The Recycle Bin takes only what its source lets be moved (else the "no" sign). The right
  button's menu shows only what the source offers (`menu_effects`), its bold entry the narrowed
  effect. Told back: a move carried out by Bevel95 as nothing (so the source deletes nothing), a
  copy as a copy; a source offering only a move gets a move, also to another drive, never a copy
  answered by "moved". Explorer's and Bevel95's own drags offer all three, so they behave as
  before. Tests `a_drop_does_only_what_the_source_offers`,
  `the_right_buttons_menu_offers_only_what_the_source_offers` (core) and
  `a_drop_carries_out_what_the_source_offers` (winkit: the look's wish was a move, the drop now a
  copy). For live tests folders.exe takes a drop "from another program" by a test message
  (`TEST_DROP` in WM_COPYDATA to its hidden window: the offered effects and test items only, into
  a test folder's window; `dragdrop::test_drop` carries it through the same DragOver and Drop as
  OLE does). Tried live (2026-11-18, `private/probes/drops/drop.ps1`, a scratch copy of folders.exe
  with its own data folder, only "Bevel95 test" items): offered a copy only, same drive: copied
  (told "copy"); offered all three: moved (told "nothing"). A real drag by Windows' drag loop needs
  a pressed button (a drag started without one returned at once and dropped nothing), so no real
  mouse drag was made; the desktop's handler, the Recycle Bin and the right button's menu are
  covered by the tests only.
- **Files that vanish when the drag call returns** (the safety review of 2026-11-18, "suspected
  1", proven): archive and mail programs hand over files they extracted to a temporary folder and
  delete them as soon as their DoDragDrop returns; Bevel95's copy started on a thread after its
  Drop had returned and found them gone. Proven live with a throwaway source of our own
  (`private/probes/drops/drop.ps1 -Phase testdrop -DeleteAfter`: the drop by folders.exe's test
  message, answered when the drop handler returns, as DoDragDrop returns to a real source; the
  file deleted the moment the answer came): a 21-byte and a 64 MB file, offered as a copy, both
  never arrived (Windows' file machinery said 0x2, file not found; the Undo list emptied). Now
  (`dragdrop::way_of`, `drop_job`, `wait_for_drop_jobs`), for a copy or move of a drop:
  - **A drag of Bevel95's own** (in the same program, or between its two programs: `run` marks
    its data object with the registered format "Bevel95 drag") is carried out as before, the drop
    answered at once: its files stay where they are until Bevel95 moves them.
  - **A source that says it can wait** (IDataObjectAsyncCapability, async mode on) is told when
    the work starts (StartOperation) and when it is done (EndOperation from the job's thread, the
    capability marshaled to it; told what the source is told at the drop: a copy as a copy, a move
    as nothing); the drop answers at once.
  - **Any other source** gets its answer only once the copy or move is done, as Windows' Explorer
    does for such sources: the drop waits for the job's event, handling the window's messages
    meanwhile (MsgWaitForMultipleObjects; a WM_QUIT taken meanwhile is posted again), so the window
    keeps painting and answering; Windows' progress window runs on the job's thread. The source's
    drag loop waits meanwhile. Shortcuts made by a drop and drops on the Recycle Bin are carried
    out as before.
  - New windows-sys feature `Win32_System_Com_Marshal` (CoMarshalInterThreadInterfaceInStream) in
    `bevel-winkit`. Test `a_drop_from_another_program_answers_once_its_copy_is_done` (the drop
    waits for a copy on another thread; the source deleted afterwards; the copy whole; a drag of
    Bevel95's own does not wait). Tried live after the repair, the same as above: both files
    arrived whole (the drop answered after 99 and 98 ms); a 4000-file folder of 250 MB: the drop
    answered after 15.8 s, the window meanwhile answering 251 times, the longest wait 4 ms. Not
    tried live: a real archive or mail program, a source that can wait, and whether today's File
    Explorer says it can wait (a fresh shell data object does not, by the test; if Explorer's
    drags do not either, its window's drag waits for Bevel95's copy, as for any other such
    source: its window keeps painting, the files are the same).
- **What can be dragged**: files and folders, from every view of a folder window and from the
  desktop. A drive, Control Panel, Printers and the look's own icons never leave their window (Windows
  made shortcuts of them: not built); on the desktop they still move about.
- **Where they can drop** (`core::drag`, `FolderWindow::drop_at`, `Desktop::drop_target`): a
  folder's icon, a drive, My Documents and the Recycle Bin (each drawn selected while the pointer is
  on it), My Computer (drawn selected, refusing, as the PCs did), a row of Explorer's tree (selected,
  the old selection without fill), the empty area or a file's icon (the folder shown; the desktop's
  own folder for the desktop). Refused with the "no" sign: a folder into itself or below itself, a
  move to where the files already are (in another window of the same folder), anything outside the
  icons and the tree (title bar, toolbar). A program's icon takes nothing (Windows opened the program
  with the files: left out).
- **In its own window**: Large and Small Icons move the icons to where they were dropped (the
  desktop's code: `Desktop::move_by`), remembered per folder in folders.txt (`places`), forgotten by
  Arrange Icons; List and Details move nothing. A file dropped into the folder a window shows lands
  where the pointer let go (`place_new`), the next ones beside it; the same on the desktop.
- **The pointer and the picture**: the look's own drag pointers from the measured pixels
  (`drag::pointer`, a colour cursor made by CreateIconIndirect at Windows' pointer size); the "no"
  sign's hot spot was not measured (its middle). The dragged picture (`Desktop::drag_picture`, each
  look's way) rides in a see-through window of its own (layered, click-through) beside the pointer,
  shown only over Bevel95's windows (98 drew none over other programs). Not copied: 95's and 98's
  pointer drawn into the picture and clipped to the window under it, 95's checkerboard following the
  screen's pixels while moving, the dotted outlines' XOR (drawn in the inverse of the window's ground).
- **Keys and buttons**: a Ctrl or Shift press on an icon can now start a drag too (Ctrl unselects an
  icon already selected at the release, when no drag came); the right button drags with the menu at
  the drop; Esc or the other button cancels.
- **The tree under a drag**: a closed row opens (2000: after 1 s still; 98 and 95: at a move after
  400 ms, PROVISIONAL), and the tree and the icons scroll a row every 150 ms within 16 pixels of their
  edges (Bevel95's, PROVISIONAL: not measured).
- **The Recycle Bin**: 98 asks Delete's own question first (on the desktop the bar shows it); 95
  and 2000 send the files at once, as Delete does after its question (to the Recycle Bin, with Undo).
- **Undo** (`core::drag::UndoList`, `winkit::undo`): one list for the desktop and every folder
  window, as the shell's was, in `undo.txt` in the data folder (two programs share it; at most 10
  steps, PROVISIONAL: the PCs went back several without a limit being reached). Every move, copy,
  rename and delete made in Bevel95 (drops, Paste, Rename, Delete) adds a step; a shortcut made by a
  drag does not (98). Edit > Undo first in the Edit menu with each look's words (2000 with Ctrl+Z
  beside it; grayed "Undo" when there is nothing), the status bar's "Undo Move of 'a3'", Undo on the
  empty area's and the desktop's menu after Paste Shortcut while there is something, Ctrl+Z in folder
  windows and on the desktop. Taking a step back: a move goes back to its old folder, a rename back
  to its old name, a copy goes to the Recycle Bin after the look's question ("Are you sure you want
  to delete 'x'?"), a delete comes back out of the Recycle Bin with the Recycle Bin's own Restore
  (its "undelete" verb on the items its records say were these, deleted at that time). A step that
  cannot be done (an item gone, its old place taken) changes nothing and gets the look's message
  (98's and 2000's measured wording for a rename; the same pattern for a move or a copy).
- **Undo takes back only what it can prove** (2026-11-17; an outside review of 2026-11-17 found
  three faults: a copy took every name new in the destination folder for a copy, so a download
  arriving meanwhile went to the Recycle Bin with Undo Copy; a move recorded a folder merged into
  one of the same name, so Undo carried back files that were always in the destination; undo.txt
  had only a lock inside each program and was written in place, so the two programs could lose
  each other's step or read half a list). The owner asked whether Undo should go for being hard to
  get right; the main session decided to repair it now and keep the repair inside `file_ops.rs`,
  `undo.rs`, `core::drag` and their callers, so it can still be taken out.
  - **Decided before, checked after, item by item** (`core::drag::plan`, `moved`, `copied`, pure
    and tested): before a move or copy each item gets its place at the destination and whether
    that place was free; afterwards an item is recorded only if its place was free and it is there
    (a move: and gone from where it was; a copy: and like its source: a file of the same size and
    time of change, which Windows' copy keeps; a folder of the same number of items and total
    size). An item that merged into or replaced something is not recorded. Nothing else of the
    destination folder is ever looked at.
  - **A copy into its own folder**: Bevel95 chooses the name Windows would give ("a - Copy.txt",
    "a - Copy (2).txt"; a folder's dot is no extension, "v1.2 - Copy"; a file's last dot is,
    "arc.tar - Copy.gz": measured on Windows 11 with FOF_RENAMEONCOLLISION) and hands Windows the
    whole destination (FOF_MULTIDESTFILES), so the name is known exactly. Windows' own answer (the
    name mappings of FOF_WANTMAPPINGHANDLE) was tried too and works, but windows-sys has no type
    for its memory layout (it would be written by hand), and a name known before the copy fits the
    same "free before" check as every other item.
  - **A copy carries its proof** (`Proof`: a file's size and time of change; a folder's number of
    items, total size and newest change, read through the whole folder). Undo Copy sends to the
    Recycle Bin only copies still exactly as made, after the look's question, checked again after
    Yes; a copy changed since stays and the look's message says so (`copies_left`, Bevel95's words,
    PROVISIONAL: "Cannot delete file: 'x' was changed after it was copied, so Undo left it.").
    Moves, renames and deletes need no proof (taking them back destroys nothing).
  - **What cannot be taken back empties the list** (`Recorded::Unproven`, PROVISIONAL, todo.md
    "Waiting for you"): after a merge, a replacement or any move or copy of which nothing could be
    recorded, Undo is grayed until the next step; it never reaches past the last operation to an
    older step. Part of an operation recorded: a step of that part. The one exception since
    2026-11-18: an operation that provably did nothing (below).
  - **One list, two programs**: every change of undo.txt happens inside a named mutex of the
    copy (`Local\Bevel95 undo list.<copy>`, one per copy since the fifth safety review of
    2026-11-18, B1, as each copy has its own undo.txt; taken by both programs and every thread; a wait of 3 s; a
    change that cannot get it is not made, logged, and undo.txt is removed rather than left
    reaching past it), and the file is written under a temporary name and put in place, so it is
    never read half written. The file starts with `bevel95 undo 2`; a file of the older format is
    dropped without an error. Test: two threads each adding 20 steps keep all 40 (without the lock
    21 were left).
  - **Undo Delete** restores only Recycle Bin records deleted within the operation's own start and
    end (two seconds' margin), one per path: before, a later delete of the same path in another
    program was restored as well.
  - Tried live (2026-11-17, `private/probes/undo-safety/live.ps1`: a scratch copy of folders.exe with
    its own settings.txt, so its undo.txt was not the owner's; only "Bevel95 test" items, by test
    messages and posted keys): a copy of a 3000-file folder during which a second file was written
    into the destination (at 67 of 3000), then Undo Copy: one item asked about, the copy gone, the
    second file kept; a rename, then a folder moved onto one of the same name (Windows 11 merged
    without asking): the list empty, Edit's Undo grayed (photographed), Undo changed nothing, the
    rename stayed; a move, a copy, two copies into their own folder ("- Copy", "- Copy (2)") of which
    the second was changed (the message, photographed, and the copy kept; the first went), a rename
    and a delete, each taken back once. Not tried live: the desktop (its items are the owner's
    Desktop folder), a file that replaced another (Windows' question), the lock between the two
    programs at once (tested by threads).
- **Three leftovers repaired before going public** (2026-11-18, the main session's decisions; the
  leftovers todo.md listed after the repair above):
  - **Nothing Bevel95 creates is written over a file it did not just make itself.** Create
    Shortcut, Paste Shortcut and a drag that makes a link chose a free "Shortcut to ..." name and
    then saved the link there (IPersistFile::Save writes over whatever is there), so a file another
    program put under that name in between was overwritten. Now the name is claimed as the file is
    made (`file_ops::new_shortcut`): the link is written through a stream created with
    SHCreateStreamOnFileEx and STGM_FAILIFTHERE (CREATE_NEW underneath; IPersistStream::Save), a
    shortcut's shortcut through a file opened with `create_new`; a name found taken is left alone
    and the next name is tried (up to 100). Setup's two desktop shortcuts are saved the same way
    (`file_ops::save_shell_link_new`): a file already there is never written over (the shortcut is
    then not made). A copy into its own folder (every item a "- Copy") also passes
    FOF_RENAMEONCOLLISION, so a planned "- Copy" name taken meanwhile gets Windows' next name instead
    of Windows' question to replace it (that copy is then not recorded); mixed with items from
    elsewhere, those keep Windows' question, as in Windows. Checked and already safe: New Folder
    and New Text Document (desktop and folder windows: `create_dir` and `create_new` fail on a
    taken name, nothing replaced, the log says so), New > Shortcut's placeholder for Windows'
    wizard (`create_new`, then the next name), Rename, Paste and drops (Windows' file machinery
    without FOF_NOCONFIRMATION: Windows asks before anything is replaced), Undo Move and Undo
    Rename (Windows' machinery; the old place checked free first). Everything else Bevel95 writes
    lives in its data folder. Tests: `a_shortcut_never_writes_over_a_file_that_took_its_name`
    (failed before: the other file was overwritten), `a_shortcut_is_never_written_over_a_file`.
  - **Undo never makes the window wait.** Undo took its step off the list (the shared lock: up to
    3 s) and read Undo Copy's proofs (a copied folder read through) on the window's thread. Now
    `undo::undo_off_thread` does both on a thread of its own and hands back what Undo does;
    `confirm::undo` posts it to a message-only window of the window's own thread (class
    `Bevel95UndoBack`), where the look's question or message opens once it is known. An Undo while
    the last is still being checked does nothing (it would reach the step before). The rule stays:
    only copies still exactly as made go to the Recycle Bin, checked again after Yes. Test:
    `undo_copy_reads_its_proof_off_the_callers_thread` (failed the old way).
  - **An operation that provably did nothing leaves the Undo list alone** (`core::drag::recorded`,
    `UndoList::leave`, pure and tested): without a step, the list is left as it is only when every
    planned place was free before and is still free, and every source is still where it was
    (nothing made, nothing gone). In every other case it is emptied as before, and in doubt too: a
    place counts as free only when Windows says nothing is there, a source as there only when it
    can be read. So a copy of a folder into itself (Windows refuses it), a drop stopped before
    anything happened, keep the older steps; a cancelled "replace?" question of Windows' still
    empties the list (its place was taken before). Test:
    `an_operation_that_provably_did_nothing_leaves_the_list_alone` (failed with the old rule).
  - Tried live (2026-11-18, `private/probes/undo-safety/live2.ps1`: a scratch copy of folders.exe in
    `live2\` with its own settings.txt and undo.txt; only "Bevel95 test" items, by test messages
    and posted keys to the scratch copy's own windows): Create Shortcut twice on a test file ("...
    .lnk" and "... (2).lnk", both pointing at it); a copy of a folder of 20,040 files, then Undo
    Copy: its question came after 733 ms, while the window answered 17 times meanwhile, the longest
    wait 7 ms (photographed), Yes sent the copy to the Recycle Bin; a rename, then a copy of a folder
    into itself (Windows' "Interrupted Action" window, closed as its Cancel): the log said the
    operation provably did nothing, the rename stayed on the list and Undo took it back. Not tried
    live: a file appearing under a shortcut's name in the instant between (a race: only the unit
    test makes it), the desktop's side (its items are the owner's Desktop folder), Setup's
    shortcuts (no real Install).
- **Saving the list, failures read off the disk, one Undo at a time** (an outside review of
  2026-11-18 found all three; the side chosen wherever a case was open: nothing of the person's
  is changed and Undo is unavailable):
  - **A list that cannot be saved never leaves an older step in reach** (`undo::change_with`,
    `record`, `empty`, `can_change`, `last`, `take_last`). `change` logged a failed write and went
    on as if saved. Now a record after an operation that cannot be saved (the write fails, the
    lock cannot be had, or undo.txt cannot be read) removes undo.txt while the lock is still held
    (the list is empty). If that fails too, Undo is unavailable in both programs for as long as
    the list on the disk cannot be changed, and that is asked of the disk each time, never noted
    in a program's memory: `last` (which the menus use to show or gray Undo) gives nothing while
    undo.txt is read-only or cannot be opened for writing (nothing is written by asking), and
    taking a step back first takes it off the list and **saves** the list; when that cannot be
    saved, the step is not carried out, nothing changes, and the look's "cannot be done" message
    for that step comes (the list keeps it). Once the file can be changed again, the list is
    emptied before anything else (the owed emptying, below; until 2026-11-18 the older step was
    reachable again then). **Changed**: an unreadable
    undo.txt was kept as it is at a record; now a record removes it (the step could not be saved
    into it); Undo still sees an unreadable list as empty and never writes over it. Tests:
    `undo::tests::a_list_that_cannot_be_saved_never_leaves_an_older_step_in_reach` (a failing
    writer: emptied; Undo with a failing writer: nothing taken, the list as it was; read-only:
    Undo grayed and nothing taken; undo.txt held open by another program, neither written nor
    removed: unavailable, then available again), `an_unreadable_list_is_kept_as_it_is` (now also:
    a record removes it).
  - **What an operation did is read off the disk however it returned**. `delete_now` recorded
    only after success, so an error after some items had gone to the Recycle Bin left the older
    step in reach. Each operation now, after an error, Windows' question answered Cancel, or
    success alike:
    - Delete and a drop on the Recycle Bin (`delete_recorded`): the items gone (Windows says
      nothing is there) are a step; none gone and each still there is nothing (the list stays);
      else the list is emptied.
    - Rename (`rename_recorded`): renamed (the new name free before and there now, the old one
      gone; for a change of only the case of letters, the name on the disk) is a step; the old
      name still there and the new one as it was is nothing; else emptied.
    - Move, copy, Paste and the drops (`move_recorded`, `copy_into`): already read off the disk
      after any outcome (each item's planned place, `core::drag::recorded`); the operation is now
      given to them (`move_recorded_with`, `copy_into_with`) so a test can make it fail.
    - Undo's own operations (`undo::after_undo`, `taken_back`): Undo Move, Undo Rename and Undo
      Delete wholly taken back leave the list as it is (its older steps are right again; Undo
      Delete is looked at for up to 3 s, as the Recycle Bin's Restore may finish after its call);
      anything else (an error, Windows' question, a part done, nothing done) empties the list:
      the step has left it and is partly or wholly still in effect, and putting it back could
      land it after a newer step. Undo Copy: every copy of the step gone afterwards leaves the
      list; a copy left (changed since, an error) empties it. **No** (or its question closed
      any other way without Yes) put the step back since 2026-11-18 (below; it emptied the list
      before). A step that cannot be done at all (`possible`) leaves the list as before.
    Tests: `file_ops::tests::an_operation_that_fails_part_way_records_what_it_did` (each operation
    fails with an error after doing part of its work: delete one of two, a rename, a rename of the
    case only, a move and a copy of one of two; and failing before anything: the list stays),
    `undo::tests::undo_reads_off_the_disk_what_its_operation_did`.
  - **One Undo at a time** (`undo::UndoMark`, `running`). `undo_at` took a step off the list and
    started its work on another thread; a second Undo could take the next step meanwhile, find
    its items not yet back and lose it. Now an Undo holds a mark from taking its step until its
    file operation has returned, Undo Copy's question included: a named mutex of the copy
    (`Local\Bevel95 undo running.<copy>`, used only by its existence: made by the Undo, closed
    when it ends, so it is shared by the desktop's and the folder windows' programs and goes
    with its process at the latest). Another Undo meanwhile, in either program, does nothing and
    takes nothing off the list (logged "another Undo is being carried out"), and `last` gives
    nothing, so Undo shows grayed. A mark that cannot be made means no Undo. Test:
    `undo::tests::a_second_undo_meanwhile_takes_nothing` (Undo Copy's question open: a second
    Undo on the same thread and one on another take nothing, the list keeps its older step, Undo
    shows unavailable; No: the list as before the Undo, the mark gone, the next Undo can run).
  - Tried live (`private/probes/codex2/undo/live.ps1`, a scratch copy of folders.exe with its own
    settings.txt and undo.txt, 98 look, only "Bevel95 test" items, by test messages and keys
    posted to its own windows): a rename and a copy (2 steps); Undo: Undo Copy's question
    (photographed), the list 1 step; a second Undo while it was open: still 1 step, the log
    "another Undo is being carried out"; Yes: the copy to the Recycle Bin, the rename's step kept;
    undo.txt held open by the script (share read only): Undo gave 98's "Error Renaming File"
    message (photographed), the file and the list unchanged; let go: Undo renamed it back. Not
    tried live: a real partial failure of Windows' file machinery (the tests make one), the
    desktop's side (the owner's Desktop folder), a read-only undo.txt (the test).
- **No to Undo Copy, and an emptying owed** (2026-11-18, the main session's brief; the side
  chosen wherever a case was open: nothing of the person's is changed and Undo is unavailable):
  - **No puts the step back** (`undo::Asking`, `put_back`). Undo takes its step off the list
    before the question, and No left the list emptied. Nothing happened, so now on No (and when
    the question is closed any other way without Yes: Esc, its X, a new question taking its
    place, the posted answer lost) the step goes back where it was and the list is exactly as
    before Ctrl+Z, but only when the list is still exactly as the Undo left it: the same steps
    and the same save (each save of undo.txt now ends with a line naming it, `saved\t<process>.
    <time>.<count>`, which the list's reader skips as every unknown line; the file's times were
    tried first and are the same for two saves within the clock's step of about 15 ms), and no
    emptying owed. An operation recorded meanwhile, in either program, also one that emptied an
    already empty list (such a record now saves the empty list, so the next check sees it), or a
    list that cannot be saved with the step: the list is emptied, as before. No question at all
    (every copy changed since it was made: only the message): emptied, as before. "One Undo at a
    time" holds through the question and the putting back (the mark goes after it). Test:
    `undo::tests::no_to_undo_copy_puts_its_step_back` (fails the old way).
  - **A list that could not be emptied is emptied as soon as it can be** (`undo::owe`, `owed`,
    `paid`, `settle`). A record that could neither save nor remove undo.txt left Undo grayed
    while the disk said the file could not be changed, and once it could, an older step was back
    in reach. Now the list is owed its emptying, and whoever does anything with the list next
    pays it first, inside the lock: before showing whether Undo is there (`last`: only then it
    waits for the lock, at most 200 ms, on the window's thread; not had, Undo shows grayed),
    before an Undo (`take_last`: while it cannot be paid nothing is taken), before the next
    record (`record_with`: while it cannot be paid the record empties the list itself before its
    step), before putting Undo Copy's step back. The debt is a named event of the session per
    list (`Local\Bevel95 undo owed <hash of the list's path>`, unsignaled while owed, set once
    paid), held open by the program that owes it for its run, so the other program (the
    desktop's or the folder windows') sees and pays it too; an event that cannot be made is kept
    in the program's memory only. Paid means undo.txt is removed (or a record saved a list with
    nothing older). Test: `undo::tests::a_list_that_could_not_be_emptied_is_emptied_as_soon_as_it_can_be`
    (a failing writer and a file held open so it cannot be removed, then let go: paid before
    `last`, before an Undo, before the next record; the event seen; paid once, a later list
    kept). `a_list_that_cannot_be_saved_never_leaves_an_older_step_in_reach` now ends with the
    list emptied instead of its older step back.
  - Tried live (`private/probes/quiet-tests/live.ps1`, a scratch copy of the installed build's
    folders.exe with its own settings.txt and undo.txt, 98 look, only "Bevel95 test" items, by
    test messages and keys posted to its own windows): a rename and a copy (2 steps); Undo: the
    question, the list 1 step meanwhile; Esc (No): the copy stays, the list 2 steps again; Undo
    again and Yes: the copy to the Recycle Bin (taken out again by the script), 1 step left; a
    rename while undo.txt was held open by the script: neither saved nor removed (the log says
    so); let go, Undo: the log "the emptying the list was owed is paid", undo.txt gone, the first
    rename not taken back. Not tried live: the desktop's side (the owner's Desktop folder), the
    debt paid by the other program (the test reads the event), a write failing for another
    reason than a file held open.
- **The desktop drags files** (on for good since 2026-11-16, after the live test below; until then
  it was built but switched off by a flag file, `programs\drag-test.on`, because the owner's windows
  covered the desktop at every idle moment). The switch is gone from the code and the docs.
- **Tried live on the desktop** (2026-11-16, `private/probes/desktop-drag/live.ps1`; the owner allowed
  pressing Show Desktop at idle moments, in pairs). The first pair (Show Desktop, nothing, Show
  Desktop) found the owner's windows already all minimized and the desktop bare; it changed nothing
  (the same windows, none restored or minimized). Every later run therefore pressed no Show Desktop
  ("bare" mode: each press and drop still asked the desktop what lay there and that no window covered
  it, and the windows were compared before and after: the same each time). Only "Bevel95 test drag
  ..." items on the owner's Desktop folder, each result checked on disk; the owner's desktop items
  compared by name, size and time before and after every run: unchanged. 98 at size 2: an icon
  moved on the desktop (nothing on disk, its place kept, exactly where dropped); a file onto a test
  folder (moved) and Ctrl+Z (back); Ctrl onto the folder (a copy) and Ctrl+Z (Undo Copy's question,
  Yes: the copy to the Recycle Bin); Ctrl+Shift ("Shortcut to Bevel95 test drag C.txt.lnk" in the
  folder); the right button's menu on the folder (photographed: Move Here bold, Copy Here, Create
  Shortcut(s) Here, Cancel; the dragged picture stays shown under the menu, as on 98's and 2000's
  PCs) and Copy Here; Esc during a drag (nothing); two icons selected and dropped on the folder (both
  moved) and Ctrl+Z (both back); a drop on the Recycle Bin (98's question, photographed, Yes) and
  Ctrl+Z (back out of the Recycle Bin); a file from the desktop into a scratch folder window and back
  out onto an empty spot (it lay where it was dropped); a file from a window of Windows' own File
  Explorer of ours (closed again) onto the desktop (moved, where dropped); the desktop's own menu by
  a right-click afterwards. 2000: the right button's menu (Copy Here first) and Esc, the Recycle Bin
  without a question, Ctrl+Z. 95: a file onto the folder, the Recycle Bin without a question, two
  Ctrl+Z. The looks were changed by the Theme menu's message and put back; settings.txt byte for byte
  as before. On the installed bar after switching on: one move onto the folder and its Ctrl+Z.
  Afterwards the test items, our records in the Recycle Bin, `desktop.txt` (the first move had
  frozen every icon's place; it did not exist before) and `undo.txt` were removed, so the owner's
  icons stand in columns again.
  - Found and fixed: a file back from the Recycle Bin in the 95 look took My Documents' waiting cell
    (95 hides it) and lay under My Documents in the 98 look; a place kept for an icon the look does
    not show now counts as taken (`relayout`, test `a_new_item_in_95_leaves_my_documents_place_free`).
  - Seen, Windows' own: Restore out of the Recycle Bin (Bevel95's Undo Delete and Windows' own
    "undelete" verb alike, tried with Shell.Application) leaves the item's `$I` record behind without
    its `$R` file. Nothing changed.
- **Tried with the real mouse** (2026-11-16, a scratch copy of folders.exe in
  `private/probes/drag18live`, `m.ps1` with drags whose two ends are checked to be the scratch copy's
  windows after the idle gate; only "Bevel95 test ..." items, each result checked on disk): 98 at size
  2: a move between two windows and Undo by Ctrl+Z; Ctrl onto a folder's icon (copy, the folder
  selected, the half picture); a move in the same window (place kept in folders.txt); the right
  button's menu (photographed) and Copy Here; Undo Copy's question and Yes; Delete and Undo Delete
  (back from the Recycle Bin); Esc during a drag; a drag onto a tree row that opened under the
  pointer, and Undo; Bevel95 to Windows' File Explorer and back (a test Explorer window of ours,
  closed again). 2000 at size 1: the right button's menu (Copy Here first) and Esc, a move, Undo. 95
  at size 2: a move with the checkerboard picture, Undo. On 2026-11-16 also, 98 at size 2 in Windows
  Explorer: two items selected and dropped on a folder (both moved) and Ctrl+Z; Shift onto a folder in
  the other window (moved); Ctrl+Shift onto a folder (a shortcut); a drop on the tree's Recycle Bin
  row (98's question, Yes) and Ctrl+Z. Not tried live: other programs than File Explorer, a second
  drive, real Shift and Ctrl+Shift on the desktop's other looks.
- **Resources**: 81 real drags in one window left the folders program at USER 37 → 36, GDI 92 → 92; 30
  windows opened and closed afterwards: USER 20 → 19, GDI 47 → 47 (139/137 with 31 open). (The owner
  found repeated real drags excessive: later leak checks use test messages, at most 20.)
- **The toolbar's Undo button** (2026-11-16): 98 between two separators after Paste, 2000 right after
  Delete, as on the PCs (`explorer::toolbar`, `Tool::Undo`); it does what Edit > Undo does (with
  nothing to take back, nothing). Never grayed: every capture shows it plain, also in fresh windows
  with nothing to undo. Pictures (`explorer_art`, cut by `private/probes/desktop-drag/cutundo.ps1`): 98's
  plain and hot from `win98-truecolor-folder-mycomputer` and `-hot-undo`; 2000's plain from
  `win2000-truecolor-folder-mycomputer`, its hot one from 2000's own hot picture strip (browseui.dll
  bitmap 279), as no True Color capture shows it (the same way gives hot Delete exactly as its
  capture). Pressed: the hot picture one pixel down and right in the sunken edge, as every button.
  Putting Undo in its place moved 98's Delete and Properties to where the PC had them, so the
  comparisons no longer mask them; that showed 98 cutting its toolbar 4 pixels before the band's end
  (now so). Compared: `vm-win98-folder-x-hot-undo` (new), Undo plain in `vm-win98-folder-x-mycomputer`
  and `-c` (no longer masked), `vm-win2000-folder-x-undo` (new, the button alone: 2000's buttons
  before it are not all built): 0 differing pixels. Golden pictures of the 98 and 2000 folder windows
  drawn again (the toolbar only). Tried with the real mouse in a scratch copy, 98 and 2000 at size 2:
  plain, hot, pressed photographed; a file moved onto a folder and the Undo button brought it back
  (on disk).
- **Not built (todo)**: dropping on a program, dragging drives and the look's own icons out of their window, the drag picture over
  the taskbar, Windows' tooltip-like "Move/Copy" text of today's Explorer (never in 95 to 2000).
## File jobs outlive their window (2026-11-18, the safety review)
- **Why**: a copy or move runs on a thread nobody waited for, so folders.exe, which ends with
  its last window, and the bar on Exit, Restart, `--restore`, Setup's Update and Remove and
  `tools\install.ps1` cut a running copy short: a half-written file, a folder half in each
  place, no Undo step (found by the safety review of 2026-11-18, A2; reproduced live before the
  repair: 191 of 4000 files copied, the process gone 110 ms after its last window closed).
- **Counted** (`winkit::jobs`): every job started through `file_ops::on_thread` (drops, Paste,
  Rename, Delete, Empty Recycle Bin, shortcuts, Undo and its checks) counts from the moment it is
  asked for until its thread is done, also when it panics (`jobs::Running`, a counter dropped
  with the thread's work). Windows' progress window of a copy and its questions belong to such a
  thread (they show while its copy runs) and are covered by it. **Nothing else counts** (changed
  by the second safety review of 2026-11-18, B1): Windows' Properties windows and its other
  dialogs counted too (`jobs::windows_open`, now gone), so a Properties window left open,
  minimized or behind other windows, kept the bar's process, and with it the bar's mutex, for
  good: the desktop shortcut's bar met the mutex and exited without a word, a Restart without a
  watchdog was lost, and Setup and install.ps1 said a copy was being finished when none was. A
  Properties window now closes with its program, as before the first repair of 2026-11-18.
- **folders.exe**: when its last window closes while anything counts, it stays without a window
  (its hidden window still takes the hand-over, so a folder window asked for meanwhile opens as
  usual and the program goes on), then ends as before. Windows' progress window stays and can be
  cancelled there. Its messages are handled meanwhile (`jobs::wait_while_busy`).
- **The bar**: asked to end, it ends as always (its windows go, Windows' taskbar comes back at
  once, in `on_destroy`), and **at the same moment it gives up its place** (the second safety
  review of 2026-11-18, B1; `bar::give_up_place`, the last thing `on_destroy` does before the
  window goes): the single-instance mutex is closed, and the hotkeys, the keyboard hook, the shell
  hook, the notification-area stand-in, Bevel95's own notification icon and the classic desktop
  are already gone. Then, in `main`, if a file job runs, the process stays without a window
  until it ends (`finish_file_jobs`, its messages handled; it puts up the mark
  `Local\Bevel95.FinishingFileJobs.<pid>`, `jobs::mark_finishing`), and only then exits. Such an
  old process holds nothing that makes it "the running Bevel95"; a new bar can start beside it
  at once. What the two share is already safe to share: the Undo list (its named lock,
  `undo::lock`, is the same for the bar and folders.exe; the old process records the job's step
  in it when the job ends), the state files (each written whole under a temporary name with
  the process number in it, so two writers never meet in one temporary file), task-order.txt
  (saved by the old bar before it gave up its place) and the log (a line at a time). The old
  process never touches Windows' taskbar again: nothing hides it after `on_destroy`, and its
  panic hook does nothing once the place is given up (`bar::gave_up_place`), so a panic in the
  finishing job cannot undo a new bar's record. What each one does now, decided so that both
  promises hold (Windows' taskbar always comes back; a running job is never cut short):
  - **The desktop shortcut** starts a new bar at once (the mutex is free).
  - **Restart** with the watchdog: the bar puts up its restart mark
    (`Local\Bevel95.Restart.<pid>`, `bar::mark_restart`) before its window goes. **The
    watchdog** now waits for the process to end *or* for the bar's window to go
    (`watchdog::look`, every 100 ms): a window gone while the process runs is a bar that ended the
    ordinary way and gave up its place; the watchdog restores (again), starts the new bar at once
    if the restart mark is up, and ends. A process that ends (killed, crashed) is handled as
    before. So exactly one watchdog watches exactly one bar; the old process finishing a job has
    none (it has nothing left to give back). Before, the watchdog waited for the process: the new
    bar came when the job was done (15.9 s), and with a Properties window left open never.
  - **Restart without a watchdog** (`--after <pid>`) waits until the old process has ended *or*
    the bar's mutex is free (`watchdog::wait_for_place`, at most 10 s), then starts. Before, it
    waited 10 s for the process, then met the mutex and exited: the restart was lost. A portable
    copy's `--after`, watchdog and the watchdog's new bar carry the flag `--portable-copy` (the
    third safety review of 2026-11-18, A1: "Bevel95 Setup", portable copies).
  - **`--restore`** waits, as before, only for the bar's window (at most 3 s, then ends a hung
    bar by force); the window goes at once, so a bar finishing a job is never ended by force. It
    logs that the bar is finishing a copy only when the old process holds that mark, restores
    Windows' taskbar (again) and reports success.
  - **Setup's Update** waits up to 10 s after closing the bar for every copy of the installed
    bevel95.exe to end (`wait_for_copies`, `FINISH_WAIT`); if one still runs, nothing is
    changed (the staged files go) and the bar that was running is started again (its old version:
    the place is free). Setup then says what is true (`still_running_message`): when one of
    them holds the mark, "... is still finishing a copy or move of your files ... so its program
    cannot be replaced yet. Nothing was changed. Run this Setup again once the copy or move has
    finished."; otherwise "Another copy of Bevel95's program is still running and did not end, so
    it cannot be replaced. Nothing was changed. Restart the computer, or end it in Task Manager,
    and run this Setup again." (both PROVISIONAL). **Setup's Remove** waits its 5 s as before and
    says the same two ways ("... Nothing was removed. Run Remove again ..."). This replaces the
    rule of 2026-11-17 that removed anyway after 5 s and kept the log.
  - **`tools\install.ps1`** waits 10 s for the old bar as before; if its process still runs and
    holds the mark, it says every 15 s that it is finishing a copy and keeps waiting, at most 30
    minutes, then stops without replacing bevel95.exe; without the mark it stops at once and
    says the old Bevel95 did not end and is not finishing a copy. Either way its last step starts
    the installed bar again (the old version, if it was not replaced).
  - **A job that never ends** (a question of Windows' left unanswered): nothing of Bevel95 is
    blocked by it but replacing or removing the program files, which Setup and install.ps1
    refuse with the words above.
- **When Windows ends the session** (WM_ENDSESSION), Windows ends the process at once whatever
  runs: nothing can be done then (Windows' own copy is cut short the same way).
- **However a job ends, Undo stays truthful**: a drop, Paste, Rename or Delete whose thread
  ends before its outcome was recorded (it panicked) empties the Undo list (`file_ops::UndoGuard`),
  so the list never points at the step before it; a job that ends normally records by the
  existing rules.
- For live tests the classic desktop takes a copy "from another program" by a test message
  (`TEST_DROP` in WM_COPYDATA to the desktop window: test items into a "Bevel95 test" folder
  only, carried out by `file_ops::drop_files` on a thread of the bar's program).
- Tests: `a_file_job_counts_until_it_is_done` (file_ops), `jobs_are_counted_until_their_threads_are_done`.
  Tried live (2026-11-18, `private/probes/drops/drop.ps1 -Phase cut`, a scratch copy of folders.exe
  with its own data folder, only "Bevel95 test" items: a 4000-file folder of 250 MB dropped into
  a test window, the last window closed by message after 0.8 s): before the repair cut short as
  above; after it whole (4000 of 4000 files, every byte), the process gone 17.4 s after the
  window closed; again with a folder window asked for while waiting (it opened in the same
  process; whole, ended 15.7 s after). On the owner's bar once, at idle (`bar.ps1`): the same
  copy started in the bar's program, then Restart by message: the bar's window gone after 153
  ms, Windows' taskbar showing after 523 ms, the copy whole, the old process ended after 15.8 s
  and the new bar started by the watchdog 65 ms later (the installed file; the log's start
  clean; the owner's undo.txt as before). Setup's Remove in its test mode (`setup.ps1`: a
  windowless stand-in running the scratch installed bevel95.exe): its message after 5.1 s,
  nothing removed, the stand-in not ended. Leak check: 20 copies by the test message in one
  scratch window: handles 417 → 418, threads 11 → 11, USER 22 → 22 (GDI 59 → 82, the same with
  the build before this repair: the new icons' pictures). Not tried live: Exit and `--restore`
  during a copy (the same path as Restart), Setup's Update (its pages), a Properties window
  holding the program, WM_ENDSESSION.
- **The second review's repair (B1), tests and tries**: `tests/jobs_rule.rs` (a visible
  `#32770` and `OperationStatusWindow` of the process do not make it busy; failed before),
  `jobs::tests::the_finishing_mark_is_seen_by_process`, `bar::tests::a_place_given_up_is_free_for_the_next_bar`,
  `watchdog::tests::a_bar_that_gave_up_its_place_is_restarted_at_once` and
  `after_starts_once_the_place_is_free` (the place freed after 400 ms while the process runs: the
  wait ended at once; before, `--after` waited 10 s for the process),
  `install::tests::remove_waits_for_other_copies` (a copy without the mark: "did not end", never
  "finishing a copy"; with the mark: "finishing"). For live tests the classic desktop opens
  Windows' Properties window of a test file by a test message (`TEST_PROPERTIES` in WM_COPYDATA:
  only a "Bevel95 test" file in a "Bevel95 test" folder). Tried live on the owner's bar, at idle,
  by messages (`private/probes/drops/bar2.ps1`): (1) Properties of a test file open in the bar's
  program (one `#32770`), then Restart: the old bar's window gone after 161 ms and its process
  ended at the same moment (the Properties window with it), the new bar showing after 863 ms;
  (2) a copy of 4000 test files (250 MB) running in the bar's program, then Restart: the old
  window gone after 163 ms, the new bar showing after 918 ms (361 files copied by then), the old
  process ended after 16.4 s, the copy whole (4000 of 4000 files, every size equal); the log has
  the watchdog's "the bar's window is gone and its place is free (a Restart)". After each:
  one bar and one watchdog; the new bar the installed file, started by the old watchdog (which
  then ended). Not tried live: Exit, `--restore`, the desktop shortcut and `--after` while a
  job runs (the same give-up path; `--after` by its test), Setup's Update and Remove against a
  finishing bar (their words by the test), install.ps1's mark check (its "finishing" branch).

## Settings and state files are written whole and never replaced unread (2026-11-18, the safety review)
- **Why** (found by the safety review of 2026-11-18, B1 and suspected 4): a settings.txt the bar
  could not read (saved by Notepad as UTF-16, an ANSI file with an accented letter, a moment's
  sharing violation from a backup program) was overwritten with the defaults at the next start,
  and every later save read with "the defaults if unreadable" and wrote that back: one bad read
  and the file was the defaults plus one setting. Every state file was written in place
  (`fs::write` truncates first), so a power cut in that moment left an empty or partial file;
  desktop.txt, written at every icon move, most of all. A settings.txt saved "UTF-8 with BOM"
  lost its first setting (the mark stuck to `era`; proven by
  `config::tests::a_byte_order_mark_is_not_part_of_the_first_setting`, which failed before).
- **One shared way** (`winkit::state_file`):
  - `write_whole`: the text under a temporary name beside the file
    (`<file>.<pid>.<n>.tmp`), flushed (FlushFileBuffers), then put in place in one step
    (MoveFileExW with MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH; windows-sys feature
    `Win32_Storage_FileSystem`, already on). A reader with the file open makes Windows refuse
    the replacement for a moment: tried again every 20 ms, at most 25 times, so a save on a
    window's thread waits at most about half a second (it was 50 times, a second). On a failure
    the temporary file goes and the file is as it was.
  - **Read-only files, links, leftovers** (found by the second safety review of 2026-11-18, C1):
    - A **read-only** destination is refused at once, before a temporary file is made
      (`state_file::is_read_only` tells its error; its line "... is read-only: Bevel95 keeps it as
      it is and does not save it" is logged once per file and run, and the frequent callers leave
      it out of their own "could not save" line). Before, Windows' refusal looked like "open just
      now" and was tried again for a second, so the classic desktop stood still for a second at
      every icon moved. A folder where the file should be fails at once too.
    - A **symbolic link** is written through to the file it points at (the link stays; the
      temporary file goes beside that file). A file with **more than one hard link** is written in
      place (truncated, written, flushed), as before the safe write existed: a rename would part it
      from its other names. That case keeps the old risk of a part-written file at a power cut.
    - **Leftovers** of the safe write (a minute old) are cleared when a program starts
      (`log::started`, in the data folder) and by Setup's Remove, so Remove can leave nothing
      behind (`state_file::clear_leftovers`). **Only the temporary names of Bevel95's own state
      files** go (`state_file::OWN_FILES`, the thirteen files that go through the safe write:
      `<own file>.<pid>.<n>.tmp`; the third safety review of 2026-11-18, C3: any
      `<anything>.txt.<n>.<n>.tmp` went, which in a portable copy's folder, where the person may keep
      files, could be one of theirs, deleted without the Recycle Bin). A test checks that each of
      them is in Remove's list. Test: `leftovers_of_a_save_are_cleared` (now also three of a
      person's names of that shape stay; they went before).
    - Tests: `a_read_only_file_is_refused_at_once` (three saves, each under 100 ms, logged once;
      it took about a second before), `links_stay_links` (a hard link sees the new text; a symbolic
      link stays a link when Windows lets the test make one, which needs Developer Mode or an
      administrator), `leftovers_of_a_save_are_cleared` (two old own names go; a young one, a
      foreign `.tmp`, Setup's staging name stay). The test of readers now lets its reader pause a
      millisecond between reads, as a real one does.
  - `read`: missing is `Missing` (defaults, may be created, as before); there but not usable is
    `Unreadable` (any open or read error, UTF-16 with or without its mark, NUL bytes, not UTF-8):
    kept as it is, never converted. A UTF-8 byte-order mark is skipped (and `config::parse` skips
    one too).
  - `load` (a program that keeps its own picture of the file and saves it whole): an unreadable
    file is logged once; the program runs on its defaults for it, and every `save` of it is
    skipped (logged by the caller's "could not save ..." line) while the file is there, even once
    it reads again, since the program's picture is the defaults and not the file. A later
    successful `load` lifts that.
  - `change` (read again for every change: run-history.txt): skipped while the file cannot be
    read. settings.txt goes through `change_settings`, which also never writes a file without
    Bevel95's mark outside the installed folder, and in it puts the mark back (the third and
    fourth safety reviews of 2026-11-18, A1 and C1: "Bevel95 Setup"). A run whose data folder is
    not known writes nothing (`write_whole` refuses; "Bevel95 Setup", where a copy keeps its data),
    except the note of a used record that its `--restore` leaves beside the program
    (`write_whole_even_if_not_known`, only for that file: the sixth safety review of 2026-11-18,
    B1, "Milestone 1 decisions").
- **Every file through it**: settings.txt (the bar's load, which creates it when missing; the
  clock's offset and zone; Taskbar Properties' options; every other setting through
  `save_setting`; Setup's Install answers and Portable's new file), programs.txt (its read, and
  its save, which reads it first and is skipped while it cannot be read) and programs.old.txt
  (written whole first; if that fails the new list is not saved, so the earlier list is never
  lost), desktop.txt, folders.txt, task-order.txt, run-history.txt, undo.txt (its own
  temporary-name write now the shared one; an unreadable list is kept and Undo sees an empty
  one; since an outside review of 2026-11-18 a record that cannot save into it removes it:
  "Dragging files", saving the list), calculator.txt, minefield.txt, solitaire.txt (it had its own safe write, now the shared
  one, with its lock as before) and windows-taskbar-state.txt ("Milestone 1 decisions"). The
  reads changed with them: the bar's settings, the look every program reads
  (`winkit::look`), Setup's look of an installed copy, programs.txt, desktop.txt, folders.txt,
  task-order.txt, run-history.txt, undo.txt and the three programs' files.
- Not changed: the log (appended a line at a time), Setup's program files (their own staging
  and read-back, "Bevel95 Setup"), the font fixtures and golden pictures (tests).
- A temporary file left by a power cut in the middle of a save is never read; since the second
  review it is cleared at the next start and by Remove (above).
- Tests: `state_file::tests` (a reader never sees a part of a file while 30 writes of 256 KB
  replace it: it saw parts in 3 of 60 reads with `fs::write`; a failed write; what reading
  finds; an unreadable file kept byte for byte after a load, a save and a change),
  `bar::tests::an_unreadable_settings_file_is_never_replaced` (a UTF-16 settings.txt kept after
  the load and a save; it was replaced by the defaults before), `undo::tests::an_unreadable_list_is_kept_as_it_is`.

## One "Bevel95" entry and Quick Search (2026-11-16, the owner: "consolidate settings a bit")
- What the owner asked for: the bar's menu had Theme, Size and Bevel95 Settings beside Windows'
  own entries; "That could just be Bevel95 and an arrow leading to Switch to Windows Taskbar, Theme,
  and Size (and perhaps also the Quick Search we made)"; theme switching, sizes and the Bevel95
  Settings link in the search too; "Eventually I think the search we have should become a kind of
  'do anything' modern tool for bevel95 ... we could call it Quick Search"; "The desktop menu could
  also consolidate a little."
- **The bar's menu** (`bar_menu`) reads as Windows' own plus one entry, the same in every look:
  **Bevel95** (a submenu arrow) right above **Properties**, where Bevel95 Settings, Theme and Size
  stood (right-clicked on the clock: Adjust Date/Time and a line first, as before). The submenu
  (`own_menu`, `BarEntry::Bevel95`, `popup::Kind::BarOwn`) holds everything of ours the menu held,
  nothing lost: Bevel95 Settings, Theme >, Size >, Quick Search, a line, Switch back to Windows
  (Ctrl+Win+T), a line, Restart, Exit. Theme and Size are now submenus of a submenu (the menu stack
  had no depth limit). Windows 95, 98 and 2000 had more entries (Cascade, Tile, Minimize All
  Windows); they are still left out, as before, because Bevel95 does not do them.
- **The desktop's menu** (`desktop`): the look's own entries as they were (Undo where that Windows had
  it, after Paste Shortcut), then a line and **Bevel95** (`Entry::Bevel95`, `own_menu`,
  `popup::Kind::DesktopOwn`): Bevel95 Settings (its Desktop page, as before), Quick Search
  (`DesktopCommand::QuickSearch`, code 25 at the end), a line, Switch to Windows desktop. Decided
  differently from the brief's order (Switch, Settings, Quick Search): the bar's submenu's order, so
  the two read alike; the owner's to change.
- **Find: Programs is now Quick Search**, one name in every look (Bevel95's own tool, not something a
  Windows had; "Search: Programs" in the 2000 look until now): the window's title, the last entry of
  Start > Find (2000: Search), "&Quick Search..." (was "&Programs..."), the Windows key choice on
  Bevel95 Settings' Taskbar page, "Opens &Quick Search" (was "Opens &Find", 2000 "Opens Sea&rch"),
  the log ("opened Quick Search: N programs"). In settings.txt `windows_key = find` keeps its word:
  `find` now means Quick Search (an old file must not break); `BarCommand::Find` and `core::find`
  keep their names in the code, and the golden pictures keep theirs (`*-find-programs`).
- **Quick Search finds things to do** (`core::quick`, a table made to grow: name, other words, a
  small picture, `Action`): Bevel95 Settings, Taskbar Properties, Date/Time Properties, Windows 95 /
  98 / 2000 look (found by "theme", "look"), Size 1x to 4x (the Size menu's, found by "size",
  "scale"), Switch back to Windows, Show the classic desktop, Switch to Windows desktop, Run...,
  Shut Down..., Windows Explorer, My Computer, Restart Bevel95, Exit Bevel95. Each is found by
  "Bevel95" too. New bar commands carry the new ones (`BarCommand::Run`, `ShutDown`, `Explorer`,
  `MyComputer`, `DesktopOn`, `DesktopOff`, codes 7 to 12 at the end); Quick Search closes, then posts
  the same message the menu entry would (`find_window::run_command`), so a look or size takes effect
  at once. A command that cannot apply (the look in use, the desktop already on) is listed and does
  nothing, as the bar already ignored such a choice from the menus.
  - Shown in the folder "Bevel95"; pictures: the look's own where it had the thing (Run, Shut Down,
    the Taskbar picture for Taskbar Properties and Switch back to Windows, Control Panel's for
    Date/Time, Explorer, My Computer), else Bevel95's silver B.
  - With nothing typed the list is the programs only, as before; a command shows once every typed
    word is in its name, its other words or "Bevel95". Ranking as before, a command's other words
    counting as words of its name for rank (b); Bevel95's own (its programs and the commands) first
    among equals. The status line says "N item(s) found".
  - Decided differently from the brief: the Windows Explorer command is left out while Bevel95's own
    Windows Explorer program is listed (`find::with_commands`): it would be a second row with the same
    name doing the same thing.
- PROVISIONAL: the name "Quick Search", every command's name and words, "item(s) found", the order of
  both Bevel95 submenus, the word "Bevel95" for the entry.
- **Renamed "Switch back to Windows"** (2026-11-17, the owner: "Yes on "Switch back to Windows""): the
  entry that was "Switch to Windows taskbar", in the bar's Bevel95 submenu (Ctrl+Win+T still shown
  beside it) and in Quick Search (still found by "taskbar", "windows 11", "switch", "restore"). It does
  what it did. Bevel95's menus show no underlined letters, so no access key can clash. Not renamed:
  the desktop shortcut "Restore Windows taskbar" (it closes Bevel95), the tooltip of Bevel95's icon in
  Windows' notification area, the names in the code (`BarCommand::SwitchToWindowsTaskbar`). Golden
  pictures changed, each only on that entry's row (git and a pixel comparison: x 441..541, y
  346..354): `<look>-bevel95-menu`, `-theme-menu`, `-size-menu`, `-16-colors-menu` (12; the last three
  show the submenu under their own); no other picture changed.
- Proof: unit tests (`bar_menu`: the menu and its submenu; `desktop`: the menu and its submenu, codes;
  `quick`: the table, a picture for each in every look; `find`: commands showing, ranking with
  programs, a new reading keeping them; `properties`: the new label). Golden pictures changed:
  `*-context-menu`, `*-theme-menu`, `*-size-menu` (now three menus deep), `*-desktop-menu`,
  `*-bevel95-settings-taskbar`, `*-find-programs`; new: `*-bevel95-menu`, `*-desktop-bevel95-menu`.
  `cargo compare-refs`, the full run (2026-11-16, after this job and "Dragging files"): all 704 cases
  match with no differing pixel (one skipped for a missing capture, win2000-calculator-sci-menu-help).
  - Live (`private/probes/bevel-menu/live.ps1`: real input only at idle moments, never while another
    program covers the screen, right-clicks only where the bar says no task button is or the desktop
    says no icon is): in all three looks at size 1 and 2, the bar's menu by a real right-click,
    Bevel95, Theme, Size by moving the pointer, closed by Esc (one level each) and by a click on the
    bar; the desktop's menu the same, closed by Esc and by a click on the desktop; Quick Search
    chosen from both menus (opens in front, Esc closes it); in Quick Search real keys typed "theme",
    "size", "sett" (rows as above), and "windows 98 look" with Enter (the look in use: nothing
    changed, logged). The looks and sizes were changed by the Theme and Size menus' own messages and
    put back; settings.txt was byte for byte as before. Leak check: one menu open, the Bevel95 and
    Theme submenus opened and closed 20 times by the arrow keys: USER 37 → 37, GDI 44 → 44.
  - Not tried by Claude: Restart, Exit and Switch back to Windows (from the submenu or Quick
    Search), so that the owner's bar never went away; they are on the owner's checklist.
- **Quick Search lists only programs by default** (2026-11-18, the owner after the release: "a
  bunch of stuff in quick search that should probably be hidden by default. Lots of about and
  documentation etc."). The Programs menu itself is unchanged; only Quick Search's list is
  narrowed, and a hidden entry is not found by typing either.
  - The rule is the core's (`find::hidden`, `find::listed`, unit-tested). **Since 2026-11-19
    only what is known to be no program is hidden** (the review before 0.1.1 found the first
    rule, "list only known program files", hiding shortcuts to scripts and other launchers:
    .vbs, .js, .wsf, .ps1, .hta, .jar, .py, .ahk, .scr, ClickOnce's .application). A hidden
    program is the costly mistake, a listed document the cheap one. An entry is hidden when
    what it opens is a web address or an Internet shortcut (`find::Target::Web`), a folder
    (`Folder`), or a file of a known document kind (`Document`, `find::DOCUMENT_EXTENSIONS`:
    txt, rtf, doc, docx, odt, pdf, chm, hlp, htm, html, mht, mhtml, md, log, nfo, ini, xml,
    json, csv, xls, xlsx, ppt, pptx, bmp, gif, jpg, jpeg, png); or when its name's first word
    (the letters and digits before the first other mark) is "Uninstall" or "About", case as it
    may be, whatever it opens: "Aboutface" and "Uninstaller" are listed. Everything else is
    listed: programs (.exe, .com, .bat, .cmd, .msc, .cpl, .pif, .appref-ms, Windows'
    Store-style apps, Windows Installer's advertised shortcuts: `Program`), and every other
    kind of file, a file without an extension, a shortcut that cannot be read and one to a
    shell object with no file, such as a Control Panel item (`Unknown`). Bevel95's own
    programs and the things to do (`core::quick`) are never hidden.
  - The Windows side tells the target (`find_window::target_kind`): a shortcut (.lnk) is opened
    with Windows' ShellLink object and only read (IPersistFile::Load, IShellLinkW::GetPath with
    no flags; nothing is searched for or repaired); IShellLinkDataList::GetFlags' SLDF_HAS_DARWINID
    marks an advertised shortcut, whose path is only its program's picture; any other entry is
    told by its own name. **The target itself is never asked** (since 2026-11-19; before, every
    target was asked whether it is a folder, so a dead network place stalled the reading for
    the network's timeout and a sleeping disk woke at every opening): whether it is a folder is
    taken from the WIN32_FIND_DATAW that GetPath fills, the target's attributes as the shortcut
    recorded them when it was made (MS-SHLLINK keeps them in the shortcut's header; none of
    GetPath's SLGP flags changes where that data comes from, and only IShellLink::Resolve looks
    for a target). Tried: a shortcut to a folder that was removed afterwards still reads as a
    folder, in 69 ms with the first COM start, a few ms for the next (`recorded_folder`, an
    ignored test). Reading shortcuts reads the disk, so it is done only on the "find
    programs" thread that already reads the list; the bar starts one such reading after its
    icons are read ahead, so the first opening finds the kinds known. Should a window open before
    any reading, the list is read without opening shortcuts (every shortcut `Unknown`, so listed)
    and corrected when the reading comes.
  - **The setting**: `quick_search = programs | everything` in settings.txt
    (`config::QuickSearch`, default `programs`, which a file without the line also means), and on
    Bevel95 Settings' Taskbar page a group "Quick Search" under Buttons (8 below it, as high as
    the General page's Startup group) with one check box, "Also list &documents, web pages and
    uninstallers", ticked for `everything`. It applies at once to an open Quick Search.
    PROVISIONAL: the words of the group and the box (the brief's were "Quick Search also lists
    documents, web pages and uninstallers"; under a group titled Quick Search the shorter form
    reads without repeating the name). The golden pictures `<look>-bevel95-settings-taskbar`
    changed by the new group; no other picture changed.
  - On the PC it was built on: 268 entries in all (4 of them Bevel95's own), 45 hidden by the
    target rule (24 web pages, 19 documents, 2 folders), 16 by the name rule, 207 listed. With
    the rule of 2026-11-19 (`quick_search_counts`, an ignored test): 266 in all, 24 hidden as
    web pages, 19 as documents, 2 as folders, 16 by name, 205 listed (this PC has no shortcut
    to a script; its documents are all of the listed kinds).
  - **Enter starts only a row shown selected** (2026-11-19, the review before 0.1.1): a reading
    that arrives while the window is open can take away the selected program (it was found to
    be a document), and an Enter already waiting is handled before the new list is painted, so
    it would have started the row that came under the selection, never shown selected. Now
    such a reading leaves nothing selected (`FindWindow::selection_lost`: no row highlighted,
    Enter does nothing) until the person moves (Up, Down, Page Up or Down select the first row
    shown), types or clicks a row; the same when rows come where nothing was found. A reading
    that keeps the selected program keeps it selected, as before.
  - Limits written down, not repaired (the review before 0.1.1): two shortcut names that differ
    only in a letter Windows' file system compares otherwise than Unicode's case rules (the
    Turkish dotless ı, the long s, the Kelvin sign) may merge into one entry of the Programs
    menu and of Quick Search; a shortcut's target is read into a buffer of 1024 characters
    (GetPath returns at most MAX_PATH, so a longer target is not expected to come back whole).
    The "find programs" thread now ends its COM use (CoUninitialize after a successful
    CoInitializeEx), and quick-search.txt's "could not save" is logged once in a run.
  - **Paths are compared as bytes, never cut** (2026-11-19, the review before 0.1.1): the
    first version cut a target's text at byte 17 to compare it with `shell:AppsFolder\`, and a
    shortcut's name 4 bytes from its end to compare it with ".lnk". Where that byte fell inside
    a letter of another alphabet (a target under a folder with a Cyrillic, Greek or Chinese
    name) the cut panicked, and by the panic rule the bar ended a few seconds after every
    start. Both compare bytes now (`find::classify`, `find_window::is_shortcut`), tested with
    such paths. A sweep of the whole workspace for text cut by a count of bytes found one more
    place that text from outside could reach: a folder window's name of a file without its
    extension was cut by the length of the lowercased extension, which can differ from the
    original's (`shell::quick_item`: it takes the path's own stem now).
- **Quick Search's column line can be dragged** (2026-11-18, the owner: "the attribute divider
  can't be moved in quick search"). The line between Name and In Folder works as a folder
  window's Details header divider (`find::Part::Divider`): it takes the mouse 8 left to 7 right of
  In Folder's first pixel (`desktop_icons::DIVIDER_LEFT`, `DIVIDER_RIGHT`, the Reference PCs'
  measure), shows the header's double-bar pointer (`Pointer::HeaderDivider`), draws an inverted
  line through the header and the rows while dragged, and Name takes the width the line shows
  when it is let go; a double-click on it fits Name to the longest name found (as a Details
  column fits its widest entry). Limits, ours: Name at least 38 (its picture and a few letters),
  In Folder at least 40; a remembered width that no longer fits (the scroll bar came) is shown
  within them. The width is remembered in a state file of its own, `quick-search.txt`
  (`name_width = N`, `find::state_text` and `read_state`, written whole through
  `winkit::state_file`; one of `state_file::OWN_FILES` and Setup's Remove list): it is the
  window's state, not a setting, as `task-order.txt` and the games' files are. Golden pictures:
  none changed (the window opens at 150 as before). Tried live (the 2000 look at size 2): one real
  drag 40 to the right (the line drawn, the column wider on release, the file written), one real
  double-click on the line (fitted, here to the limit), the window opened again with the width
  kept; the test's file was removed again afterwards.
## 16 Colors (2026-11-16, the owner: "a 16 color and True Color option")
- What the owner asked for: "I think we should have a 16 color and True Color option ... As a kid, I
  recall the 16 color icons (so that's the nostalgia)". A PC without its proper display driver showed
  Windows in 16 colors. Everything in Bevel95 was drawn as the three Windows looked in True Color.
  High Color looked the same as True Color on all three Reference PCs (no choice); 256 colors is for
  later (`Depth` is an enum, a third value can join it).
- **What each Windows did** (measured on the Reference PCs in 16 colors, `private/probes/colors16/
  survey.txt`, captures `private/reference/refs/vm/<pc>-16color-*`; values in theme-spec.md "16 Colors"): 95 changed
  almost nothing (only the 3D light DFDFDF became silver; its icons are 16-color drawings anyway); 98
  also lost its title-bar gradients (plain navy, plain gray) and has 16-color drawings of its icons
  and a black line-art Explorer toolbar; 2000 took the nearest of the 16 for everything (desktop teal,
  face silver, dark shadow black, selection navy, tooltip white), plain title bars, 16-color icon and
  toolbar drawings, and its own 16-color Shut Down banner. 95 and 98 fill a plain area in a color
  outside the 16 with a 4×4 pattern of two of them (the tooltip: white with two yellow dots in every
  4×4); 2000 never does. A selected icon is a checkerboard of navy over the picture in all three.
  Grayed-out labels are plain gray in 95 and 98 (etched in True Color).
- **How it is built: at the source, not as a filter over the finished picture.**
  - The setting: `colors = true|16` in settings.txt (`config::Config::colors`, a file without the
    line is True Color), Bevel95's own: Bevel95 Settings' General page (a **Colors** group beside
    **Look**, "16 Colors" and "True Color" as Windows' Display Properties worded them;
    `properties::Part::Colors`), the bar's **Bevel95 > Colors** submenu (between Theme and Size;
    `bar_menu::colors_menu`, `BarEntry::Colors`, `popup::Kind::Colors`), and Quick Search's
    "16 Colors" and "True Color" (`quick::Action::Colors`, found by "color", "colors", "colour",
    "colours", "vga"). It takes effect at once: `bar::set_colors` (posted as `WM_BAR_COLORS` =
    WM_APP + 40, wParam the index in `Depth::ALL`: 0 = 16 Colors, 1 = True Color) swaps the bar's
    theme, closes an open Run or Shut Down box (as a new look does), rethemes Taskbar Properties and
    Bevel95 Settings, gives the desktop the new theme and tells every program (`Bevel95.LookChanged`;
    `winkit::look` reads `colors` with the look).
  - `core::colors16`: `Depth`, Windows' 16 colors in VGA order, the conversion rules as data
    (`nearest` for 2000, `table_9x` for 95 and 98: the measured table of every color at 6 bits per
    channel, `core/assets/vga-9x.txt`, kept LF by `.gitattributes` because it is read by position),
    `fill` (95's and 98's dotted areas; only the measured FFFFE1 is dotted, any other color outside
    the 16 is filled plain with its table color), `picture` (a picture without a 16-color drawing:
    each pixel by the look's rule, see-through below half alpha), `selected` (the navy checkerboard,
    counted from where the picture is drawn), `invert` and `focus_dot` (the dotted focus rectangle
    by palette number, i -> 15 - i).
  - Every theme knows its era and depth (`Theme::era`, `Theme::depth`); `theme::theme_in(era, depth)`
    gives `WIN95_16`, `WIN98_16`, `WIN2000_16`: the same sizes, the colors of the 16-color screen,
    `Colors::etch` (what a grayed label is etched with: white, or the face on 95's and 98's 16-color
    screens, so it comes out plain gray), 95's and 98's menus without etching, 98's Start-menu icon
    squares the menu's own. A painter that draws something differently in 16 colors asks the theme:
    the caption (`window::caption_fill`, plain), pictures drawn through `colors16::draw_picture` (task
    buttons, notification icons, menu icons, window and dialog icons), the tooltip's area, Taskbar
    Properties' preview (its pieces cut from True Color captures, by the rule), 2000's Shut Down
    banner (its own 16-color drawing, cut from the Reference PC: `assets/win2000-shutdown-banner-16.rgb`),
    the desktop's selected pictures and focus frames (`desktop_icons::sixteen_picture`).
  - True Color's path is untouched: every True Color golden picture and comparison case is the same,
    except the pictures that show the new choice itself (Bevel95 Settings' General page, the
    Bevel95 / Theme / Size menus one entry longer).
- **Each look stays pure**: a 16-color drawing is used only in its own Windows' look; where a Windows
  had none, its own rule converts the True Color picture.
- **Decided differently from the brief**: (1) the measured 95/98 table is used as it is, also where
  four patches of the survey's TESTTC picture (3A6EA5, 4080C0, C08040, 40C080, all within a step of
  a border) came out differently from the table's nearest step (the table is exact at its own steps;
  the patches lie between them). (2) The Colors group shares the General page's first row with Look
  (the page had no room for another full-width group). (3) Windows 2000's grayed labels stay etched
  in 16 colors (confirmed later by Run's grayed OK on its Reference PC). (4) The focus rectangle's dots on 2000's Shut Down list
  alternate yellow and black, on the desktop's selected label yellow and navy, as captured.
- **Windows' own 16-color drawings** (2026-11-16, second step, the part the owner asked for by
  name): `core::colors16_art`, generated by `private/probes/colors16/match` (a small Rust program
  beside the survey, not part of the workspace). It renders each of Bevel95's True Color pictures
  of the 98 and 2000 looks (the desktop's, drives, Control Panel, Printers, the Start menu's large
  and small pictures, Settings' and Find's, folder and window pictures, the address bar's and
  Explorer's small pictures, the folder windows' toolbar), finds it among the Reference PCs' own
  files (the 8-bit twin of each 4-bit icon of SHELL32, EXPLORER and MYDOCS; 98's toolbar from its
  256- and 16-color screens, `private/probes/measure17/toolbar98`; 2000's in BROWSEUI.DLL's strips
  278/279 → 274/275, a picture taking only its own 16-wide cell) and takes the 4-bit drawing from
  the same place: 174 entries, each with the function that draws its True Color picture.
  `colors16::picture` looks a picture up by its pixels (a fingerprint of size and pixels, the
  table filled once) before converting by rule, so every place that draws through `colors16`
  shows the look's own drawing without knowing of it. Matches below 90% of the picture's pixels
  are not used (the report, `private/probes/colors16/match/report.txt`, lists every score). Hot
  pictures drawn differently from True Color's (98's Views, 2000's Up and Views) take their strip's
  picture where the cold one is; the grayed Back, Forward and Up are made from the plain drawing
  as Windows grays a picture (white copy under gray). Each drawing only in its own look.
  - Through `colors16` now also: the folder windows' toolbar, throbber, address bar and its list,
    trees (`tree::paint_row` takes the theme), Quick Search's list, Remove Shortcuts, the Confirm
    and Rename windows; a desktop shortcut's picture and arrow each looked up on their own.
  - The selected icon's checkerboard is counted from the picture's own corner (navy at its corner):
    95's, 98's and 2000's My Computer on the Reference PCs.
- **The web view** in 16 colors: 98 shows no watercolor (the Reference PC), its stretched line
  each pixel by 98's rule (red, yellow, lime, aqua: measured; the survey's "plain line" was the
  look of it at a distance); 2000's corner picture and line by nearest color (PROVISIONAL: its PC
  showed IE's dithered gray picture and a dotted line). **The volume control**'s focus dots XOR
  palette numbers. **Minesweeper**: 95's and 98's face bitmap has one color outside the 16
  (BFBF00), converted by the rule. Calculator, Minesweeper and Solitaire are otherwise 16-color
  drawings already: only their windows change.
- **Third step (2026-11-17)**: more of the PCs' files read straight out of their disk images with
  7-Zip (`private/reference/vm/x16/<pc>`, no PC started) and their icons dumped (`private/reference/refs/files/<pc>/more-icons`:
  NOTEPAD, SHDOCVW, WUPDMGR, IEXPLORE, CALC, SOL, WINMINE, HH, WINHLP32, SNDVOL32, SYSTRAY, PRINTUI,
  NETSHELL, STOBJECT, CDFVIEW, WEBCHECK, INETCPL). The matcher also takes a 4-bit-only image (a
  picture its Windows had only in 16 colors: True Color shows that same drawing) and cuts from a
  16-color capture what a Windows drew only on its screen: 98's and 2000's throbbers (SHDOCVW has
  them only at 256 colors), 2000's Go button, and Minesweeper's 16×16 title-bar picture on 2000
  (WINMINE has only a 32×32 icon). New: Notepad's window picture (98, 2000; the owner's "notepad
  icons look more complex than I remember"), Windows Update, 2000's Printers and small and closed
  folders, 98's small Favorites, Help and Run, 2000's Internet. The three masks are off. Help: the
  HTML Help viewers' pictures through `colors16`, the page each pixel by the rule (PROVISIONAL:
  Internet Explorer dithered pictures).
- **What is converted by rule only, and why** (one list; the scores in
  `private/probes/colors16/match/report.txt`):
  - No 16-color drawing of the same picture in the PCs' files (below 90% alike): 98's small Start
    pictures (Programs, Documents, Settings, Find, Log Off, Shut Down, Windows Update) and its
    Program group, the trees' group and Start Menu pictures (both), 98's Internet, 2000's small
    Log Off and hot Go, the speaker (SNDVOL32's own is 16-color only and 88% alike: Bevel95's True
    Color speaker differs from it slightly).
  - Pictures Bevel95 took from other files than that Windows' (no 98 or 2000 version to match):
    the 5¼", removable, network and RAM-disk drives.
  - The tray's network and battery pictures (BATMETER's are bitmaps of other sizes; not matched).
  - Today's programs' icons (decided: the rule, as Windows did for a picture without one).
  - Nothing to convert (each was a 16-color drawing in True Color already): 95's pictures, 98's and
    2000's Calculator, 2000's Solitaire, the Help viewers' pictures, the muted speaker, the Taskbar
    and Find Files pictures, 98's Minesweeper and Solitaire (16-bit programs, 16-color icons only).
  - A selected picture (98's selected hard disk) is never drawn in 16 colors: the checkerboard goes
    over the plain one.
- **Not built**: new captures of 98's Start menu (the survey's caught it sliding), 2000's Taskbar
  Properties and Date/Time (its captures show other windows), cases for the message boxes (the
  captured "cannot find" box is not one Bevel95 draws), Solitaire (its window is sized by its
  player, the cards and backs unchanged as the survey found), 95's Minesweeper and Solitaire (not
  installed on its PC: Windows Setup from its CD, not quick), and 95's WinHelp pages in 16 colors
  (95 hardly changes there).- Proof: `colors16` unit tests (rows of both measured tables, the 16 staying themselves, the dotted
  tooltip, the checkerboard), config, menus, Quick Search; golden pictures `<look>-16-<state>` (27, new)
  and `sixteen_color_scenes_use_only_the_16` (every pixel of every 16-color scene is one of the 16);
  `cargo compare-refs` 16-color cases (`core/tests/sixteen_cases`, `read_capture` translating the
  emulator's 16 colors, `locate` finding a window on 2000's 640×480 screens): Shut Down (three looks),
  Taskbar Properties (95, 98), My Computer selected on the desktop (three looks, Windows' own
  drawings), the Start menu (95, 2000), Run (three looks), the folder windows' toolbars (98's whole
  rebar, 2000's Back/Forward/Up with the title bar), Calculator (three looks), Minesweeper (98,
  2000), the bar (98), Date/Time (95, 98): 23 cases, 0 differing pixels, no masks for missing pictures. Live: the option
  switched by message on the installed bar in each look (`private/probes/colors16/live.ps1`: Bevel95
  Settings, Taskbar Properties and the Start button photographed, True Color and the look put back,
  settings.txt byte for byte as before).

## Groundwork for sharing (2026-11-17, the owner: making Bevel95 public)
Three small changes before strangers run Bevel95, report problems and send their logs (todo.md,
"Towards something sharable"). Each its own commit.
- **The classic desktop on and off from the bar and by a key** (the owner: "currently I can turn
  off the classic desktop with a right click but it's not that easy to flip back on ... also have a
  short cut if there's one free"). The bar's Bevel95 submenu has an entry right under Switch to
  Windows taskbar (since renamed Switch back to Windows), worded the same way: **Switch to Windows desktop** while the classic desktop is
  on, **Switch to classic desktop** while it is off (`bar_menu::Own`, the submenu's state when it
  opens: `popup::Kind::BarOwn(Own)`; `BarCommand::DesktopOff` / `DesktopOn`, which Quick Search's two
  desktop commands already used). Both entries, the desktop menu's own Switch to Windows desktop and
  Bevel95 Settings' Desktop box are the one setting `desktop` (`bar::set_desktop`, which also updates
  an open Bevel95 Settings).
  - **The key: Ctrl+Win+W** (`bar_menu::DESKTOP_KEY`, registered beside Ctrl+Win+T as hotkey 2). Tried
    on this Windows 11 (25H2) by registering every Ctrl+Win+letter for a moment: taken were C, D (new
    virtual desktop, the obvious one), E, F, L, M, N, O, P, Q, S, V and T (the bar's own); free were
    A, B, G, H, I, J, K, R, U, W, X, Y, Z. W, as T is for the taskbar ("Windows desktop" /
    wallpaper). Shown beside the entry in the bar's submenu and in the desktop's Bevel95 submenu, as
    Ctrl+Win+T is; if another program holds it at the bar's start, the log says so
    ("RegisterHotKey(Ctrl+Win+W) ... failed") and the menus show the entries without it
    (`bar::desktop_key`).
  - **While the bar is switched to Windows' taskbar** the key does nothing (logged): the classic
    desktop shows only with Bevel95's bar, so a switch nobody sees would only surprise on the way
    back. Ctrl+Win+T brings the bar back with the desktop as it was.
  - PROVISIONAL: the entry's two wordings and the key.
- **About Bevel95, with a version.** One version for the whole project, the workspace's (`version =
  "0.1.0"` in Cargo.toml's `[workspace.package]`, which every crate takes); the build is the commit
  it was made from, asked of git by `platform/winkit/build.rs` (`git rev-parse --short=7 HEAD`,
  asked again when HEAD or the index changes, i.e. at every commit) and kept as
  `bevel_winkit::BUILD`; a build without git (or outside the repository) says "unknown". No new
  crate. Since `tools\install.ps1` runs before the step's commit, the installed build names the
  commit before the step until the next install.
  - **The name in one place**: `bevel_core::product_name!` (a literal, for `concat!`) and
    `bevel_core::about::NAME`. The About window, the log's first line, Bevel95 Setup's title, its
    labels and its message boxes take it from there. Not taken from it (they would have to move
    with a new name by hand, on purpose): the settings folder `%LOCALAPPDATA%\Bevel95`, window
    classes (`Bevel95Window`, ...), the desktop shortcut "Bevel95" that `tools\install.ps1` keeps,
    registered message names, "Bevel95 Settings" and the Bevel95 submenus' word.
  - **The window** (`core::about::AboutWindow`, `platform/win/src/about_window.rs`): the last entry
    of the bar's Bevel95 submenu, after a line (`BarCommand::About`, code 13), and Quick Search's
    "About Bevel95" (found by "about", "version", "build", "help", "licence", "license"; shown also
    for "p" in the golden Quick Search picture, by "help"). Built like the programs' About boxes
    (the shell's About): the look's frame, title bar and font, the silver B at the left, the name,
    "Version 0.1.0 (build 1a2b3c4)", what it is ("A Windows 95, 98 and 2000 look for today's
    Windows"), an etched line, the notice ("Not affiliated with Microsoft. Windows is a trademark of
    Microsoft Corporation; the pictures of its user interface belong to Microsoft.", wrapped to the
    window) and OK. 380 wide (the shell's About is 348; the line of what it is needs a little
    more). Opens in the middle of the bar's screen, above other windows like the bar's other
    windows; OK, Enter, Esc and X close it; a new look, colors or size close it.
  - Decided differently from the look's own manner: Windows 2000's About boxes carried Microsoft's
    Windows 2000 banner; Bevel95's does not (it is not Microsoft's product), so the three looks
    share the 95 and 98 layout in their own frame and font.
  - **The log's first line** at every start: `Bevel95 0.1.0 (build 1a2b3c4): bevel95.exe starting`
    (`winkit::log::started`), written by the bar when it starts (after Setup, so a cancelled first
    run still leaves nothing behind), by `--restore` and the `--list-...` diagnoses, and by each
    program (Minesweeper, Solitaire, Calculator, the folder windows, the Help viewer) first thing.
    The watchdog's two short-lived jobs write none.
  - PROVISIONAL: the line of what it is and the notice (the owner's to word; todo.md "Waiting for
    you"), the window's layout.
- **No private text in the log**: the rule is in "Milestone 1 decisions". The lines changed, by
  kind: window titles (SetForegroundWindow refused, a command not sent: now the window's class and
  program file); notification icons' tooltips (an icon added: now its program file and whether it
  has a tooltip); Run's typed text (started, could not start: its kind); a program started from a
  menu that failed (its kind); New on the desktop (the name made: "a folder", "a text document");
  the classic pictures' places (Documents' path among them: counts); paths of Bevel95's files
  (settings.txt, the clock, task order, programs.txt, the Windows taskbar's state file, Start with
  Windows, Setup's copy, update and start, Calculator's and Minesweeper's saves: `path_kind`);
  `--list-programs` (each program's and folder's name and the recent documents' names: now counts
  and the tree's depth); `--list-status` (the network's name: whether it has one); an unknown
  argument (shown only if it looks like an option). Kept: Bevel95's own commands, settings, file
  names and its own desktop items' ids (`::my-computer`), Windows' class names, program file names,
  error codes. Lines already in an existing log stay until it reaches 1 MB and starts afresh.
- Proof: `cargo test --workspace` 458 passed, 0 failed, 6 ignored; clippy clean. Unit tests:
  `bar_menu` (the submenu's entries, the desktop entry's four wordings), `desktop` (the key beside
  its entry), `about` (name, version, log line, the OK button following the notice), `quick`,
  `find` (22 commands), `winkit::log` (`path_kind`, `typed_kind`), `tests/private_log.rs`. Golden
  pictures changed: `<look>-bevel95-menu`, `-theme-menu`, `-size-menu`, `-16-colors-menu`,
  `-desktop-bevel95-menu`, `-find-programs` (About Bevel95 found by "p"); new: `<look>-about`,
  `<look>-16-about`. `cargo compare-refs`: the desktop menu cases, then the full run after the
  three commits: 729 cases, no differing pixel (one skipped for a missing capture,
  win2000-calculator-sci-menu-help).
  - Live on the installed bar (`private/probes/release/live.ps1`: real input only at idle moments,
    never while another program covered the screen, right-clicks only where the bar says no task
    button is): in each look the desktop entry chosen twice from the bar's menu (off, then on;
    settings.txt and the desktop window followed, the entry's wording changed); Ctrl+Win+W by
    real key presses twice in the 2000 look (off, on); About Bevel95 from the bar's menu in 95,
    98 and 2000 (closed by Enter and Esc), from Quick Search in 98 ("about", Enter; closed by a
    click on OK), in 95 with 16 Colors. The looks and colors were switched by their menus'
    messages and put back; settings.txt byte for byte as before. The new log after the install
    held no quoted text; its lines were checked by their templates only.
  - Not tried live: the desktop's own Bevel95 submenu (the owner's windows covered the desktop:
    golden picture and unit test only), the key while the Windows taskbar is shown, the key
    taken by another program, a build without git.

## Bevel95's icon in Windows' notification area (2026-11-17, the owner: "a tray icon (only in real taskbar, not ours), so that people know it's running")
- **Why**: after Ctrl+Win+T (or Bevel95 > Switch back to Windows) Bevel95 keeps running with
  nothing to show for it; a stranger forgets it runs, or the key that brings it back.
- **When**: while Bevel95 is switched to Windows' taskbar, and only then, one icon of Bevel95's own
  is in **Windows'** notification area. While Bevel95's bar shows it exists nowhere, not in
  Bevel95's own notification area either.
- **How it reaches Windows' area** although Bevel95 answers for "Shell_TrayWnd" while its bar shows
  (`tray.rs`, "Notification area" above): Shell_NotifyIcon hands an icon to the first
  "Shell_TrayWnd" FindWindow finds. The switch destroys Bevel95's stand-in (`tray::stop`), so the
  icon is added right after it (`show_windows_taskbar`, then `own_icon::show`) and goes straight
  to Explorer's window; switching back takes the icon away first (`own_icon::hide` at the top of
  `show_ours`), before the stand-in is made again and asks programs for their icons. So it never
  passes through Bevel95's area, not even for a moment (the log, which names every icon the
  stand-in takes in by program, shows none of bevel95.exe over 31 switches).
- **The icon** (`platform/win/src/own_icon.rs`): bevel95.exe's own program icon (resource 1,
  `tools/build/program_icon.rs`), loaded with LoadImageW at Windows' small icon size for the main
  screen's DPI (16 at 100%; at 150% Windows scales the 32 picture itself, Bevel95 draws nothing
  here), its window the bar's (hidden), number 1, version 4 (NIM_SETVERSION), its messages
  `WM_APP + 41` (`WM_OWN_ICON`). Tooltip `core::tray_icon::TOOLTIP`, from the one name: **Bevel95
  (Ctrl+Win+T switches back)** (NIF_SHOWTIP, which version 4 needs for a plain tooltip).
- **Clicks** (`core::tray_icon::respond`, tested headless): a left click (NIN_SELECT, after the
  button comes up) or Enter on the icon (NIN_KEYSELECT) switches back, exactly as Ctrl+Win+T; a
  double-click too, once: its first click already switched (the icon is gone) and what Explorer
  sends after it finds Bevel95's bar showing and does nothing. The plain button messages do nothing
  by themselves. A right click (WM_CONTEXTMENU) opens the menu.
- **The menu** (`core::tray_icon::menu`, `popup::Kind::OwnIcon`): drawn by Bevel95 in the look in
  use like all its menus: **Switch to Bevel95** (bold, the default: what a click does), **Bevel95
  Settings**, **About Bevel95**, a line, **Exit**. Each posts the bar the same command its entry of
  the Bevel95 submenu does (`BarCommand::Settings`, `About`, `Exit`; Switch to Bevel95 is the new
  `BarCommand::SwitchToBevel95`, code 14, which does nothing while the bar shows, as
  `SwitchToWindowsTaskbar` now does nothing while Windows' taskbar shows). Exit ends Bevel95 and
  leaves Windows' taskbar as it is. It opens at the pointer, upward (`tray_icon::place_menu`: its
  bottom at the pointer, to the right unless that runs off the screen, always on it), takes the
  focus as the bar's menus do, and closes at a click elsewhere and on Esc. Golden pictures
  `<look>-own-icon-menu` (the menu alone).
- **While switched, the bar places and reserves nothing** (`reposition` returns at once): Bevel95
  Settings, opened from the icon, may apply a size or look meanwhile, and that used to register
  the appbar (Windows' work area would have shrunk under Windows' own taskbar). `show_ours` places
  the bar when it comes back.
- **When it comes and goes**: added at each switch to Windows' taskbar; taken away when switching
  back (click, menu, Ctrl+Win+T, the command), at exit, restart and `--restore` (all through the
  bar's own `on_destroy`; Setup's update and Remove close the bar with `--restore`). Explorer
  restarted ("TaskbarCreated" from Explorer, not Bevel95's own broadcast) while switched: the
  icon is added to the new taskbar (or, if it still has it, brought up to date with NIM_MODIFY).
  The bar killed or crashed while switched: the icon stays in Windows' area, dead, until the
  pointer passes over it (Windows' rule). Windows' taskbar refuses to be asked sooner: a delete
  naming the dead window fails (`private/probes/tray-icon/dead-icon.ps1`: a throwaway icon of a
  throwaway process, killed; its delete with the dead handle answered no), so the watchdog does
  not try. The bar the watchdog starts after a restart shows its own bar and adds no icon, so
  there is never a second one at once.
- **Windows 11 puts a new program's icon behind its arrow (overflow)**: Windows' rule and the
  person's choice; Bevel95 does nothing about it (README says so).
- Log lines: "own icon: in Windows' notification area", "... taken away", "switching back to
  Bevel95 (its notification icon: Select)", the failures of Shell_NotifyIcon, LoadImageW and
  DestroyIcon. No text from outside Bevel95.
- No new crate, no new windows-sys feature (`Win32_UI_Shell`, `Win32_UI_HiDpi`,
  `Win32_UI_WindowsAndMessaging` were on). NIN_KEYSELECT (NIN_SELECT | NINF_KEY) is not in
  windows-sys and is defined in `own_icon.rs`.
- PROVISIONAL: the tooltip's words and the menu's (todo.md "Waiting for you").
- Proof: `cargo test --workspace` 470 passed, 0 failed, 6 ignored; clippy clean. Unit tests
  `tray_icon` (tooltip, click rules with a double-click switching once, the menu and its commands,
  placement), `bar_menu` (the new code). Golden pictures: three new, none changed.
  - Live on the installed bar (`private/probes/tray-icon/live.ps1`: only window messages, each step
    after 8 s without input and with no other program's window covering the screen): switched by
    the bar's own command; Windows' taskbar answered for the icon (Shell_NotifyIconGetRect gave the
    overflow arrow's place, a NIM_MODIFY with nothing to change was accepted); Explorer's messages
    for a click, a double-click (the whole sequence: one switch back), Enter, a right click; the
    menu opened at the pointer upward (bottom edge at the pointer's y), photographed alone in the
    2000 look at 2x: pixel for pixel the golden picture; Esc closed it (the bar stayed switched);
    Switch to Bevel95, Bevel95 Settings and About Bevel95 by the menu's keys (each window
    photographed, then closed); "TaskbarCreated" posted to the bar (the icon brought up to date);
    Exit once (the icon taken away, the bar ended, the watchdog restored), then the bar started
    again with `tools\start.ps1` (parent explorer.exe). Every try ended with the bar showing and
    the classic desktop as before. Leak check: 20 switches there and back: USER 30 → 30, GDI
    43 → 43.
  - Not tried: a real click on the icon (it sits behind Windows' arrow; reaching it means
    guessing where in the overflow it is), a real Explorer restart (it would close the owner's
    folder windows), the menu in the 95 and 98 looks live (golden pictures only).
## Windows' pictures in one place (2026-11-17, groundwork for sharing, part 3)
The owner: Bevel95 ships with its pictures built in, one download that works ("I do like that our
builds currently don't sit next to a pile of files"), but what was taken from Windows 95, 98 and
2000 must sit in one separable place: listed, and removable in one move if Microsoft asked.
- **The place: `art/`, the workspace member `bevel-art`** (the name suggested in the brief; the
  code had nothing better: the core's modules for each group were already called `*_art`). Data
  and nothing else: `#![no_std]`, `#![forbid(unsafe_code)]`, no dependencies, no drawing and no
  rules. `art/src/*.rs` hold the pictures written as text (rows and palettes, as before), the
  captions and sheet tables; `art/assets/` the 53 files that were in `core/assets` (pictures,
  Minesweeper's three sounds, the Help viewers' and Solitaire's screen fonts), byte for byte.
  `private/art-inventory.md`, the project's private list (not in the public copy
  since 2026-11-18), lists every one, its look, size, source and user. (`art/NOTICE.md`, which
  said whose they are, was removed on 2026-11-18: "The public copy".)
- **The one way in**: `bevel_art::get(what, look, depth) -> Option<Art>`, a picture looked up by
  what it is (`"start/programs"`, `"tray/net-idle"`, `"16-colors/desktop-mycomputer"`), the Windows
  it was taken from (`Look`) and the screen it was taken from (`Depth::TrueColor` or `Sixteen`).
  `Art` is the form it is kept in: `Rows` (a picture written as text), `Bytes` (a file),
  `Colors`, `Captions`, `Places`, `Text`. A picture the three Windows had alike answers for each
  (`EVERY_LOOK`); the core's rules decide where else a picture shows (98's Windows Update entry
  shows 2000's picture: the core asks for 2000's). `entries()` lists them (tests, the inventory).
- **Looked up while building**: `get` is a `const fn`, and the core looks every picture up in a
  `const` or `static` item, a `const { }` block or a `const fn` (`core::art`: `rows`, `bytes`,
  `exactly`, `colors`, `captions`, `places`; `ClassicIcon::art`, `Drawing::art`, `Banner::art`). So a
  name that is not there stops the build, and only the pictures a program uses end up inside it,
  as before. One mistake found on the way: four lookups ran while the program ran (in closures and
  match arms), and that put all of bevel-art into every program (each about 1.6 MB bigger) until
  they became `const { }` blocks; the guard test now catches that.
- **The exception to the core's rule** "zero dependencies": `bevel-core` depends on `bevel-art`,
  the project's own data library, and on nothing else (CLAUDE.md's "Stack and layers" needs the
  line; for the owner to change).
- **What moved** (no picture changed, no golden picture changed, compare-refs as before): the
  Rust-source pictures of `core::icons` (81), `desktop_art` (26), `window_icons` (16), `status` (28),
  `folder::art` (16), `folder::tree_art` (7), `html_help::icons` (7), `colors16_art` (107 drawings),
  `banner` (3 banners and their top colors), `calculator::captions95` (85 captions), `drag` (7),
  `kit` (1), `window` (4), `dialog` (2), `paint` (6: check mark, bullet, the window menu's
  glyphs), `disk_box`, `flag`, `help::winhelp` (3) and `winhelp_article` (1), `html_help::commands`,
  `article` (the note) and `support` (its colors), `calculator::stats` (2), `folder::pane` (the close
  X), `folder::web` (98's line), `minefield::dialogs` (2000's flag), the places of the folder
  windows' sheets; and the 53 files. 385 entries in all. Each core module keeps its rules and a
  one-line lookup where the data was (`const WIN95_HELP: ClassicIcon = ClassicIcon::art("start/help",
  Look::Win95);`), moved by a small program (`private/probes/art/mover`, one job file per module).
- **What stays**: the project's own drawing, Bevel95's silver B (`core::bevel_icon`); measured rules
  and tables (`core/assets/vga-9x.txt`, `best_fit.rs`, positions, the looks' colors, the 16 VGA
  colors); the interface's words and the Help books' text; the golden pictures and the font
  fixtures (test material that shows or renders Microsoft's pictures and fonts: the inventory's last
  section says so).
- **The tools write for the new place**: `tools/dev/icon2rust.ps1`, `cutmenu.ps1` and `cuttask.ps1`
  print bevel-art's `Rows` constants; `private/probes/colors16/match` writes `art/src/colors16.rs` (the
  drawings) and `core/src/colors16_art.rs` (which True Color picture each belongs to); the cutters
  under `private/probes` (`web/cut.ps1`, `web/cutfo.ps1`, `desktop-menu/cuticons.ps1`,
  `desktop-drag/cutundo.ps1`, and the Python ones, which this PC cannot run: no Python) write to
  `art/assets`; `folders2/cut.ps1` takes its output path as before. Run once each: icon2rust gave 20
  of 95's folder, drive and tree pictures, cutmenu 95's Printers, cuttask 98's Notepad, cutdesk with
  icon2rust 98's My Computer, byte for byte as in `art/src`; the 16-color matcher rewrote the old
  `colors16_art.rs` byte for byte before the change and both new files byte for byte after it;
  `web/cut.ps1`, `cutfo.ps1` and `cuticons.ps1` rewrote their 8 files unchanged; `folders2/cut.ps1`
  gave the sheets' first 96,708 and 70,732 bytes and all 85 table lines as committed. `cargo
  regen-golden` writes where it did (the golden pictures are not moved); not run (`cargo test`
  compares every golden picture with the drawing: all equal).
- **The guard** (`art/tests/one_place.rs`, part of `cargo test`): fails when a file of pictures,
  sounds or fonts (`.rgb .rgba .pic .idx .idx4 .bits .pcm .palrle .i4 .glyphs .bin .bmp .ico .cur
  .wav .gif`, or a PNG outside `core/tests/golden`), six rows of a picture, a palette or a data
  file's bytes appear in the project outside `art/` (allowed: the golden pictures, the font
  fixtures, `core/src/bevel_icon.rs`); when a lookup would run while a program runs; when an entry
  of bevel-art is listed twice or not whole.
- **Sizes** (release, before at ddee9ee, after): bevel95.exe 4,320,256 → 4,321,792; bevel-help.exe
  1,998,848 → 1,987,072; calculator.exe 2,038,272 → 2,084,864; folders.exe 3,166,208 → 3,168,768;
  minefield.exe 1,795,072 → 1,794,560; solitaire.exe 1,875,456 → 1,875,456. Calculator's 46 KB are
  code laid out differently by the compiler (its Help catalog's functions, compared by the linker's
  map: `.text` +36 KB, `.rdata` +9 KB, no picture twice).
- **Proof**: `cargo test --workspace` 474 passed, 0 failed, 6 ignored (4 new: the guard's);
  clippy clean; no golden picture changed; the full `cargo compare-refs` after the last picture
  moved: 729 cases with no differing pixel, one skipped for a missing capture as before
  (win2000-calculator-sci-menu-help). Installed: the bar started through Explorer, its log clean.

## Every picture from the owner's own Windows (2026-11-18, groundwork for sharing)
The owner: "why are we taking "the 16x16 Start flag come from RetroBar" if that's actually a windows
icon?" A few early pictures reached the project through someone else's file or published
screenshot (RetroBar's flag, GUIdebook's screenshots, the v86 emulator's windows). All are pictures
of Windows itself; each was looked for in the owner's own material and, where found identical, its
recorded origin became the Reference PCs. No pixel and no golden picture changed.
- **How**: `private/probes/own-sources/scan` (a small program reading bevel-art) searches every True
  Color capture of a picture's Windows (`private/reference/refs/vm`, 1,712) for a place where each drawn pixel is
  the same, then the pictures read out of the PCs' files (`private/reference/refs/files`: icons with their
  see-through masks; the bitmaps of EXPLORER.EXE and SHELL32.DLL, dumped by
  `private/probes/own-sources/bitmaps.ps1`), and cuts each find into `private/probes/own-sources/cuts`. The
  16-color captures cannot serve (QEMU shows those colors with values of its own). No Reference PC
  was started. Reports: `private/probes/own-sources/scan-*-report.txt`.
- **Found identical, 68 of 83 pictures**: the flag (in 388, 390 and 400 captures of 95, 98, 2000);
  the three Start banners (every capture with the Start menu open, and bitmap 157 of each
  EXPLORER.EXE) and their top colors; 23 of the 25 large Start entries; 95's and 2000's program
  groups and 98's Favorites folder; the Run, Shut Down and Log Off pictures (2000's Log Off key is the
  middle 32×32 of the 48×48 256-color image of SHELL32.DLL's icon 45, see-through pixels included);
  2000's Shut Down banner; 17 of the 25 small Start entries (95's and 98's are the 16-color images
  of the same icons in the PCs' files, 2000's Documents a capture); 98's four window-menu pictures
  (Calculator's window menu, each at its measured place); 9 of the 14 Taskbar Properties pictures
  (98's and 2000's EXPLORER.EXE bitmaps 150, 149, 151 and icons 107, 108; 2000's bitmaps with its
  3-D grays C0C0C0 and DFDFDF drawn as D4D0C8, as Windows maps them). 98's Programs folder is the
  16-color image of SHELL32.DLL's icon 20, so its source is that file now.
- **Not found, origin unchanged (15)**: 98's and 2000's 32×32 Log Off (the PCs' Start menus have no
  Log Off; SHELL32.DLL's icon 45 differs in 118 and 189 of their 305 pixels); 95's small Documents
  (95's own SHELL32.DLL icon 21 differs in 1 pixel: v86's 95 had Internet Explorer 4); 2000's other
  seven small entries (in no capture, and in none of the icons read from 2000's SHELL32.DLL,
  EXPLORER.EXE and the other copied files); both Taskbar Properties designs' window (38 of 7,254 pixels differ: where bitmap 149 is
  see-through, v86's screen showed the taskbar behind) and taskbar without clock (538 pixels, x 184
  to 258: Windows paints the clock away); 2000's Clear picture. Finding them needs a Reference PC
  capture (Start menu with small icons and Log Off shown, Taskbar Properties in True Color); done
  the same day on the owner's word: the next section.
- **Seen on the way, not changed**: in True Color Windows 98 draws the 256-color images of its
  Programs folder and of 8 of its 10 small Start entries; Bevel95 has their 16-color images (55 to
  216 pixels differ each). The 95 look's Taskbar Properties shows 98's (Internet Explorer 4's)
  pictures; the owner's Windows 95 EXPLORER.EXE has its own (its taskbar differs in 421 pixels).
- **Numbers**: theme-spec.md's values whose only source was RetroBar, read off the captures
  (`private/probes/own-sources/barcolors.ps1`, `vm/<pc>-truecolor-tasks-1`): Highlight FFFFFF, Shadow
  808080, Text 000000, Clock text 000000, the selected menu text FFFFFF, the Start button's padding
  2, 1 (the flag at 6, 579), the notification area's padding 2 (its first icon 3 in, 4 in 2000): all
  hold, their source is now the Reference PC. Not shown by any capture, still RetroBar's: a
  flashing button (face, text, weight) and the 1 pixel after a task button's label.

## The last pictures from the owner's own Windows (2026-11-18)
The owner: "I think it's obvious we should match the discs." and "having our own clean set of images
sounds proper. We can look into variants down the line." So the fifteen of the section above were
looked for on the Reference PCs themselves, and since then every picture in `art/` comes from them.
- **Captured** (all three PCs, one at a time, snapshot mode, shut down after; 63 True Color captures
  in the new folder `private/reference/refs/vm/own-pictures`, by `private/probes/own-pictures/p.ps1`, a copy of
  `colors16/c.ps1` that saves there): Taskbar Properties from Start > Settings with the keyboard, as
  it opens, each check box changed with its access key, the second page (`<pc>-truecolor-props-*`);
  2000's Start menu with Display Favorites and Display Logoff ticked (Advanced page) with large and
  with small icons; 95's with small icons; 98's with Log Off. 98 shows Log Off only once someone has
  logged on: Passwords (`control password.cpl`) > User Profiles > "Users can customize...", Restart
  (it came back; snapshot mode keeps the change until QEMU stops), log on as "Bevel95" with no
  password. Each Start menu as it opened and with each entry selected in turn (`-sel-<n>`, Down),
  which gives the see-through pixels; 98's Log Off window too.
- **Found identical** (`private/probes/own-sources/scan` run on copies of the captures; report
  `private/probes/own-pictures/scan-report.txt`): 98's and 2000's 32×32 Log Off, 2000's seven small
  entries, both designs' preview window and taskbar without clock, 2000's Clear. Their origin is
  now the Reference PC; no pixel changed. Why the earlier check missed the preview window: it
  compared it with bitmap 149 alone, which carries the auto-hide outline over the window's bottom;
  Windows' window is bitmap 149's top 100 rows over bitmap 152 (62×17), exactly. 98's Log Off
  window shows the same key as 2000's (`dialog/log-off`, at 279, 264).
- **Replaced** (the owner's differ; same size and place):
  - 95's small Documents: 1 pixel ((12, 8) is see-through, v86's 95 drew it white).
  - 95's small Find: its Start menu draws SHELL32.DLL's icon 23, the same drawing as icon 134
    (which Bevel95 had, from v86) one pixel further right: 122 pixels.
  - The 95 look's Taskbar Properties preview: plain Windows 95's own Start menus large (1,893 of
    15,048 pixels differ from Internet Explorer 4's) and small (1,362 of 11,385), taskbar (421 of
    7,801) and taskbar without clock (170 of 7,801), `art/assets/w95-preview-*.rgb`, cut from its
    PC's Taskbar Properties (the same as its EXPLORER.EXE bitmaps 150, 149, 151); its window,
    Customize and Documents pictures are 98's. Every piece sits where Bevel95 puts it (the PCs'
    previews at 37, 243 on the screen: frame and places measured from v86 hold), so no layout
    changed. The "ie4-" files are now 98's only.
  - Each look its own where it showed another's: 98's Windows Update (it showed 2000's; 356 pixels),
    2000's Favorites (it showed 98's), 2000's small Favorites and small Log Off (98's).
- **New comparison cases** (`core/tests/reference.rs`, `own_start_cases`, `own_properties_cases`;
  `BEVEL95_CASE=own-`): 12 Start menus (98 with Log Off, 2000 large and small, 95 small; as opened
  and with entries selected) and 18 Taskbar Properties windows (each PC's six states), every one
  exact. Masked in them, none a picture: 2000's "Use Personalized Menus" and its Start Menu Settings
  list (as in the v86 cases); in 95 and 98 the row under the selected tab (Bevel95 draws the page's
  light edge there, the PCs do not: private/todo.md); in 95 its own words ("Show Clock", the second
  page's sentences), where the 95 look has Internet Explorer 4's (private/todo.md, for the owner).
  2000's menus begin with "Set Program Access and Defaults" and 95's has "Suspend": ours is compared
  around them. 95's small Start menu is 127 pixels wide on its PC (arrows 9 from the edge), Bevel95's
  121 (a PROVISIONAL rule, theme-spec.md): compared to past the labels; the width is left for a
  layout job (private/todo.md), as it changes a golden picture beyond the pictures. The v86 cases of the 95 look's
  Taskbar Options page now leave its preview out (that 95 had Internet Explorer 4); the 16-color
  case `win95-16-properties` compares it again (it masked it), exact.
- **Golden pictures changed** (each shows a replaced picture, nothing else; pixels changed):
  `win95-properties` 2,314 and `win95-16-properties` 2,314 (the preview), `win95-start-menu-small`
  123 (Documents 1, Find 122), `win95-find-programs` 121 (Quick Search shows 95's small Find),
  `win98-start-menu` 356 and `win98-favorites` 356 (Windows Update), `win2000-start-menu-small` 75
  (Log Off). The README's `docs/img/start-menus.png` drawn again: 356 pixels (98's Windows Update).

## The public copy (2026-11-17, groundwork for sharing, part 4)
Nothing is published by this; it prepares what a stranger reads and a script that makes a clean
copy, for the owner to publish when they decide (a fresh history, todo.md "Towards something
sharable").
- **Licence**: GNU GPL version 3 or later (`GPL-3.0-or-later`) for the project's own code and
  writing: `LICENSE` is the text from gnu.org unchanged; every crate takes `license` from the
  workspace, except `bevel-art`, which has none (Windows' pictures are not Bevel95's to license).
  The owner kept the name and "or later" on 2026-11-18 ("Bevel95 is fine.", "V3 or later seems
  fine.").
- **Short, like the projects it stands beside (2026-11-18)**: the owner: "simpler is better",
  "Do we need notice.md at all? Seems like we are drawing attention to all of the assets we
  use.", and about a copyright line: "Does retrobar list "copyright"? Seems weird to put that
  in." So the README is short (what it is, the pictures, features, install, switching back,
  building, the licence paragraph, and last "See also", links to related projects as a nod to
  the community: the owner, "I don't think we need "Inspired by RetroBar"" but "a section with
  links to other projects like retrobar and other big ones, at the very bottom"), modelled on
  RetroBar's; everything else it held went into a `docs/guide.md`. Changed the same day: the
  owner, "By simple I didn't mean to remove all of the detail about installing for example. I
  was really just referring to the license etc and notes at the bottom." So the README carries
  its instructions again (its text of before, word for word down to Contributing, but for the
  note under the pictures), and only its bottom is short: the License paragraph and "See also"
  replace the old licence, notice and credits. `docs/guide.md` was removed. There is no `NOTICE.md` and no `art/NOTICE.md`, and no "Copyright (C)"
  line of Bevel95's own anywhere: the README's License paragraph alone says that the Windows
  artwork is Microsoft's and not covered, and that Bevel95 is not affiliated with Microsoft.
  The inventory of Windows' pictures (`private/art-inventory.md`) stays in this checkout, private.
  The credits that left the README (RetroBar's theme numbers, ManagedShell's way of hiding the
  taskbar, 98.css, GUIdebook, Toasty Tech) were sources of measurements and ideas, not of code,
  and stay named where they were used (theme-spec.md, this file).
- **The stranger's pages**: README.md (what it does and does not do, needs, install, switching
  back, removing, what is missing, files, log, reporting, building, contributing, License, See
  also; Undo not advertised while undecided; its "Files it uses" names every file Remove deletes,
  which `install.rs`'s test `remove_knows_every_data_file` checks), two pictures in `docs/img/`:
  `programs.png`, composed from golden pictures, and `start-menus.png`, which first was too
  (the golden scene copies a period Windows screen, with Internet Explorer, Outlook Express and a
  plain folder for Windows Explorer, so the owner asked why it showed what Bevel95 does not have)
  and since 2026-11-18 is drawn by the ignored test `draw_readme_start_menus` in
  `core/tests/golden.rs`: the three looks, task buttons of Bevel95's own programs with their
  pictures, the Programs menu put together as the bar does for "Bevel95's own programs" (the
  table `bevel_core::programs::OWN_PROGRAMS` and `add_own_programs`, moved into the core from
  `start_menu.rs` for this, unchanged), opened down to Games. Run it again with `cargo test -p
  bevel-core --test golden -- --ignored --exact draw_readme_start_menus`; it is not a golden
  image and no test compares it. Also CONTRIBUTING.md, `docs/roadmap.md`, the bug report form
  (`.github/ISSUE_TEMPLATE/bug_report.yml`) and a build-and-test workflow
  (`.github/workflows/test.yml`, untried). The owner's checklists moved unchanged to
  `private/checklists.md`. The art guard allows `docs/img/`.
- **What is not public, and why**: the maintainer's working instructions for their AI helper
  (`CLAUDE.local.md`: paths of their PC, preferences; the public `CLAUDE.md` is a short version
  for contributors' AI helpers), their running list and checklists (`private/todo.md`,
  `private/checklists.md`: questions to the maintainer, what was tried on their PC; the public
  side has `docs/roadmap.md`), the inventory of Windows' pictures (`private/art-inventory.md`,
  since 2026-11-18, above). Everything else was repaired in place so that one text serves both:
  tests that used a PC's user name and profile path, a packaged app's ID, program names in
  comments and here, the dev scripts' build path (now asked of cargo), `install.ps1`'s guard (any
  packaged app's private folder, not one app's). `.cargo/config.toml` keeps only the static C
  runtime and the aliases; the build goes to cargo's own `target`.
- **The private material** (the reference captures, the Reference PCs, the discs) is found
  through one setting, the environment variable **`BEVEL95_PRIVATE`**, default this checkout's
  untracked `private` folder, with the Reference PCs and the captures in its `reference` folder
  (`reference/refs`, `reference/vm`, `reference/qemu`, `reference/iso`): `cargo compare-refs`
  (`BEVEL95_REFS` still wins for the captures alone), `tools/vm.ps1`, `tools/fetch-refs.ps1`,
  `tools/dev/extract-caps.ps1`; each says plainly what is missing. The picture cutters take their
  inputs as arguments. `cargo test --workspace` needs none of it.
- **One folder since 2026-11-18**: the working folder is the public project; `.gitignore` keeps
  `/private/` and `/CLAUDE.local.md` out of git, and nothing is exported any more (the first
  version made a scanned copy with `tools/export.ps1`; its last run made this repository's first
  state). Before a push, **`tools\check-public.ps1`** scans every tracked file for every kind in
  the private word list (`private/export-private-words.txt`, `kind|regex` per line, never
  tracked: a PC's user name and profile paths, the project's path, the maintainer's name and
  e-mail, the computer's and the Wi-Fi networks' names, the discs' names, the shapes of product
  keys, emulator command lines with discs, an AI app's private folders) and lists every author and
  committer of the commits not yet pushed; it fails on a private word (kind, file, line; never the
  text) and on any name or address but the public identity's. It changes nothing and contacts
  nothing. Without the word list it stops.

## Adding Help to a program

`programs/bevel-help.exe` displays the catalog supplied by the calling program. Keep
authored articles, headings, links and keywords in the app's source. Cargo links the
shared `bevel-core` and `bevel-winkit` Rust crates into each executable that uses them.
Changes to shared drawing code reach each program when it is rebuilt. A fix confined
to the viewer reaches all callers when the bundled `bevel-help.exe` is updated. Each
caller opens its own Help session.

Both games supply their topics through the same catalog interface. In the Windows 95
look, the viewer chooses the default article width from the monitor, measures the
wrapped topic and caps its height at the space below its opening position. This lets
an app add a short or long topic without specifying a window size. See the
[WinHelp measurements](theme-spec.md#help-viewer).

Build a `bevel_core::help::Catalog` and keep a `bevel_winkit::help::HelpViewer::default()`
in the calling window's state. An editor with a single topic could define its book this way.

```rust
use bevel_core::help::{Catalog, Entry, EntryIcon, Keyword, Style, Topic};

fn editor_help() -> Catalog {
    Catalog {
        title: "Editor Help".into(),
        home: 0,
        topics: vec![Topic::from_paragraphs(
            "Saving a file",
            &[(Style::Body, "Choose File > Save As, enter a name, then click Save.")],
        )],
        contents: vec![Entry {
            label: "Saving a file".into(),
            depth: 0,
            topic: Some(0),
            icon: EntryIcon::Page,
        }],
        index: vec![Keyword { label: "save".into(), topic: 0 }],
    }
}
```

In the Help menu handler, call
`viewer.show(hwnd, era, scale, editor_help(), bevel_winkit::help::Target::Contents)`
and handle its `std::io::Result<()>`. Pass the calling window handle (or a null handle
for an independent window), its `Era` and its integer scale from 1 to 8. Use `Target::Index`
or `Target::Search` for a navigation tab, `Target::Topic(0)` for an article and
`Target::UsingHelp` for the viewer's own guide in the same session. Topic numbers are
zero-based positions in `topics`, including the home topic and all link destinations.
Contents entries are in display order, with depth zero at the root.

The client starts the viewer and writes the catalog to its standard-input pipe.
Another call with the same owner, catalog, era and scale navigates that open viewer.
Dropping the client or calling `viewer.close()` closes the pipe and ends the session.
The viewer uses the supplied book for navigation, searching and printing. The protocol
and client implementation are in [wire.rs](../core/src/help/wire.rs) and
[help.rs](../platform/winkit/src/help.rs). For a working caller, see
[Minesweeper's catalogs](../core/src/minefield/html_help_catalog.rs) and its
[Help menu handler](../programs/minefield/src/main.rs).

//! The bar window: a borderless, always-on-top strip along the bottom of the main screen.
//! It reserves its space like the real taskbar (an appbar), never takes the keyboard focus
//! from your apps, and paints itself with `bevel_core` at 1×, scaled up by a whole number.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bevel_core::bar_menu::{self, BarCommand};
use bevel_core::config::{self, Config, QuickSearch, StartOptions, TaskbarOptions, WindowsKey};
use bevel_core::datetime::Moment;
use bevel_core::desktop::DesktopCommand;
use bevel_core::dialog::DialogKind;
use bevel_core::fb::{Framebuffer, Icon, Rect};
use bevel_core::flyout::{self, Flyout};
use bevel_core::layout::BarLayout;
use bevel_core::paint::{self, Bar, ScrollArrow, Task, TaskState};
use bevel_core::programs::{Folder, Shown};
use bevel_core::properties::Page;
use bevel_core::scale::Scale;
use bevel_core::start::{self, StartEntry};
use bevel_core::tasks::{self, ButtonPress, Click, ClickState, Group, Motion, Release, TaskList, WindowInfo};
use bevel_core::text::{Font, Fonts};
use bevel_core::theme::{self, Depth, Era, Theme};
use bevel_core::window_icons;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SYSTEMTIME, WPARAM,
};
use windows_sys::Win32::Globalization::{DATE_LONGDATE, GetDateFormatEx, GetTimeFormatEx, TIME_NOSECONDS};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, InvalidateRect, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
    PAINTSTRUCT, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI,
    SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetCapture, GetDoubleClickTime, MOD_CONTROL, MOD_NOREPEAT, MOD_WIN, RegisterHotKey, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT,
    TrackMouseEvent, UnregisterHotKey,
};
use windows_sys::Win32::UI::Shell::{ABN_FULLSCREENAPP, ABN_POSCHANGED};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DeregisterShellHookWindow, DestroyWindow, DispatchMessageW,
    GetCursorPos, GetMessageW, HSHELL_HIGHBIT, HSHELL_REDRAW, HTCLIENT, WM_SETCURSOR,
    HSHELL_WINDOWACTIVATED, HSHELL_WINDOWCREATED, HSHELL_WINDOWDESTROYED, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOP,
    HWND_TOPMOST, IDC_ARROW,
    KillTimer, LoadCursorW, MA_NOACTIVATE, MSG, PostMessageW, PostQuitMessage, RegisterClassExW,
    RegisterShellHookWindow, RegisterWindowMessageW, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SM_CXDRAG, SetTimer, SetWindowPos, ShowWindow, TranslateMessage, WM_APP, WM_CAPTURECHANGED, WM_DESTROY, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_ENDSESSION, WM_ERASEBKGND, WM_HOTKEY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_PAINT, WM_QUERYENDSESSION, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETTINGCHANGE,
    WM_TIMER, WM_WINDOWPOSCHANGED, WNDCLASSEXW, WNDPROC, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::fullscreen::{Fullscreen, HSHELL_FULLSCREENENTER, HSHELL_FULLSCREENEXIT};
use crate::log::{data_dir, log, log_win32};
use bevel_winkit::state_file;
use crate::util::{Private, blit, point, rect_h, rect_w, wide};
use crate::{
    about_window, appbar, apps, audio, autostart, datetime_window, desktop, dialogs, explorer, find_window, font, keys, kit, own_icon, popup,
    properties_window, quick_launch, start_menu, status, tip, tray, volume_popup, watchdog, window_list, zones,
};

/// Bevel95's own notification icons, the network connection and the battery (Windows 11 hands
/// out neither): keys no program's icon has (no window handle is negative).
pub const NETWORK_KEY: tray::Key = (-95, 1);
pub const POWER_KEY: tray::Key = (-95, 2);
/// How often (in ticks) the connection's counters are looked at: the screens' blink rate.
const NETWORK_TICKS: u32 = 2;
/// Windows' own Display settings (the screen's resolution and scale): what Properties on the
/// desktop's menu opens, where Windows 95 to XP had their Display Properties.
pub const WINDOWS_DISPLAY_SETTINGS: &str = "ms-settings:display";

/// Not in windows-sys; WinUser.h defines them as HSHELL_REDRAW | HSHELL_HIGHBIT and
/// HSHELL_WINDOWACTIVATED | HSHELL_HIGHBIT.
const HSHELL_FLASH: u32 = HSHELL_REDRAW | HSHELL_HIGHBIT;
const HSHELL_RUDEAPPACTIVATED: u32 = HSHELL_WINDOWACTIVATED | HSHELL_HIGHBIT;
/// Every this many ticks (2 s) the window list is re-read in full, in case a change
/// notification was missed.
const REFRESH_TICKS: u32 = 20;

/// Window class of the bar; `--restore` finds a running bar by it.
pub const BAR_CLASS: &str = "Bevel95Bar";
const TICK: usize = 1;
/// How often the bar checks the clock, the screen, fullscreen apps and the Windows
/// taskbar (which can pop back up; RetroBar's library checks every 100 ms too).
const TICK_MS: u32 = 100;
const HOTKEY_TOGGLE: i32 = 1;
/// The classic desktop on and off (`bar_menu::DESKTOP_KEY`).
const HOTKEY_DESKTOP: i32 = 2;
// The bar's own messages that do or change something are registered by name (`Private`: a number
// Windows gives every program of the session that asks by the same name), as the desktop's and
// the folder windows' are: a fixed `WM_APP + n` could be hit by another program's stray broadcast,
// which could close the bar, restart it, switch the look or the size and save it (the fourth
// safety review of 2026-11-18, C3). Their names and old numbers are listed in decisions.md
// ("Desktop icons", the registered messages); `tools\dev\postbar.ps1` posts one by its name.
/// Appbar notifications from the shell (the callback message given to SHAppBarMessage).
static WM_APPBAR: Private = Private::new("Bevel95.Bar.AppBar");
/// "Paint again later" (when a paint arrived while the state was busy). Only paints: a number.
const WM_REPAINT: u32 = WM_APP + 2;
/// A menu entry was chosen: wParam = its `BarCommand::code`, lParam = 1 if Shift was held.
pub static WM_BAR_COMMAND: Private = Private::new("Bevel95.Bar.Command");
/// A size was chosen in the Size submenu: the new size is wParam / lParam, lParam 0 (a whole
/// size, as before quarter sizes came: `tools\dev\postbar.ps1 Size 3`) or 4 (wParam in quarters:
/// 7 is 1.75). `size_of_message` reads it. A size between whole numbers asks first whether to
/// keep it (`choose_size`); the question's answer comes back as lParam 8 (keep) or 9 (back),
/// wParam the question's ticket.
pub static WM_BAR_SIZE: Private = Private::new("Bevel95.Bar.Size");
/// A Start menu entry without a submenu was chosen (Run, Shut Down, ...): wParam = its index.
pub static WM_START_ACTION: Private = Private::new("Bevel95.Bar.StartAction");
/// A look was chosen in the Theme submenu: wParam = its index in `Era::ALL`.
pub static WM_BAR_THEME: Private = Private::new("Bevel95.Bar.Theme");
/// Open the Taskbar Properties window (the Start menu's Settings > Taskbar... entry).
pub static WM_OPEN_PROPERTIES: Private = Private::new("Bevel95.Bar.OpenProperties");
/// The volume or mute may have changed (Windows' volume icon changed, or our volume control
/// was used): the speaker's picture is brought up to date. Only reads and paints: a number.
pub const WM_VOLUME_CHANGED: u32 = WM_APP + 9;
/// A command of the classic desktop (its menus, a key on its icons) for the bar to carry out:
/// wParam = its `DesktopCommand::code`, lParam `desktop::command_param` (the icons' ticket, by
/// the keyboard). Registered by name, like the desktop's own messages (the third safety review of
/// 2026-11-18, C4: `WM_APP + 10` could be hit by another program's broadcast).
static WM_DESKTOP_COMMAND: crate::util::Private = crate::util::Private::new("Bevel95.Desktop.Command");

/// The message `WM_DESKTOP_COMMAND`.
pub fn wm_desktop_command() -> u32 {
    WM_DESKTOP_COMMAND.id()
}
/// A Create Shortcut wizard started by Taskbar Properties' Add... has ended (`add_shortcut`).
pub static WM_SHORTCUT_WIZARD_ENDED: Private = Private::new("Bevel95.Bar.ShortcutWizardEnded");
/// Sent by a live test (`private/probes/drag/smooth.ps1`): whose task button is at the point
/// (wParam, lParam) in the bar window (screen pixels from its corner)? Answers with the handle
/// of the button's first window, or 0 if no button is there. It only answers (it does and changes
/// nothing), so it keeps its number.
/// (WM_APP + 12 is Quick Search's programs read, which only refreshes its list.)
pub const WM_TASK_AT: u32 = WM_APP + 16;
/// The colors were chosen (the Colors submenu, Quick Search, a live test): wParam = its index
/// in `Depth::ALL` (0: 16 Colors, 1: 256 Colors, 2: True Color; before 256 Colors came, 1 was
/// True Color).
pub static WM_BAR_COLORS: Private = Private::new("Bevel95.Bar.Colors");
/// Quick Launch, for live tests (`tools\dev\postbar.ps1 QuickLaunch 100`): wParam 100 has the
/// folder read again (the band's Refresh); 101 opens Windows 2000's list of the icons left out
/// (where none is left out, a list of every icon of the band: the list window's leak check) and
/// 102 closes it; any other wParam does nothing. It never clicks an icon or a row:
/// those start the person's real programs and act on their windows, and a test command acts on
/// test items only (the review before 0.1.2, A1).
pub static WM_QUICK_LAUNCH: Private = Private::new("Bevel95.Bar.QuickLaunch");
/// Display Properties for live tests (`display_window::test_message`): opens and closes it and the
/// Pattern window, and shows a test's wallpaper or pattern on the desktop without saving it.
pub static WM_DISPLAY: Private = Private::new("Bevel95.Bar.Display");
/// A live test's long menu (brief 92, `private\probes\start-long\live.ps1`): opens a menu of made-up
/// entries and folders ("Entry 000" ...; wParam: how many entries, 1 to 500) laid out and placed by
/// the look's own rule as the Programs menu beside the Start menu would be. Its entries start
/// nothing: choosing one closes the menu and writes a line to the log. A test command for test
/// items only (`popup::open_test_long_menu`); it had no `WM_APP` number before.
pub static WM_TEST_LONG_MENU: Private = Private::new("Bevel95.Bar.TestLongMenu");
/// Monitor Lines chosen (the Bevel95 menus' Monitor Lines, Quick Search): wParam its index in
/// `monitor_lines::Strength::ALL` (0 Off, 1 Light, 2 Medium, 3 Strong), put on and saved.
pub static WM_BAR_MONITOR_LINES: Private = Private::new("Bevel95.Bar.MonitorLines");
/// A live test's Monitor Lines (brief 102, `private\probes\monitor-lines\live.ps1`): wParam 1 to 3
/// shows Light, Medium or Strong for at most `MONITOR_LINES_TEST_SECS` seconds WITHOUT saving
/// anything (settings.txt is not touched); 0 ends it at once. Answers the lines' window (0: none).
pub static WM_TEST_MONITOR_LINES: Private = Private::new("Bevel95.Bar.TestMonitorLines");
/// A live test's lines end by themselves this long after its FIRST message, whatever comes after
/// (brief 104, N5); a new test can begin only `MONITOR_LINES_TEST_GAP_SECS` after a test's first
/// message, so no stream of messages keeps them up.
const MONITOR_LINES_TEST_SECS: u64 = 20;
const MONITOR_LINES_TEST_GAP_SECS: u64 = 60;
/// The project's own screen-photographing helpers (`tools\dev\lines-pause.ps1`, brief 104): wParam 1
/// takes the lines away for at most `MONITOR_LINES_PAUSE_SECS` seconds, 0 ends the pause at once.
/// Nothing is saved. Answers 1 if the lines' window was there or a pause was on, else 0.
pub static WM_PAUSE_MONITOR_LINES: Private = Private::new("Bevel95.Bar.PauseMonitorLines");
const MONITOR_LINES_PAUSE_SECS: u64 = 60;

/// Every message of the bar registered by name, with the `WM_APP + n` it was before (the names
/// `tools\dev\postbar.ps1` knows and decisions.md lists).
pub static BAR_MESSAGES: [(&Private, u32); 13] = [
    (&WM_APPBAR, 1),
    (&WM_BAR_COMMAND, 3),
    (&WM_BAR_SIZE, 4),
    (&WM_START_ACTION, 5),
    (&keys::WM_START_KEY, 6),
    (&WM_BAR_THEME, 7),
    (&WM_OPEN_PROPERTIES, 8),
    (&WM_SHORTCUT_WIZARD_ENDED, 11),
    (&keys::WM_DRAG_ESC, 13),
    (&WM_BAR_COLORS, 40),
    (&own_icon::WM_OWN_ICON, 41),
    (&WM_QUICK_LAUNCH, 42),
    (&WM_DISPLAY, 43),
];

/// Registers them all at the start, so that no thread (the keyboard hook's least of all) asks
/// Windows for a number in the middle of its work.
fn register_messages() {
    for (message, _) in &BAR_MESSAGES {
        message.id();
    }
    // The bar's own, posted by its threads only (never by a live test).
    crate::pictures::WM_PICTURE.id();
    crate::pictures::WM_PICTURE_AGAIN.id();
    crate::wallpaper_browse::WM_BROWSED.id();
    WM_TEST_LONG_MENU.id();
    WM_BAR_MONITOR_LINES.id();
    WM_TEST_MONITOR_LINES.id();
    WM_PAUSE_MONITOR_LINES.id();
    bevel_winkit::on_top::WM_RAISE_LINES.id();
}
/// The timer that slides an auto-hiding bar in and out, and its step.
const SLIDE: usize = 2;
const SLIDE_MS: u32 = 15;
/// How many ticks (100 ms each) the pointer must stay off an auto-hiding bar before it slides
/// away.
const HIDE_AFTER_TICKS: u32 = 5;
/// Rows of an auto-hidden bar left on the screen (Windows 2000 in the v86 emulator: its top
/// edge, 2px).
const HIDDEN_ROWS: i32 = 2;
/// Timers that show a tooltip once the pointer has rested, and take it away again.
const TIP_SHOW: usize = 3;
const TIP_HIDE: usize = 4;
/// The timer that repaints the task buttons while one is dragged or they slide (and only
/// then), and its step: 15 ms, about one frame of a 60 Hz screen (Windows rounds it up to its
/// timer's pace, 15.6 ms unless a program asked for finer). The pointer's moves are painted
/// as they come besides (`tasks::Motion`; "Task buttons: grouping and dragging" in decisions.md).
const MOTION: usize = 5;
const MOTION_MS: u32 = 15;
/// What the pointer rests on, for tooltips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hover {
    Tray(tray::Key),
    Clock,
    /// A Quick Launch icon (its place on the band).
    Quick(usize),
}

/// The bar window, for the panic hook (which must not touch the app state).
pub static BAR_HWND: AtomicUsize = AtomicUsize::new(0);
/// Whether `bar_menu::DESKTOP_KEY` was registered at the start (another program may hold it):
/// the menus show it beside their desktop entry only then.
static DESKTOP_KEY: AtomicBool = AtomicBool::new(false);
/// This start fell back to the classic style after a failure (`failsafe`): it says so once.
static TELL_FAILURE: AtomicBool = AtomicBool::new(false);
/// The "TaskbarCreated" message Explorer broadcasts when it (re)starts.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
/// The "SHELLHOOK" message: windows opening, closing, activating, changing, flashing.
static SHELL_HOOK: AtomicU32 = AtomicU32::new(0);

pub struct App {
    pub era: Era,
    /// True Color, 256 Colors or 16 Colors (`colors` in settings.txt).
    pub colors: Depth,
    /// The look in these colors (`theme::theme_in(era, colors)`).
    pub theme: &'static Theme,
    regular: Font,
    bold: Font,
    /// Scale from the settings file for the bar style in use; `None` = automatic.
    scale_setting: Option<Scale>,
    /// Each bar style's own size as settings.txt keeps it (`config::scale_key`; the owner,
    /// 2026-11-25): a switch of style puts its own size on.
    style_scales: bevel_core::config::Config,
    /// How a size between whole numbers is drawn (the "scaling" setting).
    scaling: bevel_core::scale::Way,
    /// A size between whole numbers chosen in the Size menu or Quick Search, on but not saved
    /// while the look's question asks whether to keep it (`choose_size`).
    size_asking: bar_menu::SizeAsking,
    /// The size in use (a whole number, or a quarter step between: `bevel_core::scale`).
    pub scale: Scale,
    /// The main screen, in screen pixels.
    pub monitor: RECT,
    /// Where the bar was last placed.
    placed: RECT,
    dpi: u32,
    clock: String,
    /// Our bar is showing (false after "Switch back to Windows").
    ours: bool,
    /// The fullscreen app the bar stepped behind, if any (`fullscreen.rs`).
    fullscreen: Fullscreen,
    watchdog: Option<u32>,
    exit_code: u32,
    rehides: u32,
    ticks: u32,
    /// Open menus, root first.
    pub menus: Vec<popup::Popup>,
    /// The taskbar windows now, and the buttons they make.
    pub windows: Vec<WindowInfo>,
    task_list: TaskList,
    pub groups: Vec<Group>,
    programs: window_list::Programs,
    pub icons: HashMap<u64, Option<Icon>>,
    /// Under XP's and 7's styles: each window's own large picture at its own size
    /// (`window_list::window_big_icon`), for the live pictures above size 1 and 7's slots of a
    /// window whose program file has no picture (the owner, 2026-11-25).
    big_window_icons: HashMap<u64, Option<Icon>>,
    /// Windows that asked for attention and have not been activated since.
    flashing: HashSet<u64>,
    active: Option<u64>,
    /// The window in front at the last refresh (also when it is no taskbar window).
    front: u64,
    /// The window that was in front when the classic desktop was last pressed, while it still is
    /// (`tasks::still_left`): Windows did not let the desktop come forward, but the person left
    /// that program, so its button is not drawn pressed and a click on it brings it forward.
    left_for_desktop: Option<u64>,
    /// Where each long menu was last scrolled to, by `popup::scroll_key` (while the bar runs:
    /// Windows 98 reopened a menu where it was left, survey part 1).
    pub menu_scroll: HashMap<String, i32>,
    /// The program window in front as the left press on a task button saw it (`tasks::front_seen`),
    /// and the one Windows had in front at that press, for the click at its release.
    press_front: Option<(Option<u64>, Option<u64>)>,
    /// Where things were last drawn (1× pixels), to find what a click hit.
    layout: Option<BarLayout>,
    /// The task button the left mouse button went down on, and whether the pointer is
    /// still over it (then it is drawn pushed in, as in Windows 95).
    pressed: Option<usize>,
    pressed_inside: bool,
    /// That press closed the pressed button's own list of windows: its click ends there.
    pressed_closed_list: bool,
    /// The press followed until the release: a click, or a drag that moves the button
    /// (`tasks::ButtonPress`; `pressed` is where the button is now).
    press: Option<ButtonPress>,
    /// Where a dragged button was when its drag began (for the log).
    drag_from: Option<usize>,
    /// The pointer across the bar (screen pixels) at the last move of a drag.
    drag_x: i32,
    /// Where the task buttons are drawn while one is dragged or they slide (`tasks::Motion`),
    /// and since when the MOTION timer runs, with how many frames were painted and how many
    /// times the timer came (`None`: at rest).
    motion: Motion,
    motion_on: Option<(Instant, u32, u32)>,
    /// The clock the motion counts its milliseconds on.
    epoch: Instant,
    /// The window a click last brought forward, and when (`check_activated`).
    pub activated: Option<(u64, Instant)>,
    /// The row of task buttons shown when they need several, and a scroll arrow held down.
    task_row: usize,
    scroll_pressed: Option<ScrollArrow>,
    /// The Programs folders, read when the Start menu opens.
    pub start_programs: Folder,
    /// Your Favorites (Windows 98's Start menu), read when the Start menu opens.
    pub start_favorites: Folder,
    /// The Taskbar Options and Windows 2000's Start Menu Settings in effect.
    pub options: TaskbarOptions,
    pub start_options: StartOptions,
    /// Bevel95's own desktop is switched on (it shows while our bar does: `desktop.rs`).
    pub desktop: bool,
    /// What the Programs menu shows (Bevel95 Settings, Programs page).
    pub programs_shown: Shown,
    /// What a lone press of the Windows key opens (Bevel95 Settings, Taskbar page).
    pub windows_key: WindowsKey,
    /// A program's windows share one task button (Bevel95 Settings, Taskbar page); the task
    /// list carries it out (`TaskList::group`).
    pub group: bool,
    /// What Quick Search lists (Bevel95 Settings, Taskbar page).
    pub quick_search: QuickSearch,
    /// The battery's percentage stands beside its icon (Bevel95 Settings, Taskbar page).
    pub battery_percent: bool,
    /// Properties on the desktop's menu opens Windows' own display settings (Bevel95 Settings,
    /// Desktop page), not Display Properties.
    pub windows_display_settings: bool,
    /// Quick Launch: whether it shows, its icons, the one under the pointer and the one held down.
    pub ql: quick_launch::State,
    /// Auto hide: how many pixels of the bar are on the screen now, how many it is sliding
    /// to, and how many ticks the pointer has been away from it.
    shown_px: i32,
    target_px: i32,
    away_ticks: u32,
    /// The widest time of day in the clock's format: the clock's part of the well is made that
    /// wide, so it keeps its width all day.
    clock_widest: String,
    /// The notification icons shown at the last paint, left to right.
    tray_keys: Vec<tray::Key>,
    /// Where the notification area's stand-in windows were last put (the bar, the well).
    stand_in: Option<(RECT, RECT)>,
    /// What the pointer is on, whether WM_MOUSELEAVE is armed, and whether a tooltip shows.
    hover: Option<Hover>,
    tracking: bool,
    tip_showing: bool,
    /// The notification icon a mouse button went down on (its release goes to it too).
    tray_pressed: Option<(tray::Key, tray::Button)>,
    /// MS Sans Serif, the old dialog font, which the volume control uses in every look.
    pub shell_font: Font,
    /// The speakers are muted (the speaker is drawn with the red "no" sign).
    volume_muted: bool,
    /// When a click on the speaker last opened the volume control: a second click within
    /// Windows' double-click time makes it a double-click (Windows' volume mixer).
    speaker_clicked: Option<u32>,
    /// The network connection and the battery, as their icons show them (`status.rs`), and
    /// when one of them was last clicked (a second click soon after is a double-click).
    network: bevel_core::status::Network,
    network_reader: status::NetworkReader,
    power: bevel_core::status::Power,
    status_clicked: Option<(tray::Key, u32)>,
    /// Seconds Bevel95's clock runs ahead of the PC's (Date/Time Properties sets it).
    pub clock_offset: i64,
    /// The zone Bevel95's clock keeps (`None`: the PC's) and whether it follows daylight
    /// saving time (Date/Time Properties' Time Zone page).
    pub clock_zone: Option<&'static str>,
    pub clock_dst: bool,
    /// When the clock was last clicked: a second click soon after opens Date/Time Properties.
    clock_clicked: Option<u32>,
    /// Windows asked whether the session may end (sign-out, restart, shut down): the Windows
    /// taskbar is back and must stay so until Windows says the session goes on after all.
    ending: bool,
    /// Monitor Lines as settings.txt has them (`monitor_lines`; brief 102), and a live test's
    /// strength in force for a few seconds instead, never saved (`WM_TEST_MONITOR_LINES`).
    pub monitor_lines: bevel_core::monitor_lines::Strength,
    monitor_lines_test: Option<(bevel_core::monitor_lines::Strength, Instant)>,
    /// When the last live test began (its first message): its cap and the gap count from it.
    monitor_lines_test_began: Option<Instant>,
    /// A screen-photographing helper's pause (`WM_PAUSE_MONITOR_LINES`): since when.
    monitor_lines_paused: Option<Instant>,
    /// Tabs in folder windows as settings.txt has them (`folder_tabs`, `folder_tabs_close`;
    /// brief 110): Bevel95 Settings' Folders page shows them.
    pub folder_tabs: (bevel_core::folder::tabs::Place, bevel_core::folder::tabs::CloseAt),
    /// How the taskbar and the Start menu are drawn (`bar_style`, brief 112): Windows XP's goes
    /// with the 2000 look (`era`) for everything else, always in True Color.
    pub bar_style: theme::BarStyle,
    /// The style settings.txt says (in the run after a failure the bar runs classic while this is
    /// still XP or 7: `theme::look_chosen`).
    pub saved_style: theme::BarStyle,
    /// settings.txt's `later_looks = yes` (read at the start; written by nobody): Windows XP and 7
    /// offered also while a classic look is in use (brief 115; `theme::later_looks_shown`).
    pub later_looks: bool,
}

impl App {
    /// Whether the bar is drawn in Windows XP's style (brief 112).
    pub fn xp(&self) -> bool {
        self.bar_style.is_later()
    }

    pub fn fonts(&self) -> Fonts<'_> {
        Fonts { regular: &self.regular, bold: &self.bold }
    }

    /// The folder tree a cascading Start submenu shows.
    pub fn start_tree(&self, tree: popup::Tree) -> &Folder {
        match tree {
            popup::Tree::Programs => &self.start_programs,
            popup::Tree::Favorites => &self.start_favorites,
        }
    }

    /// Whether Quick Launch shows now (settings.txt, else the look's default).
    pub fn quick_launch_on(&self) -> bool {
        // Not shown under Windows XP's bar style (XP shipped with it hidden; brief 112).
        !self.xp() && self.ql.on(self.era)
    }

    /// Quick Launch as settings.txt (or the look's default) has it, shown or not (Bevel95
    /// Settings' box; brief 112).
    pub fn quick_launch_setting(&self) -> bool {
        self.ql.on(self.era)
    }

    /// The top edge of the bar, in screen pixels.
    pub fn bar_top_on_screen(&self) -> i32 {
        self.placed.top
    }

    /// The Start button, in screen pixels.
    pub fn start_button_on_screen(&self) -> Option<Rect> {
        let b = self.layout.as_ref()?.start;
        let r = self.scale.rect(b);
        Some(Rect::new(self.placed.left + r.x, self.placed.top + r.y, r.w, r.h))
    }

    /// The Quick Launch band as last laid out (1x), while it shows.
    pub fn quick_band(&self) -> Option<&bevel_core::quick_launch::Layout> {
        self.layout.as_ref()?.quick.as_ref()
    }

    /// A rectangle of the bar (1x) on the screen.
    pub fn on_screen(&self, r: Rect) -> Rect {
        let r = self.scale.rect(r);
        Rect::new(self.placed.left + r.x, self.placed.top + r.y, r.w, r.h)
    }

    /// Screen position of task button `index`'s top-left corner (if it is on the row shown).
    pub fn button_position(&self, index: usize) -> Option<(i32, i32)> {
        let layout = self.layout.as_ref()?;
        let button = layout.tasks.get(index.checked_sub(layout.first_task)?)?;
        Some((self.placed.left + self.scale.at(button.x), self.placed.top + self.scale.at(button.y)))
    }

    /// A window's picture: the classic one Bevel95 has for it (Notepad's, Explorer's), else
    /// its own icon.
    pub fn window_icon(&self, id: u64) -> Option<Icon> {
        match self.windows.iter().find(|w| w.id == id).and_then(|w| w.classic) {
            Some(kind) => Some(window_icons::icon(self.era, kind)),
            None => self.icons.get(&id).cloned().flatten(),
        }
    }

    /// A button's large live picture: its first window's own large one (`big_window_icons`);
    /// none for a window Bevel95 shows a classic picture for (those stay enlarged).
    fn group_big_icon(&self, group: &Group) -> Option<Icon> {
        let first = *group.windows.first()?;
        if self.group_is_classic(group) {
            return None;
        }
        self.big_window_icons.get(&first).cloned().flatten()
    }

    /// Whether a button shows a classic picture Bevel95 has for its first window.
    fn group_is_classic(&self, group: &Group) -> bool {
        group.windows.first().is_some_and(|&first| self.windows.iter().find(|w| w.id == first).and_then(|w| w.classic).is_some())
    }

    /// A button's picture: its first window's, except that a button holding several of
    /// Explorer's windows shows a folder.
    fn group_icon(&self, group: &Group) -> Option<Icon> {
        let first = *group.windows.first()?;
        match self.windows.iter().find(|w| w.id == first).and_then(|w| w.classic) {
            Some(kind) if group.windows.len() > 1 => Some(window_icons::icon(self.era, kind.for_group())),
            _ => self.window_icon(first),
        }
    }
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` on the app state, if there is one and it is not already in use further up the
/// stack. (Windows calls window procedures re-entrantly, so Win32 calls that can send us
/// messages are never made while the state is borrowed; a busy state just means "skip".)
/// A skip is logged with the caller's place in the code (the code review of 2026-11-09: a
/// skipped handler was silent), the first 20 and then every 100th.
#[track_caller]
pub fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    let caller = std::panic::Location::caller();
    APP.with(|cell| match cell.try_borrow_mut() {
        Ok(mut app) => app.as_mut().map(f),
        Err(_) => {
            static SKIPS: AtomicU32 = AtomicU32::new(0);
            let n = SKIPS.fetch_add(1, Ordering::Relaxed) + 1;
            if n <= 20 || n.is_multiple_of(100) {
                log(&format!(
                    "the bar's state was busy: skipped what {}:{} wanted to do (skip {n})",
                    caller.file().rsplit(['\\', '/']).next().unwrap_or(""),
                    caller.line()
                ));
            }
            None
        }
    })
}

/// The name of the bar's place: one bar at a time holds it (`take_place`).
pub const PLACE_NAME: &str = "Local\\Bevel95.Bar";

/// The bar's place, held while this bar runs (a mutex handle; 0: not held).
static PLACE: AtomicUsize = AtomicUsize::new(0);
/// Set once this bar has given up its place (`give_up_place`): from then on this process is no
/// longer "the running Bevel95", only, perhaps, an old bar finishing a file job.
static GAVE_UP: AtomicBool = AtomicBool::new(false);

/// Takes the place `name` (a named mutex) for this process; false if another holds it.
fn take_place(name: &str) -> Option<usize> {
    let name = wide(name);
    // SAFETY: a null-terminated name; the handle is returned to the caller, who keeps or closes it.
    unsafe {
        let mutex = CreateMutexW(null(), 0, name.as_ptr());
        if mutex.is_null() {
            return None;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(mutex);
            return None;
        }
        Some(mutex as usize)
    }
}

/// Gives up the bar's place, so a new bar can start at once (the second safety review of
/// 2026-11-18, B1): called at the end of `on_destroy`, once the windows, the hotkeys, the keyboard
/// hook and the notification-area stand-in are gone and Windows' taskbar is back. What may still
/// run in this process afterwards is a file job finishing (`main::finish_file_jobs`).
fn give_up_place() {
    GAVE_UP.store(true, Ordering::SeqCst);
    let place = PLACE.swap(0, Ordering::SeqCst);
    if place != 0 {
        // SAFETY: the handle `run` kept; closing it frees the name for the next bar.
        unsafe { CloseHandle(place as windows_sys::Win32::Foundation::HANDLE) };
    }
}

/// Whether this bar has given up its place (see `give_up_place`).
pub fn gave_up_place() -> bool {
    GAVE_UP.load(Ordering::SeqCst)
}

/// The name of the mark a bar ending for a Restart puts up before its window goes: the watchdog,
/// which sees the window gone while the process may still finish a file job, starts the new bar
/// at once when it finds it (`watchdog::run`).
pub fn restart_mark_name(pid: u32) -> String {
    format!("Local\\Bevel95.Restart.{pid}")
}

/// Puts up this bar's restart mark (kept open until the process ends).
fn mark_restart() {
    let name = wide(&restart_mark_name(std::process::id()));
    // SAFETY: a null-terminated name; the handle stays open on purpose until the process ends.
    if unsafe { windows_sys::Win32::System::Threading::CreateEventW(null(), 1, 1, name.as_ptr()) }.is_null() {
        log_win32("CreateEventW(the restart mark)");
    }
}

/// Runs the bar until it exits; returns the process exit code.
pub fn run() -> u32 {
    crate::log::started();
    register_messages();
    // Each look's wallpaper and pattern, before the desktop first shows.
    crate::display_window::load();
    // SAFETY: plain Win32 calls.
    unsafe {
        // (Already set if Bevel95 Setup showed first: Windows then answers "access denied".)
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0
            && GetLastError() != windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED
        {
            log_win32("SetProcessDpiAwarenessContext");
        }
    }
    match take_place(PLACE_NAME) {
        Some(mutex) => PLACE.store(mutex, Ordering::SeqCst),
        None => {
            log("another Bevel95 bar is already running; this one exits");
            return 0;
        }
    }

    let mut settings = load_settings();
    let saved_style = settings.bar_style;
    // The way back after a failure (the review of brief 112): the bar stopped by an error in a
    // style other than the classic one last time: the classic style for this run, said at every
    // start until a look is chosen (the mark stays till then); settings.txt is left as it is. A
    // mark under the classic style is old: it goes.
    let (style, tell) = theme::style_after_failure(settings.bar_style, crate::failsafe::marked());
    if !tell {
        crate::failsafe::clear();
    }
    TELL_FAILURE.store(tell, Ordering::SeqCst);
    settings.bar_style = style;
    crate::failsafe::style_now(style);
    // The picture store of XP and 7 takes wishes only under those styles (brief 115).
    crate::seven_store::set_later(style.is_later());
    // Windows XP's bar style goes with the 2000 look for everything else (brief 112).
    if settings.bar_style.is_later() {
        settings.era = Era::Win2000;
    }
    zones::set_clock(settings.clock_zone, settings.clock_dst);
    let theme = theme::bar_theme(settings.era, settings.colors, settings.bar_style);
    start_menu::set_own_programs(!settings.bar_style.is_later());
    let regular = font::letters(theme.font_face, theme.font_px, false).with_period_glyphs();
    let bold = font::letters(theme.font_face, theme.font_px, true).with_period_glyphs();

    // SAFETY: the window procedures are valid for the life of the program; handles checked.
    let bar = unsafe {
        let instance = GetModuleHandleW(null());
        register_class(instance, BAR_CLASS, Some(bar_proc));
        register_class(instance, popup::MENU_CLASS, Some(popup::menu_proc));
        kit::ensure_class();
        kit::register::<dialogs::DialogHost>();
        register_class(instance, dialogs::DIM_CLASS, Some(dialogs::dim_proc));
        kit::register::<properties_window::PropertiesHost>();
        kit::register::<properties_window::SettingsHost>();
        kit::register::<crate::remove_window::RemoveHost>();
        kit::register::<datetime_window::DateTimeHost>();
        kit::register::<find_window::FindHost>();
        register_class(instance, tray::TRAY_CLASS, Some(tray::tray_proc));
        register_class(instance, tray::NOTIFY_CLASS, Some(tray::tray_proc));
        register_class(instance, tip::TIP_CLASS, Some(tip::tip_proc));
        register_class(instance, volume_popup::VOLUME_CLASS, Some(volume_popup::volume_proc));
        register_class(instance, desktop::DESKTOP_CLASS, Some(desktop::desktop_proc));
        let class = wide(BAR_CLASS);
        let title = wide("Bevel95");
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            null(),
        )
    };
    if bar.is_null() {
        log_win32("CreateWindowExW(bar)");
        return 1;
    }
    BAR_HWND.store(bar as usize, Ordering::SeqCst);

    // SAFETY: plain Win32 call.
    let watchdog = watchdog::start(unsafe { GetCurrentProcessId() }, bar);
    let created = wide("TaskbarCreated");
    // SAFETY: valid null-terminated string.
    TASKBAR_CREATED.store(unsafe { RegisterWindowMessageW(created.as_ptr()) }, Ordering::SeqCst);

    let clock_widest = widest_time(&regular);
    let shell_font = font::letters("MS Sans Serif", 11, false);
    audio::init();
    let volume_muted = audio::Volume::open().is_some_and(|v| v.muted());
    APP.with(|cell| {
        *cell.borrow_mut() = Some(App {
            era: settings.era,
            colors: settings.colors,
            theme,
            regular,
            bold,
            scale_setting: settings.scale_of(settings.bar_style),
            style_scales: settings,
            scaling: settings.scaling,
            size_asking: bar_menu::SizeAsking::default(),
            scale: Scale::ONE,
            monitor: RECT::default(),
            placed: RECT::default(),
            dpi: 96,
            clock: clock_text(settings.clock_offset),
            ours: false,
            fullscreen: Fullscreen::default(),
            watchdog,
            exit_code: 0,
            rehides: 0,
            ticks: 0,
            menus: Vec::new(),
            windows: Vec::new(),
            task_list: {
                // Windows 7 always combines a program's windows (brief 114).
                let mut list = TaskList::new(settings.group || settings.bar_style == theme::BarStyle::Seven);
                list.set_order(load_task_order());
                list
            },
            groups: Vec::new(),
            programs: window_list::Programs::default(),
            icons: HashMap::new(),
            big_window_icons: HashMap::new(),
            flashing: HashSet::new(),
            active: None,
            front: 0,
            left_for_desktop: None,
            menu_scroll: HashMap::new(),
            press_front: None,
            layout: None,
            pressed: None,
            pressed_inside: false,
            pressed_closed_list: false,
            press: None,
            drag_from: None,
            drag_x: 0,
            motion: Motion::default(),
            motion_on: None,
            epoch: Instant::now(),
            activated: None,
            task_row: 0,
            scroll_pressed: None,
            start_programs: Folder::default(),
            start_favorites: Folder::default(),
            options: settings.options,
            start_options: settings.start,
            desktop: settings.desktop,
            programs_shown: settings.programs,
            windows_key: settings.windows_key,
            group: settings.group,
            quick_search: settings.quick_search,
            battery_percent: settings.battery_percent,
            windows_display_settings: settings.windows_display_settings,
            ql: quick_launch::State::new(settings.quick_launch),
            shown_px: 0,
            target_px: 0,
            away_ticks: 0,
            clock_widest,
            tray_keys: Vec::new(),
            stand_in: None,
            hover: None,
            tracking: false,
            tip_showing: false,
            tray_pressed: None,
            shell_font,
            volume_muted,
            speaker_clicked: None,
            network: bevel_core::status::Network::default(),
            network_reader: status::NetworkReader::default(),
            power: status::power(),
            status_clicked: None,
            clock_offset: settings.clock_offset,
            clock_zone: settings.clock_zone,
            clock_dst: settings.clock_dst,
            clock_clicked: None,
            ending: false,
            monitor_lines: settings.monitor_lines,
            monitor_lines_test: None,
            monitor_lines_test_began: None,
            monitor_lines_paused: None,
            folder_tabs: (settings.folder_tabs, settings.folder_tabs_close),
            bar_style: settings.bar_style,
            saved_style,
            later_looks: settings.later_looks,
        });
    });
    // The bar's other windows (the kit's, a program's menus, the dragged picture) put the Monitor
    // Lines back above them as they come up (brief 102).
    bevel_winkit::on_top::set(crate::monitor_lines::raise);
    let scale = settings.scale_of(settings.bar_style).map_or_else(|| "auto".to_string(), |s| s.label());
    log(&format!("started: {} look in {}, scale setting {scale}, {:?}", theme.slug, settings.colors.title(), settings.options));

    let shell_hook = wide("SHELLHOOK");
    // SAFETY: plain Win32 calls on our own window.
    unsafe {
        if RegisterHotKey(bar, HOTKEY_TOGGLE, MOD_CONTROL | MOD_WIN | MOD_NOREPEAT, u32::from(b'T')) == 0 {
            log_win32("RegisterHotKey(Ctrl+Win+T)");
        }
        let key = bar_menu::DESKTOP_KEY;
        if RegisterHotKey(bar, HOTKEY_DESKTOP, MOD_CONTROL | MOD_WIN | MOD_NOREPEAT, u32::from(bar_menu::DESKTOP_KEY_LETTER)) == 0 {
            log_win32(&format!("RegisterHotKey({key}) (another program holds it? The menus show the desktop's entry without it)"));
        } else {
            DESKTOP_KEY.store(true, Ordering::SeqCst);
        }
        SHELL_HOOK.store(RegisterWindowMessageW(shell_hook.as_ptr()), Ordering::SeqCst);
        if RegisterShellHookWindow(bar) == 0 {
            log_win32("RegisterShellHookWindow (window changes will only show every 2 s)");
        }
    }
    keys::install(bar);
    show_ours(bar);
    // Windows' Quick Launch folder, read on a thread of its own (and again when it changes).
    quick_launch::start(bar);
    // SAFETY: plain Win32 call on our own window.
    if unsafe { SetTimer(bar, TICK, TICK_MS, None) } == 0 {
        log_win32("SetTimer");
    }
    // Windows' list of apps is read right away, on a thread of its own, so the Start menu and
    // the icons read ahead (3 s from now) have its Store-style apps.
    apps::refresh();
    start_menu::read_icons_ahead(settings.era);
    if TELL_FAILURE.load(Ordering::SeqCst) {
        log("the last bar stopped with an error in a later Windows' style: this run is classic (settings.txt is as it was)");
        crate::confirm_window::message("Bevel95", &crate::failsafe::message());
    }

    let mut msg = MSG::default();
    // SAFETY: the standard message loop.
    unsafe {
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    APP.with(|cell| cell.borrow_mut().take());
    msg.wParam as u32
}

/// # Safety
/// `proc_` must be a valid window procedure for the life of the program.
unsafe fn register_class(instance: *mut core::ffi::c_void, name: &str, proc_: WNDPROC) {
    let name = wide(name);
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: proc_,
        hInstance: instance,
        // SAFETY: loading a system cursor.
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        lpszClassName: name.as_ptr(),
        ..Default::default()
    };
    // SAFETY: a fully initialized class description (the name is copied by Windows).
    if unsafe { RegisterClassExW(&class) } == 0 {
        log_win32("RegisterClassExW");
    }
}

/// Reads %LOCALAPPDATA%\Bevel95\settings.txt, creating it with the defaults if missing. A file
/// that is there but cannot be read is kept as it is: the bar runs on the defaults, and each
/// later save of a setting is skipped while it cannot be read (`state_file`).
fn load_settings() -> Config {
    settings_at(&data_dir().join("settings.txt"))
}

fn settings_at(path: &std::path::Path) -> Config {
    let path = path.to_path_buf();
    let text = match state_file::load(&path) {
        state_file::Read::Text(text) => text,
        state_file::Read::Missing => {
            // Through the settings' own write: a file that came meanwhile and is not Bevel95's is
            // never written over (the third safety review of 2026-11-18, A1).
            if let Err(e) = change_settings(&path, str::to_string)
                && !state_file::already_logged(&e)
            {
                log(&format!("could not create {}: {e}", crate::log::path_kind(&path)));
            }
            config::DEFAULT_FILE.to_string()
        }
        state_file::Read::Unreadable(_) => config::DEFAULT_FILE.to_string(),
    };
    let (settings, warnings) = config::parse(&text);
    for warning in warnings {
        log(&format!("settings.txt {warning}"));
    }
    settings
}

/// The time of day and date in the zone Bevel95's clock keeps (normally the PC's own), before
/// Bevel95's offset.
pub fn real_now() -> Moment {
    datetime_window::to_moment(&zones::now())
}

/// Bevel95's own clock: the PC's time, `offset` seconds on (what Date/Time Properties set).
pub fn clock_now(offset: i64) -> Moment {
    Moment::from_seconds(real_now().to_seconds() + offset)
}

/// Bevel95's time in the user's short time format ("3:45 PM").
fn clock_text(offset: i64) -> String {
    time_text(&datetime_window::to_systemtime(clock_now(offset)))
}

/// Sets how far Bevel95's clock runs from the PC's, and saves it in settings.txt.
pub fn set_clock_offset(offset: i64) {
    let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
    with_app(|app| {
        app.clock_offset = offset;
        app.clock = clock_text(offset);
    });
    let path = data_dir().join("settings.txt");
    let value = offset.to_string();
    if let Err(e) = change_settings(&path, |text| config::with_setting(text, "clock_offset", &value))
        && !state_file::already_logged(&e)
    {
        log(&format!("could not save the clock setting to {}: {e}", crate::log::path_kind(&path)));
    }
    log(&format!("Bevel95's clock now runs {offset} s from the PC's"));
    properties_window::clock_offset_changed(offset);
    // SAFETY: our own window.
    unsafe { InvalidateRect(bar, null(), 0) };
}

/// Sets the zone Bevel95's clock keeps (`None`: the PC's) and whether it follows daylight
/// saving time, and saves both in settings.txt.
pub fn set_clock_zone(zone: Option<&'static str>, dst: bool) {
    let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
    zones::set_clock(zone, dst);
    with_app(|app| {
        app.clock_zone = zone;
        app.clock_dst = dst;
        app.clock = clock_text(app.clock_offset);
    });
    let path = data_dir().join("settings.txt");
    let changed = change_settings(&path, |text| {
        let text = config::with_setting(text, "clock_zone", zone.unwrap_or("pc"));
        config::with_setting(&text, "clock_dst", if dst { "yes" } else { "no" })
    });
    if let Err(e) = changed
        && !state_file::already_logged(&e)
    {
        log(&format!("could not save the clock's time zone to {}: {e}", crate::log::path_kind(&path)));
    }
    log(&format!("Bevel95's clock now keeps {} ({})", zone.unwrap_or("the PC's zone"), if dst { "with daylight saving" } else { "standard time all year" }));
    // SAFETY: our own window.
    unsafe { InvalidateRect(bar, null(), 0) };
}

/// Whether a click on the clock comes within Windows' double-click time of the last one
/// (then the two open Date/Time Properties). Either way the next click starts afresh.
fn clock_double_click() -> bool {
    let now = press_time();
    let earlier = with_app(|app| app.clock_clicked.replace(now)).flatten();
    let double = earlier.is_some_and(|at| within_double_click(at, now));
    if double {
        with_app(|app| app.clock_clicked = None);
    }
    double
}

/// A time in the user's short time format.
fn time_text(t: &SYSTEMTIME) -> String {
    let mut buf = [0u16; 64];
    // SAFETY: valid pointers; the buffer length is passed. A null locale name means
    // LOCALE_NAME_USER_DEFAULT.
    let n = unsafe { GetTimeFormatEx(null(), TIME_NOSECONDS, t, null(), buf.as_mut_ptr(), buf.len() as i32) };
    if n > 1 {
        // Some formats use no-break spaces, which a Windows 95 font does not have.
        String::from_utf16_lossy(&buf[..n as usize - 1]).replace(['\u{202F}', '\u{A0}'], " ")
    } else {
        format!("{}:{:02}", t.wHour, t.wMinute)
    }
}

/// The widest time of day in the user's format and `font` ("12:00 AM"): the clock keeps that
/// width all day, as Windows' did.
fn widest_time(font: &Font) -> String {
    let mut t = SYSTEMTIME::default();
    // SAFETY: a valid out-pointer (for the date part of the times formatted below).
    unsafe { GetLocalTime(&mut t) };
    (0..24 * 60)
        .map(|m| time_text(&SYSTEMTIME { wHour: (m / 60) as u16, wMinute: (m % 60) as u16, wSecond: 0, ..t }))
        .max_by_key(|text| font.measure(text))
        .unwrap_or_default()
}

/// Bevel95's date in the user's long date format ("Sunday, November 8, 2026"): the clock's
/// tooltip.
fn long_date(offset: i64) -> String {
    let mut buf = [0u16; 128];
    let day = datetime_window::to_systemtime(clock_now(offset));
    // SAFETY: the buffer length is passed; null means the user's locale and its format.
    let n = unsafe { GetDateFormatEx(null(), DATE_LONGDATE, &day, null(), buf.as_mut_ptr(), buf.len() as i32, null()) };
    if n > 1 { String::from_utf16_lossy(&buf[..n as usize - 1]).replace(['\u{202F}', '\u{A0}'], " ") } else { String::new() }
}

/// Bevel95's date in the user's short date format ("11/25/2026"): Windows 7's clock's second line
/// (brief 114).
fn short_date(offset: i64) -> String {
    let mut buf = [0u16; 64];
    let day = datetime_window::to_systemtime(clock_now(offset));
    // SAFETY: the buffer length is passed; null means the user's locale and its format.
    let n = unsafe {
        GetDateFormatEx(null(), windows_sys::Win32::Globalization::DATE_SHORTDATE, &day, null(), buf.as_mut_ptr(), buf.len() as i32, null())
    };
    if n > 1 { String::from_utf16_lossy(&buf[..n as usize - 1]).replace(['\u{202F}', '\u{A0}'], " ") } else { String::new() }
}

/// The main screen's rectangle and DPI.
fn primary_monitor() -> (RECT, u32) {
    // SAFETY: valid out-pointers.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(monitor, &mut info);
        let (mut dpi_x, mut dpi_y) = (96, 96);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        (info.rcMonitor, dpi_x)
    }
}

fn same_rect(a: &RECT, b: &RECT) -> bool {
    (a.left, a.top, a.right, a.bottom) == (b.left, b.top, b.right, b.bottom)
}

/// Shows our bar: hides the Windows taskbar, reserves the screen edge, appears.
fn show_ours(bar: HWND) {
    // Bevel95's own notification icon leaves Windows' taskbar first: once the stand-in below
    // is up, it would be taken for an icon of Bevel95's own notification area.
    own_icon::hide(bar);
    explorer::hide();
    appbar::register(bar, WM_APPBAR.id());
    with_app(|app| {
        app.ours = true;
        app.fullscreen = Fullscreen::default();
    });
    keys::set_active(true);
    reposition(bar);
    refresh(bar);
    // SAFETY: our own window.
    unsafe { ShowWindow(bar, SW_SHOWNA) };
    // The notification area: programs are asked for their icons once the bar is in place.
    if let Some(placed) = with_app(|app| app.placed) {
        tray::start(bar, placed);
    }
    sync_desktop();
    sync_lines();
}

/// Whether the classic desktop shows (switched on, and the bar is Bevel95's): `sync_desktop`'s
/// test.
pub fn classic_desktop_shows() -> bool {
    with_app(|app| app.ours && app.desktop && !app.xp()).unwrap_or(false)
}

/// Bevel95's own desktop shows while it is switched on and our bar is showing, in the color of
/// the look in use; otherwise it is taken away (Windows' own desktop shows). Never under XP's bar
/// style (brief 112: "no apps etc"; the `desktop` setting keeps its value for a classic look).
pub fn sync_desktop() {
    let Some(look) = with_app(|app| {
        (app.ours && app.desktop && !app.xp()).then(|| desktop::Look {
            era: app.era,
            theme: app.theme,
            font: app.regular.clone(),
            scale: app.scale,
            background: crate::display_window::choice(app.era),
            picture: None,
        })
    }) else {
        return;
    };
    // A picture to read (a JPEG, the person's own) is asked for on its own thread; the desktop
    // shows its color or pattern until it has come (`display_window::pictures_came`).
    let look = look.map(|mut l| {
        l.picture = crate::display_window::picture_for(l.era, &l.background);
        l
    });
    match look {
        Some(look) => desktop::show(look),
        None => {
            desktop::hide();
            // The desktop's picture is no longer wanted (read again when it shows again).
            crate::display_window::desktop_gone();
        }
    }
    // The desktop has been handed its picture: a large one is let go (`pictures::let_go_large`).
    crate::display_window::settle();
}

/// Switches Bevel95's desktop on or off and remembers it in settings.txt.
fn set_desktop(on: bool) {
    if with_app(|app| std::mem::replace(&mut app.desktop, on) == on).unwrap_or(true) {
        return;
    }
    save_setting("desktop", if on { "yes" } else { "no" });
    log(if on { "desktop: switched on" } else { "desktop: switched off (the bar's Bevel95 menu, its key or Bevel95 Settings brings it back)" });
    sync_desktop();
    properties_window::desktop_changed(on);
}

/// Whether `bar_menu::DESKTOP_KEY` is the bar's (the menus show it then).
pub fn desktop_key() -> bool {
    DESKTOP_KEY.load(Ordering::SeqCst)
}

/// `bar_menu::DESKTOP_KEY`: the classic desktop on if it is off, off if it is on, as the
/// Bevel95 submenu's entry. While the bar is switched to Windows' taskbar the key does nothing
/// (the classic desktop shows only with the bar, so a switch nobody sees would only surprise at
/// the way back).
fn toggle_desktop() {
    let Some((ours, on)) = with_app(|app| (app.ours, app.desktop)) else {
        return;
    };
    if !ours {
        log(&format!("{}: ignored while the Windows taskbar is shown", bar_menu::DESKTOP_KEY));
        return;
    }
    if xp_refuses("the classic desktop") {
        return;
    }
    popup::close(false);
    log(&format!("{}: the classic desktop {}", bar_menu::DESKTOP_KEY, if on { "off" } else { "on" }));
    set_desktop(!on);
}

/// Under Windows XP's or 7's bar style, a switch that shows nothing there but would change what a
/// classic look gets (Quick Launch, the classic desktop, 16 or 256 colors) does nothing, and the
/// log says why once per run (the review of brief 112, finding 4; the style named: the review of
/// brief 114, doubt 9). True if it was refused.
fn xp_refuses(what: &str) -> bool {
    static SAID: AtomicBool = AtomicBool::new(false);
    let Some(style) = with_app(|app| app.bar_style).filter(|s| s.is_later()) else {
        return false;
    };
    if !SAID.swap(true, Ordering::SeqCst) {
        let name = theme::look_title(Era::Win2000, style);
        log(&format!("{what}: ignored under the {name} style (it shows nothing there; choose a classic look first; said once)"));
    }
    true
}

/// Carries out an entry of the desktop's right-click menu or of the menu on an icon (`code`: its
/// `DesktopCommand::code`; `by_keyboard`: a key on the desktop asked, not a menu). The icon
/// commands act on `desktop::targets_of(ticket)`, the selection when that menu opened or that key
/// went down (the ticket travels with the message: a later click or key cannot replace it), and
/// only when the menu on those icons holds them (the look's own icons are never cut, copied,
/// deleted or renamed).
fn run_desktop_command(code: usize, by_keyboard: bool, ticket: u32) {
    // A live test's command (`desktop::WM_TEST_COMMAND`): only what acts on test items, and its
    // Paste and Undo take test items only (the third safety review of 2026-11-18, C4).
    if desktop::is_test_ticket(ticket) {
        let allowed = DesktopCommand::from_code(code).is_some_and(|c| desktop::test_command_allowed(c, !desktop::targets_of(ticket).is_empty()));
        if !allowed {
            log("desktop: a test's command was refused (it would not act on test items only)");
            return;
        }
        crate::file_ops::for_a_test(|| run_desktop_command_now(code, by_keyboard, ticket));
        return;
    }
    run_desktop_command_now(code, by_keyboard, ticket);
}

fn run_desktop_command_now(code: usize, by_keyboard: bool, ticket: u32) {
    use DesktopCommand::*;
    let command = DesktopCommand::from_code(code);
    let targets = desktop::targets_of(ticket);
    if let Some(c) = command
        && matches!(c, Open | Explore | Cut | Copy | CreateShortcut | Delete | Rename | ItemProperties | EmptyRecycleBin)
    {
        let kinds: Vec<_> = targets.iter().map(|t| t.kind).collect();
        let offered = bevel_core::desktop::item_entries(&kinds).contains(&Some(c));
        // Alt+Enter with nothing selected: the desktop's own Properties.
        if c == ItemProperties && targets.is_empty() {
            properties_window::open_settings_on(Page::Desktop);
            return;
        }
        if !offered || (c == EmptyRecycleBin && !desktop::recycle_full()) {
            return;
        }
    }
    let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
    match command {
        Some(Open) => targets.into_iter().for_each(|t| desktop::open_item(t.id)),
        Some(Explore) => crate::file_ops::explore(&targets),
        Some(ItemProperties) => crate::file_ops::properties(&targets),
        Some(Cut | Copy) => {
            let cut = command == Some(Cut);
            if let Some(seq) = crate::file_ops::put_on_clipboard(bar, &targets, cut) {
                let ids = if cut { targets.iter().map(|t| t.id.clone()).collect() } else { Vec::new() };
                desktop::set_cut(ids, seq);
            }
        }
        Some(Paste) => {
            if let Some(dir) = crate::desktop_items::desktop_folder() {
                crate::file_ops::paste(bar, dir);
            }
        }
        Some(PasteShortcut) => {
            if let Some(dir) = crate::desktop_items::desktop_folder() {
                crate::file_ops::paste_shortcuts(bar, dir, desktop::select_when_shown);
            }
        }
        Some(CreateShortcut) => {
            if let Some(dir) = crate::desktop_items::desktop_folder() {
                crate::file_ops::create_shortcuts(&targets, dir, desktop::select_when_shown);
            }
        }
        Some(Delete) => crate::confirm_window::ask_delete(targets, by_keyboard),
        Some(EmptyRecycleBin) => crate::confirm_window::ask_empty(by_keyboard),
        Some(Rename) => {
            if let Some(t) = targets.into_iter().next() {
                desktop::rename(t.id);
            }
        }
        Some(DesktopCommand::SwitchToWindowsDesktop) => set_desktop(false),
        // Windows' own Display settings, started as the Start menu's Settings entries start
        // Windows' pages (a failure is logged there). Bevel95 Settings stays on the menu's
        // Bevel95 entry.
        Some(DesktopCommand::Properties) => crate::display_window::open_from_desktop(),
        // Its Desktop page, where the desktop is switched on and off.
        Some(DesktopCommand::Settings) => properties_window::open_settings_on(Page::Desktop),
        Some(DesktopCommand::Arrange(by)) => desktop::arrange(by),
        Some(DesktopCommand::AutoArrange) => desktop::toggle_auto_arrange(),
        Some(DesktopCommand::LineUp) => desktop::line_up(),
        Some(DesktopCommand::Refresh) => crate::desktop_items::read_again(),
        Some(DesktopCommand::NewShortcut) => crate::add_shortcut::start_on_desktop(),
        Some(DesktopCommand::NewFolder) => desktop::new_item(true),
        Some(DesktopCommand::NewTextDocument) => desktop::new_item(false),
        Some(DesktopCommand::Undo) => crate::confirm_window::undo(),
        Some(DesktopCommand::QuickSearch) => open_find(),
        // Only a drive in a folder window has it (the desktop shows no drives).
        Some(DesktopCommand::UnlockDrive) | None => {}
    }
}

/// Re-reads the taskbar windows and rebuilds the buttons; repaints if anything changed.
/// Looking windows up talks to other programs, so it happens outside the app state.
fn refresh(bar: HWND) {
    let hwnds = window_list::taskbar_windows();
    let front = window_list::active_window() as u64;
    let Some((mut programs, known_icons, known_big, later)) = with_app(|app| {
        (
            std::mem::take(&mut app.programs),
            app.icons.keys().copied().collect::<HashSet<u64>>(),
            app.big_window_icons.keys().copied().collect::<HashSet<u64>>(),
            app.bar_style.is_later(),
        )
    }) else {
        return;
    };
    let mut infos: Vec<WindowInfo> = hwnds.iter().map(|&h| window_list::window_info(h, &mut programs)).collect();
    programs.keep_only(&hwnds);
    let new_icons: Vec<(u64, Option<Icon>)> = hwnds
        .iter()
        .filter(|&&h| !known_icons.contains(&(h as u64)))
        .map(|&h| (h as u64, window_list::icon(h, &programs.of(h).path)))
        .collect();
    // The windows' large pictures, under XP and 7 only (asked as the small ones are).
    let new_big: Vec<(u64, Option<Icon>)> = if later {
        hwnds.iter().filter(|&&h| !known_big.contains(&(h as u64))).map(|&h| (h as u64, window_list::window_big_icon(h))).collect()
    } else {
        Vec::new()
    };
    let changed = with_app(|app| {
        app.programs = programs;
        let icons_changed = !new_icons.is_empty() || !new_big.is_empty();
        app.icons.extend(new_icons);
        app.icons.retain(|id, _| infos.iter().any(|w| w.id == *id));
        app.big_window_icons.extend(new_big);
        app.big_window_icons.retain(|id, _| infos.iter().any(|w| w.id == *id));
        // The person left the program in front for the classic desktop: its button is not drawn
        // pressed while it stays in front (brief 90).
        app.left_for_desktop = tasks::still_left(app.left_for_desktop, front);
        let active = infos.iter().any(|w| w.id == front && app.left_for_desktop != Some(front)).then_some(front);
        if let Some(id) = active {
            app.flashing.remove(&id);
        }
        app.flashing.retain(|id| infos.iter().any(|w| w.id == *id));
        for w in &mut infos {
            w.flashing = app.flashing.contains(&w.id);
        }
        let groups = app.task_list.update(&infos, active);
        let changed = icons_changed || groups != app.groups || active != app.active;
        // Another window came to the front: if its button is on a row not shown, show that row
        // (not while a task button is held: a drag stays in its row).
        if active != app.active
            && app.press.is_none()
            && let (Some(id), Some(layout)) = (active, app.layout.as_ref())
            && let Some(index) = groups.iter().position(|g| g.windows.contains(&id))
        {
            app.task_row = bevel_core::layout::row_of(layout, index);
        }
        // A pressed button is found again by its windows (buttons may have come or gone); if
        // they are all gone, so is the press.
        let mut press_gone = false;
        if let Some(press) = &app.press {
            app.pressed = tasks::find(&groups, &press.windows);
            press_gone = app.pressed.is_none();
        }
        app.windows = infos;
        let regrouped = groups != app.groups;
        app.groups = groups;
        app.active = active;
        app.front = front;
        let dragging = app.press.as_ref().is_some_and(|p| p.dragging);
        (changed, press_gone, (dragging && regrouped && !press_gone).then_some(app.drag_x))
    })
    .unwrap_or((false, false, None));
    let (changed, press_gone, drag_at) = changed;
    if press_gone {
        log("task buttons: the pressed button's window closed; the press ends");
        end_press(bar, false);
    }
    if changed {
        // SAFETY: our own window.
        unsafe { InvalidateRect(bar, null(), 0) };
        // A window came or went during a drag: once the bar is laid out anew, the dragged
        // button takes the place its middle is over then.
        if let Some(x) = drag_at {
            // SAFETY: our own window; the paint brings `layout` up to date.
            unsafe { UpdateWindow(bar) };
            drag_move(bar, x);
        }
    }
}

/// The task button under a point in the bar window (screen pixels from its corner), as an
/// index into `App::groups`.
fn task_at(x: i32, y: i32) -> Option<usize> {
    with_app(|app| {
        let layout = app.layout.as_ref()?;
        let i = layout.tasks.iter().position(|r| r.contains(app.scale.old(x), app.scale.old(y)))?;
        Some(layout.first_task + i)
    })
    .flatten()
}

thread_local! {
    /// A pinned program's slot held down (its shortcut), until the release.
    static PINNED_PRESS: RefCell<Option<String>> = const { RefCell::new(None) };
    /// The last pinned program started, and when: the same one again within the double-click
    /// time is the second click of a double click, and starts nothing.
    static PINNED_STARTED: RefCell<Option<(String, std::time::Instant)>> = const { RefCell::new(None) };
}

/// Whether a release over pinned slot `link` at `now` starts its program (not the second click of
/// a double click on it).
fn pinned_start_due(link: &str, now: std::time::Instant) -> bool {
    // SAFETY: a plain query.
    let wait = std::time::Duration::from_millis(u64::from(unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() }));
    PINNED_STARTED.with(|s| {
        let mut s = s.borrow_mut();
        let due = bevel_core::seven_bar::start_due(s.as_ref().map(|(l, at)| (l.as_str(), *at)), link, now, wait);
        if due {
            *s = Some((link.to_string(), now));
        }
        due
    })
}

/// What a press on Windows 7's bar that is not on a running program's slot does (brief 114).
enum SevenExtra {
    ShowDesktop,
    /// A pinned program that is not running: its shortcut starts.
    Pinned(String),
}

/// Windows 7's Show desktop, or a pinned program's slot, under a point in the bar window.
fn seven_extra_at(x: i32, y: i32) -> Option<SevenExtra> {
    with_app(|app| {
        let layout = app.layout.as_ref()?;
        let (x, y) = (app.scale.old(x), app.scale.old(y));
        if layout.show_desktop.is_some_and(|r| r.contains(x, y)) {
            return Some(SevenExtra::ShowDesktop);
        }
        let k = layout.pinned.iter().position(|r| r.contains(x, y))?;
        app.ql.shortcuts.get(k).map(|s| SevenExtra::Pinned(s.path.display().to_string()))
    })
    .flatten()
}

/// The scroll arrow under a point in the bar window, if the arrows are showing.
fn scroll_arrow_at(x: i32, y: i32) -> Option<ScrollArrow> {
    with_app(|app| {
        let s = app.layout.as_ref()?.scroll?;
        let (x, y) = (app.scale.old(x), app.scale.old(y));
        if s.up.contains(x, y) {
            Some(ScrollArrow::Up)
        } else if s.down.contains(x, y) {
            Some(ScrollArrow::Down)
        } else {
            None
        }
    })
    .flatten()
}

/// Shows the previous or next row of task buttons (the layout keeps it within the rows).
fn scroll_tasks(bar: HWND, arrow: ScrollArrow) {
    with_app(|app| {
        app.task_row = match arrow {
            ScrollArrow::Up => app.task_row.saturating_sub(1),
            ScrollArrow::Down => app.task_row + 1,
        };
    });
    // SAFETY: our own window.
    unsafe { InvalidateRect(bar, null(), 0) };
}

/// Whether a point in the bar window (screen pixels from its corner) is on the Start button.
fn on_start_button(x: i32, y: i32) -> bool {
    with_app(|app| app.layout.as_ref().is_some_and(|l| l.start.contains(app.scale.old(x), app.scale.old(y)))).unwrap_or(false)
}

/// The notification icon under a point in the bar window (screen pixels from its corner).
fn tray_icon_at(x: i32, y: i32) -> Option<tray::Key> {
    with_app(|app| {
        let layout = app.layout.as_ref()?;
        let i = layout.tray_icon_at(app.theme, app.scale.old(x), app.scale.old(y))?;
        app.tray_keys.get(i).copied()
    })
    .flatten()
}

/// Whether a point in the bar window is on the clock.
fn on_clock(x: i32, y: i32) -> bool {
    with_app(|app| {
        app.layout.as_ref().is_some_and(|l| l.clock.w > 0 && l.clock.contains(app.scale.old(x), app.scale.old(y)))
    })
    .unwrap_or(false)
}

/// The Quick Launch icon under a point in the bar window (screen pixels from its corner), as its
/// place on the band.
fn quick_at(x: i32, y: i32) -> Option<usize> {
    with_app(|app| app.layout.as_ref()?.quick.as_ref()?.button_at(app.scale.old(x), app.scale.old(y))).flatten()
}

/// Whether a point in the bar window is on the Quick Launch band's own part (its lines, grips and
/// gaps), not on one of its icons: the band's menu opens there.
fn on_band(x: i32, y: i32) -> bool {
    with_app(|app| {
        let band = app.layout.as_ref()?.quick.as_ref()?;
        let (x, y) = (app.scale.old(x), app.scale.old(y));
        Some(band.rect.contains(x, y) && band.button_at(x, y).is_none())
    })
    .flatten()
    .unwrap_or(false)
}

/// The Quick Launch icon under the pointer rises (`None`: none); repainted when that changes.
fn set_quick_hot(bar: HWND, hot: Option<usize>) {
    if with_app(|app| std::mem::replace(&mut app.ql.hot, hot) != hot).unwrap_or(false) {
        // SAFETY: our own window.
        unsafe { InvalidateRect(bar, null(), 0) };
    }
}

/// What part of the Quick Launch band is under a point in the bar window (screen pixels from its
/// corner): an icon, an overflow control (98's arrows, 2000's "»"), the grip before the task buttons.
fn quick_part_at(x: i32, y: i32) -> Option<bevel_core::quick_launch::Part> {
    with_app(|app| app.layout.as_ref()?.quick.as_ref()?.part_at(app.scale.old(x), app.scale.old(y))).flatten()
}

/// The overflow control under the pointer is drawn with its ring (`None`: none); repainted when
/// that changes.
fn set_quick_hot_control(bar: HWND, hot: Option<bevel_core::quick_launch::Control>) {
    if with_app(|app| std::mem::replace(&mut app.ql.hot_control, hot) != hot).unwrap_or(false) {
        // SAFETY: our own window.
        unsafe { InvalidateRect(bar, null(), 0) };
    }
}

/// A press on the Quick Launch band's grip, its arrows or its "»" (brief 70): the grip starts a
/// drag of the band's width (the bar keeps the mouse until the release); an arrow of Windows 98
/// scrolls the icons by one at once and stays pushed in until the release; Windows 2000's "»"
/// opens the list of the icons left out (`list_was_open`: the press closed it, so it only closes).
/// True when the press was the band's.
fn press_quick_part(bar: HWND, x: i32, y: i32, list_was_open: bool) -> bool {
    use bevel_core::quick_launch::{self as ql, Control, Part};
    match quick_part_at(x, y) {
        Some(Part::Grip) => {
            with_app(|app| {
                let shown = app.layout.as_ref().and_then(|l| l.quick.as_ref()).map_or(0, |b| b.width);
                app.ql.sizing.press(app.scale.old(x), shown);
            });
            // SAFETY: our own window; capture lets us follow the pointer off the bar.
            unsafe { SetCapture(bar) };
            // A window holding the mouse gets no WM_SETCURSOR: the drag's pointer is set here.
            bevel_winkit::pointer::show(bevel_core::kit::Pointer::HeaderDivider);
            true
        }
        Some(Part::Control(c @ (Control::Left | Control::Right))) => {
            with_app(|app| {
                let Some(band) = app.layout.as_ref().and_then(|l| l.quick.as_ref()) else {
                    return;
                };
                app.ql.scroll = if c == Control::Left { ql::scroll_left(band.scroll) } else { ql::scroll_right(band.scroll, band.max_scroll) };
                app.ql.pressed_control = Some(c);
            });
            // SAFETY: our own window; capture lets us see the release even off the bar.
            unsafe {
                SetCapture(bar);
                InvalidateRect(bar, null(), 0);
            }
            true
        }
        Some(Part::Control(Control::Chevron)) => {
            if !list_was_open {
                popup::open_quick_list(bar, false);
            }
            // SAFETY: our own window.
            unsafe { InvalidateRect(bar, null(), 0) };
            true
        }
        _ => false,
    }
}

/// The pointer moved to `x` while the band's grip is held: the band's end follows it. True while
/// such a drag lasts.
fn drag_quick_width(bar: HWND, x: i32) -> bool {
    let Some(changed) = with_app(|app| {
        if !app.ql.sizing.dragging() {
            return None;
        }
        let band = app.layout.as_ref().and_then(|l| l.quick.as_ref())?;
        let (min, max) = (band.min_width, band.max_width);
        Some(app.ql.sizing.move_to(app.era, app.scale.old(x), min, max))
    })
    .flatten() else {
        return false;
    };
    bevel_winkit::pointer::show(bevel_core::kit::Pointer::HeaderDivider);
    if changed {
        hide_tip(bar);
        // SAFETY: our own window.
        unsafe { InvalidateRect(bar, null(), 0) };
    }
    true
}

/// A drag of the band's grip ends (the release, or the bar lost the mouse): the band keeps the
/// width reached, and it is remembered (quick-launch.txt) when the drag moved it. True when a
/// drag was going on.
fn end_quick_drag(bar: HWND, why: &str) -> bool {
    let Some(width) = with_app(|app| app.ql.sizing.dragging().then(|| app.ql.sizing.end())).flatten() else {
        return false;
    };
    if let Some(w) = width {
        let saved = quick_launch::save_width(w);
        log(&format!("quick launch: the band's grip was dragged ({why}); its width is now {w}{}", if saved { "" } else { " (not saved)" }));
    }
    // SAFETY: our own window.
    unsafe { InvalidateRect(bar, null(), 0) };
    true
}

/// The pointer for a point in the bar window (screen pixels from its corner): Windows' two-sided
/// arrow over the band's grip, its divider pointer while the grip is dragged (resize.md, 1: the
/// plain left-right arrow while hovering, the split-bar arrow while dragging). `None`: the bar's
/// own arrow.
fn quick_pointer(x: i32, y: i32) -> Option<bevel_core::kit::Pointer> {
    use bevel_core::kit::Pointer;
    if with_app(|app| app.ql.sizing.dragging()).unwrap_or(false) {
        return Some(Pointer::HeaderDivider);
    }
    (quick_part_at(x, y) == Some(bevel_core::quick_launch::Part::Grip)).then_some(Pointer::SizeWE)
}

/// A click on the Quick Launch icon `target` (the one pressed, still under the pointer at the
/// release): what it stands for starts (`quick_launch::start_item`).
fn click_quick(target: &quick_launch::Target) {
    let Some((browser, mail)) = with_app(|app| (app.ql.browser.clone(), app.ql.mail.clone())) else {
        return;
    };
    quick_launch::start_item(target, browser.as_deref(), mail.as_deref());
}

/// The left button came up: the mouse goes back if the bar still has it (a press the bar took it
/// for may have been ended meanwhile; it must never stay with the bar).
fn give_back_mouse(bar: HWND) {
    // SAFETY: plain calls; only our own window's capture is released.
    unsafe {
        if GetCapture() == bar {
            ReleaseCapture();
        }
    }
}

/// An entry of the menu on a Quick Launch icon, carried out on the icon it opened on, and only
/// while that icon is still on the band ("Remembered by what it is, never by where it is"): Open
/// starts it as a click does; Delete takes a built-in icon off the band (remembered in
/// quick-launch.txt) and puts the person's shortcut in the Recycle Bin after the look's own
/// question, as the desktop's Delete does; Properties is Windows' own window for the shortcut.
pub fn quick_item_command(target: &quick_launch::Target, entry: bevel_core::quick_launch::IconEntry) {
    use bevel_core::quick_launch::IconEntry;
    let Some((browser, mail)) = with_app(|app| app.ql.holds(target).then(|| (app.ql.browser.clone(), app.ql.mail.clone()))).flatten() else {
        log("quick launch: a menu's command was refused (its icon is no longer on the band)");
        return;
    };
    match (entry, target) {
        (IconEntry::Open, _) => quick_launch::start_item(target, browser.as_deref(), mail.as_deref()),
        // A built-in icon: the look's question first (its name in it); Yes takes it off the band
        // (quick-launch.txt), No does nothing. Switching Quick Launch off and on brings it back.
        (IconEntry::Delete, quick_launch::Target::Builtin(b)) => {
            let b = *b;
            let Some(name) = with_app(|app| b.tooltip(app.era).lines().next().unwrap_or_default().to_string()) else {
                return;
            };
            let question = bevel_core::delete_confirm::Question::RemoveFromBand(name);
            crate::confirm_window::ask_then(
                question,
                false,
                Box::new(move || {
                    let removed = quick_launch::take_off(b);
                    with_app(|app| app.ql.removed = removed);
                    let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
                    // SAFETY: our own window.
                    unsafe { InvalidateRect(bar, null(), 0) };
                }),
            );
        }
        (IconEntry::Delete, quick_launch::Target::File(path)) => quick_launch::delete_shortcut(path, false),
        (IconEntry::Properties, quick_launch::Target::File(path)) => quick_launch::shortcut_properties(path),
        (IconEntry::Properties, quick_launch::Target::Builtin(_)) => {}
    }
}

/// Switches Quick Launch on or off (Bevel95 Settings, the Toolbars menu, the band's Close) and
/// writes `quick_launch` into settings.txt: only then, so a file that never had the line keeps the
/// look's default.
pub fn set_quick_launch(on: bool) {
    let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
    let Some(was) = with_app(|app| {
        let was = app.quick_launch_on();
        app.ql.setting = Some(on);
        app.ql.hot = None;
        was
    }) else {
        return;
    };
    save_setting("quick_launch", if on { "yes" } else { "no" });
    if was != on {
        log(if on { "quick launch: switched on" } else { "quick launch: switched off" });
    }
    // Switched off and on again: every built-in icon taken off comes back (the way back after
    // Delete on one of them).
    if on && !was {
        quick_launch::put_back_all();
        with_app(|app| app.ql.removed.clear());
    }
    properties_window::quick_launch_changed(on);
    hide_tip(bar);
    // SAFETY: our own window.
    unsafe { InvalidateRect(bar, null(), 0) };
}

/// What a point in the bar window is on, for tooltips: a notification icon or the clock.
fn hover_at(x: i32, y: i32) -> Option<Hover> {
    if let Some(key) = tray_icon_at(x, y) {
        return Some(Hover::Tray(key));
    }
    if let Some(i) = quick_at(x, y) {
        return Some(Hover::Quick(i));
    }
    with_app(|app| {
        let layout = app.layout.as_ref()?;
        (layout.clock.w > 0 && layout.clock.contains(app.scale.old(x), app.scale.old(y))).then_some(Hover::Clock)
    })
    .flatten()
}

/// The pointer's position on the screen.
fn cursor() -> POINT {
    let mut at = POINT::default();
    // SAFETY: a valid out-pointer.
    unsafe { GetCursorPos(&mut at) };
    at
}

/// The pointer is now on `now` (`None`: on neither, or off the bar). An icon's program hears
/// of every move over its icon (programs refresh their tooltip text then) and, for newer ones,
/// of the pointer coming and going. A new target gets its tooltip after the usual pause:
/// Windows' double-click time, a fifth of it if another tooltip was just showing.
fn set_hover(bar: HWND, now: Option<Hover>) {
    let at = cursor();
    let Some((before, showing)) = with_app(|app| (std::mem::replace(&mut app.hover, now), app.tip_showing)) else {
        return;
    };
    // (The speaker is ours: Windows' volume icon hears nothing, or it would open its own panel.)
    let program = |hover: Option<Hover>| match hover {
        Some(Hover::Tray(key)) if !tray::is_volume(key) => Some(key),
        _ => None,
    };
    if let Some(key) = program(now) {
        tray::mouse_move(key, at);
    }
    if before == now {
        return;
    }
    if let Some(key) = program(before) {
        tray::hover(key, false, at);
    }
    if let Some(key) = program(now) {
        tray::hover(key, true, at);
    }
    hide_tip(bar);
    if now.is_some() {
        // SAFETY: a plain query and a timer on our own window.
        unsafe {
            let pause = GetDoubleClickTime();
            SetTimer(bar, TIP_SHOW, if showing { pause / 5 } else { pause }, None);
        }
    }
}

/// Takes the tooltip away (and forgets one about to show).
fn hide_tip(bar: HWND) {
    // SAFETY: our own window's timers (harmless if not running).
    unsafe {
        KillTimer(bar, TIP_SHOW);
        KillTimer(bar, TIP_HIDE);
    }
    if with_app(|app| std::mem::replace(&mut app.tip_showing, false)).unwrap_or(false) {
        tip::hide();
    }
}

/// The pointer has rested: the tooltip of what it is on (an icon's text, or for the clock the
/// date), on top of it, for ten double-click times (Windows' default).
fn show_tip(bar: HWND) {
    // SAFETY: our own window's timer.
    unsafe { KillTimer(bar, TIP_SHOW) };
    let Some((target, tool)) = with_app(|app| {
        let target = app.hover?;
        let layout = app.layout.as_ref()?;
        let r = match target {
            Hover::Tray(key) => layout.tray[app.tray_keys.iter().position(|k| *k == key)?],
            // Windows 98's clock tooltip sits 2 rows below the well's top (the reference PC).
            Hover::Clock => Rect::new(layout.clock.x, layout.well.y + 2, layout.clock.w, layout.well.h - 4),
            // An icon cut at the band's edge (98): the part that shows.
            Hover::Quick(i) => layout.quick.as_ref()?.visible_button(i)?,
        };
        let r = app.scale.rect(r);
        Some((target, Rect::new(app.placed.left + r.x, app.placed.top + r.y, r.w, r.h)))
    })
    .flatten() else {
        return;
    };
    let text = match target {
        // The speaker's tooltip, as Windows 2000 shows it.
        Hover::Tray(key) if tray::is_volume(key) => Some("Volume".to_string()),
        Hover::Tray(NETWORK_KEY) => {
            with_app(|app| bevel_core::status::network_tooltip(app.era, &app.network))
        }
        Hover::Tray(POWER_KEY) => with_app(|app| bevel_core::status::power_tooltip(app.era, &app.power)),
        Hover::Tray(key) => tray::tooltip_of(key),
        Hover::Clock => Some(long_date(with_app(|app| app.clock_offset).unwrap_or(0))),
        Hover::Quick(i) => with_app(|app| app.ql.tooltip(app.era, i)).flatten(),
    };
    let Some(text) = text.filter(|t| !t.is_empty()) else {
        return;
    };
    if matches!(target, Hover::Quick(_)) {
        tip::show_quick_launch(bar, &text, cursor(), tool);
    } else {
        tip::show(bar, &text, cursor(), tool);
    }
    with_app(|app| app.tip_showing = true);
    // SAFETY: a plain query and a timer on our own window.
    unsafe { SetTimer(bar, TIP_HIDE, GetDoubleClickTime() * 10, None) };
}

/// A mouse button went down on a notification icon: its program hears of it (the speaker is
/// ours). The pointer is not captured (as with RetroBar): a program that opens its menu right
/// away must get the mouse.
fn press_tray(bar: HWND, key: tray::Key, button: tray::Button) {
    hide_tip(bar);
    with_app(|app| app.tray_pressed = Some((key, button)));
    if !tray::is_volume(key) {
        tray::mouse_down(key, button, cursor());
    }
}

/// A press on a notification icon whose release never came (the pointer was let go off the
/// bar, which has no mouse capture then) must not take the next release, say a task button's.
fn forget_tray_press() {
    with_app(|app| app.tray_pressed = None);
}

/// A mouse button came up on the bar: if it went down on a notification icon and the pointer
/// is still on that icon, the icon's program hears of it (the speaker opens ours). True if it
/// was such a press.
fn release_tray(bar: HWND, button: tray::Button, x: i32, y: i32) -> bool {
    let Some((key, _)) = with_app(|app| app.tray_pressed.take_if(|p| p.1 == button)).flatten() else {
        return false;
    };
    if tray_icon_at(x, y) == Some(key) {
        if tray::is_volume(key) {
            speaker_click(bar, button);
        } else if key == NETWORK_KEY || key == POWER_KEY {
            status_click(bar, key, button);
        } else {
            tray::mouse_up(key, button, cursor());
        }
    }
    true
}

/// The speaker, clicked: as in Windows 95 to 2000, a click opens the volume control, a
/// double-click Windows' volume mixer, a right-click the speaker's menu. Windows' own taskbar
/// waited a double-click time before opening the volume control (to tell a click from a
/// double-click); ours opens it at once and leaves it for the mixer if a second click follows.
/// Windows 11's own panel for its volume icon is not opened.
fn speaker_click(bar: HWND, button: tray::Button) {
    match button {
        tray::Button::Left => {
            if speaker_double_click() {
                volume_popup::close(false);
                volume_popup::open_mixer();
            } else {
                with_app(|app| app.speaker_clicked = Some(press_time()));
                volume_popup::open(bar, cursor());
            }
        }
        tray::Button::Right => {
            let at = cursor();
            popup::open_speaker_menu(bar, at.x, at.y);
        }
        tray::Button::Middle => {}
    }
}

/// Bevel95's own network or battery icon, clicked. Network: a click opens Windows 11's own list
/// of Wi-Fi networks (the one its taskbar opens: where a phone's hotspot shows up), a
/// double-click its network status (Status, as in 95 to 2000). Battery: a double-click opens
/// Windows' battery settings (the Power Meter's place). A right-click gives each its menu.
fn status_click(bar: HWND, key: tray::Key, button: tray::Button) {
    let at = cursor();
    match button {
        tray::Button::Left => {
            let now = press_time();
            let double = with_app(|app| {
                let last = app.status_clicked.take();
                let double = last.is_some_and(|(k, when)| k == key && within_double_click(when, now));
                if !double {
                    app.status_clicked = Some((key, now));
                }
                double
            })
            .unwrap_or(false);
            match (key == NETWORK_KEY, double) {
                (true, false) => window_list::launch(status::WIFI_LIST),
                (true, true) => window_list::launch(status::NETWORK_STATUS),
                (false, true) => window_list::launch(status::POWER_METER),
                (false, false) => {}
            }
        }
        tray::Button::Right => popup::open_status_menu(bar, key == NETWORK_KEY, at.x, at.y),
        tray::Button::Middle => {}
    }
}

/// A mouse button went down on the bar: if one of Windows' own flyouts is in front (the Wi-Fi
/// list the network icon opens, quick settings, the notification center), the bar takes the
/// front for a moment, so Windows closes the flyout as it does when its taskbar is clicked
/// (ours never takes the front by itself, so the flyout stayed, and Windows refused to bring
/// any window forward meanwhile). Returns which flyout was in front, if one was.
pub fn close_flyout(bar: HWND) -> Option<Flyout> {
    let front = window_list::foreground();
    if front.is_null() {
        return None;
    }
    let kind = flyout::flyout(&window_list::class_name(front), &window_list::program_path(front))?;
    let how = crate::util::bring_forward(bar);
    let closed = window_list::foreground() != front;
    static TIMES: AtomicU32 = AtomicU32::new(0);
    let n = TIMES.fetch_add(1, Ordering::Relaxed) + 1;
    if !closed || n <= 3 || n.is_multiple_of(100) {
        log(&format!(
            "a press on the bar found Windows' {kind:?} flyout in front: {} ({how:?}, time {n})",
            if closed { "it lost the front" } else { "it is STILL in front" }
        ));
    }
    Some(kind)
}

/// Whether this click on the speaker comes within Windows' double-click time of the one that
/// opened the volume control (then the two make a double-click). Either way the next click
/// starts afresh.
fn speaker_double_click() -> bool {
    let now = press_time();
    with_app(|app| app.speaker_clicked.take().is_some_and(|at| within_double_click(at, now))).unwrap_or(false)
}

/// When the mouse message being handled happened (Windows' message time, as the desktop and the
/// kit windows tell double-clicks), not when the bar got to it: a bar held up (a slow Explorer, a
/// program being started) still tells a double-click from two clicks (brief 90). A press read
/// late is counted in the log.
fn press_time() -> u32 {
    // SAFETY: plain queries.
    let (at, now) = unsafe { (windows_sys::Win32::UI::WindowsAndMessaging::GetMessageTime() as u32, windows_sys::Win32::System::SystemInformation::GetTickCount()) };
    let late = now.wrapping_sub(at);
    if late > 250 && late < 60_000 {
        static LATE: AtomicU32 = AtomicU32::new(0);
        bevel_winkit::log::counted(&LATE, &format!("bar: a click was read {late} ms after it happened; told by its own time"));
    }
    at
}

/// Two clicks at message times `earlier` and `now` are a double-click (Windows' double-click time).
fn within_double_click(earlier: u32, now: u32) -> bool {
    // SAFETY: a plain query.
    now.wrapping_sub(earlier) <= unsafe { GetDoubleClickTime() }
}

/// Asks for a WM_MOUSELEAVE when the pointer leaves the bar (once per visit).
fn arm_leave(bar: HWND) {
    if with_app(|app| !std::mem::replace(&mut app.tracking, true)).unwrap_or(false) {
        let mut track =
            TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: bar, dwHoverTime: 0 };
        // SAFETY: a correctly sized struct for our own window.
        unsafe { TrackMouseEvent(&mut track) };
    }
}

/// Carries out an entry of Windows XP's Start menu that has no submenu (brief 112): each goes where
/// the classic Start menu's matching entry goes; the folders open in Windows' own Explorer (XP's
/// bar style has no folder windows of Bevel95's); Search is Quick Search; Log Off and Turn Off
/// Computer open the 2000 look's Log Off and Shut Down windows (PROVISIONAL).
pub fn xp_start_action(item: bevel_core::xp_start::Item) {
    use bevel_core::xp_start::{Item, Right};
    if let Some(place) = xp_start_place(item) {
        window_list::launch(&place);
        return;
    }
    match item {
        Item::Right(Right::Search) => open_find(),
        Item::Right(Right::Run) => dialogs::open(DialogKind::Run),
        Item::LogOff => dialogs::open(DialogKind::LogOff),
        Item::TurnOff => dialogs::open(DialogKind::ShutDown),
        // Places (`xp_start_place`), cascades and programs: opened or started by the menu itself.
        _ => {}
    }
}

/// The place an entry of Windows 7's Start menu opens (brief 114): as the XP menu's matching
/// entry; the user's name their profile folder; Default Programs Windows' own default apps page.
/// `None`: the search box, Shut down, a cascade or a program.
pub fn seven_start_place(item: bevel_core::seven_start::Item) -> Option<String> {
    use bevel_core::seven_start::{Item, Right};
    use bevel_core::xp_start::{Item as XpItem, Right as XpRight};
    let xp = |r: XpRight| xp_start_place(XpItem::Right(r));
    match item {
        Item::User => Some(bevel_winkit::shell::profile_folder().map_or("shell:Profile".into(), |p| p.display().to_string())),
        Item::Right(Right::Documents) => xp(XpRight::MyDocuments),
        Item::Right(Right::Pictures) => xp(XpRight::MyPictures),
        Item::Right(Right::Music) => xp(XpRight::MyMusic),
        Item::Right(Right::Computer) => xp(XpRight::MyComputer),
        Item::Right(Right::ControlPanel) => xp(XpRight::ControlPanel),
        Item::Right(Right::Devices) => xp(XpRight::Printers),
        Item::Right(Right::DefaultPrograms) => Some("ms-settings:defaultapps".into()),
        Item::Right(Right::Help) => xp(XpRight::Help),
        _ => None,
    }
}

/// Carries out an entry of Windows 7's Start menu that is no place (brief 114): the search box
/// opens Quick Search (PROVISIONAL: 7 searched in place); Shut down and its arrow the 2000 look's
/// Shut Down window (PROVISIONAL).
pub fn seven_start_action(item: bevel_core::seven_start::Item) {
    use bevel_core::seven_start::Item;
    match item {
        Item::Search => open_find(),
        Item::ShutDown => dialogs::open(DialogKind::ShutDown),
        _ => {}
    }
}

/// The place an entry of XP's Start menu opens (a folder, Windows' own window, the Help page):
/// started by the menu as the classic entries' folders are, while its vanished window still
/// holds the focus, so the new window may come to the front (the review of brief 112,
/// finding 8). `None`: a dialog, Quick Search, a cascade or a program.
pub fn xp_start_place(item: bevel_core::xp_start::Item) -> Option<String> {
    use bevel_core::xp_start::{Item, Right};
    Some(match item {
        Item::Right(Right::MyDocuments) => match start_menu::documents_folder() {
            Some(dir) => dir.display().to_string(),
            None => "shell:Personal".into(),
        },
        Item::Right(Right::MyPictures) => "shell:My Pictures".into(),
        Item::Right(Right::MyMusic) => "shell:My Music".into(),
        Item::Right(Right::MyComputer) => "shell:MyComputerFolder".into(),
        Item::Right(Right::ControlPanel) => "shell:ControlPanelFolder".into(),
        Item::Right(Right::Printers) => "shell:PrintersFolder".into(),
        Item::Right(Right::Help) => "https://support.microsoft.com/windows".into(),
        _ => return None,
    })
}

/// Carries out a Start menu entry that has no submenu.
fn start_action(index: usize) {
    let (era, opts) = with_app(|app| (app.era, app.start_options)).unwrap_or_default();
    match start::entry_at(era, opts, index) {
        Some(StartEntry::Help) => window_list::launch("https://support.microsoft.com/windows"),
        Some(StartEntry::WindowsUpdate) => window_list::launch("ms-settings:windowsupdate"),
        Some(StartEntry::Run) => dialogs::open(DialogKind::Run),
        Some(StartEntry::ShutDown) => dialogs::open(DialogKind::ShutDown),
        Some(StartEntry::LogOff) => dialogs::open(DialogKind::LogOff),
        _ => {}
    }
}

/// The window in front at a press on the bar, as a task button's click is to see it
/// (`tasks::front_seen`), and the one Windows has in front. Read first of all at the press, before
/// Quick Search, a flyout, a menu or the volume control is closed (the review of brief 90, R1).
/// The press's desktop mark is used up.
fn front_at_press() -> (Option<u64>, Option<u64>) {
    let front = window_list::active_window() as u64;
    let front = (front != 0).then_some(front);
    // One of the bar's own windows that has no button: a menu, the volume control, Quick Search.
    let own = front.is_some_and(|w| window_list::is_own(w as HWND));
    let seen = with_app(|app| {
        let own_in_front = own && !app.windows.iter().any(|w| Some(w.id) == front);
        tasks::front_seen(front, app.left_for_desktop.take(), own_in_front)
    })
    .flatten();
    (seen, front)
}

/// A click on a task button: one window comes to the front (or is minimized if it already
/// is); several windows get a list to pick from (`tasks::click` decides).
fn click_task(bar: HWND, index: usize) {
    let Some((windows, list_was_open)) = with_app(|app| {
        let list_was_open = std::mem::take(&mut app.pressed_closed_list);
        app.groups.get(index).map(|g| (g.windows.clone(), list_was_open))
    })
    .flatten() else {
        return;
    };
    // The window in front as the press saw it (`tasks::front_seen`: not the tick's, which can be
    // 100 ms old, nor the release's, after a menu the press closed had handed the front on);
    // read now only without a press (a click made otherwise). Outside the app state: whether the
    // button's one window is minimized.
    let (front, raw) = with_app(|app| app.press_front.take()).flatten().unwrap_or_else(|| {
        let front = window_list::active_window();
        let front = (!front.is_null()).then_some(front as u64);
        (front, front)
    });
    let minimized = matches!(windows.as_slice(), [only] if window_list::is_minimized(*only as HWND));
    match tasks::click(&windows, ClickState { front, minimized, list_was_open }) {
        Click::Nothing => {}
        Click::Minimize(id) => window_list::minimize(id as HWND),
        Click::Activate(id) => {
            // Brought forward though Windows had it in front at the press: the desktop was the
            // last thing pressed, and Windows had not let the desktop come forward (brief 90).
            if raw == Some(id) && front != raw {
                static INSTEAD: AtomicU32 = AtomicU32::new(0);
                bevel_winkit::log::counted(
                    &INSTEAD,
                    "task buttons: a click brought its window forward though it was in front: the desktop was the last thing pressed",
                );
            }
            window_list::activate(id as HWND);
            with_app(|app| app.activated = Some((id, Instant::now())));
        }
        Click::OpenList => popup::open_window_list(bar, index),
    }
}

/// How long after a click's activate its window must be in front (and not minimized), or the
/// log says it was not (`check_activated`).
const ACTIVATE_CHECK: Duration = Duration::from_millis(500);

/// On the tick: a window a click on its button (or in its button's list) brought forward that
/// is still not in front half a second later is logged, with its class and program file but
/// no title, so a button that "does nothing" leaves evidence.
fn check_activated() {
    let Some((id, _)) =
        with_app(|app| app.activated.take_if(|(_, at)| at.elapsed() >= ACTIVATE_CHECK)).flatten()
    else {
        return;
    };
    let hwnd = id as HWND;
    if !window_list::exists(hwnd) {
        return;
    }
    let front = window_list::active_window();
    let minimized = window_list::is_minimized(hwnd);
    if front != hwnd || minimized {
        let exe = window_list::program_path(hwnd);
        let exe = exe.rsplit(['\\', '/']).next().unwrap_or("");
        log(&format!(
            "a click's window is not in front {} ms later: {}, program {exe}{}; in front: {}",
            ACTIVATE_CHECK.as_millis(),
            window_list::kind_of(hwnd),
            if minimized { ", still minimized" } else { "" },
            window_list::kind_of(front),
        ));
    }
}

/// The buttons built again from the windows as last read (after their order changed).
fn regroup(app: &mut App) {
    app.groups = app.task_list.update(&app.windows, app.active);
}

/// Windows' drag threshold (SM_CXDRAG, 4 at 96 DPI) in the bar's screen pixels: scaled like
/// everything the bar draws.
fn drag_threshold(scale: Scale) -> i32 {
    // SAFETY: a plain query.
    let at_96 = unsafe { GetSystemMetricsForDpi(SM_CXDRAG, 96) };
    scale.len(at_96.max(1))
}

/// The pointer moved to `x` (bar pixels) while a task button is held down: past Windows' drag
/// threshold the press becomes a drag. The button then follows the pointer within the row
/// shown, and takes the place its middle is over (`tasks::drag_step`); the neighbours slide out
/// of its way as the bar paints (`tasks::Motion`, on the MOTION timer).
fn drag_move(bar: HWND, x: i32) {
    // No dragging of Windows 7's slots in this version (brief 114).
    if with_app(|app| app.bar_style == theme::BarStyle::Seven).unwrap_or(false) {
        return;
    }
    let Some(started) = with_app(|app| {
        let threshold = drag_threshold(app.scale);
        let press = app.press.as_mut()?;
        let started = press.moved(x, threshold);
        if !press.dragging {
            return None;
        }
        let grab = press.grab;
        app.drag_x = x;
        if started {
            app.drag_from = app.pressed;
            // From now on Esc is the drag's (an atomic switch: nothing waits here).
            keys::set_dragging(true);
        }
        app.pressed_inside = true;
        let at = app.pressed?;
        let layout = app.layout.as_ref()?;
        let first = layout.first_task;
        let row: Vec<(i32, i32)> = layout.tasks.iter().map(|r| (r.x, r.w)).collect();
        let place = at.checked_sub(first).filter(|&p| p < row.len())?;
        let (to, _) = tasks::drag_step(&row, place, grab, app.scale.old(x))?;
        if to != place {
            let groups = std::mem::take(&mut app.groups);
            app.task_list.move_button(&groups, at, first + to);
            app.pressed = Some(first + to);
            regroup(app);
        }
        Some(started)
    })
    .flatten() else {
        return;
    };
    if started {
        start_motion(bar);
    }
    // The dragged button is drawn where the pointer is now.
    invalidate_tasks(bar);
}

/// Starts the MOTION timer (a drag began), unless it runs already.
fn start_motion(bar: HWND) {
    let begins = with_app(|app| {
        let begins = app.motion_on.is_none();
        if begins {
            app.motion_on = Some((Instant::now(), 0, 0));
        }
        begins
    });
    if begins.unwrap_or(false) {
        // SAFETY: our own window.
        if unsafe { SetTimer(bar, MOTION, MOTION_MS, None) } == 0 {
            log_win32("SetTimer (task buttons' motion)");
        }
    }
}

/// On the MOTION timer: the task buttons are painted again while a drag lasts or one of them
/// slides; once everything is at rest the timer stops and the last frame is painted at the
/// buttons' places.
fn motion_step(bar: HWND) {
    let at_rest = with_app(|app| {
        let dragging = app.press.as_ref().is_some_and(|p| p.dragging);
        let now = app.epoch.elapsed().as_millis() as u64;
        if let Some((_, _, steps)) = app.motion_on.as_mut() {
            *steps += 1;
        }
        if dragging || app.motion.sliding(now) {
            return None;
        }
        app.motion.clear();
        app.motion_on.take()
    })
    .flatten();
    if let Some((since, frames, steps)) = at_rest {
        // SAFETY: our own window.
        unsafe { KillTimer(bar, MOTION) };
        log(&format!(
            "task buttons: a drag and its slides painted {frames} frames in {} ms ({steps} timer steps)",
            since.elapsed().as_millis()
        ));
    }
    invalidate_tasks(bar);
}

/// Asks for the task buttons' part of the bar to be painted again (only that part reaches the
/// screen; the bar's picture is made whole at 1x, which is quick).
fn invalidate_tasks(bar: HWND) {
    let area = with_app(|app| {
        let layout = app.layout.as_ref()?;
        let (first, last) = (layout.tasks.first()?, layout.tasks.last()?);
        let d = app.scale.dirty(Rect::new(first.x, 0, last.right() - first.x, app.theme.bar.height));
        Some(RECT { left: d.x, top: 0, right: d.right(), bottom: app.scale.len(app.theme.bar.height) })
    })
    .flatten();
    // SAFETY: our own window; a null rectangle means the whole window.
    unsafe { InvalidateRect(bar, area.as_ref().map_or(null(), |r| r as *const RECT), 0) };
}

/// Ends the press on a task button without a click (its window closed, Esc, the mouse capture
/// lost): a dragged button goes back where it was if `put_back`. Returns whether it was a drag.
fn end_press(bar: HWND, put_back: bool) -> bool {
    keys::set_dragging(false);
    let dragged = with_app(|app| {
        app.pressed = None;
        app.drag_from = None;
        let press = app.press.take()?;
        if press.dragging && put_back {
            app.task_list.set_order(press.saved);
            regroup(app);
        }
        Some(press.dragging)
    })
    .flatten()
    .unwrap_or(false);
    // SAFETY: our own window (its WM_CAPTURECHANGED finds no press any more).
    unsafe {
        ReleaseCapture();
        InvalidateRect(bar, null(), 0);
    }
    dragged
}

/// The file that keeps the order of the task buttons across a restart of the bar: the time
/// Windows started, then the windows' handles. Handles live only until Windows restarts, so
/// a file from before that is ignored.
fn task_order_file() -> std::path::PathBuf {
    data_dir().join("task-order.txt")
}

/// When Windows started, in seconds since 1970 (to the second or two).
fn boot_time() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    // SAFETY: a plain query.
    now.saturating_sub(unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() } / 1000)
}

fn save_task_order() {
    let Some(order) = with_app(|app| app.task_list.order().to_vec()) else {
        return;
    };
    let mut text = format!("boot {}\n", boot_time());
    for id in order {
        text += &format!("{id:x}\n");
    }
    if let Err(e) = state_file::save(&task_order_file(), &text)
        && !state_file::is_read_only(&e)
    {
        log(&format!("could not save the order of the task buttons: {e}"));
    }
}

/// The order kept by `save_task_order`, if it is from since Windows started (else nothing).
fn load_task_order() -> Vec<u64> {
    let Some(text) = state_file::load(&task_order_file()).text() else {
        return Vec::new();
    };
    let mut lines = text.lines();
    let boot = lines.next().and_then(|l| l.strip_prefix("boot ")).and_then(|b| b.trim().parse::<u64>().ok());
    if !boot.is_some_and(|b| b.abs_diff(boot_time()) <= 60) {
        return Vec::new();
    }
    let order: Vec<u64> = lines.filter_map(|l| u64::from_str_radix(l.trim(), 16).ok()).collect();
    log(&format!("task buttons: the order kept from before the restart has {} windows", order.len()));
    order
}

/// Middle-click: start another copy of the button's program.
fn launch_task(index: usize) {
    let path = with_app(|app| {
        let first = *app.groups.get(index)?.windows.first()?;
        Some(app.programs.of(first as HWND).path)
    })
    .flatten();
    match path {
        Some(path) if !path.is_empty() => window_list::launch(&path),
        _ => log("middle-click: no program file known for this button"),
    }
}

/// "Switch back to Windows": hides our bar, gives the space back, restores Windows'. Our
/// notification area's window goes too, and programs are asked to send their icons to Windows'
/// taskbar again (it has them already: everything was passed on to it).
fn show_windows_taskbar(bar: HWND) {
    popup::close(false);
    volume_popup::close(false);
    hide_tip(bar);
    with_app(|app| {
        app.ours = false;
        app.stand_in = None;
    });
    // The Monitor Lines go with the bar.
    crate::monitor_lines::set(None);
    // Windows' own desktop comes back with its taskbar.
    desktop::hide();
    keys::set_active(false);
    // SAFETY: our own window.
    unsafe { ShowWindow(bar, SW_HIDE) };
    tray::stop();
    appbar::remove(bar);
    explorer::restore();
    tray::broadcast_taskbar_created();
    // With the stand-in gone, Bevel95's own icon goes to Windows' notification area: a sign
    // that Bevel95 still runs, and the way back (`own_icon.rs`).
    let dpi = with_app(|app| app.dpi).unwrap_or(96);
    own_icon::show(bar, dpi);
}

/// Windows' taskbar told the bar about Bevel95's own notification icon (`own_icon.rs`): a
/// click switches back, as Ctrl+Win+T does; a right click opens the icon's menu.
fn own_icon_event(bar: HWND, wparam: WPARAM, lparam: LPARAM) {
    let (event, x, y) = own_icon::event_of(wparam, lparam);
    let ours = with_app(|app| app.ours).unwrap_or(true);
    match bevel_core::tray_icon::respond(event, !ours) {
        bevel_core::tray_icon::Response::SwitchBack => {
            log(&format!("switching back to Bevel95 (its notification icon: {event:?})"));
            popup::close(false);
            show_ours(bar);
        }
        bevel_core::tray_icon::Response::OpenMenu => popup::open_own_icon_menu(bar, x, y),
        bevel_core::tray_icon::Response::Nothing => {}
    }
}

fn toggle(bar: HWND) {
    if with_app(|app| app.ours).unwrap_or(false) {
        log("switching to the Windows taskbar");
        show_windows_taskbar(bar);
    } else {
        log("switching back to Bevel95");
        show_ours(bar);
    }
}

/// Fits the bar to the main screen at the current scale (also after the screen or the
/// Windows display scale changes). It reserves its strip of the screen like the real taskbar,
/// except when it hides itself (Auto hide): then it reserves nothing and sits mostly below
/// the screen's edge until the pointer comes. Always on top decides whether other windows can
/// cover it.
fn reposition(bar: HWND) {
    // Switched to Windows' taskbar (Bevel95 Settings, opened from Bevel95's notification icon,
    // may apply a new size or look meanwhile): no space is reserved and nothing is placed until
    // the bar comes back (`show_ours` places it then).
    if !with_app(|app| app.ours).unwrap_or(false) {
        return;
    }
    let (monitor, dpi) = primary_monitor();
    let Some((height, conceded, scale, options, shown)) = with_app(|app| {
        app.monitor = monitor;
        app.dpi = dpi;
        app.scale = config::scale_in_force(app.scale_setting, app.scaling, dpi);
        let height = app.scale.len(app.theme.bar.height);
        let hidden = app.scale.len(HIDDEN_ROWS);
        // A new size or setting: no slide, straight to where the bar should be.
        app.target_px = if app.options.auto_hide && app.target_px <= hidden { hidden } else { height };
        app.shown_px = app.target_px;
        (height, app.fullscreen.is_behind(), app.scale, app.options, app.shown_px)
    }) else {
        return;
    };
    // SAFETY: our own timer (harmless if it is not running).
    unsafe { KillTimer(bar, SLIDE) };
    let rc = if options.auto_hide {
        appbar::remove(bar);
        RECT { left: monitor.left, top: monitor.bottom - shown, right: monitor.right, bottom: monitor.bottom - shown + height }
    } else {
        appbar::register(bar, WM_APPBAR.id());
        appbar::place(bar, monitor, height)
    };
    // Changing the auto-hide setting makes Windows send several "settings changed"
    // messages, each of which re-fits the bar; only log real moves.
    let moved = with_app(|app| !same_rect(&std::mem::replace(&mut app.placed, rc), &rc)).unwrap_or(true);
    if moved {
        log(&format!(
            "placed the bar at {},{} {}x{} (screen {}x{}, {dpi} DPI, scale {scale})",
            rc.left,
            rc.top,
            rect_w(&rc),
            rect_h(&rc),
            rect_w(&monitor),
            rect_h(&monitor)
        ));
    }
    // SAFETY: our own window.
    unsafe {
        SetWindowPos(bar, z_order(conceded, options), rc.left, rc.top, rect_w(&rc), rect_h(&rc), SWP_NOACTIVATE);
        InvalidateRect(bar, null(), 0);
    }
    // The desktop's icons follow the scale and the screen's free area.
    sync_desktop();
    // The Monitor Lines follow the screen and the size at once, and stay above the bar just
    // put on top.
    sync_lines();
    crate::monitor_lines::raise();
}

/// Where the bar goes among the windows: behind a fullscreen app; else above every window
/// (Always on top), or among them, where others can cover it.
fn z_order(conceded: bool, options: TaskbarOptions) -> HWND {
    if conceded {
        HWND_BOTTOM
    } else if options.always_on_top {
        HWND_TOPMOST
    } else {
        HWND_NOTOPMOST
    }
}

/// Auto hide: starts sliding the bar onto the screen (`show`) or off it.
fn slide(bar: HWND, show: bool) {
    let start = with_app(|app| {
        if !app.options.auto_hide {
            return false;
        }
        let target = if show { app.scale.len(app.theme.bar.height) } else { app.scale.len(HIDDEN_ROWS) };
        let start = app.target_px != target;
        app.target_px = target;
        start
    })
    .unwrap_or(false);
    if start {
        // SAFETY: our own window and timer.
        unsafe { SetTimer(bar, SLIDE, SLIDE_MS, None) };
    }
}

/// One step of the slide: a sixth of the bar's height (as quick as Windows 95's).
fn slide_step(bar: HWND) {
    let Some((rc, done)) = with_app(|app| {
        let height = app.scale.len(app.theme.bar.height);
        let step = (height / 6).max(1);
        let d = app.target_px - app.shown_px;
        app.shown_px += d.signum() * d.abs().min(step);
        let m = app.monitor;
        let rc = RECT {
            left: m.left,
            top: m.bottom - app.shown_px,
            right: m.right,
            bottom: m.bottom - app.shown_px + height,
        };
        app.placed = rc;
        (rc, app.shown_px == app.target_px)
    }) else {
        return;
    };
    // SAFETY: our own window and timer.
    unsafe {
        SetWindowPos(bar, null_mut(), rc.left, rc.top, rect_w(&rc), rect_h(&rc), SWP_NOZORDER | SWP_NOACTIVATE);
        if done {
            KillTimer(bar, SLIDE);
        }
    }
}

/// Before the Start menu opens from the keyboard: an auto-hidden bar comes up at once, and a
/// bar that is not always on top comes in front of the other windows.
fn bring_up(bar: HWND) {
    let Some((options, hidden)) = with_app(|app| (app.options, app.shown_px < app.scale.len(app.theme.bar.height))) else {
        return;
    };
    if options.auto_hide && hidden {
        with_app(|app| {
            app.target_px = app.scale.len(app.theme.bar.height);
            app.away_ticks = 0;
        });
        reposition(bar);
    }
    if !options.always_on_top {
        // SAFETY: our own window.
        unsafe { SetWindowPos(bar, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
    }
}

/// Auto hide: the bar comes up while the pointer is on it (or one of its menus is open, or a
/// button is held), and slides away half a second after the pointer leaves.
fn auto_hide_tick(bar: HWND) {
    let mut at = POINT::default();
    // SAFETY: a valid out-pointer.
    unsafe { GetCursorPos(&mut at) };
    let Some((wanted, away)) = with_app(|app| {
        let m = app.monitor;
        let over = at.x >= m.left && at.x < m.right && at.y >= app.placed.top && at.y < m.bottom;
        // Also while the Quick Launch band's grip is dragged, an arrow or an icon of it held down.
        let busy = !app.menus.is_empty()
            || app.pressed.is_some()
            || app.scroll_pressed.is_some()
            || app.ql.sizing.dragging()
            || app.ql.pressed_control.is_some()
            || app.ql.held.is_some();
        app.away_ticks = if over || busy { 0 } else { app.away_ticks + 1 };
        (over || busy, app.away_ticks)
    }) else {
        return;
    };
    if wanted {
        slide(bar, true);
    } else if away >= HIDE_AFTER_TICKS {
        slide(bar, false);
    }
}

/// What the Programs menu shows from now on (into effect and into settings.txt): the Start
/// menu takes the choice each time it opens.
pub fn set_programs_shown(shown: Shown) {
    if with_app(|app| std::mem::replace(&mut app.programs_shown, shown) != shown).unwrap_or(false) {
        save_setting("programs", shown.setting());
        log(&format!("programs: the menu now shows {}", shown.setting()));
    }
}

/// Puts what Taskbar Properties or Bevel95 Settings says into effect and into settings.txt
/// (`values`: that window's own settings, the rest as they are in effect: `Sheet::merge`).
pub fn apply_properties(values: bevel_core::properties::Values) {
    let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
    if values.start_with_windows != autostart::is_on() {
        autostart::set(values.start_with_windows);
    }
    let changed = with_app(|app| {
        let changed = app.options != values.options
            || app.start_options != values.start
            || app.desktop != values.desktop
            || app.group != values.group;
        app.options = values.options;
        app.start_options = values.start;
        app.desktop = values.desktop;
        app.group = values.group;
        app.task_list.group = values.group || app.bar_style == theme::BarStyle::Seven;
        changed
    })
    .unwrap_or(false);
    if changed {
        let config = Config {
            options: values.options,
            start: values.start,
            desktop: values.desktop,
            group: values.group,
            ..Config::default()
        };
        let path = data_dir().join("settings.txt");
        let changed = change_settings(&path, |text| {
            let mut text = text.to_string();
            for (key, value) in config::flag_settings(&config) {
                text = config::with_setting(&text, key, value);
            }
            text
        });
        if let Err(e) = changed
            && !state_file::already_logged(&e)
        {
            log(&format!("could not save the taskbar options to {}: {e}", crate::log::path_kind(&path)));
        }
        log(&format!(
            "properties: {:?}, {:?}, desktop {}, group {}",
            values.options, values.start, values.desktop, values.group
        ));
    }
    set_programs_shown(values.programs);
    if with_app(|app| std::mem::replace(&mut app.windows_key, values.windows_key) != values.windows_key).unwrap_or(false) {
        save_setting("windows_key", values.windows_key.setting());
        log(&format!("the Windows key now opens {}", values.windows_key.setting()));
    }
    let quick_search = if values.quick_search_all { QuickSearch::Everything } else { QuickSearch::Programs };
    if with_app(|app| std::mem::replace(&mut app.quick_search, quick_search) != quick_search).unwrap_or(false) {
        save_setting("quick_search", quick_search.setting());
        log(&format!("Quick Search now lists {}", quick_search.setting()));
        find_window::programs_read();
    }
    // Quick Launch: written only when it changes from what holds now (the look's default while
    // settings.txt does not say).
    // (The setting, not whether the band shows: under XP's bar style it never shows, brief 112.)
    if with_app(|app| app.quick_launch_setting() != values.quick_launch).unwrap_or(false) {
        set_quick_launch(values.quick_launch);
    }
    // Written only when changed, so a settings.txt that never had the line does not get one.
    let battery_percent = values.battery_percent;
    if with_app(|app| std::mem::replace(&mut app.battery_percent, battery_percent) != battery_percent).unwrap_or(false) {
        save_setting("battery_percent", if battery_percent { "yes" } else { "no" });
        log(&format!("the battery's percentage is now {}", if battery_percent { "shown" } else { "hidden" }));
    }
    // Written only when changed, as the battery's.
    let windows_display = values.windows_display;
    if with_app(|app| std::mem::replace(&mut app.windows_display_settings, windows_display) != windows_display).unwrap_or(false) {
        save_setting("windows_display_settings", if windows_display { "yes" } else { "no" });
        log(&format!("the desktop's Properties now opens {}", if windows_display { "Windows' display settings" } else { "Display Properties" }));
    }
    if with_app(|app| app.clock_offset != values.clock_offset).unwrap_or(false) {
        set_clock_offset(values.clock_offset);
    }
    if with_app(|app| app.scaling != values.scale.way).unwrap_or(false) {
        set_scaling(bar, values.scale.way);
    }
    if with_app(|app| app.scale.quarters() != values.scale.quarters()).unwrap_or(false) {
        set_size(bar, values.scale);
    }
    if with_app(|app| app.era != values.era || app.bar_style != values.bar_style || app.saved_style != values.bar_style).unwrap_or(false) {
        crate::failsafe::clear();
        set_era(bar, values.era, values.bar_style);
    }
    if with_app(|app| app.colors != values.colors).unwrap_or(false) {
        set_colors(bar, values.colors);
    }
    // Monitor Lines (the Desktop page; brief 102), after the size, so they are put on for the size
    // now in force (brief 104, N3): written only when changed.
    if with_app(|app| app.monitor_lines != values.monitor_lines).unwrap_or(false) {
        set_monitor_lines(values.monitor_lines, false);
    }
    // Tabs in folder windows (the Folders page; brief 110): written only when changed; the open
    // folder windows follow at once (they read the settings again as at a change of look).
    let tabs_now = (values.folder_tabs, values.folder_tabs_close);
    if with_app(|app| std::mem::replace(&mut app.folder_tabs, tabs_now) != tabs_now).unwrap_or(false) {
        save_setting("folder_tabs", values.folder_tabs.word());
        save_setting("folder_tabs_close", values.folder_tabs_close.word());
        log(&format!("folder windows' tabs: {}, the X {}", values.folder_tabs.word(), values.folder_tabs_close.word()));
        kit::tell_programs_look_changed();
    }
    sync_desktop();
    reposition(bar);
    refresh(bar);
}

/// The checks that run every TICK_MS while our bar is showing.
fn tick(bar: HWND) {
    // Bevel95's own window resources: logged every ten minutes, warned about past 2,000.
    bevel_winkit::log::watch_gui_resources();
    let Some((ours, ending, monitor, dpi, clock_changed, ticks, last_front)) = with_app(|app| {
        let now = clock_text(app.clock_offset);
        let changed = now != app.clock;
        app.clock = now;
        app.ticks = app.ticks.wrapping_add(1);
        (app.ours, app.ending, app.monitor, app.dpi, changed, app.ticks, app.front)
    }) else {
        return;
    };
    // A live test's Monitor Lines end by themselves; the lines follow what decides them (they go
    // while the bar does not show: `sync_lines`).
    // A helper's pause ends by itself too.
    let (test_over, pause_over) = with_app(|app| {
        let over = app.monitor_lines_test.is_some()
            && app.monitor_lines_test_began.is_some_and(|at| at.elapsed().as_secs() >= MONITOR_LINES_TEST_SECS);
        if over {
            app.monitor_lines_test = None;
        }
        let pause_over = app.monitor_lines_paused.is_some_and(|at| at.elapsed().as_secs() >= MONITOR_LINES_PAUSE_SECS);
        if pause_over {
            app.monitor_lines_paused = None;
        }
        (over, pause_over)
    })
    .unwrap_or_default();
    if test_over {
        log("monitor lines: the live test's lines ended by themselves");
    }
    if pause_over {
        log("monitor lines: a helper's pause ended by itself");
    }
    sync_lines();
    // While the session may be ending, the Windows taskbar stays as it was put back; so it does
    // after a panic, until the bar's window closes (`main::install_panic_hook`).
    if !ours || ending || bevel_winkit::panic::happened() {
        return;
    }
    if clock_changed {
        // SAFETY: our own window.
        unsafe { InvalidateRect(bar, null(), 0) };
    }
    // The window list: in full every 2 s (in case a notification was missed), and at once
    // when another window comes to the front.
    if ticks % REFRESH_TICKS == 0 || window_list::active_window() as u64 != last_front {
        refresh(bar);
    }
    check_activated();
    crate::find_window::close_if_turned_away();
    // The network and battery icons: the connection's counters every 200 ms (its screens light
    // while data moves), the battery every 2 s; repainted only when a picture changes.
    if ticks.is_multiple_of(NETWORK_TICKS) && status_changed(ticks) {
        // SAFETY: our own window.
        unsafe { InvalidateRect(bar, null(), 0) };
    }
    // The notification area: our window stays the one programs find, and icons of programs
    // that ended without removing them go (every 2 s).
    tray::keep_first();
    if ticks % REFRESH_TICKS == 0 && tray::prune() {
        // SAFETY: our own window.
        unsafe { InvalidateRect(bar, null(), 0) };
    }
    // Bevel95's desktop stays right on top of Windows' own, as large as all screens.
    desktop::tick();
    // The Windows taskbar likes to pop back up (Win key, mouse at the screen edge, ...).
    if explorer::any_visible() {
        let n = with_app(|app| {
            app.rehides += 1;
            app.rehides
        })
        .unwrap_or(0);
        if n <= 3 || n % 100 == 0 {
            log(&format!("the Windows taskbar reappeared; hiding it again (time {n})"));
        }
        explorer::hide();
    }
    // Screen size or Windows display scale changed?
    let (now_monitor, now_dpi) = primary_monitor();
    if !same_rect(&now_monitor, &monitor) || now_dpi != dpi {
        reposition(bar);
    }
    // A fullscreen app (a video, a game) on our screen: step behind it, like the real taskbar.
    let (changed, fullscreen, options) =
        with_app(|app| (app.fullscreen.update(&now_monitor), app.fullscreen.is_behind(), app.options))
            .unwrap_or_default();
    if changed {
        log(if fullscreen { "a fullscreen app is showing; stepping behind it" } else { "back on top" });
        // SAFETY: our own window.
        unsafe {
            SetWindowPos(bar, z_order(fullscreen, options), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)
        };
        // The Monitor Lines go and come back with the bar, at the same moment.
        sync_lines();
    }
    if options.auto_hide && !fullscreen {
        auto_hide_tick(bar);
    }
    // A window come above the Monitor Lines goes back under them (`monitor_lines::keep_on_top`).
    crate::monitor_lines::keep_on_top();
}

/// Reads the connection (and every 2 s the battery) afresh; true if an icon's picture changed.
fn status_changed(ticks: u32) -> bool {
    with_app(|app| {
        let network = app.network_reader.read();
        let look = |n: &bevel_core::status::Network| (n.connected, n.sending, n.receiving);
        let mut changed = look(&network) != look(&app.network);
        app.network = network;
        if ticks.is_multiple_of(REFRESH_TICKS) {
            let power = status::power();
            changed |= power != app.power;
            app.power = power;
        }
        changed
    })
    .unwrap_or(false)
}

fn paint_bar(bar: HWND) {
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: paired BeginPaint/EndPaint on our own window.
    let hdc = unsafe { BeginPaint(bar, &mut ps) };
    let icons = tray::shown();
    let painted = with_app(|app| {
        // Bevel95's own icons first (Windows' system icons were the first to arrive), then the
        // programs'. The battery's only on a PC with a battery, as in Windows 95 to 2000.
        let mut own: Vec<(tray::Key, Icon)> =
            vec![(NETWORK_KEY, bevel_core::status::network_icon(app.era, &app.network))];
        if app.power.battery {
            own.push((POWER_KEY, bevel_core::status::power_icon(app.era, &app.power)));
        }
        let width = rect_w(&app.monitor);
        let width_1x = app.scale.old_cover(width).max(1);
        let mut fb = Framebuffer::new(width_1x, app.theme.bar.height, 0);
        // A dragged button keeps the look it had before the press (raised, or sunken for the
        // window in front); only a press that is not (yet) a drag looks pushed in.
        let dragging = app.press.as_ref().is_some_and(|p| p.dragging);
        let tasks = app
            .groups
            .iter()
            .enumerate()
            .map(|(i, group)| Task {
                title: group.label.clone(),
                icon: app.group_icon(group),
                state: if app.pressed == Some(i) && app.pressed_inside && !dragging {
                    TaskState::Active
                } else {
                    group.state
                },
            })
            .collect();
        let start_pressed = app.menus.first().is_some_and(popup::Popup::is_start);
        let mut state = Bar {
            start_pressed,
            tasks,
            // Windows' volume icon shows the classic speaker (muted or not).
            tray: own
                .iter()
                .map(|(_, icon)| Some(icon.clone()))
                .chain(icons.iter().map(|i| {
                    if i.is_volume() { Some(bevel_core::icons::speaker(app.volume_muted)) } else { i.picture.clone() }
                }))
                .collect(),
            // Show Clock off: no clock (and without icons, no well either).
            clock: if app.options.show_clock { app.clock.clone() } else { String::new() },
            clock_widest: app.clock_widest.clone(),
            // The battery's percentage beside its icon (Bevel95's own option): only with the
            // icon and while Windows knows the percentage.
            tray_number: own
                .iter()
                .position(|(key, _)| *key == POWER_KEY)
                .filter(|_| app.battery_percent)
                .and_then(|i| Some((i, bevel_core::status::percent_text(&app.power)?))),
            row: app.task_row,
            scroll_pressed: app.scroll_pressed,
            quick_launch: if app.quick_launch_on() {
                app.ql.refresh(app.era);
                Some(app.ql.band(app.era, app.menus.first().is_some_and(popup::Popup::is_quick_list)))
            } else {
                None
            },
            ..Bar::default()
        };
        // Windows 7's slots (brief 114): Quick Launch's shortcuts pinned, a running program in its
        // pinned place (its program file the shortcut's target), 32×32 pictures, the short date.
        if app.bar_style == theme::BarStyle::Seven {
            let era = app.era;
            app.ql.refresh(era);
            let links: Vec<String> = app.ql.shortcuts.iter().map(|s| s.path.display().to_string()).collect();
            let programs: Vec<String> = app.groups.iter().map(|g| g.key.clone()).collect();
            // Only lookups here: the files are read on the store's own thread, which repaints the
            // bar when they arrive (the review of brief 114, finding 2).
            crate::seven_store::want(crate::seven_store::Asker::Bar, &[links.clone(), programs.clone()].concat());
            let targets: Vec<Option<String>> = links.iter().map(|l| crate::seven_store::program_of(l)).collect();
            let tasks_of = bevel_core::seven_bar::pin_tasks(&targets, &programs);
            state.pinned = links
                .iter()
                .zip(tasks_of)
                .map(|(link, task)| bevel_core::paint::Pinned { icon: crate::seven_store::picture(link), task })
                .collect();
            // A true 32×32 for every slot (the owner, 2026-11-25): the program file's, else the
            // window's own large one shrunk to 32, and only then its 16×16 doubled.
            state.big_icons = app
                .groups
                .iter()
                .map(|g| {
                    crate::seven_store::picture(&g.key)
                        .or_else(|| app.group_big_icon(g).filter(|i| i.width >= 32).map(|i| i.downscaled(32)))
                })
                .collect();
            state.date = if app.options.show_clock { short_date(app.clock_offset) } else { String::new() };
        }
        if let Some((_, frames, _)) = app.motion_on.as_mut() {
            *frames += 1;
            let layout = paint::bar_layout(app.theme, &app.fonts(), width_1x, &state);
            let row: Vec<(i32, i32)> = layout.tasks.iter().map(|r| (r.x, r.w)).collect();
            let first = layout.first_task;
            // The dragged button: where the pointer puts it in the row shown.
            let dragged = app
                .press
                .as_ref()
                .filter(|p| p.dragging)
                .zip(app.pressed.and_then(|at| at.checked_sub(first)))
                .and_then(|(p, at)| {
                    tasks::drag_step(&row, at, p.grab, app.scale.old(app.drag_x)).map(|(_, left)| (at, left))
                });
            let keys: Vec<&str> = app.groups.iter().skip(first).take(row.len()).map(|g| g.key.as_str()).collect();
            let now = app.epoch.elapsed().as_millis() as u64;
            state.task_x = app.motion.frame(&keys, &row, dragged, now);
            state.task_on_top = dragged.map(|(at, _)| at);
        }
        let finals = final_pictures(app, &mut state, width_1x);
        let layout = paint::paint_bar(&mut fb, app.theme, &app.fonts(), &state);
        app.task_row = layout.scroll.map_or(0, |s| s.row); // within the rows there are now
        let mut screen = app.scale.enlarge(&fb);
        // XP's and 7's live pictures at their final size over the enlarged bar (the owner,
        // 2026-11-25: a deliberate exception to "paint at 1x and enlarge").
        for (r, icon) in &finals {
            paint::draw_final_icon(&mut screen, app.scale, *r, icon);
        }
        blit(hdc, &screen, width);
        // Where the icons and the well are on the screen: for programs that ask where their
        // icon is, and for the stand-in windows (only moved when that changed).
        app.tray_keys = own.iter().map(|(key, _)| *key).chain(icons.iter().map(tray::TrayIcon::key)).collect();
        let (s, x0, y0) = (app.scale, app.placed.left, app.placed.top);
        let on_screen = |r: Rect| RECT { left: x0 + s.at(r.x), top: y0 + s.at(r.y), right: x0 + s.at(r.right()), bottom: y0 + s.at(r.bottom()) };
        let rects: Vec<(tray::Key, RECT)> = app.tray_keys.iter().zip(&layout.tray).map(|(k, r)| (*k, on_screen(*r))).collect();
        let well = on_screen(layout.well);
        let moved = app.stand_in.is_none_or(|(b, w)| !same_rect(&b, &app.placed) || !same_rect(&w, &well));
        if moved {
            app.stand_in = Some((app.placed, well));
        }
        // The band as it was laid out: the pictures wanted, 98's scroll position kept within it,
        // 2000's band grown by icons arriving (`quick_launch::State::shown`).
        let era = app.era;
        app.ql.shown(era, layout.quick.as_ref());
        app.layout = Some(layout);
        (rects, well, moved.then_some(app.placed))
    });
    // SAFETY: see above.
    unsafe {
        EndPaint(bar, &ps);
        if painted.is_none() {
            PostMessageW(bar, WM_REPAINT, 0, 0);
        }
    }
    if let Some((rects, well, moved)) = painted {
        tray::set_screen_rects(rects, well);
        if let Some(placed) = moved {
            tray::place(placed, well);
        }
    }
}

/// The live program pictures drawn at their final size over the enlarged bar, under XP's and 7's
/// styles above size 1 (the owner, 2026-11-25: "icons at 2x on 7" looked enlarged): each box's
/// true picture of `Scale::rect(box)`'s size, from the store (a shortcut's or a program file's,
/// read off the bar's thread) or the window's own large one; those found are put in
/// `Bar::drawn_later`, so the 1x painting leaves them out. A box with no picture that large keeps
/// its enlarged 1x one. The classic looks never get here.
fn final_pictures(app: &App, state: &mut Bar, width_1x: i32) -> Vec<(Rect, Icon)> {
    use bevel_core::paint::LiveIcon;
    use crate::seven_store::{self, Asker};
    // Also at size 1 (the owner, 2026-11-25: "1x still isn't what I'd expect"): the shell's own
    // small pictures come at the screen's scale and were shrunk crudely; these are averaged.
    // The classic looks never touch the store (brief 115; `seven_store::set_later`).
    if !app.bar_style.is_later() {
        return Vec::new();
    }
    let boxes = paint::live_icon_boxes(app.theme, &app.fonts(), width_1x, state);
    let mut wanted: Vec<(String, i32)> = Vec::new();
    let mut out = Vec::new();
    for (which, r) in boxes {
        let side = app.scale.rect(r).w;
        let window = |t: usize| app.groups.get(t).and_then(|g| app.group_big_icon(g)).filter(|i| i.width >= side);
        let mut file = |path: String| {
            let found = seven_store::picture_at(&path, side);
            wanted.push((path, side));
            found
        };
        let picture = match which {
            LiveIcon::Pinned(k) => app.ql.shortcuts.get(k).and_then(|s| file(s.path.display().to_string())),
            LiveIcon::Task(t) if app.bar_style == theme::BarStyle::Seven => {
                let key = app.groups.get(t).map(|g| g.key.clone()).filter(|k| seven_store::is_file(k));
                key.and_then(&mut file).or_else(|| window(t))
            }
            // XP: a window Bevel95 shows a classic picture for keeps it; else the program file's
            // picture, else the window's own large one.
            LiveIcon::Task(t) => {
                let group = app.groups.get(t);
                if group.is_none_or(|g| app.group_is_classic(g)) {
                    None
                } else {
                    let key = group.map(|g| g.key.clone()).filter(|k| seven_store::is_file(k));
                    key.and_then(&mut file).or_else(|| window(t))
                }
            }
        };
        if let Some(p) = picture {
            state.drawn_later.push(which);
            out.push((r, p));
        }
    }
    seven_store::want_sized(Asker::BarSized, &wanted);
    out
}

/// Carries out a chosen menu entry (`code`: its `BarCommand::code`).
fn run_command(bar: HWND, code: usize, shift: bool) {
    let Some(command) = BarCommand::from_code(code) else {
        return;
    };
    // Under XP and 7 a switch nobody sees there would only surprise at the way back (the review
    // of brief 112, finding 4: it put back deleted icons); every route to it, the menus' and
    // Quick Search's (the review of brief 114, finding 5).
    if let Some(what) = command.shows_nothing_under_later()
        && xp_refuses(what)
    {
        return;
    }
    let ours = with_app(|app| app.ours).unwrap_or(false);
    match command {
        BarCommand::AdjustDateTime => datetime_window::open(),
        BarCommand::SwitchToWindowsTaskbar if !ours => log("menu: switch to the Windows taskbar: it shows already"),
        BarCommand::SwitchToWindowsTaskbar => {
            log("menu: switch to the Windows taskbar");
            show_windows_taskbar(bar);
        }
        BarCommand::SwitchToBevel95 if ours => log("menu: switch to Bevel95: its bar shows already"),
        BarCommand::SwitchToBevel95 => {
            log("switching back to Bevel95 (its notification icon's menu)");
            show_ours(bar);
        }
        BarCommand::Properties => properties_window::open(),
        BarCommand::Settings => properties_window::open_settings(),
        BarCommand::Find => open_find(),
        BarCommand::Run => dialogs::open(DialogKind::Run),
        BarCommand::ShutDown => dialogs::open(DialogKind::ShutDown),
        BarCommand::Explorer => match start_menu::own_program_target("folders.exe") {
            Some(target) => window_list::launch(&target),
            None => log("Quick Search: the folder windows' program is missing"),
        },
        BarCommand::MyComputer => desktop::open_item("::my-computer".into()),
        BarCommand::DesktopOn => set_desktop(true),
        BarCommand::DesktopOff => set_desktop(false),
        BarCommand::About => {
            popup::close(false);
            about_window::open();
        }
        // The latest release page in the person's browser, as About's button opens it (brief 100).
        BarCommand::CheckForUpdates => {
            popup::close(false);
            about_window::open_page(bevel_core::about::Part::Updates);
        }
        BarCommand::QuickLaunch => {
            let on = with_app(|app| app.quick_launch_on()).unwrap_or(false);
            set_quick_launch(!on);
        }
        BarCommand::QuickLaunchClose => set_quick_launch(false),
        BarCommand::QuickLaunchOpen => quick_launch::open_folder(),
        BarCommand::QuickLaunchRefresh => quick_launch::read_again(),
        BarCommand::Restart => restart(bar),
        BarCommand::Exit if shift => restart(bar),
        BarCommand::Exit => {
            log("menu: exit");
            // SAFETY: our own window; WM_DESTROY does the cleanup.
            unsafe { DestroyWindow(bar) };
        }
    }
}

/// Opens Find (or brings it forward), as the bar's own window, after any menu of ours is gone.
fn open_find() {
    popup::close(false);
    volume_popup::close(false);
    find_window::open();
}

/// Every save of the bar's settings.txt (`state_file::change_settings`). A save refused because a
/// portable copy's settings.txt has lost its first line is no longer silent: once per run the bar
/// says so on screen, in the look's own message window (the fourth safety review of 2026-11-18,
/// C1), at its next tick (`show_settings_notice`), not in the middle of what saved.
fn change_settings(path: &std::path::Path, f: impl FnOnce(&str) -> String) -> std::io::Result<()> {
    let result = state_file::change_settings(path, f);
    if let Err(e) = &result
        && state_file::is_not_ours(e)
    {
        let _ = SETTINGS_NOTICE.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst);
    }
    result
}

/// The notice that the settings could not be saved: 0 none yet, 1 to be shown, 2 shown (once per run).
static SETTINGS_NOTICE: AtomicU32 = AtomicU32::new(0);

/// The notice's words. PROVISIONAL.
const SETTINGS_NOTICE_TEXT: &str = concat!(
    bevel_core::product_name!(),
    " could not save your settings: the first line \"# Bevel95 settings\" is missing from settings.txt next to the \
     program, so ",
    bevel_core::product_name!(),
    " cannot tell that the file is its own and does not write it.\n\nPut that line back as the first line of the file \
     to save settings again. Until then a change holds only until ",
    bevel_core::product_name!(),
    " ends."
);

/// Shows the notice (the bar's tick), once per run.
fn show_settings_notice() {
    if SETTINGS_NOTICE.compare_exchange(1, 2, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
        log("settings.txt could not be saved (its mark is missing): the look's message says so (once per run)");
        crate::confirm_window::message(bevel_core::about::NAME, SETTINGS_NOTICE_TEXT);
    }
}

/// Writes one setting into settings.txt, keeping everything else in the file as it was.
fn save_setting(key: &str, value: &str) {
    let path = data_dir().join("settings.txt");
    if let Err(e) = change_settings(&path, |text| config::with_setting(text, key, value))
        && !state_file::already_logged(&e)
    {
        log(&format!("could not save {key} to {}: {e}", crate::log::path_kind(&path)));
    }
}

/// The wParam and lParam of `WM_BAR_SIZE` for `scale`: in quarters.
pub fn size_message(scale: Scale) -> (usize, isize) {
    (scale.quarters() as usize, 4)
}

/// The size a `WM_BAR_SIZE` asks for: wParam a whole size (lParam 0) or quarters (lParam 4),
/// from 1 to 8; anything else is no size.
fn size_of_message(wparam: usize, lparam: isize) -> Option<Scale> {
    let quarters = match lparam {
        0 => i32::try_from(wparam).ok()?.checked_mul(4)?,
        4 => i32::try_from(wparam).ok()?,
        _ => return None,
    };
    (bevel_core::scale::MIN_QUARTERS..=bevel_core::scale::MAX_QUARTERS).contains(&quarters).then(|| Scale::from_quarters(quarters))
}

/// `WM_BAR_SIZE`'s lParam for the Size question's answers (wParam its ticket): keep the size, or
/// go back to the one before. Posted by the question's own actions, so that the size changes
/// only once the question's window has gone.
const SIZE_KEEP: isize = 8;
const SIZE_BACK: isize = 9;

/// The Size menu and Quick Search (`WM_BAR_SIZE`): a whole size at once and saved (`set_size`);
/// a size between whole numbers is put on at once, so the person sees it, but saved only after
/// Yes to the look's question; No, closing it, or the bar ending or changing its look while it is
/// open goes back to the size before, and nothing is saved. It asks every time, also from
/// another such size.
fn choose_size(bar: HWND, scale: Scale) {
    if scale.is_whole() {
        set_size(bar, scale);
        return;
    }
    let chosen = with_app(|app| {
        let chosen = app.size_asking.choose(app.scale_setting, scale);
        if let bar_menu::SizeChosen::Ask(scale, _) = chosen {
            app.scale_setting = Some(scale);
        }
        chosen
    });
    let Some(bar_menu::SizeChosen::Ask(scale, ticket)) = chosen else {
        return;
    };
    log(&format!("size: {scale}x, on until the question is answered"));
    size_changed(bar);
    let bar_id = bar as usize; // the window's id travels in the closures as a number
    let answer = move |keep: bool| -> Box<dyn FnOnce()> {
        Box::new(move || {
            // SAFETY: posting to the bar's own window (gone: the post fails harmlessly).
            unsafe { PostMessageW(bar_id as HWND, WM_BAR_SIZE.id(), ticket, if keep { SIZE_KEEP } else { SIZE_BACK }) };
        })
    };
    crate::confirm_window::ask_choice(bevel_core::delete_confirm::Question::KeepSize, false, answer(true), answer(false));
}

/// The Size question's answer (`choose_size`); an answer of an older question does nothing.
fn size_answered(bar: HWND, ticket: usize, keep: bool) {
    let answer = with_app(|app| {
        let answer = app.size_asking.answer(ticket, keep)?;
        if let bar_menu::SizeAnswer::Back(before) = answer {
            app.scale_setting = before;
        }
        Some(answer)
    });
    match answer.flatten() {
        Some(bar_menu::SizeAnswer::Keep(scale)) => {
            let style = with_app(|app| {
                let style = app.bar_style;
                app.style_scales.set_scale_of(style, Some(scale));
                style
            })
            .unwrap_or_default();
            save_setting(config::scale_key(style), &scale.label());
            log(&format!("size: {scale}x kept"));
            // Bevel95's other programs read the size from settings.txt: now it is there.
            kit::tell_programs_look_changed();
        }
        Some(bar_menu::SizeAnswer::Back(_)) => {
            log("size: back to the size before (the question was not answered Yes); nothing saved");
            size_changed(bar);
        }
        None => {}
    }
}

/// Switches to a new scale at once and remembers it in settings.txt (a whole size from the Size
/// menu, Bevel95 Settings' OK or Apply). A Size question still open is closed: its size is gone.
fn set_size(bar: HWND, scale: Scale) {
    let scale = Scale::from_quarters(scale.quarters());
    if with_app(|app| app.size_asking.forget()).unwrap_or(false) {
        bevel_winkit::confirm::close_asked();
    }
    // The size of the bar style in use, in its own line (the owner, 2026-11-25).
    let style = with_app(|app| {
        app.scale_setting = Some(scale);
        app.style_scales.set_scale_of(app.bar_style, Some(scale));
        app.bar_style
    })
    .unwrap_or_default();
    save_setting(config::scale_key(style), &scale.label());
    log(&format!("size: {scale}x ({})", config::scale_key(style)));
    size_changed(bar);
}

/// Bevel95 Settings: a size between whole numbers is drawn another way from now on, remembered
/// in settings.txt (at a whole size nothing shows of it).
fn set_scaling(bar: HWND, way: bevel_core::scale::Way) {
    with_app(|app| app.scaling = way);
    save_setting("scaling", way.setting());
    log(&format!("a size between whole numbers is drawn {} now", way.setting()));
    size_changed(bar);
}

/// After a change of the size or of its way of drawing: the bar, its desktop and every program
/// of Bevel95's take it.
fn size_changed(bar: HWND) {
    reposition(bar);
    refresh(bar);
    // Bevel95's other programs take the new size too: they read settings.txt again.
    kit::tell_programs_look_changed();
    // So do Taskbar Properties and Bevel95 Settings, if they are open.
    properties_window::look_changed();
    find_window::look_changed();
    about_window::look_changed();
    crate::display_window::look_changed();
}

/// Theme menu: switches to another look at once (its colors, sizes and font) and remembers
/// it in settings.txt.
fn set_era(bar: HWND, era: Era, style: theme::BarStyle) {
    // Windows XP's bar style goes with the 2000 look (brief 112).
    let era = if style.is_later() { Era::Win2000 } else { era };
    let Some((apply, write_style)) = with_app(|app| theme::look_chosen((app.era, app.bar_style), app.saved_style, (era, style))) else {
        return;
    };
    // Written only when it differs from what settings.txt says, so a settings.txt that never had
    // the line does not get one; also when nothing changes on screen (the classic look shown
    // chosen in the run after a failure: the review of brief 114, finding 5).
    if write_style {
        with_app(|app| app.saved_style = style);
        save_setting("bar_style", style.setting());
        log(&format!("look: the {} style is kept for the next start", theme::look_title(era, style)));
    }
    if !apply {
        return;
    }
    // Each style has its own size (the owner, 2026-11-25): a Size question still open belongs to
    // the style left (its answer does nothing now), and the new style's size goes on
    // (`reposition` below reserves the screen's strip for it).
    let forgot = with_app(|app| {
        let forgot = app.size_asking.forget();
        app.bar_style = style;
        app.scale_setting = app.style_scales.scale_of(style);
        forgot
    })
    .unwrap_or(false);
    if forgot {
        log("size: the question about a size was left unanswered by a change of style; nothing saved");
    }
    crate::failsafe::style_now(style);
    // Back to a classic look: the picture store forgets everything and sleeps (brief 115).
    crate::seven_store::set_later(style.is_later());
    start_menu::set_own_programs(!style.is_later());
    // Windows 7 always combines a program's windows (brief 114); the setting is kept as it is.
    with_app(|app| app.task_list.group = app.group || style == theme::BarStyle::Seven);
    start_menu::set_era(era);
    let theme = theme::bar_theme(era, with_app(|app| app.colors).unwrap_or_default(), style);
    let regular = font::letters(theme.font_face, theme.font_px, false).with_period_glyphs();
    let bold = font::letters(theme.font_face, theme.font_px, true).with_period_glyphs();
    // A dialog open in the old look goes away, and so does a question of Quick Launch's (its
    // action with it: nothing is done); menus are already closed.
    dialogs::close();
    bevel_winkit::confirm::close_asked();
    let widest = widest_time(&regular);
    // The setting, not whether the band shows (under XP it never shows: the review of brief 112,
    // finding 1, an open Settings window was told "off" and saved it).
    let quick_launch = with_app(|app| {
        let before = app.quick_launch_setting();
        app.era = era;
        app.theme = theme;
        app.regular = regular;
        app.bold = bold;
        app.clock_widest = widest;
        app.layout = None;
        app.ql.hot = None;
        let after = app.quick_launch_setting();
        (before != after).then_some(after)
    })
    .flatten();
    // A look whose default differs (95 off, 98 and 2000 on) while settings.txt does not say.
    if let Some(on) = quick_launch {
        properties_window::quick_launch_changed(on);
    }
    // Under XP the classic look the person had stays in settings.txt as it was (the review of
    // brief 112, finding 9): XP runs over 2000 whatever `era` says, and the earlier look is
    // there to see on the way back.
    if !style.is_later() {
        save_setting("era", era.setting());
    }
    log(&format!("look: {}", theme::look_title(era, style)));
    // The desktop takes the new look's color.
    sync_desktop();
    reposition(bar);
    refresh(bar);
    // Bevel95's other programs take the new look too: they read settings.txt again.
    kit::tell_programs_look_changed();
    // So do Taskbar Properties and Bevel95 Settings, if they are open.
    properties_window::look_changed();
    find_window::look_changed();
    about_window::look_changed();
    crate::display_window::look_changed();
}

/// Colors menu: switches between True Color, 256 Colors and 16 Colors at once (the look's colors, patterns
/// and pictures; sizes and font stay) and remembers it in settings.txt.
fn set_colors(bar: HWND, depth: Depth) {
    if xp_refuses("the Colors choice") {
        return;
    }
    let Some(era) = with_app(|app| (app.colors != depth).then_some(app.era)).flatten() else {
        return;
    };
    // A dialog open in the old colors goes away, as for a new look, and so does a question of
    // Quick Launch's (nothing of it is done); menus are already closed.
    dialogs::close();
    bevel_winkit::confirm::close_asked();
    with_app(|app| {
        app.colors = depth;
        app.theme = theme::bar_theme(era, depth, app.bar_style);
        app.layout = None;
    });
    save_setting("colors", depth.setting());
    log(&format!("colors: {}", depth.title()));
    sync_desktop();
    reposition(bar);
    refresh(bar);
    // SAFETY: our own window.
    unsafe { InvalidateRect(bar, null(), 0) };
    kit::tell_programs_look_changed();
    properties_window::look_changed();
    find_window::look_changed();
    about_window::look_changed();
    crate::display_window::look_changed();
}

/// Monitor Lines as they should be now (brief 102): shown while the bar is Bevel95's and showing
/// (not switched to Windows' taskbar, not stepped behind a full-screen program, the session not
/// ending, no panic), at a whole size from 2 up, with a strength other than Off (a live test's,
/// while it lasts, else the setting's). Called wherever one of these may change and on every tick;
/// it changes nothing when nothing changed. A busy state changes nothing either.
pub fn sync_lines() {
    use bevel_core::monitor_lines::{self as lines, Strength};
    let Some(wanted) = with_app(|app| {
        let strength = app.monitor_lines_test.map_or(app.monitor_lines, |(s, _)| s);
        let shows = app.ours
            && !app.ending
            && !app.fullscreen.is_behind()
            && app.monitor_lines_paused.is_none()
            && !bevel_winkit::panic::happened();
        let period = lines::period(app.scale)?;
        let m = app.monitor;
        (shows && strength != Strength::Off && rect_w(&m) > 0 && rect_h(&m) > 0).then_some(crate::monitor_lines::Wanted {
            left: m.left,
            top: m.top,
            width: rect_w(&m),
            height: rect_h(&m),
            period,
            strength,
        })
    }) else {
        return;
    };
    crate::monitor_lines::set(wanted);
}

/// The Monitor Lines menus and Quick Search (`from_menu`), or Bevel95 Settings: the lines put on
/// (where they can show) and saved; an open settings window shows a menu's choice.
fn set_monitor_lines(strength: bevel_core::monitor_lines::Strength, from_menu: bool) {
    let Some(changed) = with_app(|app| {
        // A choice ends a live test's lines too.
        app.monitor_lines_test = None;
        std::mem::replace(&mut app.monitor_lines, strength) != strength
    }) else {
        return;
    };
    if changed {
        save_setting("monitor_lines", strength.setting());
        let possible = with_app(|app| bevel_core::monitor_lines::possible(app.scale)).unwrap_or(false);
        log(&format!(
            "monitor lines: set to {}{}",
            strength.setting(),
            if possible { "" } else { " (they show once the size is a whole number from 2 up)" }
        ));
        if from_menu {
            properties_window::monitor_lines_changed(strength);
        }
    }
    sync_lines();
}

/// `WM_TEST_MONITOR_LINES`: a live test's lines on (wParam 1 to 3) or off (0), never saved; answers
/// the lines' window, if one shows. A test lasts at most `MONITOR_LINES_TEST_SECS` from its first
/// message (a later "on" changes the strength, never the end), and a new one can begin only
/// `MONITOR_LINES_TEST_GAP_SECS` after the last one began (brief 104, N5). Logged only when it did
/// something.
fn test_monitor_lines(wparam: usize) -> LRESULT {
    use bevel_core::monitor_lines::Strength;
    let strength = match wparam {
        1..=3 => Strength::ALL.get(wparam).copied(),
        _ => None,
    };
    let done = with_app(|app| {
        let running = app.monitor_lines_test.is_some();
        match strength {
            Some(s) if running => {
                app.monitor_lines_test = Some((s, Instant::now()));
                Some(format!("monitor lines: the live test shows {} now (not saved)", s.setting()))
            }
            Some(s) => {
                let may = app.monitor_lines_test_began.is_none_or(|at| at.elapsed().as_secs() >= MONITOR_LINES_TEST_GAP_SECS);
                may.then(|| {
                    app.monitor_lines_test = Some((s, Instant::now()));
                    app.monitor_lines_test_began = Some(Instant::now());
                    format!("monitor lines: a live test's {} for at most {MONITOR_LINES_TEST_SECS} s (not saved)", s.setting())
                })
            }
            None => app.monitor_lines_test.take().map(|_| "monitor lines: the live test's lines ended".to_string()),
        }
    })
    .flatten();
    if let Some(line) = done {
        log(&line);
        sync_lines();
    }
    crate::monitor_lines::window() as LRESULT
}

/// `WM_PAUSE_MONITOR_LINES`: a screen-photographing helper's pause (wParam 1) or its end (0).
/// Nothing is saved; the pause ends by itself after `MONITOR_LINES_PAUSE_SECS`. Answers 1 if the
/// lines' window was there or a pause was on (the helper then knows to wait for the screen).
fn pause_monitor_lines(wparam: usize) -> LRESULT {
    let showing = !crate::monitor_lines::window().is_null();
    let Some((was_paused, changed)) = with_app(|app| {
        let was = app.monitor_lines_paused.is_some();
        let changed = match wparam {
            // Only while the lines show: with them off a photograph needs nothing.
            1 if !was && showing => {
                app.monitor_lines_paused = Some(Instant::now());
                true
            }
            0 if was => {
                app.monitor_lines_paused = None;
                true
            }
            _ => false,
        };
        (was, changed)
    }) else {
        return 0;
    };
    if changed {
        log(if wparam == 1 { "monitor lines: paused for a photograph (at most 60 s)" } else { "monitor lines: the pause ended" });
        sync_lines();
    }
    LRESULT::from(showing || was_paused)
}

/// Ends this bar and starts a fresh one: through the watchdog if it is running (it restores
/// the Windows taskbar, then starts the new bar), else directly.
fn restart(bar: HWND) {
    let watchdog = with_app(|app| app.watchdog).flatten();
    if watchdog.is_some_and(watchdog::is_running) {
        with_app(|app| app.exit_code = watchdog::RESTART_EXIT_CODE);
        log("menu: restart (the watchdog will start the new bar)");
    } else {
        // SAFETY: plain Win32 calls; the handle is closed right away.
        let pid = unsafe { GetCurrentProcessId() };
        if let Some((handle, _)) = watchdog::spawn(&format!("--after {pid}")) {
            unsafe { CloseHandle(handle) };
        }
        log("menu: restart (no watchdog: started the new bar directly)");
    }
    // SAFETY: our own window; WM_DESTROY does the cleanup.
    unsafe { DestroyWindow(bar) };
}

fn on_destroy(bar: HWND) {
    // The buttons' order (as dragged) for the bar that starts next (a restart, an update).
    save_task_order();
    // The Monitor Lines go first, for good: nothing of them outlives the bar's window (a file job
    // may keep the process a while).
    crate::monitor_lines::end();
    popup::close(false);
    volume_popup::close(false);
    dialogs::close();
    properties_window::close();
    about_window::close();
    // A question of Quick Launch's goes too, doing nothing (with a file job still running the
    // process outlives this window, and the question would stay as a window that does not answer).
    bevel_winkit::confirm::close_asked();
    desktop::hide();
    quick_launch::stop();
    tip::destroy();
    keys::uninstall();
    // SAFETY: our own window, timers, hotkey and shell hook.
    unsafe {
        KillTimer(bar, TICK);
        KillTimer(bar, SLIDE);
        KillTimer(bar, TIP_SHOW);
        KillTimer(bar, TIP_HIDE);
        KillTimer(bar, MOTION);
        UnregisterHotKey(bar, HOTKEY_TOGGLE);
        if DESKTOP_KEY.swap(false, Ordering::SeqCst) {
            UnregisterHotKey(bar, HOTKEY_DESKTOP);
        }
        DeregisterShellHookWindow(bar);
    }
    // Our notification area goes first, so the calls below reach Windows' taskbar directly;
    // once it is back, programs are asked to send it their icons (it has them already).
    // Bevel95's own notification icon (there while switched to Windows' taskbar) goes too.
    own_icon::hide(bar);
    with_app(|app| app.ours = false);
    tray::stop();
    appbar::remove(bar);
    explorer::restore();
    tray::broadcast_taskbar_created();
    BAR_HWND.store(0, Ordering::SeqCst);
    let code = with_app(|app| app.exit_code).unwrap_or(0);
    // Before the window goes (the watchdog takes a window gone for "the bar has given up its
    // place"): a Restart's mark, then the place itself.
    if code == watchdog::RESTART_EXIT_CODE {
        mark_restart();
    }
    give_up_place();
    log(&format!("bar closed (exit code {code}); its place is free for the next bar"));
    // SAFETY: ends our message loop.
    unsafe { PostQuitMessage(code as i32) };
}

unsafe extern "system" fn bar_proc(bar: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let ours = || with_app(|app| app.ours).unwrap_or(false);
    match msg {
        // Clicking the bar must not take the focus from the app you are using.
        WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            paint_bar(bar);
            return 0;
        }
        WM_REPAINT => {
            // SAFETY: our own window.
            unsafe { InvalidateRect(bar, null(), 0) };
            return 0;
        }
        WM_TIMER if wparam == TICK => {
            tick(bar);
            show_settings_notice();
            return 0;
        }
        WM_TIMER if wparam == SLIDE => {
            slide_step(bar);
            return 0;
        }
        WM_TASK_AT => {
            let window = task_at(wparam as i32, lparam as i32)
                .and_then(|i| with_app(|app| app.groups.get(i).and_then(|g| g.windows.first().copied())).flatten());
            return window.unwrap_or(0) as LRESULT;
        }
        WM_TIMER if wparam == MOTION => {
            motion_step(bar);
            return 0;
        }
        WM_TIMER if wparam == TIP_SHOW => {
            show_tip(bar);
            return 0;
        }
        WM_TIMER if wparam == TIP_HIDE => {
            hide_tip(bar);
            return 0;
        }
        WM_VOLUME_CHANGED => {
            let muted = audio::Volume::open().is_some_and(|v| v.muted());
            if with_app(|app| std::mem::replace(&mut app.volume_muted, muted) != muted).unwrap_or(false) {
                // SAFETY: our own window.
                unsafe { InvalidateRect(bar, null(), 0) };
            }
            return 0;
        }
        m if m == WM_OPEN_PROPERTIES.id() => {
            properties_window::open();
            return 0;
        }
        m if m == wm_desktop_command() => {
            let (ticket, by_keyboard) = desktop::command_of_param(lparam);
            run_desktop_command(wparam, by_keyboard, ticket);
            return 0;
        }
        // A rename on the classic desktop changes a file's extension: the look's warning.
        m if m == desktop::WM_DESKTOP_RENAME_ASK.id() => {
            // lParam 1: a name that cannot be a file's name (the look's message); else the
            // extension warning.
            if lparam == 1 {
                crate::confirm_window::bad_name();
            } else {
                crate::confirm_window::ask_rename(wparam == 1);
            }
            return 0;
        }
        // A press on the classic desktop (posted by its own thread): Windows' flyouts, our
        // menus and the volume control close, as at a press on the bar.
        m if m == desktop::WM_DESKTOP_PRESS.id() => {
            crate::find_window::close_for_a_press();
            close_flyout(bar);
            popup::close(false);
            volume_popup::close(false);
            // Read after the menus closed: the program in front now is the one the person left.
            let front = window_list::active_window() as u64;
            with_app(|app| app.left_for_desktop = (front != 0).then_some(front));
            // Its button is drawn raised at once (the buttons are built again).
            refresh(bar);
            return 0;
        }
        m if m == desktop::wm_desktop_menu() => {
            popup::open_desktop_menu(bar, wparam as isize as i32, lparam as i32);
            return 0;
        }
        m if m == desktop::wm_desktop_item_menu() => {
            let (x, y) = desktop::point_of_param(lparam);
            popup::open_desktop_item_menu(bar, x, y, wparam as u32);
            return 0;
        }
        m if m == WM_SHORTCUT_WIZARD_ENDED.id() => {
            crate::add_shortcut::wizard_ended();
            return 0;
        }
        WM_HOTKEY if wparam as i32 == HOTKEY_TOGGLE => {
            popup::close(false);
            toggle(bar);
            return 0;
        }
        WM_HOTKEY if wparam as i32 == HOTKEY_DESKTOP => {
            toggle_desktop();
            return 0;
        }
        WM_LBUTTONDOWN => {
            forget_tray_press();
            // The window in front at the press, read before anything closes (closing Quick
            // Search, a flyout, a menu or the volume control hands the front to the program under
            // it): what a task button's click decides from (brief 90, the review's R1).
            let (seen, front_now) = front_at_press();
            crate::find_window::close_for_a_press();
            // First of all, a Windows flyout in front closes (and the network icon's own list
            // is not opened again by the press that closed it).
            let closed_flyout = close_flyout(bar);
            let was_open = popup::start_open();
            // A button's list closes at this press, as a click outside it would; remember
            // whose it was, so that a click on that same button does not open it again.
            let list_open = popup::window_list_open();
            // The same for Windows 2000's Quick Launch list: a press on its "»" only closes it.
            let quick_list_was_open = popup::quick_list_open();
            popup::close(false);
            // The bar never takes the focus, so a click on it does not reach the volume
            // control as a click elsewhere would (Windows' own taskbar did take it): it closes
            // the volume control here. A click on the speaker while it is open just closes it,
            // unless it follows the opening click closely: then the two are a double-click.
            let volume_was_open = volume_popup::close(false);
            let (x, y) = point(lparam);
            if let Some(key) = tray_icon_at(x, y) {
                if !flyout::icon_press_acts(key == NETWORK_KEY, closed_flyout) {
                    hide_tip(bar);
                } else if !(volume_was_open && tray::is_volume(key)) {
                    press_tray(bar, key, tray::Button::Left);
                } else if speaker_double_click() {
                    volume_popup::open_mixer();
                }
                return 0;
            }
            hide_tip(bar);
            // The band's grip, its arrows, its "»".
            if press_quick_part(bar, x, y, quick_list_was_open) {
                return 0;
            }
            // A Quick Launch icon sinks while held; it acts when the button comes up over it.
            // It is remembered by what it is (`quick_launch::Held`), not only by its place.
            if let Some(i) = quick_at(x, y) {
                let held = with_app(|app| {
                    app.ql.held = app.ql.target(i).map(|target| bevel_core::quick_launch::Held { index: i, target });
                    app.ql.held.is_some()
                });
                if held == Some(true) {
                    // SAFETY: our own window; capture lets us see the release even off the bar.
                    unsafe {
                        SetCapture(bar);
                        InvalidateRect(bar, null(), 0);
                    }
                }
                return 0;
            }
            // A double-click on the clock opens Date/Time Properties, as in Windows.
            if on_clock(x, y) {
                if clock_double_click() {
                    datetime_window::open();
                }
                return 0;
            }
            // The Start menu opens as the button goes down (and a second click closes it),
            // as in Windows 95.
            if on_start_button(x, y) {
                if !was_open {
                    popup::open_start(bar, false);
                }
                return 0;
            }
            // A scroll arrow acts as it goes down (like Windows' up-down control) and stays
            // pushed in until the button comes up.
            if let Some(arrow) = scroll_arrow_at(x, y) {
                with_app(|app| app.scroll_pressed = Some(arrow));
                scroll_tasks(bar, arrow);
                // SAFETY: our own window.
                unsafe { SetCapture(bar) };
                return 0;
            }
            // Windows 7's Show desktop and a pinned program that is not running (brief 114).
            if task_at(x, y).is_none()
                && let Some(extra) = seven_extra_at(x, y)
            {
                match extra {
                    SevenExtra::ShowDesktop => quick_launch::show_desktop(),
                    // Held like a Quick Launch icon: started at the release over the same slot
                    // (the review of brief 114, finding 4).
                    SevenExtra::Pinned(link) => {
                        PINNED_PRESS.with(|p| *p.borrow_mut() = Some(link));
                        // SAFETY: our own window; capture lets us see the release even off it.
                        unsafe { SetCapture(bar) };
                    }
                }
                return 0;
            }
            if let Some(index) = task_at(x, y) {
                with_app(|app| {
                    app.pressed = Some(index);
                    app.pressed_inside = true;
                    // The windows of a button belong to it alone, so one shared window is
                    // enough to tell its list (a window may have come or gone meanwhile).
                    app.pressed_closed_list = list_open.as_ref().is_some_and(|list| {
                        app.groups.get(index).is_some_and(|g| g.windows.iter().any(|id| list.contains(id)))
                    });
                    // Followed until the release: a click, or a drag (`drag_move`).
                    let windows = app.groups.get(index).map(|g| g.windows.clone()).unwrap_or_default();
                    // Where on the button it was pressed: that point stays under the pointer
                    // if it is dragged.
                    let left = app.layout.as_ref().and_then(|l| l.tasks.get(index.checked_sub(l.first_task)?)).map_or(0, |r| r.x);
                    let grab = app.scale.old(x) - left;
                    app.press = Some(ButtonPress::new(windows, x, app.task_list.order().to_vec(), grab));
                    app.press_front = Some((seen, front_now));
                });
                // SAFETY: our own window; capture lets us see the release even off the bar.
                unsafe {
                    SetCapture(bar);
                    InvalidateRect(bar, null(), 0);
                }
            } else {
                // The bar's empty space (as on Windows' own taskbar, nothing happens): counted, so
                // a press that "did nothing" can be told apart from one on a part (brief 90).
                static ON_NOTHING: AtomicU32 = AtomicU32::new(0);
                bevel_winkit::log::counted(&ON_NOTHING, "bar: a press on nothing that acts");
            }
            return 0;
        }
        WM_MOUSEMOVE => {
            // The pointer reached an auto-hidden bar's strip: up it comes.
            slide(bar, true);
            let (x, y) = point(lparam);
            arm_leave(bar);
            // The band's grip held: only its drag.
            if drag_quick_width(bar, x) {
                return 0;
            }
            set_hover(bar, hover_at(x, y));
            set_quick_hot(bar, quick_at(x, y));
            set_quick_hot_control(bar, match quick_part_at(x, y) {
                Some(bevel_core::quick_launch::Part::Control(c)) => Some(c),
                _ => None,
            });
            drag_move(bar, x);
            if let Some(index) = with_app(|app| app.pressed.filter(|_| !app.press.as_ref().is_some_and(|p| p.dragging)))
                .flatten()
            {
                let inside = task_at(x, y) == Some(index);
                if with_app(|app| std::mem::replace(&mut app.pressed_inside, inside) != inside).unwrap_or(false) {
                    // SAFETY: our own window.
                    unsafe { InvalidateRect(bar, null(), 0) };
                }
            }
            return 0;
        }
        // Over the band's grip, and while it is dragged: the pointer of a band's grip.
        WM_SETCURSOR if (lparam as u32 & 0xffff) == HTCLIENT => {
            let at = cursor();
            if let Some((left, top)) = with_app(|app| (app.placed.left, app.placed.top))
                && let Some(pointer) = quick_pointer(at.x - left, at.y - top)
                && bevel_winkit::pointer::show(pointer)
            {
                return 1;
            }
        }
        WM_MOUSELEAVE => {
            with_app(|app| app.tracking = false);
            set_hover(bar, None);
            set_quick_hot(bar, None);
            set_quick_hot_control(bar, None);
            return 0;
        }
        WM_LBUTTONUP => {
            let (x, y) = point(lparam);
            // A pinned program of Windows 7's bar: started only at the release over its own slot,
            // and once for a double click (the review of brief 114, finding 4).
            if let Some(link) = PINNED_PRESS.with(|p| p.borrow_mut().take()) {
                give_back_mouse(bar);
                let over = matches!(seven_extra_at(x, y), Some(SevenExtra::Pinned(l)) if l == link);
                if over && pinned_start_due(&link, std::time::Instant::now()) {
                    window_list::launch(&link);
                }
                return 0;
            }
            if release_tray(bar, tray::Button::Left, x, y) {
                return 0;
            }
            // The band's grip: the drag ends, the width is remembered; then the mouse goes back.
            if end_quick_drag(bar, "released") {
                give_back_mouse(bar);
                return 0;
            }
            // A 98 arrow pops back up (it scrolled at the press).
            if with_app(|app| app.ql.pressed_control.take()).flatten().is_some() {
                give_back_mouse(bar);
                // SAFETY: our own window.
                unsafe { InvalidateRect(bar, null(), 0) };
                return 0;
            }
            // A Quick Launch icon held down: the mouse goes back whatever happened to the band
            // meanwhile, and one click only if that very icon is under the pointer now.
            if let Some(held) = with_app(|app| app.ql.held.take()).flatten() {
                let under = quick_at(x, y).and_then(|i| with_app(|app| app.ql.target(i)).flatten());
                let release = bevel_core::quick_launch::release(Some(held), under);
                if release.give_back {
                    give_back_mouse(bar);
                }
                // SAFETY: our own window.
                unsafe { InvalidateRect(bar, null(), 0) };
                if let Some(target) = release.start {
                    click_quick(&target);
                }
                return 0;
            }
            if with_app(|app| app.scroll_pressed.take()).flatten().is_some() {
                // SAFETY: our own window.
                unsafe {
                    ReleaseCapture();
                    InvalidateRect(bar, null(), 0);
                }
                return 0;
            }
            let Some((index, press, from, count)) = with_app(|app| {
                let index = app.pressed.take()?;
                Some((index, app.press.take(), app.drag_from.take(), app.groups.len()))
            })
            .flatten() else {
                // Nothing of the bar's was held: the mouse must not stay with the bar all the same.
                give_back_mouse(bar);
                return 0;
            };
            keys::set_dragging(false);
            // SAFETY: our own window.
            unsafe {
                ReleaseCapture();
                InvalidateRect(bar, null(), 0);
            }
            let order = with_app(|app| app.task_list.order().to_vec()).unwrap_or_default();
            let past_threshold = press.as_ref().is_some_and(ButtonPress::went_past_threshold);
            let on_button = task_at(x, y) == Some(index);
            let release = press.map_or(if on_button { Release::Click } else { Release::Nothing }, |p| p.release(&order, on_button));
            match release {
                // A drag: the button stays where it was dropped; nothing is clicked.
                Release::Dropped => {
                    let from = from.map_or(String::from("?"), |f| (f + 1).to_string());
                    log(&format!("task buttons: a drag moved a button from place {from} to {} of {count}", index + 1));
                    save_task_order();
                }
                // Let go on the button in its own place: a click, also past the drag threshold and
                // back (brief 90).
                Release::Click => {
                    if past_threshold {
                        static BACK: AtomicU32 = AtomicU32::new(0);
                        bevel_winkit::log::counted(&BACK, "task buttons: a press went past the drag threshold, the button stayed in its place: a click");
                    }
                    click_task(bar, index);
                }
                // Let go elsewhere: nothing (the review of brief 90, R2).
                Release::Nothing => {
                    with_app(|app| app.press_front = None);
                }
            }
            return 0;
        }
        WM_CAPTURECHANGED => {
            // A drag of the band's grip that loses the mouse ends where it is; the band keeps
            // that width. An arrow held down pops back up.
            end_quick_drag(bar, "the bar lost the mouse");
            if with_app(|app| app.ql.pressed_control.take()).flatten().is_some() {
                // SAFETY: our own window.
                unsafe { InvalidateRect(bar, null(), 0) };
            }
            // A drag that loses the mouse (another window took it) puts its button back.
            let (held, dragged) = with_app(|app| {
                app.tray_pressed = None;
                app.drag_from = None;
                app.press_front = None;
                let dragged = match app.press.take() {
                    Some(press) if press.dragging => {
                        app.task_list.set_order(press.saved);
                        regroup(app);
                        true
                    }
                    _ => false,
                };
                (app.pressed.take().is_some() | app.scroll_pressed.take().is_some() | app.ql.held.take().is_some(), dragged)
            })
            .unwrap_or((false, false));
            if dragged {
                keys::set_dragging(false);
                log("task buttons: a drag lost the mouse; the button went back");
            }
            if held {
                // SAFETY: our own window.
                unsafe { InvalidateRect(bar, null(), 0) };
            }
            return 0;
        }
        m if m == keys::WM_DRAG_ESC.id() => {
            if with_app(|app| app.press.as_ref().is_some_and(|p| p.dragging)).unwrap_or(false) {
                end_press(bar, true);
                log("task buttons: Esc ended a drag; the button went back");
            }
            return 0;
        }
        WM_MBUTTONDOWN => {
            forget_tray_press();
            crate::find_window::close_for_a_press();
            close_flyout(bar);
            popup::close(false);
            volume_popup::close(false);
            let (x, y) = point(lparam);
            if let Some(key) = tray_icon_at(x, y) {
                press_tray(bar, key, tray::Button::Middle);
            }
            return 0;
        }
        WM_MBUTTONUP => {
            let (x, y) = point(lparam);
            if release_tray(bar, tray::Button::Middle, x, y) {
                return 0;
            }
            // Under Windows 7's style the middle button does nothing in this version (brief 114).
            if let Some(index) = task_at(x, y).filter(|_| !with_app(|app| app.bar_style == theme::BarStyle::Seven).unwrap_or(false)) {
                launch_task(index);
            }
            return 0;
        }
        WM_RBUTTONDOWN => {
            forget_tray_press();
            crate::find_window::close_for_a_press();
            close_flyout(bar);
            popup::close(false);
            volume_popup::close(false);
            let (x, y) = point(lparam);
            match tray_icon_at(x, y) {
                Some(key) => press_tray(bar, key, tray::Button::Right),
                None => hide_tip(bar),
            }
            return 0;
        }
        WM_RBUTTONUP => {
            let (x, y) = point(lparam);
            if release_tray(bar, tray::Button::Right, x, y) {
                return 0;
            }
            let at = cursor();
            if let Some(i) = quick_at(x, y) {
                popup::open_quick_item_menu(bar, i, at.x, at.y);
                return 0;
            }
            if on_band(x, y) {
                popup::open_band_menu(bar, at.x, at.y);
                return 0;
            }
            match task_at(x, y) {
                Some(index) => popup::open_task_menu(bar, index, at.x, at.y),
                None => popup::open(bar, at.x, at.y, on_clock(x, y)),
            }
            return 0;
        }
        m if m == WM_TEST_LONG_MENU.id() => {
            popup::open_test_long_menu(bar, wparam.clamp(1, 500));
            return 0;
        }
        m if m == WM_BAR_COMMAND.id() => {
            run_command(bar, wparam, lparam != 0);
            return 0;
        }
        m if m == WM_BAR_SIZE.id() => {
            match lparam {
                SIZE_KEEP | SIZE_BACK => size_answered(bar, wparam, lparam == SIZE_KEEP),
                _ => {
                    if let Some(scale) = size_of_message(wparam, lparam) {
                        choose_size(bar, scale);
                    }
                }
            }
            return 0;
        }
        m if m == WM_BAR_THEME.id() => {
            // The Theme menu's entry (`theme::LOOKS`: 3 is Windows XP).
            if let Some(&(era, style)) = theme::LOOKS.get(wparam) {
                // A look chosen: the mark of a failed style goes (`failsafe`).
                crate::failsafe::clear();
                set_era(bar, era, style);
            }
            return 0;
        }
        m if m == WM_DISPLAY.id() => {
            crate::display_window::test_message(wparam, lparam);
            return 0;
        }
        m if m == crate::pictures::WM_PICTURE.id() => {
            crate::display_window::pictures_came();
            return 0;
        }
        m if m == crate::pictures::WM_PICTURE_AGAIN.id() => {
            crate::display_window::desktop_needs_again(wparam as u64);
            return 0;
        }
        m if m == crate::wallpaper_browse::WM_BROWSED.id() => {
            crate::display_window::browsed(wparam as u64);
            return 0;
        }
        m if m == WM_QUICK_LAUNCH.id() => {
            match wparam {
                100 => quick_launch::read_again(),
                // The leak check of Windows 2000's list (brief 70): open it (every icon of the band
                // where none is left out), and close it again. Neither starts anything.
                101 => popup::open_quick_list(bar, true),
                102 if popup::quick_list_open() => popup::close(true),
                _ => {}
            }
            return 0;
        }
        m if m == WM_BAR_COLORS.id() => {
            if let Some(&depth) = Depth::ALL.get(wparam) {
                set_colors(bar, depth);
            }
            return 0;
        }
        m if m == WM_BAR_MONITOR_LINES.id() => {
            if let Some(strength) = bar_menu::monitor_lines_at(wparam) {
                set_monitor_lines(strength, true);
            }
            return 0;
        }
        m if m == WM_TEST_MONITOR_LINES.id() => return test_monitor_lines(wparam),
        m if m == WM_PAUSE_MONITOR_LINES.id() => return pause_monitor_lines(wparam),
        // Bevel95's own windows came up (a companion program's, or one of another thread of the
        // bar's): the lines go where they belong. Does nothing else, whoever posts it.
        m if m == bevel_winkit::on_top::WM_RAISE_LINES.id() => {
            crate::monitor_lines::raise();
            return 0;
        }
        m if m == WM_START_ACTION.id() => {
            start_action(wparam);
            return 0;
        }
        // The Windows key (wParam: the key) with windows_key = find: Find opens, or closes
        // when it is the window in front, or comes forward when it is behind. An open menu of
        // ours closes first. (How the keyboard hook reports the key is unchanged.)
        m if m == keys::WM_START_KEY.id() && wparam != 0 && with_app(|app| app.windows_key == WindowsKey::Find).unwrap_or(false) => {
            let find_in_front = find_window::in_front();
            popup::close(false);
            volume_popup::close(false);
            if find_in_front {
                find_window::close();
            } else if ours() {
                open_find();
            }
            return 0;
        }
        find_window::WM_FIND_PROGRAMS_READ => {
            find_window::programs_read();
            return 0;
        }
        // The Windows key or Ctrl+Esc (see keys.rs): open or close the Start menu.
        m if m == keys::WM_START_KEY.id() => {
            if popup::start_open() {
                popup::close(true);
            } else if ours() {
                bring_up(bar);
                popup::open_start(bar, true);
            }
            return 0;
        }
        m if m == WM_APPBAR.id() => {
            match wparam as u32 {
                ABN_POSCHANGED if ours() => reposition(bar),
                ABN_FULLSCREENAPP => {
                    log(&format!("shell: fullscreen app {}", if lparam != 0 { "opened" } else { "closed" }))
                }
                _ => {}
            }
            return 0;
        }
        WM_WINDOWPOSCHANGED => appbar::window_pos_changed(bar),
        // The time format may have changed: the clock's widest time is worked out again.
        WM_SETTINGCHANGE if ours() => {
            with_app(|app| {
                app.clock_widest = widest_time(&app.regular);
                app.clock = clock_text(app.clock_offset);
            });
            reposition(bar);
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED if ours() => reposition(bar),
        // Signing out or shutting down: put the Windows taskbar setting back while Explorer
        // is surely still running. The 100 ms tick leaves it alone from here on, or it would
        // hide it again (and switch its auto-hide back on) before the session ends.
        WM_QUERYENDSESSION => {
            log("the session may end: restoring the Windows taskbar");
            with_app(|app| app.ending = true);
            explorer::restore();
            // The Monitor Lines go with the bar (they come back if the session goes on).
            sync_lines();
            return 1;
        }
        WM_ENDSESSION => {
            if wparam != 0 {
                // Once more (safe to repeat), in case anything changed it since.
                explorer::restore();
            } else {
                with_app(|app| app.ending = false);
                if ours() {
                    log("the session goes on: hiding the Windows taskbar again");
                    explorer::hide();
                }
            }
            return 0;
        }
        WM_DESTROY => {
            on_destroy(bar);
            return 0;
        }
        _ if msg != 0 && msg == SHELL_HOOK.load(Ordering::SeqCst) => {
            let id = lparam as u64;
            match wparam as u32 {
                HSHELL_FLASH => {
                    with_app(|app| app.flashing.insert(id));
                }
                // Its title or icon changed: read the icon again.
                HSHELL_REDRAW => {
                    with_app(|app| app.icons.remove(&id));
                }
                HSHELL_WINDOWACTIVATED | HSHELL_RUDEAPPACTIVATED => {
                    with_app(|app| app.flashing.remove(&id));
                }
                HSHELL_WINDOWCREATED | HSHELL_WINDOWDESTROYED => {}
                // The next tick (within 0.1 s) steps behind it or comes back.
                HSHELL_FULLSCREENENTER | HSHELL_FULLSCREENEXIT => {
                    let entered = wparam as u32 == HSHELL_FULLSCREENENTER;
                    // Which kind of window (no title): Windows reports the classic desktop too.
                    let window = lparam as HWND;
                    log(&format!("shell: {} {} fullscreen", window_list::kind_of(window), if entered { "entered" } else { "left" }));
                    with_app(|app| app.fullscreen.reported(lparam as HWND, entered));
                    return 0;
                }
                _ => return 0,
            }
            if ours() {
                refresh(bar);
            }
            return 0;
        }
        _ if msg != 0 && msg == TASKBAR_CREATED.load(Ordering::SeqCst) => {
            // Our own call for icons: nothing to do here.
            if tray::own_broadcast() {
                return 0;
            }
            // Explorer restarted: it forgot our reserved space and shows its taskbar again.
            // Programs have just sent their icons to whichever "Shell_TrayWnd" came first, maybe
            // Explorer's new one: ours goes in front again and asks once more.
            if ours() {
                log("Explorer restarted: hiding its taskbar and reserving our space again");
                explorer::hide();
                appbar::register(bar, WM_APPBAR.id());
                reposition(bar);
                tray::keep_first();
                tray::broadcast_taskbar_created();
                tray::ask_again_soon();
            } else if own_icon::wanted() {
                // Switched to Windows' taskbar: Bevel95's icon goes to the new one.
                log("Explorer restarted: Bevel95's notification icon goes to Windows' taskbar again");
                let dpi = with_app(|app| app.dpi).unwrap_or(96);
                own_icon::show(bar, dpi);
            }
            return 0;
        }
        m if m == own_icon::WM_OWN_ICON.id() => {
            own_icon_event(bar, wparam, lparam);
            return 0;
        }
        _ => {}
    }
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(bar, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bar's messages that do or change something are registered by name, each its own, and
    /// listed in decisions.md with their old numbers; the only `WM_APP + n` left in the bar's
    /// files are those that only paint, read or answer (the fourth safety review of 2026-11-18,
    /// C3: a stray broadcast of `WM_APP + 3` with a command's code closed or restarted the bar).
    #[test]
    fn the_bars_messages_are_registered_by_name() {
        let decisions = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/decisions.md")).unwrap();
        let decisions = decisions.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut names: Vec<&str> = BAR_MESSAGES.iter().map(|(m, _)| m.name()).collect();
        for (message, old) in &BAR_MESSAGES {
            assert!(message.name().starts_with("Bevel95.Bar."), "{}", message.name());
            let listed = |end: &str| decisions.contains(&format!("`{}` (WM_APP + {old}{end}", message.name()));
            assert!(listed(")") || listed(","), "{} is listed in decisions.md", message.name());
        }
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), BAR_MESSAGES.len(), "each name its own");
        let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src/");
        let mut left: Vec<String> = Vec::new();
        for file in ["bar.rs", "keys.rs", "own_icon.rs", "popup.rs", "find_window.rs", "start_menu.rs", "add_shortcut.rs"] {
            for line in std::fs::read_to_string(format!("{src}{file}")).unwrap().lines() {
                let code = line.split("//").next().unwrap_or("");
                let declared = ["const ", "pub const "].iter().any(|d| code.trim_start().starts_with(d));
                if declared && code.contains("WM_APP + ") {
                    let name = code.split_whitespace().skip_while(|w| *w != "const").nth(1).unwrap_or(code);
                    left.push(name.trim_end_matches(':').to_string());
                }
            }
        }
        left.sort();
        assert_eq!(left, ["WM_FIND_PROGRAMS_READ", "WM_RELEASE_WIN", "WM_REPAINT", "WM_TASK_AT", "WM_VOLUME_CHANGED"]);
    }

    /// One bar holds the place; once it is given up (closed), a new bar takes it at once, while the
    /// old process still runs (the second safety review of 2026-11-18, B1).
    #[test]
    fn a_place_given_up_is_free_for_the_next_bar() {
        let name = format!("Local\\Bevel95.Test.Bar.{}", std::process::id());
        let first = take_place(&name).expect("the place is free");
        assert!(take_place(&name).is_none(), "a second bar meets the first");
        // SAFETY: the handle taken above.
        unsafe { CloseHandle(first as windows_sys::Win32::Foundation::HANDLE) };
        let next = take_place(&name).expect("given up: the next bar takes it");
        // SAFETY: as above.
        unsafe { CloseHandle(next as windows_sys::Win32::Foundation::HANDLE) };
    }

    /// settings.txt that cannot be read (UTF-16, as Notepad can save it) is kept byte for byte: the
    /// bar runs on the defaults and a setting changed meanwhile is not saved over it (it was
    /// replaced by the defaults: the safety review of 2026-11-18, B1).
    #[test]
    fn an_unreadable_settings_file_is_never_replaced() {
        let dir = std::env::temp_dir().join(format!("bevel95-bar-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.txt");
        let utf16: Vec<u8> =
            [0xFEFFu16].into_iter().chain("era = win2000\r\nscale = 3\r\n".encode_utf16()).flat_map(u16::to_le_bytes).collect();
        std::fs::write(&path, &utf16).unwrap();
        assert_eq!(settings_at(&path), Config::default());
        assert_eq!(std::fs::read(&path).unwrap(), utf16, "kept after the load");
        assert!(state_file::change_settings(&path, |t| config::with_setting(t, "scale", "2")).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), utf16, "kept after a save");
        // With a byte-order mark (UTF-8): read as if it were not there.
        std::fs::write(&path, b"\xEF\xBB\xBFera = win2000\r\n").unwrap();
        assert_eq!(settings_at(&path).era, bevel_core::theme::Era::Win2000);
        // Missing: created with the defaults.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(settings_at(&path), Config::default());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), config::DEFAULT_FILE);
        // Readable but not Bevel95's (no mark): read for this run, never written (the third
        // safety review of 2026-11-18, A1: a setting changed wrote Bevel95's keys into it).
        let theirs: &[u8] = b"[Options]\r\nscale = 3\r\n";
        std::fs::write(&path, theirs).unwrap();
        let _ = settings_at(&path);
        assert!(state_file::change_settings(&path, |t| config::with_setting(t, "era", "win98")).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), theirs, "their file byte for byte");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

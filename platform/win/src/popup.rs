//! Menus as windows, painted by `bevel_core` in the classic style: the Start menu and the
//! menus that cascade from it, the bar's right-click menu (with its Theme and Size submenus),
//! the list of an app's windows, a task button's right-click menu, and the desktop's.
//!
//! Open menus form a stack: the first ("root") menu takes the focus, so Esc, the arrow keys
//! and access keys work and a click anywhere else closes everything, as Windows' own menus
//! do. Submenus open beside their entry (after a short hover, or at once on a click, the
//! Right arrow or an access key) and never take the focus themselves; keys always act on
//! the deepest menu.

use std::ptr::{null, null_mut};
use std::sync::atomic::Ordering;

use bevel_core::bar_menu::{self, BarCommand, BarEntry};
use bevel_core::fb::{Framebuffer, Icon, Rect};
use bevel_core::icons;
use bevel_core::layout::{place_popup, place_start_menu};
use bevel_core::paint::{self, EntryKind, Long, Menu, MenuEntry, MenuLayout, MenuStyle, Strip};
use bevel_core::start::LongKind;
use bevel_core::programs::{Entry, Folder, Item};
use bevel_core::scale::Scale;
use bevel_core::start::{self, DocumentsEntry, StartEntry};
use bevel_core::text::{escape_access_keys, split_access_key};
use bevel_core::theme::Era;
use bevel_core::volume;
use bevel_core::window_menu::{self, GroupCommand, WindowCommand};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateRectRgn, EndPaint, InvalidateRect, PAINTSTRUCT, SetWindowRgn, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_RETURN, VK_RIGHT,
    VK_SHIFT, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetForegroundWindow, IsWindow, KillTimer, MA_NOACTIVATE,
    PostMessageW, SW_SHOW, SW_SHOWNA, SetTimer, ShowWindow, WA_INACTIVE, WM_ACTIVATE, WM_CHAR,
    WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_PAINT, WM_TIMER, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::bar::{BAR_HWND, WM_BAR_COLORS, WM_BAR_COMMAND, WM_BAR_SIZE, WM_BAR_THEME, WM_START_ACTION, with_app};
use crate::log::{log, log_win32};
use crate::util::{blit, point, rect_h, rect_w, take_focus, wide};
use crate::{start_menu, window_list};

pub const MENU_CLASS: &str = "Bevel95Menu";

/// Hovering a submenu entry this long opens it (Windows' default menu show delay is 400).
const HOVER_MS: u32 = 300;
const HOVER_TIMER: usize = 1;
/// A scrolling menu steps while the pointer rests on one of its strips (`paint::scroll_pace`).
const SCROLL_TIMER: usize = 2;

/// What a menu is for, which decides what choosing an entry does.
#[derive(Clone, Debug)]
pub enum Kind {
    /// The bar's right-click menu: entries are `bar_menu` entries (true: right-clicked on the
    /// clock, so it starts with Adjust Date/Time).
    BarMenu(bool),
    /// Its Bevel95 submenu: entries are `bar_menu::own_entry` ones.
    BarOwn(bar_menu::Own),
    /// The Theme submenu: entry i is the look `bar_menu::theme_at(i)`.
    Theme,
    /// The Colors submenu: entry i is `bar_menu::colors_at(i)`.
    Colors,
    /// The Size submenu: entry i is size `bar_menu::size_at(i)`.
    Size,
    /// The Monitor Lines submenu (the bar's and the desktop's Bevel95 menus): entry i is
    /// `bar_menu::monitor_lines_at(i)`.
    MonitorLines,
    /// An app's windows: entry i brings window i to the front.
    Windows(Vec<u64>),
    /// A task button's right-click menu for its one window: entries are `window_menu` ones.
    WindowMenu(u64),
    /// A task button's right-click menu for its several windows: `window_menu` group entries.
    GroupMenu(Vec<u64>),
    /// The Start menu: entry i is `start::entry_at(era, i)`.
    Start,
    /// Windows XP's Start menu (brief 112): entry i is `xp_start::item_at(menu, i)`; the pinned
    /// programs' and the often-used ones' targets, in the menu's order.
    XpStart(Vec<String>, Vec<String>),
    /// Windows 7's Start menu (brief 114): entry i is `seven_start::item_at(menu, i)`; the
    /// often-used programs' targets, in the menu's order.
    SevenStart(Vec<String>),
    /// A folder of the Programs or Favorites menus, by entry indices from that menu down.
    Folder(Tree, Vec<usize>),
    /// Your recently opened documents.
    Documents(Vec<Item>),
    /// Settings, or Find (Search): entry i opens `start::fixed_target(entry, era, i)`.
    Fixed(StartEntry),
    /// The speaker's right-click menu: Open Volume Controls, Adjust Audio Properties.
    Speaker,
    /// The network icon's menu (`status::network_menu`) and the battery icon's.
    Network,
    Power,
    /// The right-click menu of Bevel95's desktop: entry i is `bevel_core::desktop::entry_at(era, i)`.
    Desktop,
    /// Its Arrange Icons, New and Bevel95 submenus.
    DesktopArrange,
    DesktopNew,
    DesktopOwn,
    /// The menu on the selected desktop icons, of these kinds: entry i is
    /// `bevel_core::desktop::item_at(kinds, i)`, carried out on the icons of the ticket
    /// (`desktop::targets_of`), the selection when the menu opened.
    DesktopItem(Vec<bevel_core::desktop::ItemKind>, u32),
    /// Bevel95's own icon's menu in Windows' notification area: entry i is
    /// `bevel_core::tray_icon::command_at(i)`.
    OwnIcon,
    /// The Toolbars submenu (98 and 2000): entry i is `bar_menu::toolbars_entry(i)`.
    Toolbars,
    /// The Quick Launch band's own menu: entry i is `quick_launch::band_entry(era, i)`.
    Band,
    /// The menu on a Quick Launch icon, by what the icon is (and whether it is the person's
    /// shortcut): entry i is `quick_launch::icon_entry(shortcut, i)`.
    QuickItem(crate::quick_launch::Target, bool),
    /// Windows 2000's list of the Quick Launch icons left out (its "»"): entry i starts the icon
    /// `targets[i]`, by what it is, and only while it is still on the band.
    QuickList(Vec<crate::quick_launch::Target>),
    /// A live test's long menu (`open_test_long_menu`), by its folders' indices from the top: its
    /// entries start nothing.
    Test(Vec<usize>),
}

/// The folder trees that cascade from the Start menu (`App::start_tree`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tree {
    Programs,
    /// Windows 98's Favorites.
    Favorites,
}

/// An open menu (one level of the stack).
pub struct Popup {
    hwnd: HWND,
    kind: Kind,
    menu: Menu,
    style: MenuStyle,
    /// Where its entries are, at 1×.
    layout: MenuLayout,
    /// Where the window is, in screen pixels.
    rect: Rect,
    /// For a submenu: the entry of the menu below it that opened it.
    parent_entry: Option<usize>,
    /// For the root menu: the window that had the focus before (to give it back).
    previous: HWND,
    /// WM_MOUSELEAVE tracking is armed.
    tracking: bool,
    /// XP's and 7's Start menus above size 1: the program pictures drawn at their final size over
    /// the enlarged menu (1x box, true picture; `start_finals`).
    finals: Vec<(Rect, bevel_core::fb::Icon)>,
}

impl Popup {
    pub fn is_start(&self) -> bool {
        matches!(self.kind, Kind::Start | Kind::XpStart(..) | Kind::SevenStart(..))
    }

    /// Windows 2000's Quick Launch list (its "»" is drawn pushed in while it is open).
    pub fn is_quick_list(&self) -> bool {
        matches!(self.kind, Kind::QuickList(_))
    }

    /// A menu whose opening changes how the bar looks (the Start button, the "»"): the bar is
    /// painted again when it closes.
    fn shows_on_bar(&self) -> bool {
        self.is_start() || self.is_quick_list()
    }
}

/// Where a root menu opens.
enum At {
    /// At a clicked point (flipping up/left when there is no room), like TrackPopupMenu.
    Point(i32, i32),
    /// The speaker's menu, for a click at a point: where that look's Windows opens it.
    Speaker(Era, i32, i32),
    /// A task button's list of windows, from the button's top-left corner (screen pixels): as
    /// `Point`, within the room above the bar (brief 94).
    TaskButton(i32, i32),
    /// The Start menu, on top of the Start button (screen pixels).
    StartButton(Rect),
    /// Upward from a point (Bevel95's icon on Windows' taskbar: `tray_icon::place_menu`).
    Above(i32, i32),
    /// Windows 2000's Quick Launch list, above its "»" (screen pixels: `quick_launch::place_list`).
    Chevron(Rect),
}

/// Opens the bar's right-click menu for a click at screen point (x, y); `clock`: the click was
/// on the clock.
pub fn open(bar: HWND, x: i32, y: i32, clock: bool) {
    let show_keys = with_app(|app| app.era.always_shows_keys()).unwrap_or(true);
    let era = with_app(|app| app.era).unwrap_or_default();
    let menu = Menu { show_keys, ..bar_menu::menu(era, clock) };
    open_root(bar, Kind::BarMenu(clock), menu, MenuStyle::Popup, At::Point(x, y));
}

/// Opens the Quick Launch band's own menu for a right-click on its grips, lines or gaps at screen
/// point (x, y).
pub fn open_band_menu(bar: HWND, x: i32, y: i32) {
    let Some(era) = with_app(|app| app.era) else {
        return;
    };
    let menu = Menu { show_keys: era.always_shows_keys(), ..bevel_core::quick_launch::band_menu(era) };
    open_root(bar, Kind::Band, menu, MenuStyle::Popup, At::Point(x, y));
}

/// Opens the menu on Quick Launch icon `index` (its place on the band) for a right-click at screen
/// point (x, y).
pub fn open_quick_item_menu(bar: HWND, index: usize, x: i32, y: i32) {
    let Some((era, target)) = with_app(|app| Some((app.era, app.ql.target(index)?))).flatten() else {
        return;
    };
    let shortcut = matches!(target, crate::quick_launch::Target::File(_));
    let menu = Menu { show_keys: era.always_shows_keys(), ..bevel_core::quick_launch::icon_menu(shortcut) };
    open_root(bar, Kind::QuickItem(target, shortcut), menu, MenuStyle::Popup, At::Point(x, y));
}

/// Opens Windows 2000's list of the Quick Launch icons left out, above the band's "»" (a press on
/// it; brief 70). `all`: every icon of the band instead (only for the leak check of
/// `bar::WM_QUICK_LAUNCH`, which opens the list also where nothing is left out; it starts nothing).
pub fn open_quick_list(bar: HWND, all: bool) {
    let Some((targets, menu, at)) = with_app(|app| {
        let band = app.quick_band()?;
        let rows: Vec<usize> = if all { (0..app.ql.items.len()).collect() } else { band.hidden.clone() };
        if rows.is_empty() {
            return None;
        }
        // Where the list opens: above the "»" (or, for the leak check without one, the band's end).
        let at = app.on_screen(band.chevron.unwrap_or(Rect::new(band.end_line, band.rect.y, 1, band.rect.h)));
        let pictures = app.ql.band(app.era, false).icons;
        let name = |i: usize| app.ql.tooltip(app.era, i).map(|t| t.lines().next().unwrap_or_default().to_string()).unwrap_or_default();
        // One list of (name, picture, target): a row whose icon has no target any more (the folder
        // shrank before the next paint) is left out whole, so no row can stand for another icon.
        let rows: Vec<(String, Option<Icon>, _)> =
            rows.iter().filter_map(|&i| Some((name(i), pictures.get(i).cloned().flatten(), app.ql.target(i)?))).collect();
        if rows.is_empty() {
            return None;
        }
        let (entries, targets): (Vec<_>, Vec<_>) = rows.into_iter().map(|(n, p, t)| ((n, p), t)).unzip();
        let all_names: Vec<String> = (0..app.ql.items.len()).map(name).collect();
        Some((targets, bevel_core::quick_launch::list_menu(entries, &all_names), at))
    })
    .flatten() else {
        return;
    };
    open_root(bar, Kind::QuickList(targets), menu, MenuStyle::Cascade, At::Chevron(at));
}

/// Whether Windows 2000's Quick Launch list is open.
pub fn quick_list_open() -> bool {
    with_app(|app| app.menus.first().is_some_and(Popup::is_quick_list)).unwrap_or(false)
}

/// Opens the list of windows of task button `index`, above the button.
pub fn open_window_list(bar: HWND, index: usize) {
    let Some((windows, menu, (x, y))) = with_app(|app| {
        let group = app.groups.get(index)?;
        let entries = group
            .windows
            .iter()
            .filter_map(|id| app.windows.iter().find(|w| w.id == *id))
            .map(|w| MenuEntry::item(&escape_access_keys(&w.title), app.window_icon(w.id)))
            .collect();
        Some((group.windows.clone(), Menu::new(entries), app.button_position(index)?))
    })
    .flatten() else {
        return;
    };
    open_root(bar, Kind::Windows(windows), menu, MenuStyle::Cascade, At::TaskButton(x, y));
}

/// Opens the right-click menu of task button `index` at screen point (x, y): its window's
/// own menu, or the group menu for a button with several windows.
pub fn open_task_menu(bar: HWND, index: usize, x: i32, y: i32) {
    let Some((windows, era)) = with_app(|app| Some((app.groups.get(index)?.windows.clone(), app.era))).flatten() else {
        return;
    };
    let show_keys = era.always_shows_keys();
    let (kind, menu) = match windows.as_slice() {
        [] => return,
        // Asking the window for its state talks to it, so it happens outside the app state.
        [only] => (Kind::WindowMenu(*only), window_menu::menu(era, window_list::window_state(*only as HWND))),
        _ => (Kind::GroupMenu(windows), window_menu::group_menu()),
    };
    open_root(bar, kind, Menu { show_keys, ..menu }, MenuStyle::Popup, At::Point(x, y));
}

/// Opens the speaker's right-click menu for a click at screen point (x, y).
pub fn open_speaker_menu(bar: HWND, x: i32, y: i32) {
    let Some(era) = with_app(|app| app.era) else {
        return;
    };
    let menu = volume::speaker_menu(era.always_shows_keys());
    open_root(bar, Kind::Speaker, menu, MenuStyle::Popup, At::Speaker(era, x, y));
}

/// Opens the network icon's (`network`) or the battery icon's right-click menu for a click at
/// screen point (x, y). The battery's opens where that look's speaker menu does (the same
/// program drew both: SYSTRAY.EXE, 2000's STOBJECT.DLL); the network's at the pointer.
pub fn open_status_menu(bar: HWND, network: bool, x: i32, y: i32) {
    let Some(era) = with_app(|app| app.era) else {
        return;
    };
    let keys = era.always_shows_keys();
    if network {
        open_root(bar, Kind::Network, bevel_core::status::network_menu(era, keys), MenuStyle::Popup, At::Point(x, y));
    } else {
        open_root(bar, Kind::Power, bevel_core::status::power_menu(era, keys), MenuStyle::Popup, At::Speaker(era, x, y));
    }
}

/// The desktop menu that is open holds Undo (its entries after it moved one down).
static DESKTOP_UNDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Opens the right-click menu of Bevel95's desktop for a click at screen point (x, y).
pub fn open_desktop_menu(bar: HWND, x: i32, y: i32) {
    let Some(era) = with_app(|app| app.era) else {
        return;
    };
    let can_paste = crate::file_ops::clipboard_has_files();
    // Undo shows while there is something to take back (the list the folder windows share).
    let undo = bevel_winkit::undo::last();
    DESKTOP_UNDO.store(undo.is_some(), std::sync::atomic::Ordering::SeqCst);
    let menu = Menu { show_keys: era.always_shows_keys(), ..bevel_core::desktop::menu_with_undo(era, can_paste, undo.as_ref()) };
    open_root(bar, Kind::Desktop, menu, MenuStyle::Popup, At::Point(x, y));
}

/// Opens the menu on the selected desktop icons (`desktop::targets_of(ticket)`) for a click at
/// screen point (x, y).
pub fn open_desktop_item_menu(bar: HWND, x: i32, y: i32, ticket: u32) {
    let Some(era) = with_app(|app| app.era) else {
        return;
    };
    let kinds: Vec<_> = crate::desktop::targets_of(ticket).iter().map(|t| t.kind).collect();
    if kinds.is_empty() {
        return;
    }
    let menu = Menu { show_keys: era.always_shows_keys(), ..bevel_core::desktop::item_menu(era, &kinds, crate::desktop::recycle_full()) };
    open_root(bar, Kind::DesktopItem(kinds, ticket), menu, MenuStyle::Popup, At::Point(x, y));
}

/// Opens the menu of Bevel95's own icon in Windows' notification area for a right click at
/// screen point (x, y), upward from it.
pub fn open_own_icon_menu(bar: HWND, x: i32, y: i32) {
    let Some(era) = with_app(|app| app.era) else {
        return;
    };
    let menu = Menu { show_keys: era.always_shows_keys(), ..bevel_core::tray_icon::menu() };
    open_root(bar, Kind::OwnIcon, menu, MenuStyle::Popup, At::Above(x, y));
}

/// Whether the Start menu is open (the Start button is then drawn pressed).
pub fn start_open() -> bool {
    with_app(|app| app.menus.first().is_some_and(Popup::is_start)).unwrap_or(false)
}

/// The windows of the task button whose list is open, if one is.
pub fn window_list_open() -> Option<Vec<u64>> {
    with_app(|app| match app.menus.first().map(|p| &p.kind) {
        Some(Kind::Windows(windows)) => Some(windows.clone()),
        _ => None,
    })
    .flatten()
}

/// Opens the Start menu above the Start button. Opened from the keyboard, Windows 2000 also
/// underlines the access keys (Windows 95 and 98 always do).
pub fn open_start(bar: HWND, by_keyboard: bool) {
    let Some((era, opts, small, button, shown)) = with_app(|app| {
        Some((app.era, app.start_options, app.options.small_icons, app.start_button_on_screen()?, app.programs_shown))
    })
    .flatten() else {
        return;
    };
    // The folder trees are read afresh each time, so new shortcuts show up. Windows' list of
    // apps is shown as last read and read again meanwhile, for the next time. What of it all
    // the Programs menu shows is the owner's choice (Bevel95 Settings, Programs page).
    let programs = start_menu::programs(shown);
    crate::apps::refresh();
    let favorites = if start::entries(era, opts).contains(&Some(StartEntry::Favorites)) {
        start_menu::favorites()
    } else {
        Folder::default()
    };
    with_app(|app| {
        app.start_programs = programs;
        app.start_favorites = favorites;
        // The scroll places of folders that are gone go with them (the list never grows without
        // end: brief 92).
        let mut places = std::mem::take(&mut app.menu_scroll);
        places.retain(|key, _| place_still_there(app, key));
        app.menu_scroll = places;
    });
    // The room above the Start button, in the look's pixels (the later Windows' menus shorten to
    // it: the review of brief 112, finding 3).
    let room = with_app(|app| app.scale.old_fit(button.y - app.monitor.top)).unwrap_or(bevel_core::xp_start::HEIGHT);
    if with_app(|app| app.bar_style == bevel_core::theme::BarStyle::Seven).unwrap_or(false) {
        let (kind, mut menu) = seven_start_menu(room);
        menu.show_keys = by_keyboard;
        open_root(bar, kind, menu, MenuStyle::SevenStart, At::StartButton(button));
        return;
    }
    if with_app(|app| app.xp()).unwrap_or(false) {
        let (kind, mut menu) = xp_start_menu(room);
        menu.show_keys = by_keyboard;
        open_root(bar, kind, menu, MenuStyle::XpStart, At::StartButton(button));
        return;
    }
    let (style, mut menu) = start_menu_shown(era, opts, small);
    menu.show_keys = era.always_shows_keys() || by_keyboard;
    open_root(bar, Kind::Start, menu, style, At::StartButton(button));
}

/// Windows XP's Start menu (brief 112): the Windows user's name, the shortcuts of Quick Launch as
/// the pinned programs, the programs started most often through Bevel95's Start menu and Quick
/// Search (`often::top`). The pictures: the shell's 16×16 ones doubled (PROVISIONAL: XP drew its
/// own 32×32 ones).
fn xp_start_menu(room: i32) -> (Kind, Menu) {
    use bevel_core::xp_start;
    let era = Era::Win2000;
    // Quick Launch's folder as last read, whatever its band shows (under XP it never paints, and
    // only its paint took the readings in: the review of brief 112, finding 2).
    let pinned: Vec<(String, String)> = with_app(|app| {
        let era = app.era;
        app.ql.refresh(era);
        app.ql.shortcuts.iter().map(|s| (bevel_core::quick_launch::shortcut_tooltip(&s.name).to_string(), s.path.display().to_string())).collect()
    })
    .unwrap_or_default();
    let often: Vec<(String, String)> = crate::often::top(6);
    // Only those that fit get their pictures read (the review's doubt 14).
    let (p, o, ..) = xp_start::fit(pinned.len(), often.len(), room);
    let (pinned, often): (Vec<_>, Vec<_>) = (pinned.into_iter().take(p).collect(), often.into_iter().take(o).collect());
    let picture = |path: &str| start_menu::file_icon(era, path).map(|i| doubled(&i));
    let with_pictures = |list: &[(String, String)]| list.iter().map(|(name, path)| (name.clone(), picture(path))).collect::<Vec<_>>();
    let menu = xp_start::menu_in(&start_menu::user_name(), &with_pictures(&pinned), &with_pictures(&often), room);
    let kind = Kind::XpStart(pinned.into_iter().map(|(_, t)| t).collect(), often.into_iter().map(|(_, t)| t).collect());
    (kind, menu)
}

/// Windows 7's Start menu (brief 114): the Windows user's name, the programs started most often
/// (their shell's 32×32 pictures), for a room `room` rows high.
fn seven_start_menu(room: i32) -> (Kind, Menu) {
    use bevel_core::seven_start;
    let often: Vec<(String, String)> = crate::often::top(12);
    let (_, p, _) = seven_start::fit(often.len(), room);
    let often: Vec<(String, String)> = often.into_iter().take(p).collect();
    // The pictures as far as the store has read them (never read here, on the bar's thread: the
    // review of brief 114, finding 2); the ones wanted now are there at the next opening.
    let files: Vec<String> = often.iter().map(|(_, t)| t.clone()).collect();
    crate::seven_store::want(crate::seven_store::Asker::Menu, &files);
    let rows: Vec<(String, Option<bevel_core::fb::Icon>)> = often.iter().map(|(n, t)| (n.clone(), crate::seven_store::picture(t))).collect();
    let menu = seven_start::menu(&start_menu::user_name(), &rows, room);
    (Kind::SevenStart(often.into_iter().map(|(_, t)| t).collect()), menu)
}

/// A picture with every pixel doubled (the shell's 16×16 as XP's 32×32 places).
fn doubled(icon: &bevel_core::fb::Icon) -> bevel_core::fb::Icon {
    let (w, h) = (icon.width * 2, icon.height * 2);
    let pixels = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| icon.pixels[((y / 2) * icon.width + x / 2) as usize]).collect();
    bevel_core::fb::Icon { width: w, height: h, pixels }
}

/// The Start menu as the bar shows it: the person's user name in Log Off, each entry's picture;
/// "Show small icons in Start menu": 16×16 icons and no banner. Shared by `open_start` and the live
/// test's menu, so that the test menu stands where the real Programs menu would (brief 94).
fn start_menu_shown(era: Era, opts: bevel_core::config::StartOptions, small: bool) -> (MenuStyle, Menu) {
    let user = start_menu::user_name();
    if small {
        (MenuStyle::StartSmall, start::menu(era, opts, &user, |e| icons::start_icon_small(era, e)))
    } else {
        (MenuStyle::Start, start::menu(era, opts, &user, |e| icons::start_icon(era, e)))
    }
}

fn open_root(bar: HWND, kind: Kind, menu: Menu, style: MenuStyle, at: At) {
    close(false);
    // SAFETY: a plain query.
    let previous = unsafe { GetForegroundWindow() };
    let (long, offset) = long_of(&kind);
    // The look's rule for a long menu, columns wider than the whole screen scrolling instead
    // (`start::layout_root`, the same code the tests draw with). The menus of the Start menu's
    // family that open from the bar get the room above it, never the bar's rows (brief 94): the
    // Start menu (`start::place_start`), a task button's list and 2000's Quick Launch list.
    let Some((layout, scale, screen, room)) = with_app(|app| {
        let m = app.monitor;
        let screen = Rect::new(m.left, m.top, rect_w(&m), rect_h(&m));
        let room = match at {
            At::StartButton(button) => start::room_above(screen, button.y),
            At::TaskButton(_, y) => start::room_above(screen, y),
            At::Chevron(chevron) => start::room_above(screen, chevron.y),
            _ => screen,
        };
        let layout = match at {
            At::StartButton(button) => {
                start::place_start(app.theme, &app.fonts(), app.era, &menu, style, long, offset, app.scale, screen, button).0
            }
            _ => start::layout_root(app.theme, &app.fonts(), app.era, &menu, style, long, offset, app.scale, room),
        };
        (layout, app.scale, screen, room)
    }) else {
        return;
    };
    if style != MenuStyle::Popup && layout.scroll.is_some() && !matches!(long, Long::Scroll { .. }) {
        log(&format!("a menu of {} entries did not fit the screen in columns: it scrolls", menu.entries.len()));
    }
    let (w, h) = (scale.len(layout.width), scale.len(layout.height));
    let (left, top) = match at {
        At::Point(x, y) => place_popup(x, y, w, h, screen),
        At::TaskButton(x, y) => place_popup(x, y, w, h, room),
        At::Speaker(era, x, y) => volume::place_menu(era, x, y, w, h, screen),
        At::StartButton(button) => place_start_menu(button, w, h, room),
        At::Above(x, y) => bevel_core::tray_icon::place_menu(x, y, w, h, screen),
        At::Chevron(chevron) => bevel_core::quick_launch::place_list(chevron, w, h, room),
    };
    show_root(bar, kind, menu, style, layout, Rect::new(left, top, w, h), previous);
}

/// XP's and 7's Start menus at every size (the owner, 2026-11-25; at size 1 too: the shell's own
/// small pictures come at the screen's scale and were shrunk crudely): each program row's picture
/// at its final size (`Scale::rect` of its 1x box), averaged from the large one, as far as the
/// store has read it (never read here, on the bar's thread); those found are taken out of `menu`,
/// so the 1x painting leaves their boxes empty. The ones not read yet keep their 1x picture this
/// time; the next opening has them. The classic menus and the submenus never get here.
fn start_finals(kind: &Kind, menu: &mut Menu, layout: &MenuLayout) -> Vec<(Rect, bevel_core::fb::Icon)> {
    use crate::seven_store::{self, Asker};
    let Some(scale) = with_app(|app| app.scale) else {
        return Vec::new();
    };
    let boxes: Vec<(usize, Rect, String)> = match kind {
        Kind::XpStart(pinned, often) => bevel_core::xp_start::live_icon_boxes(menu, layout)
            .into_iter()
            .filter_map(|(i, r)| {
                let target = match bevel_core::xp_start::item_at(menu, i)? {
                    bevel_core::xp_start::Item::Pinned(j) => pinned.get(j)?,
                    bevel_core::xp_start::Item::Often(j) => often.get(j)?,
                    _ => return None,
                };
                Some((i, r, target.clone()))
            })
            .collect(),
        Kind::SevenStart(often) => bevel_core::seven_start::live_icon_boxes(menu)
            .into_iter()
            .filter_map(|(i, r)| Some((i, r, often.get(i)?.clone())))
            .collect(),
        _ => return Vec::new(),
    };
    let mut wanted = Vec::new();
    let mut finals = Vec::new();
    for (i, r, target) in boxes {
        let side = scale.rect(r).w;
        if let Some(icon) = seven_store::picture_at(&target, side) {
            if let Some(entry) = menu.entries.get_mut(i) {
                entry.icon = None;
            }
            finals.push((r, icon));
        }
        wanted.push((target, side));
    }
    seven_store::want_sized(Asker::MenuSized, &wanted);
    finals
}

/// Shows a root menu laid out and placed (screen pixels), taking the focus.
fn show_root(bar: HWND, kind: Kind, menu: Menu, style: MenuStyle, layout: MenuLayout, rect: Rect, previous: HWND) {
    let Some(hwnd) = create(bar, rect, false) else {
        return;
    };
    let start = matches!(kind, Kind::Start | Kind::XpStart(..) | Kind::SevenStart(..) | Kind::QuickList(_));
    let mut menu = menu;
    let finals = start_finals(&kind, &mut menu, &layout);
    let popup = Popup { hwnd, kind, menu, style, layout, rect, parent_entry: None, previous, tracking: false, finals };
    with_app(|app| app.menus.push(popup));
    // SAFETY: our own windows.
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        take_focus(hwnd);
        if start {
            InvalidateRect(bar, null(), 0); // the Start button (or the "»") is drawn pressed now
        }
    }
}

/// The live test's long menu (`bar::WM_TEST_LONG_MENU`, brief 92): two folders ("Long Folder",
/// 60 items; "Test Folder", 5 items), then `n` entries "Entry 000" and on, all made up. Laid out and
/// placed by the look's own rule as the Programs menu beside the Start menu would be (the Start menu
/// itself is not shown: it holds the person's own words). Its entries start nothing.
pub fn open_test_long_menu(bar: HWND, n: usize) {
    close(false);
    // SAFETY: a plain query.
    let previous = unsafe { GetForegroundWindow() };
    let Some((era, small, button, opts)) =
        with_app(|app| Some((app.era, app.options.small_icons, app.start_button_on_screen()?, app.start_options))).flatten()
    else {
        return;
    };
    let menu = test_menu(era, &[], n);
    let long = start::long_rule(era, LongKind::Programs, opts.scroll_programs);
    // The Start menu the bar would show (the person's name read, never logged), laid out and placed
    // by the same code as `open_root`'s (brief 94); it is not shown.
    let (style, start_menu) = start_menu_shown(era, opts, small);
    let (start_long, start_offset) = long_of(&Kind::Start);
    let Some((layout, rect)) = with_app(|app| {
        let m = app.monitor;
        let screen = Rect::new(m.left, m.top, rect_w(&m), rect_h(&m));
        let s = app.scale;
        // Where the Start menu would stand, and its Programs entry where it shows.
        let (start_layout, x, y) =
            start::place_start(app.theme, &app.fonts(), era, &start_menu, style, start_long, start_offset, s, screen, button);
        let (w, h) = (s.len(start_layout.width), s.len(start_layout.height));
        let parent = Rect::new(x, y, w, h);
        let programs = start::entries(era, opts).iter().position(|e| *e == Some(StartEntry::Programs))?;
        let r = s.rect(start_layout.shown_rect(programs)?);
        let entry = Rect::new(x + r.x, y + r.y, r.w, r.h);
        let (layout, left, top) = start::place_cascade(app.theme, &app.fonts(), era, &menu, MenuStyle::Cascade, long, 0, s, screen, parent, entry, false);
        let rect = Rect::new(left, top, s.len(layout.width), s.len(layout.height));
        Some((layout, rect))
    })
    .flatten() else {
        return;
    };
    log(&format!("test long menu: opened with {n} entries ({} columns{})", layout.columns.len(), if layout.scroll.is_some() { ", scrolling" } else { "" }));
    show_root(bar, Kind::Test(Vec::new()), menu, MenuStyle::Cascade, layout, rect, previous);
}

/// The live test's made-up menus (`open_test_long_menu`): at the top, two folders and `n` entries;
/// in "Long Folder" 60 items, in "Test Folder" 5.
fn test_menu(era: Era, path: &[usize], n: usize) -> Menu {
    let group = icons::program_group(era);
    let entries: Vec<MenuEntry> = match path {
        [] => {
            let mut e = vec![MenuEntry::submenu("Long Folder", Some(group.clone())), MenuEntry::submenu("Test Folder", Some(group))];
            e.extend((0..n).map(|i| MenuEntry::item(&format!("Entry {i:03}"), None)));
            e
        }
        [0] => (0..60).map(|i| MenuEntry::item(&format!("Item {i:02}"), None)).collect(),
        _ => (0..5).map(|i| MenuEntry::item(&format!("Test {i}"), None)).collect(),
    };
    Menu { show_keys: true, ..Menu::new(entries) }
}

/// The submenu that entry `index` of the menu at `level` opens: what it is, its entries and
/// its style. Reading icons talks to the shell, so it happens outside the app state.
fn submenu_of(level: usize, index: usize) -> Option<(Kind, Menu, MenuStyle)> {
    enum Want {
        Ready(Kind, Menu, MenuStyle),
        Folder(Tree, Vec<usize>, Folder),
        Documents,
        Fixed(StartEntry),
    }
    let (want, era, show_keys) = with_app(|app| {
        let parent = app.menus.get(level)?;
        if parent.menu.entries.get(index)?.kind != EntryKind::Submenu {
            return None;
        }
        let want = match &parent.kind {
            Kind::BarMenu(clock) if bar_menu::entry(app.era, *clock, index) == Some(BarEntry::Bevel95) => {
                let own = bar_menu::Own { desktop: app.desktop, desktop_key: crate::bar::desktop_key() };
                Want::Ready(Kind::BarOwn(own), bar_menu::own_menu_in(own, app.xp()), MenuStyle::Popup)
            }
            Kind::BarMenu(clock) if bar_menu::entry(app.era, *clock, index) == Some(BarEntry::Toolbars) => {
                Want::Ready(Kind::Toolbars, bar_menu::toolbars_menu_in(app.quick_launch_on(), app.xp()), MenuStyle::Popup)
            }
            Kind::Band if bevel_core::quick_launch::band_entry(app.era, index) == Some(bevel_core::quick_launch::BandEntry::Toolbars) => {
                Want::Ready(Kind::Toolbars, bar_menu::toolbars_menu(app.quick_launch_on()), MenuStyle::Popup)
            }
            Kind::BarOwn(own) if bar_menu::own_entry(*own, index) == Some(BarEntry::Theme) => {
                Want::Ready(Kind::Theme, bar_menu::theme_menu(app.era, app.bar_style, app.later_looks), MenuStyle::Popup)
            }
            Kind::BarOwn(own) if bar_menu::own_entry(*own, index) == Some(BarEntry::Colors) => {
                Want::Ready(Kind::Colors, bar_menu::colors_menu(app.colors), MenuStyle::Popup)
            }
            Kind::BarOwn(own) if bar_menu::own_entry(*own, index) == Some(BarEntry::Size) => {
                Want::Ready(Kind::Size, bar_menu::size_menu(app.scale), MenuStyle::Popup)
            }
            Kind::BarOwn(own) if bar_menu::own_entry(*own, index) == Some(BarEntry::MonitorLines) => {
                Want::Ready(Kind::MonitorLines, bar_menu::monitor_lines_menu(app.monitor_lines, app.scale), MenuStyle::Popup)
            }
            Kind::DesktopOwn if bevel_core::desktop::own_entry(index) == Some(bevel_core::desktop::OwnEntry::MonitorLines) => {
                Want::Ready(Kind::MonitorLines, bar_menu::monitor_lines_menu(app.monitor_lines, app.scale), MenuStyle::Popup)
            }
            Kind::Desktop => match bevel_core::desktop::entry_at_with_undo(app.era, DESKTOP_UNDO.load(std::sync::atomic::Ordering::SeqCst), index)? {
                bevel_core::desktop::Entry::ArrangeIcons => {
                    Want::Ready(Kind::DesktopArrange, bevel_core::desktop::arrange_menu(crate::desktop::auto_arrange()), MenuStyle::Popup)
                }
                bevel_core::desktop::Entry::New => Want::Ready(Kind::DesktopNew, bevel_core::desktop::new_menu(), MenuStyle::Popup),
                bevel_core::desktop::Entry::Bevel95 => {
                    Want::Ready(Kind::DesktopOwn, bevel_core::desktop::own_menu(crate::bar::desktop_key()), MenuStyle::Popup)
                },
                bevel_core::desktop::Entry::Command(_) => return None,
            },
            Kind::Start => match start::entry_at(app.era, app.start_options, index)? {
                StartEntry::Programs => Want::Folder(Tree::Programs, Vec::new(), app.start_programs.clone()),
                StartEntry::Favorites => Want::Folder(Tree::Favorites, Vec::new(), app.start_favorites.clone()),
                StartEntry::Documents => Want::Documents,
                e @ (StartEntry::Settings | StartEntry::Find) => Want::Fixed(e),
                _ => return None,
            },
            // XP's All Programs and My Recent Documents: the classic cascades (2000's look for now,
            // PROVISIONAL).
            Kind::XpStart(..) => match bevel_core::xp_start::item_at(&parent.menu, index)? {
                bevel_core::xp_start::Item::AllPrograms => Want::Folder(Tree::Programs, Vec::new(), app.start_programs.clone()),
                bevel_core::xp_start::Item::Right(bevel_core::xp_start::Right::MyRecentDocuments) => Want::Documents,
                _ => return None,
            },
            // 7's All Programs: the classic Programs cascade beside it (PROVISIONAL).
            Kind::SevenStart(..) => match bevel_core::seven_start::item_at(&parent.menu, index)? {
                bevel_core::seven_start::Item::AllPrograms => Want::Folder(Tree::Programs, Vec::new(), app.start_programs.clone()),
                _ => return None,
            },
            Kind::Test(path) => {
                let mut path = path.clone();
                path.push(index);
                Want::Ready(Kind::Test(path.clone()), test_menu(app.era, &path, 0), MenuStyle::Cascade)
            }
            Kind::Folder(tree, path) => {
                let mut path = path.clone();
                path.push(index);
                let folder = app.start_tree(*tree).folder_at(&path)?.clone();
                Want::Folder(*tree, path, folder)
            }
            _ => return None,
        };
        Some((want, app.era, parent.menu.show_keys))
    })
    .flatten()?;
    let started = std::time::Instant::now();
    let (kind, mut menu, style) = match want {
        Want::Ready(kind, menu, style) => (kind, menu, style),
        Want::Folder(tree, path, folder) => {
            let folder_icon = match tree {
                Tree::Programs => icons::program_group(era),
                Tree::Favorites => icons::favorites_folder(),
            };
            let menu = folder.menu(Some(&folder_icon), |item| start_menu::file_icon(era, &item.target));
            (Kind::Folder(tree, path), menu, MenuStyle::Cascade)
        }
        Want::Documents => {
            let documents = start_menu::recent_documents(era);
            let my_documents = icons::my_documents(era);
            let menu =
                start::documents_menu(era, &documents, my_documents, |item| start_menu::file_icon(era, &item.target));
            (Kind::Documents(documents), menu, MenuStyle::Cascade)
        }
        Want::Fixed(entry) => (Kind::Fixed(entry), start::fixed_menu(entry, era), MenuStyle::Cascade),
    };
    let took = started.elapsed().as_millis();
    if took > 250 {
        log(&format!("a submenu of {} entries took {took} ms to read (icons from the shell)", menu.entries.len()));
    }
    menu.show_keys = show_keys;
    Some((kind, menu, style))
}

/// Opens the submenu of entry `index` of the menu at `level` (closing deeper ones). An entry that
/// a scrolling menu shows not, or not whole (a key chose it, or the strips scrolled it away
/// after the pointer did), is first brought into view, and the place remembered (brief 92).
fn open_child(level: usize, index: usize, highlight_first: bool) {
    close_from(level + 1);
    let Some((kind, mut menu, style)) = submenu_of(level, index) else {
        return;
    };
    let moved = with_app(|app| {
        let parent = app.menus.get_mut(level)?;
        if !parent.layout.reveal(index) {
            return None;
        }
        let offset = parent.layout.scroll.map_or(0, |s| s.offset);
        let hwnd = parent.hwnd;
        remember(app, level, offset);
        Some(hwnd)
    })
    .flatten();
    if let Some(hwnd) = moved {
        // SAFETY: our own window.
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
    let Some((parent_rect, entry_rect, in_columns, bar)) = with_app(|app| {
        let parent = app.menus.get(level)?;
        let e = parent.layout.shown_rect(index)?;
        // Beside the entry's own column only in a menu of the Start menu's family (Windows' own
        // popup menus keep their old placement: brief 92).
        let in_columns = parent.layout.columns.len() > 1 && parent.style != MenuStyle::Popup;
        let r = app.scale.rect(e);
        let entry = Rect::new(parent.rect.x + r.x, parent.rect.y + r.y, r.w, r.h);
        Some((parent.rect, entry, in_columns, BAR_HWND.load(Ordering::SeqCst) as HWND))
    })
    .flatten() else {
        return;
    };
    if highlight_first {
        menu.hover = first_entry(&menu, style, true);
    }
    let (long, offset) = long_of(&kind);
    // Laid out and placed by `start::place_cascade`, the same code the golden pictures draw with.
    let Some((layout, left, top, scale)) = with_app(|app| {
        let m = app.monitor;
        let screen = Rect::new(m.left, m.top, rect_w(&m), rect_h(&m));
        let (layout, x, y) =
            start::place_cascade(app.theme, &app.fonts(), app.era, &menu, style, long, offset, app.scale, screen, parent_rect, entry_rect, in_columns);
        (layout, x, y, app.scale)
    }) else {
        return;
    };
    if layout.scroll.is_some() && !matches!(long, Long::Scroll { .. }) {
        log(&format!("a menu of {} entries did not fit the screen in columns: it scrolls", menu.entries.len()));
    }
    let (w, h) = (scale.len(layout.width), scale.len(layout.height));
    let rect = Rect::new(left, top, w, h);
    let Some(hwnd) = create(bar, rect, true) else {
        return;
    };
    let popup = Popup {
        hwnd,
        kind,
        menu,
        style,
        layout,
        rect,
        parent_entry: Some(index),
        previous: null_mut(),
        tracking: false,
        finals: Vec::new(),
    };
    with_app(|app| app.menus.push(popup));
    // Opened by a key on its first entry: a scrolling menu shows it.
    if highlight_first {
        reveal_hover(level + 1);
    }
    // SAFETY: our own window, shown without taking the focus from the root menu.
    unsafe { ShowWindow(hwnd, SW_SHOWNA) };
    // Under the Monitor Lines at once (brief 102).
    crate::monitor_lines::raise();
}

/// Which of the Start menu's long menus `kind` is (`start::long_rule`).
fn long_kind(kind: &Kind) -> LongKind {
    match kind {
        Kind::Folder(Tree::Programs, _) => LongKind::Programs,
        Kind::Folder(Tree::Favorites, _) => LongKind::Favorites,
        Kind::Test(_) => LongKind::Programs,
        _ => LongKind::Other,
    }
}

/// How `kind` goes on when it is longer than the screen in the bar's look, and where it was last
/// scrolled to (Windows 98 reopened a menu where it was left: survey part 1, 3).
fn long_of(kind: &Kind) -> (Long, i32) {
    with_app(|app| {
        let long = start::long_rule(app.era, long_kind(kind), app.start_options.scroll_programs);
        let offset = scroll_key(app, kind).and_then(|key| app.menu_scroll.get(&key).copied()).unwrap_or(0);
        (long, offset)
    })
    .unwrap_or((Long::Columns { own_widths: true }, 0))
}

/// Between the names of a folder's path in a scroll place's key (no file name holds it).
const KEY_SEP: char = '\u{1F}';

/// The name under which a menu's scroll place is remembered while the bar runs: by what the menu
/// is, never by where it stands (brief 92): a folder by the names of its path in its tree, the
/// Start menu, Documents, Settings and Find by what they are. The lists of a task button or of
/// Quick Launch are not remembered. Never shown or logged.
fn scroll_key(app: &crate::bar::App, kind: &Kind) -> Option<String> {
    match kind {
        Kind::Folder(tree, path) => {
            let (root, name) = match tree {
                Tree::Programs => (&app.start_programs, "Programs"),
                Tree::Favorites => (&app.start_favorites, "Favorites"),
            };
            let mut key = name.to_string();
            for k in 1..=path.len() {
                key.push(KEY_SEP);
                key.push_str(&root.folder_at(&path[..k])?.name);
            }
            Some(key)
        }
        Kind::Start => Some("Start".to_string()),
        Kind::Documents(_) => Some("Documents".to_string()),
        Kind::Fixed(entry) => Some(format!("{entry:?}")),
        _ => None,
    }
}

/// Remembers where the menu at `level` is scrolled to (`scroll_key`).
fn remember(app: &mut crate::bar::App, level: usize, offset: i32) {
    if let Some(key) = app.menus.get(level).and_then(|p| scroll_key(app, &p.kind)) {
        app.menu_scroll.insert(key, offset);
    }
}

/// Whether the menu a scroll place's key names is still there (its folders read anew), by the exact
/// names the key was stored under (`scroll_key`), so a folder renamed only in case drops its place
/// (brief 94).
fn place_still_there(app: &crate::bar::App, key: &str) -> bool {
    let mut names = key.split(KEY_SEP);
    let root = match names.next() {
        Some("Programs") => &app.start_programs,
        Some("Favorites") => &app.start_favorites,
        _ => return true,
    };
    let mut folder = root;
    for name in names {
        match folder.folders.iter().find(|f| f.name == name) {
            Some(next) => folder = next,
            None => return false,
        }
    }
    true
}
fn create(owner: HWND, rect: Rect, submenu: bool) -> Option<HWND> {
    let class = wide(MENU_CLASS);
    let ex = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | if submenu { WS_EX_NOACTIVATE } else { 0 };
    // SAFETY: a registered class; the bar owns the menus so they stay above it.
    let hwnd = unsafe {
        CreateWindowExW(
            ex,
            class.as_ptr(),
            null(),
            WS_POPUP,
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            owner,
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        )
    };
    if hwnd.is_null() {
        log_win32("CreateWindowExW(menu)");
        return None;
    }
    Some(hwnd)
}

/// Closes every open menu. With `give_back_focus`, the window that had the focus before
/// gets it back (after Esc or a bar command; not when the user clicked another window or
/// started a program, which should keep the focus).
pub fn close(give_back_focus: bool) {
    let menus = with_app(|app| std::mem::take(&mut app.menus)).unwrap_or_default();
    let Some(root) = menus.first() else {
        return;
    };
    // SAFETY: plain Win32 calls; `previous` is checked to still exist.
    unsafe {
        for popup in menus.iter().rev() {
            DestroyWindow(popup.hwnd);
        }
        // The focus goes back only once the menus are gone: destroying the active menu hands the
        // activation to its owner, the bar, which took it from the window given the focus before
        // (found by brief 94's live try: the bar stayed in front). Brought forward the way the
        // menu came forward (`util::bring_forward`: direct, else with the input joined).
        if give_back_focus && !root.previous.is_null() && IsWindow(root.previous) != 0 {
            crate::util::bring_forward(root.previous);
        }
        if root.shows_on_bar() {
            InvalidateRect(BAR_HWND.load(Ordering::SeqCst) as HWND, null(), 0); // Start (or the "»") pops back up
        }
    }
}

/// Takes every open menu off the screen at once, as Windows does at a click, and pops the Start
/// button back up, without giving up the focus: the windows stay, with nothing left of their
/// shape, until `destroy`. So a program started in between may still come to the front, and
/// the menu no longer waits on the screen while the program starts.
fn vanish() -> Vec<Popup> {
    let menus = with_app(|app| std::mem::take(&mut app.menus)).unwrap_or_default();
    for popup in &menus {
        // SAFETY: our own windows; the system owns the region from here on.
        unsafe { SetWindowRgn(popup.hwnd, CreateRectRgn(0, 0, 0, 0), 1) };
    }
    if menus.first().is_some_and(Popup::shows_on_bar) {
        let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
        // SAFETY: our own window, painted now (the program may take a moment to start).
        unsafe {
            InvalidateRect(bar, null(), 0);
            UpdateWindow(bar);
        }
    }
    menus
}

/// Closes menus that `vanish` took off the screen (the focus is not given back).
fn destroy(menus: Vec<Popup>) {
    for popup in menus.iter().rev() {
        // SAFETY: our own windows.
        unsafe { DestroyWindow(popup.hwnd) };
    }
}

/// Closes the menus at `level` and deeper (keeps the ones below).
fn close_from(level: usize) {
    let closing = with_app(|app| app.menus.split_off(level.min(app.menus.len()))).unwrap_or_default();
    for popup in closing.iter().rev() {
        // SAFETY: our own windows.
        unsafe { DestroyWindow(popup.hwnd) };
    }
}

/// Which level of the stack `hwnd` is.
fn level_of(hwnd: HWND) -> Option<usize> {
    with_app(|app| app.menus.iter().position(|p| p.hwnd == hwnd)).flatten()
}

/// The deepest open menu (where the keys act).
fn deepest() -> Option<usize> {
    with_app(|app| app.menus.len().checked_sub(1)).flatten()
}

/// What choosing an entry of a menu does, beyond opening a submenu.
enum Action {
    /// A command of the bar's menu; true if Shift was held.
    BarCommand(BarCommand, bool),
    /// The Theme menu's entry (`bevel_core::theme::LOOKS`).
    Theme(usize),
    Colors(usize),
    Size(Scale),
    /// The Monitor Lines submenu's entry, by its index.
    MonitorLines(usize),
    Window(u64),
    WindowCommand(u64, WindowCommand),
    GroupCommand(Vec<u64>, GroupCommand),
    Open(String),
    /// A Start menu entry without a submenu (Run, Shut Down, ...), by its index.
    Start(usize),
    Target(start::Target),
    /// An entry of the speaker's menu, by its index.
    Speaker(usize),
    /// A desktop command, and the ticket of the icons it acts on (0: none).
    Desktop(bevel_core::desktop::DesktopCommand, u32),
    /// An entry of the menu on a Quick Launch icon, and the icon.
    QuickItem(crate::quick_launch::Target, bevel_core::quick_launch::IconEntry),
}

/// Runs entry `index` of the menu at `level`.
fn choose(level: usize, index: usize) {
    let Some((kind, entry_kind, era, opts, xp_item, seven_item)) = with_app(|app| {
        let popup = app.menus.get(level)?;
        let entry = popup.menu.entries.get(index).filter(|e| e.choosable())?;
        let xp_item = matches!(popup.kind, Kind::XpStart(..)).then(|| bevel_core::xp_start::item_at(&popup.menu, index)).flatten();
        let seven_item = matches!(popup.kind, Kind::SevenStart(..)).then(|| bevel_core::seven_start::item_at(&popup.menu, index)).flatten();
        Some((popup.kind.clone(), entry.kind, app.era, app.start_options, xp_item, seven_item))
    })
    .flatten() else {
        return; // a separator or a grayed-out entry: nothing happens
    };
    if entry_kind == EntryKind::Submenu {
        open_child(level, index, false);
        return;
    }
    // The live test's menu: its entries start nothing, and the window that was in front gets the
    // focus back, as after Esc (brief 94).
    if let Kind::Test(path) = &kind {
        close(true);
        log(&format!("test long menu: entry {index} of level {} chosen (it starts nothing)", path.len()));
        return;
    }
    let action = match kind {
        Kind::BarMenu(_) | Kind::BarOwn(_) => {
            let entry = match kind {
                Kind::BarMenu(clock) => bar_menu::entry(era, clock, index),
                Kind::BarOwn(own) => bar_menu::own_entry(own, index),
                _ => None,
            };
            let Some(BarEntry::Command(command)) = entry else {
                return;
            };
            // SAFETY: a plain query.
            let shift = unsafe { GetKeyState(i32::from(VK_SHIFT)) } < 0;
            Action::BarCommand(command, shift)
        }
        Kind::Theme => match bar_menu::theme_at(index) {
            Some(_) => Action::Theme(index),
            None => return,
        },
        Kind::Colors => match bar_menu::colors_at(index) {
            Some(_) => Action::Colors(index),
            None => return,
        },
        Kind::Size => match bar_menu::size_at(index) {
            Some(scale) => Action::Size(scale),
            None => return,
        },
        Kind::MonitorLines => match bar_menu::monitor_lines_at(index) {
            Some(_) => Action::MonitorLines(index),
            None => return,
        },
        Kind::Windows(windows) => match windows.get(index) {
            Some(&id) => Action::Window(id),
            None => return,
        },
        Kind::WindowMenu(id) => match window_menu::command_at(index) {
            Some(command) => Action::WindowCommand(id, command),
            None => return,
        },
        Kind::GroupMenu(windows) => match window_menu::group_command_at(index) {
            Some(command) => Action::GroupCommand(windows, command),
            None => return,
        },
        Kind::Start if start::entry_at(era, opts, index).is_some() => Action::Start(index),
        Kind::Start => return,
        // XP's Start menu (brief 112): a program is started (and counted), the rest goes where the
        // classic Start menu's matching entry goes (`bar::xp_start_action`).
        Kind::XpStart(pinned, often) => match xp_item {
            Some(bevel_core::xp_start::Item::Pinned(i)) => match pinned.get(i) {
                Some(target) => Action::Open(target.clone()),
                None => return,
            },
            Some(bevel_core::xp_start::Item::Often(i)) => match often.get(i) {
                Some(target) => {
                    let name = with_app(|app| app.menus.get(level).and_then(|p| p.menu.entries.get(index)).map(|e| e.label.clone())).flatten().unwrap_or_default();
                    crate::often::count(&name, target);
                    Action::Open(target.clone())
                }
                None => return,
            },
            Some(item) => match crate::bar::xp_start_place(item) {
                // A folder or Windows' own window: as the classic entries open theirs.
                Some(place) => Action::Open(place),
                None => {
                    close(false);
                    crate::bar::xp_start_action(item);
                    return;
                }
            },
            None => return,
        },
        // 7's Start menu (brief 114): a program is started (and counted); a place opens as the
        // classic entries' do; the search box, Shut down and its arrow as `bar::seven_start_action`.
        Kind::SevenStart(often) => match seven_item {
            Some(bevel_core::seven_start::Item::Program(i)) => match often.get(i) {
                Some(target) => {
                    let name = with_app(|app| app.menus.get(level).and_then(|p| p.menu.entries.get(index)).map(|e| e.label.clone())).flatten().unwrap_or_default();
                    crate::often::count(&name, target);
                    Action::Open(target.clone())
                }
                None => return,
            },
            Some(item) => match crate::bar::seven_start_place(item) {
                Some(place) => Action::Open(place),
                None => {
                    close(false);
                    crate::bar::seven_start_action(item);
                    return;
                }
            },
            None => return,
        },
        Kind::Folder(tree, path) => {
            let target = with_app(|app| match app.start_tree(tree).folder_at(&path)?.entry(index)? {
                Entry::Item(item) => Some((item.name.clone(), item.target.clone())),
                Entry::Folder(_) => None,
            });
            match target.flatten() {
                Some((name, target)) => {
                    // A program of the Programs menu: counted for XP's Start menu (brief 112).
                    if tree == Tree::Programs {
                        crate::often::count(&name, &target);
                    }
                    Action::Open(target)
                }
                None => return,
            }
        }
        Kind::Documents(documents) => match start::documents_entry(era, documents.len(), index) {
            Some(DocumentsEntry::MyDocuments) => match start_menu::documents_folder() {
                Some(dir) => Action::Open(dir.display().to_string()),
                None => return,
            },
            Some(DocumentsEntry::Document(i)) => Action::Open(documents[i].target.clone()),
            None => return,
        },
        Kind::Fixed(entry) => match start::fixed_target(entry, era, index) {
            Some(target) => Action::Target(target),
            None => return,
        },
        Kind::Speaker => Action::Speaker(index),
        Kind::Network => match bevel_core::status::network_command(era, index) {
            Some(bevel_core::status::NetworkCommand::Status) => Action::Open(crate::status::NETWORK_STATUS.to_string()),
            Some(bevel_core::status::NetworkCommand::Connections) => Action::Open(crate::status::connections()),
            None => return,
        },
        Kind::Power => {
            Action::Open(if index == 0 { crate::status::power_options() } else { crate::status::POWER_METER.to_string() })
        }
        Kind::Desktop => match with_app(|app| bevel_core::desktop::entry_at_with_undo(app.era, DESKTOP_UNDO.load(std::sync::atomic::Ordering::SeqCst), index)).flatten() {
            Some(bevel_core::desktop::Entry::Command(command)) => Action::Desktop(command, 0),
            _ => return,
        },
        Kind::DesktopArrange => match bevel_core::desktop::arrange_at(index) {
            Some(command) => Action::Desktop(command, 0),
            None => return,
        },
        Kind::DesktopNew => match bevel_core::desktop::new_at(index) {
            Some(command) => Action::Desktop(command, 0),
            None => return,
        },
        Kind::DesktopOwn => match bevel_core::desktop::own_at(index) {
            Some(command) => Action::Desktop(command, 0),
            None => return,
        },
        Kind::DesktopItem(kinds, ticket) => match bevel_core::desktop::item_at(&kinds, index) {
            Some(command) => Action::Desktop(command, ticket),
            None => return,
        },
        Kind::OwnIcon => match bevel_core::tray_icon::command_at(index) {
            Some(command) => Action::BarCommand(command, false),
            None => return,
        },
        Kind::Toolbars => match bar_menu::toolbars_entry(index) {
            Some(command) => Action::BarCommand(command, false),
            None => return,
        },
        Kind::Band => {
            use bevel_core::quick_launch::BandEntry;
            let command = match bevel_core::quick_launch::band_entry(era, index) {
                Some(BandEntry::Refresh) => BarCommand::QuickLaunchRefresh,
                Some(BandEntry::Open) => BarCommand::QuickLaunchOpen,
                Some(BandEntry::Close) => BarCommand::QuickLaunchClose,
                Some(BandEntry::AdjustDateTime) => BarCommand::AdjustDateTime,
                Some(BandEntry::Properties) => BarCommand::Properties,
                _ => return,
            };
            Action::BarCommand(command, false)
        }
        Kind::QuickItem(target, shortcut) => match bevel_core::quick_launch::icon_entry(shortcut, index) {
            Some(entry) => Action::QuickItem(target, entry),
            None => return,
        },
        // A row of the list starts the icon it was made for, as the icon's Open does (only while
        // that icon is still on the band: `bar::quick_item_command`).
        Kind::QuickList(targets) => match bevel_core::quick_launch::list_choice(&targets, index, |_| true) {
            Some(target) => Action::QuickItem(target, bevel_core::quick_launch::IconEntry::Open),
            None => return,
        },
        // Handled above (it starts nothing).
        Kind::Test(_) => return,
    };
    let bar = BAR_HWND.load(Ordering::SeqCst) as HWND;
    match action {
        // A bar command: close (focus back where it was), then let the bar carry it out.
        Action::BarCommand(command, shift) => {
            close(true);
            // SAFETY: posting to our own bar window.
            unsafe { PostMessageW(bar, WM_BAR_COMMAND.id(), command.code(), LPARAM::from(shift)) };
        }
        // The desktop's commands, the same way.
        Action::Desktop(command, ticket) => {
            close(true);
            // SAFETY: posting to our own bar window.
            unsafe { PostMessageW(bar, crate::bar::wm_desktop_command(), command.code(), crate::desktop::command_param(ticket, false)) };
        }
        Action::Theme(index) => {
            close(true);
            // SAFETY: posting to our own bar window.
            unsafe { PostMessageW(bar, WM_BAR_THEME.id(), index, 0) };
        }
        Action::Colors(index) => {
            close(true);
            // SAFETY: posting to our own bar window.
            unsafe { PostMessageW(bar, WM_BAR_COLORS.id(), index, 0) };
        }
        Action::MonitorLines(index) => {
            close(true);
            // SAFETY: posting to our own bar window.
            unsafe { PostMessageW(bar, crate::bar::WM_BAR_MONITOR_LINES.id(), index, 0) };
        }
        Action::Size(scale) => {
            close(true);
            let (wparam, lparam) = crate::bar::size_message(scale);
            // SAFETY: posting to our own bar window.
            unsafe { PostMessageW(bar, WM_BAR_SIZE.id(), wparam, lparam) };
        }
        // A window from an app's list: close, and bring that window to the front.
        Action::Window(id) => {
            close(false);
            // (A window in the list is always brought forward, minimized or not: the list has
            // no "minimize the one in front".)
            window_list::activate(id as HWND);
            with_app(|app| app.activated = Some((id, std::time::Instant::now())));
        }
        // While our (vanished) menu still has the focus, so the window may take it.
        Action::WindowCommand(id, command) => {
            let menus = vanish();
            window_list::window_command(id as HWND, command);
            destroy(menus);
        }
        Action::GroupCommand(windows, command) => {
            close(command == GroupCommand::Minimize);
            let windows: Vec<HWND> = windows.iter().map(|&id| id as HWND).collect();
            window_list::group_command(&windows, command);
        }
        // The menus go at the click, as in Windows; the program or document starts while our
        // vanished menu still has the focus (so it may come to the front), then the menus close
        // without taking the focus back.
        Action::Open(target) => {
            let menus = vanish();
            let started = std::time::Instant::now();
            window_list::launch(&target);
            let took = started.elapsed().as_millis();
            if took > 250 {
                log(&format!("starting a menu entry took {took} ms (its menu was already gone)"));
            }
            destroy(menus);
        }
        Action::Target(target) => {
            let menus = vanish();
            start_menu::open_target(target);
            destroy(menus);
        }
        // Windows' volume mixer or Sound settings, the same way.
        Action::Speaker(index) => {
            let menus = vanish();
            if index == 0 {
                crate::volume_popup::open_mixer();
            } else {
                crate::volume_popup::open_sound_settings();
            }
            destroy(menus);
        }
        // An icon's Open starts it while the vanished menu still has the focus (so the program may
        // come to the front); Delete and Properties after the menu has closed.
        Action::QuickItem(target, entry) => {
            let menus = vanish();
            crate::bar::quick_item_command(&target, entry);
            destroy(menus);
        }
        // Run, Shut Down and the like: the bar carries them out once the menu is gone.
        Action::Start(index) => {
            close(false);
            // SAFETY: posting to our own bar window.
            unsafe { PostMessageW(bar, WM_START_ACTION.id(), index, 0) };
        }
    }
}

/// The first (or last) entry that can be highlighted.
fn first_entry(menu: &Menu, style: MenuStyle, from_top: bool) -> Option<usize> {
    let selectable = |&i: &usize| menu.selectable(i, style);
    if from_top { (0..menu.entries.len()).find(selectable) } else { (0..menu.entries.len()).rev().find(selectable) }
}

/// The menu entry under a point in a menu window (screen pixels from its corner).
fn entry_at(hwnd: HWND, x: i32, y: i32) -> Option<usize> {
    with_app(|app| {
        let popup = app.menus.iter().find(|p| p.hwnd == hwnd)?;
        let (s, l) = (app.scale, &popup.layout);
        l.hit(&popup.menu, popup.style, s.old_in(x, l.width), s.old_in(y, l.height))
    })
    .flatten()
}

/// Highlights `entry` (or nothing) in the menu at `level`. Returns true if that changed it.
fn set_hover(level: usize, entry: Option<usize>) -> bool {
    let changed = with_app(|app| match app.menus.get_mut(level) {
        Some(popup) if popup.menu.hover != entry => {
            popup.menu.hover = entry;
            Some(popup.hwnd)
        }
        _ => None,
    })
    .flatten();
    if let Some(hwnd) = changed {
        // SAFETY: our own window.
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
    changed.is_some()
}

fn mouse_move(hwnd: HWND, x: i32, y: i32) {
    let Some(level) = level_of(hwnd) else {
        return;
    };
    let arm = with_app(|app| match app.menus.get_mut(level) {
        Some(popup) if !popup.tracking => {
            popup.tracking = true;
            true
        }
        _ => false,
    })
    .unwrap_or(false);
    if arm {
        let mut track = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: 0,
        };
        // SAFETY: a correctly sized struct for our own window.
        unsafe { TrackMouseEvent(&mut track) };
    }
    // A scrolling menu's strip under the pointer: it steps after a moment, and on while the
    // pointer rests there (98 draws the strip sunken meanwhile).
    let strip = strip_at(hwnd, x, y);
    if set_strip(level, strip) {
        match strip {
            Some(_) => {
                let first = with_app(|app| app.menus[level].layout.scroll.map(|s| paint::scroll_pace(s.flat).0)).flatten();
                // SAFETY: a timer on our own window.
                unsafe { SetTimer(hwnd, SCROLL_TIMER, first.unwrap_or(150), None) };
            }
            // SAFETY: our own window's timer.
            None => unsafe {
                KillTimer(hwnd, SCROLL_TIMER);
            },
        }
    }
    if set_hover(level, entry_at(hwnd, x, y)) {
        // Hovering decides after a moment whether a submenu opens or closes.
        // SAFETY: a timer on our own window.
        unsafe { SetTimer(hwnd, HOVER_TIMER, HOVER_MS, None) };
    }
}

/// The strip of a scrolling menu under a point in its window (screen pixels from its corner).
fn strip_at(hwnd: HWND, x: i32, y: i32) -> Option<Strip> {
    with_app(|app| {
        let popup = app.menus.iter().find(|p| p.hwnd == hwnd)?;
        let (s, l) = (app.scale, &popup.layout);
        l.strip_at(s.old_in(x, l.width), s.old_in(y, l.height))
    })
    .flatten()
}

/// Marks `strip` (or none) as the one under the pointer in the menu at `level`. True if that
/// changed it. The window is painted again only where that shows (98's sunken strip; Windows
/// 2000's flat strips look the same: brief 92).
fn set_strip(level: usize, strip: Option<Strip>) -> bool {
    let changed = with_app(|app| {
        let popup = app.menus.get_mut(level)?;
        let scroll = popup.layout.scroll.as_mut()?;
        (scroll.hot != strip).then(|| {
            scroll.hot = strip;
            (popup.hwnd, !scroll.flat)
        })
    })
    .flatten();
    if let Some((hwnd, true)) = changed {
        // SAFETY: our own window.
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
    changed.is_some()
}

/// The pointer has rested on a strip for a step: the menu scrolls one step toward it (a submenu it
/// had open closes); at the end the strip goes, and so does the timer. The place is remembered
/// while the bar runs, so the menu reopens where it was left (98, measured).
fn scroll_step(hwnd: HWND) {
    let Some(level) = level_of(hwnd) else {
        // SAFETY: our own window's timer.
        unsafe { KillTimer(hwnd, SCROLL_TIMER) };
        return;
    };
    let stepped = with_app(|app| {
        let popup = app.menus.get_mut(level)?;
        let scroll = popup.layout.scroll.as_mut()?;
        let strip = scroll.hot?;
        let moved = scroll.step(strip, paint::scroll_step(scroll));
        if !scroll.shows(strip) {
            scroll.hot = None;
        }
        let (offset, repeat, going) = (scroll.offset, paint::scroll_pace(scroll.flat).1, scroll.hot.is_some());
        remember(app, level, offset);
        Some((moved, repeat, going))
    })
    .flatten();
    let Some((moved, repeat, going)) = stepped else {
        // SAFETY: our own window's timer.
        unsafe { KillTimer(hwnd, SCROLL_TIMER) };
        return;
    };
    if moved {
        close_from(level + 1);
        // SAFETY: our own window.
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
    // SAFETY: our own window's timer (set anew: the pace after the first step).
    unsafe {
        if going {
            SetTimer(hwnd, SCROLL_TIMER, repeat, None);
        } else {
            KillTimer(hwnd, SCROLL_TIMER);
        }
    }
}

/// The highlighted entry of the menu at `level` is brought into view (a key chose it), and the
/// place remembered.
fn reveal_hover(level: usize) {
    let hwnd = with_app(|app| {
        let popup = app.menus.get_mut(level)?;
        let i = popup.menu.hover?;
        if !popup.layout.reveal(i) {
            return None;
        }
        let offset = popup.layout.scroll.map_or(0, |s| s.offset);
        remember(app, level, offset);
        Some(app.menus[level].hwnd)
    })
    .flatten();
    if let Some(hwnd) = hwnd {
        close_from(level + 1);
        // SAFETY: our own window.
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
}

/// The hover moment passed: open the hovered entry's submenu, or close one whose entry is
/// no longer hovered.
fn hover_settled(hwnd: HWND) {
    // SAFETY: our own window's timer.
    unsafe { KillTimer(hwnd, HOVER_TIMER) };
    let Some(level) = level_of(hwnd) else {
        return;
    };
    let Some((hover, is_submenu, open_for)) = with_app(|app| {
        let popup = app.menus.get(level)?;
        let hover = popup.menu.hover;
        let is_submenu = hover.is_some_and(|i| popup.menu.entries[i].kind == EntryKind::Submenu);
        Some((hover, is_submenu, app.menus.get(level + 1).and_then(|c| c.parent_entry)))
    })
    .flatten() else {
        return;
    };
    match hover {
        Some(i) if is_submenu && open_for != Some(i) => open_child(level, i, false),
        Some(i) if !is_submenu && open_for.is_some() && open_for != Some(i) => close_from(level + 1),
        _ => {}
    }
}

/// Once the keyboard is used, Windows 2000 shows the access keys too (95 and 98 always do).
fn show_keys() {
    let windows = with_app(|app| {
        app.menus
            .iter_mut()
            .filter(|p| !p.menu.show_keys)
            .map(|p| {
                p.menu.show_keys = true;
                p.hwnd
            })
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    for hwnd in windows {
        // SAFETY: our own windows.
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
}

/// Keys always act on the deepest open menu: Up/Down move (wrapping), Right/Enter open a
/// submenu, Left/Esc go back one level (Esc on the root closes), Enter chooses.
fn key(vk: u16) {
    let Some(level) = deepest() else {
        return;
    };
    if matches!(vk, VK_UP | VK_DOWN | VK_LEFT | VK_RIGHT | VK_RETURN) {
        show_keys();
    }
    let hover = with_app(|app| app.menus[level].menu.hover).flatten();
    let is_submenu =
        |i: usize| with_app(|app| app.menus[level].menu.entries[i].kind == EntryKind::Submenu).unwrap_or(false);
    // Windows 95's columns: Left and Right go to the same row of the column beside (survey part
    // 1, 3); Right on a folder still opens it, Left in the first column goes back.
    let beside = |toward: i32| {
        with_app(|app| {
            let popup = &app.menus[level];
            let era_95 = app.era == Era::Win95 && popup.style != MenuStyle::Popup && popup.layout.columns.len() > 1;
            era_95.then(|| hover.and_then(|i| popup.layout.beside(&popup.menu, popup.style, i, toward))).flatten()
        })
        .flatten()
    };
    match vk {
        VK_ESCAPE if level == 0 => close(true),
        VK_LEFT if beside(-1).is_some() => {
            set_hover(level, beside(-1));
        }
        VK_ESCAPE | VK_LEFT if level > 0 => close_from(level),
        VK_RIGHT => {
            if let Some(i) = hover.filter(|&i| is_submenu(i)) {
                open_child(level, i, true);
            } else if let Some(next) = beside(1) {
                set_hover(level, Some(next));
            }
        }
        VK_RETURN => {
            if let Some(i) = hover {
                if is_submenu(i) {
                    open_child(level, i, true);
                } else {
                    choose(level, i);
                }
            }
        }
        VK_UP | VK_DOWN => {
            let next = with_app(|app| {
                let (menu, style) = (&app.menus[level].menu, app.menus[level].style);
                let Some(current) = menu.hover else {
                    return first_entry(menu, style, vk == VK_DOWN);
                };
                let n = menu.entries.len();
                let step = if vk == VK_DOWN { 1 } else { n - 1 };
                (1..=n).map(|k| (current + k * step) % n).find(|&i| menu.selectable(i, style))
            })
            .flatten();
            if next.is_some() {
                set_hover(level, next);
                // A scrolling menu brings the entry into view (98: just above the bottom strip;
                // wrapping round jumps to the other end).
                reveal_hover(level);
            }
        }
        _ => {}
    }
}

/// A typed character: the entries whose access key it is (the "&" letter, or else the first
/// letter, as in Windows' Start menu). One entry: it is chosen (or its submenu opened);
/// several: the highlight moves on to the next of them.
fn access_key(ch: char) {
    let Some(level) = deepest() else {
        return;
    };
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let Some((matches, hover)) = with_app(|app| {
        let menu = &app.menus[level].menu;
        let key_of = |label: &str| {
            let (text, key) = split_access_key(label);
            text.chars().nth(key.unwrap_or(0)).map(lower)
        };
        let matches: Vec<usize> = (0..menu.entries.len())
            .filter(|&i| menu.entries[i].choosable() && key_of(&menu.entries[i].label) == Some(lower(ch)))
            .collect();
        (matches, menu.hover)
    }) else {
        return;
    };
    match matches.as_slice() {
        [] => {}
        [only] => {
            show_keys();
            set_hover(level, Some(*only));
            choose(level, *only);
        }
        _ => {
            let next = matches.iter().find(|&&i| hover.is_some_and(|h| i > h)).unwrap_or(&matches[0]);
            set_hover(level, Some(*next));
            reveal_hover(level);
        }
    }
}

fn paint_menu(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: paired BeginPaint/EndPaint on our own window.
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    with_app(|app| {
        let Some(popup) = app.menus.iter().find(|p| p.hwnd == hwnd) else {
            return;
        };
        let mut fb = Framebuffer::new(popup.layout.width, popup.layout.height, 0);
        paint::paint_menu(&mut fb, app.theme, &app.fonts(), &popup.menu, popup.style, &popup.layout);
        let mut big = app.scale.enlarge(&fb);
        // XP's and 7's program pictures at their final size (the owner, 2026-11-25).
        for (r, icon) in &popup.finals {
            paint::draw_final_icon(&mut big, app.scale, *r, icon);
        }
        blit(hdc, &big, big.width);
    });
    // SAFETY: see above.
    unsafe { EndPaint(hwnd, &ps) };
}

pub unsafe extern "system" fn menu_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => return 1,
        WM_PAINT => {
            paint_menu(hwnd);
            return 0;
        }
        // Clicking a submenu must not take the focus from the root menu.
        WM_MOUSEACTIVATE if level_of(hwnd).is_some_and(|l| l > 0) => return MA_NOACTIVATE as LRESULT,
        WM_MOUSEMOVE => {
            let (x, y) = point(lparam);
            mouse_move(hwnd, x, y);
            return 0;
        }
        WM_MOUSELEAVE => {
            let Some(level) = level_of(hwnd) else {
                return 0;
            };
            // Keep an entry highlighted while its submenu is open (the pointer went there).
            let keep = with_app(|app| {
                app.menus[level].tracking = false;
                app.menus.get(level + 1).and_then(|child| child.parent_entry)
            })
            .flatten();
            set_hover(level, keep);
            // The pointer left a strip: the scrolling stops.
            if set_strip(level, None) {
                // SAFETY: our own window's timer.
                unsafe { KillTimer(hwnd, SCROLL_TIMER) };
            }
            return 0;
        }
        WM_TIMER if wparam == HOVER_TIMER => {
            hover_settled(hwnd);
            return 0;
        }
        WM_TIMER if wparam == SCROLL_TIMER => {
            scroll_step(hwnd);
            return 0;
        }
        WM_LBUTTONUP => {
            let (x, y) = point(lparam);
            if let (Some(level), Some(index)) = (level_of(hwnd), entry_at(hwnd, x, y)) {
                choose(level, index);
            }
            return 0;
        }
        WM_KEYDOWN => {
            key(wparam as u16);
            return 0;
        }
        WM_CHAR => {
            if let Some(ch) = char::from_u32(wparam as u32).filter(|c| !c.is_control()) {
                access_key(ch);
            }
            return 0;
        }
        // Another window got the focus (a click elsewhere): close everything, without
        // taking the focus back.
        WM_ACTIVATE if (wparam & 0xFFFF) as u32 == WA_INACTIVE && level_of(hwnd) == Some(0) => {
            close(false);
            return 0;
        }
        _ => {}
    }
    // SAFETY: default handling for everything else.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

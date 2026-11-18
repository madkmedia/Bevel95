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
use bevel_core::fb::{Framebuffer, Rect};
use bevel_core::icons;
use bevel_core::layout::{place_popup, place_start_menu, place_submenu};
use bevel_core::paint::{self, EntryKind, Menu, MenuEntry, MenuLayout, MenuStyle};
use bevel_core::programs::{Entry, Folder, Item};
use bevel_core::scale::upscale;
use bevel_core::start::{self, DocumentsEntry, StartEntry};
use bevel_core::text::{escape_access_keys, split_access_key};
use bevel_core::theme::{Era, SubmenuPlacement};
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
    PostMessageW, SW_SHOW, SW_SHOWNA, SetForegroundWindow, SetTimer, ShowWindow, WA_INACTIVE, WM_ACTIVATE, WM_CHAR,
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
    /// An app's windows: entry i brings window i to the front.
    Windows(Vec<u64>),
    /// A task button's right-click menu for its one window: entries are `window_menu` ones.
    WindowMenu(u64),
    /// A task button's right-click menu for its several windows: `window_menu` group entries.
    GroupMenu(Vec<u64>),
    /// The Start menu: entry i is `start::entry_at(era, i)`.
    Start,
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
}

impl Popup {
    pub fn is_start(&self) -> bool {
        matches!(self.kind, Kind::Start)
    }
}

/// Where a root menu opens.
enum At {
    /// At a clicked point (flipping up/left when there is no room), like TrackPopupMenu.
    Point(i32, i32),
    /// The speaker's menu, for a click at a point: where that look's Windows opens it.
    Speaker(Era, i32, i32),
    /// The Start menu, on top of the Start button (screen pixels).
    StartButton(Rect),
    /// Upward from a point (Bevel95's icon on Windows' taskbar: `tray_icon::place_menu`).
    Above(i32, i32),
}

/// Opens the bar's right-click menu for a click at screen point (x, y); `clock`: the click was
/// on the clock.
pub fn open(bar: HWND, x: i32, y: i32, clock: bool) {
    let show_keys = with_app(|app| app.era.always_shows_keys()).unwrap_or(true);
    let menu = Menu { show_keys, ..bar_menu::menu(clock) };
    open_root(bar, Kind::BarMenu(clock), menu, MenuStyle::Popup, At::Point(x, y));
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
    open_root(bar, Kind::Windows(windows), menu, MenuStyle::Cascade, At::Point(x, y));
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
    });
    let user = start_menu::user_name();
    // "Show small icons in Start menu": 16×16 icons and no banner.
    let (style, mut menu) = if small {
        (MenuStyle::StartSmall, start::menu(era, opts, &user, |e| icons::start_icon_small(era, e)))
    } else {
        (MenuStyle::Start, start::menu(era, opts, &user, |e| icons::start_icon(era, e)))
    };
    menu.show_keys = era.always_shows_keys() || by_keyboard;
    open_root(bar, Kind::Start, menu, style, At::StartButton(button));
}

fn open_root(bar: HWND, kind: Kind, menu: Menu, style: MenuStyle, at: At) {
    close(false);
    // SAFETY: a plain query.
    let previous = unsafe { GetForegroundWindow() };
    let Some((layout, scale, screen)) = measure(&menu, style) else {
        return;
    };
    let (w, h) = (layout.width * scale, layout.height * scale);
    let (left, top) = match at {
        At::Point(x, y) => place_popup(x, y, w, h, screen),
        At::Speaker(era, x, y) => volume::place_menu(era, x, y, w, h, screen),
        At::StartButton(button) => place_start_menu(button, w, h, screen),
        At::Above(x, y) => bevel_core::tray_icon::place_menu(x, y, w, h, screen),
    };
    let rect = Rect::new(left, top, w, h);
    let Some(hwnd) = create(bar, rect, false) else {
        return;
    };
    let start = matches!(kind, Kind::Start);
    let popup = Popup { hwnd, kind, menu, style, layout, rect, parent_entry: None, previous, tracking: false };
    with_app(|app| app.menus.push(popup));
    // SAFETY: our own windows.
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        take_focus(hwnd);
        if start {
            InvalidateRect(bar, null(), 0); // the Start button is drawn pressed now
        }
    }
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
            Kind::BarMenu(clock) if bar_menu::entry(*clock, index) == Some(BarEntry::Bevel95) => {
                let own = bar_menu::Own { desktop: app.desktop, desktop_key: crate::bar::desktop_key() };
                Want::Ready(Kind::BarOwn(own), bar_menu::own_menu(own), MenuStyle::Popup)
            }
            Kind::BarOwn(own) if bar_menu::own_entry(*own, index) == Some(BarEntry::Theme) => {
                Want::Ready(Kind::Theme, bar_menu::theme_menu(app.era), MenuStyle::Popup)
            }
            Kind::BarOwn(own) if bar_menu::own_entry(*own, index) == Some(BarEntry::Colors) => {
                Want::Ready(Kind::Colors, bar_menu::colors_menu(app.colors), MenuStyle::Popup)
            }
            Kind::BarOwn(own) if bar_menu::own_entry(*own, index) == Some(BarEntry::Size) => {
                Want::Ready(Kind::Size, bar_menu::size_menu(app.scale), MenuStyle::Popup)
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

/// Opens the submenu of entry `index` of the menu at `level` (closing deeper ones).
fn open_child(level: usize, index: usize, highlight_first: bool) {
    close_from(level + 1);
    let Some((kind, mut menu, style)) = submenu_of(level, index) else {
        return;
    };
    let Some((parent_rect, entry_rect, rule, frame, bar)) = with_app(|app| {
        let parent = app.menus.get(level)?;
        let e = *parent.layout.rects.get(index)?;
        let s = app.scale;
        let entry = Rect::new(parent.rect.x + e.x * s, parent.rect.y + e.y * s, e.w * s, e.h * s);
        let (rule, frame) = match style {
            MenuStyle::Popup => (app.theme.popup.submenu, app.theme.popup.frame),
            _ => (app.theme.start_menu.submenu, app.theme.start_menu.frame),
        };
        let rule = SubmenuPlacement { overlap: rule.overlap * s, margin_bottom: rule.margin_bottom * s, ..rule };
        Some((parent.rect, entry, rule, frame * s, BAR_HWND.load(Ordering::SeqCst) as HWND))
    })
    .flatten() else {
        return;
    };
    if highlight_first {
        menu.hover = first_entry(&menu, style, true);
    }
    let Some((layout, scale, screen)) = measure(&menu, style) else {
        return;
    };
    let (w, h) = (layout.width * scale, layout.height * scale);
    let (left, top) = place_submenu(parent_rect, entry_rect, w, h, frame, rule, screen);
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
    };
    with_app(|app| app.menus.push(popup));
    // SAFETY: our own window, shown without taking the focus from the root menu.
    unsafe { ShowWindow(hwnd, SW_SHOWNA) };
}

/// A menu's layout at 1×, the scale, and the main screen.
fn measure(menu: &Menu, style: MenuStyle) -> Option<(MenuLayout, i32, Rect)> {
    with_app(|app| {
        let m = app.monitor;
        let screen = Rect::new(m.left, m.top, rect_w(&m), rect_h(&m));
        let layout = paint::layout_menu(app.theme, &app.fonts(), menu, style, screen.h / app.scale);
        (layout, app.scale, screen)
    })
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
        if give_back_focus && !root.previous.is_null() && IsWindow(root.previous) != 0 {
            SetForegroundWindow(root.previous);
        }
        for popup in menus.iter().rev() {
            DestroyWindow(popup.hwnd);
        }
        if root.is_start() {
            InvalidateRect(BAR_HWND.load(Ordering::SeqCst) as HWND, null(), 0); // Start pops back up
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
    if menus.first().is_some_and(Popup::is_start) {
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
    Theme(Era),
    Colors(usize),
    Size(i32),
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
}

/// Runs entry `index` of the menu at `level`.
fn choose(level: usize, index: usize) {
    let Some((kind, entry_kind, era, opts)) = with_app(|app| {
        let popup = app.menus.get(level)?;
        let entry = popup.menu.entries.get(index).filter(|e| e.choosable())?;
        Some((popup.kind.clone(), entry.kind, app.era, app.start_options))
    })
    .flatten() else {
        return; // a separator or a grayed-out entry: nothing happens
    };
    if entry_kind == EntryKind::Submenu {
        open_child(level, index, false);
        return;
    }
    let action = match kind {
        Kind::BarMenu(_) | Kind::BarOwn(_) => {
            let entry = match kind {
                Kind::BarMenu(clock) => bar_menu::entry(clock, index),
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
            Some(era) => Action::Theme(era),
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
        Kind::Folder(tree, path) => {
            let target = with_app(|app| match app.start_tree(tree).folder_at(&path)?.entry(index)? {
                Entry::Item(item) => Some(item.target.clone()),
                Entry::Folder(_) => None,
            });
            match target.flatten() {
                Some(target) => Action::Open(target),
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
        Action::Theme(era) => {
            close(true);
            let index = Era::ALL.iter().position(|&e| e == era).unwrap_or(0);
            // SAFETY: posting to our own bar window.
            unsafe { PostMessageW(bar, WM_BAR_THEME.id(), index, 0) };
        }
        Action::Colors(index) => {
            close(true);
            // SAFETY: posting to our own bar window.
            unsafe { PostMessageW(bar, WM_BAR_COLORS.id(), index, 0) };
        }
        Action::Size(scale) => {
            close(true);
            // SAFETY: posting to our own bar window.
            unsafe { PostMessageW(bar, WM_BAR_SIZE.id(), scale as usize, 0) };
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
        popup.layout.hit(&popup.menu, popup.style, x / app.scale, y / app.scale)
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
    if set_hover(level, entry_at(hwnd, x, y)) {
        // Hovering decides after a moment whether a submenu opens or closes.
        // SAFETY: a timer on our own window.
        unsafe { SetTimer(hwnd, HOVER_TIMER, HOVER_MS, None) };
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
    match vk {
        VK_ESCAPE if level == 0 => close(true),
        VK_ESCAPE | VK_LEFT if level > 0 => close_from(level),
        VK_RIGHT => {
            if let Some(i) = hover.filter(|&i| is_submenu(i)) {
                open_child(level, i, true);
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
        let big = upscale(&fb, app.scale);
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
            return 0;
        }
        WM_TIMER if wparam == HOVER_TIMER => {
            hover_settled(hwnd);
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

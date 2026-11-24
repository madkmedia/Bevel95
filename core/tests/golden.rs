//! Golden-image tests: fixed states of the bar and Start menu, painted at 1× with the real
//! Windows glyphs (from `tests/fixtures`), compared pixel for pixel with the PNGs in
//! `tests/golden`. Any difference fails the test.
//!
//! The PNGs are only rewritten on request, with `cargo regen-golden`, which also rewrites
//! `docs/preview.html`: the same pictures embedded in one self-contained page, for eyeballing.
//! When a comparison fails, `<name>.actual.png` (what the code draws now) and
//! `<name>.diff.png` (differences in red) are written next to the golden image.

use std::fmt::Write as _;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use bevel_core::desktop_icons::ViewMode;
use bevel_core::bar_menu::{self, BarEntry};
use bevel_core::dialog::{self, Dialog, DialogKind};
use bevel_core::fb::{Framebuffer, Icon, Rect};
use bevel_core::layout::{self, place_popup, place_start_menu, place_submenu};
use bevel_core::paint::{self, Bar, Long, Menu, MenuEntry, MenuLayout, MenuStyle, ScrollArrow, Strip, Task, TaskState};
use bevel_core::programs::{self, Folder, Item, Shown};
use bevel_core::scale::{Scale, upscale};
use bevel_core::tasks::{self, Group, TaskList, WindowInfo};
use bevel_core::text::{Font, Fonts, fixture_name};
use bevel_core::theme::{Depth, Era, Theme, WIN95, WIN98, WIN2000, rgb};
use bevel_core::window_menu::{self, WindowState};
use bevel_core::config::{StartOptions, TaskbarOptions};
use bevel_core::properties;
use bevel_core::window_icons::{self, ClassicWindow};
use bevel_core::{icons, start};

const WIDTH: i32 = 640;
const SCREEN_H: i32 = 480;
const CLOCK: &str = "3:45 PM";
/// The widest time of day (the clock's part of the well is made this wide).
const WIDEST: &str = "12:00 AM";

fn test_dir(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join(sub)
}

fn load_font(theme: &Theme, bold: bool) -> Font {
    let path = test_dir("fixtures").join(fixture_name(theme.font_face, theme.font_px, bold));
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Font::parse(&src).unwrap().with_period_glyphs()
}

// ---- test data: placeholder 16×16 icons (not real Windows icons) ----

#[rustfmt::skip]
const COMPUTER: [&str; 16] = [
    "................",
    ".KKKKKKKKKKKKK..",
    ".KLLLLLLLLLLLK..",
    ".KLKKKKKKKKKLK..",
    ".KLKTTTTTTTKLK..",
    ".KLKTTTTTTTKLK..",
    ".KLKTTTTTTTKLK..",
    ".KLKTTTTTTTKLK..",
    ".KLKKKKKKKKKLK..",
    ".KLLLLLLLLLLLK..",
    ".KKKKKKKKKKKKK..",
    "....KLLLLLK.....",
    "..KKKKKKKKKKK...",
    "..KLLLLLLLLLK...",
    "..KKKKKKKKKKK...",
    "................",
];

#[rustfmt::skip]
const DOCUMENT: [&str; 16] = [
    "................",
    "..KKKKKKKK......",
    "..KWWWWWWKK.....",
    "..KWWWWWWKWK....",
    "..KWNNNNWKKKK...",
    "..KWWWWWWWWWK...",
    "..KWNNNNNNNWK...",
    "..KWWWWWWWWWK...",
    "..KWNNNNNNNWK...",
    "..KWWWWWWWWWK...",
    "..KWNNNNNNNWK...",
    "..KWWWWWWWWWK...",
    "..KWNNNNNWWWK...",
    "..KWWWWWWWWWK...",
    "..KKKKKKKKKKK...",
    "................",
];

#[rustfmt::skip]
const MAIL: [&str; 16] = [
    "................",
    "................",
    "................",
    ".KKKKKKKKKKKKKK.",
    ".KLKWWWWWWWWKLK.",
    ".KWLKWWWWWWKLWK.",
    ".KWWLKWWWWKLWWK.",
    ".KWWWLKWWKLWWWK.",
    ".KWWWWLKKLWWWWK.",
    ".KWWWWWLLWWWWWK.",
    ".KWWWWWWWWWWWWK.",
    ".KWWWWWWWWWWWWK.",
    ".KKKKKKKKKKKKKK.",
    "................",
    "................",
    "................",
];

#[rustfmt::skip]
const FOLDER: [&str; 16] = [
    "................",
    "................",
    "..KKKKK.........",
    ".KYYYYYK........",
    "KYWYYYYYKKKKKK..",
    "KWYYYYYYYYYYYYK.",
    "KYYYYYYYYYYYYYK.",
    "KYYYYYYYYYYYYYK.",
    "KYYYYYYYYYYYYYK.",
    "KYYYYYYYYYYYYYK.",
    "KYYYYYYYYYYYYYK.",
    "KYYYYYYYYYYYYYK.",
    "KKKKKKKKKKKKKKK.",
    "................",
    "................",
    "................",
];

/// An icon from ASCII art, each pixel repeated `scale` times (2 gives the 32×32 menu icons).
fn icon(art: &[&str; 16], scale: i32) -> Icon {
    let color = |b: u8| match b {
        b'K' => rgb(0x000000),
        b'W' => rgb(0xFFFFFF),
        b'L' => rgb(0xC0C0C0),
        b'N' => rgb(0x000080),
        b'T' => rgb(0x008080),
        b'Y' => rgb(0xFFFF00),
        b'.' => 0,
        _ => panic!("unknown icon color {}", b as char),
    };
    let size = 16 * scale;
    let mut pixels = Vec::with_capacity((size * size) as usize);
    for y in 0..size {
        for x in 0..size {
            pixels.push(color(art[(y / scale) as usize].as_bytes()[(x / scale) as usize]));
        }
    }
    Icon { width: size, height: size, pixels }
}

fn task(title: &str, art: &[&str; 16], state: TaskState) -> Task {
    Task { title: title.to_string(), icon: Some(icon(art, 1)), state }
}

/// "3 groups, one active, one flashing".
fn three_groups() -> Vec<Task> {
    vec![
        task("My Computer", &COMPUTER, TaskState::Normal),
        task("Microsoft Word - Document1", &DOCUMENT, TaskState::Active),
        task("Inbox - Outlook Express", &MAIL, TaskState::Flashing),
    ]
}

/// Enough buttons that they have to shrink.
fn crowded() -> Vec<Task> {
    vec![
        task("My Computer", &COMPUTER, TaskState::Normal),
        task("Exploring - C:\\", &FOLDER, TaskState::Normal),
        task("Microsoft Word - Report.doc", &DOCUMENT, TaskState::Active),
        task("Inbox - Outlook Express", &MAIL, TaskState::Normal),
        task("Paint - untitled", &DOCUMENT, TaskState::Normal),
        task("Notepad - todo.txt", &DOCUMENT, TaskState::Normal),
        task("Calculator", &COMPUTER, TaskState::Normal),
        task("MS-DOS Prompt", &COMPUTER, TaskState::Flashing),
        task("Recycle Bin", &FOLDER, TaskState::Normal),
    ]
}

/// More buttons than fit even at their narrowest: 24 apps, the 17th of them active.
fn overfull() -> Vec<Task> {
    let arts = [&COMPUTER, &FOLDER, &DOCUMENT, &MAIL];
    (0..24)
        .map(|i| {
            let state = if i == 16 { TaskState::Active } else { TaskState::Normal };
            task(&format!("Window {}", i + 1), arts[i % arts.len()], state)
        })
        .collect()
}

// ---- scenes ----

fn bar_image(theme: &Theme, fonts: &Fonts, start_pressed: bool, tasks: Vec<Task>) -> Framebuffer {
    let mut fb = Framebuffer::new(WIDTH, theme.bar.height, 0);
    paint::paint_bar(&mut fb, theme, fonts, &Bar { start_pressed, tasks, clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(), ..Bar::default() });
    fb
}

/// The overfull bar on its second row, the down arrow held down.
fn second_row_image(theme: &Theme, fonts: &Fonts) -> Framebuffer {
    let mut fb = Framebuffer::new(WIDTH, theme.bar.height, 0);
    let state = Bar {
        tasks: overfull(),
        clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(),
        row: 1,
        scroll_pressed: Some(ScrollArrow::Down),
        ..Bar::default()
    };
    paint::paint_bar(&mut fb, theme, fonts, &state);
    fb
}

/// A drag in progress (Bevel95's own): My Computer, pressed 20 from its left, dragged right
/// over Word, which covered more than half of it 10 ms after the drag's start; Word is halfway
/// through its slide to the free place; Inbox stays. Positions from `tasks::drag_step` and
/// `tasks::Motion`, as the bar computes them.
fn drag_image(theme: &Theme, fonts: &Fonts) -> Framebuffer {
    let [computer, word, inbox]: [Task; 3] = three_groups().try_into().unwrap();
    let mut state = Bar { tasks: vec![computer.clone(), word.clone(), inbox.clone()], clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(), ..Bar::default() };
    let row: Vec<(i32, i32)> = paint::bar_layout(theme, fonts, WIDTH, &state).tasks.iter().map(|r| (r.x, r.w)).collect();
    let mut motion = tasks::Motion::default();
    let grab = 20;
    let pointer = |moved: i32| row[0].0 + grab + moved;
    let (place, left) = tasks::drag_step(&row, 0, grab, pointer(30)).unwrap();
    assert_eq!(place, 0);
    motion.frame(&["computer", "word", "inbox"], &row, Some((0, left)), 0);
    let moved = row[1].0 - row[0].0 - 20;
    let (place, left) = tasks::drag_step(&row, 0, grab, pointer(moved)).unwrap();
    assert_eq!(place, 1, "past the line between the first two places");
    motion.frame(&["word", "computer", "inbox"], &row, Some((1, left)), 10);
    let x = motion.frame(&["word", "computer", "inbox"], &row, Some((1, left)), 10 + tasks::SLIDE_MS / 2);
    assert!(x[0] > row[0].0 && x[0] < row[1].0, "Word on its way");
    state.tasks = vec![word, computer, inbox];
    state.task_x = x;
    state.task_on_top = Some(1);
    let mut fb = Framebuffer::new(WIDTH, theme.bar.height, 0);
    paint::paint_bar(&mut fb, theme, fonts, &state);
    fb
}

/// A task button's right-click menu, placed the way the real one is for a right-click on
/// the button at (button.x + 20, 466): for one window (pointer on Maximize), or the group menu.
fn task_menu_scene(theme: &Theme, era: Era, fonts: &Fonts, group: bool) -> Framebuffer {
    let screen_rect = Rect::new(0, 0, WIDTH, SCREEN_H);
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let mut bar = Framebuffer::new(WIDTH, theme.bar.height, 0);
    let state = Bar { tasks: three_groups(), clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(), ..Bar::default() };
    let layout = paint::paint_bar(&mut bar, theme, fonts, &state);
    screen.blit(&bar, 0, SCREEN_H - bar.height);
    let menu = if group {
        window_menu::group_menu()
    } else {
        let normal = WindowState {
            resizable: true,
            has_minimize_box: true,
            has_maximize_box: true,
            closable: true,
            ..WindowState::default()
        };
        Menu { hover: Some(4), ..window_menu::menu(era, normal) }
    };
    let menu = Menu { show_keys: era.always_shows_keys(), ..menu };
    let (fb, _) = menu_image(theme, fonts, &menu, MenuStyle::Popup);
    let (x, y) = place_popup(layout.tasks[1].x + 20, SCREEN_H - 14, fb.width, fb.height, screen_rect);
    screen.blit(&fb, x, y);
    screen
}

/// A menu laid out and painted on its own.
fn menu_image(theme: &Theme, fonts: &Fonts, menu: &Menu, style: MenuStyle) -> (Framebuffer, MenuLayout) {
    let layout = paint::layout_menu(theme, fonts, menu, style, SCREEN_H);
    let mut fb = Framebuffer::new(layout.width, layout.height, 0);
    paint::paint_menu(&mut fb, theme, fonts, menu, style, &layout);
    (fb, layout)
}

/// The screen rectangle of entry `index` of a menu shown at `menu`.
fn entry_on_screen(menu: Rect, layout: &MenuLayout, index: usize) -> Rect {
    let r = layout.rects[index];
    Rect::new(menu.x + r.x, menu.y + r.y, r.w, r.h)
}

/// A 640×480 "screen": desktop color, the bar at the bottom with Start pressed, the Start
/// menu above it ("Start menu open with 10 items") with the pointer on Programs, and the
/// Programs menu open beside it, pointer on its second entry.
fn start_menu_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    let screen_rect = Rect::new(0, 0, WIDTH, SCREEN_H);
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, true, three_groups());
    let bar_top = SCREEN_H - bar.height;
    screen.blit(&bar, 0, bar_top);

    let mut menu = start::menu(era, StartOptions::default(), "Admin", |e| icons::start_icon(era, e));
    let programs_index = menu.entries.iter().position(|e| e.label == "&Programs").expect("Programs");
    menu.hover = Some(programs_index);
    menu.show_keys = era.always_shows_keys();
    let (fb, layout) = menu_image(theme, fonts, &menu, MenuStyle::Start);
    let button = layout::start_button(theme, fonts);
    let button = Rect::new(button.x, bar_top + button.y, button.w, button.h);
    let (x, y) = place_start_menu(button, fb.width, fb.height, screen_rect);
    screen.blit(&fb, x, y);

    let group = icons::program_group(era);
    let mut programs = Folder::default();
    for folder in ["Accessories", "Games", "StartUp"] {
        programs.folder_mut(&[folder]);
    }
    for (name, _) in [("Internet Explorer", 0), ("MS-DOS Prompt", 1), ("Outlook Express", 2), ("Windows Explorer", 3)] {
        programs.add(&[], Item { name: name.to_string(), target: String::new() });
    }
    let art = [&COMPUTER, &DOCUMENT, &MAIL, &FOLDER];
    let mut n = 0;
    let mut sub = programs.menu(Some(&group), |_| {
        n += 1;
        Some(icon(art[(n - 1) % art.len()], 1))
    });
    sub.hover = Some(1);
    sub.show_keys = menu.show_keys;
    let (sub_fb, _) = menu_image(theme, fonts, &sub, MenuStyle::Cascade);
    let parent = Rect::new(x, y, fb.width, fb.height);
    let entry = entry_on_screen(parent, &layout, programs_index);
    let (sx, sy) = place_submenu(parent, entry, sub_fb.width, sub_fb.height, 3, theme.start_menu.submenu, screen_rect);
    screen.blit(&sub_fb, sx, sy);
    screen
}

/// What a long-menu scene shows (brief 91).
#[derive(Clone, Copy, PartialEq, Eq)]
enum LongScene {
    /// The Programs menu as it opens (opened by the keyboard: the first entry chosen).
    Open,
    /// Scrolled to the middle.
    Middle,
    /// Scrolled to the end, the pointer on the top strip.
    EndStripHot,
    /// Windows 2000 with "Scroll the Programs menu" on.
    ScrollSetting,
    /// The test folder "Long Folder" (60 items) open beside its entry.
    Submenu,
}

/// A `w`×`h` screen with the bar, the Start menu (Programs chosen) and a Programs menu of `n`
/// entries longer than the screen, laid out and placed as the bar does it (`start::long_rule`,
/// `place_columns`, scrolling where columns would not fit): the survey's test entries ("Long
/// Folder", "Entry nnn") after a few folders. Brief 91.
fn long_menu_scene(theme: &Theme, era: Era, fonts: &Fonts, w: i32, h: i32, n: usize, scene: LongScene) -> Framebuffer {
    let screen_rect = Rect::new(0, 0, w, h);
    let mut screen = Framebuffer::new(w, h, desktop(era));
    let mut bar = Framebuffer::new(w, theme.bar.height, 0);
    paint::paint_bar(&mut bar, theme, fonts, &Bar { start_pressed: true, clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(), ..Bar::default() });
    let bar_top = h - bar.height;
    screen.blit(&bar, 0, bar_top);
    let mut menu = start::menu(era, StartOptions::default(), "Admin", |e| icons::start_icon(era, e));
    let programs_index = menu.entries.iter().position(|e| e.label == "&Programs").expect("Programs");
    menu.hover = Some(programs_index);
    menu.show_keys = true;
    let layout = paint::layout_menu(theme, fonts, &menu, MenuStyle::Start, h);
    let mut fb = Framebuffer::new(layout.width, layout.height, 0);
    paint::paint_menu(&mut fb, theme, fonts, &menu, MenuStyle::Start, &layout);
    let button = layout::start_button(theme, fonts);
    let button = Rect::new(button.x, bar_top + button.y, button.w, button.h);
    let (x, y) = place_start_menu(button, fb.width, fb.height, screen_rect);
    screen.blit(&fb, x, y);
    let parent = Rect::new(x, y, fb.width, fb.height);
    let entry = entry_on_screen(parent, &layout, programs_index);

    let group = icons::program_group(era);
    let folders: &[&str] = if era == Era::Win2000 { &["Accessories", "Long Folder", "Startup"] } else { &["Accessories", "Long Folder", "StartUp"] };
    let mut labels: Vec<MenuEntry> = folders.iter().map(|f| MenuEntry::submenu(f, Some(group.clone()))).collect();
    let first = if era == Era::Win2000 { 100 } else { 0 };
    labels.extend((first..first + n - folders.len()).map(|i| MenuEntry::item(&format!("Entry {i:03}"), Some(icon(&DOCUMENT, 1)))));
    let mut programs = Menu::new(labels);
    programs.show_keys = true;
    programs.hover = Some(if scene == LongScene::Submenu { 1 } else { 0 });
    let scroll_programs = scene == LongScene::ScrollSetting;
    // Lays out and places a menu cascading from `entry` of `parent`: `start::place_cascade`, the
    // code the bar runs (brief 92).
    let place = |menu: &Menu, long: Long, offset: i32, parent: Rect, entry: Rect, in_columns: bool| {
        let (l, x, y) = start::place_cascade(theme, fonts, era, menu, MenuStyle::Cascade, long, offset, Scale::whole(1), screen_rect, parent, entry, in_columns);
        (l, x, y, parent)
    };
    let long = start::long_rule(era, start::LongKind::Programs, scroll_programs);
    let (mut pl, px, py, _) = place(&programs, long, 0, parent, entry, false);
    if let Some(mut s) = pl.scroll {
        match scene {
            LongScene::Middle => s.set(s.end / 2 / s.row * s.row),
            LongScene::EndStripHot => {
                s.set(s.end);
                s.hot = Some(Strip::Up);
            }
            _ => {}
        }
        pl.scroll = Some(s);
    }
    let mut pfb = Framebuffer::new(pl.width, pl.height, 0);
    paint::paint_menu(&mut pfb, theme, fonts, &programs, MenuStyle::Cascade, &pl);
    screen.blit(&pfb, px, py);
    if scene == LongScene::Submenu {
        let items: Vec<MenuEntry> = (0..60).map(|i| MenuEntry::item(&format!("Item {:02}", i + if era == Era::Win2000 { 10 } else { 0 }), Some(icon(&DOCUMENT, 1)))).collect();
        let mut sub = Menu::new(items);
        sub.show_keys = true;
        let p = Rect::new(px, py, pl.width, pl.height);
        let e = pl.shown_rect(1).unwrap();
        let e = Rect::new(px + e.x, py + e.y, e.w, e.h);
        let sub_long = start::long_rule(era, start::LongKind::Programs, false);
        let (sl, sx, sy, _) = place(&sub, sub_long, 0, p, e, pl.columns.len() > 1);
        let mut sfb = Framebuffer::new(sl.width, sl.height, 0);
        paint::paint_menu(&mut sfb, theme, fonts, &sub, MenuStyle::Cascade, &sl);
        screen.blit(&sfb, sx, sy);
    }
    screen
}

/// Windows 98's own Start menu on a screen too short for it (640x360, as a 1280x720 screen at Size
/// 2; or `h` rows): it scrolls, its banner standing still and whole, the strips over the entries'
/// part only (brief 92; as the Reference PC's Start menu showed it when it had to scroll), within
/// the room above the Start button, ending on top of the bar with the button showing (brief 94).
/// At its top, or scrolled to the end with the pointer on the top strip. Laid out and placed by
/// `start::place_start`, the code the bar runs.
fn start_scrolls_scene(theme: &Theme, fonts: &Fonts, end: bool) -> Framebuffer {
    start_scrolls_scene_sized(theme, fonts, 640, 360, end)
}

fn start_scrolls_scene_sized(theme: &Theme, fonts: &Fonts, w: i32, h: i32, end: bool) -> Framebuffer {
    let era = Era::Win98;
    let screen_rect = Rect::new(0, 0, w, h);
    let mut screen = Framebuffer::new(w, h, desktop(era));
    let mut bar = Framebuffer::new(w, theme.bar.height, 0);
    paint::paint_bar(&mut bar, theme, fonts, &Bar { start_pressed: true, clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(), ..Bar::default() });
    let bar_top = h - bar.height;
    screen.blit(&bar, 0, bar_top);
    let mut menu = start::menu(era, StartOptions::default(), "Admin", |e| icons::start_icon(era, e));
    menu.show_keys = true;
    menu.hover = Some(if end { menu.entries.len() - 1 } else { 0 });
    let long = start::long_rule(era, start::LongKind::Other, false);
    let button = layout::start_button(theme, fonts);
    let button = Rect::new(button.x, bar_top + button.y, button.w, button.h);
    let (mut layout, x, y) = start::place_start(theme, fonts, era, &menu, MenuStyle::Start, long, 0, Scale::whole(1), screen_rect, button);
    if end && let Some(mut s) = layout.scroll {
        s.set(s.end);
        s.hot = Some(Strip::Up);
        layout.scroll = Some(s);
    }
    let mut fb = Framebuffer::new(layout.width, layout.height, 0);
    paint::paint_menu(&mut fb, theme, fonts, &menu, MenuStyle::Start, &layout);
    screen.blit(&fb, x, y);
    screen
}

/// The Start menu with "Show small icons in Start menu" on (16×16 icons, no banner), pointer
/// on Settings.
fn start_menu_small_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    let screen_rect = Rect::new(0, 0, WIDTH, SCREEN_H);
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, true, three_groups());
    let bar_top = SCREEN_H - bar.height;
    screen.blit(&bar, 0, bar_top);
    let mut menu = start::menu(era, StartOptions::default(), "Admin", |e| icons::start_icon_small(era, e));
    menu.hover = menu.entries.iter().position(|e| e.label == "&Settings");
    menu.show_keys = era.always_shows_keys();
    let (fb, _) = menu_image(theme, fonts, &menu, MenuStyle::StartSmall);
    let button = layout::start_button(theme, fonts);
    let button = Rect::new(button.x, bar_top + button.y, button.w, button.h);
    let (x, y) = place_start_menu(button, fb.width, fb.height, screen_rect);
    screen.blit(&fb, x, y);
    screen
}

/// One of the two property sheets on page `page`: Taskbar Properties (0 Taskbar Options /
/// General, 1 Start Menu Programs / Advanced) or Bevel95 Settings (0 General, 1 Taskbar, 2 Desktop; its
/// Programs page has a scene of its own, `programs_scene`), as it opens: the usual settings,
/// Start with Windows on, Bevel95's clock set 5 minutes ahead (so Use the PC's Time is on), its
/// desktop switched on.
fn properties_scene(theme: &Theme, era: Era, fonts: &Fonts, sheet: properties::Sheet, page: usize) -> Framebuffer {
    let values = properties::Values {
        options: TaskbarOptions::default(),
        start: StartOptions::default(),
        era,
        colors: theme.depth,
        scale: bevel_core::scale::Scale::whole(2),
        start_with_windows: true,
        clock_offset: 300,
        desktop: true,
        programs: Shown::Everything,
        windows_key: bevel_core::config::WindowsKey::Find,
        quick_search_all: false,
        battery_percent: false,
        windows_display: false,
        quick_launch: bevel_core::quick_launch::on_by_default(era),
        group: true,
    };
    let mut p = properties::Properties::new(sheet, era, values);
    p.show_page(page);
    let l = properties::layout(&p, fonts);
    let mut fb = Framebuffer::new(l.width, l.height, 0);
    properties::paint(&mut fb, theme, fonts, &p, &l);
    fb
}

/// Bevel95 Settings' General page at a size between whole numbers (1.75): how it is drawn can be
/// chosen and the line says it is not pixel-exact. `open`: that drop-down list open, its
/// highlight one entry on from the chosen one; otherwise the keyboard on the way's list.
fn quarter_size_scene(theme: &Theme, era: Era, fonts: &Fonts, open: Option<properties::Part>) -> Framebuffer {
    use bevel_core::scale::{Scale, Way};
    let values = properties::Values {
        options: TaskbarOptions::default(),
        start: StartOptions::default(),
        era,
        colors: theme.depth,
        scale: Scale::from_quarters(7).with_way(Way::Even),
        start_with_windows: true,
        clock_offset: 300,
        desktop: true,
        programs: Shown::Everything,
        windows_key: bevel_core::config::WindowsKey::Find,
        quick_search_all: false,
        battery_percent: false,
        windows_display: false,
        quick_launch: bevel_core::quick_launch::on_by_default(era),
        group: true,
    };
    let mut p = properties::Properties::new(properties::Sheet::Bevel95, era, values);
    p.show_keys = true;
    match open {
        Some(list) => {
            p.focus = list;
            p.drop = Some((list, p.list_chosen(list).unwrap_or(0) + 1));
        }
        None => p.focus = properties::Part::WayList,
    }
    let l = properties::layout(&p, fonts);
    let mut fb = Framebuffer::new(l.width, l.height, 0);
    properties::paint(&mut fb, theme, fonts, &p, &l);
    fb
}

/// A small Programs menu for the trees of programs: Accessories with Games and System Tools
/// (two programs each) and three programs of its own, an empty StartUp, three programs in
/// Programs itself.
fn sample_programs() -> Folder {
    let item = |name: &str| Item { name: name.into(), target: format!("C:\\{name}.lnk") };
    let mut tree = Folder::default();
    for name in ["Minesweeper", "Solitaire"] {
        tree.add(&["Accessories", "Games"], item(name));
    }
    for name in ["Character Map", "Disk Cleanup"] {
        tree.add(&["Accessories", "System Tools"], item(name));
    }
    for name in ["Calculator", "Notepad", "Paint"] {
        tree.add(&["Accessories"], item(name));
    }
    tree.folder_mut(&["StartUp"]);
    for name in ["Internet Explorer", "MS-DOS Prompt", "Windows Explorer"] {
        tree.add(&[], item(name));
    }
    tree.sort();
    tree
}

/// The three of those programs that have pictures of the core's own.
fn sample_program_icon(era: Era, item: &Item) -> Option<Icon> {
    match item.name.as_str() {
        "Minesweeper" => Some(bevel_core::minefield::art::icon(era, 16)),
        "Solitaire" => Some(bevel_core::solitaire::art::icon(era, 16)),
        "Notepad" => Some(window_icons::icon(era, ClassicWindow::Notepad)),
        _ => None,
    }
}

/// Quick Search with "p" typed: Paint begins with it, MS-DOS
/// Prompt has a word that does, six more have it; the first is selected, the caret after the
/// "p". The pictures are those of the sample programs that have one.
fn find_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    use bevel_core::find::{self, FindWindow};
    use bevel_core::kit::Form;
    let mut window = FindWindow::new(era, find::programs(&sample_programs()), true);
    window.typed('p');
    for i in window.wanted_icons() {
        let item = Item { name: window.programs[i].name.clone(), target: window.programs[i].target.clone() };
        window.set_icon(i, sample_program_icon(era, &item));
    }
    window.layout(fonts);
    let (w, h) = window.size();
    let mut fb = Framebuffer::new(w, h, 0);
    window.paint(&mut fb, theme, fonts);
    fb
}

/// "Remove Shortcuts/Folders" (Taskbar Properties, Remove...) with the same small Programs
/// menu, every folder open (15 rows, so the tree scrolls), Solitaire selected and the focus
/// moved on to Remove: the selection stays on gray, Remove is the framed button, and each
/// program's picture has the shortcut arrow.
fn remove_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    use bevel_core::kit::Form;
    use bevel_core::remove::{self, Part, RemoveWindow};
    let rows = remove::rows(&sample_programs(), |item| sample_program_icon(era, item));
    let mut window = RemoveWindow::new(era, rows);
    window.typed('*');
    window.sel = window.rows.iter().position(|row| row.name == "Solitaire").expect("Solitaire");
    window.focus = Part::Remove;
    window.show_keys = true;
    window.layout(fonts);
    let (w, h) = window.size();
    let mut fb = Framebuffer::new(w, h, 0);
    window.paint(&mut fb, theme, fonts);
    fb
}

/// Bevel95 Settings' Programs page with a small Programs menu as its tree (three programs have
/// pictures of the core's own; the StartUp folder has nothing in it and so gets no row).
/// `Everything`: as the page opens, every box ticked and the folders closed. `Classic`: that
/// choice just made, so the folders of Bevel95's own programs are open. `Ticked`: every folder
/// opened (14 rows, so the list scrolls), System Tools' two programs and Paint unticked, so
/// Accessories and Programs are half ticked, and the list has the focus, on Notepad.
fn programs_scene(theme: &Theme, era: Era, fonts: &Fonts, shown: Shown) -> Framebuffer {
    let rows = programs::rows(&sample_programs(), None, |item| {
        (matches!(item.name.as_str(), "Minesweeper" | "Solitaire"), sample_program_icon(era, item))
    });
    let values = properties::Values {
        options: TaskbarOptions::default(),
        start: StartOptions::default(),
        era,
        colors: theme.depth,
        scale: bevel_core::scale::Scale::whole(2),
        start_with_windows: true,
        clock_offset: 300,
        desktop: true,
        programs: Shown::Everything,
        windows_key: bevel_core::config::WindowsKey::Find,
        quick_search_all: false,
        battery_percent: false,
        windows_display: false,
        quick_launch: bevel_core::quick_launch::on_by_default(era),
        group: true,
    };
    let mut p = properties::Properties::new(properties::Sheet::Bevel95, era, values);
    p.show_page(3);
    p.set_programs(rows);
    if shown != Shown::Everything {
        assert!(p.choose(properties::Part::Shown(shown)));
        p.focus = properties::Part::Shown(shown);
    }
    if shown == Shown::Ticked {
        let row = |name: &str| p.programs.rows.iter().position(|r| r.name == name).expect(name);
        let (tools, paint, notepad) = (row("System Tools"), row("Paint"), row("Notepad"));
        p.open_all_programs();
        assert!(p.choose(properties::Part::ProgramRow(tools)) && p.choose(properties::Part::ProgramRow(paint)));
        p.focus = properties::Part::Programs;
        p.programs.sel = notepad;
        p.show_keys = true;
    }
    let l = properties::layout(&p, fonts);
    let mut fb = Framebuffer::new(l.width, l.height, 0);
    properties::paint(&mut fb, theme, fonts, &p, &l);
    fb
}

/// Windows 98's Start menu with Favorites open (two folders and a link) and the first folder's
/// menu beside it (links only, so no room is kept for arrows).
fn favorites_scene(theme: &Theme, fonts: &Fonts) -> Framebuffer {
    let era = Era::Win98;
    let screen_rect = Rect::new(0, 0, WIDTH, SCREEN_H);
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, true, vec![]);
    let bar_top = SCREEN_H - bar.height;
    screen.blit(&bar, 0, bar_top);

    let mut menu = start::menu(era, StartOptions::default(), "Admin", |e| icons::start_icon(era, e));
    let favorites_index = menu.entries.iter().position(|e| e.label == "F&avorites").expect("Favorites");
    menu.hover = Some(favorites_index);
    menu.show_keys = true;
    let (fb, layout) = menu_image(theme, fonts, &menu, MenuStyle::Start);
    let button = layout::start_button(theme, fonts);
    let button = Rect::new(button.x, bar_top + button.y, button.w, button.h);
    let (x, y) = place_start_menu(button, fb.width, fb.height, screen_rect);
    screen.blit(&fb, x, y);

    let mut favorites = Folder::default();
    for folder in ["Channels", "Links"] {
        favorites.folder_mut(&[folder]);
    }
    favorites.add(&[], Item { name: "MSN".to_string(), target: String::new() });
    for name in ["Microsoft Channel Guide", "Windows Update"] {
        favorites.add(&["Channels"], Item { name: name.to_string(), target: String::new() });
    }
    let folder = icons::favorites_folder();
    let mut sub = favorites.menu(Some(&folder), |_| Some(icon(&DOCUMENT, 1)));
    sub.hover = Some(0);
    sub.show_keys = true;
    let (sub_fb, sub_layout) = menu_image(theme, fonts, &sub, MenuStyle::Cascade);
    let parent = Rect::new(x, y, fb.width, fb.height);
    let entry = entry_on_screen(parent, &layout, favorites_index);
    let rule = theme.start_menu.submenu;
    let (sx, sy) = place_submenu(parent, entry, sub_fb.width, sub_fb.height, 3, rule, screen_rect);
    screen.blit(&sub_fb, sx, sy);

    let channels = favorites.folder_at(&[0]).expect("Channels").menu(Some(&folder), |_| Some(icon(&DOCUMENT, 1)));
    let (ch_fb, _) = menu_image(theme, fonts, &channels, MenuStyle::Cascade);
    let sub_rect = Rect::new(sx, sy, sub_fb.width, sub_fb.height);
    let entry = entry_on_screen(sub_rect, &sub_layout, 0);
    let (cx, cy) = place_submenu(sub_rect, entry, ch_fb.width, ch_fb.height, 3, rule, screen_rect);
    screen.blit(&ch_fb, cx, cy);
    screen
}

/// The bar's right-click menu, opened by a click on the bar at x = 300 and placed the way
/// the real one is (it flips upward because there is no room below), pointer on the
/// first entry, "Start with Windows" switched on.
fn context_menu_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, false, vec![]);
    screen.blit(&bar, 0, SCREEN_H - bar.height);
    let menu = Menu { hover: Some(0), ..bar_menu::menu(era, false) };
    let (fb, _) = menu_image(theme, fonts, &menu, MenuStyle::Popup);
    let (x, y) = place_popup(300, SCREEN_H - 14, fb.width, fb.height, Rect::new(0, 0, WIDTH, SCREEN_H));
    screen.blit(&fb, x, y);
    screen
}

/// Quick Launch's menus over the bar with the band (as `quick_launch_scene`), placed as the real
/// windows are: `which` "toolbars" (the bar's menu, right-clicked on the empty task area at x 300,
/// the pointer on Toolbars and its submenu open, Quick Launch ticked), "band" (the band's own menu,
/// right-clicked on its first grip, the pointer on Open) or "icon" (the menu on the first icon,
/// right-clicked on it, nothing highlighted).
fn quick_launch_menu_scene(theme: &Theme, era: Era, fonts: &Fonts, which: &str) -> Framebuffer {
    use bevel_core::quick_launch;
    let screen_rect = Rect::new(0, 0, WIDTH, SCREEN_H);
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = quick_launch_scene(theme, era, fonts, None, None, WIDTH);
    let bar_top = SCREEN_H - bar.height;
    screen.blit(&bar, 0, bar_top);
    let show_keys = era.always_shows_keys();
    let band = {
        let icons = quick_launch::builtins(era).iter().map(|&b| quick_launch::picture(era, b)).collect();
        let state = Bar { tasks: three_groups(), clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(), quick_launch: Some(quick_launch::Band { icons, ..Default::default() }), ..Bar::default() };
        paint::bar_layout(theme, fonts, WIDTH, &state).quick.expect("the band")
    };
    match which {
        "toolbars" => {
            let index = bar_menu::index_of(era, false, BarEntry::Toolbars).expect("the Toolbars entry");
            let menu = Menu { hover: Some(index), show_keys, ..bar_menu::menu(era, false) };
            let (fb, layout) = menu_image(theme, fonts, &menu, MenuStyle::Popup);
            let (x, y) = place_popup(300, SCREEN_H - 14, fb.width, fb.height, screen_rect);
            screen.blit(&fb, x, y);
            let sub = Menu { show_keys, ..bar_menu::toolbars_menu(true) };
            let (sub_fb, _) = menu_image(theme, fonts, &sub, MenuStyle::Popup);
            let parent = Rect::new(x, y, fb.width, fb.height);
            let entry = entry_on_screen(parent, &layout, index);
            let (sx, sy) = place_submenu(parent, entry, sub_fb.width, sub_fb.height, 3, theme.popup.submenu, screen_rect);
            screen.blit(&sub_fb, sx, sy);
        }
        "band" => {
            let open = (0..20).find(|&i| quick_launch::band_entry(era, i) == Some(quick_launch::BandEntry::Open));
            let menu = Menu { hover: open, show_keys, ..quick_launch::band_menu(era) };
            let (fb, _) = menu_image(theme, fonts, &menu, MenuStyle::Popup);
            let (x, y) = place_popup(band.grip + 1, bar_top + 14, fb.width, fb.height, screen_rect);
            screen.blit(&fb, x, y);
        }
        _ => {
            let menu = Menu { show_keys, ..quick_launch::icon_menu(false) };
            let (fb, _) = menu_image(theme, fonts, &menu, MenuStyle::Popup);
            let (x, y) = place_popup(band.buttons[0].x + 11, bar_top + 14, fb.width, fb.height, screen_rect);
            screen.blit(&fb, x, y);
        }
    }
    screen
}

/// Bevel95's desktop with its right-click menu, opened by a click at (200, 150) and placed the
/// way Windows places such a menu (its corner at the pointer), pointer on the first entry.
fn desktop_menu_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, false, vec![]);
    screen.blit(&bar, 0, SCREEN_H - bar.height);
    let menu = Menu { hover: Some(0), ..bevel_core::desktop::menu(era, false) };
    let (fb, _) = menu_image(theme, fonts, &menu, MenuStyle::Popup);
    let (x, y) = place_popup(200, 150, fb.width, fb.height, Rect::new(0, 0, WIDTH, SCREEN_H));
    screen.blit(&fb, x, y);
    screen
}

/// Bevel95's desktop with its icons: the look's own, then sample items of the owner's (a
/// folder, a shortcut and a text file with test pictures, a long name). The desktop has the
/// keyboard; the long name is selected and has the focus, so it shows in full; the Recycle Bin
/// is full.
fn desktop_icons_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    use bevel_core::desktop_icons::{Desktop, Item as DeskItem, Picture};
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, false, vec![]);
    let item = |name: &str, picture: Picture, shortcut: bool, folder: bool| DeskItem {
        id: format!("C:\\Users\\Owner\\Desktop\\{name}"),
        name: name.to_string(),
        picture,
        shortcut,
        folder,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    let mut d = Desktop::new(era, Rect::new(0, 0, WIDTH, SCREEN_H - bar.height));
    d.set_files(vec![
        item("Projects", Picture::Folder, false, true),
        item("Notepad", Picture::Own(Some(icon(&DOCUMENT, 2))), true, false),
        item("todo.txt", Picture::Own(Some(icon(&MAIL, 2))), false, false),
        item("Zebra notes - the quick brown fox jumps over the lazy dog.txt", Picture::Own(Some(icon(&DOCUMENT, 2))), false, false),
    ]);
    d.recycle_full = true;
    d.active = true;
    d.select_only(d.index_of("C:\\Users\\Owner\\Desktop\\Zebra notes - the quick brown fox jumps over the lazy dog.txt"));
    d.paint(&mut screen, theme, fonts, Rect::new(0, 0, WIDTH, SCREEN_H - bar.height));
    screen.blit(&bar, 0, SCREEN_H - bar.height);
    screen
}

/// The classic desktop with a wallpaper or a pattern of the look's own (`display::background`
/// over the whole screen), the look's icons and two of the owner's items, My Computer selected
/// with the keyboard.
fn wallpaper_desktop_scene(theme: &Theme, era: Era, fonts: &Fonts, which: &str) -> Framebuffer {
    use bevel_core::desktop_icons::{Desktop, Item as DeskItem, Picture};
    use bevel_core::display::{self, Choice, Show};
    let (wallpaper, show, pattern) = match (era, which) {
        (Era::Win95, "desktop-wallpaper-tile") => (Some("Bubbles"), Show::Tile, None),
        (Era::Win95, "desktop-pattern") => (None, Show::Center, Some("Plaid")),
        (Era::Win95, _) => (Some("Bubbles"), Show::Center, Some("Waffle's Revenge")),
        (Era::Win98, "desktop-wallpaper-tile") => (Some("Forest"), Show::Tile, None),
        (Era::Win98, "desktop-pattern") => (None, Show::Center, Some("Daisies")),
        (Era::Win98, "desktop-wallpaper-center") => (Some("Red Blocks"), Show::Center, Some("Waffle's Revenge")),
        (Era::Win98, _) => (Some("Clouds"), Show::Stretch, Some("Bricks")),
        (_, "desktop-wallpaper-tile") => (Some("Zapotec"), Show::Tile, None),
        (_, "desktop-pattern") => (None, Show::Center, Some("Scottie")),
        (_, "desktop-wallpaper-center") => (Some("Zapotec"), Show::Center, Some("Tulip")),
        _ => (Some("Prairie Wind"), Show::Stretch, None),
    };
    let choice = Choice { wallpaper: wallpaper.map(str::to_string), show, pattern: pattern.map(str::to_string) };
    let background = display::background(theme, &choice, WIDTH, SCREEN_H).expect("a wallpaper or a pattern");
    let mut screen = background.clone();
    let bar = bar_image(theme, fonts, false, vec![]);
    let item = |name: &str, picture: Picture, folder: bool| DeskItem {
        id: format!("C:\\Users\\Owner\\Desktop\\{name}"),
        name: name.to_string(),
        picture,
        shortcut: false,
        folder,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    let area = Rect::new(0, 0, WIDTH, SCREEN_H - bar.height);
    let mut d = Desktop::new(era, area);
    d.set_background(Some(std::sync::Arc::new(background)));
    d.set_files(vec![item("Projects", Picture::Folder, true), item("todo.txt", Picture::Own(Some(icon(&MAIL, 2))), false)]);
    d.active = true;
    d.keys_used = true;
    d.select_only(Some(d.index_of("::my-computer").expect("My Computer")));
    d.focus = d.index_of("::my-computer");
    d.paint(&mut screen, theme, fonts, area);
    screen.blit(&bar, 0, SCREEN_H - bar.height);
    screen
}

/// A photo-like test picture of the tests' own making: an evening sky's gradient, a sun, and fine
/// lines (every 9th column and row a dark line) that show how it is scaled.
fn test_photo(w: i32, h: i32) -> bevel_core::display::Picture {
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = y * 255 / h.max(1);
            let (mut r, mut g, mut b) = (40 + t * 200 / 255, 60 + t * 120 / 255, 160 - t * 100 / 255);
            let (dx, dy) = (x - w * 2 / 3, y - h * 3 / 5);
            if dx * dx + dy * dy < (h / 6) * (h / 6) {
                (r, g, b) = (255, 220, 120);
            }
            if x % 9 == 0 || y % 9 == 0 {
                (r, g, b) = (r / 3, g / 3, b / 3);
            }
            pixels.push(0xFF00_0000 | (r as u32) << 16 | (g as u32) << 8 | b as u32);
        }
    }
    bevel_core::display::Picture { width: w, height: h, pixels }
}

/// Display Properties' Background page, or the Pattern window, in the look's own design.
fn display_scene(theme: &Theme, era: Era, fonts: &Fonts, which: &str) -> Framebuffer {
    use bevel_core::display::form::DisplayProperties;
    use bevel_core::display::pattern::PatternWindow;
    use bevel_core::display::{Choice, Show};
    let pattern_name = if era == Era::Win2000 { "Weave" } else { "Bricks" };
    if which == "display-pattern-window" {
        let w = PatternWindow::new(era, Some(pattern_name));
        let mut fb = Framebuffer::new(318, 205, 0);
        w.paint(&mut fb, theme, fonts);
        return fb;
    }
    let mut p = DisplayProperties::new(era, Choice::plain());
    match which {
        "display-wallpaper" => {
            let name = match era {
                Era::Win95 => "Bubbles",
                Era::Win98 => "Forest",
                Era::Win2000 => "Zapotec",
            };
            p.set_shown(Choice { wallpaper: Some(name.into()), show: Show::Center, pattern: None });
            p.focus = bevel_core::display::form::Part::List(bevel_core::display::form::Which::Wallpaper);
        }
        "display-pattern" => p.set_shown(Choice { wallpaper: None, show: Show::Center, pattern: Some(pattern_name.into()) }),
        // A person's own picture chosen with Browse... (a test picture made here), in the list by
        // its name and on the monitor by the BMPs' rule, Center.
        "display-own-picture" => {
            let path = "C:\\Users\\Someone\\Pictures\\Evening Sky.jpg";
            p.set_shown(Choice { wallpaper: Some(path.into()), show: Show::Center, pattern: None });
            p.set_read(bevel_core::display::PictureId::Own(path.into()), Some(std::sync::Arc::new(test_photo(640, 400))));
            p.focus = bevel_core::display::form::Part::List(bevel_core::display::form::Which::Wallpaper);
        }
        // 2000's JPEG entries, one chosen as on the Reference PC (its monitor shows the desktop
        // color: a golden picture is drawn without the bar's reader of JPEGs).
        "display-jpeg-list" => {
            p.set_shown(Choice { wallpaper: Some("Windows 2000".into()), show: Show::Center, pattern: None });
            p.focus = bevel_core::display::form::Part::List(bevel_core::display::form::Which::Wallpaper);
        }
        _ => {}
    }
    let mut fb = Framebuffer::new(404, 448, 0);
    p.paint(&mut fb, theme, fonts);
    fb
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ItemScene {
    Menu,
    Rename,
    Cut,
    Confirm,
    ExtensionWarning,
    /// The bar's question after a size between whole numbers was chosen in its Size menu.
    KeepSize,
}

/// Bevel95's desktop with some of the owner's items (test pictures) and the menu on an icon,
/// a label being renamed, a cut icon, or the question before a delete.
fn desktop_item_scene(theme: &Theme, era: Era, fonts: &Fonts, scene: ItemScene) -> Framebuffer {
    use bevel_core::delete_confirm::{ConfirmWindow, Question};
    use bevel_core::desktop::{self as desk, ItemKind};
    use bevel_core::desktop_icons::{Desktop, EditKey, Item as DeskItem, Picture};
    use bevel_core::kit::Form;
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, false, vec![]);
    let item = |name: &str, picture: Picture, folder: bool| DeskItem {
        id: format!("C:\\Users\\Owner\\Desktop\\{name}"),
        name: name.to_string(),
        picture,
        shortcut: false,
        folder,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    let area = Rect::new(0, 0, WIDTH, SCREEN_H - bar.height);
    let mut d = Desktop::new(era, area);
    d.set_files(vec![
        item("Projects", Picture::Folder, true),
        item("letter.txt", Picture::Own(Some(icon(&DOCUMENT, 2))), false),
        item("mail.txt", Picture::Own(Some(icon(&MAIL, 2))), false),
    ]);
    d.active = true;
    d.keys_used = true;
    let letter = d.index_of("C:\\Users\\Owner\\Desktop\\letter.txt").unwrap();
    d.select_only(Some(letter));
    match scene {
        ItemScene::Rename => {
            d.begin_rename(letter);
            for c in "Letter to Mum".chars() {
                d.rename_key(EditKey::Char(c), false);
            }
            d.rename_key(EditKey::Left, true);
            d.rename_key(EditKey::Left, true);
            d.rename_key(EditKey::Left, true);
        }
        ItemScene::Cut => {
            d.set_cut(["C:\\Users\\Owner\\Desktop\\mail.txt".to_string(), "C:\\Users\\Owner\\Desktop\\letter.txt".to_string()]);
        }
        ItemScene::ExtensionWarning => {
            d.begin_rename(letter);
            d.rename_paste("letter.dat");
            d.rename_asking(true);
        }
        ItemScene::KeepSize => d.select_only(None),
        ItemScene::Menu | ItemScene::Confirm => {}
    }
    if matches!(scene, ItemScene::Confirm | ItemScene::ExtensionWarning | ItemScene::KeepSize) {
        d.active = false;
    }
    d.paint(&mut screen, theme, fonts, area);
    screen.blit(&bar, 0, SCREEN_H - bar.height);
    match scene {
        ItemScene::Menu => {
            let r = d.icon_rect(letter);
            let menu = Menu { hover: Some(0), ..desk::item_menu(era, &[ItemKind::File], false) };
            let (fb, _) = menu_image(theme, fonts, &menu, MenuStyle::Popup);
            let (x, y) = place_popup(r.x + 16, r.y + 16, fb.width, fb.height, Rect::new(0, 0, WIDTH, SCREEN_H));
            screen.blit(&fb, x, y);
        }
        ItemScene::Confirm | ItemScene::KeepSize => {
            let question =
                if matches!(scene, ItemScene::KeepSize) { Question::KeepSize } else { Question::File("letter.txt".into()) };
            let mut w = ConfirmWindow::new(era, question, false);
            w.layout(fonts);
            let (ww, wh) = w.size();
            let mut fb = Framebuffer::new(ww, wh, 0);
            w.paint(&mut fb, theme, fonts);
            screen.blit(&fb, (WIDTH - ww) / 2, (SCREEN_H - wh) / 2);
        }
        ItemScene::ExtensionWarning => {
            let mut w = bevel_core::rename_warning::RenameWarning::new(era, true);
            w.layout(fonts);
            let (ww, wh) = w.size();
            let mut fb = Framebuffer::new(ww, wh, 0);
            w.paint(&mut fb, theme, fonts);
            let (x, y) = bevel_core::rename_warning::place((WIDTH, SCREEN_H), (ww, wh));
            screen.blit(&fb, x, y);
        }
        _ => {}
    }
    screen
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Moving {
    Select,
    DragOne,
    DragSeveral,
}

/// Bevel95's desktop after some icons were moved (by the real drag code: `Desktop::press`,
/// `drag_to`, `release`), in the middle of a selection rectangle or a drag.
fn desktop_moving_scene(theme: &Theme, era: Era, fonts: &Fonts, moving: Moving) -> Framebuffer {
    use bevel_core::desktop_icons::{Button, Desktop, Item as DeskItem, Mods, Picture};
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, false, vec![]);
    let item = |name: &str, picture: Picture, folder: bool| DeskItem {
        id: format!("C:\\Users\\Owner\\Desktop\\{name}"),
        name: name.to_string(),
        picture,
        shortcut: false,
        folder,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    let area = Rect::new(0, 0, WIDTH, SCREEN_H - bar.height);
    let mut d = Desktop::new(era, area);
    d.set_files(vec![
        item("Projects", Picture::Folder, true),
        item("letter.txt", Picture::Own(Some(icon(&DOCUMENT, 2))), false),
        item("mail.txt", Picture::Own(Some(icon(&MAIL, 2))), false),
    ]);
    d.active = true;
    let f = fonts.regular;
    let none = Mods::default();
    let drag = |d: &mut Desktop, i: usize, dx: i32, dy: i32| {
        let r = d.icon_rect(i);
        d.press(f, r.x + 8, r.y + 8, Button::Left, none);
        d.drag_to(f, r.x + 8 + dx, r.y + 8 + dy);
        d.release(f, r.x + 8 + dx, r.y + 8 + dy);
    };
    let at = |d: &Desktop, name: &str| d.items.iter().position(|i| i.name == name).unwrap();
    // Spread out: the folder to the middle, the letter beside it, the mail a little lower.
    for (name, dx, dy) in [("Projects", 230, 40), ("letter.txt", 300, -60), ("mail.txt", 180, 30)] {
        let i = at(&d, name);
        drag(&mut d, i, dx, dy);
    }
    let r = d.icon_rect(at(&d, "Recycle Bin"));
    d.press(f, r.x + 8, r.y + 8, Button::Left, none);
    d.release(f, r.x + 8, r.y + 8);
    let r = d.icon_rect(at(&d, "Projects"));
    d.press(f, r.x + 8, r.y + 8, Button::Left, Mods { ctrl: true, shift: false });
    d.release(f, r.x + 8, r.y + 8);
    match moving {
        Moving::Select => {
            let r = d.icon_rect(at(&d, "letter.txt"));
            d.press(f, r.right() + 60, r.y - 10, Button::Left, Mods { ctrl: true, shift: false });
            d.drag_to(f, r.x - 70, r.y + 150);
        }
        Moving::DragOne => {
            let r = d.icon_rect(at(&d, "Projects"));
            d.press(f, r.x + 8, r.y + 8, Button::Left, none);
            d.release(f, r.x + 8, r.y + 8);
            d.press(f, r.x + 8, r.y + 8, Button::Left, none);
            d.drag_to(f, r.x + 60, r.y + 25);
        }
        Moving::DragSeveral => {
            let r = d.icon_rect(at(&d, "mail.txt"));
            d.press(f, r.x + 8, r.y + 8, Button::Left, Mods { ctrl: true, shift: false });
            d.release(f, r.x + 8, r.y + 8);
            d.press(f, r.x + 8, r.y + 8, Button::Left, none);
            d.drag_to(f, r.x + 8 + 150, r.y + 8 - 120);
        }
    }
    d.paint(&mut screen, theme, fonts, area);
    screen.blit(&bar, 0, SCREEN_H - bar.height);
    screen
}

/// Dragging files: mail.txt carried over the Projects folder (drawn selected as the drop place),
/// its dragged picture as the window beside the pointer shows it (laid over the screen with its
/// see-through pixels), and the look's copy pointer; with `menu`, the right-button drag's menu
/// opened at the drop instead, Move Here bold.
fn drag_files_scene(theme: &Theme, era: Era, fonts: &Fonts, menu: bool) -> Framebuffer {
    use bevel_core::desktop_icons::{Button, Desktop, Item as DeskItem, Mods, Picture};
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, false, vec![]);
    let item = |name: &str, picture: Picture, folder: bool| DeskItem {
        id: format!("C:\\Users\\Owner\\Desktop\\{name}"),
        name: name.to_string(),
        picture,
        shortcut: false,
        folder,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    let area = Rect::new(0, 0, WIDTH, SCREEN_H - bar.height);
    let mut d = Desktop::new(era, area);
    d.set_files(vec![item("Projects", Picture::Folder, true), item("mail.txt", Picture::Own(Some(icon(&MAIL, 2))), false)]);
    d.active = true;
    let f = fonts.regular;
    let at = |d: &Desktop, name: &str| d.items.iter().position(|i| i.name == name).unwrap();
    let mail = at(&d, "mail.txt");
    let r = d.icon_rect(mail);
    let from = (r.x + 8, r.y + 8);
    d.press(f, from.0, from.1, Button::Left, Mods::default());
    d.drag_to(f, from.0 + 40, from.1);
    let (items, from, _) = d.take_drag().expect("a drag began");
    let target = d.icon_rect(at(&d, "Projects"));
    let pointer = (target.x + 14, target.y + 12);
    d.drop_hover = d.drop_target(f, pointer.0, pointer.1, &[d.items[mail].id.clone()]);
    assert!(d.drop_hover.is_some(), "the folder takes the file");
    let picture = d.drag_picture(theme, f, &items, desktop(era));
    d.drop_hover = d.drop_hover.filter(|_| !menu);
    d.paint(&mut screen, theme, fonts, area);
    screen.blit(&bar, 0, SCREEN_H - bar.height);
    if menu {
        let (m, _) = bevel_core::drag::drop_menu(era, bevel_core::drag::Effect::Move, &[bevel_core::drag::Effect::Move, bevel_core::drag::Effect::Copy, bevel_core::drag::Effect::Link]);
        let (fb, _) = menu_image(theme, fonts, &Menu { hover: Some(0), ..m }, MenuStyle::Popup);
        let (x, y) = place_popup(pointer.0, pointer.1, fb.width, fb.height, Rect::new(0, 0, WIDTH, SCREEN_H));
        screen.blit(&fb, x, y);
        return screen;
    }
    if let Some((pic, x0, y0)) = picture {
        let (dx, dy) = (pointer.0 - from.0, pointer.1 - from.1);
        for y in 0..pic.height {
            for x in 0..pic.width {
                let p = pic.get(x, y);
                let a = p >> 24;
                let (sx, sy) = (x0 + dx + x, y0 + dy + y);
                if a == 0 || sx < 0 || sy < 0 || sx >= screen.width || sy >= screen.height {
                    continue;
                }
                let under = screen.get(sx, sy);
                let ch = |s: u32| ((((p >> s) & 0xFF) * a + ((under >> s) & 0xFF) * (255 - a)) / 255) << s;
                screen.set(sx, sy, 0xFF00_0000 | ch(16) | ch(8) | ch(0));
            }
        }
    }
    let art = bevel_core::drag::pointer(era, Some(bevel_core::drag::Effect::Copy));
    let (w, h) = art.size();
    for y in 0..h {
        for x in 0..w {
            let p = art.pixel(x, y);
            if p != 0 {
                screen.set(pointer.0 + x, pointer.1 + y, p);
            }
        }
    }
    screen
}

/// Three apps, one of them with three windows (so its button reads "3 Notepad" and is the
/// active one), and that app's list of windows open above its button, pointer on the
/// second window. The buttons come from the real grouping code (`TaskList`).
fn window_list_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    let window = |id: u64, title: &str, app: &str, flashing: bool| WindowInfo {
        id,
        title: title.to_string(),
        group: format!("C:\\Program Files\\{app}\\{app}.exe"),
        app_name: app.to_string(),
        flashing,
        classic: None,
    };
    let windows = [
        window(1, "My Computer", "Explorer", false),
        window(2, "todo.txt - Notepad", "Notepad", false),
        window(3, "Inbox - Outlook Express", "Outlook Express", true),
        window(4, "notes.txt - Notepad", "Notepad", false),
        window(5, "Untitled - Notepad", "Notepad", false),
    ];
    let art = |app: &str| match app {
        "Explorer" => &COMPUTER,
        "Notepad" => &DOCUMENT,
        _ => &MAIL,
    };
    let groups = TaskList::default().update(&windows, Some(4));
    let app_of = |group: &Group| windows.iter().find(|w| w.id == group.windows[0]).map(|w| w.app_name.clone());
    let tasks = groups.iter().map(|g| task(&g.label, art(&app_of(g).unwrap_or_default()), g.state)).collect();

    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let mut bar = Framebuffer::new(WIDTH, theme.bar.height, 0);
    let layout = paint::paint_bar(&mut bar, theme, fonts, &Bar { tasks, clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(), ..Bar::default() });
    screen.blit(&bar, 0, SCREEN_H - bar.height);

    let notepad = groups.iter().position(|g| g.windows.len() == 3).expect("the Notepad group");
    let entries = groups[notepad]
        .windows
        .iter()
        .filter_map(|id| windows.iter().find(|w| w.id == *id))
        .map(|w| MenuEntry::item(&w.title, Some(icon(&DOCUMENT, 1))))
        .collect();
    let menu = Menu { hover: Some(1), ..Menu::new(entries) };
    let (fb, _) = menu_image(theme, fonts, &menu, MenuStyle::Cascade);
    let button = layout.tasks[notepad];
    let (x, y) = place_popup(button.x, SCREEN_H - bar.height, fb.width, fb.height, Rect::new(0, 0, WIDTH, SCREEN_H));
    screen.blit(&fb, x, y);
    screen
}

/// The right-click menu with the pointer on Bevel95 and its submenu open beside it; with
/// `which` (Theme or Size), the pointer on that entry of the submenu and its own submenu open
/// beside it, the current choice (this look; 2×) marked with a bullet. All placed as the real
/// windows are.
fn bar_submenu_scene(theme: &Theme, era: Era, fonts: &Fonts, which: Option<BarEntry>) -> Framebuffer {
    let screen_rect = Rect::new(0, 0, WIDTH, SCREEN_H);
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, false, vec![]);
    screen.blit(&bar, 0, SCREEN_H - bar.height);

    let index = bar_menu::index_of(era, false, BarEntry::Bevel95).expect("the Bevel95 entry");
    let menu = Menu { hover: Some(index), ..bar_menu::menu(era, false) };
    let (fb, layout) = menu_image(theme, fonts, &menu, MenuStyle::Popup);
    let (x, y) = place_popup(300, SCREEN_H - 14, fb.width, fb.height, screen_rect);
    screen.blit(&fb, x, y);

    // The classic desktop on (as it starts) and its key registered.
    let on = bar_menu::Own { desktop: true, desktop_key: true };
    let own_index = which.and_then(|e| bar_menu::own_index_of(on, e));
    let own = Menu { hover: own_index, ..bar_menu::own_menu(on) };
    let (own_fb, own_layout) = menu_image(theme, fonts, &own, MenuStyle::Popup);
    let parent = Rect::new(x, y, fb.width, fb.height);
    let entry = entry_on_screen(parent, &layout, index);
    let (ox, oy) = place_submenu(parent, entry, own_fb.width, own_fb.height, 3, theme.popup.submenu, screen_rect);
    screen.blit(&own_fb, ox, oy);
    let (Some(which), Some(own_index)) = (which, own_index) else {
        return screen;
    };

    let submenu = match which {
        BarEntry::Theme => bar_menu::theme_menu(era),
        BarEntry::Colors => bar_menu::colors_menu(theme.depth),
        _ => bar_menu::size_menu(bevel_core::scale::Scale::whole(2)),
    };
    let (sub, _) = menu_image(theme, fonts, &submenu, MenuStyle::Popup);
    let parent = Rect::new(ox, oy, own_fb.width, own_fb.height);
    let entry = entry_on_screen(parent, &own_layout, own_index);
    let (sx, sy) = place_submenu(parent, entry, sub.width, sub.height, 3, theme.popup.submenu, screen_rect);
    screen.blit(&sub, sx, sy);
    screen
}

/// The menu of Bevel95's own notification icon (`tray_icon::menu`) as its window shows it, nothing
/// highlighted (it opens over Windows' taskbar, which is not drawn here).
fn own_icon_menu_image(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    let menu = Menu { show_keys: era.always_shows_keys(), ..bevel_core::tray_icon::menu() };
    menu_image(theme, fonts, &menu, MenuStyle::Popup).0
}

/// The desktop's right-click menu (as `desktop_menu_scene`) with the pointer on Bevel95 and its
/// submenu open beside it.
fn desktop_own_menu_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    let screen_rect = Rect::new(0, 0, WIDTH, SCREEN_H);
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let bar = bar_image(theme, fonts, false, vec![]);
    screen.blit(&bar, 0, SCREEN_H - bar.height);
    let menu = bevel_core::desktop::menu(era, false);
    let index = menu.entries.len() - 1;
    let menu = Menu { hover: Some(index), ..menu };
    let (fb, layout) = menu_image(theme, fonts, &menu, MenuStyle::Popup);
    let (x, y) = place_popup(200, 150, fb.width, fb.height, screen_rect);
    screen.blit(&fb, x, y);
    let (sub, _) = menu_image(theme, fonts, &bevel_core::desktop::own_menu(true), MenuStyle::Popup);
    let parent = Rect::new(x, y, fb.width, fb.height);
    let entry = entry_on_screen(parent, &layout, index);
    let (sx, sy) = place_submenu(parent, entry, sub.width, sub.height, 3, theme.popup.submenu, screen_rect);
    screen.blit(&sub, sx, sy);
    screen
}

/// A dialog as it first opens (Run with "notepad" typed, to show the text and the caret).
fn dialog_scene(theme: &Theme, fonts: &Fonts, kind: DialogKind, era: Era) -> Framebuffer {
    let mut d = Dialog::new(kind, era);
    if kind == DialogKind::Run {
        d.text = "notepad".to_string();
        d.caret = 7;
    }
    let l = dialog::layout(&d);
    let mut fb = Framebuffer::new(l.width, l.height, 0);
    dialog::paint_dialog(&mut fb, theme, fonts, &d, &l);
    fb
}

/// Run's field with part of its text selected, as Windows' edit box draws a selection (brief 95).
fn field_selected_scene(theme: &Theme, fonts: &Fonts, era: Era) -> Framebuffer {
    let mut d = Dialog::new(DialogKind::Run, era);
    d.text = "notepad readme.txt".to_string();
    d.caret = 7;
    d.anchor = Some(4);
    d.caret_on = true;
    let l = dialog::layout(&d);
    let mut fb = Framebuffer::new(l.width, l.height, 0);
    dialog::paint_dialog(&mut fb, theme, fonts, &d, &l);
    fb
}

/// A text field's right-click menu (`edit::Edit::menu`) drawn by the look's popup menu: with
/// `selected` "pad" of "notepad" selected, else a change made and nothing selected; `clipboard`:
/// the clipboard holds text (brief 95).
fn field_menu_scene(theme: &Theme, fonts: &Fonts, selected: bool, clipboard: bool) -> Framebuffer {
    use bevel_core::edit::{Edit, Op};
    let mut e = Edit::new("notepad");
    if selected {
        for _ in 0..3 {
            e.apply(Op::Left, true);
        }
    } else {
        e.type_char('s');
    }
    let menu = e.menu(clipboard);
    let layout = paint::layout_menu(theme, fonts, &menu, MenuStyle::Popup, 480);
    let mut fb = Framebuffer::new(layout.width, layout.height, 0);
    paint::paint_menu(&mut fb, theme, fonts, &menu, MenuStyle::Popup, &layout);
    fb
}

/// Run with its list of earlier commands open under the field, on the desktop's color, as the
/// window host shows it (the window and the list beside the window's bottom edge).
fn run_list_scene(theme: &Theme, fonts: &Fonts, era: Era) -> Framebuffer {
    use bevel_core::dialog::DialogWindow;
    use bevel_core::kit::{Controller, Form, Key, Mods};
    let history = [
        "notepad", "calc", "winver", "charmap", "c:\\", "c:\\windows", "sol", "freecell", "winmine", "control desk.cpl",
        "sndvol32 /t", "explorer",
    ];
    let mut w = DialogWindow::new(Dialog::new(DialogKind::Run, era), history.iter().map(|c| c.to_string()).collect());
    w.layout(fonts);
    let mut c = Controller::default();
    let button = w.rect(dialog::Part::DropDown).unwrap();
    c.mouse_down(&mut w, button.x + 8, button.y + 8);
    c.mouse_up(&mut w, button.x + 8, button.y + 8);
    for _ in 0..3 {
        c.key(&mut w, Key::Down, Mods::default());
    }
    w.dialog.caret_on = false;
    let (width, height) = w.size();
    let drop = w.drop_rect().unwrap();
    let mut fb = Framebuffer::new(width.max(drop.right()), height.max(drop.bottom()), theme.colors.desktop);
    let mut own = Framebuffer::new(width, height, 0);
    w.paint(&mut own, theme, fonts);
    fb.blit(&own, 0, 0);
    let mut list = Framebuffer::new(drop.w, drop.h, 0);
    w.paint_drop(&mut list, theme, fonts);
    fb.blit(&list, drop.x, drop.y);
    fb
}

/// A known mine layout that fits the Beginner board in all three versions.
const FIELD_MINES: [(usize, usize); 10] = [(3, 0), (0, 2), (7, 6), (6, 7), (5, 3), (1, 6), (2, 6), (7, 1), (4, 5), (7, 3)];

/// About Bevel95 as it opens, the build shown as "1a2b3c4" (the real one is the commit).
fn about_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    use bevel_core::kit::Form;
    let mut w = bevel_core::about::AboutWindow::new(era, "1a2b3c4");
    w.layout(fonts);
    let (width, height) = w.size();
    let mut fb = Framebuffer::new(width, height, 0);
    w.paint(&mut fb, theme, fonts);
    fb
}

/// Bevel95 Setup's pages in `era`'s look, as they open: a first run, its second page (the
/// questions, with this look chosen), and another copy over an installed one.
fn setup_scene(theme: &Theme, era: Era, fonts: &Fonts, which: &str) -> Framebuffer {
    use bevel_core::kit::Form;
    use bevel_core::setup::{Setup, SetupKind, SetupWindow};
    let setup = match which {
        "first-run" => Setup::in_look(SetupKind::FirstRun, era),
        "questions" => {
            let mut s = Setup::in_look(SetupKind::FirstRun, era);
            assert!(!s.ok());
            s
        }
        "update" => Setup::in_look(SetupKind::OtherCopy { newer: true, can_run: true }, era),
        "update-one-file" => Setup::in_look(SetupKind::OtherCopy { newer: true, can_run: false }, era),
        _ => unreachable!("{which}"),
    };
    let mut w = SetupWindow::new(setup, fonts.regular);
    w.layout(fonts);
    let (width, height) = w.size();
    let mut fb = Framebuffer::new(width, height, 0);
    w.paint(&mut fb, theme, fonts);
    fb
}

/// Deterministic play, loss, win and dialog scenes using the same forms as the game window.
fn calculator_scene(theme: &Theme, era: Era, fonts: &Fonts, which: &str) -> Framebuffer {
    use bevel_core::calculator::dialogs::About;
    use bevel_core::calculator::form::{CalculatorWindow, Command, Part};
    use bevel_core::kit::Form;
    fn image<F: Form>(form: &mut F, theme: &Theme, fonts: &Fonts) -> Framebuffer {
        form.layout(fonts);
        let (w, h) = form.size();
        let mut fb = Framebuffer::new(w, h, 0);
        form.paint(&mut fb, theme, fonts);
        fb
    }
    let mut w = CalculatorWindow::new(era);
    match which {
        "play" => {
            w.paste_text("42:m1/3=");
            w.press(Some(Part::Button(7)), 0, 0);
            w.set_pressed(Some(Part::Button(7)));
            image(&mut w, theme, fonts)
        }
        "menu" => {
            w.command(Command::DigitGrouping);
            w.paste_text("1234567.89");
            w.show_menu(Some(1));
            let window = image(&mut w, theme, fonts);
            let (dropped, _) = menu_image(theme, fonts, &w.menu(1), MenuStyle::Popup);
            let title = w.title_rect(1).expect("the View title");
            let mut fb = Framebuffer::new(window.width.max(title.x + dropped.width), window.height, 0);
            fb.blit(&window, 0, 0);
            fb.blit(&dropped, title.x, title.bottom());
            fb
        }
        "scientific" => {
            w.command(Command::Scientific);
            w.paste_text("7:m2*(3+FF");
            w.engine.press(bevel_core::calculator::engine::Key::Radix(bevel_core::calculator::engine::Radix::Hex));
            w.paste_text("ih");
            image(&mut w, theme, fonts)
        }
        "statistics" => {
            w.command(Command::Scientific);
            w.engine.press(bevel_core::calculator::engine::Key::Sta);
            w.paste_text("5\\7\\2.5\\");
            let mut stats = bevel_core::calculator::stats::StatsBox::new(era);
            stats.set_lines(w.engine.stats_lines());
            image(&mut stats, theme, fonts)
        }
        _ => image(&mut About::new(era, Some(261_684)), theme, fonts),
    }
}

fn minefield_scene(theme: &Theme, era: Era, fonts: &Fonts, which: &str) -> Framebuffer {
    use bevel_core::kit::{Controller, Form};
    use bevel_core::minefield::board::SQUARE;
    use bevel_core::minefield::dialogs::Sheet;
    use bevel_core::minefield::form::{MinefieldWindow, Part};
    use bevel_core::minefield::game::{Field, Game, Level};
    use bevel_core::minefield::scores::Settings;
    fn image<F: Form>(form: &mut F, theme: &Theme, fonts: &Fonts) -> Framebuffer {
        form.layout(fonts);
        let (w, h) = form.size();
        let mut fb = Framebuffer::new(w, h, 0);
        form.paint(&mut fb, theme, fonts);
        fb
    }
    let custom = Field { width: 20, height: 12, mines: 40 };
    let mut w = MinefieldWindow::new(era, Level::Beginner, custom, true, 1);
    w.layout(fonts);
    let field = w.field();
    w.game = Game::with_mines(field.width, field.height, &FIELD_MINES);
    match which {
        "play" => {
            for (x, y) in [(0, 0), (6, 2), (5, 4), (6, 5)] {
                w.game.open(x, y);
            }
            w.game.mark(3, 0, true);
            w.game.mark(5, 3, true);
            w.game.mark(7, 7, true);
            w.game.mark(7, 7, true);
            w.game.set_seconds(47);
            let grid = w.rect(Part::Field).expect("the field's hit rectangle");
            let mut held = Controller::default();
            held.mouse_down(&mut w, grid.x + 4 * SQUARE + SQUARE / 2, grid.y + 7 * SQUARE + SQUARE / 2);
            image(&mut w, theme, fonts)
        }
        "lost" => {
            w.game.open(0, 0);
            w.game.open(6, 5);
            w.game.mark(3, 0, true);
            w.game.mark(4, 4, true);
            w.game.open(5, 3);
            w.game.stop_clock(112);
            w.set_active(false);
            image(&mut w, theme, fonts)
        }
        "menu" => {
            for y in 0..field.height {
                for x in 0..field.width {
                    if !FIELD_MINES.contains(&(x, y)) {
                        w.game.open(x, y);
                    }
                }
            }
            w.game.stop_clock(318);
            w.show_menu(Some(0));
            let window = image(&mut w, theme, fonts);
            // The Game menu drops from its title: under the menu bar, at the title's left edge.
            let mut menu = w.menu(0);
            menu.hover = Some(3);
            let title = w.title_rect(0).expect("the Game title");
            let (dropped, _) = menu_image(theme, fonts, &menu, MenuStyle::Popup);
            let mut fb = Framebuffer::new(
                window.width.max(title.x + dropped.width),
                window.height.max(title.bottom() + dropped.height),
                desktop(era),
            );
            fb.blit(&window, 0, 0);
            fb.blit(&dropped, title.x, title.bottom());
            fb
        }
        "custom" => image(&mut Sheet::custom(era, custom, (Field::MAX_WIDTH, Field::MAX_HEIGHT)), theme, fonts),
        "best" => {
            let mut settings = Settings::default();
            settings.record(Level::Beginner, 12, "Ada");
            settings.record(Level::Expert, 301, "Grace Hopper");
            image(&mut Sheet::best_times(era, &settings), theme, fonts)
        }
        other => panic!("no Minesweeper scene called {other}"),
    }
}

/// The bottom right of a 640×480 screen: the bar with three notification icons (two test
/// drawings and the classic speaker), and either a tooltip over the first icon (the pointer
/// resting on it) or the volume control opened from the speaker.
fn tray_scene(theme: &Theme, era: Era, fonts: &Fonts, msans: &Font, volume_open: bool) -> Framebuffer {
    use bevel_core::{tooltip, volume};
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let mut bar = Framebuffer::new(WIDTH, theme.bar.height, 0);
    let state = Bar {
        tray: vec![Some(icon(&MAIL, 1)), Some(icon(&FOLDER, 1)), Some(icons::speaker(false))],
        clock: CLOCK.to_string(),
        clock_widest: WIDEST.to_string(),
        ..Bar::default()
    };
    let layout = paint::paint_bar(&mut bar, theme, fonts, &state);
    let bar_y = SCREEN_H - bar.height;
    screen.blit(&bar, 0, bar_y);
    let screen_rect = Rect::new(0, 0, WIDTH, SCREEN_H);
    if volume_open {
        let speaker = layout.tray[2];
        let at = (speaker.x + 8, bar_y + speaker.y + 8);
        let (x, y) = place_popup(at.0, at.1, volume::SIZE.0, volume::SIZE.1, screen_rect);
        let mut popup = Framebuffer::new(volume::SIZE.0, volume::SIZE.1, 0);
        let v = volume::VolumeControl {
            level: 0.7,
            muted: false,
            focus: volume::Focus::Slider,
            show_keys: era.always_shows_keys(),
            mute_pressed: false,
        };
        volume::paint(&mut popup, theme, msans, &v);
        screen.blit(&popup, x, y);
    } else {
        let first = layout.tray[0];
        let tool = Rect::new(first.x, bar_y + first.y, first.w, first.h);
        let text = "Inbox - 3 new messages";
        let (w, h) = tooltip::size(fonts.regular, text);
        let work = Rect::new(0, 0, WIDTH, bar_y);
        let (x, y) = tooltip::place((first.x + 8, tool.y + 8), w, h, tool, screen_rect, work, 20, 1);
        let mut tip = Framebuffer::new(w, h, 0);
        tooltip::paint(&mut tip, theme, fonts.regular, text);
        screen.blit(&tip, x, y);
    }
    // Only the bottom right corner matters.
    let mut out = Framebuffer::new(320, 200, 0);
    out.blit(&screen, -(WIDTH - 320), -(SCREEN_H - 200));
    out
}

/// The right end of the bar with Bevel95's own option "Show the battery's percentage beside
/// its icon" on: the connection and the battery (Bevel95's own icons, first in the area), the
/// classic speaker, and "87%" in the battery's own part, in the clock's font, color and baseline.
fn battery_percent_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    use bevel_core::status::{self, Network, Power};
    let power = Power { battery: true, on_ac: Some(false), percent: Some(87), ..Power::default() };
    let net = Network { connected: true, ..Network::default() };
    let mut bar = Framebuffer::new(WIDTH, theme.bar.height, 0);
    let state = Bar {
        tray: vec![Some(status::network_icon(era, &net)), Some(status::power_icon(era, &power)), Some(icons::speaker(false))],
        tray_number: status::percent_text(&power).map(|text| (1, text)),
        clock: CLOCK.to_string(),
        clock_widest: WIDEST.to_string(),
        ..Bar::default()
    };
    paint::paint_bar(&mut bar, theme, fonts, &state);
    let mut out = Framebuffer::new(240, bar.height, 0);
    out.blit(&bar, -(WIDTH - 240), 0);
    out
}

/// Quick Launch switched on (the 95 look draws it by 98's rule): the look's own icons and Windows
/// Explorer (Bevel95's own icons only), then the task buttons; the icon `hot` under the pointer
/// (raised), the icon `pressed` held down. `width`: the whole bar, or only its left end.
fn quick_launch_scene(theme: &Theme, era: Era, fonts: &Fonts, hot: Option<usize>, pressed: Option<usize>, width: i32) -> Framebuffer {
    use bevel_core::quick_launch;
    let icons = quick_launch::builtins(era).iter().map(|&b| quick_launch::picture(era, b)).collect();
    let mut bar = Framebuffer::new(WIDTH, theme.bar.height, 0);
    let state = Bar {
        tasks: three_groups(),
        clock: CLOCK.to_string(),
        clock_widest: WIDEST.to_string(),
        quick_launch: Some(quick_launch::Band { icons, hot, pressed, ..Default::default() }),
        ..Bar::default()
    };
    paint::paint_bar(&mut bar, theme, fonts, &state);
    if width >= WIDTH {
        return bar;
    }
    let mut out = Framebuffer::new(width, bar.height, 0);
    out.blit(&bar, 0, 0);
    out
}

/// The band made narrower and wider (brief 70): what `quick_launch_sized` draws.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BandSize {
    /// The look's own icons, the band dragged 40 wider: empty face after them.
    Widened,
    /// Nine icons (the look's own, then Bevel95's own window pictures as test shortcuts), the band
    /// dragged to its narrowest: 98's right arrow (the pointer on it) and the fifth icon cut; 2000's
    /// "»" (the pointer on it) with four icons.
    Narrowed,
    /// 98 (and the 95 look): the same, scrolled by two presses of the right arrow, the pointer on
    /// the left arrow.
    Scrolled,
    /// 98's right arrow held down at the end: grayed until the release.
    HeldAtEnd,
    /// 2000: the "»" clicked, its list of the icons left out open above it, the pointer on its
    /// second row.
    List,
}

/// Bevel95's own test icons for the sized band: the look's built-in icons, then window pictures.
fn sized_icons(era: Era) -> (Vec<Option<bevel_core::fb::Icon>>, Vec<String>) {
    use bevel_core::quick_launch;
    use bevel_core::window_icons::{self, ClassicWindow::*};
    let mut icons: Vec<Option<bevel_core::fb::Icon>> = quick_launch::builtins(era).iter().map(|&b| quick_launch::picture(era, b)).collect();
    let mut names: Vec<String> = quick_launch::builtins(era).iter().map(|&b| quick_launch::item_name(era, quick_launch::Item::Builtin(b), &[])).collect();
    for (kind, name) in [(Notepad, "Notepad"), (MyComputer, "My Computer"), (RecycleBin, "Recycle Bin"), (ControlPanel, "Control Panel"), (Folder, "Temp"), (MyDocuments, "My Documents")] {
        icons.push(Some(window_icons::icon(era, kind)));
        names.push(name.to_string());
    }
    icons.truncate(9);
    names.truncate(9);
    (icons, names)
}

/// The left end of the bar (240 wide; the list scene: the desktop's lower left with the bar) with
/// the band made narrower or wider (`BandSize`).
fn quick_launch_sized(theme: &Theme, era: Era, fonts: &Fonts, which: BandSize) -> Framebuffer {
    use bevel_core::quick_launch::{self, Control};
    let (icons, names) = match which {
        BandSize::Widened => {
            let (mut i, mut n) = sized_icons(era);
            let k = quick_launch::builtins(era).len();
            i.truncate(k);
            n.truncate(k);
            (i, n)
        }
        _ => sized_icons(era),
    };
    let count = icons.len();
    let mut band = quick_launch::Band { icons, ..Default::default() };
    match which {
        BandSize::Widened => band.width = Some(quick_launch::natural_width(count) + 40),
        BandSize::Narrowed => {
            band.width = Some(quick_launch::min_width(era, count));
            band.hot_control = Some(if era == Era::Win2000 { Control::Chevron } else { Control::Right });
        }
        BandSize::Scrolled => {
            band.width = Some(quick_launch::min_width(era, count));
            band.scroll = quick_launch::scroll_right(quick_launch::scroll_right(0, i32::MAX), i32::MAX);
            band.hot_control = Some(Control::Left);
        }
        BandSize::HeldAtEnd => {
            band.width = Some(quick_launch::min_width(era, count));
            band.scroll = i32::MAX;
            band.pressed_control = Some(Control::Right);
        }
        BandSize::List => {
            band.width = Some(quick_launch::min_width(era, count));
            band.list_open = true;
        }
    }
    let state = Bar { tasks: three_groups(), clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(), quick_launch: Some(band.clone()), ..Bar::default() };
    let mut bar = Framebuffer::new(WIDTH, theme.bar.height, 0);
    let layout = paint::paint_bar(&mut bar, theme, fonts, &state);
    if which != BandSize::List {
        let mut out = Framebuffer::new(240, bar.height, 0);
        out.blit(&bar, 0, 0);
        return out;
    }
    // The list over the desktop's lower left, above the "»".
    const H: i32 = 140;
    let mut screen = Framebuffer::new(320, H, desktop(era));
    let bar_top = H - bar.height;
    screen.blit(&bar, 0, bar_top);
    let q = layout.quick.expect("the band");
    let chevron = q.chevron.expect("the chevron");
    let rows = q.hidden.iter().map(|&i| (names[i].clone(), band.icons[i].clone())).collect();
    let menu = Menu { hover: Some(1), ..quick_launch::list_menu(rows, &names) };
    let (fb, _) = menu_image(theme, fonts, &menu, MenuStyle::Cascade);
    let (x, y) = quick_launch::place_list(Rect::new(chevron.x, bar_top + chevron.y, chevron.w, chevron.h), fb.width, fb.height, Rect::new(0, 0, 320, H));
    screen.blit(&fb, x, y);
    screen
}

thread_local! {
    /// The colors of the scenes being drawn (the 16-color scenes set it while they are drawn).
    static DEPTH: std::cell::Cell<Depth> = const { std::cell::Cell::new(Depth::TrueColor) };
}

/// The desktop behind the scenes: each look's own desktop color, which Bevel95's desktop shows
/// (in the colors of the scenes being drawn).
fn desktop(era: Era) -> u32 {
    bevel_core::theme::theme_in(era, DEPTH.get()).colors.desktop
}

/// One golden image, and where and how it appears in `docs/preview.html`.
struct Scene {
    section: &'static str,
    name: String,
    caption: &'static str,
    image: Framebuffer,
}

fn scenes() -> Vec<Scene> {
    let mut out = Vec::new();
    let looks = [
        (&WIN95, Era::Win95, "Windows 95"),
        (&WIN98, Era::Win98, "Windows 98"),
        (&WIN2000, Era::Win2000, "Windows 2000"),
    ];
    for (theme, era, section) in looks {
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        let mut add = |state: &str, caption: &'static str, image: Framebuffer| {
            out.push(Scene { section, name: format!("{}-{state}", theme.slug), caption, image });
        };
        add("empty", "Empty bar", bar_image(theme, &fonts, false, vec![]));
        add("three-groups", "Three groups: normal, active, flashing", bar_image(theme, &fonts, false, three_groups()));
        add("start-pressed", "Start pressed", bar_image(theme, &fonts, true, vec![]));
        add(
            "crowded",
            "Crowded: nine buttons shrink, all to the same width, and cut their titles",
            bar_image(theme, &fonts, false, crowded()),
        );
        add(
            "overfull",
            "Too many buttons: they shrink to their icons, then scroll arrows page through rows (first row)",
            bar_image(theme, &fonts, false, overfull()),
        );
        add("overfull-row-2", "The second row, down arrow held down", second_row_image(theme, &fonts));
        add(
            "drag",
            "Dragging a task button (Bevel95's own, as from Windows 7): My Computer follows the pointer over the others, Word halfway through its slide to the free place",
            drag_image(theme, &fonts),
        );
        add(
            "window-menu",
            "Right-click on a task button: its window's menu, pointer on Maximize (pictures in 98 and 2000)",
            task_menu_scene(theme, era, &fonts, false),
        );
        add(
            "group-menu",
            "Right-click on a button with several windows: the group menu (Windows XP's)",
            task_menu_scene(theme, era, &fonts, true),
        );
        add(
            "start-menu",
            "Start menu open, pointer on Programs, its menu open beside it (classic icons from the real Start menu; the app icons are test drawings; the desktop color is only scenery)",
            start_menu_scene(theme, era, &fonts),
        );
        add(
            "start-menu-small",
            "Start menu with \"Show small icons in Start menu\" on, pointer on Settings (Windows 98's exact; 95's and 2000's provisional)",
            start_menu_small_scene(theme, era, &fonts),
        );
        add(
            "context-menu",
            "Right-click menu on the bar, pointer on the first entry",
            context_menu_scene(theme, era, &fonts),
        );
        add(
            "properties",
            "Properties (right-click the bar): Windows' own first page, its preview showing the options",
            properties_scene(theme, era, &fonts, properties::Sheet::Taskbar, 0),
        );
        add(
            "properties-start-menu",
            "Properties, second page (Windows 2000: Advanced; entries Bevel95 does not do are grayed out)",
            properties_scene(theme, era, &fonts, properties::Sheet::Taskbar, 1),
        );
        add(
            "remove-shortcuts",
            "Remove Shortcuts/Folders (Properties, second page, Remove...): Windows' own tree of the Programs menu",
            remove_scene(theme, era, &fonts),
        );
        add(
            "bevel95-settings",
            "Bevel95 Settings (right-click the bar): look, size, starting with Windows, and the clock",
            properties_scene(theme, era, &fonts, properties::Sheet::Bevel95, 0),
        );
        add(
            "bevel95-settings-quarter",
            "Bevel95 Settings at a size between whole numbers (1.75): \"Allow quarter sizes\" ticked by itself, how it is drawn can be chosen; the keyboard on that list",
            quarter_size_scene(theme, era, &fonts, None),
        );
        add(
            "bevel95-settings-size-list",
            "Bevel95 Settings, the list of sizes open: 1 to 4 in quarter steps, 1.75 chosen, the pointer one below",
            quarter_size_scene(theme, era, &fonts, Some(properties::Part::SizeList)),
        );
        add(
            "bevel95-settings-way-list",
            "Bevel95 Settings, the list of the ways a size between whole numbers is drawn, open",
            quarter_size_scene(theme, era, &fonts, Some(properties::Part::WayList)),
        );
        add(
            "bevel95-settings-taskbar",
            "Bevel95 Settings, Taskbar page: what the Windows key opens, and grouping a program's buttons (Windows XP's wording)",
            properties_scene(theme, era, &fonts, properties::Sheet::Bevel95, 1),
        );
        add(
            "bevel95-settings-desktop",
            "Bevel95 Settings, Desktop page: the switch for Bevel95's own desktop",
            properties_scene(theme, era, &fonts, properties::Sheet::Bevel95, 2),
        );
        add(
            "bevel95-settings-programs",
            "Bevel95 Settings, Programs page as it opens: the menu shows everything; the tree's folders are closed",
            programs_scene(theme, era, &fonts, Shown::Everything),
        );
        add(
            "bevel95-settings-programs-classic",
            "Programs page, Bevel95's own classic programs chosen: their folders open to show them",
            programs_scene(theme, era, &fonts, Shown::Classic),
        );
        add(
            "bevel95-settings-programs-ticked",
            "Programs page, only the ticked programs, every folder open: a folder with some unticked is half ticked",
            programs_scene(theme, era, &fonts, Shown::Ticked),
        );
        add(
            "desktop-icons",
            "Bevel95's desktop icons: the look's own (Recycle Bin full), then the owner's items in columns (test pictures); the long name is selected and has the keyboard focus, so it shows in full",
            desktop_icons_scene(theme, era, &fonts),
        );
        for (state, caption) in [
            ("desktop-wallpaper-tile", "The classic desktop with a wallpaper tiled from the top-left corner (Display Properties, Background); each name in a box of the desktop color, My Computer selected"),
            ("desktop-pattern", "The classic desktop with a pattern: its 8x8 bits from the screen's corner, black on the desktop color; My Computer selected"),
            ("desktop-wallpaper-center", "A small wallpaper centered on the whole screen, the pattern around it; My Computer selected"),
            ("desktop-wallpaper-stretch", "A wallpaper stretched over the whole screen by Windows' own stepping (98 and 2000: 95 had no Stretch); My Computer selected"),
        ] {
            if era != Era::Win95 || state != "desktop-wallpaper-stretch" {
                add(state, caption, wallpaper_desktop_scene(theme, era, &fonts, state));
            }
        }
        for (state, caption) in [
            ("display-background", "Display Properties (the desktop's Properties), its Background page as it opens: nothing chosen, Apply grayed (only this tab for now)"),
            ("display-wallpaper", "Display Properties with a wallpaper chosen: the little monitor shows it as each Windows scaled it, Apply usable"),
            ("display-pattern", "Display Properties with a pattern chosen: the monitor shows it 1:1 on the desktop color"),
            ("display-pattern-window", "The Pattern window (98, 2000: Pattern...) with a pattern chosen, its Preview, Edit Pattern... grayed for now"),
        ] {
            if era != Era::Win95 || state != "display-pattern-window" {
                add(state, caption, display_scene(theme, era, &fonts, state));
            }
        }
        add(
            "display-own-picture",
            "Display Properties with a picture of the person's own chosen with Browse... (a test picture): in the list by its name where Windows' order puts it, on the monitor scaled as each Windows scaled its pictures",
            display_scene(theme, era, &fonts, "display-own-picture"),
        );
        if era == Era::Win2000 {
            add(
                "display-jpeg-list",
                "Windows 2000's JPEG wallpapers in its list (the picture files' small picture), Windows 2000 chosen; the monitor shows the desktop color here because a golden picture is drawn without the bar's JPEG reader",
                display_scene(theme, era, &fonts, "display-jpeg-list"),
            );
        }
        add(
            "desktop-menu",
            "Bevel95's desktop (each look's own color) with its right-click menu, pointer on the first entry",
            desktop_menu_scene(theme, era, &fonts),
        );
        add(
            "desktop-bevel95-menu",
            "The desktop's right-click menu with its Bevel95 submenu open",
            desktop_own_menu_scene(theme, era, &fonts),
        );
        add(
            "desktop-select",
            "Bevel95's desktop with icons moved to free places: Ctrl+click chose one, the dotted selection rectangle (being drawn) two more",
            desktop_moving_scene(theme, era, &fonts, Moving::Select),
        );
        add(
            "desktop-drag",
            "One icon being dragged over another: Windows 95's checkerboard, 98's half-and-half, 2000's see-through picture; the icon stays behind, selected",
            desktop_moving_scene(theme, era, &fonts, Moving::DragOne),
        );
        add(
            "desktop-drag-several",
            "Three selected icons being dragged: dotted outlines in Windows 95 and 98, see-through pictures in 2000",
            desktop_moving_scene(theme, era, &fonts, Moving::DragSeveral),
        );
        add(
            "drag-files",
            "A file dragged with Ctrl onto a folder: the folder drawn selected as the drop place, the look's dragged picture beside the pointer, the copy pointer",
            drag_files_scene(theme, era, &fonts, false),
        );
        add(
            "drag-files-menu",
            "The right-button drag's menu at the drop: Move Here (bold), Copy Here, Create Shortcut(s) Here, Cancel (2000: Copy Here first)",
            drag_files_scene(theme, era, &fonts, true),
        );
        add(
            "desktop-item-menu",
            "The menu on a file's icon (each look's own entries that work here; Open in bold), pointer on Open",
            desktop_item_scene(theme, era, &fonts, ItemScene::Menu),
        );
        add(
            "desktop-rename",
            "A label being renamed: the edit box over it, the end of the typed name selected, the caret after it",
            desktop_item_scene(theme, era, &fonts, ItemScene::Rename),
        );
        add(
            "desktop-cut",
            "Two icons after Cut: their pictures faded as each look drew them (one of them selected)",
            desktop_item_scene(theme, era, &fonts, ItemScene::Cut),
        );
        add(
            "desktop-confirm-delete",
            "The look's own Confirm File Delete window before a file goes to the Recycle Bin, Yes focused",
            desktop_item_scene(theme, era, &fonts, ItemScene::Confirm),
        );
        add(
            "keep-size-question",
            "After a size between whole numbers is chosen in the bar's Size menu: the look's question whether to keep it, Yes focused (words provisional)",
            desktop_item_scene(theme, era, &fonts, ItemScene::KeepSize),
        );
        add(
            "desktop-rename-extension",
            "Renaming letter.txt to letter.dat: the look's own Rename warning over the rename box, Yes focused",
            desktop_item_scene(theme, era, &fonts, ItemScene::ExtensionWarning),
        );
        add(
            "bevel95-menu",
            "Right-click menu with its Bevel95 submenu open: everything of Bevel95's own in one entry",
            bar_submenu_scene(theme, era, &fonts, None),
        );
        add("about", "About Bevel95 (Bevel95 > About Bevel95): name, version and build, what it is, the notice (wording provisional)", about_scene(theme, era, &fonts));
        add(
            "own-icon-menu",
            "The menu of Bevel95's own icon in Windows' notification area (there only while switched to Windows' taskbar), alone; wording provisional",
            own_icon_menu_image(theme, era, &fonts),
        );
        add("setup-first-run", "Bevel95 Setup on a PC without Bevel95 (the one file started): install or keep it portable", setup_scene(theme, era, &fonts, "first-run"));
        add("setup-questions", "Bevel95 Setup's second page when installing: the look (the window is drawn in the one chosen), the classic desktop, the Windows key, Start with Windows; the owner's defaults ticked", setup_scene(theme, era, &fonts, "questions"));
        add("setup-update", "Bevel95 Setup over an installed Bevel95 (drawn in its look): update, just run this copy, or remove", setup_scene(theme, era, &fonts, "update"));
        add("setup-update-one-file", "Bevel95 Setup over an installed Bevel95, started from the one file: update or remove (its programs are only inside the file, so it cannot just run)", setup_scene(theme, era, &fonts, "update-one-file"));
        add(
            "theme-menu",
            "Right-click menu, Bevel95, with the Theme submenu open; the look in use has a bullet (bullet shape provisional)",
            bar_submenu_scene(theme, era, &fonts, Some(BarEntry::Theme)),
        );
        add(
            "size-menu",
            "Right-click menu, Bevel95, with the Size submenu open; the size in use (2×) has a bullet (bullet shape provisional), the whole sizes in bold",
            bar_submenu_scene(theme, era, &fonts, Some(BarEntry::Size)),
        );
        add("run", "Run, with \"notepad\" typed", dialog_scene(theme, &fonts, DialogKind::Run, era));
        add("field-selected", "Run's field with part of its text selected: \"pad\" of \"notepad readme.txt\" (brief 95)", field_selected_scene(theme, &fonts, era));
        add("field-menu", "A text field's right-click menu with \"pad\" selected and text on the clipboard: Undo grey (nothing to undo) (brief 95)", field_menu_scene(theme, &fonts, true, true));
        add("field-menu-grey", "The same with nothing selected, nothing to paste, after a change: Cut, Copy, Paste and Delete grey, Undo there", field_menu_scene(theme, &fonts, false, false));
        add(
            "run-list",
            "Run's list of earlier commands, dropped down by its button and moved down three with the arrow key (12 commands: a scroll bar; 2000 shows them all with the bar grayed)",
            run_list_scene(theme, &fonts, era),
        );
        add(
            "find-programs",
            "Quick Search (Bevel95's own, opened by the Windows key) with \"p\" typed",
            find_scene(theme, era, &fonts),
        );
        add("shut-down", "Shut Down Windows", dialog_scene(theme, &fonts, DialogKind::ShutDown, era));
        if era != Era::Win95 {
            add(
                "log-off",
                "Log Off Windows (Windows 98 and 2000)",
                dialog_scene(theme, &fonts, DialogKind::LogOff, era),
            );
        }
        // A Programs menu longer than the screen, as each look's Windows showed it (brief 91; the
        // survey's test entries; the screen at 640x480 and at 800x600).
        let (n640, n800) = match era {
            Era::Win95 => (80, 50),
            Era::Win98 => (117, 57),
            Era::Win2000 => (46, 59),
        };
        let long_caption: &'static str = match era {
            Era::Win95 => "A Programs menu longer than the screen (640x480, 80 entries): columns, moved to the screen's left edge as they pass the right one",
            Era::Win98 => "A Programs menu longer than the screen (640x480, 117 entries): one column the screen's height, scrolling between its arrow strips",
            Era::Win2000 => "A Programs menu longer than the screen (640x480, 46 entries): columns beside the Start menu",
        };
        add("long-programs-640", long_caption, long_menu_scene(theme, era, &fonts, 640, 480, n640, LongScene::Open));
        let long_caption: &'static str = match era {
            Era::Win95 => "800x600, 50 entries: two columns beside the Start menu, each as wide as its widest entry, an etched line between",
            Era::Win98 => "800x600, 57 entries: one scrolling column",
            Era::Win2000 => "800x600, 59 entries: three columns beside the Start menu (their widths provisional)",
        };
        add("long-programs-800", long_caption, long_menu_scene(theme, era, &fonts, 800, 600, n800, LongScene::Open));
        add(
            "long-submenu-800",
            "800x600: a long folder (60 items) open beside its entry, as its look opened a long submenu",
            long_menu_scene(theme, era, &fonts, 800, 600, if era == Era::Win98 { 57 } else { 40 }, LongScene::Submenu),
        );
        match era {
            Era::Win95 => add(
                "long-programs-640-scrolls",
                "640x480, 110 entries: the columns would not fit even at the left edge (the real Windows 95 ran off the screen), so it scrolls, with Windows 98's strips (provisional)",
                long_menu_scene(theme, era, &fonts, 640, 480, 110, LongScene::Open),
            ),
            Era::Win98 => {
                add("long-programs-800-middle", "800x600, 117 entries, scrolled to the middle: both strips", long_menu_scene(theme, era, &fonts, 800, 600, 117, LongScene::Middle));
                add("start-scrolls", "The Start menu on a screen too short for it (640x360): it scrolls, the banner standing still, the bottom strip over the entries only", start_scrolls_scene(theme, &fonts, false));
                add("start-scrolls-end", "The same scrolled to the end, the pointer on the top strip (sunken, no arrow), the last entry chosen", start_scrolls_scene(theme, &fonts, true));
                add("start-scrolls-384", "The case that started it: a 768-row screen at Size 2 (683x384). The Start menu does not fit above the Start button, so it scrolls and ends on top of the bar, the button showing", start_scrolls_scene_sized(theme, &fonts, 683, 384, false));
                add(
                    "long-programs-800-end",
                    "800x600, 117 entries, scrolled to the end with the pointer on the top strip: it is drawn sunken, without its arrow; the bottom strip is gone",
                    long_menu_scene(theme, era, &fonts, 800, 600, 117, LongScene::EndStripHot),
                );
            }
            Era::Win2000 => {
                add(
                    "long-programs-640-scrolls",
                    "640x480, 92 entries: the columns would run off the right edge (as the real Windows 2000 did), so it scrolls (provisional)",
                    long_menu_scene(theme, era, &fonts, 640, 480, 92, LongScene::Open),
                );
                add(
                    "long-programs-800-scroll-setting",
                    "800x600, 59 entries, \"Scroll the Programs menu\" on: one column with Windows 2000's flat strips",
                    long_menu_scene(theme, era, &fonts, 800, 600, 59, LongScene::ScrollSetting),
                );
            }
        }
        if era == Era::Win98 {
            add(
                "favorites",
                "Start menu with Favorites open: folders get Windows 98's folder icon, and a menu without submenus is narrower",
                favorites_scene(theme, &fonts),
            );
        }
        add(
            "window-list",
            "One button per app: Notepad has 3 windows (active), Outlook Express wants attention; Notepad's list of windows is open",
            window_list_scene(theme, era, &fonts),
        );
        // The volume control's text is MS Sans Serif in every look.
        let msans = load_font(&WIN95, false);
        add(
            "tray-tooltip",
            "Notification area: two programs' icons (test drawings), the classic speaker, the clock; the pointer rests on the first icon, so its tooltip shows",
            tray_scene(theme, era, &fonts, &msans, false),
        );
        add(
            "volume",
            "The speaker clicked: the classic volume control at 70% (Windows 2000's exact; 95's and 98's provisional)",
            tray_scene(theme, era, &fonts, &msans, true),
        );
        add(
            "battery-percent",
            "Bevel95's own option (off unless switched on): the battery's percentage beside its icon, in the clock's font",
            battery_percent_scene(theme, era, &fonts),
        );
        add(
            "quick-launch",
            "Quick Launch on (98 and 2000 by default; 95 when switched on, by 98's rule): the look's own icons and Windows Explorer, the pointer on the first",
            quick_launch_scene(theme, era, &fonts, Some(0), None, WIDTH),
        );
        add(
            "quick-launch-pressed",
            "Quick Launch: the second icon held down",
            quick_launch_scene(theme, era, &fonts, None, Some(1), 240),
        );
        add(
            "quick-launch-band-menu",
            "Quick Launch: the band's own menu (right-click on its grip), pointer on Open; what Bevel95 does not do is grayed",
            quick_launch_menu_scene(theme, era, &fonts, "band"),
        );
        add(
            "quick-launch-icon-menu",
            "Quick Launch: the menu on an icon (first pass: Open and Delete)",
            quick_launch_menu_scene(theme, era, &fonts, "icon"),
        );
        add(
            "quick-launch-widened",
            "Quick Launch dragged 40 wider by the grip before the task buttons: empty face after the icons",
            quick_launch_sized(theme, era, &fonts, BandSize::Widened),
        );
        if era == Era::Win2000 {
            add(
                "quick-launch-narrowed",
                "Quick Launch with nine icons (test pictures) dragged to its narrowest: four icons and the \"»\", the pointer on it",
                quick_launch_sized(theme, era, &fonts, BandSize::Narrowed),
            );
            add(
                "quick-launch-list",
                "The \"»\" clicked: the list of the icons left out, above it, as wide as the longest name on the band; the pointer on the second row",
                quick_launch_sized(theme, era, &fonts, BandSize::List),
            );
        } else {
            add(
                "quick-launch-narrowed",
                "Quick Launch with nine icons (test pictures) dragged to its narrowest: four icons, the fifth cut, the right arrow under the pointer",
                quick_launch_sized(theme, era, &fonts, BandSize::Narrowed),
            );
            add(
                "quick-launch-scrolled",
                "The same after two presses of the right arrow: the left arrow (under the pointer) and the icons moved on",
                quick_launch_sized(theme, era, &fonts, BandSize::Scrolled),
            );
            add(
                "quick-launch-held-at-end",
                "The right arrow held down at the end: grayed until the release, then gone",
                quick_launch_sized(theme, era, &fonts, BandSize::HeldAtEnd),
            );
        }
        if era != Era::Win95 {
            add(
                "toolbars-menu",
                "The bar's menu with Toolbars open: Quick Launch ticked, the other toolbars grayed",
                quick_launch_menu_scene(theme, era, &fonts, "toolbars"),
            );
        }
        add(
            "minefield",
            "Minesweeper in play, with flags, a question mark and a square held down",
            minefield_scene(theme, era, &fonts, "play"),
        );
        add(
            "minefield-lost",
            "Minesweeper after a loss, inactive, with the detonated mine, remaining mines and a wrong flag",
            minefield_scene(theme, era, &fonts, "lost"),
        );
        add(
            "minefield-menu",
            "Minesweeper after a win, with Intermediate highlighted in the Game menu",
            minefield_scene(theme, era, &fonts, "menu"),
        );
        add("minefield-custom", "Minesweeper's Custom Field dialog", minefield_scene(theme, era, &fonts, "custom"));
        add("minefield-best-times", "Minesweeper's best times", minefield_scene(theme, era, &fonts, "best"));
        add(
            "calculator",
            "Calculator: 1 ÷ 3 = with a number in memory, the 7 held down with the mouse",
            calculator_scene(theme, era, &fonts, "play"),
        );
        add(
            "calculator-menu",
            "Calculator's View menu (Windows 2000: Digit grouping on), a grouped number behind it",
            calculator_scene(theme, era, &fonts, "menu"),
        );
        add("calculator-about", "About Calculator", calculator_scene(theme, era, &fonts, "about"));
        add(
            "calculator-scientific",
            "Calculator's Scientific view: Hex, a parenthesis open, memory, Inv and Hyp ticked",
            calculator_scene(theme, era, &fonts, "scientific"),
        );
        add(
            "calculator-statistics",
            "Calculator's Statistics Box with three numbers, the last one chosen",
            calculator_scene(theme, era, &fonts, "statistics"),
        );
    }
    // Windows 98's and 2000's folder windows: Explorer's toolbar and address bar.
    for (era, theme, section) in [(Era::Win98, &WIN98, "Windows 98"), (Era::Win2000, &WIN2000, "Windows 2000")] {
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        let prefix = if era == Era::Win98 { "win98" } else { "win2000" };
        for (state, caption) in [
            ("folder-explorer", "A folder window gone from My Computer to a folder (Back and Up lit), a file selected, the pointer on a toolbar button"),
            ("folder-explorer-list", "The address bar's list open: the Desktop, My Computer and its drives, the way down to the folder shown"),
        ] {
            out.push(Scene { section, name: format!("{prefix}-{state}"), caption, image: explorer_scene(theme, era, &fonts, state) });
        }
    }
    // Explorer: the tree of all folders beside the icons, in each look.
    for (era, theme, section) in [(Era::Win95, &WIN95, "Windows 95"), (Era::Win98, &WIN98, "Windows 98"), (Era::Win2000, &WIN2000, "Windows 2000")] {
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        let caption = "Explorer: the tree of all folders opened down to a folder (selected, the tree has the keyboard), its icons beside it";
        out.push(Scene { section, name: format!("{}-explorer-tree", era_prefix(era)), caption, image: tree_scene(theme, era, &fonts) });
    }
    // Windows 95's folder windows.
    {
        let (regular, bold) = (load_font(&WIN95, false), load_font(&WIN95, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        for (state, caption) in [
            ("folder-mycomputer", "A folder window: My Computer with its drives, Control Panel and Printers, the hard disk selected (its free space in the status bar)"),
            ("folder-files", "A folder of files made narrower than its icons: they stay where they were and scroll bars come; the long name selected, in full"),
            ("folder-menu", "A folder window's File menu with a file selected: the status bar shows the highlighted entry's help text"),
            ("folder-hidden", "A folder with a hidden folder and a hidden file among plain ones, Windows set to show hidden items: their pictures faded as a cut item's (provisional); one protected system file still left out (plus 1 hidden)"),
        ] {
            out.push(Scene { section: "Windows 95", name: format!("win95-{state}"), caption, image: folder_scene(&fonts, state) });
        }
    }
    // Folder views in each look: Details sorted by size, reversed, a file selected; List and Small
    // Icons of the same folder.
    for (era, theme, section) in [(Era::Win95, &WIN95, "Windows 95"), (Era::Win98, &WIN98, "Windows 98"), (Era::Win2000, &WIN2000, "Windows 2000")] {
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        for (mode, state, caption) in [
            (ViewMode::Details, "folder-details", "Details: sorted by Size twice (biggest first, folders last), a file selected; 2000 shows the sort arrow"),
            (ViewMode::List, "folder-list", "List: the items column after column, one selected"),
            (ViewMode::Small, "folder-small", "Small Icons: rows of small pictures, the long label in full"),
        ] {
            out.push(Scene { section, name: format!("{}-{state}", era_prefix(era)), caption, image: views_scene(theme, era, &fonts, mode) });
        }
        // The look's own pictures and Type names for the kinds of file its Windows knew.
        for (mode, state, caption) in [
            (ViewMode::Large, "folder-kinds-large", "Kinds of file the look knew, Large Icons: each with its Windows' own picture (a program without an icon of its own: the program picture; the Internet shortcut with its arrow), the hidden one faded, Notes.txt selected"),
            (ViewMode::Small, "folder-kinds-small", "The same folder in Small Icons: the look's 16x16 pictures"),
            (ViewMode::Details, "folder-kinds-details", "The same folder in Details: the look's own Type names"),
        ] {
            out.push(Scene { section, name: format!("{}-{state}", era_prefix(era)), caption, image: kinds_scene(theme, era, &fonts, mode) });
        }
    }
    // The web view panel of 98's and 2000's folder windows.
    for (era, theme, section) in [(Era::Win98, &WIN98, "Windows 98"), (Era::Win2000, &WIN2000, "Windows 2000")] {
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        for (state, caption) in [
            ("folder-web", "The web view: a file selected, its name, type, date and size in the panel beside the icons"),
            ("folder-web-drive", "The web view of My Computer with a drive selected: its capacity, used and free space, the pie chart"),
            ("folder-web-warning", "The web view of the Windows folder: its files hidden behind the warning and Show Files"),
        ] {
            out.push(Scene { section, name: format!("{}-{state}", era_prefix(era)), caption, image: web_scene(theme, era, &fonts, state) });
        }
    }
    // Windows 2000's Folder Options, classic folders just chosen (Apply alive).
    {
        use bevel_core::folder::options::{Choice, FolderOptions, Part as P};
        use bevel_core::kit::Form;
        let (regular, bold) = (load_font(&WIN2000, false), load_font(&WIN2000, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        let mut w = FolderOptions::new(Choice::default(), true);
        w.press(Some(P::Classic), 0, 0);
        let mut fb = Framebuffer::new(w.size().0, w.size().1, 0);
        w.paint(&mut fb, &WIN2000, &fonts);
        out.push(Scene { section: "Windows 2000", name: "win2000-folder-options".into(), caption: "Folder Options (Tools menu): Web View and Browse Folders, classic folders just chosen", image: fb });
    }
    out.extend(sixteen_color_scenes());
    out.extend(reduced_color_scenes(Depth::TwoFiftySix));
    // The integer upscaler, checked on a real image.
    let three = out.iter().find(|s| s.name == "win95-three-groups").expect("scene exists").image.clone();
    out.push(Scene {
        section: "Scaling check",
        name: "win95-three-groups-2x".to_string(),
        caption: "The 95 three-groups bar, upscaled 2× by the taskbar's own code (what a 150–175% screen gets), \
                  shown at half the zoom so it lines up with the pictures above",
        image: upscale(&three, 2),
    });
    out
}

/// The 16 Colors option: a chosen set of scenes per look drawn with its 16-color theme
/// (`theme_in(era, Depth::Sixteen)`), named `<look>-16-<state>`.
fn sixteen_color_scenes() -> Vec<Scene> {
    reduced_color_scenes(Depth::Sixteen)
}

/// The same scenes for 16 Colors (`<look>-16-<state>`) or 256 Colors (`<look>-256-<state>`).
fn reduced_color_scenes(depth: Depth) -> Vec<Scene> {
    let mut out = Vec::new();
    DEPTH.set(depth);
    let sections: [(Era, &'static str); 3] = match depth {
        Depth::TwoFiftySix => [(Era::Win95, "Windows 95, 256 Colors"), (Era::Win98, "Windows 98, 256 Colors"), (Era::Win2000, "Windows 2000, 256 Colors")],
        _ => [(Era::Win95, "Windows 95, 16 Colors"), (Era::Win98, "Windows 98, 16 Colors"), (Era::Win2000, "Windows 2000, 16 Colors")],
    };
    let (tag, words) = match depth {
        Depth::TwoFiftySix => ("256", "256 Colors"),
        _ => ("16", "16 Colors"),
    };
    for (era, section) in sections {
        let theme = bevel_core::theme::theme_in(era, depth);
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        let mut add = |state: &str, caption: &'static str, image: Framebuffer| {
            // The 16-color captions as they were; the 256-color ones name their own colors.
            let caption: &'static str =
                if depth == Depth::Sixteen { caption } else { Box::leak(caption.replace("16 Colors", words).into_boxed_str()) };
            out.push(Scene { section, name: format!("{}-{tag}-{state}", theme.slug), caption, image });
        };
        add("three-groups", "16 Colors: the bar with three groups: normal, active, flashing", bar_image(theme, &fonts, false, three_groups()));
        add(
            "quick-launch",
            "16 Colors: Quick Launch with the look's own 16-color drawings, the pointer on the first icon",
            quick_launch_scene(theme, era, &fonts, Some(0), None, WIDTH),
        );
        add(
            "start-menu",
            "16 Colors: the Start menu with Programs open (pictures without a 16-color drawing converted by the look's rule)",
            start_menu_scene(theme, era, &fonts),
        );
        add("colors-menu", "16 Colors: the bar's menu, Bevel95, with the Colors submenu open", bar_submenu_scene(theme, era, &fonts, Some(BarEntry::Colors)));
        add("bevel95-settings", "16 Colors: Bevel95 Settings with 16 Colors chosen", properties_scene(theme, era, &fonts, properties::Sheet::Bevel95, 0));
        add("properties", "16 Colors: Taskbar Properties", properties_scene(theme, era, &fonts, properties::Sheet::Taskbar, 0));
        add("run", "16 Colors: Run (98 and 2000: a plain title bar)", dialog_scene(theme, &fonts, DialogKind::Run, era));
        add("about", "16 Colors: About Bevel95", about_scene(theme, era, &fonts));
        add("setup-first-run", "16 Colors: Bevel95 Setup, first page", setup_scene(theme, era, &fonts, "first-run"));
        add("setup-questions", "16 Colors: Bevel95 Setup's questions", setup_scene(theme, era, &fonts, "questions"));
        add("setup-update", "16 Colors: Bevel95 Setup over an installed Bevel95", setup_scene(theme, era, &fonts, "update"));
        add("setup-update-one-file", "16 Colors: Bevel95 Setup over an installed Bevel95, from the one file", setup_scene(theme, era, &fonts, "update-one-file"));
        add("shut-down", "16 Colors: Shut Down Windows", dialog_scene(theme, &fonts, DialogKind::ShutDown, era));
        let msans = load_font(&WIN95, false);
        add("tray-tooltip", "16 Colors: the notification area with a tooltip (95 and 98: white dotted with yellow)", tray_scene(theme, era, &fonts, &msans, false));
        add("volume", "16 Colors: the classic volume control", tray_scene(theme, era, &fonts, &msans, true));
        add("calculator", "16 Colors: Calculator (its pictures are 16-color drawings already)", calculator_scene(theme, era, &fonts, "play"));
        add("calculator-scientific", "16 Colors: Calculator's Scientific view", calculator_scene(theme, era, &fonts, "scientific"));
        add("minefield", "16 Colors: Minesweeper in play", minefield_scene(theme, era, &fonts, "play"));
        add("minefield-lost", "16 Colors: Minesweeper after a loss, inactive", minefield_scene(theme, era, &fonts, "lost"));
        add(
            "desktop-icons",
            "16 Colors: the desktop's icons, the long name selected: a navy checkerboard over its picture (98's and 2000's pictures by the look's rule until their own 16-color drawings come)",
            desktop_icons_scene(theme, era, &fonts),
        );
        if era != Era::Win95 {
            add(
                "folder-explorer",
                "16 Colors: a folder window with the look's own 16-color toolbar (black line art) and pictures",
                explorer_scene(theme, era, &fonts, "folder-explorer"),
            );
            add(
                "folder-web-drive",
                "16 Colors: the web view (98: no watercolor picture, its line by the rule)",
                web_scene(theme, era, &fonts, "folder-web-drive"),
            );
        }
    }
    DEPTH.set(Depth::TrueColor);
    out
}

/// Sizes measured on Windows itself, run in the v86 emulator (docs/theme-spec.md, "Window
/// menu"): the window menu of Windows 95 (127×117, "Alt+F4" 72 from the left of an entry)
/// and 98 (144 wide with its pictures, "Alt+F4" at 89), and Windows 98 Notepad's Edit menu
/// (137×186, shortcuts at 84).
#[test]
fn menus_measure_like_windows() {
    let normal = WindowState { resizable: true, has_minimize_box: true, has_maximize_box: true, closable: true, ..WindowState::default() };
    let measure = |theme: &Theme, menu: &Menu| {
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let l = paint::layout_menu(theme, &Fonts { regular: &regular, bold: &bold }, menu, MenuStyle::Popup, SCREEN_H);
        (l.width, l.height, l.shortcut_x)
    };
    assert_eq!(measure(&WIN95, &window_menu::menu(Era::Win95, normal)), (127, 117, 72));
    assert_eq!(measure(&WIN98, &window_menu::menu(Era::Win98, normal)), (144, 117, 89));
    let edit = [
        "&Undo\tCtrl+Z", "-", "Cu&t\tCtrl+X", "&Copy\tCtrl+C", "&Paste\tCtrl+V", "De&lete\tDel", "-", "Select &All",
        "Time/&Date\tF5", "-", "&Word Wrap", "Set &Font...",
    ];
    let entries = edit.iter().map(|&l| if l == "-" { MenuEntry::separator() } else { MenuEntry::item(l, None) });
    assert_eq!(measure(&WIN98, &Menu::new(entries.collect())), (137, 186, 84));
}

// ---- PNG in/out (RGB, 8 bits) ----

fn png_bytes(fb: &Framebuffer) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, fb.width as u32, fb.height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let data: Vec<u8> = fb.pixels.iter().flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8]).collect();
    writer.write_image_data(&data).unwrap();
    writer.finish().unwrap();
    bytes
}

fn write_png(path: &Path, fb: &Framebuffer) {
    std::fs::write(path, png_bytes(fb)).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn read_png(path: &Path) -> Result<Framebuffer, String> {
    let file = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let mut reader = png::Decoder::new(file).read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgb || info.bit_depth != png::BitDepth::Eight {
        return Err(format!("expected 8-bit RGB, found {:?} {:?}", info.color_type, info.bit_depth));
    }
    let bytes = &buf[..info.line_size * info.height as usize];
    let pixels = bytes.as_chunks::<3>().0.iter().map(|&[r, g, b]| rgb(u32::from_be_bytes([0, r, g, b]))).collect();
    Ok(Framebuffer { width: info.width as i32, height: info.height as i32, pixels })
}

/// Number of differing pixels and a picture of them: red where different, faded elsewhere.
fn diff(expected: &Framebuffer, actual: &Framebuffer) -> (usize, Framebuffer) {
    if (expected.width, expected.height) != (actual.width, actual.height) {
        return (expected.pixels.len().max(actual.pixels.len()), actual.clone());
    }
    let mut count = 0;
    let pixels = expected
        .pixels
        .iter()
        .zip(&actual.pixels)
        .map(|(&e, &a)| {
            if (e ^ a) & 0x00FF_FFFF != 0 {
                count += 1;
                rgb(0xFF0000)
            } else {
                let fade = |shift: u32| (((e >> shift) & 0xFF) + 3 * 255) / 4;
                rgb((fade(16) << 16) | (fade(8) << 8) | fade(0))
            }
        })
        .collect();
    (count, Framebuffer { width: actual.width, height: actual.height, pixels })
}

// ---- the preview page ----

fn preview_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/preview.html")
}

/// Standard base64 with `=` padding, for embedding the PNGs in the preview page.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (u32::from(b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() { ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// Every golden image embedded in one self-contained page, so it opens anywhere (a browser,
/// or a viewer that shows a copy of the file without the PNGs beside it).
fn preview_html(figures: &[(&Scene, Vec<u8>)]) -> String {
    let mut body = String::new();
    let mut section = "";
    for (scene, png) in figures {
        if scene.section != section {
            section = scene.section;
            let _ = writeln!(body, "\n  <h2>{section}</h2>");
        }
        let native = if scene.name.ends_with("-2x") { r#" data-native-scale="2""# } else { "" };
        let _ = writeln!(
            body,
            "  <figure><figcaption>{caption}: <code>{name}.png</code></figcaption>\n    \
             <div class=\"frame\"><img{native} alt=\"{caption}\" src=\"data:image/png;base64,{data}\"></div></figure>",
            caption = scene.caption,
            name = scene.name,
            data = base64(png),
        );
    }
    PREVIEW_TEMPLATE.replace("{{FIGURES}}", &body)
}

const PREVIEW_TEMPLATE: &str = r##"<!doctype html>
<!-- Generated by `cargo regen-golden` from core/tests/golden/*.png (see core/tests/golden.rs). Do not edit by hand. -->
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Bevel95 golden images</title>
<style>
  :root { --bg: #f4f4f2; --fg: #1d1d1b; --muted: #65655f; --line: #d6d6d0; --card: #ffffff; --accent: #000080; }
  @media (prefers-color-scheme: dark) {
    :root { --bg: #161615; --fg: #ececea; --muted: #a3a39d; --line: #34342f; --card: #1f1f1d; --accent: #8fa8ff; }
  }
  * { box-sizing: border-box; }
  body { margin: 0; padding: 24px 16px 48px; background: var(--bg); color: var(--fg);
         font: 15px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; }
  main { max-width: 1100px; margin: 0 auto; }
  h1 { font-size: 22px; margin: 0 0 4px; }
  h2 { font-size: 17px; margin: 32px 0 12px; border-bottom: 1px solid var(--line); padding-bottom: 4px; }
  p { margin: 0 0 12px; color: var(--muted); max-width: 70ch; }
  a { color: var(--accent); }
  .zoom { display: flex; gap: 6px; align-items: center; margin: 16px 0 8px; flex-wrap: wrap; }
  .zoom button { font: inherit; padding: 4px 12px; border: 1px solid var(--line); border-radius: 6px;
                 background: var(--card); color: var(--fg); cursor: pointer; }
  .zoom button[aria-pressed="true"] { border-color: var(--accent); color: var(--accent); font-weight: 600; }
  figure { margin: 0 0 20px; }
  figcaption { font-size: 13px; color: var(--muted); margin-bottom: 6px; }
  figcaption code { color: var(--fg); }
  .frame { overflow-x: auto; background: var(--card); border: 1px solid var(--line); border-radius: 6px; padding: 8px; }
  img { display: block; image-rendering: pixelated; image-rendering: crisp-edges; max-width: none; }
</style>
</head>
<body>
<main>
  <h1>Bevel95 golden images</h1>
  <p>The exact pictures the tests compare against (<code>core/tests/golden</code>), enlarged with hard square
     pixels. Compare them with real Windows 95 / 2000 screenshots. What to look at closely is listed under
     "Flagged for your eyes" in <a href="theme-spec.md#flagged-for-your-eyes">docs/theme-spec.md</a>.</p>
  <p>Zoom counts real screen pixels, whatever your Windows display scale: at 1× each picture pixel is exactly
     one screen pixel, which is what a Bevel95 taskbar set to 1× looks like on this screen.</p>
  <div class="zoom" role="group" aria-label="Zoom">
    <span>Zoom:</span>
    <button type="button" data-zoom="1">1×</button>
    <button type="button" data-zoom="2">2×</button>
    <button type="button" data-zoom="3">3×</button>
    <button type="button" data-zoom="4">4×</button>
    <span id="scale-note"></span>
  </div>
{{FIGURES}}</main>
<script>
  // Sizes are set in CSS pixels, which are devicePixelRatio screen pixels each (1.5 at 150%),
  // so divide by it: zoom z then means exactly z screen pixels per picture pixel.
  const images = [...document.querySelectorAll("img")];
  const buttons = [...document.querySelectorAll("[data-zoom]")];
  const note = document.getElementById("scale-note");
  let zoom = 3;
  function apply() {
    const dpr = window.devicePixelRatio || 1;
    for (const img of images) {
      const size = () => {
        img.style.width = (img.naturalWidth * zoom / (Number(img.dataset.nativeScale) || 1) / dpr) + "px";
      };
      if (img.complete) size(); else img.addEventListener("load", size, { once: true });
    }
    for (const b of buttons) b.setAttribute("aria-pressed", String(Number(b.dataset.zoom) === zoom));
    // Same rule as the taskbar: max(1, round(dpi / 96)), halves rounding up.
    note.textContent = "· this screen is at " + Math.round(dpr * 100) + "%, so the taskbar would draw at "
      + Math.max(1, Math.round(dpr)) + "×";
  }
  function setZoom(z) {
    zoom = z;
    apply();
    try { localStorage.setItem("bevel95-zoom", String(z)); } catch (e) {}
  }
  for (const b of buttons) b.addEventListener("click", () => setZoom(Number(b.dataset.zoom)));
  // Re-fit when the scale changes (window moved to another monitor, or browser zoom).
  (function watch() {
    matchMedia("(resolution: " + window.devicePixelRatio + "dppx)")
      .addEventListener("change", () => { apply(); watch(); }, { once: true });
  })();
  try { zoom = Number(localStorage.getItem("bevel95-zoom")) || 3; } catch (e) {}
  apply();
</script>
</body>
</html>
"##;

// ---- the tests ----

#[test]
fn base64_known_answers() {
    // RFC 4648 test vectors.
    let cases = [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ];
    for (input, want) in cases {
        assert_eq!(base64(input.as_bytes()), want, "{input:?}");
    }
}

/// A 16-color screen shows nothing but the 16 colors: every pixel of every 16-color scene is one.
#[test]
fn sixteen_color_scenes_use_only_the_16() {
    let mut wrong = Vec::new();
    for scene in sixteen_color_scenes() {
        let others: std::collections::BTreeSet<u32> =
            scene.image.pixels.iter().filter(|&&p| bevel_core::colors16::index(p).is_none()).map(|p| p & 0xFF_FFFF).collect();
        if !others.is_empty() {
            let some: Vec<String> = others.iter().take(6).map(|c| format!("{c:06X}")).collect();
            wrong.push(format!("{}: {} other colors, e.g. {}", scene.name, others.len(), some.join(" ")));
        }
    }
    assert!(wrong.is_empty(), "\n{}\n", wrong.join("\n"));
}

/// A 256-color screen shows the 16, the four other static colors, Windows 2000's own desktop and
/// face, and the colors of the pictures the browser and the folder windows drew from more colors
/// (cut from the Reference PCs' 256-color screens): every pixel of every 256-color scene is one.
#[test]
fn two_fifty_six_color_scenes_use_the_screens_colors() {
    use bevel_core::colors256;
    use std::collections::BTreeSet;
    let mut wrong = Vec::new();
    for scene in reduced_color_scenes(Depth::TwoFiftySix) {
        let era = if scene.name.starts_with("win2000") { Era::Win2000 } else if scene.name.starts_with("win98") { Era::Win98 } else { Era::Win95 };
        let mut allowed: BTreeSet<u32> = bevel_core::colors16::VGA.iter().copied().collect();
        allowed.extend([0xC0DCC0, 0xA6CAF0, 0xFFFBF0, 0xA0A0A4]);
        if era == Era::Win2000 {
            allowed.extend(colors256::KEPT_2000);
            allowed.insert(0xD7D7D7); // the infotip's edge, the browser's halftone gray
        }
        if era == Era::Win98 {
            allowed.extend([0xFF3300, 0xFFCC00, 0x66CC33, 0x0099FF]); // its web view's line, True Color's own
        }
        if era != Era::Win95 {
            let theme = bevel_core::theme::theme_in(era, Depth::TwoFiftySix);
            let pictures = [bevel_core::folder::web::corner_picture_256(era).pixels.clone(), colors256::throbber(theme).expect("throbber").pixels];
            allowed.extend(pictures.iter().flatten().map(|p| p & 0xFF_FFFF));
        }
        let others: BTreeSet<u32> = scene.image.pixels.iter().map(|p| p & 0xFF_FFFF).filter(|p| !allowed.contains(p)).collect();
        if !others.is_empty() {
            let some: Vec<String> = others.iter().take(6).map(|c| format!("{c:06X}")).collect();
            wrong.push(format!("{}: {} other colors, e.g. {}", scene.name, others.len(), some.join(" ")));
        }
    }
    assert!(wrong.is_empty(), "\n{}\n", wrong.join("\n"));
}

#[test]
fn golden_images_match() {
    let preview = std::fs::read_to_string(preview_path()).unwrap_or_default();
    let mut preview_is_stale = false;
    let mut failures = Vec::new();
    for Scene { name, image, .. } in scenes() {
        let path = test_dir("golden").join(format!("{name}.png"));
        if !std::fs::read(&path).is_ok_and(|bytes| preview.contains(&base64(&bytes))) {
            preview_is_stale = true;
        }
        let actual_path = path.with_extension("actual.png");
        let diff_path = path.with_extension("diff.png");
        let expected = match read_png(&path) {
            Ok(e) => e,
            Err(e) => {
                failures.push(format!("{name}: cannot read {} ({e}); `cargo regen-golden` creates it", path.display()));
                continue;
            }
        };
        let (count, picture) = diff(&expected, &image);
        if count == 0 {
            let _ = std::fs::remove_file(&actual_path);
            let _ = std::fs::remove_file(&diff_path);
        } else {
            write_png(&actual_path, &image);
            write_png(&diff_path, &picture);
            failures.push(format!("{name}: {count} pixels differ; see {}", diff_path.display()));
        }
    }
    if preview_is_stale {
        failures
            .push("docs/preview.html does not show the current golden images; `cargo regen-golden` rewrites it".into());
    }
    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}

#[test]
#[ignore = "rewrites the golden images and docs/preview.html; run it with `cargo regen-golden`"]
fn regenerate_golden_images() {
    let dir = test_dir("golden");
    std::fs::create_dir_all(&dir).unwrap();
    let scenes = scenes();
    let mut figures = Vec::new();
    for scene in &scenes {
        let bytes = png_bytes(&scene.image);
        let path = dir.join(format!("{}.png", scene.name));
        std::fs::write(&path, &bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        println!("wrote {}", path.display());
        figures.push((scene, bytes));
    }
    std::fs::write(preview_path(), preview_html(&figures)).unwrap();
    println!("wrote {}", preview_path().display());
}

/// The owner's sheet of Quick Launch (not a golden image, and not public): the left end of the bar
/// in each look (95 with the band switched on), True Color and 16 Colors, plain, with the pointer
/// on the first icon and with the second icon held down, Bevel95's own icons only, drawn by the
/// real code; labels outside the pictures; everything enlarged 3 times by whole pixels. Written to
/// the file named by BEVEL95_SHEET: run it with `BEVEL95_SHEET=<file> cargo test -p bevel-core
/// --test golden -- --ignored --exact draw_quick_launch_sheet`.
#[test]
#[ignore = "writes the owner's sheet of Quick Launch to BEVEL95_SHEET; run it on request"]
fn draw_quick_launch_sheet() {
    let Ok(path) = std::env::var("BEVEL95_SHEET") else {
        panic!("BEVEL95_SHEET names the file to write");
    };
    const CROP: i32 = 200;
    const LABEL_W: i32 = 150;
    const GAP: i32 = 10;
    const ROW_H: i32 = 28 + 16;
    let label_font = load_font(&WIN95, false);
    let rows = [(Era::Win95, Depth::TrueColor), (Era::Win98, Depth::TrueColor), (Era::Win2000, Depth::TrueColor), (Era::Win95, Depth::Sixteen), (Era::Win98, Depth::Sixteen), (Era::Win2000, Depth::Sixteen)];
    let columns: [(&str, Option<usize>, Option<usize>); 3] = [("plain", None, None), ("pointer on the first icon", Some(0), None), ("second icon held down", None, Some(1))];
    let width = LABEL_W + columns.len() as i32 * (CROP + GAP);
    let height = 16 + rows.len() as i32 * ROW_H;
    let mut sheet = Framebuffer::new(width, height, rgb(0xFFFFFF));
    for (c, (title, _, _)) in columns.iter().enumerate() {
        label_font.draw(&mut sheet, LABEL_W + c as i32 * (CROP + GAP), 2, title, rgb(0x000000));
    }
    for (r, &(era, depth)) in rows.iter().enumerate() {
        let theme = bevel_core::theme::theme_in(era, depth);
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        let y = 16 + r as i32 * ROW_H;
        let name = format!("{}{}", era.title(), if depth == Depth::Sixteen { ", 16 Colors" } else { "" });
        label_font.draw(&mut sheet, 2, y + 8, &name, rgb(0x000000));
        for (c, &(_, hot, pressed)) in columns.iter().enumerate() {
            let bar = quick_launch_scene(theme, era, &fonts, hot, pressed, CROP);
            sheet.blit(&bar, LABEL_W + c as i32 * (CROP + GAP), y);
        }
    }
    write_png(Path::new(&path), &upscale(&sheet, 3));
    println!("wrote the sheet");
}

/// The owner's sheet of Quick Launch made narrower and wider (brief 70; not a golden image, and not
/// public): per look the band at its default, dragged wider, dragged to its narrowest with the
/// look's overflow control (98 and the 95 look: also scrolled and the arrow held at the end; 2000:
/// also with its list open), Bevel95's own icons only, drawn by the real code, labels outside the
/// pictures, enlarged 3 times by whole pixels. `BEVEL95_SHEET=<file> cargo test -p bevel-core
/// --test golden -- --ignored --exact draw_quick_launch_size_sheet`.
/// The owner's sheet for brief 91 (laid out as the survey's `menus.png`): per look the long
/// Programs menu at 640x480 and at 800x600 as Bevel95 now shows it, drawn by the real code with
/// the survey's test entries, at true size, each labeled above its picture; since brief 92 also
/// 98's Start menu scrolling.
#[test]
#[ignore = "writes the owner's sheet of long menus to BEVEL95_SHEET; run it on request"]
fn draw_long_menu_sheet() {
    let Ok(path) = std::env::var("BEVEL95_SHEET") else {
        panic!("BEVEL95_SHEET names the file to write");
    };
    const GAP: i32 = 12;
    const LABEL: i32 = 18;
    let bold = load_font(&WIN95, true);
    let width = GAP + 640 + GAP + 800 + GAP;
    // Brief 92 adds 98's Start menu itself too tall; brief 94 keeps it above the bar and adds the
    // 384-row case: a fourth row (640x360 at the top, 683x384) and a fifth (640x360 at the end).
    let height = GAP + 3 * (LABEL + 600 + GAP) + LABEL + 384 + GAP + LABEL + 360 + GAP;
    let mut sheet = Framebuffer::new(width, height, rgb(0xFFFFFF));
    for (row, era) in Era::ALL.into_iter().enumerate() {
        let theme = bevel_core::theme::theme_in(era, Depth::TrueColor);
        let (regular, boldf) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &boldf };
        let (n640, n800) = match era {
            Era::Win95 => (80, 50),
            Era::Win98 => (117, 57),
            Era::Win2000 => (46, 59),
        };
        let y = GAP + row as i32 * (LABEL + 600 + GAP);
        let look = era.title();
        bold.draw(&mut sheet, GAP, y, &format!("{look}, 640x480, {n640} entries"), rgb(0x000000));
        sheet.blit(&long_menu_scene(theme, era, &fonts, 640, 480, n640, LongScene::Open), GAP, y + LABEL);
        bold.draw(&mut sheet, GAP + 640 + GAP, y, &format!("{look}, 800x600, {n800} entries"), rgb(0x000000));
        sheet.blit(&long_menu_scene(theme, era, &fonts, 800, 600, n800, LongScene::Open), GAP + 640 + GAP, y + LABEL);
    }
    let theme = bevel_core::theme::theme_in(Era::Win98, Depth::TrueColor);
    let (regular, boldf) = (load_font(theme, false), load_font(theme, true));
    let fonts = Fonts { regular: &regular, bold: &boldf };
    let y = GAP + 3 * (LABEL + 600 + GAP);
    bold.draw(&mut sheet, GAP, y, "Windows 98, the Start menu too tall (640x360): it scrolls above the bar", rgb(0x000000));
    sheet.blit(&start_scrolls_scene(theme, &fonts, false), GAP, y + LABEL);
    bold.draw(&mut sheet, GAP + 640 + GAP, y, "The same on a 768-row screen at Size 2 (683x384)", rgb(0x000000));
    sheet.blit(&start_scrolls_scene_sized(theme, &fonts, 683, 384, false), GAP + 640 + GAP, y + LABEL);
    let y = y + LABEL + 384 + GAP;
    bold.draw(&mut sheet, GAP, y, "640x360 scrolled to the end", rgb(0x000000));
    sheet.blit(&start_scrolls_scene(theme, &fonts, true), GAP, y + LABEL);
    write_png(Path::new(&path), &sheet);
    println!("wrote the sheet");
}

/// The owner's sheet for brief 95: per look Run's field with part of its text selected, and the
/// field's menu (part selected; nothing selected and nothing to paste), each labeled above it.
/// `BEVEL95_SHEET=<file> cargo test -p bevel-core --test golden -- --ignored --exact draw_text_field_sheet`.
#[test]
#[ignore = "writes the owner's sheet of text fields to BEVEL95_SHEET; run it on request"]
fn draw_text_field_sheet() {
    let Ok(path) = std::env::var("BEVEL95_SHEET") else {
        panic!("BEVEL95_SHEET names the file to write");
    };
    const GAP: i32 = 12;
    const LABEL: i32 = 18;
    let bold = load_font(&WIN95, true);
    let mut rows: Vec<(String, Vec<Framebuffer>)> = Vec::new();
    for era in Era::ALL {
        let theme = bevel_core::theme::theme_in(era, Depth::TrueColor);
        let (regular, boldf) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &boldf };
        let pictures = vec![field_selected_scene(theme, &fonts, era), field_menu_scene(theme, &fonts, true, true), field_menu_scene(theme, &fonts, false, false)];
        rows.push((format!("{}: Run's field with \"pad\" selected; its menu; the menu with nothing selected and nothing to paste", era.title()), pictures));
    }
    let width = GAP + rows.iter().map(|(_, p)| p.iter().map(|f| f.width + GAP).sum::<i32>()).max().unwrap_or(0).max(760);
    let height = GAP + rows.iter().map(|(_, p)| LABEL + p.iter().map(|f| f.height).max().unwrap_or(0) + GAP).sum::<i32>();
    let mut sheet = Framebuffer::new(width, height, rgb(0xFFFFFF));
    let mut y = GAP;
    for (label, pictures) in &rows {
        bold.draw(&mut sheet, GAP, y, label, rgb(0x000000));
        let mut x = GAP;
        for p in pictures {
            sheet.blit(p, x, y + LABEL);
            x += p.width + GAP;
        }
        y += LABEL + pictures.iter().map(|f| f.height).max().unwrap_or(0) + GAP;
    }
    write_png(Path::new(&path), &sheet);
    println!("wrote the sheet");
}

#[test]
#[ignore = "writes the owner's sheet of Quick Launch sizes to BEVEL95_SHEET; run it on request"]
fn draw_quick_launch_size_sheet() {
    let Ok(path) = std::env::var("BEVEL95_SHEET") else {
        panic!("BEVEL95_SHEET names the file to write");
    };
    const LABEL_W: i32 = 240;
    const GAP: i32 = 8;
    let label_font = load_font(&WIN95, false);
    let bold = load_font(&WIN95, true);
    let mut pictures: Vec<(String, Framebuffer)> = Vec::new();
    for era in Era::ALL {
        let theme = bevel_core::theme::theme_in(era, Depth::TrueColor);
        let (regular, boldf) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &boldf };
        let look = era.title();
        pictures.push((format!("{look}: never dragged (as before)"), quick_launch_scene(theme, era, &fonts, None, None, 240)));
        pictures.push((format!("{look}: dragged 40 wider"), quick_launch_sized(theme, era, &fonts, BandSize::Widened)));
        if era == Era::Win2000 {
            pictures.push((format!("{look}: 9 icons, narrowest, \u{bb}"), quick_launch_sized(theme, era, &fonts, BandSize::Narrowed)));
            pictures.push((format!("{look}: the \u{bb} list open"), quick_launch_sized(theme, era, &fonts, BandSize::List)));
        } else {
            pictures.push((format!("{look}: 9 icons, narrowest, arrow"), quick_launch_sized(theme, era, &fonts, BandSize::Narrowed)));
            pictures.push((format!("{look}: scrolled two icons"), quick_launch_sized(theme, era, &fonts, BandSize::Scrolled)));
            pictures.push((format!("{look}: arrow held at the end"), quick_launch_sized(theme, era, &fonts, BandSize::HeldAtEnd)));
        }
    }
    let width = LABEL_W + pictures.iter().map(|(_, p)| p.width).max().unwrap_or(0) + GAP;
    let height = 20 + pictures.iter().map(|(_, p)| p.height + GAP).sum::<i32>();
    let mut sheet = Framebuffer::new(width, height, rgb(0xFFFFFF));
    bold.draw(&mut sheet, 2, 2, "Quick Launch made narrower and wider (Bevel95's own icons)", rgb(0x000000));
    let mut y = 20;
    for (label, picture) in &pictures {
        label_font.draw(&mut sheet, 2, y + picture.height.min(28) / 2 - 6, label, rgb(0x000000));
        sheet.blit(picture, LABEL_W, y);
        y += picture.height + GAP;
    }
    write_png(Path::new(&path), &upscale(&sheet, 3));
    println!("wrote the sheet");
}

/// The owner's sheet of 256 Colors (not a golden image, and not public): for each look the same
/// three scenes (the desktop's left part with the bar and an open Start menu; a folder window; Run
/// with its title bar) in True Color, 256 Colors and 16 Colors side by side, drawn by the real
/// code, Bevel95's own icons and test items only, labels outside the pictures. Written to the file
/// named by BEVEL95_SHEET: `BEVEL95_SHEET=<file> cargo test -p bevel-core --test golden --
/// --ignored --exact draw_colors256_sheet`.
#[test]
#[ignore = "writes the owner's sheet of 256 Colors to BEVEL95_SHEET; run it on request"]
fn draw_colors256_sheet() {
    use bevel_core::desktop_art::Art;
    use bevel_core::desktop_icons::{Item, Picture};
    use bevel_core::folder::art::{self, DriveKind, Small};
    use bevel_core::folder::{FolderWindow, Space};
    use bevel_core::kit::Form;
    let Ok(path) = std::env::var("BEVEL95_SHEET") else {
        panic!("BEVEL95_SHEET names the file to write");
    };
    const GAP: i32 = 12;
    const LABEL_W: i32 = 130;
    const HEAD: i32 = 18;
    // Each scene's crop: the Start menu's lower left, the whole folder window, Run.
    let (start_w, start_h) = (330, 330);
    let (folder_w, folder_h) = (560, 360);
    let (run_w, run_h) = (350, 175);
    let depths = [(Depth::TrueColor, "True Color"), (Depth::TwoFiftySix, "256 Colors"), (Depth::Sixteen, "16 Colors")];
    let col_w = folder_w;
    let rows: [(&str, i32); 3] = [("Start menu", start_h), ("folder window", folder_h), ("Run", run_h)];
    let block_h: i32 = HEAD + rows.iter().map(|r| r.1 + GAP).sum::<i32>();
    let width = LABEL_W + 3 * (col_w + GAP);
    let height = 20 + 3 * block_h;
    let label_font = load_font(&WIN95, false);
    let bold = load_font(&WIN95, true);
    let mut sheet = Framebuffer::new(width, height, rgb(0xFFFFFF));
    for (c, (_, title)) in depths.iter().enumerate() {
        bold.draw(&mut sheet, LABEL_W + c as i32 * (col_w + GAP), 4, title, rgb(0x000000));
    }
    for (b, era) in Era::ALL.into_iter().enumerate() {
        let top = 20 + b as i32 * block_h;
        bold.draw(&mut sheet, 2, top + 2, era.title(), rgb(0x000000));
        for (c, &(depth, _)) in depths.iter().enumerate() {
            DEPTH.set(depth);
            let theme = bevel_core::theme::theme_in(era, depth);
            let (regular, boldf) = (load_font(theme, false), load_font(theme, true));
            let fonts = Fonts { regular: &regular, bold: &boldf };
            let x = LABEL_W + c as i32 * (col_w + GAP);
            let mut y = top + HEAD;
            // The desktop's left part with the bar and the Start menu open.
            let screen = start_menu_scene(theme, era, &fonts);
            let mut crop = Framebuffer::new(start_w, start_h, 0);
            crop.blit_part(&screen, Rect::new(0, screen.height - start_h, start_w, start_h), 0, 0);
            sheet.blit(&crop, x, y);
            y += start_h + GAP;
            // A folder window: 98's and 2000's with the web view (My Computer, the hard disk
            // selected), 95's My Computer.
            let folder = if era == Era::Win95 {
                let item = |id: &str, name: &str, a: Art| Item { id: id.into(), name: name.into(), picture: Picture::Art(a), shortcut: false, folder: false, type_name: String::new(), size: 0, modified: 0 };
                let mut w = FolderWindow::new(era, "My Computer", art::small(era, Small::MyComputer), false, (400, 300));
                w.layout(&fonts);
                w.set_items(vec![
                    item("A:\\", "3\u{bd} Floppy (A:)", Art::Drive(DriveKind::Floppy)),
                    item("C:\\", "Local (C:)", Art::Drive(DriveKind::Fixed)),
                    item("D:\\", "(D:)", Art::Drive(DriveKind::CdRom)),
                    item("::control-panel", "Control Panel", Art::ControlPanel),
                    item("::printers", "Printers", Art::Printers),
                ]);
                w.spaces.insert("C:\\".into(), Space { free: 120 << 30, capacity: 476 << 30 });
                w.view.select_only(Some(1));
                w.layout(&fonts);
                let (ww, wh) = w.size_now();
                let mut fb = Framebuffer::new(ww, wh, 0);
                w.paint(&mut fb, theme, &fonts);
                fb
            } else {
                web_scene(theme, era, &fonts, "folder-web-drive")
            };
            let mut back = Framebuffer::new(folder_w, folder_h, theme.colors.desktop);
            back.blit(&folder, 0, 0);
            sheet.blit(&back, x, y);
            y += folder_h + GAP;
            // Run, its title bar and its grayed-out OK (an empty line).
            let mut d = Dialog::new(DialogKind::Run, era);
            d.show_keys = true;
            let l = dialog::layout(&d);
            let mut fb = Framebuffer::new(l.width, l.height, 0);
            dialog::paint_dialog(&mut fb, theme, &fonts, &d, &l);
            let mut back = Framebuffer::new(run_w, run_h, theme.colors.desktop);
            back.blit(&fb, 0, 0);
            sheet.blit(&back, x, y);
        }
        let mut y = top + HEAD;
        for (name, h) in rows {
            label_font.draw(&mut sheet, 2, y + 4, name, rgb(0x000000));
            y += h + GAP;
        }
    }
    DEPTH.set(Depth::TrueColor);
    write_png(Path::new(&path), &sheet);
    println!("wrote the sheet");
}

/// The README's first picture, `docs/img/start-menus.png`: the three looks side by side, each
/// showing what a real Bevel95 shows with its Programs menu set to Bevel95's own programs. It
/// is not a golden image (no test compares it). Run it with
/// `cargo test -p bevel-core --test golden -- --ignored --exact draw_readme_start_menus`.
#[test]
#[ignore = "rewrites docs/img/start-menus.png, the README's picture; run it on request"]
fn draw_readme_start_menus() {
    const GAP: i32 = 8;
    let mut out = Framebuffer::new(3 * WIDTH + 2 * GAP, SCREEN_H, rgb(0xFFFFFF));
    for (i, (era, theme)) in [(Era::Win95, &WIN95), (Era::Win98, &WIN98), (Era::Win2000, &WIN2000)].into_iter().enumerate() {
        let (regular, bold) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &bold };
        out.blit(&own_programs_scene(theme, era, &fonts), i as i32 * (WIDTH + GAP), 0);
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("docs").join("img").join("start-menus.png");
    write_png(&path, &out);
    println!("wrote {}", path.display());
}

/// The website's picture (`site/desktop.png`, or the file named by `BEVEL95_SITE_PICTURE`): a new
/// tool on today's PC, one whole Windows 98 desktop, True Color, drawn by Bevel95's own code from
/// its own tables (never a golden scene that copies an old Windows screen, never anyone's screen).
/// Bottom to top: the wallpaper; the look's own desktop icons and two shortcuts of today's
/// programs (Google Chrome, ChatGPT) with Bevel95's shortcut arrow; Windows Explorer on My
/// Computer (a disk of today's size); Minesweeper in mid-game; the Calculator with a number in
/// its display; the taskbar with Start pressed, Quick Launch's own icons, a task button for each
/// window, the network icon, the speaker and the clock; the Start menu open on Programs, the
/// pointer on Google Chrome.
///
/// The screen's size is `BEVEL95_SITE_SIZE`: `1024x768` (without it) or `600x450` (the size the
/// README's column shows on whole screen pixels at 100, 125, 150 and 200%), each arranged by eye
/// (`site_layout`: at 600 x 450 only Minesweeper of the windows).
///
/// `BEVEL95_SITE_STEP` (1 to 5) draws one step of the website's short animation instead, at 600 x
/// 450 (`site_step`), on a PC where the person chose their picture earlier in each look: Bevel95
/// just started, the Start menu opened to Minesweeper, its own programs open, then the same
/// desktop in the 2000 and the 95 look.
///
/// The only pictures from outside (the wallpaper, a 1024 x 768 8-bit RGB PNG, and the two
/// programs' icons, 8-bit RGBA PNGs at 32 and 16 pixels) are read at run time from the folder
/// named by `BEVEL95_SITE_ASSETS` and are never in the repository. The wallpaper is the person's
/// own picture ("Bloom"), centered (a smaller screen cuts it equally on all sides, as Bevel95
/// draws a centered picture larger than the screen). Without the folder the scene has Clouds
/// stretched and nothing of Chrome or ChatGPT, so the test runs anywhere.
///
/// With the Start menu open no window is in front, as on the real bar (the root menu takes the
/// keyboard, `popup.rs`; the bar then marks no task button as the one in front): every title bar
/// inactive, every task button raised. Not a golden image (no test compares it). Run it with
/// `BEVEL95_SITE_ASSETS=<folder> BEVEL95_SITE_SIZE=<w>x<h> BEVEL95_SITE_PICTURE=<file> cargo test
/// -p bevel-core --test golden -- --ignored --exact draw_site_desktop`.
#[test]
#[ignore = "rewrites site/desktop.png, the website's picture; run it on request"]
fn draw_site_desktop() {
    let assets = std::env::var_os("BEVEL95_SITE_ASSETS").map(PathBuf::from).map(|dir| SiteAssets {
        wallpaper: read_png(&dir.join("wallpaper.png")).expect("wallpaper.png: 8-bit RGB"),
        chrome: (read_icon_png(&dir.join("chrome-32.png")).unwrap(), read_icon_png(&dir.join("chrome-16.png")).unwrap()),
        chatgpt: (read_icon_png(&dir.join("chatgpt-32.png")).unwrap(), read_icon_png(&dir.join("chatgpt-16.png")).unwrap()),
    });
    let picture = match std::env::var("BEVEL95_SITE_STEP") {
        Ok(step) => {
            let step: u32 = step.parse().ok().filter(|n| (1..=5).contains(n)).expect("BEVEL95_SITE_STEP is 1 to 5");
            site_scene(assets.as_ref(), &site_step(step))
        }
        Err(_) => {
            let size = match std::env::var("BEVEL95_SITE_SIZE").as_deref() {
                Err(_) | Ok("1024x768") => (1024, 768),
                Ok("600x450") => (600, 450),
                Ok(other) => panic!("BEVEL95_SITE_SIZE is 1024x768 or 600x450, not {other}"),
            };
            site_scene(assets.as_ref(), &site_layout(size))
        }
    };
    let path = match std::env::var_os("BEVEL95_SITE_PICTURE") {
        Some(path) => PathBuf::from(path),
        None => {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("site");
            std::fs::create_dir_all(&dir).unwrap();
            dir.join("desktop.png")
        }
    };
    write_png(&path, &picture);
    println!("wrote {}", path.display());
}

/// The website's pictures from outside the project (`draw_site_desktop`): the wallpaper and the
/// two programs' icons (32 and 16 pixels).
struct SiteAssets {
    wallpaper: Framebuffer,
    chrome: (Icon, Icon),
    chatgpt: (Icon, Icon),
}

/// An 8-bit RGBA PNG as an icon (straight see-through values, 0xAARRGGBB), as `read_png` reads RGB.
fn read_icon_png(path: &Path) -> Result<Icon, String> {
    let file = BufReader::new(File::open(path).map_err(|e| format!("{}: {e}", path.display()))?);
    let mut reader = png::Decoder::new(file).read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(format!("{}: expected 8-bit RGBA, found {:?} {:?}", path.display(), info.color_type, info.bit_depth));
    }
    let bytes = &buf[..info.line_size * info.height as usize];
    let pixels = bytes.as_chunks::<4>().0.iter().map(|&[r, g, b, a]| u32::from_be_bytes([a, r, g, b])).collect();
    Ok(Icon { width: info.width as i32, height: info.height as i32, pixels })
}

/// One picture of the website: its size and look, what is on the desktop, which windows are open
/// where (and whether in front), whether the Start menu or Display Properties is open.
struct SiteScene {
    size: (i32, i32),
    era: Era,
    /// The person's picture (or, without the assets, Clouds) on the desktop; else the look's
    /// plain color.
    wallpaper: bool,
    /// The desktop icons' cells by what they are (`::my-computer`, ..., `chrome`, `chatgpt`;
    /// desktop pixels, steps of 75); an icon of the look's own not named takes the next free cell
    /// below the last one named.
    icons: Vec<(&'static str, (i32, i32))>,
    explorer: Option<((i32, i32), (i32, i32))>,
    /// A window's place, and whether it is in front.
    minesweeper: Option<((i32, i32), bool)>,
    calculator: Option<((i32, i32), bool)>,
    start_menu: bool,
    /// The Start menu opened all the way to Programs > Accessories > Games, the pointer on
    /// Minesweeper (else only Programs, the pointer on Google Chrome).
    start_to_game: bool,
}

/// The still pictures of one size, arranged by eye (the Start menu open: no window in front).
fn site_layout(size: (i32, i32)) -> SiteScene {
    const C: i32 = bevel_core::desktop_icons::CELL;
    let ids = ["::my-computer", "::my-documents", "::recycle-bin", "chrome", "chatgpt"];
    let cells: [(i32, i32); 5] = match size {
        // The look's own down the left column, the two shortcuts at the top of the second (in the
        // first ChatGPT's name would lie under the Start menu).
        (1024, _) => [(0, 0), (0, C), (0, 2 * C), (C, 0), (C, C)],
        _ => SITE_SMALL_ICONS,
    };
    let icons = ids.into_iter().zip(cells).collect();
    let base = SiteScene {
        size,
        era: Era::Win98,
        wallpaper: true,
        icons,
        explorer: None,
        minesweeper: None,
        calculator: None,
        start_menu: true,
        start_to_game: false,
    };
    match size {
        // Explorer top right, Minesweeper under it at the right, the Calculator between
        // Minesweeper and the Programs menu.
        (1024, _) => SiteScene {
            explorer: Some(((452, 300), (556, 12))),
            minesweeper: Some(((840, 360), false)),
            calculator: Some(((556, 452), false)),
            ..base
        },
        // The Start menu fills the left from about y 42: Minesweeper between the Programs menu and
        // the icons down the right edge.
        _ => SiteScene { minesweeper: Some((SITE_SMALL_MINESWEEPER, false)), ..base },
    }
}

/// The 600 x 450 icons down the right edge: My Computer, My Documents, Recycle Bin, Google Chrome,
/// ChatGPT. ChatGPT's picture is black: swapped with Chrome's, it stands in the fourth cell, a
/// light ground of the column under the person's picture (measured: mean brightness 157 of 255
/// there, 33 in the fifth cell, where it would be swallowed by dark blue).
const SITE_SMALL_ICONS: [(i32, i32); 5] = [(525, 0), (525, 75), (525, 150), (525, 300), (525, 225)];
const SITE_SMALL_MINESWEEPER: (i32, i32) = (330, 40);

/// One step of the website's animation, 600 x 450, each what Bevel95 really shows at that point
/// on a PC where the person chose their picture as the wallpaper earlier, in each look (a start
/// of Bevel95 then shows it at once); the icons stand in the same cells throughout.
/// 1. Just started: the 98 look with the person's picture, nothing open.
/// 2. The Start menu opened to the program the next step shows: Programs > Accessories > Games,
///    the pointer on Minesweeper.
/// 3. Minesweeper and the Calculator open, Minesweeper in front.
/// 4. The Windows 2000 look, 5. the Windows 95 look: the same desktop, picture and programs in
///    that look's icons, bar and colors (Quick Launch as each look has it by default: none in 95),
///    Minesweeper still in front. 95 has no My Documents: its cell stays empty, as the icons keep
///    their places by what they are once arranged.
fn site_step(step: u32) -> SiteScene {
    let ids = ["::my-computer", "::my-documents", "::recycle-bin", "chrome", "chatgpt"];
    let base = SiteScene {
        size: (600, 450),
        era: Era::Win98,
        wallpaper: true,
        icons: ids.into_iter().zip(SITE_SMALL_ICONS).collect(),
        explorer: None,
        minesweeper: None,
        calculator: None,
        start_menu: false,
        start_to_game: false,
    };
    let programs = |front: bool| SiteScene {
        minesweeper: Some((SITE_SMALL_MINESWEEPER, front)),
        calculator: Some(((40, 120), false)),
        ..site_step(1)
    };
    match step {
        1 => base,
        2 => SiteScene { start_menu: true, start_to_game: true, ..base },
        3 => programs(true),
        4 => SiteScene { era: Era::Win2000, ..programs(true) },
        _ => SiteScene { era: Era::Win95, ..programs(true) },
    }
}
/// The name the person's picture is read under (the list shows it without its ending: "Bloom").
const SITE_WALLPAPER: &str = "C:\\Users\\Public\\Pictures\\Bloom.png";

/// Draws one picture of the website (`SiteScene`).
fn site_scene(assets: Option<&SiteAssets>, scene: &SiteScene) -> Framebuffer {
    use bevel_core::desktop_icons::{Desktop, Item as DeskItem, Picture};
    use bevel_core::display::{self, Choice, Show};
    use bevel_core::kit::Form;
    use bevel_core::programs::{OWN_PROGRAMS, add_own_programs};
    use bevel_core::status::{self, Network};
    let (w, h) = scene.size;
    let era = scene.era;
    let theme = bevel_core::theme::theme_in(era, Depth::TrueColor);
    let (regular, bold) = (load_font(theme, false), load_font(theme, true));
    let fonts = Fonts { regular: &regular, bold: &bold };
    let screen_rect = Rect::new(0, 0, w, h);
    let bar_top = h - theme.bar.height;

    // 1. The wallpaper: the person's own picture centered (cut equally on a smaller screen), or
    // Clouds stretched; or the look's plain color.
    let own_picture = assets.map(|a| display::Picture { width: a.wallpaper.width, height: a.wallpaper.height, pixels: a.wallpaper.pixels.clone() });
    let background = match (&own_picture, scene.wallpaper) {
        (_, false) => None,
        (Some(picture), true) => {
            let choice = Choice { wallpaper: Some(SITE_WALLPAPER.into()), show: Show::Center, pattern: None };
            display::background_on(theme, &choice, Some(picture), &[screen_rect], w, h)
        }
        (None, true) => display::background(theme, &Choice { wallpaper: Some("Clouds".into()), show: Show::Stretch, pattern: None }, w, h),
    };
    let mut screen = background.clone().unwrap_or_else(|| Framebuffer::new(w, h, theme.colors.desktop));

    // 2. The desktop icons in their cells, nothing selected.
    let area = Rect::new(0, 0, w, bar_top);
    let mut desk = Desktop::new(era, area);
    desk.set_background(background.map(std::sync::Arc::new));
    let chrome = "C:\\Users\\Public\\Desktop\\Google Chrome.lnk";
    let chatgpt = "C:\\Users\\Public\\Desktop\\ChatGPT.lnk";
    let id_of = |name: &str| match name {
        "chrome" => chrome.to_string(),
        "chatgpt" => chatgpt.to_string(),
        other => other.to_string(),
    };
    let mut places: Vec<(String, (i32, i32))> = scene.icons.iter().map(|&(name, cell)| (id_of(name), cell)).collect();
    // The look's own icons not named take the next cells below the last one named.
    let own_ids: Vec<String> = desk.items.iter().map(|item| item.id.clone()).collect();
    let mut next = places.iter().map(|&(_, (x, y))| (x, y)).max_by_key(|&(x, y)| (x, y)).unwrap_or((0, 0));
    for id in own_ids {
        if !places.iter().any(|(p, _)| *p == id) {
            next.1 += bevel_core::desktop_icons::CELL;
            places.push((id, next));
        }
    }
    let text: String = places.iter().map(|(id, (x, y))| format!("{x} {y} {id}\n")).collect();
    desk.load_places(&text);
    let shortcut = |id: &str, name: &str, picture: &Icon| DeskItem {
        id: id.to_string(),
        name: name.to_string(),
        picture: Picture::Own(Some(picture.clone())),
        shortcut: true,
        folder: false,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    match assets {
        Some(a) => desk.set_files(vec![shortcut(chrome, "Google Chrome", &a.chrome.0), shortcut(chatgpt, "ChatGPT", &a.chatgpt.0)]),
        None => desk.set_files(vec![]),
    }
    desk.active = false;
    desk.paint(&mut screen, theme, &fonts, area);

    // 3. The windows. Google Chrome runs, minimized (its button only). With the Start menu or
    // Display Properties open no task button is the one in front (both are the bar's own windows).
    let task = |title: &str, icon: Icon, front: bool| Task {
        title: title.to_string(),
        icon: Some(icon),
        state: if front { TaskState::Active } else { TaskState::Normal },
    };
    let mut tasks = Vec::new();
    if let Some(a) = assets {
        tasks.push(task("Google Chrome", a.chrome.1.clone(), false));
    }
    // Explorer: 98's status bar has fixed parts (240 and 218 pixels), so the drive's free space,
    // in the middle part, would need about 660 pixels across; narrower, the middle part has no
    // room and stays out (a part cut in the middle of a word looks broken).
    if let Some((explorer_size, at)) = scene.explorer {
        let explorer = site_explorer(theme, era, &fonts, explorer_size);
        screen.blit(&explorer, at.0, at.1);
        tasks.push(task("Exploring - My Computer", bevel_core::folder::tree_art::explorer(era), false));
    }
    // The one in front is drawn last.
    let mut windows: Vec<(Framebuffer, (i32, i32), bool)> = Vec::new();
    if let Some((at, front)) = scene.minesweeper {
        let mut game = minefield_in_play(era, &fonts);
        game.set_active(front);
        windows.push((form_image(&mut game, theme, &fonts), at, front));
        tasks.push(task(bevel_core::minefield::form::TITLE, bevel_core::minefield::art::icon(era, 16), front));
    }
    if let Some((at, front)) = scene.calculator {
        // Standard view, a sum done: 1024 x 768 = 786432.
        let mut calc = bevel_core::calculator::form::CalculatorWindow::new(era);
        calc.paste_text("1024*768=");
        calc.set_active(front);
        windows.push((form_image(&mut calc, theme, &fonts), at, front));
        tasks.push(task(bevel_core::calculator::form::TITLE, bevel_core::calculator::art::icon(era, 16), front));
    }
    windows.sort_by_key(|(_, _, front)| *front);
    for (fb, at, _) in &windows {
        screen.blit(fb, at.0, at.1);
    }
    // 4. The taskbar (Quick Launch as the look has it by default).
    let quick_launch = bevel_core::quick_launch::on_by_default(era).then(|| {
        let icons = bevel_core::quick_launch::builtins(era).iter().map(|&b| bevel_core::quick_launch::picture(era, b)).collect();
        bevel_core::quick_launch::Band { icons, ..Default::default() }
    });
    let net = Network { connected: true, ..Network::default() };
    let state = Bar {
        start_pressed: scene.start_menu,
        tasks,
        tray: vec![Some(status::network_icon(era, &net)), Some(icons::speaker(false))],
        clock: CLOCK.to_string(),
        clock_widest: WIDEST.to_string(),
        quick_launch,
        ..Bar::default()
    };
    let mut bar = Framebuffer::new(w, theme.bar.height, 0);
    paint::paint_bar(&mut bar, theme, &fonts, &state);
    screen.blit(&bar, 0, bar_top);
    let menu_image = |menu: &Menu, style: MenuStyle| {
        let layout = paint::layout_menu(theme, &fonts, menu, style, h);
        let mut fb = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut fb, theme, &fonts, menu, style, &layout);
        (fb, layout)
    };

    if !scene.start_menu {
        return screen;
    }

    // 5. The Start menu, open on Programs, the pointer on Google Chrome (without the assets:
    // on Accessories); or opened to Programs > Accessories > Games, the pointer on Minesweeper.
    let mut menu = start::menu(era, StartOptions::default(), "Admin", |e| icons::start_icon(era, e));
    let programs_index = menu.entries.iter().position(|e| e.label == "&Programs").expect("Programs");
    menu.hover = Some(programs_index);
    menu.show_keys = era.always_shows_keys();
    let (fb, layout) = menu_image(&menu, MenuStyle::Start);
    let button = layout::start_button(theme, &fonts);
    let button = Rect::new(button.x, bar_top + button.y, button.w, button.h);
    let (x, y) = place_start_menu(button, fb.width, fb.height, screen_rect);
    screen.blit(&fb, x, y);
    let mut programs = Folder::default();
    add_own_programs(&mut programs, era, |program| Some(program.file.to_string()));
    if assets.is_some() {
        programs.add(&[], Item { name: "Google Chrome".into(), target: "chrome".into() });
        programs.add(&[], Item { name: "ChatGPT".into(), target: "chatgpt".into() });
    }
    programs.sort();
    let group = icons::program_group(era);
    let picture = |item: &Item| match (item.target.as_str(), assets) {
        ("chrome", Some(a)) => Some(a.chrome.1.clone()),
        ("chatgpt", Some(a)) => Some(a.chatgpt.1.clone()),
        _ => OWN_PROGRAMS.iter().find(|p| p.file == item.target).map(|p| (p.icon)(era)),
    };
    let path: &[&str] = match (scene.start_to_game, assets.is_some()) {
        (true, _) => &["Accessories", "Games", bevel_core::minefield::form::TITLE],
        (false, true) => &["Google Chrome"],
        (false, false) => &["Accessories"],
    };
    // Each menu beside the entry it opens from, placed as the bar places a cascading menu
    // (`place_submenu` against the screen: turned left or moved up where it would not fit).
    let mut parent = Rect::new(x, y, fb.width, fb.height);
    let mut entry = entry_on_screen(parent, &layout, programs_index);
    let mut at: Vec<usize> = Vec::new();
    for wanted in path {
        let folder = programs.folder_at(&at).expect("folder");
        let mut sub = folder.menu(Some(&group), picture);
        let index = sub.entries.iter().position(|e| e.label == *wanted).unwrap_or_else(|| panic!("{wanted} in the menu"));
        sub.hover = Some(index);
        sub.show_keys = menu.show_keys;
        let (sub_fb, sub_layout) = menu_image(&sub, MenuStyle::Cascade);
        let (sx, sy) = place_submenu(parent, entry, sub_fb.width, sub_fb.height, 3, theme.start_menu.submenu, screen_rect);
        screen.blit(&sub_fb, sx, sy);
        parent = Rect::new(sx, sy, sub_fb.width, sub_fb.height);
        entry = entry_on_screen(parent, &sub_layout, index);
        at.push(index);
    }
    screen
}

/// A form painted at its own size.
fn form_image<F: bevel_core::kit::Form>(form: &mut F, theme: &Theme, fonts: &Fonts) -> Framebuffer {
    form.layout(fonts);
    let (w, h) = form.size();
    let mut fb = Framebuffer::new(w, h, 0);
    form.paint(&mut fb, theme, fonts);
    fb
}

/// Windows Explorer on My Computer for the website's picture, built as `tree_scene` builds its
/// window: the tree Desktop > My Computer (open) > the drives and Control Panel, My Documents,
/// Recycle Bin, My Computer selected; on the right the floppy and the hard disk, C: selected, of
/// today's size (hundreds of GB free); inactive.
fn site_explorer(theme: &Theme, era: Era, fonts: &Fonts, size: (i32, i32)) -> Framebuffer {
    use bevel_core::desktop_art::Art;
    use bevel_core::desktop_icons::{Item, Picture};
    use bevel_core::folder::art::{self, DriveKind, Small};
    use bevel_core::folder::pane::{NodeKind as K, NodeSpec, Tree};
    use bevel_core::folder::{FolderWindow, Space, tree_art};
    use bevel_core::kit::Form;
    let spec = |id: &str, name: &str, kind: K, has_rows: bool| NodeSpec { id: id.into(), name: name.into(), kind, has_rows, hidden: false };
    let mut tree = Tree::new();
    let _ = tree.wanted();
    tree.set_rows(
        "::desktop",
        vec![
            spec("::my-computer", "My Computer", K::MyComputer, true),
            spec("::my-documents", "My Documents", K::MyDocuments, true),
            spec("::recycle-bin", "Recycle Bin", K::RecycleBin, false),
        ],
    );
    tree.set_rows(
        "::my-computer",
        vec![
            spec("A:\\", "3\u{bd} Floppy (A:)", K::Drive(DriveKind::Floppy), true),
            spec("C:\\", "Local Disk (C:)", K::Drive(DriveKind::Fixed), true),
            spec("D:\\", "(D:)", K::Drive(DriveKind::CdRom), true),
            spec("::control-panel", "Control Panel", K::ControlPanel, false),
        ],
    );
    tree.follow(vec!["::my-computer".into()], 20);
    let drive = |id: &str, name: &str, kind: DriveKind| Item {
        id: id.into(),
        name: name.into(),
        picture: Picture::Art(Art::Drive(kind)),
        shortcut: false,
        folder: false,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    let title = "Exploring - My Computer";
    let icon = tree_art::explorer(era);
    let mut w = FolderWindow::new(era, title, icon.clone(), false, size);
    w.layout(fonts);
    w.show_place("My Computer", icon.clone(), false, "My Computer", art::listed(era, Small::MyComputer));
    w.title = title.to_string();
    w.icon = icon;
    w.place_name = "My Computer".into();
    w.exploring = true;
    w.tree = tree;
    w.show_tree(true);
    w.spaces.insert("C:\\".into(), Space { free: 312 << 30, capacity: 476 << 30 });
    w.set_items(vec![drive("A:\\", "3\u{bd} Floppy (A:)", DriveKind::Floppy), drive("C:\\", "Local Disk (C:)", DriveKind::Fixed)]);
    w.view.select_only(Some(1));
    w.disk_free = Some(312 << 30);
    w.set_active(false);
    w.layout(fonts);
    w.tree_selection_shown();
    let (ww, wh) = w.size_now();
    let mut fb = Framebuffer::new(ww, wh, 0);
    w.paint(&mut fb, theme, fonts);
    fb
}

/// Minesweeper in mid-game: `minefield_scene`'s "play" board (Beginner, its mines), its squares
/// opened and a few more, three flags on mines, 47 seconds, and no square held down, so the face
/// smiles.
fn minefield_in_play(era: Era, fonts: &Fonts) -> bevel_core::minefield::form::MinefieldWindow {
    use bevel_core::kit::Form;
    use bevel_core::minefield::form::MinefieldWindow;
    use bevel_core::minefield::game::{Field, Game, Level};
    let custom = Field { width: 20, height: 12, mines: 40 };
    let mut w = MinefieldWindow::new(era, Level::Beginner, custom, true, 1);
    w.layout(fonts);
    let field = w.field();
    w.game = Game::with_mines(field.width, field.height, &FIELD_MINES);
    for (x, y) in [(0, 0), (6, 2), (5, 4), (6, 5), (3, 3), (4, 2), (2, 3), (3, 4), (4, 3), (2, 2), (4, 1)] {
        assert!(!FIELD_MINES.contains(&(x, y)), "a safe square");
        w.game.open(x, y);
    }
    for (x, y) in [(3, 0), (5, 3), (7, 1)] {
        w.game.mark(x, y, true);
    }
    w.game.set_seconds(47);
    w
}

/// One look's screen for the README: the bar with three windows of Bevel95's own programs (My
/// Computer, Minesweeper active, Calculator) and their own pictures, Start pressed, the Start
/// menu open on Programs, and the Programs menu put together as the bar does it when it shows
/// only Bevel95's own programs (`programs::add_own_programs`, each entry with its own picture,
/// folders with the look's program group picture), opened down to Games.
fn own_programs_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    use bevel_core::programs::{OWN_PROGRAMS, add_own_programs};
    let screen_rect = Rect::new(0, 0, WIDTH, SCREEN_H);
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let own = |title: &str, icon: Icon, state: TaskState| Task { title: title.to_string(), icon: Some(icon), state };
    let tasks = vec![
        own("My Computer", window_icons::icon(era, ClassicWindow::MyComputer), TaskState::Normal),
        own(bevel_core::minefield::form::TITLE, bevel_core::minefield::art::icon(era, 16), TaskState::Active),
        own(bevel_core::calculator::form::TITLE, bevel_core::calculator::art::icon(era, 16), TaskState::Normal),
    ];
    let bar = bar_image(theme, fonts, true, tasks);
    let bar_top = SCREEN_H - bar.height;
    screen.blit(&bar, 0, bar_top);

    let mut menu = start::menu(era, StartOptions::default(), "Admin", |e| icons::start_icon(era, e));
    let programs_index = menu.entries.iter().position(|e| e.label == "&Programs").expect("Programs");
    menu.hover = Some(programs_index);
    menu.show_keys = era.always_shows_keys();
    let (fb, layout) = menu_image(theme, fonts, &menu, MenuStyle::Start);
    let button = layout::start_button(theme, fonts);
    let button = Rect::new(button.x, bar_top + button.y, button.w, button.h);
    let (x, y) = place_start_menu(button, fb.width, fb.height, screen_rect);
    screen.blit(&fb, x, y);

    // The Programs tree as the bar builds it (each program's target is its file here).
    let mut programs = Folder::default();
    add_own_programs(&mut programs, era, |program| Some(program.file.to_string()));
    programs.sort();
    let group = icons::program_group(era);
    let picture = |item: &Item| OWN_PROGRAMS.iter().find(|p| p.file == item.target).map(|p| (p.icon)(era));
    // Programs > Accessories > Games > Minesweeper, each menu placed beside the entry it opens from.
    let mut parent = Rect::new(x, y, fb.width, fb.height);
    let mut entry = entry_on_screen(parent, &layout, programs_index);
    let mut path: Vec<usize> = Vec::new();
    for wanted in ["Accessories", "Games", bevel_core::minefield::form::TITLE] {
        let folder = programs.folder_at(&path).expect("folder");
        let mut sub = folder.menu(Some(&group), picture);
        let index = sub.entries.iter().position(|e| e.label == wanted).unwrap_or_else(|| panic!("{wanted}"));
        sub.hover = Some(index);
        sub.show_keys = menu.show_keys;
        let (sub_fb, sub_layout) = menu_image(theme, fonts, &sub, MenuStyle::Cascade);
        let (sx, sy) = place_submenu(parent, entry, sub_fb.width, sub_fb.height, 3, theme.start_menu.submenu, screen_rect);
        screen.blit(&sub_fb, sx, sy);
        parent = Rect::new(sx, sy, sub_fb.width, sub_fb.height);
        entry = entry_on_screen(parent, &sub_layout, index);
        path.push(index);
    }
    screen
}

/// Windows 95's folder windows: My Computer, a folder of files (test pictures) and one with its File
/// menu open, on the look's desktop.
fn folder_scene(fonts: &Fonts, which: &str) -> Framebuffer {
    use bevel_core::desktop_art::Art;
    use bevel_core::desktop_icons::{Item, Picture};
    use bevel_core::folder::art::{self, DriveKind, Small};
    use bevel_core::folder::{FolderWindow, Space, menus};
    use bevel_core::kit::Form;
    let item = |id: &str, name: &str, picture: Picture, folder: bool, size: u64| Item {
        id: id.to_string(),
        name: name.to_string(),
        picture,
        shortcut: false,
        folder,
        type_name: String::new(),
        size,
        modified: 0,
    };
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(Era::Win95));
    let at = (40, 30);
    let mut w = if which == "folder-mycomputer" {
        let mut w = FolderWindow::new(Era::Win95, "My Computer", art::small(Era::Win95, Small::MyComputer), false, (265, 294));
        w.layout(fonts);
        w.set_items(vec![
            item("A:\\", "3\u{bd} Floppy (A:)", Picture::Art(Art::Drive(DriveKind::Floppy)), false, 0),
            item("C:\\", "Local (C:)", Picture::Art(Art::Drive(DriveKind::Fixed)), false, 0),
            item("D:\\", "(D:)", Picture::Art(Art::Drive(DriveKind::CdRom)), false, 0),
            item("E:\\", "Stick (E:)", Picture::Art(Art::Drive(DriveKind::Removable)), false, 0),
            item("::control-panel", "Control Panel", Picture::Art(Art::ControlPanel), false, 0),
            item("::printers", "Printers", Picture::Art(Art::Printers), false, 0),
        ]);
        w.spaces.insert("C:\\".into(), Space { free: 120 << 30, capacity: 476 << 30 });
        w.view.select_only(Some(1));
        w
    } else {
        let doc = icon(&DOCUMENT, 2);
        let mut w = FolderWindow::new(Era::Win95, "Letters", art::small(Era::Win95, Small::Folder), true, (340, 294));
        w.layout(fonts);
        w.set_items(vec![
            item("C:\\L\\Old", "Old", Picture::Folder, true, 0),
            item("C:\\L\\To the bank about the account.txt", "To the bank about the account.txt", Picture::Own(Some(doc.clone())), false, 2_048),
            item("C:\\L\\Notes.txt", "Notes.txt", Picture::Own(Some(doc.clone())), false, 489),
            item("C:\\L\\Plans.txt", "Plans.txt", Picture::Own(Some(doc.clone())), false, 31_000),
            item("C:\\L\\Lists.txt", "Lists.txt", Picture::Own(Some(doc)), false, 700),
        ]);
        w.hidden = 1;
        w.total_size = 34_237;
        if which == "folder-hidden" {
            w.set_items(vec![
                item("C:\\L\\Old", "Old", Picture::Folder, true, 0),
                item("C:\\L\\Private", "Private", Picture::Folder, true, 0),
                item("C:\\L\\Notes.txt", "Notes.txt", Picture::Own(Some(icon(&DOCUMENT, 2))), false, 489),
                item("C:\\L\\Secret.txt", "Secret.txt", Picture::Own(Some(icon(&DOCUMENT, 2))), false, 1_200),
                item("C:\\L\\Plans.txt", "Plans.txt", Picture::Own(Some(icon(&DOCUMENT, 2))), false, 31_000),
            ]);
            w.view.set_hidden(["C:\\L\\Private".to_string(), "C:\\L\\Secret.txt".to_string()]);
            w.view.select_only(Some(0));
        } else if which == "folder-files" {
            let long = w.view.index_of("C:\\L\\To the bank about the account.txt");
            w.view.select_only(long);
            w.resize((230, 220), false);
        } else {
            w.view.select_only(Some(2));
            w.chosen = Some(0);
        }
        w
    };
    w.set_screen(at.0, at.1);
    w.layout(fonts);
    if which == "folder-menu" {
        let rows = menus::bar_rows(&w.menu_state(false), 0);
        w.help = Some(rows[0].help.to_string());
    }
    let (ww, wh) = w.size_now();
    let mut fb = Framebuffer::new(ww, wh, 0);
    w.paint(&mut fb, &WIN95, fonts);
    screen.blit(&fb, at.0, at.1);
    if which == "folder-menu" {
        let rows = menus::bar_rows(&w.menu_state(false), 0);
        let menu = Menu { hover: Some(0), ..menus::menu(&rows) };
        let (img, _) = menu_image(&WIN95, fonts, &menu, MenuStyle::Popup);
        let r = w.title_rect(0).unwrap();
        screen.blit(&img, at.0 + r.x, at.1 + r.bottom());
    }
    screen
}
/// Windows 98's and 2000's folder window: a folder of files reached from My Computer (Back lit),
/// a file selected and the pointer on a toolbar button, or the address bar's list open.
fn explorer_scene(theme: &Theme, era: Era, fonts: &Fonts, which: &str) -> Framebuffer {
    use bevel_core::desktop_icons::{Item, Picture};
    use bevel_core::folder::art::{self, DriveKind, Small};
    use bevel_core::folder::menus::Nav;
    use bevel_core::folder::{FolderWindow, ListRow, Tool};
    use bevel_core::kit::Form;
    let doc = icon(&DOCUMENT, 2);
    let item = |name: &str, folder: bool, size: u64| Item {
        id: format!("C:\\Letters\\{name}"),
        name: name.to_string(),
        picture: if folder { Picture::Folder } else { Picture::Own(Some(doc.clone())) },
        shortcut: false,
        folder,
        type_name: "Text Document".into(),
        size,
        modified: 0,
    };
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let at = (20, 20);
    let mut w = FolderWindow::new(era, "Letters", art::small(era, Small::Folder), true, (420, 300));
    w.layout(fonts);
    w.show_place("Letters", art::small(era, Small::Folder), true, if era == Era::Win98 { "C:\\Letters" } else { "Letters" }, art::listed(era, Small::Folder));
    w.set_nav(Nav { back: true, up: true, ..Nav::default() });
    w.set_items(vec![item("Old", true, 0), item("Notes.txt", false, 489), item("Plans.txt", false, 31_000), item("To the bank about the account.txt", false, 2_048)]);
    w.total_size = 33_537;
    if which == "folder-explorer" {
        w.view.select_only(Some(2));
        w.bar.hot_tool = Some((if era == Era::Win98 { Tool::Copy } else { Tool::Delete }, false));
    } else {
        let row = |level: i32, small: Small, name: &str| ListRow { level, icon: art::listed(era, small), name: name.to_string() };
        w.field.focus();
        let rows = vec![
            row(0, Small::Desktop, "Desktop"),
            row(1, Small::MyComputer, "My Computer"),
            row(2, Small::Drive(DriveKind::Floppy), "3\u{bd} Floppy (A:)"),
            row(2, Small::Drive(DriveKind::Fixed), "Local Disk (C:)"),
            row(3, Small::ClosedFolder, "Letters"),
            row(2, Small::ControlPanel, "Control Panel"),
            row(1, Small::RecycleBin, "Recycle Bin"),
        ];
        w.set_list(Some((rows, Some(4))));
    }
    w.set_screen(at.0, at.1);
    w.layout(fonts);
    let (ww, wh) = w.size_now();
    let mut fb = Framebuffer::new(ww, wh, 0);
    w.paint(&mut fb, theme, fonts);
    screen.blit(&fb, at.0, at.1);
    screen
}

fn era_prefix(era: Era) -> &'static str {
    match era {
        Era::Win95 => "win95",
        Era::Win98 => "win98",
        Era::Win2000 => "win2000",
    }
}

/// An Explorer window in `era`: the tree read down to C:\Letters (its folders below it, a long
/// name cut at the pane's edge, the scroll bar across), Letters selected with the keyboard in
/// the tree, its icons on the right.
fn tree_scene(theme: &Theme, era: Era, fonts: &Fonts) -> Framebuffer {
    use bevel_core::desktop_icons::{Item, Picture};
    use bevel_core::folder::art::{self, DriveKind, Small};
    use bevel_core::folder::menus::Nav;
    use bevel_core::folder::pane::{NodeKind as K, NodeSpec, Tree};
    use bevel_core::folder::{FolderWindow, tree_art};
    use bevel_core::kit::Form;
    let spec = |id: &str, name: &str, kind: K, has_rows: bool| NodeSpec { id: id.into(), name: name.into(), kind, has_rows, hidden: false };
    let mut tree = Tree::new();
    let _ = tree.wanted();
    let mut on_desktop = vec![spec("::my-computer", "My Computer", K::MyComputer, true), spec("::recycle-bin", "Recycle Bin", K::RecycleBin, false)];
    if era != Era::Win95 {
        on_desktop.insert(if era == Era::Win98 { 1 } else { 0 }, spec("::my-documents", "My Documents", K::MyDocuments, true));
    }
    tree.set_rows("::desktop", on_desktop);
    tree.set_rows(
        "::my-computer",
        vec![
            spec("A:\\", "3\u{bd} Floppy (A:)", K::Drive(DriveKind::Floppy), true),
            spec("C:\\", "Local (C:)", K::Drive(DriveKind::Fixed), true),
            spec("D:\\", "(D:)", K::Drive(DriveKind::CdRom), true),
            spec("::control-panel", "Control Panel", K::ControlPanel, false),
        ],
    );
    tree.set_rows("C:\\", vec![spec("C:\\Letters", "Letters", K::Folder, true), spec("C:\\Programs", "Programs", K::Folder, true)]);
    tree.set_rows("C:\\Letters", vec![spec("C:\\Letters\\Old", "Old", K::Folder, false), spec("C:\\Letters\\To the bank about the account", "To the bank about the account", K::Folder, false)]);
    tree.follow(vec!["::my-computer".into(), "C:\\".into(), "C:\\Letters".into()], 20);
    let doc = icon(&DOCUMENT, 2);
    let item = |name: &str, folder: bool, size: u64| Item {
        id: format!("C:\\Letters\\{name}"),
        name: name.to_string(),
        picture: if folder { Picture::Folder } else { Picture::Own(Some(doc.clone())) },
        shortcut: false,
        folder,
        type_name: "Text Document".into(),
        size,
        modified: 0,
    };
    let mut screen = Framebuffer::new(WIDTH, SCREEN_H, desktop(era));
    let at = (20, 20);
    let title = if era == Era::Win2000 { "Letters".to_string() } else { "Exploring - Letters".to_string() };
    let icon = tree_art::explorer(era);
    let mut w = FolderWindow::new(era, &title, icon.clone(), true, (520, 330));
    w.layout(fonts);
    if era != Era::Win95 {
        w.show_place("Letters", icon.clone(), true, if era == Era::Win98 { "C:\\Letters" } else { "Letters" }, art::listed(era, Small::Folder));
        w.set_nav(Nav { back: true, up: true, ..Nav::default() });
    }
    w.title = title;
    w.icon = icon;
    w.place_name = "Letters".into();
    w.exploring = true;
    w.tree = tree;
    w.show_tree(true);
    w.set_items(vec![item("Old", true, 0), item("To the bank about the account", true, 0), item("Notes.txt", false, 489), item("Plans.txt", false, 31_000)]);
    w.total_size = 31_489;
    w.disk_free = Some(120 << 30);
    w.tree_focus = true;
    w.set_screen(at.0, at.1);
    w.layout(fonts);
    w.tree_selection_shown();
    let (ww, wh) = w.size_now();
    let mut fb = Framebuffer::new(ww, wh, 0);
    w.paint(&mut fb, theme, fonts);
    screen.blit(&fb, at.0, at.1);
    screen
}
/// A folder window of `era` in `mode` (the golden pictures of the folder views).
fn views_scene(theme: &Theme, era: Era, fonts: &Fonts, mode: ViewMode) -> Framebuffer {
    use bevel_core::desktop_icons::{Item, Picture, local_filetime};
    use bevel_core::folder::{FolderWindow, art};
    use bevel_core::kit::Form;
    let doc = icon(&DOCUMENT, 2);
    let item = |name: &str, folder: bool, size: u64, kind: &str, day: i64| Item {
        id: format!("C:\\L\\{name}"),
        name: name.to_string(),
        picture: if folder { Picture::Folder } else { Picture::Own(Some(doc.clone())) },
        shortcut: false,
        folder,
        type_name: kind.to_string(),
        size,
        modified: local_filetime(2026, 11, day, 14, 39),
    };
    let items = vec![
        item("Letters", true, 0, "File folder", 1),
        item("Photos", true, 0, "File folder", 2),
        item("To the bank about the account.txt", false, 2_048, "Text Document", 3),
        item("Notes.txt", false, 489, "Text Document", 4),
        item("Plans.txt", false, 31_000, "Text Document", 5),
        item("Lists.txt", false, 1_638_400, "Text Document", 6),
    ];
    let small = art::Small::Folder;
    let icon16 = art::small(era, small);
    let mut w = FolderWindow::new(era, "Letters", icon16.clone(), true, (430, 300));
    w.layout(fonts);
    if era != Era::Win95 {
        w.show_place("Letters", icon16, true, "C:\\Letters", art::listed(era, small));
    }
    w.set_view_mode(mode);
    w.set_items(items);
    w.view.keys_used = true;
    if mode == ViewMode::Details {
        w.header_click(1);
        w.header_click(1);
    }
    let i = w.view.index_of("C:\\L\\Plans.txt").expect("item");
    w.view.select_only(Some(i));
    w.layout(fonts);
    let mut fb = Framebuffer::new(430, 300, 0);
    w.paint(&mut fb, theme, fonts);
    fb
}
/// A folder of `era` holding one file of each main kind its Windows knew, in `mode`: each with the
/// look's own picture and Type name (`file_kinds::decide`, as the folder program reads them), the
/// shortcut's arrow, a hidden file faded, one file selected.
fn kinds_scene(theme: &Theme, era: Era, fonts: &Fonts, mode: ViewMode) -> Framebuffer {
    use bevel_core::desktop_icons::{Item, Picture, local_filetime};
    use bevel_core::file_kinds::{self, Choice, Kind};
    use bevel_core::folder::{FolderWindow, art};
    use bevel_core::kit::Form;
    let mut names = vec![
        "Notes.txt", "Setup.ini", "Start.bat", "Tools.dll", "Program.exe", "Edit.com", "Help.hlp", "Picture.bmp", "Photo.jpg", "Chime.wav",
        "Tune.mid", "Movie.avi", "Page.htm", "Letter.doc", "Story.rtf", "Fonts.fon", "Settings.reg", "Site.url", "Hidden.sys",
    ];
    names.extend(match era {
        Era::Win95 => &["Font.ttf"][..],
        Era::Win98 => &["Book.chm", "Script.js", "Files.cab", "Desert.theme"][..],
        Era::Win2000 => &["Book.chm", "Script.js", "Files.cab", "Setup.log", "Install.msi", "Song.mp3"][..],
    });
    let items: Vec<Item> = names
        .iter()
        .enumerate()
        .map(|(n, name)| {
            let d = file_kinds::decide(era, name);
            let picture = match d.picture {
                Choice::Look(k) => Picture::Kind(k),
                Choice::OwnOrProgram => Picture::Kind(Kind::Program),
                // A made-up Internet Shortcut without a picture of its own: the kind's.
                Choice::OwnOrLook => Picture::Kind(Kind::Ending),
                Choice::Today => panic!("{name}: a kind {era:?} knew"),
            };
            Item {
                id: format!("C:\\Kinds\\{name}"),
                name: name.to_string(),
                picture,
                shortcut: name.ends_with(".url"),
                folder: false,
                type_name: d.type_name.expect("a known kind's name").to_string(),
                size: 1_000 + 37 * n as u64,
                modified: local_filetime(2026, 11, 23, 9, n as u64),
            }
        })
        .collect();
    let small = art::Small::Folder;
    let icon16 = art::small(era, small);
    let size = if mode == ViewMode::Details { (520, 430) } else { (560, 430) };
    let mut w = FolderWindow::new(era, "Kinds", icon16.clone(), true, size);
    w.layout(fonts);
    if era != Era::Win95 {
        w.show_place("Kinds", icon16, true, "C:\\Kinds", art::listed(era, small));
    }
    w.set_view_mode(mode);
    w.set_items(items);
    if mode == ViewMode::Details {
        let mut c = w.view.columns.clone();
        c[2] = 190;
        w.view.set_columns(&c);
    }
    w.view.set_hidden(["C:\\Kinds\\Hidden.sys".to_string()]);
    let i = w.view.index_of("C:\\Kinds\\Notes.txt").expect("item");
    w.view.select_only(Some(i));
    w.layout(fonts);
    let mut fb = Framebuffer::new(size.0, size.1, 0);
    w.paint(&mut fb, theme, fonts);
    fb
}

/// The web view's fonts from the fixtures (98: Verdana; 2000: the look's Tahoma and its 13pt bold).
fn web_fonts(era: Era, fonts: &Fonts) -> std::sync::Arc<bevel_core::folder::web::WebFonts> {
    use bevel_core::folder::web::WebFonts;
    let fixture = |name: &str| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        Font::parse(&std::fs::read_to_string(&path).unwrap()).unwrap().with_period_glyphs()
    };
    std::sync::Arc::new(match era {
        Era::Win2000 => WebFonts { body: fonts.regular.clone(), bold: fonts.bold.clone(), title: fixture("tahoma-17px-bold.font") },
        _ => WebFonts { body: fixture("verdana-11px.font"), bold: fixture("verdana-11px-bold.font"), title: fixture("verdana-21px-bold.font") },
    })
}

/// A 98 or 2000 folder window with its web view: a file selected, My Computer with a drive
/// selected, the Windows folder's warning page.
fn web_scene(theme: &Theme, era: Era, fonts: &Fonts, which: &str) -> Framebuffer {
    use bevel_core::desktop_art::{self, Art};
    use bevel_core::desktop_icons::{Item, Picture, local_filetime};
    use bevel_core::folder::art::{self, DriveKind, Small};
    use bevel_core::folder::web::{Guard, Kind};
    use bevel_core::folder::{FolderWindow, Space};
    use bevel_core::kit::Form;
    let doc = icon(&DOCUMENT, 2);
    let size = (560, 360);
    let (title, small, files) = match which {
        "folder-web-drive" => ("My Computer", Small::MyComputer, false),
        "folder-web-warning" => ("Windows", Small::Folder, true),
        _ => ("Letters", Small::Folder, true),
    };
    let icon16 = art::small(era, small);
    let mut w = FolderWindow::new(era, title, icon16.clone(), files, size);
    w.layout(fonts);
    w.show_place(title, icon16, files, title, art::listed(era, small));
    w.web.fonts = Some(web_fonts(era, fonts));
    let (kind, picture, guard) = match which {
        "folder-web-drive" => (Kind::MyComputer, Art::MyComputer, None),
        "folder-web-warning" => (Kind::Folder, Art::Folder, Some(Guard::System)),
        _ => (Kind::Folder, Art::Folder, None),
    };
    w.set_web_place(kind, desktop_art::picture(era, picture, false, 0), guard, false, None);
    w.set_web(true);
    let item = |name: &str, folder: bool, size: u64| Item {
        id: format!("C:\\Letters\\{name}"),
        name: name.trim_end_matches(".txt").to_string(),
        picture: if folder { Picture::Folder } else { Picture::Own(Some(doc.clone())) },
        shortcut: false,
        folder,
        type_name: if folder { "File Folder".into() } else { "Text Document".into() },
        size,
        modified: local_filetime(2026, 11, 15, 14, 39),
    };
    match which {
        "folder-web-drive" => {
            let drive = |id: &str, name: &str, kind: DriveKind| Item {
                id: id.into(),
                name: name.into(),
                picture: Picture::Art(Art::Drive(kind)),
                shortcut: false,
                folder: false,
                type_name: String::new(),
                size: 0,
                modified: 0,
            };
            w.spaces.insert("C:\\".into(), Space { free: 300 << 20, capacity: 1 << 30 });
            w.set_items(vec![drive("A:\\", "3\u{bd} Floppy (A:)", DriveKind::Floppy), drive("C:\\", if era == Era::Win2000 { "Local Disk (C:)" } else { " (C:)" }, DriveKind::Fixed)]);
            w.view.select_only(Some(1));
        }
        "folder-web-warning" => w.set_items(vec![item("System", true, 0), item("Notes.txt", false, 489)]),
        _ => {
            w.set_items(vec![item("Photos", true, 0), item("Notes.txt", false, 489), item("Plans.txt", false, 31_000)]);
            w.view.select_only(Some(2));
        }
    }
    w.view.keys_used = true;
    w.layout(fonts);
    let mut fb = Framebuffer::new(size.0, size.1, 0);
    w.paint(&mut fb, theme, fonts);
    fb
}
/// The owner's sheet of wallpapers (not a golden image, and not public), laid out as the survey's
/// `backgrounds.png`: per look the Background page as it opens beside three 800x600 desktops (a
/// tiled wallpaper, a pattern, a centered picture: the survey's own choices), Bevel95's own icons,
/// the bar, every picture at true size, labels outside the pictures; drawn by the real code. Written
/// to the file named by BEVEL95_SHEET: `BEVEL95_SHEET=<file> cargo test -p bevel-core --test golden
/// -- --ignored --exact draw_display_sheet`.
#[test]
#[ignore = "writes the owner's sheet of wallpapers to BEVEL95_SHEET; run it on request"]
fn draw_display_sheet() {
    use bevel_core::desktop_icons::Desktop;
    use bevel_core::display::form::DisplayProperties;
    use bevel_core::display::{self, Choice, Show};
    let Ok(path) = std::env::var("BEVEL95_SHEET") else {
        panic!("BEVEL95_SHEET names the file to write");
    };
    let (sw, sh, gap, label) = (800, 600, 16, 20);
    let label_font = load_font(&WIN95, false);
    let bold = load_font(&WIN95, true);
    let width = gap + 404 + 3 * (gap + sw) + gap;
    let height = 3 * (label + sh + gap) + 20;
    let mut sheet = Framebuffer::new(width, height, rgb(0xFFFFFF));
    let c = |w: Option<&str>, s: Show, p: Option<&str>| Choice { wallpaper: w.map(str::to_string), show: s, pattern: p.map(str::to_string) };
    for (row, era) in Era::ALL.into_iter().enumerate() {
        let theme = bevel_core::theme::theme_in(era, Depth::TrueColor);
        let (regular, boldf) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &boldf };
        let y = 4 + row as i32 * (label + sh + gap);
        let look = era.title();
        bold.draw(&mut sheet, gap, y, &format!("{look}: Background page as it opens"), rgb(0x000000));
        let p = DisplayProperties::new(era, Choice::plain());
        let mut window = Framebuffer::new(404, 448, 0);
        p.paint(&mut window, theme, &fonts);
        sheet.blit(&window, gap, y + label);
        let desktops: [(String, Choice); 3] = match era {
            Era::Win95 => [
                ("Bubbles, Tile".into(), c(Some("Bubbles"), Show::Tile, None)),
                ("Pattern Plaid, no wallpaper".into(), c(None, Show::Center, Some("Plaid"))),
                ("Setup, Center (pattern Plaid around it)".into(), c(Some("Setup"), Show::Center, Some("Plaid"))),
            ],
            Era::Win98 => [
                ("Forest, Tile".into(), c(Some("Forest"), Show::Tile, None)),
                ("Pattern Bricks, no wallpaper".into(), c(None, Show::Center, Some("Bricks"))),
                ("Red Blocks, Center".into(), c(Some("Red Blocks"), Show::Center, None)),
            ],
            Era::Win2000 => [
                ("Zapotec, Tile".into(), c(Some("Zapotec"), Show::Tile, None)),
                ("Pattern Scottie, no wallpaper".into(), c(None, Show::Center, Some("Scottie"))),
                ("Zapotec, Center".into(), c(Some("Zapotec"), Show::Center, None)),
            ],
        };
        for (k, (name, choice)) in desktops.iter().enumerate() {
            let x = gap + 404 + gap + k as i32 * (sw + gap);
            label_font.draw(&mut sheet, x, y, &format!("{look}: {name}"), rgb(0x000000));
            let mut screen = display::background(theme, choice, sw, sh).expect("a wallpaper or a pattern");
            let mut bar = Framebuffer::new(sw, theme.bar.height, 0);
            paint::paint_bar(&mut bar, theme, &fonts, &Bar { clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(), ..Bar::default() });
            let area = Rect::new(0, 0, sw, sh - bar.height);
            let mut d = Desktop::new(era, area);
            d.set_background(Some(std::sync::Arc::new(screen.clone())));
            d.paint(&mut screen, theme, &fonts, area);
            screen.blit(&bar, 0, sh - bar.height);
            sheet.blit(&screen, x, y + label);
        }
    }
    label_font.draw(&mut sheet, gap, height - 16, "Every picture at true size (view at 100%). Drawn by Bevel95's own code, its own icons; lay it beside private\\probes\\display\\backgrounds.png.", rgb(0x808080));
    write_png(Path::new(&path), &sheet);
    println!("wrote the sheet");
}

/// The owner's sheet of a person's own picture (brief 73; not a golden image, not public): per
/// look the Background page with a test picture of the tests' own making chosen with Browse...,
/// beside 800x600 desktops showing it tiled, centered and (98, 2000) stretched, every picture at
/// true size, labels outside the pictures; drawn by the real code. `BEVEL95_SHEET=<file> cargo
/// test -p bevel-core --test golden -- --ignored --exact draw_own_picture_sheet`.
#[test]
#[ignore = "writes the owner's sheet of own pictures to BEVEL95_SHEET; run it on request"]
fn draw_own_picture_sheet() {
    use bevel_core::desktop_icons::Desktop;
    use bevel_core::display::form::DisplayProperties;
    use bevel_core::display::{self, Choice, PictureId, Show};
    let Ok(path) = std::env::var("BEVEL95_SHEET") else {
        panic!("BEVEL95_SHEET names the file to write");
    };
    let (sw, sh, gap, label) = (800, 600, 16, 20);
    let label_font = load_font(&WIN95, false);
    let bold = load_font(&WIN95, true);
    let width = gap + 404 + 3 * (gap + sw) + gap;
    let height = 3 * (label + sh + gap) + 20;
    let mut sheet = Framebuffer::new(width, height, rgb(0xFFFFFF));
    let file = "C:\\Users\\Someone\\Pictures\\Evening Sky.jpg";
    let photo = std::sync::Arc::new(test_photo(320, 200));
    for (row, era) in Era::ALL.into_iter().enumerate() {
        let theme = bevel_core::theme::theme_in(era, Depth::TrueColor);
        let (regular, boldf) = (load_font(theme, false), load_font(theme, true));
        let fonts = Fonts { regular: &regular, bold: &boldf };
        let y = 4 + row as i32 * (label + sh + gap);
        let look = era.title();
        bold.draw(&mut sheet, gap, y, &format!("{look}: Background page, \"Evening Sky\" chosen"), rgb(0x000000));
        let mut p = DisplayProperties::new(era, Choice::plain());
        p.set_shown(Choice { wallpaper: Some(file.into()), show: Show::Center, pattern: None });
        p.set_read(PictureId::Own(file.into()), Some(photo.clone()));
        p.focus = bevel_core::display::form::Part::List(bevel_core::display::form::Which::Wallpaper);
        let mut window = Framebuffer::new(404, 448, 0);
        p.paint(&mut window, theme, &fonts);
        sheet.blit(&window, gap, y + label);
        let ways: &[(&str, Show)] = if era == Era::Win95 {
            &[("tiled", Show::Tile), ("centered", Show::Center)]
        } else {
            &[("tiled", Show::Tile), ("centered", Show::Center), ("stretched", Show::Stretch)]
        };
        for (k, (name, show)) in ways.iter().enumerate() {
            let x = gap + 404 + gap + k as i32 * (sw + gap);
            label_font.draw(&mut sheet, x, y, &format!("{look}: the test picture (320x200) {name}"), rgb(0x000000));
            let choice = Choice { wallpaper: Some(file.into()), show: *show, pattern: None };
            let mut screen = display::background_on(theme, &choice, Some(&photo), &[], sw, sh).expect("the picture");
            let mut bar = Framebuffer::new(sw, theme.bar.height, 0);
            paint::paint_bar(&mut bar, theme, &fonts, &Bar { clock: CLOCK.to_string(), clock_widest: WIDEST.to_string(), ..Bar::default() });
            let area = Rect::new(0, 0, sw, sh - bar.height);
            let mut d = Desktop::new(era, area);
            d.set_background(Some(std::sync::Arc::new(screen.clone())));
            d.paint(&mut screen, theme, &fonts, area);
            screen.blit(&bar, 0, sh - bar.height);
            sheet.blit(&screen, x, y + label);
        }
    }
    label_font.draw(&mut sheet, gap, height - 16, "Every picture at true size (view at 100%), drawn by Bevel95's own code at the look's 1x pixels. The test picture is one the test draws itself (a gradient, a sun, a dark line every 9 pixels).", rgb(0x808080));
    write_png(Path::new(&path), &sheet);
    println!("wrote the sheet");
}
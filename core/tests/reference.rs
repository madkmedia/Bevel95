//! Opt-in comparison with real Windows 95 / 98 / 2000 screenshots, pixel for pixel.
//!
//! The screenshots are not part of this repository (they are Microsoft's pictures); see
//! "Reference screenshots" in docs/theme-spec.md for where each one comes from.
//! tools/fetch-refs.ps1 puts them in `private/reference/refs` (kept out of git), which `cargo compare-refs`
//! reads unless `BEVEL95_REFS` names another folder. For each case this draws
//! the same state with our code, compares the listed regions (app icons and other things we
//! cannot reproduce are masked out), prints how many pixels differ, and writes
//! `<folder>/out/<case>.png`: the screenshot, ours, and the differences in red, enlarged.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

// Use the same viewer-owned book that the game's Using Help command opens.
#[path = "../../programs/help/src/manual.rs"]
mod help_manual;
// Use the same monochrome font resources as the Help viewer, with positions measured
// independently in the guest. None of the article layout or painting code is called here.
#[allow(dead_code)] // other viewer fonts are not part of these article probes
#[path = "../src/html_help/fonts.rs"]
mod article_fonts;
// The fonts' way into bevel-art (Windows' pictures), as the core has it.
#[allow(dead_code, unused_imports)]
#[path = "../src/art.rs"]
mod art;

use bevel_core::dialog::{self, Dialog, DialogKind};
use bevel_core::fb::{Framebuffer, Icon, Rect};
use bevel_core::layout::{BarLayout, place_start_menu, place_submenu};
use bevel_core::paint::{self, Bar, EntryKind, Long, Menu, MenuEntry, MenuLayout, MenuStyle, Task, TaskState};
use bevel_core::text::{self, Font, Fonts, fixture_name};
use bevel_core::theme::{Era, SubmenuPlacement, Theme, WIN95, WIN98, WIN2000, rgb};
use bevel_core::tooltip;

fn load_font(theme: &Theme, bold: bool) -> Font {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(fixture_name(
        theme.font_face,
        theme.font_px,
        bold,
    ));
    Font::parse(&std::fs::read_to_string(&path).unwrap()).unwrap().with_period_glyphs()
}

/// Any 8-bit PNG (palette, gray, RGB, with or without alpha) as opaque pixels.
fn read_png(path: &Path) -> Framebuffer {
    let file = BufReader::new(File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let channels = match reader.output_color_type().0 {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => unreachable!("EXPAND turns palettes into RGB"),
    };
    let mut pixels = Vec::with_capacity((info.width * info.height) as usize);
    for row in buf[..info.line_size * info.height as usize].chunks(info.line_size) {
        for px in row[..info.width as usize * channels].chunks(channels) {
            let (r, g, b) = if channels < 3 { (px[0], px[0], px[0]) } else { (px[0], px[1], px[2]) };
            pixels.push(rgb(u32::from_be_bytes([0, r, g, b])));
        }
    }
    Framebuffer { width: info.width as i32, height: info.height as i32, pixels }
}

fn write_png(path: &Path, fb: &Framebuffer) {
    let file = File::create(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), fb.width as u32, fb.height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let data: Vec<u8> = fb.pixels.iter().flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8]).collect();
    writer.write_image_data(&data).unwrap();
}

/// A region of our picture compared with a region of the same size in the screenshot.
struct Region {
    ours: (i32, i32),
    reference: Rect,
    /// Screen rectangle containing independently authored Help prose. Non-ink background
    /// pixels inside it are still checked, including the textured Windows 98 welcome page.
    authored_text: Option<(Rect, HelpPaper)>,
    reflowed_scrollbar: Option<HelpScrollbar>,
    /// Source and destination of the Notes icon when authored lines before it rewrap.
    reflowed_note: Option<(Rect, Rect)>,
}

/// The authored article changed the guest thumb length or enabled state. Check the full
/// control against the color and geometry samples in the original enabled/disabled controls.
struct HelpScrollbar {
    at: (i32, i32),
    era: Era,
    thumb_height: i32,
}

#[derive(Clone, Copy)]
enum HelpPaper {
    Solid(u32),
    Welcome,
}

impl HelpPaper {
    fn contains(self, pixel: u32) -> bool {
        match self {
            Self::Solid(color) => pixel == rgb(color),
            Self::Welcome => {
                // This texture has an indexed palette. Where a glyph is erased by the
                // changed wording, the revealed color must belong to that same paper.
                const BACKDROP: &[u8] = include_bytes!("../../art/assets/html-help/welcome.palrle");
                let [_, r, g, b] = pixel.to_be_bytes();
                pixel == rgb(0xffffff) || BACKDROP[..768].as_chunks::<3>().0.contains(&[r, g, b])
            }
        }
    }
}

struct Case {
    name: &'static str,
    /// File name in the `BEVEL95_REFS` folder.
    file: &'static str,
    ours: Framebuffer,
    regions: Vec<Region>,
    /// Rectangles of the screenshot left out of the comparison (app icons, tray icons).
    masks: Vec<Rect>,
    /// The screenshot was taken with 16 colors: our colors are shown as the nearest of those.
    vga: bool,
}

/// Windows' 16 colors. A 16-color screen shows the 3D light color (0xDFDFDF) as light gray.
const VGA: [u32; 16] = [
    0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xC0C0C0, 0x808080, 0xFF0000, 0x00FF00,
    0xFFFF00, 0x0000FF, 0xFF00FF, 0x00FFFF, 0xFFFFFF,
];

fn nearest_vga(p: u32) -> u32 {
    let channel = |c: u32, shift: u32| ((c >> shift) & 0xFF) as i32;
    let distance = |v: u32| (0..3).map(|k| (channel(p, 8 * k) - channel(v, 8 * k)).pow(2)).sum::<i32>();
    rgb(*VGA.iter().min_by_key(|&&v| distance(v)).unwrap())
}

fn whole(fb: &Framebuffer, at: (i32, i32)) -> Region {
    region((0, 0), Rect::new(at.0, at.1, fb.width, fb.height))
}

/// A task button whose app has an icon (left out of the comparison, so it is invisible here).
fn task(title: &str, state: TaskState) -> Task {
    let invisible = Icon { width: 16, height: 16, pixels: vec![0; 256] };
    Task { title: title.to_string(), icon: Some(invisible), state }
}

/// The widest time of day in the screenshots' format (MS Sans Serif and Tahoma alike).
const WIDEST: &str = "12:00 AM";

/// A bar `width` wide with `tray` notification icons (invisible: they are masked).
#[allow(clippy::too_many_arguments)]
fn bar(
    theme: &Theme,
    fonts: &Fonts,
    width: i32,
    start_pressed: bool,
    tasks: Vec<Task>,
    tray: usize,
    clock: &str,
) -> (Framebuffer, BarLayout) {
    let mut fb = Framebuffer::new(width, theme.bar.height, 0);
    let invisible = Icon { width: 16, height: 16, pixels: vec![0; 256] };
    let state = Bar {
        start_pressed,
        tasks,
        tray: vec![Some(invisible); tray],
        clock: clock.to_string(),
        clock_widest: WIDEST.to_string(),
        ..Bar::default()
    };
    let layout = paint::paint_bar(&mut fb, theme, fonts, &state);
    (fb, layout)
}

/// Our picture's region at `ours` compared with the screenshot's `reference` rectangle.
fn region(ours: (i32, i32), reference: Rect) -> Region {
    Region { ours, reference, authored_text: None, reflowed_scrollbar: None, reflowed_note: None }
}

/// A menu from labels: "-" is a separator, a trailing ">" makes a submenu entry.
fn menu(labels: &[&str], hover: Option<usize>, show_keys: bool) -> Menu {
    let entries = labels
        .iter()
        .map(|&l| match l {
            "-" => MenuEntry::separator(),
            _ => match l.strip_suffix('>') {
                Some(l) => MenuEntry::submenu(l, None),
                None => MenuEntry::item(l, None),
            },
        })
        .collect();
    Menu { hover, show_keys, ..Menu::new(entries) }
}

/// A screen being put together: menus pasted onto it, and the icon cells (which hold real
/// apps' icons in the screenshots) collected as masks.
struct Screen {
    fb: Framebuffer,
    masks: Vec<Rect>,
}

impl Screen {
    fn new(w: i32, h: i32, color: u32) -> Screen {
        Screen { fb: Framebuffer::new(w, h, color), masks: Vec::new() }
    }

    /// Lays out and paints `menu`; returns its layout. Placed by `place(w, h)`.
    fn menu(
        &mut self,
        theme: &Theme,
        fonts: &Fonts,
        menu: &Menu,
        style: MenuStyle,
        place: impl FnOnce(i32, i32) -> (i32, i32),
    ) -> (Rect, MenuLayout) {
        let layout = paint::layout_menu(theme, fonts, menu, style, self.fb.height);
        let mut fb = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut fb, theme, fonts, menu, style, &layout);
        let (x, y) = place(layout.width, layout.height);
        self.fb.blit(&fb, x, y);
        if style != MenuStyle::Popup {
            let items = if style == MenuStyle::Start { &theme.start_menu.large } else { &theme.start_menu.small };
            for (e, r) in menu.entries.iter().zip(&layout.rects) {
                if e.kind != EntryKind::Separator {
                    let top = r.y + (r.h - items.icon) / 2;
                    self.masks.push(Rect::new(x + r.x + items.icon_x, y + top, items.icon, items.icon));
                }
            }
        }
        (Rect::new(x, y, layout.width, layout.height), layout)
    }
}

/// The screen rectangle of entry `i` of a menu at `menu_rect`.
fn entry(menu_rect: Rect, layout: &MenuLayout, i: usize) -> Rect {
    let r = layout.rects[i];
    Rect::new(menu_rect.x + r.x, menu_rect.y + r.y, r.w, r.h)
}

fn scaled(rule: SubmenuPlacement) -> SubmenuPlacement {
    rule
}

/// The pressed Start button of `theme` (the Start menu is open), on its own.
fn pressed_start(theme: &Theme, fonts: &Fonts) -> (Framebuffer, Rect) {
    let (bar_fb, layout) = bar(theme, fonts, 200, true, vec![], 0, "");
    let b = layout.start;
    let mut fb = Framebuffer::new(b.w, b.h, 0);
    for y in 0..b.h {
        for x in 0..b.w {
            fb.set(x, y, bar_fb.get(b.x + x, b.y + y));
        }
    }
    (fb, b)
}

/// One screenshot case built on a `Screen`: regions are screen rectangles compared with the
/// screenshot at `offset` (where the screenshot's top-left corner is on the screen).
fn screen_case(name: &'static str, file: &'static str, screen: Screen, offset: (i32, i32), regions: &[Rect]) -> Case {
    let regions = regions
        .iter()
        .map(|r| region((r.x, r.y), Rect::new(r.x - offset.0, r.y - offset.1, r.w, r.h)))
        .collect();
    let masks = screen.masks.iter().map(|m| Rect::new(m.x - offset.0, m.y - offset.1, m.w, m.h)).collect();
    Case { name, file, ours: screen.fb, regions, masks, vga: false }
}

fn start_menu_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    let white = rgb(0xFFFFFF);

    // Windows 95: the Start menu alone, then with Programs > Accessories > Multimedia open.
    // GUIdebook cut-outs of a 640Ã—480 screen; the Start button sits at (2, 456).
    let start95 = ["&Programs>", "&Documents>", "&Settings>", "&Find>", "&Help", "&Run...", "-", "Sh&ut Down..."];
    let (button_fb, b) = pressed_start(&WIN95, f95);
    let button = Rect::new(b.x, 452 + b.y, b.w, b.h);
    let screen_rect = Rect::new(0, 0, 640, 480);
    let rule = scaled(WIN95.start_menu.submenu);
    for (name, file, cascade) in [
        ("win95-start-menu", "win95-rtm-startmenu-nohighlight-cutout-164x261.png", false),
        ("win95-start-cascade", "win95-rtm-startmenu-programs-accessories-multimedia-cutout-624x292.png", true),
    ] {
        let mut s = Screen::new(640, 480, white);
        s.fb.blit(&button_fb, button.x, button.y);
        let start = menu(&start95, cascade.then_some(0), true);
        let (start_rect, start_layout) =
            s.menu(&WIN95, f95, &start, MenuStyle::Start, |w, h| place_start_menu(button, w, h, screen_rect));
        let offset = if cascade { (2, 188) } else { (2, 217) };
        let mut regions = vec![start_rect, button];
        if cascade {
            let programs = menu(
                &[
                    "Accessories>",
                    "StartUp>",
                    "Microsoft Exchange",
                    "MS-DOS Prompt",
                    "The Microsoft Network",
                    "Windows Explorer",
                ],
                Some(0),
                true,
            );
            let at = entry(start_rect, &start_layout, 0);
            let (p_rect, p_layout) = s.menu(&WIN95, f95, &programs, MenuStyle::Cascade, |w, h| {
                place_submenu(start_rect, at, w, h, 3, rule, screen_rect)
            });
            let accessories = menu(
                &[
                    "Fax>",
                    "Games>",
                    "Multimedia>",
                    "System Tools>",
                    "Calculator",
                    "Dial-Up Networking",
                    "Direct Cable Connection",
                    "HyperTerminal",
                    "Notepad",
                    "Online Registration",
                    "Paint",
                    "Phone Dialer",
                    "WordPad",
                ],
                Some(2),
                true,
            );
            let at = entry(p_rect, &p_layout, 0);
            let (a_rect, a_layout) = s.menu(&WIN95, f95, &accessories, MenuStyle::Cascade, |w, h| {
                place_submenu(p_rect, at, w, h, 3, rule, screen_rect)
            });
            let multimedia = menu(&["Media Player", "Sound Recorder", "Volume Control"], None, true);
            let at = entry(a_rect, &a_layout, 2);
            let (m_rect, _) = s.menu(&WIN95, f95, &multimedia, MenuStyle::Cascade, |w, h| {
                place_submenu(a_rect, at, w, h, 3, rule, screen_rect)
            });
            regions.extend([p_rect, a_rect, m_rect]);
        }
        let file_name = file;
        out.push(screen_case(name, file_name, s, offset, &regions));
    }

    // Windows 2000 on an 800Ã—600 screen; the Start button sits at (2, 576). First the Start
    // menu with Programs > Accessories open (GUIdebook cut-out; no Log Off entry in this
    // one), then the full desktop with Windows Update under the pointer (a tooltip covers
    // part of the menu there; it is masked).
    let (button_fb, b) = pressed_start(&WIN2000, f2k);
    let button = Rect::new(b.x, 572 + b.y, b.w, b.h);
    let screen_rect = Rect::new(0, 0, 800, 600);
    let rule = scaled(WIN2000.start_menu.submenu);
    {
        let mut s = Screen::new(800, 600, white);
        s.fb.blit(&button_fb, button.x, button.y);
        let start = menu(
            &[
                "Windows Update",
                "-",
                "&Programs>",
                "&Documents>",
                "&Settings>",
                "Sear&ch>",
                "&Help",
                "&Run...",
                "-",
                "Sh&ut Down...",
            ],
            Some(2),
            false,
        );
        let (start_rect, start_layout) =
            s.menu(&WIN2000, f2k, &start, MenuStyle::Start, |w, h| place_start_menu(button, w, h, screen_rect));
        let programs = menu(&["Accessories>", "Startup>", "Internet Explorer", "Outlook Express"], Some(0), false);
        let at = entry(start_rect, &start_layout, 2);
        let (p_rect, p_layout) = s.menu(&WIN2000, f2k, &programs, MenuStyle::Cascade, |w, h| {
            place_submenu(start_rect, at, w, h, 3, rule, screen_rect)
        });
        let accessories = menu(
            &[
                "Accessibility>",
                "Communications>",
                "Entertainment>",
                "Games>",
                "System Tools>",
                "Address Book",
                "Calculator",
                "Command Prompt",
                "Imaging",
                "Notepad",
                "Paint",
                "Synchronize",
                "Windows Explorer",
                "WordPad",
            ],
            Some(8),
            false,
        );
        let at = entry(p_rect, &p_layout, 0);
        let (a_rect, _) = s.menu(&WIN2000, f2k, &accessories, MenuStyle::Cascade, |w, h| {
            place_submenu(p_rect, at, w, h, 3, rule, screen_rect)
        });
        let regions = [start_rect, button, p_rect, a_rect];
        out.push(screen_case(
            "win2000-start-cascade",
            "win2000pro-startmenu-programs-accessories-cutout-441x302.png",
            s,
            (2, 297),
            &regions,
        ));
    }
    {
        let mut s = Screen::new(800, 600, white);
        s.fb.blit(&button_fb, button.x, button.y);
        let start = menu(
            &[
                "Windows Update",
                "-",
                "&Programs>",
                "&Documents>",
                "&Settings>",
                "Sear&ch>",
                "&Help",
                "&Run...",
                "-",
                "&Log Off Admin...",
                "Sh&ut Down...",
            ],
            Some(0),
            false,
        );
        let (start_rect, _) =
            s.menu(&WIN2000, f2k, &start, MenuStyle::Start, |w, h| place_start_menu(button, w, h, screen_rect));
        s.masks.push(Rect::new(158, 311, 642, 30)); // the Windows Update tooltip
        out.push(screen_case(
            "win2000-start-menu",
            "win2000pro-desktop-startmenu-open-taskbar-800x600.png",
            s,
            (0, 0),
            &[start_rect, button],
        ));
    }
}

/// Windows 95's own menus (Toasty Tech screenshots with their colors restored): the
/// taskbar's right-click menu, and the desktop's with its New submenu open (only that
/// submenu's first two entries are ordinary menu items; the rest are drawn by Explorer).
fn popup_cases(out: &mut Vec<Case>, f95: &Fonts) {
    let screen_rect = Rect::new(0, 0, 640, 480);
    let mut s = Screen::new(640, 480, rgb(0xFFFFFF));
    let taskbar_menu = menu(
        &[
            "&Cascade",
            "Tile &Horizontally",
            "Tile &Vertically",
            "-",
            "&Minimize All Windows",
            "&Undo Tile",
            "-",
            "P&roperties",
        ],
        Some(1),
        true,
    );
    let (r, _) = s.menu(&WIN95, f95, &taskbar_menu, MenuStyle::Popup, |_, _| (424, 354));
    s.masks.push(Rect::new(534, 383, 11, 19)); // the mouse pointer
    out.push(screen_case("win95-taskbar-menu", "win95-taskbar-contextmenu-640x480-fixedcolors.png", s, (0, 0), &[r]));

    // The Start button's right-click menu: a bold default entry, whose access-key underline is
    // one pixel longer (GDI's overhang for the bold it makes up from MS Sans Serif).
    let mut s = Screen::new(640, 480, rgb(0xFFFFFF));
    let mut start_button_menu = menu(&["&Open", "&Explore", "&Find..."], Some(0), true);
    start_button_menu.entries[0].bold = true;
    let (r, _) = s.menu(&WIN95, f95, &start_button_menu, MenuStyle::Popup, |_, _| (21, 423));
    s.masks.push(Rect::new(82, 432, 14, 22)); // the mouse pointer
    out.push(screen_case(
        "win95-start-button-menu",
        "win95-startbutton-contextmenu-640x480-fixedcolors.png",
        s,
        (0, 0),
        &[r],
    ));

    let mut s = Screen::new(640, 480, rgb(0xFFFFFF));
    let desktop_menu = menu(
        &[
            "Arrange &Icons>",
            "Lin&e up Icons",
            "-",
            "&Paste",
            "Paste &Shortcut",
            "&Undo Rename",
            "-",
            "Ne&w>",
            "-",
            "P&roperties",
        ],
        Some(7),
        true,
    );
    let (parent, layout) = s.menu(&WIN95, f95, &desktop_menu, MenuStyle::Popup, |_, _| (257, 273));
    // Explorer draws the New submenu's last four entries itself (16 rows each, and wider),
    // so the submenu is placed for its real size (178Ã—113; that placement is also checked by
    // layout.rs's tests) and compared down to its separator only.
    let new = menu(&["&Folder", "&Shortcut", "-", "Text Document"], None, true);
    let at = entry(parent, &layout, 7);
    let real = place_submenu(parent, at, 178, 113, 3, WIN95.popup.submenu, screen_rect);
    let (sub, sub_layout) = s.menu(&WIN95, f95, &new, MenuStyle::Popup, |_, _| real);
    let top = Rect::new(sub.x, sub.y, 110, sub_layout.rects[3].y);
    s.masks.push(Rect::new(sub.x, sub.y + top.h, 6, 113 - top.h)); // beside Explorer's entries
    out.push(screen_case(
        "win95-desktop-menu",
        "win95-desktop-contextmenu-new-submenu-640x480-fixedcolors.png",
        s,
        (0, 0),
        &[parent, top],
    ));
}

#[path = "views_cases/mod.rs"]
mod views_cases;
#[path = "fileicons_cases/mod.rs"]
mod fileicons_cases;
mod web_cases;
#[path = "sixteen_cases/mod.rs"]
mod sixteen_cases;
#[path = "twofiftysix_cases/mod.rs"]
mod twofiftysix_cases;

fn cases() -> Vec<Case> {
    let (r95, b95) = (load_font(&WIN95, false), load_font(&WIN95, true));
    let f95 = Fonts { regular: &r95, bold: &b95 };
    let (r2k, b2k) = (load_font(&WIN2000, false), load_font(&WIN2000, true));
    let f2k = Fonts { regular: &r2k, bold: &b2k };
    let mut out = Vec::new();

    // Windows 95 taskbar: Control Panel active, two more apps, the clock. Icons masked.
    let (ours, _) = bar(
        &WIN95,
        &f95,
        640,
        false,
        vec![
            task("Control Panel", TaskState::Active),
            task("Dial-Up Networking", TaskState::Normal),
            task("Calculator", TaskState::Normal),
        ],
        0,
        "1:49 PM",
    );
    out.push(Case {
        name: "win95-taskbar",
        file: "win95-rtm-taskbar-3tasks-1active-clock-640x28.png",
        regions: vec![whole(&ours, (0, 0))],
        ours,
        masks: vec![Rect::new(64, 8, 16, 16), Rect::new(227, 7, 16, 16), Rect::new(390, 7, 16, 16)],
        vga: false,
    });

    // Windows 2000 taskbar. It has Quick Launch (not in v1) and a notification icon (the
    // volume's, masked), so the Start button, each task button and the notification area are
    // compared one by one.
    let (ours, layout) = bar(
        &WIN2000,
        &f2k,
        800,
        false,
        vec![
            task("Control Panel", TaskState::Active),
            task("Calculator", TaskState::Normal),
            // NetMeeting's window has no icon in this screenshot.
            Task { icon: None, ..task("NetMeeting", TaskState::Normal) },
        ],
        1,
        "1:11 PM",
    );
    let full = |x: i32, w: i32| Rect::new(x, 0, w, 28);
    out.push(Case {
        name: "win2000-taskbar",
        file: "win2000pro-taskbar-quicklaunch-3tasks-1active-tray-800x28.png",
        regions: vec![
            region((0, 0), full(0, 60)),
            region((layout.tasks[0].x, 0), full(155, 160)),
            region((layout.tasks[1].x, 0), full(318, 160)),
            region((layout.tasks[2].x, 0), full(481, 160)),
            region((layout.well.x - 2, 0), full(702, 98)),
        ],
        ours,
        masks: vec![Rect::new(159, 8, 16, 16), Rect::new(322, 7, 16, 16), Rect::new(708, 7, 16, 16)],
        vga: false,
    });
    start_menu_cases(&mut out, &f95, &f2k);
    quick_launch_cases(&mut out, &f95, &f2k);
    quick_launch_size_cases(&mut out, &f95, &f2k);
    popup_cases(&mut out, &f95);
    dialog_cases(&mut out, &f95, &f2k);
    run_list_cases(&mut out, &f95, &f2k);
    win98_cases(&mut out, &f95);
    properties_cases(&mut out, &f95, &f2k);
    small_start_case(&mut out, &f95);
    small_start_case_2000(&mut out, &f2k);
    own_start_cases(&mut out, &f95, &f2k);
    long_menu_cases(&mut out, &f95, &f2k);
    field_menu_cases(&mut out, &f95, &f2k);
    own_properties_cases(&mut out, &f95, &f2k);
    tray_cases(&mut out, &f95, &f2k);
    desktop_cases(&mut out);
    desktop_icon_cases(&mut out, &f95, &f2k);
    wallpaper_desktop_cases(&mut out, &f95, &f2k);
    display_window_cases(&mut out, &f95, &f2k);
    desktop_menu_cases(&mut out, &f95, &f2k);
    desktop_move_cases(&mut out, &f95, &f2k);
    desktop_item_cases(&mut out, &f95, &f2k);
    remove_window_cases(&mut out, &f95, &f2k);
    tree_cases(&mut out);
    task_icon_cases(&mut out, &f95, &f2k);
    submenu_cases(&mut out, &f95, &f2k);
    documents_cases(&mut out, &f95, &f2k);
    datetime_cases(&mut out, &f95, &f2k);
    program_window_cases(&mut out, &f95, &f2k);
    program_menu_cases(&mut out, &f95, &f2k);
    minesweeper_cases(&mut out, &f95, &f2k);
    calculator_cases(&mut out, &f95, &f2k);
    calculator_scientific_cases(&mut out, &f95, &f2k);
    solitaire_cases(&mut out, &f95, &f2k);
    solitaire_keyboard_cases(&mut out, &f95);
    solitaire_card_cases(&mut out);
    html_help_cases(&mut out, &f95, &f2k);
    solitaire_help_cases(&mut out, &f95, &f2k);
    calculator_help_cases(&mut out, &f95, &f2k);
    html_help_command_cases(&mut out, &f95, &f2k);
    folder_window_cases(&mut out, &f95);
    folder_menu_cases(&mut out, &f95);
    explorer_window_cases(&mut out, &f95, &f2k);
    explorer_toolbar_cases(&mut out, &f95, &f2k);
    explorer_undo_cases(&mut out, &f2k);
    explorer_menu_cases(&mut out, &f95, &f2k);
    arrange_menu_cases(&mut out, &f95, &f2k);
    explorer_list_cases(&mut out, &f95, &f2k);
    explorer_tree_cases(&mut out, &f95, &f2k);
    folders_button_cases(&mut out, &f2k);
    tree_close_cases(&mut out, &f95, &f2k);
    disk_box_cases(&mut out, &f95, &f2k);
    views_cases::view_cases(&mut out, &f95, &f2k);
    fileicons_cases::fileicons_cases(&mut out, &f95, &f2k);
    web_cases::web_cases(&mut out, &f95, &f2k);
    sixteen_cases::sixteen_cases(&mut out, &f95, &f2k);
    twofiftysix_cases::twofiftysix_cases(&mut out, &f95, &f2k);
    out
}

/// Native CARDS.DLL drew the private reference sheets into a GDI bitmap against the
/// same background. Compare every pixel in each defined 71Ã—96 cell, including its corners.
fn solitaire_card_cases(out: &mut Vec<Case>) {
    use bevel_core::solitaire::art::{self, Slot};
    use bevel_core::solitaire::game::{Card, Suit};
    let only = std::env::var("BEVEL95_CASE").ok();
    for (name, file, first, count) in [
        ("solitaire-card-faces", "solitaire/original-face.png", 0, 52),
        ("solitaire-card-selection", "solitaire/original-selected.png", 0, 52),
        ("solitaire-card-backs", "solitaire/original-back.png", 54, 12),
        ("solitaire-card-foundation", "solitaire/original-slot.png", 0, 1),
        ("solitaire-card-recycle", "solitaire/original-recycle.png", 0, 1),
        ("solitaire-card-stop", "solitaire/original-stop.png", 0, 1),
    ] {
        if only.as_deref().is_some_and(|filter| !name.contains(filter)) {
            continue;
        }
        let mut fb = Framebuffer::new(923, 576, rgb(0x132547));
        let mut regions = Vec::new();
        for i in first..first + count {
            let (x, y) = (71 * (i % 13), 96 * (i / 13));
            match name {
                "solitaire-card-faces" | "solitaire-card-selection" => {
                    let card = Card { suit: Suit::ALL[(i % 4) as usize], rank: (i / 4 + 1) as u8 };
                    art::face(&mut fb, x, y, Era::Win2000, card, name == "solitaire-card-selection");
                }
                "solitaire-card-backs" => art::back(&mut fb, x, y, Era::Win2000, (i - 54) as usize),
                _ => {
                    let slot = match name {
                        "solitaire-card-foundation" => Slot::Empty,
                        "solitaire-card-recycle" => Slot::Recycle,
                        _ => Slot::Stop,
                    };
                    art::slot(&mut fb, x, y, Era::Win2000, slot);
                }
            }
            regions.push(region((x, y), Rect::new(x, y, 71, 96)));
        }
        out.push(Case { name, file, ours: fb, regions, masks: vec![], vga: false });
    }
    let name = "solitaire-card-animations";
    if only.as_deref().is_some_and(|filter| !name.contains(filter)) {
        return;
    }
    let mut fb = Framebuffer::new(4 * 71, 4 * 96, rgb(0x132547));
    let mut regions = Vec::new();
    // One independent native cdtAnimate sequence per animated back. The castle has two phases.
    for (row, (index, first_tick, phases)) in [(2, 0, 4), (9, 0, 2), (10, 196, 4), (11, 56, 4)].into_iter().enumerate()
    {
        for phase in 0..phases {
            let (x, y) = (phase * 71, row as i32 * 96);
            art::back(&mut fb, x, y, Era::Win2000, index);
            art::animate_back(&mut fb, x, y, index, first_tick + phase as u32);
            regions.push(region((x, y), Rect::new(x, y, 71, 96)));
        }
    }
    out.push(Case { name, file: "solitaire/original-animation.png", ours: fb, regions, masks: vec![], vga: false });
}

fn solitaire_deck(faces: [u8; 7], stock: &[u8]) -> [bevel_core::solitaire::game::Card; 52] {
    use bevel_core::solitaire::game::{Card, Suit};
    fn card(id: u8) -> Card {
        // CARDS.DLL's face bitmap numbers, as identified in the original.
        let suit = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades][usize::from((id - 1) / 13)];
        Card { suit, rank: 1 + (id - 1) % 13 }
    }
    let mut order = [0; 52];
    for (slot, id) in [0, 7, 13, 18, 22, 25, 27].into_iter().zip(faces) {
        order[slot] = id;
    }
    order[28..28 + stock.len()].copy_from_slice(stock);
    let known = order;
    let mut rest = (1..=52).filter(|id| !known.contains(id));
    for id in &mut order {
        if *id == 0 {
            *id = rest.next().unwrap();
        }
    }
    order.map(card)
}

/// Replay the visible cards of each captured random deal through the game's normal
/// deal and draw operations. The unobserved face-down cards may be filled in any order.
fn solitaire_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::kit::{Controller, Form};
    use bevel_core::solitaire::choices::Choices;
    use bevel_core::solitaire::form::SolitaireWindow;
    use bevel_core::solitaire::game::Game;
    use bevel_core::solitaire::menu::command_at;
    use bevel_core::solitaire::settings::Settings;

    macro_rules! captures {
        ($era:literal; $($state:literal),+) => { [$(
            ($state, concat!($era, "-solitaire-", $state), concat!("vm/", $era, "-truecolor-sol-", $state, ".png"))
        ),+] };
    }
    let w95 = captures!("win95"; "active", "play", "menu-game", "menu-game-selected", "menu-game-play", "menu-help", "options", "deck", "about", "deal-again-native-helper", "maximized");
    let w98 = captures!("win98"; "active", "active-repaint", "play", "play-repaint", "menu-game", "menu-game-selected", "menu-play", "menu-help", "options", "deck", "about", "deal-again-native-helper", "maximized");
    let w2k = captures!("win2000"; "active", "active-repaint", "play", "menu-game-fresh", "menu-game-selected", "menu-game-after-move", "menu-help", "options", "deck", "about-redacted", "deal-again", "maximized");
    let only = std::env::var("BEVEL95_CASE").ok();
    for (era, theme, fonts, at, faces, stock, back, shots) in [
        (Era::Win95, &WIN95, f95, (66, 66), [40, 27, 18, 17, 35, 30, 10], [6, 38, 51], 0, w95.as_slice()),
        (Era::Win98, &WIN98, f95, (0, 0), [7, 28, 41, 2, 49, 48, 27], [3, 14, 36], 4, w98.as_slice()),
        (Era::Win2000, &WIN2000, f2k, (132, 132), [39, 7, 34, 12, 4, 43, 14], [0; 3], 2, w2k.as_slice()),
    ] {
        for &(state, name, file) in shots {
            if only.as_deref().is_some_and(|filter| !name.contains(filter)) {
                continue;
            }
            let modern = era == Era::Win2000;
            let mut settings = Settings { deck: back, ..Settings::default() };
            if state == "menu-game-fresh" {
                settings.rules.scoring = bevel_core::solitaire::game::Scoring::Vegas;
                settings.deck = 0;
            }
            let mut masks = Vec::new();
            let (fb, origin) = if state == "options" || state == "deck" {
                let (mut dialog, pos, size) = match (modern, state) {
                    (false, "deck") => (Choices::deck(era, &settings), (at.0 + 54, at.1 + 96), (351, 259)),
                    (false, _) => (Choices::options(era, &settings), (at.0 + 54, at.1 + 96), (342, 217)),
                    (true, "deck") => (Choices::deck(era, &settings), (153, 194), (273, 207)),
                    (true, _) => (Choices::options(era, &settings), (211, 224), (207, 189)),
                };
                dialog.show_keys();
                dialog.layout(fonts);
                dialog.set_screen(pos.0, pos.1);
                assert_eq!(dialog.size(), size, "{name} dialog dimensions");
                let mut fb = Framebuffer::new(size.0, size.1, 0);
                dialog.paint(&mut fb, theme, fonts);
                (fb, pos)
            } else if matches!(state, "about" | "about-redacted" | "deal-again" | "deal-again-native-helper") {
                use bevel_core::solitaire::dialogs::Sheet;
                let (mut sheet, pos, size) = if state == "about-redacted" {
                    // The reference removes the two registration lines and host memory value.
                    masks.extend([Rect::new(274, 415, 298, 30), Rect::new(472, 459, 100, 17)]);
                    (Sheet::about(era, Some(261_684)), (166, 207), (419, 329))
                } else if state == "about" {
                    let pos = (at.0 + 36, at.1 + 77);
                    let kib = if era == Era::Win95 { 64_924 } else { 130_456 };
                    // The guest registration and 9x resource-heap readout vary by machine.
                    masks.push(Rect::new(pos.0 + 81, pos.1 + 139, 55, 11));
                    masks.push(Rect::new(pos.0 + 280, pos.1 + 214, 43, 9));
                    (Sheet::about(era, Some(kib)), pos, (348, 283))
                } else {
                    // The 2000 game displayed this at the end of its victory animation.
                    // The 9x helper invokes the original MessageBox arguments in the guest.
                    (Sheet::deal_again(era), (311, 249), (185, 119))
                };
                sheet.show_keys();
                sheet.layout(fonts);
                sheet.set_screen(pos.0, pos.1);
                assert_eq!(sheet.size(), size, "{name} dialog dimensions");
                let mut fb = Framebuffer::new(size.0, size.1, 0);
                sheet.paint(&mut fb, theme, fonts);
                (fb, pos)
            } else {
                let maximized = state == "maximized";
                let at = if maximized || matches!(state, "menu-game-fresh" | "active-repaint") { (0, 0) } else { at };
                // Some captures followed Deal or a change to the display settings.
                let (faces, stock) = match (era, state) {
                    (Era::Win95, "menu-game-selected") => ([32, 42, 14, 31, 10, 27, 34], [0; 3]),
                    (Era::Win95, "menu-game-play") => ([32, 42, 14, 31, 10, 27, 34], [48, 29, 52]),
                    (Era::Win98, "maximized") => ([7, 46, 21, 43, 44, 12, 22], [42, 17, 45]),
                    (Era::Win98, "active-repaint" | "play-repaint") => ([47, 38, 48, 24, 28, 29, 23], [25, 34, 50]),
                    (Era::Win2000, "maximized") => ([31, 17, 7, 15, 23, 35, 12], [0; 3]),
                    (Era::Win2000, "active-repaint") => ([35, 13, 18, 26, 49, 17, 48], [0; 3]),
                    (Era::Win2000, "menu-game-fresh") => ([26, 44, 19, 1, 21, 10, 36], [0; 3]),
                    _ => (faces, stock),
                };
                let game =
                    Game::from_deck(solitaire_deck(faces, &stock), settings.rules).expect("complete recorded deal");
                let mut window = SolitaireWindow::new(era, game, usize::from(settings.deck), true, false);
                if maximized {
                    window.set_size((800, 572), true);
                }
                window.set_screen(at.0, at.1);
                window.layout(fonts);
                let drawn = !modern
                    && (matches!(state, "play" | "play-repaint" | "menu-game-play" | "menu-play")
                        || era == Era::Win98 && matches!(state, "menu-game" | "maximized"));
                if drawn {
                    let mut input = Controller::default();
                    input.mouse_down(&mut window, 55, 75);
                    input.mouse_up(&mut window, 55, 75);
                    if era == Era::Win98 && state == "menu-game" {
                        assert!(window.game.undo());
                    }
                }
                if modern {
                    if matches!(state, "play" | "menu-game-after-move") {
                        window.double_click(535, 190);
                        // The robot's narrowed eyes are frame 683 (even animation tick).
                        window.tick_back();
                        window.tick_back();
                    } else {
                        // The initial/maximized captures caught the robot's unmodified eyes.
                        for _ in 0..3 {
                            window.tick_back();
                        }
                    }
                }
                let seconds = match (era, state) {
                    (Era::Win95, "play") => 48,
                    (Era::Win95, "menu-game-play") => 65,
                    (Era::Win2000, "menu-game-after-move") => 2,
                    (Era::Win98, "menu-game") => 6,
                    (Era::Win98, "menu-play") => 5,
                    (Era::Win98, "maximized" | "play-repaint") => 4,
                    (Era::Win2000, "play") => 1,
                    _ => 0,
                };
                window.tick(seconds);
                // Minimizing and restoring repaints the felt. The 95 played capture
                // also came after a repaint, which clears the deal animation's last stroke.
                if matches!(state, "active-repaint" | "play-repaint") || (era == Era::Win95 && state == "play") {
                    window.repaint_board();
                }
                let menu = state.starts_with("menu-").then(|| {
                    let title = usize::from(state == "menu-help");
                    let row = if matches!(state, "menu-game-selected" | "menu-game-after-move") { 2 } else { 0 };
                    window.show_menu(Some(title));
                    window.reached_by_keyboard(true);
                    window.menu_status = Some(command_at(era, title, row).unwrap().status(era));
                    (title, row)
                });
                window.layout(fonts);
                let size = window.size();
                assert_eq!(
                    size,
                    if maximized {
                        (800, 572)
                    } else if modern {
                        (593, 430)
                    } else {
                        (597, 434)
                    },
                    "{name} window dimensions"
                );
                let mut fb = Framebuffer::new(size.0, size.1, 0);
                window.paint(&mut fb, theme, fonts);
                if let Some((title, row)) = menu {
                    let pos = window.title_rect(title).unwrap();
                    let mut menu = window.menu(title);
                    menu.hover = Some(row);
                    menu.show_keys = true;
                    let layout = paint::layout_menu(theme, fonts, &menu, MenuStyle::Popup, 600);
                    let mut popup = Framebuffer::new(layout.width, layout.height, 0);
                    paint::paint_menu(&mut popup, theme, fonts, &menu, MenuStyle::Popup, &layout);
                    fb.blit(&popup, pos.x, pos.bottom());
                }
                (fb, at)
            };
            let regions = vec![whole(&fb, origin)];
            out.push(Case { name, file, ours: fb, regions, masks, vga: false });
        }
    }
}

/// Moving the seven of spades onto the eight of diamonds with the keyboard in the resized
/// Windows 98 game. An arrow key positions the system pointer at the top of the chosen card.
fn solitaire_keyboard_cases(out: &mut Vec<Case>, fonts: &Fonts) {
    use bevel_core::kit::{Controller, Effect, Form, Key, Mods};
    use bevel_core::solitaire::form::{Part, SolitaireWindow};
    use bevel_core::solitaire::game::Game;

    let order = solitaire_deck([7, 46, 21, 43, 44, 12, 22], &[42, 17, 45, 32, 15, 25]);
    let game = Game::from_deck(order, Default::default()).unwrap();
    let mut window = SolitaireWindow::new(Era::Win98, game, 4, true, false);
    window.set_size((697, 468), false);
    window.layout(fonts);
    let mut input = Controller::default();
    // Two stock turns, then Tab through waste, foundations and tableau to the first column.
    for key in [Key::Space, Key::Space, Key::Tab, Key::Tab, Key::Tab] {
        input.key(&mut window, key, Mods::default());
    }
    let mut pointer = (0, 0);
    let only = std::env::var("BEVEL95_CASE").ok();
    for (name, file, key, seconds, expected) in [
        ("win98-solitaire-key-tableau", "vm/win98-truecolor-sol-key-tableau.png", Key::Right, 104, (158, 154)),
        ("win98-solitaire-key-tableau-held", "vm/win98-truecolor-sol-key-tableau-held.png", Key::Space, 105, (158, 169)),
        ("win98-solitaire-key-tableau-dest", "vm/win98-truecolor-sol-key-tableau-dest.png", Key::Right, 107, (252, 172)),
        ("win98-solitaire-key-tableau-drop", "vm/win98-truecolor-sol-key-tableau-drop.png", Key::Space, 108, (252, 172)),
    ] {
        window.tick(seconds - window.game.seconds());
        for effect in input.key(&mut window, key, Mods::default()) {
            if let Effect::Activate(Part::MovePointer(x, y)) = effect { pointer = (x, y); }
        }
        if only.as_deref().is_some_and(|filter| !name.contains(filter)) { continue; }
        assert_eq!(pointer, expected, "{name} system pointer position");
        let mut ours = Framebuffer::new(697, 468, 0);
        window.paint(&mut ours, &WIN98, fonts);
        let regions = vec![whole(&ours, (0, 0))];
        let masks = vec![Rect::new(pointer.0, pointer.1, 11, 19)];
        out.push(Case { name, file, ours, regions, masks, vga: false });
    }
}

/// Compare the complete viewer command dialogs, including the edit caret and keyboard cues.
fn html_help_command_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::html_help::commands::CommandDialog;
    use bevel_core::kit::Form;

    let only = std::env::var("BEVEL95_CASE").ok();
    let find = CommandDialog::find as fn(Era) -> CommandDialog;
    for (name, file, era, at, size, dialog) in [
        ("win98-minesweeper-help-command-find", "vm/win98-truecolor-mine-help-find.png", Era::Win98,
            (221, 240), (357, 119), find),
        ("win98-minesweeper-help-command-print", "vm/win98-truecolor-mine-help-print.png", Era::Win98,
            (165, 144), (294, 162), CommandDialog::print),
        ("win98-minesweeper-help-command-finished", "vm/win98-truecolor-mine-help-find-miss.png", Era::Win98,
            (287, 249), (234, 119), CommandDialog::finished),
        ("win2000-minesweeper-help-command-find", "vm/win2000-truecolor-mine-help-find.png", Era::Win2000,
            (221, 240), (357, 119), find),
        ("win2000-minesweeper-help-command-print", "vm/win2000-truecolor-mine-help-print-root.png", Era::Win2000,
            (352, 167), (305, 175), CommandDialog::print),
        ("win2000-minesweeper-help-command-finished", "vm/win2000-truecolor-mine-help-find-miss.png", Era::Win2000,
            (285, 249), (236, 119), CommandDialog::finished),
    ] {
        if only.as_deref().is_some_and(|filter| !name.contains(filter)) { continue; }
        let (theme, fonts) = if era == Era::Win98 { (&WIN98, f95) } else { (&WIN2000, f2k) };
        let mut form = dialog(era);
        form.layout(fonts);
        form.set_screen(at.0, at.1);
        assert_eq!(form.size(), size, "{name}");
        let mut ours = Framebuffer::new(size.0, size.1, 0);
        form.paint(&mut ours, theme, fonts);
        let regions = vec![whole(&ours, at)];
        out.push(Case { name, file, ours, regions, masks: vec![], vga: false });
    }
    // Open the ambiguous keyword from the original Index through the viewer's query field.
    for (name, file, at, keyboard) in [
        ("win2000-minesweeper-help-command-topics-found", "vm/win2000-truecolor-mine-help-topics-found.png",
            (195, 331), false),
        ("win2000-minesweeper-help-command-topics-found-keys", "vm/win2000-truecolor-mine-help-topics-found-keys.png",
            (119, 186), true),
    ] {
        if only.as_deref().is_some_and(|filter| !name.contains(filter)) { continue; }
        let book = bevel_core::minefield::html_help_catalog::catalog(Era::Win2000);
        let mut viewer = bevel_core::html_help::HtmlHelp::new(Era::Win2000, book, bevel_core::html_help::Tab::Index);
        viewer.paste("hints and tips");
        let mut found = bevel_core::html_help::topics::TopicsFound::new(
            Era::Win2000, viewer.title().to_owned(), viewer.keyword_topics(),
        );
        if keyboard { found.show_keys(); }
        found.layout(f2k);
        found.set_screen(at.0, at.1);
        assert_eq!(found.size(), (477, 236), "{name}");
        let mut ours = Framebuffer::new(477, 236, 0);
        found.paint(&mut ours, &WIN2000, f2k);
        let regions = vec![whole(&ours, at)];
        out.push(Case { name, file, ours, regions, masks: vec![], vga: false });
    }
}

/// Exercise the game form and its own menu entries against the screenshots of WINMINE.EXE.
/// The coordinates below are guest screen pixels, measured independently of the renderer.
/// Calculator's Standard view against the Reference PCs' True Color captures: whole windows
/// (the window where each PC first opened it), each menu open, About.
fn calculator_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::calculator::dialogs::About;
    use bevel_core::calculator::engine::Key as K;
    use bevel_core::calculator::form::{Command, CalculatorWindow, Part};
    use bevel_core::kit::{Form, Key, Mods};

    fn render(form: &mut impl Form, theme: &Theme, fonts: &Fonts) -> Framebuffer {
        form.layout(fonts);
        let (width, height) = form.size();
        let mut fb = Framebuffer::new(width, height, 0);
        form.paint(&mut fb, theme, fonts);
        fb
    }
    let only = std::env::var("BEVEL95_CASE").ok();
    // GDI's made-up bold MS Sans Serif is one pixel wider than its letters (its overhang),
    // as the program's own fonts have it.
    let bold95 = f95.bold.clone().with_overhang(1);
    let f95 = Fonts { regular: f95.regular, bold: &bold95 };
    const COMMON: &[&str] = &[
        "std-fresh", "std-negative", "std-longest", "std-longest-negative", "std-memory", "std-mc",
        "std-error-div0", "std-error-inv0", "std-error-sqrtneg", "std-held-7-corner",
        "std-held-7-dragged-off", "std-held-plus-corner", "std-after-click-7", "std-key-8-flash",
        "menu-edit", "menu-view", "about",
    ];
    for (era, theme, fonts, prefix, at, more) in [
        (Era::Win95, &WIN95, f95, "win95", (44, 44), &["menu-help"][..]),
        (Era::Win98, &WIN98, f95, "win98", (22, 22),
            &["std-held-7", "std-held-plus", "std-after-click-plus", "std-after-click-away"][..]),
        (Era::Win2000, &WIN2000, *f2k, "win2000", (132, 132),
            &["menu-help", "std-after-click-plus", "std-digit-grouping", "std-digit-grouping-decimal",
              "std-digit-grouping-longest", "std-digit-grouping-negative", "menu-view-grouping-on"][..]),
    ] {
        for &state in COMMON.iter().chain(more) {
            let name: &'static str = Box::leak(format!("{prefix}-calculator-{state}").into_boxed_str());
            if only.as_deref().is_some_and(|filter| !name.contains(filter)) {
                continue;
            }
            let file: &'static str = Box::leak(format!("vm/{prefix}-truecolor-calc-{state}.png").into_boxed_str());
            let mut w = CalculatorWindow::new(era);
            let mut masks = Vec::new();
            // What each capture's display shows before its own keys (95's held and clicked
            // captures caught a stray 6; 98's were taken after a 7).
            let before = match (era, state) {
                (Era::Win95, s) if s.starts_with("std-held") || s.starts_with("std-after") => "6",
                (Era::Win98, s) if s.starts_with("std-after") || s.starts_with("std-held-plus") => "7",
                _ => "",
            };
            w.paste_text(before);
            // These were taken after the memory capture: the memory still held a number.
            if ["std-negative", "std-longest", "std-error"].iter().any(|s| state.starts_with(s)) {
                w.paste_text("5:m:q");
            }
            let digits = if era == Era::Win95 { "1234567890123" } else { "12345678901234567890123456789012" };
            match state {
                "std-negative" => {
                    w.paste_text("123");
                    w.engine.press(K::Negate);
                }
                "std-longest" => w.paste_text(digits),
                "std-longest-negative" => {
                    w.paste_text(digits);
                    w.engine.press(K::Negate);
                }
                "std-memory" => w.paste_text("5:m"),
                "std-mc" => w.paste_text("5:m:c:q"),
                "std-error-div0" => w.paste_text("1/0="),
                "std-error-inv0" => w.paste_text("0r"),
                "std-error-sqrtneg" => {
                    w.paste_text("4");
                    w.engine.press(K::Negate);
                    w.paste_text("@");
                }
                "std-digit-grouping" => w.paste_text("1234567"),
                "std-digit-grouping-decimal" => w.paste_text("1234567.89"),
                "std-digit-grouping-longest" => w.paste_text(digits),
                "std-digit-grouping-negative" => {
                    w.paste_text("4096");
                    w.engine.press(K::Negate);
                }
                // Held with the mouse (7 is button 7, + button 22), let go off it, or clicked.
                s if s.starts_with("std-held") || s.starts_with("std-after") => {
                    let button = Part::Button(if s.contains("plus") { 22 } else { 7 });
                    w.press(Some(button), 0, 0);
                    if !s.ends_with("dragged-off") && !s.starts_with("std-after") {
                        w.set_pressed(Some(button));
                    }
                }
                // The key 8 caught while its button flashes (the digit comes after the flash).
                "std-key-8-flash" => {
                    w.press_key(K::Digit(8));
                }
                _ => {}
            }
            // The guest's mouse pointer, over the button it holds or has just clicked.
            let pointer = match (era, state) {
                (Era::Win95, "std-held-7-corner") => Some((143, 196)),
                (Era::Win95, "std-held-plus-corner") => Some((263, 298)),
                (Era::Win98, "std-held-7-corner") => Some((112, 161)),
                (Era::Win98, "std-held-plus-corner") => Some((229, 258)),
                (Era::Win98, "std-held-7" | "std-after-click-7") => Some((97, 150)),
                (Era::Win98, "std-held-plus" | "std-after-click-plus") => Some((213, 247)),
                _ => None,
            };
            masks.extend(pointer.map(|(x, y)| Rect::new(x, y, 11, 19)));
            match (era, state) {
                // Windows 2000's pointer is larger, with a shadow.
                (Era::Win2000, "std-held-7-corner") => masks.push(Rect::new(221, 272, 17, 25)),
                (Era::Win2000, "std-held-plus-corner") => masks.push(Rect::new(338, 370, 17, 25)),
                // Windows 95's flash was caught mid-repaint: the 8 button's caption not yet
                // drawn and its right edge not yet changed (98's and 2000's are whole).
                (Era::Win95, "std-key-8-flash") => masks.push(Rect::new(150, 171, 36, 28)),
                _ => {}
            }
            if state.starts_with("std-digit-grouping") || state == "menu-view-grouping-on" {
                w.command(Command::DigitGrouping);
            }
            let (ours, regions) = if state == "about" {
                let mut about = About::new(era, Some(match era {
                    Era::Win95 => 64_924,
                    Era::Win98 => 130_456,
                    Era::Win2000 => 261_684,
                }));
                // Opened from the keyboard (Alt+H, A): 2000 shows the focus rectangle.
                about.show_keys();
                let at = (at.0 + 33, at.1 + 74);
                // The registration fields belong to the guest and the 9x resource heap has no
                // counterpart on the host (as Minesweeper's About cases).
                if era == Era::Win2000 {
                    masks.push(Rect::new(at.0 + 108, at.1 + 209, 70, 30));
                } else {
                    masks.push(Rect::new(at.0 + 81, at.1 + 136, 65, 34));
                    masks.push(Rect::new(at.0 + 279, at.1 + 211, 48, 14));
                }
                let fb = render(&mut about, theme, &fonts);
                let region = whole(&fb, at);
                (fb, vec![region])
            } else if state.contains("menu-") {
                let index = match state {
                    "menu-edit" => 0,
                    "menu-view" | "menu-view-grouping-on" => 1,
                    _ => 2,
                };
                w.menu_key();
                for _ in 0..index {
                    w.key(Key::Right, Mods::default());
                }
                w.show_menu(Some(index));
                let main = render(&mut w, theme, &fonts);
                let title = w.title_rect(index).unwrap();
                let menu = w.menu(index);
                let layout = paint::layout_menu(theme, &fonts, &menu, MenuStyle::Popup, 600);
                let mut popup = Framebuffer::new(layout.width, layout.height, 0);
                paint::paint_menu(&mut popup, theme, &fonts, &menu, MenuStyle::Popup, &layout);
                let (x, y) = (title.x, title.bottom());
                let mut combined = Framebuffer::new(main.width.max(x + popup.width), main.height.max(y + popup.height), 0);
                combined.blit(&main, 0, 0);
                combined.blit(&popup, x, y);
                let menu_region = region((x, y), Rect::new(at.0 + x, at.1 + y, popup.width, popup.height));
                (combined, vec![whole(&main, at), menu_region])
            } else {
                let fb = render(&mut w, theme, &fonts);
                let region = whole(&fb, at);
                (fb, vec![region])
            };
            out.push(Case { name, file, ours, regions, masks, vga: false });
        }
    }
}

/// Calculator's Scientific view against the Reference PCs' captures: whole windows in each
/// state, each menu open, the Statistics Box over the Calculator.
fn calculator_scientific_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::calculator::engine::{Angle, Key as K, Radix, Word};
    use bevel_core::calculator::form::{CalculatorWindow, Command};
    use bevel_core::calculator::stats::{self, StatsBox};
    use bevel_core::kit::{Form, Key, Mods};

    fn render(form: &mut impl Form, theme: &Theme, fonts: &Fonts) -> Framebuffer {
        form.layout(fonts);
        let (width, height) = form.size();
        let mut fb = Framebuffer::new(width, height, 0);
        form.paint(&mut fb, theme, fonts);
        fb
    }
    let only = std::env::var("BEVEL95_CASE").ok();
    let bold95 = f95.bold.clone().with_overhang(1);
    let f95 = Fonts { regular: f95.regular, bold: &bold95 };
    const COMMON: &[&str] = &[
        "sci-fresh", "sci-hex-0", "sci-hex-100", "sci-oct-100", "sci-bin-100", "sci-dec-100", "sci-dec-radians",
        "sci-dec-grads", "sci-hex-neg1", "sci-hex-neg1-dword", "sci-hex-neg1-word", "sci-hex-neg1-byte", "sci-inv",
        "sci-hyp", "sci-inv-hyp", "sci-paren2", "sci-paren-limit", "sci-memory", "sci-error-div0", "sci-exp-entry",
        "sci-inv-sin-result", "sci-fe", "sci-menu-view", "sci-menu-view-hex", "sci-menu-help", "sci-sta",
        "sci-sta-data", "sci-sta-main",
    ];
    for (era, theme, fonts, prefix, at, more) in [
        (Era::Win95, &WIN95, f95, "win95", (44, 44), &["sci-menu-edit", "sci-1e308"][..]),
        (Era::Win98, &WIN98, f95, "win98", (22, 22), &["sci-menu-edit", "sci-hex-neg1-f12-nothing", "sci-xy-1e99999"][..]),
        (Era::Win2000, &WIN2000, *f2k, "win2000", (132, 132),
            &["sci-hex-neg1-qword", "sci-grouping", "sci-grouping-hex-neg1", "sci-grouping-bin-neg1"][..]),
    ] {
        for &state in COMMON.iter().chain(more) {
            let name: &'static str = Box::leak(format!("{prefix}-calculator-{state}").into_boxed_str());
            if only.as_deref().is_some_and(|filter| !name.contains(filter)) {
                continue;
            }
            let file: &'static str = Box::leak(format!("vm/{prefix}-truecolor-calc-{state}.png").into_boxed_str());
            let mut w = CalculatorWindow::new(era);
            w.command(Command::Scientific);
            let keys = |w: &mut CalculatorWindow, text: &str| w.paste_text(text);
            let press = |w: &mut CalculatorWindow, key: K| w.engine.press(key);
            match state {
                // The menus were caught in Hex with 0 on 98 (the View menu's Hex capture on all).
                "sci-hex-0" | "sci-menu-view-hex" => press(&mut w, K::Radix(Radix::Hex)),
                "sci-menu-edit" | "sci-menu-help" => press(&mut w, K::Radix(Radix::Hex)),
                "sci-hex-100" | "sci-oct-100" | "sci-bin-100" | "sci-dec-100" => {
                    keys(&mut w, "100");
                    let radix = match state {
                        "sci-oct-100" => Radix::Oct,
                        "sci-bin-100" => Radix::Bin,
                        "sci-dec-100" => Radix::Dec,
                        _ => Radix::Hex,
                    };
                    press(&mut w, K::Radix(radix));
                }
                // (taken after the Dec capture: 100 still shows)
                "sci-dec-radians" | "sci-dec-grads" => {
                    keys(&mut w, "100");
                    press(&mut w, K::Angle(if state == "sci-dec-radians" { Angle::Radians } else { Angle::Grads }));
                }
                s if s.starts_with("sci-hex-neg1") => {
                    keys(&mut w, "1");
                    press(&mut w, K::Negate);
                    press(&mut w, K::Radix(Radix::Hex));
                    match s {
                        "sci-hex-neg1-dword" => press(&mut w, K::Word(Word::Dword)),
                        "sci-hex-neg1-word" => press(&mut w, K::Word(Word::Word)),
                        "sci-hex-neg1-byte" => press(&mut w, K::Word(Word::Byte)),
                        // after Byte: the number was cut to FF
                        "sci-hex-neg1-qword" => {
                            press(&mut w, K::Word(Word::Byte));
                            press(&mut w, K::Word(Word::Qword));
                        }
                        // after Byte: F12 did nothing on 98
                        "sci-hex-neg1-f12-nothing" => {
                            press(&mut w, K::Word(Word::Byte));
                            press(&mut w, K::Word(Word::Qword));
                        }
                        _ => {}
                    }
                }
                "sci-inv" => press(&mut w, K::Inv),
                "sci-hyp" => press(&mut w, K::Hyp),
                "sci-inv-hyp" => {
                    press(&mut w, K::Inv);
                    press(&mut w, K::Hyp);
                }
                                // (taken while the Statistics Box was open: Ave, Sum, s and Dat awake)
                "sci-paren2" => {
                    w.press(Some(bevel_core::calculator::form::Part::Button(3)), 0, 0);
                    w.set_pressed(None);
                    w.engine.press(K::Sta);
                    keys(&mut w, "((");
                }
                "sci-paren-limit" => keys(&mut w, &"(".repeat(26)),
                "sci-memory" => keys(&mut w, "5:m"),
                "sci-error-div0" => keys(&mut w, "1/0="),
                "sci-exp-entry" => keys(&mut w, "1:e5"),
                "sci-inv-sin-result" => keys(&mut w, ".5is"),
                "sci-fe" => keys(&mut w, "123v"),
                "sci-1e308" => keys(&mut w, "10y308="),
                // (98: taken while the Statistics Box was open)
                "sci-xy-1e99999" => {
                    w.press(Some(bevel_core::calculator::form::Part::Button(3)), 0, 0);
                    w.set_pressed(None);
                    w.engine.press(K::Sta);
                    keys(&mut w, "10y99999=");
                }
                "sci-grouping" => {
                    w.command(Command::DigitGrouping);
                    keys(&mut w, "1234567");
                }
                "sci-grouping-hex-neg1" | "sci-grouping-bin-neg1" => {
                    w.command(Command::DigitGrouping);
                    keys(&mut w, "0-1=");
                    if state == "sci-grouping-bin-neg1" {
                        press(&mut w, K::Radix(Radix::Hex));
                        press(&mut w, K::Word(Word::Byte));
                        press(&mut w, K::Radix(Radix::Bin));
                    } else {
                        press(&mut w, K::Radix(Radix::Hex));
                    }
                }
                // Sta was clicked (98 draws Dat's corner from then on).
                "sci-sta-main" | "sci-sta-data" | "sci-sta" => {
                    w.press(Some(bevel_core::calculator::form::Part::Button(3)), 0, 0);
                    w.set_pressed(None);
                    w.engine.press(K::Sta);
                    if state != "sci-sta" {
                        keys(&mut w, "5\\7\\");
                    }
                }
                _ => {}
            }
            let (ours, regions) = if state.starts_with("sci-menu-") {
                let index = match state {
                    "sci-menu-edit" => 0,
                    "sci-menu-help" => 2,
                    _ => 1,
                };
                w.menu_key();
                for _ in 0..index {
                    w.key(Key::Right, Mods::default());
                }
                w.show_menu(Some(index));
                let main = render(&mut w, theme, &fonts);
                let title = w.title_rect(index).unwrap();
                let menu = w.menu(index);
                let layout = paint::layout_menu(theme, &fonts, &menu, MenuStyle::Popup, 600);
                let mut popup = Framebuffer::new(layout.width, layout.height, 0);
                paint::paint_menu(&mut popup, theme, &fonts, &menu, MenuStyle::Popup, &layout);
                let (x, y) = (title.x, title.bottom());
                let mut combined = Framebuffer::new(main.width.max(x + popup.width), main.height.max(y + popup.height), 0);
                combined.blit(&main, 0, 0);
                combined.blit(&popup, x, y);
                let menu_region = region((x, y), Rect::new(at.0 + x, at.1 + y, popup.width, popup.height));
                (combined, vec![whole(&main, at), menu_region])
            } else if state == "sci-sta" || state == "sci-sta-data" {
                // The box in front at its place, the Calculator behind it.
                w.set_active(false);
                let main = render(&mut w, theme, &fonts);
                let mut stats_box = StatsBox::new(era);
                stats_box.set_lines(w.engine.stats_lines());
                let boxed = render(&mut stats_box, theme, &fonts);
                let place = stats::PLACE;
                let origin = (at.0.min(place.0), at.1.min(place.1));
                let width = (at.0 + main.width).max(place.0 + boxed.width) - origin.0;
                let height = (at.1 + main.height).max(place.1 + boxed.height) - origin.1;
                let mut combined = Framebuffer::new(width, height, 0);
                combined.blit(&main, at.0 - origin.0, at.1 - origin.1);
                combined.blit(&boxed, place.0 - origin.0, place.1 - origin.1);
                let calc_region = region((at.0 - origin.0, at.1 - origin.1), Rect::new(at.0, at.1, main.width, main.height));
                let box_region =
                    region((place.0 - origin.0, place.1 - origin.1), Rect::new(place.0, place.1, boxed.width, boxed.height));
                (combined, vec![calc_region, box_region])
            } else {
                let fb = render(&mut w, theme, &fonts);
                let region = whole(&fb, at);
                (fb, vec![region])
            };
            out.push(Case { name, file, ours, regions, masks: Vec::new(), vga: false });
        }
    }
}

fn minesweeper_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::kit::{Button, Controller, Form, Key, Mods};
    use bevel_core::minefield::dialogs::Sheet;
    use bevel_core::minefield::form::{Command, MinefieldWindow};
    use bevel_core::minefield::game::{Field, Game, Level};
    use bevel_core::help::winhelp::{HelpTab, HelpTopics, Part as HelpPart};
    use bevel_core::minefield::html_help_catalog;
    use bevel_core::minefield::scores::Settings;

    fn render(form: &mut impl Form, theme: &Theme, fonts: &Fonts, at: (i32, i32)) -> Framebuffer {
        form.layout(fonts);
        form.set_screen(at.0, at.1);
        let (width, height) = form.size();
        let mut fb = Framebuffer::new(width, height, 0);
        form.paint(&mut fb, theme, fonts);
        fb
    }

    fn play(game: &mut MinefieldWindow, fonts: &Fonts, state: &str) -> Rect {
        // The 95 game flooded a patch. In the other captures only a single 1 is visible.
        let mines = if game.era == Era::Win95 {
            [(0, 0), (0, 1), (0, 2), (0, 3), (2, 3), (2, 5), (2, 6), (3, 1), (5, 1), (6, 3)]
        } else {
            [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (0, 1), (4, 3)]
        };
        let field = game.field();
        game.game = Game::with_mines(field.width, field.height, &mines);
        let (opened, marked, seconds, left_pointer, right_pointer) = match game.era {
            Era::Win95 => ((4, 4), (1, 2), [2, 5, 6], Rect::new(168, 199, 11, 19), Rect::new(123, 170, 9, 18)),
            Era::Win98 => ((3, 3), (5, 5), [1, 8, 8], Rect::new(147, 188, 11, 19), Rect::new(179, 220, 11, 19)),
            Era::Win2000 => ((4, 4), (2, 4), [1, 16, 17], Rect::new(168, 248, 17, 25), Rect::new(136, 248, 17, 25)),
        };
        game.layout(fonts);
        let center = |(x, y)| (23 + 16 * x, 104 + 16 * y);
        let (x, y) = center(opened);
        let mut mouse = Controller::default();
        mouse.mouse_down(game, x, y);
        if state != "cell-held" {
            mouse.mouse_up(game, x, y);
            let marks = match state { "flag" => 1, "question" => 2, _ => 0 };
            let (mx, my) = center(marked);
            for _ in 0..marks {
                game.button(Button::Right, true, mx, my);
                game.button(Button::Right, false, mx, my);
            }
            game.set_seconds(seconds[marks]);
        }
        if matches!(state, "flag" | "question") { right_pointer } else { left_pointer }
    }

    fn loss(era: Era, field: Field) -> Game {
        // Each bit x records a mine in column x of a captured row. Replay the clicks
        // to reproduce the visible flood fill, neighboring numbers, and exploded mine.
        let (rows, opened, hit, seconds): (&[u32], &[(usize, usize)], _, _) = match era {
            Era::Win95 => (&[
                0x12e0801, 0x340000d5, 0x2109008c, 0x1041081, 0x2040c91, 0x21010828, 0x10000281, 0x102009d0,
                0xe00440, 0x30480f, 0xa000920, 0x702805, 0x220080, 0x2c01222, 0x4012009, 0x91500c1,
            ], &[(13, 0), (11, 6)], (7, 6), 13),
            Era::Win98 => (&[8, 0, 0, 2, 4, 0x1b, 2, 0xa0], &[(5, 0)], (1, 6), 9),
            Era::Win2000 => (&[
                0xbab6807, 0x10102040, 0x24168280, 0x2922100, 0xa000, 0x1c88158, 0xc021000, 0x4888001,
                0x21565101, 0x28000010, 0x2058960, 0x10020, 0x10084002, 0x5228401, 0x80c4806, 0x8c05200,
            ], &[(2, 5), (8, 6), (14, 10), (12, 4), (12, 7), (16, 4)], (15, 10), 14),
        };
        let mines: Vec<_> = rows.iter().enumerate().flat_map(|(y, row)| {
            (0..field.width).filter(move |x| row & (1 << x) != 0).map(move |x| (x, y))
        }).collect();
        let mut game = Game::with_mines(field.width, field.height, &mines);
        for &(x, y) in opened {
            game.open(x, y);
        }
        game.open(hit.0, hit.1);
        game.stop_clock(seconds);
        game
    }

    macro_rules! captures {
        ($era:literal; $($state:literal),+) => {
            [$(($state, concat!($era, "-minesweeper-", $state), concat!("vm/", $era, "-truecolor-mine-", $state, ".png"))),+]
        };
    }
    macro_rules! states {
        ($era:literal) => {
            captures!($era; "active", "intermediate", "expert", "mono", "cell-held", "play-1", "flag", "question", "lost",
                            "menu-select-1", "menu-select-2", "menu-open-1", "menu-open-1-item5", "menu-open-2", "menu-hover-1",
                            "custom", "best-times", "new-best", "about", "help-topics", "help-index", "help-play", "help-strategy")
        };
    }
    let only = std::env::var("BEVEL95_CASE").ok();
    for (era, theme, fonts, shots) in [
        (Era::Win95, &WIN95, f95, states!("win95")),
        (Era::Win98, &WIN98, f95, states!("win98")),
        (Era::Win2000, &WIN2000, f2k, states!("win2000")),
    ] {
        let modern = era == Era::Win2000;
        for (state, name, file) in shots {
            if (!modern && state == "menu-hover-1")
                || (era != Era::Win95 && state.starts_with("help-"))
                || only.as_deref().is_some_and(|filter| !name.contains(filter))
            {
                continue;
            }
            let level = match state {
                "intermediate" => Level::Intermediate,
                "expert" => Level::Expert,
                "lost" if era != Era::Win98 => Level::Expert,
                _ => Level::Beginner,
            };
            let playing = matches!(state, "cell-held" | "play-1" | "flag" | "question");
            let (at, expected_size) = match (modern, level) {
                (true, Level::Intermediate) => ((80, 80), (282, 363)),
                (true, Level::Expert) => ((80, 80), (506, 363)),
                (true, _) => ((80, 80), (170, 251)),
                (false, Level::Intermediate) => ((76, 34), (286, 367)),
                (false, Level::Expert) => ((76, 32), (510, 367)),
                (false, _) if era == Era::Win98 && state == "mono" => ((76, 28), (160, 239)),
                (false, _) if era == Era::Win98 && (playing || state == "lost") => ((76, 30), (158, 239)),
                (false, _) => ((76, 36), (160, 239)),
            };
            let settings = Settings::default();
            let mut game = MinefieldWindow::new(era, Level::Beginner, settings.custom, true, 1);
            if era == Era::Win98 && (playing || state == "lost") {
                // The 98 recording returned to Beginner after trying the other difficulties.
                game.command(Command::Level(Level::Intermediate));
                game.command(Command::Level(Level::Beginner));
            } else if level != Level::Beginner {
                game.command(Command::Level(level));
            }
            if state == "mono" {
                game.command(Command::Color);
            }
            let mut masks = Vec::new();
            if era == Era::Win98 && level == Level::Expert {
                masks.push(Rect::new(400, 300, 10, 19)); // the emulated PC's mouse pointer
            }
            if playing {
                masks.push(play(&mut game, fonts, state));
            }
            if state == "lost" {
                game.game = loss(era, game.field());
                if era == Era::Win98 {
                    masks.push(Rect::new(115, 236, 11, 19));
                }
            }
            if state == "menu-hover-1" {
                game.layout(fonts);
                game.hover(34, 32);
                masks.push(Rect::new(116, 112, 17, 25));
            } else if state.starts_with("menu-") {
                game.menu_key();
                if state.ends_with("-2") {
                    game.key(Key::Right, Mods::default());
                }
            }
            let (ours, regions) = match state {
                "help-topics" | "help-index" | "help-play" | "help-strategy" => {
                    let tab = if state == "help-index" { HelpTab::Index } else { HelpTab::Contents };
                    let book = html_help_catalog::catalog(era);
                    let mut viewer = HelpTopics::new(era, book.clone(), tab);
                    let (at, size) = match state {
                        "help-play" | "help-strategy" => {
                            // On the 800Ã—600 guest the top is y=59 and the taskbar begins at
                            // y=572. WinHelp keeps the last five work-area rows clear.
                            viewer.set_article_bounds(281, 508);
                            if state == "help-strategy" {
                                viewer.key(Key::Down, Mods::default());
                            }
                            viewer.activate(HelpPart::Display);
                            ((510, 59), (281, if state == "help-play" { 362 } else { 343 }))
                        }
                        _ => {
                            viewer.blink(false);
                            ((200, 50), (434, 428))
                        }
                    };
                    let fb = render(&mut viewer, theme, fonts, at);
                    assert_eq!((fb.width, fb.height), size, "{name} window dimensions");
                    let mut area = whole(&fb, at);
                    if viewer.is_article() {
                        let topic = &book.topics[usize::from(state == "help-strategy")];
                        // Compare the entire captured window, including all visible paper. Use the
                        // application's actual prose to check glyphs at measured WinHelp positions.
                        let paper = rgb(0xffffe2);
                        let first_words = topic.paragraphs[1].text.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
                        for (font, text, left, top) in [
                            (fonts.bold, topic.paragraphs[0].text.as_str(), 22, 54),
                            (fonts.regular, first_words.as_str(), 34, 76),
                        ] {
                            let mut expected = Framebuffer::new(font.measure(text), font.height, paper);
                            font.draw(&mut expected, 0, 0, text, rgb(0));
                            for y in 0..expected.height {
                                for x in 0..expected.width {
                                    assert_eq!(fb.get(left + x, top + y), expected.get(x, y),
                                        "{name} authored text at {},{}", left + x, top + y);
                                }
                            }
                        }
                        // The paper is inset six pixels on each side. Each topic's first numbered
                        // step or bullet is indented a further 16 pixels and uses black ink.
                        let marker = Rect::new(22, 76, 9, 13);
                        assert!((marker.y..marker.bottom()).any(|y| (marker.x..marker.right()).any(|x| fb.get(x, y) == rgb(0))),
                            "{name} first list marker");
                        area.authored_text = Some((Rect::new(at.0 + 22, at.1 + 54, size.0 - 39, size.1 - 60),
                            HelpPaper::Solid(0xffffe2)));
                    }
                    (fb, vec![area])
                }
                "menu-open-1" | "menu-open-1-item5" | "menu-open-2" => {
                    let index = usize::from(state == "menu-open-2");
                    game.show_menu(Some(index));
                    let main = render(&mut game, theme, fonts, at);
                    let title = game.title_rect(index).unwrap();
                    let mut menu = game.menu(index);
                    if state == "menu-open-1-item5" {
                        menu.hover = Some(5);
                    }
                    let layout = paint::layout_menu(theme, fonts, &menu, MenuStyle::Popup, 600);
                    let mut popup = Framebuffer::new(layout.width, layout.height, 0);
                    paint::paint_menu(&mut popup, theme, fonts, &menu, MenuStyle::Popup, &layout);
                    let (x, y) = (title.x, title.bottom());
                    let mut combined = Framebuffer::new(main.width.max(x + popup.width), main.height.max(y + popup.height), 0);
                    combined.blit(&main, 0, 0);
                    combined.blit(&popup, x, y);
                    let regions = vec![whole(&main, at), region((x, y), Rect::new(at.0 + x, at.1 + y, popup.width, popup.height))];
                    (combined, regions)
                }
                "custom" | "best-times" | "new-best" | "about" => {
                    let mut sheet = match state {
                        "custom" => Sheet::custom(era, game.field(), (40, 24)),
                        "new-best" => Sheet::new_best(era, Level::Beginner, 1, "Anonymous"),
                        "about" => {
                            let physical_kib = match era {
                                Era::Win95 => 64_924,
                                Era::Win98 => 130_456,
                                Era::Win2000 => 261_684,
                            };
                            Sheet::about(era, Some(physical_kib))
                        }
                        _ => Sheet::best_times(era, &settings),
                    };
                    if state != "new-best" {
                        sheet.show_keys(); // Custom and Best Times were opened from the keyboard
                    } else if !modern {
                        sheet.blink(false);
                    }
                    let (at, size) = match (era, state) {
                        (Era::Win2000, "custom") => ((83, 121), (201, 163)),
                        (Era::Win2000, "new-best") => ((83, 167), (156, 169)),
                        (Era::Win2000, "about") => ((113, 154), (419, 329)),
                        (Era::Win2000, _) => ((83, 167), (261, 147)),
                        (_, "custom") => ((103, 103), (237, 136)),
                        (Era::Win98, "new-best") => ((103, 95), (228, 139)),
                        (_, "new-best") => ((103, 103), (228, 139)),
                        (Era::Win98, "about") => ((109, 102), (348, 283)),
                        (_, "about") => ((109, 110), (348, 283)),
                        _ => ((103, 103), (330, 179)),
                    };
                    if state == "about" {
                        // The registration fields belong to the guest and the 9x resource heap
                        // has no counterpart on the host. The memory value is tested directly.
                        if modern {
                            masks.push(Rect::new(221, 363, 70, 30));
                        } else {
                            masks.push(Rect::new(190, at.1 + 136, 65, 34));
                            masks.push(Rect::new(388, at.1 + 211, 48, 14));
                        }
                    }
                    let fb = render(&mut sheet, theme, fonts, at);
                    assert_eq!((fb.width, fb.height), size, "{name} dialog dimensions");
                    let region = whole(&fb, at);
                    (fb, vec![region])
                }
                _ => {
                    let fb = render(&mut game, theme, fonts, at);
                    assert_eq!((fb.width, fb.height), expected_size, "{name} window dimensions");
                    let region = whole(&fb, at);
                    (fb, vec![region])
                }
            };
            out.push(Case { name, file, ours, regions, masks, vga: false });
        }
    }
}

/// Probe the glyphs and their placement independently of the article renderer. On textured
/// welcome paper only black pixels belong to the glyph.
fn assert_help_words(fb: &Framebuffer, name: &str, font: &Font, words: &str, x: i32, y: i32) {
    let mut sample = Framebuffer::new(font.measure(words), font.height, rgb(0xffffff));
    font.draw(&mut sample, 0, 0, words, rgb(0));
    for row in 0..sample.height {
        for col in 0..sample.width {
            assert_eq!(
                fb.get(x + col, y + row) == rgb(0),
                sample.get(col, row) == rgb(0),
                "{name} text at {},{}",
                x + col,
                y + row
            );
        }
    }
}

/// Solitaire opens WinHelp's topic picker on 95 and an HTML Help book on 98 and 2000.
/// Compare the complete opening windows. The HTML books supply their own prose after
/// the original heading; its glyph metrics and the paper beneath it are tested separately.
fn solitaire_help_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::help::winhelp::{HelpTab, HelpTopics};
    use bevel_core::html_help::{HtmlHelp, Part, Tab, chrome::Hit};
    use bevel_core::kit::Form;
    use bevel_core::solitaire::help::catalog;

    let only = std::env::var("BEVEL95_CASE").ok();
    // The 95 books wrap their own wording. Measure the shipped topics with the guest's
    // width and work area, and compare the frame and shared paper at both actual heights.
    for (topic, name, file, original_height, authored_height) in [
        (0, "win95-solitaire-help-play", "vm/win95-truecolor-sol-help-play.png", 426, 377),
        (1, "win95-solitaire-help-scoring", "vm/win95-truecolor-sol-help-scoring.png", 218, 407),
    ] {
        if only.as_deref().is_some_and(|filter| !name.contains(filter)) { continue; }
        let book = catalog(Era::Win95);
        let paragraphs = &book.topics[topic].paragraphs;
        let mut viewer = HelpTopics::article(Era::Win95, book.clone(), topic).unwrap();
        let (x, y) = (510, 59);
        viewer.set_article_bounds(281, 508);
        viewer.set_screen(x, y);
        viewer.layout(f95);
        assert_eq!(viewer.size(), (281, authored_height), "{name} natural dimensions");
        assert!(viewer.rect(bevel_core::help::winhelp::Part::Thumb).is_none(), "{name} fits");
        let mut ours = Framebuffer::new(281, authored_height, 0);
        viewer.paint(&mut ours, &WIN95, f95);
        assert_help_words(&ours, name, f95.bold, &paragraphs[0].text, 22, 54);
        let prefix = paragraphs[1].text.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
        assert_help_words(&ours, name, f95.regular, &prefix, 34, 76);
        let shared = original_height.min(authored_height) - 6;
        let mut page = region((0, 0), Rect::new(x, y, 281, shared));
        page.authored_text = Some((Rect::new(x + 22, y + 54, 281 - 39, shared - 54), HelpPaper::Solid(0xffffe2)));
        let bottom = region((0, authored_height - 6), Rect::new(x, y + original_height - 6, 281, 6));
        out.push(Case { name, file, ours, regions: vec![page, bottom], masks: vec![], vga: false });
    }
    for (era, name, file, at, expected_size, theme, fonts) in [
        (Era::Win95, "win95-solitaire-help-topics", "vm/win95-truecolor-sol-help-topics-initial.png",
            (200, 50), (434, 428), &WIN95, f95),
        (Era::Win98, "win98-solitaire-help-topics", "vm/win98-truecolor-sol-help-topics.png",
            (50, 0), (525, 450), &WIN98, f95),
        (Era::Win2000, "win2000-solitaire-help-contents", "vm/win2000-truecolor-sol-help-contents.png",
            (230, 30), (550, 450), &WIN2000, f2k),
    ] {
        if only.as_deref().is_some_and(|filter| !name.contains(filter)) {
            continue;
        }
        let book = catalog(era);
        let mut ours = Framebuffer::new(expected_size.0, expected_size.1, 0);
        let mut area = whole(&ours, at);
        if era == Era::Win95 {
            let mut viewer = HelpTopics::new(era, book, HelpTab::Contents);
            viewer.set_screen(at.0, at.1);
            viewer.layout(fonts);
            assert_eq!(viewer.size(), expected_size, "{name} dimensions");
            viewer.paint(&mut ours, theme, fonts);
        } else {
            let welcome = era == Era::Win98;
            let home = book.topics[book.home].clone();
            let mut viewer = HtmlHelp::new(era, book, Tab::Contents);
            viewer.set_screen(at.0, at.1);
            viewer.layout(fonts);
            assert_eq!(viewer.size(), expected_size, "{name} dimensions");
            assert_eq!(viewer.print_text(), home.plain_text(), "{name} opening topic");
            viewer.paint(&mut ours, theme, fonts);

            let (pane, margin, y, heading_font, paper) = if welcome {
                (Rect::new(233, 65, 270, 379), 35, 93, article_fonts::welcome_heading(), HelpPaper::Welcome)
            } else {
                (Rect::new(260, 65, 268, 379), 10, 15, article_fonts::heading(), HelpPaper::Solid(0xffffff))
            };
            assert_eq!(viewer.rect(Part::Control(Hit::Article)), Some(pane), "{name} viewport");
            let x = pane.x + margin;
            let heading_y = pane.y + y;
            let body_y = heading_y + heading_font.height + 7;
            assert_help_words(&ours, name, heading_font, &home.paragraphs[0].text, x, heading_y);
            let prefix = home.paragraphs[1].text.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
            assert_help_words(&ours, name, article_fonts::body(), &prefix, x, body_y);
            area.authored_text = Some((
                Rect::new(at.0 + x, at.1 + body_y, pane.w - 2 * margin, pane.bottom() - body_y), paper,
            ));
            let thumb = viewer.rect(Part::Control(Hit::ScrollThumb));
            if welcome {
                // The supplied welcome text produces a 285px thumb; the original book used 294px.
                assert_eq!(thumb, Some(Rect::new(503, 81, 16, 285)), "{name} scrollbar");
                area.reflowed_scrollbar = Some(HelpScrollbar {
                    at: (at.0 + pane.right(), at.1 + pane.y), era, thumb_height: 285,
                });
            } else {
                assert!(thumb.is_none(), "{name} overview fits without scrolling");
            }
        }
        out.push(Case { name, file, ours, regions: vec![area], masks: vec![], vga: false });
    }
}

/// Calculator's Help, opened by Help > Help Topics on each Reference PC: 95's picker (two
/// closed books), 98's welcome page with the first book open, 2000's overview, both with the
/// tree's horizontal scroll bar; and one article each on 95 and 98 (the keyboard topic). As
/// in the games' cases, the books' own wording is checked at the measured places and the
/// paper around it, the window and its controls pixel for pixel.
fn calculator_help_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::calculator::help::catalog;
    use bevel_core::help::winhelp::{HelpTab, HelpTopics, Part as WinPart};
    use bevel_core::html_help::{HtmlHelp, Part, Tab, chrome::Hit};
    use bevel_core::kit::Form;

    let only = std::env::var("BEVEL95_CASE").ok();
    let skip = |name: &str| only.as_deref().is_some_and(|filter| !name.contains(filter));
    let prefix = |text: &str| text.split_whitespace().take(3).collect::<Vec<_>>().join(" ");

    // 95: "Using keyboard equivalents" opened from the picker, at the guest's 281 pixels.
    let name = "win95-calculator-help-keyboard";
    if !skip(name) {
        let book = catalog(Era::Win95);
        let topic = 5;
        let paragraphs = &book.topics[topic].paragraphs;
        let mut viewer = HelpTopics::article(Era::Win95, book.clone(), topic).unwrap();
        let (x, y, original_height) = (510, 59, 245);
        viewer.set_article_bounds(281, 508);
        viewer.set_screen(x, y);
        viewer.layout(f95);
        let (w, authored_height) = viewer.size();
        assert_eq!(w, 281, "{name} width");
        assert!(viewer.rect(WinPart::Thumb).is_none(), "{name} fits");
        let mut ours = Framebuffer::new(281, authored_height, 0);
        viewer.paint(&mut ours, &WIN95, f95);
        assert_help_words(&ours, name, f95.bold, &paragraphs[0].text, 22, 54);
        assert_help_words(&ours, name, f95.regular, &prefix(&paragraphs[1].text), 34, 76);
        let shared = original_height.min(authored_height) - 6;
        let mut page = region((0, 0), Rect::new(x, y, 281, shared));
        page.authored_text = Some((Rect::new(x + 22, y + 54, 281 - 39, shared - 54), HelpPaper::Solid(0xffffe2)));
        let bottom = region((0, authored_height - 6), Rect::new(x, y + original_height - 6, 281, 6));
        // CALC.HLP draws Related Topics as a small gray button under a rule; the shared viewer
        // lists the related topics as links instead (in the authored text).
        let masks = vec![Rect::new(532, 263, 244, 1), Rect::new(532, 268, 12, 12)];
        out.push(Case { name, file: "vm/win95-truecolor-calc-help-keyboard.png", ours, regions: vec![page, bottom], masks, vga: false });
    }

    // 95's picker as it opened: two closed books, the first chosen.
    let name = "win95-calculator-help-topics";
    if !skip(name) {
        let mut viewer = HelpTopics::new(Era::Win95, catalog(Era::Win95), HelpTab::Contents);
        let at = (200, 50);
        viewer.set_screen(at.0, at.1);
        viewer.layout(f95);
        viewer.blink(false);
        assert_eq!(viewer.size(), (434, 428), "{name} dimensions");
        let mut ours = Framebuffer::new(434, 428, 0);
        viewer.paint(&mut ours, &WIN95, f95);
        let area = whole(&ours, at);
        out.push(Case { name, file: "vm/win95-truecolor-calc-help-topics.png", ours, regions: vec![area], masks: vec![], vga: false });
    }

    // 98 and 2000: the window as it opened, and 98's keyboard topic after opening Tips and Tricks.
    for (era, state, at, size, theme, fonts) in [
        (Era::Win98, "help-topics", (50, 0), (525, 450), &WIN98, f95),
        (Era::Win98, "help-keyboard", (50, 0), (525, 450), &WIN98, f95),
        (Era::Win2000, "help-topics", (230, 30), (550, 450), &WIN2000, f2k),
    ] {
        let prefix_name = if era == Era::Win98 { "win98" } else { "win2000" };
        let name: &'static str = Box::leak(format!("{prefix_name}-calculator-{state}").into_boxed_str());
        if skip(name) {
            continue;
        }
        let file: &'static str = Box::leak(format!("vm/{prefix_name}-truecolor-calc-{state}.png").into_boxed_str());
        let book = catalog(era);
        let mut viewer = HtmlHelp::new(era, book.clone(), Tab::Contents);
        viewer.set_screen(at.0, at.1);
        viewer.layout(fonts);
        assert_eq!(viewer.size(), size, "{name} dimensions");
        let keyboard = state == "help-keyboard";
        if keyboard {
            // Tips and Tricks opened (a double-click), then its keyboard topic chosen.
            let books = viewer.rect(Part::Control(Hit::Row(5))).unwrap();
            viewer.double_click(books.x + 30, books.y + 4);
            viewer.layout(fonts);
            let row = viewer.rect(Part::Control(Hit::Row(8))).unwrap();
            viewer.press(Some(Part::Control(Hit::Row(8))), row.x + 30, row.y + 4);
            viewer.layout(fonts);
        }
        let shown = if keyboard { 6 } else { book.home };
        assert_eq!(viewer.print_text(), book.topics[shown].plain_text(), "{name} topic");
        let mut ours = Framebuffer::new(size.0, size.1, 0);
        viewer.paint(&mut ours, theme, fonts);
        let welcome = book.topics[shown].welcome;
        let pane = viewer.rect(Part::Control(Hit::Article)).unwrap();
        let (margin, top, heading_font, paper) = if welcome {
            (35, 93, article_fonts::welcome_heading(), HelpPaper::Welcome)
        } else if era == Era::Win98 {
            (10, 15, article_fonts::bold(), HelpPaper::Solid(0xffffff))
        } else {
            (10, 15, article_fonts::heading(), HelpPaper::Solid(0xffffff))
        };
        let x = pane.x + margin;
        let paragraphs = &book.topics[shown].paragraphs;
        // 98's keyboard heading wraps after "of" in the 270-pixel pane, as the original did.
        let heading: Vec<&str> = if keyboard {
            vec!["Using keyboard equivalents of", "Calculator buttons"]
        } else {
            vec![paragraphs[0].text.as_str()]
        };
        for (i, line) in heading.iter().enumerate() {
            assert_help_words(&ours, name, heading_font, line, x, pane.y + top + i as i32 * heading_font.height);
        }
        let body_y = pane.y + top + heading.len() as i32 * heading_font.height + 7;
        assert_help_words(&ours, name, article_fonts::body(), &prefix(&paragraphs[1].text), x, body_y);
        let mut area = whole(&ours, at);
        area.authored_text =
            Some((Rect::new(at.0 + x, at.1 + body_y, pane.w - 2 * margin, pane.bottom() - body_y), paper));
        let thumb = viewer.rect(Part::Control(Hit::ScrollThumb));
        // A thumb of another length than the guest's (98's welcome page: 294; its keyboard
        // table was a page of its own length) is checked as a control of that length.
        let guest = if (era, state) == (Era::Win98, "help-topics") { 294 } else { 0 };
        if let Some(r) = thumb.filter(|r| r.h != guest) {
            area.reflowed_scrollbar =
                Some(HelpScrollbar { at: (at.0 + pane.right(), at.1 + pane.y), era, thumb_height: r.h });
        }
        // The 98 table starts 163 pixels down the pane. Its cells reflow with the
        // viewer's fonts; the article checksum also checks every cell in reading order.
        let masks =
            if keyboard { vec![Rect::new(at.0 + pane.x, at.1 + pane.y + 163, pane.w, pane.h - 163)] } else { vec![] };
        out.push(Case { name, file, ours, regions: vec![area], masks, vga: false });
    }
}

/// The same shared HTML Help form that displays the game's book. Headings and viewer controls
/// remain directly comparable to the guest. The game supplies its own instructions underneath.
fn html_help_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::help::Style;
    use bevel_core::html_help::{HtmlHelp, Part, Tab, chrome::Hit};
    use bevel_core::kit::Form;
    use bevel_core::minefield::html_help_catalog;

    macro_rules! captures {
        ($era:literal; $($state:literal),+) => { [$(
            ($state, concat!($era, "-minesweeper-", $state), concat!("vm/", $era, "-truecolor-mine-", $state, ".png"))
        ),+] };
    }
    let old = captures!("win98"; "help-topics", "help-welcome-clean", "help-article-object", "help-article-playing", "help-article-strategy-fresh",
        "help-index", "help-search-empty", "help-search-typed", "help-search-results", "help-hidden", "help-options-initial-clean", "help-web", "help-web-fresh");
    let nt = captures!("win2000"; "help-topics", "help-search", "help-play", "help-custom", "help-search-empty",
        "help-search-typed", "help-search-results", "help-hidden", "using-help", "help-options", "help-web");
    let only = std::env::var("BEVEL95_CASE").ok();
    for (era, theme, fonts, shots) in
        [(Era::Win98, &WIN98, f95, old.as_slice()), (Era::Win2000, &WIN2000, f2k, nt.as_slice())]
    {
        for &(state, name, file) in shots {
            if only.as_deref().is_some_and(|filter| !name.contains(filter)) {
                continue;
            }
            let old = era == Era::Win98;
            let book = if state == "using-help" { help_manual::using_help() } else { html_help_catalog::catalog(era) };
            let home = book.home;
            let tab = match state {
                "help-index" | "help-search" => Tab::Index,
                "help-search-empty" | "help-search-typed" | "help-search-results" => Tab::Search,
                "help-options" | "help-web" if !old => Tab::Search,
                _ => Tab::Contents,
            };
            let topic = match state {
                "help-article-object" | "help-index" => 0,
                "help-search-empty" | "help-search-typed" | "help-search-results" if old => 0,
                "help-play" | "help-article-playing" => 1,
                "help-hidden" if old => 1,
                "help-article-strategy-fresh" => 2,
                "help-custom" | "help-search-empty" | "help-search-typed" | "help-search-results" | "help-hidden" => 3,
                "help-options" | "help-web" if !old => 3,
                _ => home,
            };
            let displayed = book.topics[topic].clone();
            let welcome = displayed.welcome;
            let text = displayed.plain_text();
            let mut viewer = HtmlHelp::new(era, book, tab);
            if topic != home {
                assert!(viewer.show_topic(topic));
            }
            if matches!(state, "help-search-typed" | "help-search-results") {
                for ch in "mines".chars() {
                    viewer.typed(ch);
                }
                if state == "help-search-results" {
                    // The Windows 98 capture used Enter in the query. On 2000 the
                    // operator clicked List Topics, leaving its dotted focus rectangle.
                    if !old {
                        viewer.set_focus(Part::Control(Hit::ListTopics));
                    }
                    let action = viewer.enter();
                    viewer.activate(action);
                    if old {
                        viewer.blink(false);
                    }
                }
            }
            let hidden = state == "help-hidden";
            if hidden {
                viewer.activate(Part::Control(Hit::NavToggle));
            }
            let at = match (old, hidden) {
                (true, true) => (270, 0),
                (true, false) => (50, 0),
                (false, true) => (484, 30),
                (false, false) => (230, 30),
            };
            viewer.set_screen(at.0, at.1);
            viewer.layout(fonts);
            let (w, h) = viewer.size();
            let expected_width = match (old, hidden) {
                (true, true) => 305,
                (true, false) => 525,
                (false, true) => 296,
                (false, false) => 550,
            };
            assert_eq!((w, h), (expected_width, 450), "{name} dimensions");
            if state.starts_with("help-options") || state.starts_with("help-web") {
                let options = state.starts_with("help-options");
                let restored = old && state == "help-web";
                if restored {
                    // The guest had hidden and restored its navigation pane in this
                    // recording. The restored document retains the hidden pane's width.
                    viewer.activate(Part::Control(Hit::NavToggle));
                    viewer.activate(Part::Control(Hit::NavToggle));
                }
                viewer.activate(Part::Control(if options { Hit::Options } else { Hit::WebHelp }));
                viewer.layout(fonts);
                let mut fb = Framebuffer::new(w, h, 0);
                viewer.paint(&mut fb, theme, fonts);
                // For Options check the popup and its toolbar button. The 2000
                // support directory retains its heading art and nine link labels.
                let areas = if options {
                    vec![
                        Rect::new(169, 23, 55, if old { 36 } else { 40 }),
                        if old { Rect::new(169, 59, 120, 177) } else { Rect::new(169, 63, 140, 194) },
                    ]
                } else {
                    let pane = if restored {
                        Rect::new(232, 65, 277, 379)
                    } else if old {
                        Rect::new(233, 65, 270, 379)
                    } else {
                        Rect::new(260, 65, 268, 379)
                    };
                    assert_eq!(viewer.rect(Part::Control(Hit::Article)), Some(pane), "{name} article viewport");
                    vec![pane]
                };
                let mut regions: Vec<_> = areas
                    .into_iter()
                    .map(|r| region((r.x, r.y), Rect::new(at.0 + r.x, at.1 + r.y, r.w, r.h)))
                    .collect();
                if old && !options {
                    // The 98 support page has its original backdrop and heading, followed
                    // by the viewer's locally authored advice at the measured body baseline.
                    let title = article_fonts::web_heading();
                    let y = 65 + 80;
                    let copy = viewer.print_text();
                    let mut paragraphs = copy.split("\n\n");
                    let heading = paragraphs.next().unwrap();
                    let pane = regions[0].reference;
                    let x = pane.x - at.0 + 35;
                    let lines = if restored {
                        assert_help_words(&fb, name, title, heading, x, y);
                        1
                    } else {
                        // The initial 270px viewport wraps the last word of the heading.
                        let (first, last) = heading.rsplit_once(' ').unwrap();
                        assert_help_words(&fb, name, title, first, x, y);
                        assert_help_words(&fb, name, title, last, x, y + title.height);
                        2
                    };
                    let body_y = y + lines * title.height + 7;
                    let prefix = paragraphs.next().unwrap().split_whitespace().take(3).collect::<Vec<_>>().join(" ");
                    assert_help_words(&fb, name, article_fonts::body(), &prefix, x, body_y);
                    regions[0].authored_text =
                        Some((Rect::new(pane.x + 35, body_y, pane.w - 70, 444 - body_y), HelpPaper::Welcome));
                }
                out.push(Case { name, file, ours: fb, regions, masks: vec![], vga: false });
                continue;
            }
            assert_eq!(viewer.print_text(), text, "{name} displayed topic");
            let mut fb = Framebuffer::new(w, h, 0);
            viewer.paint(&mut fb, theme, fonts);
            let pane = if hidden {
                Rect::new(6, 65, if old { 277 } else { 268 }, 379)
            } else if old {
                Rect::new(233, 65, 270, 379)
            } else {
                Rect::new(260, 65, 268, 379)
            };
            // The heading was retained in each article. Its placement, all ink and the paper
            // above the first body baseline are compared in full (including the 98 welcome logo).
            let heading_font = if welcome {
                article_fonts::welcome_heading()
            } else if !old && displayed.paragraphs[0].style == Style::Title {
                article_fonts::heading()
            } else {
                article_fonts::bold()
            };
            let heading_y = if welcome { 93 } else { 15 };
            let body_y = heading_y + heading_font.height + 7;
            let margins = if welcome { 35 } else { 10 };
            let changed = Rect::new(pane.x + margins, pane.y + body_y, pane.w - 2 * margins, pane.h - body_y);
            // Check the supplied heading and original writing at their measured baselines. On
            // the textured welcome paper only the black foreground belongs to the glyph.
            let first = &displayed.paragraphs[1];
            let prefix = first.text.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
            let indent = match first.style {
                Style::Step(_) if old => 22,
                Style::Step(_) => 24,
                Style::Bullet => 17,
                _ => 0,
            };
            for (font, words, x, y) in [
                (heading_font, displayed.paragraphs[0].text.as_str(), pane.x + margins, pane.y + heading_y),
                (article_fonts::body(), prefix.as_str(), changed.x + indent, changed.y),
            ] {
                assert_help_words(&fb, name, font, words, x, y);
            }
            let mut area = whole(&fb, at);
            area.authored_text = Some((
                Rect::new(at.0 + changed.x, at.1 + changed.y, changed.w, changed.h),
                if welcome { HelpPaper::Welcome } else { HelpPaper::Solid(0xffffff) },
            ));
            let thumb = viewer.rect(Part::Control(Hit::ScrollThumb));
            let height = thumb.map_or(0, |r| r.h);
            // These scroll extents correspond to the locally supplied text at the
            // measured default viewport. Preserve them independently of viewer layout.
            assert_eq!(
                height,
                if welcome {
                    312
                } else if state == "help-play" {
                    329
                } else {
                    0
                },
                "{name} scrollbar thumb height"
            );
            if let Some(r) = thumb {
                assert_eq!((r.x, r.y, r.w), (pane.right(), pane.y + 16, 16), "{name} top of scrollbar thumb");
            }
            let guest_height = if welcome {
                294
            } else if state == "using-help" {
                308
            } else {
                0
            };
            if height != guest_height {
                area.reflowed_scrollbar =
                    Some(HelpScrollbar { at: (at.0 + pane.right(), at.1 + pane.y), era, thumb_height: height });
            }
            if state == "help-play" {
                // Two more 13-pixel text lines precede the marker in the authored
                // instructions. Compare the original 10Ã—10 glyph at its new baseline.
                area.reflowed_note = Some((Rect::new(500, 189, 10, 10), Rect::new(500, 215, 10, 10)));
            }
            let masks = match (era, state) {
                (Era::Win98, "help-topics") => vec![Rect::new(400, 300, 10, 19)],
                (Era::Win2000, "help-topics" | "help-search" | "using-help") => vec![Rect::new(400, 299, 17, 25)],
                _ => vec![],
            };
            out.push(Case { name, file, ours: fb, regions: vec![area], masks, vga: false });
        }
    }
}

/// A program's window on the reference PCs (True Color): the real Minesweeper on Windows 95, 98
/// and 2000 and Calculator on Windows 98, each where it opened. Compared: the frame, the title bar
/// with its three buttons (Maximize grayed out) and the menu bar; in front and not, a title
/// chosen with the keyboard, a title with its menu open, and in Windows 2000 the pointer on a
/// title. Left out: the program's own icon and everything inside the window.
fn program_window_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::window::{self, ButtonLook, Frame, TitleButton, TitleLook};
    use TitleLook::{Chosen, Hot, Plain};
    struct Window {
        era: Era,
        at: (i32, i32),
        size: (i32, i32),
        title: &'static str,
        menus: &'static [&'static str],
    }
    let mine95 = Window { era: Era::Win95, at: (76, 36), size: (160, 239), title: "Minesweeper", menus: &["&Game", "&Help"] };
    let calc98 =
        Window { era: Era::Win98, at: (22, 22), size: (260, 252), title: "Calculator", menus: &["&Edit", "&View", "&Help"] };
    // Windows 98's own Minesweeper (its Games were installed on the reference PC on 2026-11-09).
    let mine98 = Window { era: Era::Win98, at: (76, 36), size: (160, 239), title: "Minesweeper", menus: &["&Game", "&Help"] };
    let mine2k = Window { era: Era::Win2000, at: (80, 80), size: (170, 251), title: "Minesweeper", menus: &["&Game", "&Help"] };
    // (name, capture, window, each title's look, in front, access keys underlined, only the
    // rows above the menu's drop-down, more of the capture to leave out)
    type Shot<'a> = (&'static str, &'static str, &'a Window, &'a [TitleLook], bool, bool, bool, &'a [Rect]);
    let shots: [Shot; 22] = [
        ("win95-window", "vm/win95-truecolor-mine-active.png", &mine95, &[Plain, Plain], true, true, false, &[]),
        ("win95-window-title-chosen", "vm/win95-truecolor-mine-menu-select-1.png", &mine95, &[Chosen, Plain], true, true, false, &[]),
        ("win95-window-title-chosen-2", "vm/win95-truecolor-mine-menu-select-2.png", &mine95, &[Plain, Chosen], true, true, false, &[]),
        ("win95-window-menu-open", "vm/win95-truecolor-mine-menu-open-1.png", &mine95, &[Chosen, Plain], true, true, true, &[]),
        // The Custom Field box is in front (it covers part of the right edge).
        ("win95-window-behind", "vm/win95-truecolor-mine-custom.png", &mine95, &[Plain, Plain], false, true, false, &[Rect::new(104, 100, 240, 140)]),
        ("win98-window", "vm/win98-truecolor-calc-active.png", &calc98, &[Plain, Plain, Plain], true, true, false, &[]),
        ("win98-window-title-chosen", "vm/win98-truecolor-calc-menu-select-1.png", &calc98, &[Chosen, Plain, Plain], true, true, false, &[]),
        ("win98-window-title-chosen-2", "vm/win98-truecolor-calc-menu-select-2.png", &calc98, &[Plain, Chosen, Plain], true, true, false, &[]),
        ("win98-window-menu-open", "vm/win98-truecolor-calc-menu-open-1.png", &calc98, &[Chosen, Plain, Plain], true, true, true, &[]),
        ("win98-window-menu-open-3", "vm/win98-truecolor-calc-menu-open-3.png", &calc98, &[Plain, Plain, Chosen], true, true, true, &[]),
        // The pointer on "View" (drawn into the capture): nothing shows in Windows 98.
        ("win98-window-pointer", "vm/win98-truecolor-calc-menu-hover-2.png", &calc98, &[Plain, Plain, Plain], true, true, false, &[Rect::new(74, 50, 14, 22)]),
        ("win98-window-behind", "vm/win98-truecolor-calc-inactive.png", &calc98, &[Plain, Plain, Plain], false, true, false, &[]),
        ("win98-window-mine", "vm/win98-truecolor-mine-active.png", &mine98, &[Plain, Plain], true, true, false, &[]),
        ("win98-window-mine-title-chosen", "vm/win98-truecolor-mine-menu-select-1.png", &mine98, &[Chosen, Plain], true, true, false, &[]),
        ("win98-window-mine-title-chosen-2", "vm/win98-truecolor-mine-menu-select-2.png", &mine98, &[Plain, Chosen], true, true, false, &[]),
        ("win98-window-mine-menu-open", "vm/win98-truecolor-mine-menu-open-1.png", &mine98, &[Chosen, Plain], true, true, true, &[]),
        // The Custom Field box is in front (it covers part of the right edge).
        ("win98-window-mine-behind", "vm/win98-truecolor-mine-custom.png", &mine98, &[Plain, Plain], false, true, false, &[Rect::new(104, 100, 240, 140)]),
        ("win2000-window", "vm/win2000-truecolor-mine-active.png", &mine2k, &[Plain, Plain], true, false, false, &[]),
        ("win2000-window-title-chosen", "vm/win2000-truecolor-mine-menu-select-1.png", &mine2k, &[Chosen, Plain], true, true, false, &[]),
        ("win2000-window-menu-open", "vm/win2000-truecolor-mine-menu-open-1.png", &mine2k, &[Chosen, Plain], true, true, true, &[]),
        // The pointer on "Game" raises it (the pointer and its shadow are in the capture).
        ("win2000-window-pointer", "vm/win2000-truecolor-mine-menu-hover-1.png", &mine2k, &[Hot, Plain], true, false, false, &[Rect::new(114, 112, 22, 12)]),
        ("win2000-window-behind", "vm/win2000-truecolor-mine-custom.png", &mine2k, &[Plain, Plain], false, false, false, &[Rect::new(83, 121, 204, 166)]),
    ];
    let invisible = Icon { width: 16, height: 16, pixels: vec![0; 256] };
    for (name, file, w, looks, active, keys, above_only, more) in shots {
        let (theme, fonts) = match w.era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        let (width, height) = w.size;
        let button = |b| ButtonLook { button: b, rect: window::title_button_rect(width, b), pressed: false, enabled: b != TitleButton::Maximize };
        let buttons = [button(TitleButton::Minimize), button(TitleButton::Maximize), button(TitleButton::Close)];
        let mut ours = Framebuffer::new(width, height, 0);
        let frame = Frame { era: w.era, title: w.title, icon: Some(&invisible), active, buttons: &buttons };
        window::paint_frame(&mut ours, theme, fonts, &frame);
        let rects = window::menu_bar_layout(fonts, w.menus);
        window::paint_menu_bar(&mut ours, theme, fonts, w.era, w.menus, &rects, looks, keys, active);
        let (x, y) = w.at;
        // Everything above the window's contents; its side and bottom edges where nothing covers them.
        let top = window::CLIENT_Y_WITH_MENU - i32::from(above_only);
        let mut regions = vec![region((0, 0), Rect::new(x, y, width, top))];
        if !above_only {
            let below = height - top;
            regions.extend([
                region((0, top), Rect::new(x, y + top, 3, below)),
                region((width - 3, top), Rect::new(x + width - 3, y + top, 3, below)),
                region((0, height - 3), Rect::new(x, y + height - 3, width, 3)),
            ]);
        }
        let mut masks = vec![Rect::new(x + 5, y + 4, 16, 16)];
        masks.extend_from_slice(more);
        out.push(Case { name, file, regions, ours, masks, vga: false });
    }
}


/// A program's menus on the reference PCs (True Color): a menu bar's drop-downs (under their
/// title, at its left edge) and the window menu (under the title bar's left end), with check
/// marks, a shortcut text and grayed-out entries, which Windows 95 and 98 draw etched in True
/// Color (gray over a white copy), plain gray on the selection.
fn program_menu_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::paint::Mark;
    use bevel_core::window_menu::{self, WindowState};
    // The real Minesweeper's Game menu (Windows 2000 adds Sound); "*" marks a checked entry.
    let game = |sound: bool, hover: usize, keys: bool| {
        let mut labels = vec!["&New\tF2", "-", "*&Beginner", "&Intermediate", "&Expert", "&Custom...", "-", "*&Marks (?)", "*Co&lor"];
        if sound {
            labels.push("&Sound");
        }
        labels.extend(["-", "Best &Times...", "-", "E&xit"]);
        let entries = labels
            .iter()
            .map(|&l| match (l, l.strip_prefix('*')) {
                ("-", _) => MenuEntry::separator(),
                (_, Some(checked)) => MenuEntry { mark: Some(Mark::Check), ..MenuEntry::item(checked, None) },
                _ => MenuEntry::item(l, None),
            })
            .collect();
        Menu { hover: Some(hover), show_keys: keys, ..Menu::new(entries) }
    };
    // Calculator's Edit menu with nothing to paste.
    let edit = |hover: usize| {
        let mut m = menu(&["&Copy\tCtrl+C", "&Paste\tCtrl+V"], Some(hover), true);
        m.entries[1].disabled = true;
        m
    };
    // A window that cannot be sized or maximized, as Windows lists it (the first entry chosen
    // by the keyboard, grayed out or not).
    let fixed = WindowState { has_minimize_box: true, closable: true, ..WindowState::default() };
    let window = |era: Era, hover: usize| Menu { hover: Some(hover), show_keys: true, ..window_menu::program_menu(era, fixed) };
    // (name, capture, look, where the menu is in the capture, the menu)
    type Shot = (&'static str, &'static str, Era, (i32, i32), Menu);
    let shots: Vec<Shot> = vec![
        ("win95-menu-game","vm/win95-truecolor-mine-menu-open-1.png", Era::Win95, (79, 76), game(false, 0, true)),
        ("win95-menu-game-5", "vm/win95-truecolor-mine-menu-open-1-item5.png", Era::Win95, (79, 76), game(false, 5, true)),
        ("win95-menu-help", "vm/win95-truecolor-mine-menu-open-2.png", Era::Win95, (119, 76), menu(&["&Help Topics", "-", "&About Minesweeper"], Some(0), true)),
        ("win95-menu-window", "vm/win95-truecolor-mine-sysmenu.png", Era::Win95, (79, 57), window(Era::Win95, 0)),
        ("win98-menu-edit", "vm/win98-truecolor-calc-menu-open-1.png", Era::Win98, (25, 62), edit(0)),
        ("win98-menu-edit-grayed", "vm/win98-truecolor-calc-menu-open-1-item2.png", Era::Win98, (25, 62), edit(1)),
        ("win98-menu-window", "vm/win98-truecolor-calc-sysmenu-item.png", Era::Win98, (25, 43), window(Era::Win98, 1)),
        ("win98-menu-game", "vm/win98-truecolor-mine-menu-open-1.png", Era::Win98, (79, 76), game(false, 0, true)),
        ("win98-menu-game-5", "vm/win98-truecolor-mine-menu-open-1-item5.png", Era::Win98, (79, 76), game(false, 5, true)),
        ("win98-menu-help", "vm/win98-truecolor-mine-menu-open-2.png", Era::Win98, (119, 76), menu(&["&Help Topics", "-", "&About Minesweeper"], Some(0), true)),
        ("win98-menu-window-mine", "vm/win98-truecolor-mine-sysmenu.png", Era::Win98, (79, 57), window(Era::Win98, 0)),
        ("win2000-menu-game", "vm/win2000-truecolor-mine-menu-open-1.png", Era::Win2000, (83, 120), game(true, 0, true)),
        ("win2000-menu-game-5", "vm/win2000-truecolor-mine-menu-open-1-item5.png", Era::Win2000, (83, 120), game(true, 5, true)),
        ("win2000-menu-window-mine", "vm/win2000-truecolor-mine-sysmenu.png", Era::Win2000, (83, 101), window(Era::Win2000, 0)),
    ];
    for (name, file, era, at, m) in shots {
        let (theme, fonts) = match era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        let layout = paint::layout_menu(theme, fonts, &m, MenuStyle::Popup, 600);
        let mut ours = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut ours, theme, fonts, &m, MenuStyle::Popup, &layout);
        out.push(Case { name, file, regions: vec![whole(&ours, at)], ours, masks: Vec::new(), vga: false });
    }
}

/// Date/Time Properties on the reference PCs: the same window, wherever each opened it, at the
/// time its clock showed.
fn datetime_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::datetime::{self, DateTime, Moment, Part};
    let at = |day: u32, hour: u32, minute: u32, second: u32| Moment { year: 2026, month: 11, day, hour, minute, second };
    let shots = [
        ("win95-datetime", "vm/win95-truecolor-datetime-1.png", Era::Win95, (66, 66), at(8, 1, 3, 31), Part::Month),
        ("win98-datetime", "vm/win98-truecolor-datetime-2.png", Era::Win98, (0, 0), at(28, 22, 36, 42), Part::Month),
        ("win2000-datetime", "vm/win2000-truecolor-datetime-1.png", Era::Win2000, (132, 132), at(8, 22, 50, 32), Part::Month),
    ];
    for (name, file, era, (x, y), shown, focus) in shots {
        let (theme, fonts) = match era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        let mut d = DateTime::new(era, shown, "Pacific Standard Time".to_string());
        d.screen = (x, y);
        d.focus = focus;
        // On 98 the calendar had been moved to the 28th (not applied yet): Apply is on.
        d.changed = era == Era::Win98;
        // Opened from the keyboard (the Run box): Windows 2000 shows the access keys too.
        d.show_keys = true;
        let l = datetime::layout(&d, fonts);
        let mut ours = Framebuffer::new(l.width, l.height, 0);
        datetime::paint(&mut ours, theme, fonts, &d, &l);
        // The hands are not drawn yet (this also leaves out the minute dots near the diagonals).
        let hands = Rect::new(x + 243, y + 91, 104, 104);
        out.push(Case { name, file, regions: vec![whole(&ours, (x, y))], ours, masks: if std::env::var_os("BEVEL95_HANDS").is_some() { vec![] } else { vec![hands] }, vga: false });
    }

    // Windows 98's time field while being set from the keyboard (window at 0,0): the chosen
    // part highlighted, typed into, AM/PM chosen; with the caret where it was captured shown.
    use bevel_core::datetime::TimeEntry::{self, Selected, Typed};
    let typed = |t: &str| Typed(t.to_string());
    let field: [(&'static str, &'static str, Moment, usize, TimeEntry, bool); 8] = [
        ("win98-time-typed-hour", "vm/win98-truecolor-datetime-type-0.png", at(9, 3, 54, 53), 0, typed("3"), true),
        ("win98-time-minutes", "vm/win98-truecolor-datetime-type-3.png", at(9, 3, 54, 53), 1, Selected, true),
        ("win98-time-typed-minute", "vm/win98-truecolor-datetime-type-4.png", at(9, 3, 5, 53), 1, typed("5"), true),
        ("win98-time-typed-seconds", "vm/win98-truecolor-datetime-type-9.png", at(9, 3, 59, 4), 2, typed("04"), false),
        ("win98-time-am", "vm/win98-truecolor-datetime-type-10.png", at(9, 3, 59, 4), 3, Selected, true),
        ("win98-time-hour", "vm/win98-truecolor-datetime-type-15.png", at(9, 3, 59, 4), 0, Selected, true),
        ("win98-time-typed-1", "vm/win98-truecolor-datetime-type-16.png", at(9, 1, 59, 4), 0, typed("1"), true),
        ("win98-time-typed-11", "vm/win98-truecolor-datetime-type-17.png", at(9, 11, 59, 4), 0, typed("11"), false),
    ];
    timezone_cases(out, f95, f2k);
    for (name, file, shown, part, entry, caret) in field {
        let mut d = DateTime::new(Era::Win98, shown, "Pacific Standard Time".to_string());
        d.focus = Part::Time;
        d.time_field = part;
        d.entry = entry;
        d.caret = caret;
        d.show_keys = true;
        let l = datetime::layout(&d, f95);
        let mut ours = Framebuffer::new(l.width, l.height, 0);
        datetime::paint(&mut ours, &WIN98, f95, &d, &l);
        let well = Rect::new(256, 222, 77, 20);
        out.push(Case { name, file, regions: vec![region((well.x, well.y), well)], ours, masks: vec![], vga: false });
    }
}
/// The Time Zone page on the reference PCs (window where each opened it): the zone field with
/// the focus, its list dropped down at several scroll positions, the daylight saving box grayed
/// out (95, 98) or left out (2000) for a zone without daylight saving time, the box with the
/// focus, and Windows 2000's map turned to eight more zones.
fn timezone_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::datetime::{self, DateTime, Moment, Part};
    use bevel_core::timezone;
    struct Shot {
        name: &'static str,
        file: &'static str,
        era: Era,
        at: (i32, i32),
        zone: usize,
        list: Option<usize>,
        focus: Part,
        no_dst: Option<usize>,
        changed: bool,
    }
    let shot = |name, file, era, at, zone, list, focus, no_dst, changed| Shot { name, file, era, at, zone, list, focus, no_dst, changed };
    let z = Part::Zone;
    let shots = [
        shot("win98-timezone", "vm/win98-truecolor-timezone-1.png", Era::Win98, (0, 0), 4, None, z, None, true),
        shot("win98-timezone-list-1", "vm/win98-truecolor-timezone-list-1.png", Era::Win98, (0, 0), 0, Some(0), z, Some(0), true),
        shot("win98-timezone-list-3", "vm/win98-truecolor-timezone-list-3.png", Era::Win98, (0, 0), 28, Some(14), z, None, true),
        shot("win98-timezone-list-5", "vm/win98-truecolor-timezone-list-5.png", Era::Win98, (0, 0), 56, Some(42), z, Some(56), true),
        shot("win98-timezone-list-6", "vm/win98-truecolor-timezone-list-6.png", Era::Win98, (0, 0), 61, Some(47), z, Some(61), true),
        shot("win98-timezone-no-dst", "vm/win98-truecolor-timezone-central.png", Era::Win98, (0, 0), 61, None, z, Some(61), true),
        shot("win95-timezone", "vm/win95-truecolor-timezone-1.png", Era::Win95, (44, 44), 4, None, z, None, false),
        shot("win95-timezone-list-1", "vm/win95-truecolor-timezone-list-1.png", Era::Win95, (44, 44), 0, Some(0), z, Some(0), true),
        shot("win95-timezone-list-4", "vm/win95-truecolor-timezone-list-4.png", Era::Win95, (44, 44), 42, Some(28), z, Some(42), true),
        shot("win95-timezone-list-6", "vm/win95-truecolor-timezone-list-6.png", Era::Win95, (44, 44), 51, Some(37), z, None, true),
        shot("win95-timezone-no-dst", "vm/win95-truecolor-timezone-hawaii.png", Era::Win95, (44, 44), 2, None, z, Some(2), true),
        shot("win95-timezone-dst-focus", "vm/win95-truecolor-timezone-checkfocus2.png", Era::Win95, (44, 44), 51, None, Part::Dst, None, true),
        shot("win2000-timezone", "vm/win2000-truecolor-timezone-1.png", Era::Win2000, (110, 110), 4, None, z, None, false),
        shot("win2000-timezone-list-1", "vm/win2000-truecolor-timezone-list-1.png", Era::Win2000, (110, 110), 0, Some(0), z, Some(0), true),
        shot("win2000-timezone-list-5", "vm/win2000-truecolor-timezone-list-5.png", Era::Win2000, (110, 110), 56, Some(42), z, None, true),
        shot("win2000-timezone-list-7", "vm/win2000-truecolor-timezone-list-7.png", Era::Win2000, (110, 110), 73, Some(59), z, Some(73), true),
        shot("win2000-timezone-dst-focus", "vm/win2000-truecolor-timezone-checkfocus.png", Era::Win2000, (110, 110), 66, None, Part::Dst, None, true),
        shot("win2000-timezone-no-dst", "vm/win2000-truecolor-timezone-z61tab.png", Era::Win2000, (110, 110), 60, None, Part::Ok, Some(60), true),
        shot("win2000-timezone-z01", "vm/win2000-truecolor-timezone-z01.png", Era::Win2000, (110, 110), 0, None, z, Some(0), true),
        shot("win2000-timezone-z13", "vm/win2000-truecolor-timezone-z13.png", Era::Win2000, (110, 110), 12, None, z, None, true),
        shot("win2000-timezone-z26", "vm/win2000-truecolor-timezone-z26.png", Era::Win2000, (110, 110), 25, None, z, None, true),
        shot("win2000-timezone-z34", "vm/win2000-truecolor-timezone-z34.png", Era::Win2000, (110, 110), 33, None, z, None, true),
        shot("win2000-timezone-z48", "vm/win2000-truecolor-timezone-z48.png", Era::Win2000, (110, 110), 47, None, z, Some(47), true),
        shot("win2000-timezone-z61", "vm/win2000-truecolor-timezone-z61b.png", Era::Win2000, (110, 110), 60, None, z, Some(60), true),
        shot("win2000-timezone-z67", "vm/win2000-truecolor-timezone-z67.png", Era::Win2000, (110, 110), 66, None, z, None, true),
        shot("win2000-timezone-z74", "vm/win2000-truecolor-timezone-z74.png", Era::Win2000, (110, 110), 73, None, z, Some(73), true),
    ];
    let now = Moment { year: 2026, month: 11, day: 9, hour: 7, minute: 58, second: 0 };
    for s in shots {
        let (theme, fonts) = match s.era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        let mut d = DateTime::new(s.era, now, "Pacific Standard Time".to_string());
        d.screen = s.at;
        d.show_page(1);
        d.zone = s.zone;
        d.zone_has_dst = (0..timezone::zones(s.era).len()).map(|i| Some(i) != s.no_dst).collect();
        if let Some(top) = s.list {
            d.zone_list = true;
            d.zone_top = top;
        }
        d.focus = s.focus;
        d.changed = s.changed;
        d.show_keys = true;
        let l = datetime::layout(&d, fonts);
        let mut ours = Framebuffer::new(l.width, l.height, 0);
        datetime::paint(&mut ours, theme, fonts, &d, &l);
        // The PCs' mouse pointer, where it rested in some captures.
        let pointer = Rect::new(398, 298, 14, 22);
        out.push(Case { name: s.name, file: s.file, regions: vec![whole(&ours, s.at)], ours, masks: vec![pointer], vga: false });
    }
}

/// Run's list of earlier commands, dropped down on the reference PCs
/// (`vm/<era>-truecolor-runlist-*.png`; the commands are the PCs' own, typed for these
/// captures). The Run window and its list are compared; the field's inside is masked (Windows
/// shows the command selected there, Bevel95 a caret).
fn run_list_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::dialog::DialogWindow;
    use bevel_core::droplist::Shown;
    use bevel_core::kit::Form;
    const W95: &[&str] = &[
        "c:\\windows\\system",
        "c:\\windows",
        "c:\\",
        "winver",
        "calc",
        "notepad",
        "sndvol32 /t",
        "rundll32.exe shell32.dll,Control_RunDLL sysdm.cpl,@1",
        "control desk.cpl",
        "command /k dir c:\\windows\\system\\iosubsys",
        "control desk.cpl,,3",
        "command /c ren c:\\windows\\system\\iosubsys\\esdi_506.pdr esdi_506.pd_",
    ];
    const W98: &[&str] = &[
        "freecell",
        "sol",
        "c:\\windows",
        "c:\\",
        "charmap",
        "winver",
        "calc",
        "notepad",
        "winmine",
        "control appwiz.cpl,,2",
        "control desk.cpl",
        "control sysdm.cpl",
        "control sysdm.cpl @1",
        "control desk.cpl,,5",
        "rundll32.exe shell32.dll,Control_RunDLL sysdm.cpl,@1",
    ];
    const W2K: &[&str] = &[
        "c:\\winnt\\repair",
        "c:\\winnt\\config",
        "c:\\winnt\\web",
        "c:\\winnt\\cursors",
        "c:\\winnt\\system",
        "c:\\winnt\\inf",
        "c:\\winnt\\temp",
        "c:\\winnt\\media",
        "c:\\winnt\\help",
        "c:\\winnt\\fonts",
        "c:\\winnt\\system32",
        "mspaint",
        "freecell",
        "sol",
        "c:\\winnt",
        "c:\\",
        "charmap",
        "winver",
        "calc",
        "notepad",
        "systray",
        "sndvol32 /t",
        "control mmsys.cpl",
        "control hdwwiz.cpl",
        "control desk.cpl",
        "devmgmt.msc",
        "control desk.cpl,,3",
    ];
    struct Shot {
        name: &'static str,
        file: &'static str,
        era: Era,
        /// The Run window's place on the PC's screen.
        at: (i32, i32),
        history: Vec<&'static str>,
        shown: Shown,
        /// The drop-down button held down; the pointer resting there (masked).
        held: bool,
        pointer: Option<(i32, i32)>,
    }
    let shot = |name, file, era, at, history: Vec<&'static str>, top, selected, held, pointer| Shot {
        name,
        file,
        era,
        at,
        history,
        shown: Shown { top, selected },
        held,
        pointer,
    };
    let (a95, a2k) = ((5, 405), (5, 389));
    let shots = [
        shot("win95-runlist-6", "vm/win95-truecolor-runlist-f4-open.png", Era::Win95, a95,
            [&W95[6..8], &W95[8..10], &W95[10..12]].concat(), 0, Some(0), false, None),
        shot("win95-runlist-many", "vm/win95-truecolor-runlist-f4-open-many.png", Era::Win95, a95, W95.to_vec(), 0, Some(0), false, None),
        shot("win95-runlist-typed", "vm/win95-truecolor-runlist-open-typed-ca.png", Era::Win95, a95, W95.to_vec(), 2, None, false, None),
        shot("win98-runlist-7", "vm/win98-truecolor-runlist-f4-open.png", Era::Win98, a95,
            [&W98[8..9], &W98[9..15]].concat(), 0, Some(0), false, None),
        shot("win98-runlist-many", "vm/win98-truecolor-runlist-f4-open-many.png", Era::Win98, a95, W98.to_vec(), 0, Some(0), false, None),
        shot("win98-runlist-down", "vm/win98-truecolor-runlist-open-down3.png", Era::Win98, a95, W98.to_vec(), 0, Some(3), false, None),
        shot("win98-runlist-wheel", "vm/win98-truecolor-runlist-wheel-after.png", Era::Win98, a95, W98.to_vec(), 5, Some(2), false, Some((217, 538))),
        shot("win2000-runlist-7", "vm/win2000-truecolor-runlist-f4-open.png", Era::Win2000, a2k,
            [&W2K[20..26], &W2K[26..27]].concat(), 0, Some(0), false, None),
        shot("win2000-runlist-full", "vm/win2000-truecolor-runlist-f4-open-full.png", Era::Win2000, a2k, W2K[..26].to_vec(), 0, Some(0), false, None),
        shot("win2000-runlist-down", "vm/win2000-truecolor-runlist-open-down2.png", Era::Win2000, a2k, W2K[..26].to_vec(), 0, Some(2), false, None),
        shot("win2000-runlist-held", "vm/win2000-truecolor-runlist-btn-held.png", Era::Win2000, a2k, W2K[..26].to_vec(), 1, Some(1), true, Some((325, 480))),
    ];
    let only = std::env::var("BEVEL95_CASE").ok();
    for s in shots {
        if only.as_deref().is_some_and(|o| !s.name.contains(o)) {
            continue;
        }
        let (theme, fonts) = match s.era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        let mut d = Dialog::new(DialogKind::Run, s.era);
        d.text = s.history[s.shown.selected.unwrap_or(0)].to_string();
        d.caret_on = false;
        d.show_keys = true;
        d.pressed = s.held.then_some(dialog::Part::DropDown);
        let mut w = DialogWindow::new(d, s.history.iter().map(|c| c.to_string()).collect());
        w.layout(fonts);
        w.screen = s.at;
        w.screen_bottom = 600;
        w.list = Some(s.shown);
        let (width, height) = w.size();
        let drop = w.drop_rect().unwrap();
        // Ours: the window, and its list beside it (where it goes, relative to the window).
        let gap = 4;
        let mut ours = Framebuffer::new(width + gap + drop.w, height.max(drop.h), 0);
        let mut own = Framebuffer::new(width, height, 0);
        w.paint(&mut own, theme, fonts);
        ours.blit(&own, 0, 0);
        let mut list = Framebuffer::new(drop.w, drop.h, 0);
        w.paint_drop(&mut list, theme, fonts);
        ours.blit(&list, width + gap, 0);
        let screen = |r: Rect| Rect::new(s.at.0 + r.x, s.at.1 + r.y, r.w, r.h);
        // The window less what the list covers (the bands above and below it, the strips
        // beside it), then the list.
        let (y0, y1) = (drop.y.clamp(0, height), drop.bottom().clamp(0, height));
        let parts = [
            Rect::new(0, 0, width, y0),
            Rect::new(0, y1, width, height - y1),
            Rect::new(0, y0, drop.x, y1 - y0),
            Rect::new(drop.right(), y0, width - drop.right(), y1 - y0),
        ];
        let mut regions: Vec<Region> =
            parts.iter().filter(|r| r.w > 0 && r.h > 0).map(|&r| region((r.x, r.y), screen(r))).collect();
        regions.push(region((width + gap, 0), screen(drop)));
        let field = w.rect(dialog::Part::Command).unwrap();
        let mut masks = vec![screen(Rect::new(field.x + 2, field.y + 2, field.w - 4 - 16, field.h - 4))];
        if let Some((x, y)) = s.pointer {
            masks.push(Rect::new(x, y, 20, 30));
        }
        out.push(Case { name: s.name, file: s.file, ours, regions, masks, vga: false });
    }
}

/// The Start menu's Settings and Find (Search) submenus with their classic icons, on the
/// reference PCs: `vm/<era>-truecolor-<menu>-hover.png` opened by the pointer (nothing
/// selected), `-<n>.png` with entry n selected by the keyboard. The PCs have no network, so
/// Find has no "Computer..." (Bevel95 neither); Windows 98's Settings also has Active Desktop,
/// and 98's and 2000's Find has People (not in Bevel95). So each of our entries is compared
/// with the capture's row of the same entry (less the separators' inset at the right end,
/// where the wider real menu goes on), and the whole menu where both have the same entries.
/// Find (Search) ends with Bevel95's own "Quick Search..." (until 2026-11-16 "Programs..."), which no capture has: its cases compare
/// the captured rows only (ours_after).
fn submenu_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::start::{StartEntry, fixed_menu};
    struct Sub {
        name: &'static str,
        theme: &'static Theme,
        era: Era,
        entry: StartEntry,
        /// Screen position of the first entry's rectangle in the captures.
        at: (i32, i32),
        /// The capture's top row of each of our entries (separators included).
        tops: &'static [i32],
        same_entries: bool,
        /// Entries of ours after the captured ones (Bevel95's own), compared with nothing.
        ours_after: usize,
        /// Capture suffix, our selected entry, access keys shown.
        captures: &'static [(&'static str, Option<usize>, bool)],
    }
    let subs = [
        Sub {
            name: "win95-settings",
            theme: &WIN95,
            era: Era::Win95,
            entry: StartEntry::Settings,
            at: (163, 372),
            tops: &[372, 394, 416],
            same_entries: true,
            ours_after: 0,
            captures: &[("hover", None, true), ("1", Some(0), true), ("2", Some(1), true), ("3", Some(2), true)],
        },
        Sub {
            name: "win95-find",
            theme: &WIN95,
            era: Era::Win95,
            entry: StartEntry::Find,
            at: (163, 404),
            tops: &[404],
            same_entries: true,
            ours_after: 1,
            captures: &[("hover", None, true), ("1", Some(0), true)],
        },
        Sub {
            name: "win98-settings",
            theme: &WIN98,
            era: Era::Win98,
            entry: StartEntry::Settings,
            at: (168, 384),
            // Active Desktop is the capture's row at 464.
            tops: &[384, 404, 424, 444, 484, 492],
            same_entries: false,
            ours_after: 0,
            captures: &[
                ("hover", None, true),
                ("1", Some(0), true),
                ("2", Some(1), true),
                ("3", Some(2), true),
                ("4", Some(3), true),
                ("6", Some(5), true),
            ],
        },
        Sub {
            name: "win98-find",
            theme: &WIN98,
            era: Era::Win98,
            entry: StartEntry::Find,
            at: (168, 420),
            tops: &[420, 440],
            same_entries: false,
            ours_after: 1,
            captures: &[("hover", None, true), ("1", Some(0), true), ("2", Some(1), true)],
        },
        Sub {
            name: "win2000-settings",
            theme: &WIN2000,
            era: Era::Win2000,
            entry: StartEntry::Settings,
            at: (253, 405),
            tops: &[405, 425, 433, 453, 473],
            same_entries: true,
            ours_after: 0,
            captures: &[
                ("hover", None, true),
                ("1", Some(0), true),
                ("2", Some(2), true),
                ("3", Some(3), true),
                ("4", Some(4), true),
            ],
        },
        Sub {
            name: "win2000-search",
            theme: &WIN2000,
            era: Era::Win2000,
            entry: StartEntry::Find,
            at: (253, 437),
            tops: &[437, 457],
            same_entries: false,
            ours_after: 1,
            captures: &[("hover", None, true), ("1", Some(0), true), ("2", Some(1), true)],
        },
    ];
    for sub in subs {
        let fonts = if sub.era == Era::Win2000 { f2k } else { f95 };
        for &(suffix, selected, show_keys) in sub.captures {
            let menu = Menu { hover: selected, show_keys, ..fixed_menu(sub.entry, sub.era) };
            let layout = paint::layout_menu(sub.theme, fonts, &menu, MenuStyle::Cascade, 600);
            let mut ours = Framebuffer::new(layout.width, layout.height, 0);
            paint::paint_menu(&mut ours, sub.theme, fonts, &menu, MenuStyle::Cascade, &layout);
            assert_eq!(layout.rects.len(), sub.tops.len() + sub.ours_after, "{}", sub.name);
            let first = layout.rects[0];
            let cut = if sub.same_entries { 0 } else { sub.theme.start_menu.sep.inset };
            let mut regions: Vec<Region> = layout
                .rects
                .iter()
                .zip(sub.tops)
                .map(|(r, &top)| region((r.x, r.y), Rect::new(sub.at.0 + r.x - first.x, top, r.w - cut, r.h)))
                .collect();
            if sub.same_entries && sub.ours_after == 0 {
                regions.push(whole(&ours, (sub.at.0 - first.x, sub.at.1 - first.y)));
            }
            let (prefix, menu_name) = sub.name.split_once('-').unwrap();
            out.push(Case {
                name: format!("vm-{}-{suffix}", sub.name).leak(),
                file: format!("vm/{prefix}-truecolor-{menu_name}-{suffix}.png").leak(),
                ours,
                regions,
                masks: Vec::new(),
                vga: false,
            });
        }
    }
}

/// The Start menu's Documents submenu on the reference PCs, after opening zebra, apple and
/// mango (in that order) from Explorer (`vm/<era>-truecolor-documents-<n>.png`, opened from the
/// keyboard, so its first entry is selected). Windows 95 sorts by name (its own Readme too),
/// 98 lists them in the order they were opened under "My Documents" and a line, 2000 sorts by
/// name under the same two, and shows a grayed "(Empty)" with none. The documents' own icons
/// come from the PC's shell, so their cells are left out; "My Documents"' icon is compared.
fn documents_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::programs::Item;
    use bevel_core::start;
    type Shot = (&'static str, &'static str, Era, (i32, i32), &'static [&'static str]);
    let shots: [Shot; 5] = [
        ("win98-documents-empty", "vm/win98-truecolor-documents-0.png", Era::Win98, (165, 345), &[]),
        ("win98-documents", "vm/win98-truecolor-documents-2.png", Era::Win98, (165, 345), &["zebra", "apple", "mango"]),
        ("win2000-documents-empty", "vm/win2000-truecolor-documents-0.png", Era::Win2000, (250, 370), &[]),
        ("win2000-documents", "vm/win2000-truecolor-documents-1.png", Era::Win2000, (250, 370), &["apple", "mango", "zebra"]),
        ("win95-documents", "vm/win95-truecolor-documents-1.png", Era::Win95, (160, 337), &["apple", "mango", "Readme", "zebra"]),
    ];
    for (name, file, era, at, names) in shots {
        let (theme, fonts) = match era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        let docs: Vec<Item> = names.iter().map(|n| Item { name: n.to_string(), target: String::new() }).collect();
        let my_documents = bevel_core::icons::my_documents(era);
        let menu = Menu { hover: Some(0), show_keys: true, ..start::documents_menu(era, &docs, my_documents, |_| None) };
        let layout = paint::layout_menu(theme, fonts, &menu, MenuStyle::Cascade, 600);
        let mut ours = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut ours, theme, fonts, &menu, MenuStyle::Cascade, &layout);
        let items = &theme.start_menu.small;
        let first_doc = menu.entries.len() - docs.len();
        let masks = layout.rects[first_doc..]
            .iter()
            .map(|r| Rect::new(at.0 + r.x + items.icon_x, at.1 + r.y + (r.h - items.icon) / 2, items.icon, items.icon))
            .collect();
        out.push(Case { name, file, regions: vec![whole(&ours, at)], ours, masks, vga: false });
    }
}

/// Task buttons with the classic pictures Bevel95 gives Notepad and Explorer's windows, on the
/// reference PCs (`vm/<era>-truecolor-tasks-<n>.png`): windows opened one after another, each
/// pressed as it opened, then all of them raised. Windows 98 and 2000 show Quick Launch (not
/// in Bevel95): its band ends a few pixels before the notification area, so their task area is
/// narrower (517 and 566 pixels: the only widths that give the buttons the captures' widths
/// with 4, 5 and 6 windows). Our bar is drawn with the same task area, and each button is
/// compared where the capture has it.
fn task_icon_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::window_icons::{self, ClassicWindow::*};
    let sets = [
        (
            "win95",
            &WIN95,
            f95,
            Era::Win95,
            60,
            None,
            0,
            &[
                ("My Computer", MyComputer),
                ("Recycle Bin", RecycleBin),
                ("Untitled - Notepad", Notepad),
                ("Control Panel", ControlPanel),
                ("Windows", Folder),
            ][..],
        ),
        (
            "win98",
            &WIN98,
            f95,
            Era::Win98,
            176,
            Some(517),
            2,
            &[
                ("My Documents", MyDocuments),
                ("My Computer", MyComputer),
                ("Recycle Bin", RecycleBin),
                ("Untitled - Notepad", Notepad),
                ("Control Panel", ControlPanel),
                ("Temp", Folder),
            ][..],
        ),
        (
            "win2000",
            &WIN2000,
            f2k,
            Era::Win2000,
            155,
            Some(566),
            0,
            &[
                ("My Computer", MyComputer),
                ("My Documents", MyDocuments),
                ("Recycle Bin", RecycleBin),
                ("Untitled - Notepad", Notepad),
                ("Control Panel", ControlPanel),
                ("Temp", Folder),
            ][..],
        ),
    ];
    for (prefix, theme, fonts, era, first_x, area, tray, windows) in sets {
        // How far right of ours the capture's first button is (its Quick Launch), and the bar
        // width that gives our task area the capture's width.
        let (_, plain) = bar(theme, fonts, 800, false, vec![task("x", TaskState::Normal)], tray, WIDEST);
        let shift = first_x - plain.tasks[0].x;
        // Where our task area ends: 95's 2 before the well; 98's and 2000's bands end 7 and 6
        // before it (`quick_launch::task_band`).
        let end_gap = match era {
            Era::Win98 => 7,
            Era::Win2000 => 6,
            Era::Win95 => theme.tray.margin_x,
        };
        let our_area = plain.well.x - end_gap - plain.tasks[0].x;
        let width = area.map_or(800, |a| 800 - our_area + a);
        for n in 1..=windows.len() + 1 {
            let tasks = windows[..n.min(windows.len())]
                .iter()
                .enumerate()
                .map(|(i, &(title, kind))| Task {
                    title: title.to_string(),
                    icon: Some(window_icons::icon(era, kind)),
                    state: if i + 1 == n { TaskState::Active } else { TaskState::Normal },
                })
                .collect();
            let (ours, layout) = bar(theme, fonts, width, false, tasks, tray, WIDEST);
            let regions = layout.tasks.iter().map(|r| region((r.x, 0), Rect::new(r.x + shift, 572, r.w, 28))).collect();
            // Windows 2000 SP4's Tahoma is a little narrower than today's in a few letters, so
            // in the narrowest buttons it fits one letter more before the "...": "My Docume..."
            // and "Untitled - N..." where today's Tahoma gives "My Docum..." and "Untitled -
            // ...". Those label ends are left out (private/todo.md, Windows 2000's own Tahoma).
            let masks = if prefix == "win2000" && n >= 6 {
                vec![Rect::new(318, 578, 21, 18), Rect::new(504, 578, 21, 18)]
            } else {
                Vec::new()
            };
            out.push(Case {
                name: format!("vm-{prefix}-tasks-{n}").leak(),
                file: format!("vm/{prefix}-truecolor-tasks-{n}.png").leak(),
                ours,
                regions,
                masks,
                vga: false,
            });
        }
    }
}

/// Quick Launch on the Reference PCs (brief 55; private/probes/quick-launch/captures.md): the bar
/// from its left edge to the notification area, band and task buttons together, as Bevel95 draws it
/// with Quick Launch on: the default band with no window (`ql-default-0`) and the band with the
/// windows of `tasks-<n>` (the task buttons where the band puts them, with no narrowed task area);
/// the band alone with each icon raised (`ql-kbd-focus-<icon>`: the keyboard's focus draws the same
/// edge as the pointer, without the pointer over it); and the band alone in GUIdebook's S14 and S11.
/// Windows 98's fourth icon, View Channels, is not Bevel95's: its cell is left empty and masked.
fn quick_launch_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::quick_launch::{self, Band, Builtin};
    use bevel_core::window_icons::{self, ClassicWindow::*};
    let band = |era: Era, hot: Option<usize>| {
        let icons = match era {
            Era::Win98 => vec![
                quick_launch::picture(era, Builtin::InternetExplorer),
                quick_launch::picture(era, Builtin::OutlookExpress),
                quick_launch::picture(era, Builtin::ShowDesktop),
                None,
            ],
            _ => vec![
                quick_launch::picture(era, Builtin::ShowDesktop),
                quick_launch::picture(era, Builtin::InternetExplorer),
                quick_launch::picture(era, Builtin::OutlookExpress),
            ],
        };
        Band { icons, hot, ..Band::default() }
    };
    let draw = |theme: &Theme, fonts: &Fonts, width: i32, tasks: Vec<Task>, tray: usize, band: Band| {
        let mut fb = Framebuffer::new(width, theme.bar.height, 0);
        let invisible = Icon { width: 16, height: 16, pixels: vec![0; 256] };
        let state = Bar {
            tasks,
            tray: vec![Some(invisible); tray],
            clock: WIDEST.to_string(),
            clock_widest: WIDEST.to_string(),
            quick_launch: Some(band),
            ..Bar::default()
        };
        let layout = paint::paint_bar(&mut fb, theme, fonts, &state);
        (fb, layout)
    };
    let sets = [
        (
            "win98",
            &WIN98,
            f95,
            Era::Win98,
            // The tasks-<n> captures have two notification icons, ql-default-0 three.
            (2, 3),
            vec![Rect::new(141, 579, 16, 16)],
            &[
                ("My Documents", MyDocuments),
                ("My Computer", MyComputer),
                ("Recycle Bin", RecycleBin),
                ("Untitled - Notepad", Notepad),
                ("Control Panel", ControlPanel),
                ("Temp", Folder),
            ][..],
            &["ie", "oe", "showdesktop", "channels"][..],
        ),
        (
            "win2000",
            &WIN2000,
            f2k,
            Era::Win2000,
            (0, 1),
            Vec::new(),
            &[
                ("My Computer", MyComputer),
                ("My Documents", MyDocuments),
                ("Recycle Bin", RecycleBin),
                ("Untitled - Notepad", Notepad),
                ("Control Panel", ControlPanel),
                ("Temp", Folder),
            ][..],
            &["showdesktop", "ie", "oe"][..],
        ),
    ];
    for (prefix, theme, fonts, era, (tray_tasks, tray_default), cells, windows, focus) in sets {
        for n in 0..=windows.len() + 1 {
            let tasks: Vec<Task> = windows[..n.min(windows.len())]
                .iter()
                .enumerate()
                .map(|(i, &(title, kind))| Task {
                    title: title.to_string(),
                    icon: Some(window_icons::icon(era, kind)),
                    state: if i + 1 == n { TaskState::Active } else { TaskState::Normal },
                })
                .collect();
            let tray = if n == 0 { tray_default } else { tray_tasks };
            let (ours, layout) = draw(theme, fonts, 800, tasks, tray, band(era, None));
            // Windows 2000 SP4's Tahoma fits one letter more before "..." in the narrowest buttons
            // (as in `task_icon_cases`).
            let mut masks = cells.clone();
            if prefix == "win2000" && n >= 6 {
                masks.extend([Rect::new(318, 578, 21, 18), Rect::new(504, 578, 21, 18)]);
            }
            let file = if n == 0 { format!("vm/{prefix}-truecolor-ql-default-0.png") } else { format!("vm/{prefix}-truecolor-tasks-{n}.png") };
            out.push(Case {
                name: format!("vm-{prefix}-quick-launch-{n}").leak(),
                file: file.leak(),
                regions: vec![region((0, 0), Rect::new(0, 572, layout.well.x, 28))],
                ours,
                masks,
                vga: false,
            });
        }
        for (k, icon) in focus.iter().enumerate() {
            let (ours, layout) = draw(theme, fonts, 800, Vec::new(), tray_default, band(era, Some(k)));
            let right = layout.quick.as_ref().expect("the band").rect.right();
            out.push(Case {
                name: format!("vm-{prefix}-quick-launch-hot-{icon}").leak(),
                file: format!("vm/{prefix}-truecolor-ql-kbd-focus-{icon}.png").leak(),
                regions: vec![region((0, 0), Rect::new(0, 572, right, 28))],
                ours,
                masks: cells.clone(),
                vga: false,
            });
        }
    }
    // Quick Launch switched off (`ql-off-<n>`: no window, the first window of `tasks-7`, its six windows;
    // the last one active): 98's and 2000's bar keeps the line after Start and one grip, the
    // task buttons start after it, 98's line before the notification area stays. The whole bar from
    // x 0 to the well.
    for (prefix, theme, fonts, era, tray, windows) in [
        (
            "win98",
            &WIN98,
            f95,
            Era::Win98,
            3,
            [("My Documents", MyDocuments), ("My Computer", MyComputer), ("Recycle Bin", RecycleBin), ("Untitled - Notepad", Notepad), ("Control Panel", ControlPanel), ("Temp", Folder)],
        ),
        (
            "win2000",
            &WIN2000,
            f2k,
            Era::Win2000,
            1,
            [("My Computer", MyComputer), ("My Documents", MyDocuments), ("Recycle Bin", RecycleBin), ("Untitled - Notepad", Notepad), ("Control Panel", ControlPanel), ("Temp", Folder)],
        ),
    ] {
        for n in [0usize, 1, 6] {
            let chosen = &windows[..n];
            let tasks: Vec<Task> = chosen
                .iter()
                .enumerate()
                .map(|(i, &(title, kind))| Task {
                    title: title.to_string(),
                    icon: Some(window_icons::icon(era, kind)),
                    state: if i + 1 == chosen.len() { TaskState::Active } else { TaskState::Normal },
                })
                .collect();
            let (ours, layout) = bar(theme, fonts, 800, false, tasks, tray, WIDEST);
            // Windows 2000 SP4's Tahoma fits one letter more before "..." ("Untitled - N..."): that
            // label's end is left out, as in `task_icon_cases`.
            let masks = if prefix == "win2000" && n == 6 { vec![Rect::new(462, 578, 21, 18)] } else { Vec::new() };
            out.push(Case {
                name: format!("vm-{prefix}-quick-launch-off-{n}").leak(),
                file: format!("vm/{prefix}-truecolor-ql-off-{n}.png").leak(),
                regions: vec![region((0, 0), Rect::new(0, 572, layout.well.x, 28))],
                ours,
                masks,
                vga: false,
            });
        }
    }
    // An icon held down (`ql-pressed-<icon>`): the edge the other way round, the icon 1 pixel right
    // and down; the pointer over it is masked.
    for (name, file, theme, fonts, era, k, pointer) in [
        ("vm-win98-quick-launch-pressed-ie", "vm/win98-truecolor-ql-pressed-ie.png", &WIN98, f95, Era::Win98, 0, Rect::new(79, 586, 15, 14)),
        ("vm-win2000-quick-launch-pressed-showdesktop", "vm/win2000-truecolor-ql-pressed-showdesktop.png", &WIN2000, f2k, Era::Win2000, 0, Rect::new(80, 582, 18, 18)),
    ] {
        let pressed = Band { pressed: Some(k), ..band(era, None) };
        let (ours, layout) = draw(theme, fonts, 800, Vec::new(), 0, pressed);
        let right = layout.quick.as_ref().expect("the band").rect.right();
        let mut masks = vec![pointer];
        if era == Era::Win98 {
            masks.push(Rect::new(141, 579, 16, 16));
        }
        out.push(Case { name, file, regions: vec![region((0, 0), Rect::new(0, 572, right, 28))], ours, masks, vga: false });
    }
    // GUIdebook's S14 (98, 640 wide) and S11 (2000, 800 wide): the band from its first line to the
    // first task button (the same default band, the bar at y 0).
    for (name, file, theme, fonts, era, width, cells) in [
        ("win98-taskbar-quick-launch", "win98-taskbar-quicklaunch-3tasks-1active-tray-640x28.png", &WIN98, f95, Era::Win98, 640, vec![Rect::new(141, 7, 16, 16)]),
        ("win2000-taskbar-quick-launch", "win2000pro-taskbar-quicklaunch-3tasks-1active-tray-800x28.png", &WIN2000, f2k, Era::Win2000, 800, Vec::new()),
    ] {
        let (ours, layout) = draw(theme, fonts, width, Vec::new(), 1, band(era, None));
        let b = layout.quick.as_ref().expect("the band").rect;
        out.push(Case { name, file, regions: vec![region((b.x, 0), Rect::new(b.x, 0, b.w, 28))], ours, masks: cells, vga: false });
    }
}

/// Quick Launch made narrower and wider on the Reference PCs (brief 70; private/probes/quick-launch/
/// resize.md, captures `<pc>-truecolor-qlsize-*`): the band from x 0 to the first task button with
/// the width the capture shows, its icons and its overflow control. The test shortcuts of the
/// captures were the PCs' own programs (games, System Tools, 98's View Channels), which Bevel95
/// does not have: their cells are drawn empty and masked; so is the pointer where it is drawn into
/// the capture. Windows 2000's list is compared whole, its rows' pictures masked.
fn quick_launch_size_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::quick_launch::{self, Band, Builtin, Control};
    // The band's first `own` icons are Bevel95's (the look's built-in ones in the capture's order),
    // the others the PCs' own programs.
    let icons = |era: Era, count: usize| -> Vec<Option<Icon>> {
        let own: Vec<Builtin> = match era {
            Era::Win98 => vec![Builtin::InternetExplorer, Builtin::OutlookExpress, Builtin::ShowDesktop],
            _ => vec![Builtin::ShowDesktop, Builtin::InternetExplorer, Builtin::OutlookExpress],
        };
        (0..count).map(|i| own.get(i).and_then(|&b| quick_launch::picture(era, b))).collect()
    };
    struct Q {
        name: &'static str,
        file: &'static str,
        era: Era,
        icons: usize,
        width: i32,
        scroll: i32,
        hot: Option<Control>,
        pressed: Option<Control>,
        list: Option<(&'static [&'static str], Option<usize>)>,
        pointer: Option<Rect>,
    }
    let q = |name, file, era, icons, width| Q { name, file, era, icons, width, scroll: 0, hot: None, pressed: None, list: None, pointer: None };
    // The names on 2000's band in the 7-icon captures (its list is as wide as the longest).
    const NAMES_7: &[&str] = &["Show Desktop", "Launch Internet Explorer Browser", "Launch Outlook Express", "FreeCell", "Minesweeper", "Pinball", "Solitaire"];
    let cases = [
        q("vm-win98-ql-size-kept-width-arrow", "vm/win98-truecolor-qlsize-8icons-3windows-band-kept-width-arrow.png", Era::Win98, 8, 107),
        Q { hot: Some(Control::Right), pointer: Some(Rect::new(154, 584, 13, 16)), ..q("vm-win98-ql-size-right-arrow-hot", "vm/win98-truecolor-qlsize-8icons-pointer-on-right-arrow.png", Era::Win98, 8, 107) },
        Q { scroll: i32::MAX, ..q("vm-win98-ql-size-scrolled-to-end", "vm/win98-truecolor-qlsize-8icons-scrolled-to-end-pointer-away.png", Era::Win98, 8, 107) },
        q("vm-win98-ql-size-narrowest", "vm/win98-truecolor-qlsize-run3-8icons-released-after-left-92.png", Era::Win98, 8, 131),
        Q { pointer: Some(Rect::new(200, 580, 16, 18)), ..q("vm-win98-ql-size-widened", "vm/win98-truecolor-qlsize-1window-released-wider-by-40.png", Era::Win98, 4, 147) },
        q("vm-win98-ql-size-widened-8", "vm/win98-truecolor-qlsize-8icons-released-wider-by-120.png", Era::Win98, 8, 251),
        q("vm-win2000-ql-size-chevron", "vm/win2000-truecolor-qlsize-7icons-released-after-left-115.png", Era::Win2000, 7, 119),
        Q { hot: Some(Control::Chevron), pointer: Some(Rect::new(170, 585, 20, 15)), ..q("vm-win2000-ql-size-chevron-hot", "vm/win2000-truecolor-qlsize-7icons-chevron-hot.png", Era::Win2000, 7, 119) },
        Q { list: Some((NAMES_7, None)), ..q("vm-win2000-ql-size-list", "vm/win2000-truecolor-qlsize-7icons-chevron-list-open-pointer-away.png", Era::Win2000, 7, 119) },
        Q { list: Some((NAMES_7, Some(1))), pointer: Some(Rect::new(246, 538, 24, 32)), ..q("vm-win2000-ql-size-list-row-hot", "vm/win2000-truecolor-qlsize-7icons-chevron-list-row-pinball-hot-2.png", Era::Win2000, 7, 119) },
        q("vm-win2000-ql-size-line-271", "vm/win2000-truecolor-qlsize-11icons-line-271-b.png", Era::Win2000, 11, 211),
        q("vm-win2000-ql-size-kept-wider", "vm/win2000-truecolor-qlsize-5icons-after-deleting-6-band-shrank.png", Era::Win2000, 5, 211),
    ];
    for c in cases {
        let (theme, fonts) = if c.era == Era::Win2000 { (&WIN2000, f2k) } else { (&WIN98, f95) };
        let band = Band {
            icons: icons(c.era, c.icons),
            width: Some(c.width),
            scroll: c.scroll,
            hot_control: c.hot,
            pressed_control: c.pressed,
            list_open: c.list.is_some(),
            ..Band::default()
        };
        let mut bar_fb = Framebuffer::new(800, theme.bar.height, 0);
        let state = Bar { clock: WIDEST.to_string(), clock_widest: WIDEST.to_string(), quick_launch: Some(band.clone()), ..Bar::default() };
        let layout = paint::paint_bar(&mut bar_fb, theme, fonts, &state);
        let ql = layout.quick.expect("the band");
        let mut screen = Screen::new(800, 600, 0);
        screen.fb.blit(&bar_fb, 0, 572);
        let mut regions = vec![Rect::new(0, 572, ql.rect.right(), 28)];
        // The PCs' own programs' cells: the part of each that shows.
        for i in 3..c.icons {
            if let Some(r) = ql.visible_button(i) {
                screen.masks.push(Rect::new(r.x, 572 + r.y, r.w, r.h));
            }
        }
        if let Some((names, hover)) = c.list {
            let chevron = ql.chevron.expect("the chevron");
            let rows = ql.hidden.iter().map(|&i| (names[i].to_string(), None)).collect();
            let all: Vec<String> = names.iter().map(|n| n.to_string()).collect();
            let menu = Menu { hover, ..quick_launch::list_menu(rows, &all) };
            let at = Rect::new(chevron.x, 572 + chevron.y, chevron.w, chevron.h);
            let (r, _) = screen.menu(theme, fonts, &menu, MenuStyle::Cascade, |w, h| quick_launch::place_list(at, w, h, Rect::new(0, 0, 800, 600)));
            regions.push(r);
        }
        if let Some(p) = c.pointer {
            screen.masks.push(p);
        }
        out.push(screen_case(c.name, c.file, screen, (0, 0), &regions));
    }
}

/// The notification area on the reference PCs (real Windows in QEMU, 800Ã—600 True Color,
/// captured exactly; `vm/` in the reference folder, see docs/theme-spec.md): with no, one and
/// two icons (masked) and at different times of day, which shows that the clock's part keeps
/// its width and centers the time.
fn tray_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    let captures: [(&'static str, &'static str, &Theme, &Fonts, usize, &str); 4] = [
        ("vm-win95-tray", "vm/win95-truecolor-desktop.png", &WIN95, f95, 0, "12:33 AM"),
        ("vm-win98-tray-1-icon", "vm/win98-truecolor-desktop.png", &WIN98, f95, 1, "12:05 AM"),
        ("vm-win98-tray-2-icons", "vm/win98-truecolor-tray-2icons.png", &WIN98, f95, 2, "7:27 AM"),
        ("vm-win2000-tray", "vm/win2000-truecolor-desktop.png", &WIN2000, f2k, 0, "12:22 AM"),
    ];
    for (name, file, theme, fonts, icons, clock) in captures {
        let (ours, layout) = bar(theme, fonts, 800, false, Vec::new(), icons, clock);
        let x = layout.well.x - 2;
        let masks = layout.tray.iter().map(|r| Rect::new(r.x, 572 + r.y, r.w, r.h)).collect();
        out.push(Case {
            name,
            file,
            regions: vec![region((x, 0), Rect::new(x, 572, 800 - x, 28))],
            ours,
            masks,
            vga: false,
        });
    }
    // Tooltips on the same Windows 98: the clock's (its long date) and Task Scheduler's, where
    // `tooltip::place`'s test puts them.
    let tips = [
        ("vm-win98-tooltip-clock", "vm/win98-truecolor-tooltip-clock.png", "Sunday, November 08, 2026", (657, 561)),
        ("vm-win98-tooltip-icon", "vm/win98-truecolor-tooltip-icon.png", "Task Scheduler is ready.", (676, 562)),
    ];
    for (name, file, text, (x, y)) in tips {
        let (w, h) = tooltip::size(f95.regular, text);
        let mut ours = Framebuffer::new(w, h, 0);
        tooltip::paint(&mut ours, &WIN98, f95.regular, text);
        out.push(Case { name, file, regions: vec![region((0, 0), Rect::new(x, y, w, h))], ours, masks: vec![], vga: false });
    }
    // Windows 2000's volume control (MS Sans Serif, 2000's colors): opened from the Run box
    // at (400, 300), where the keyboard had been used, at half volume, all the way up and down,
    // then Mute checked; and from the speaker's icon at (715, 451) with the mouse (no focus
    // rectangle until Alt), then unmuted with Alt+M.
    use bevel_core::volume::{self, Focus, VolumeControl};
    let base = VolumeControl { level: 0.5, muted: false, focus: Focus::Slider, show_keys: true, mute_pressed: false };
    let popups = [
        ("vm-win2000-volume", "vm/win2000-truecolor-volume-popup.png", (400, 300), base),
        ("vm-win2000-volume-loudest", "vm/win2000-truecolor-volume-home.png", (400, 300), VolumeControl { level: 1.0, ..base }),
        ("vm-win2000-volume-silent", "vm/win2000-truecolor-volume-end.png", (400, 300), VolumeControl { level: 0.0, ..base }),
        (
            "vm-win2000-volume-muted",
            "vm/win2000-truecolor-volume-muted.png",
            (400, 300),
            VolumeControl { level: 0.0, muted: true, focus: Focus::Mute, ..base },
        ),
        (
            "vm-win2000-volume-from-tray",
            "vm/win2000-truecolor-volume-from-tray-muted.png",
            (715, 451),
            VolumeControl { level: 0.0, muted: true, show_keys: false, ..base },
        ),
        (
            "vm-win2000-volume-from-tray-unmuted",
            "vm/win2000-truecolor-volume-from-tray.png",
            (715, 451),
            VolumeControl { level: 0.0, focus: Focus::Mute, ..base },
        ),
    ];
    // Windows 95's (its own colors; keyboard cues always shown): from the Run box at (400, 300),
    // and from the speaker, where it sits on the screen's bottom edge, its right edge at the
    // pointer (733, 589).
    let base95 = VolumeControl { show_keys: true, ..base };
    let popups95 = [
        ("vm-win95-volume", "vm/win95-truecolor-volume-popup.png", (400, 300), base95),
        ("vm-win95-volume-loudest", "vm/win95-truecolor-volume-home.png", (400, 300), VolumeControl { level: 1.0, ..base95 }),
        ("vm-win95-volume-silent", "vm/win95-truecolor-volume-end.png", (400, 300), VolumeControl { level: 0.0, ..base95 }),
        (
            "vm-win95-volume-muted",
            "vm/win95-truecolor-volume-muted.png",
            (400, 300),
            VolumeControl { level: 0.0, muted: true, focus: Focus::Mute, ..base95 },
        ),
        (
            "vm-win95-volume-from-tray",
            "vm/win95-truecolor-volume-from-tray.png",
            (652, 464),
            VolumeControl { level: 0.0, muted: true, ..base95 },
        ),
    ];
    let screen = Rect::new(0, 0, 800, 600);
    for (name, file, (x, y), v) in popups95 {
        let mut ours = Framebuffer::new(volume::SIZE.0, volume::SIZE.1, 0);
        volume::paint(&mut ours, &WIN95, f95.regular, &v);
        if name == "vm-win95-volume-from-tray" {
            assert_eq!(volume::place(Era::Win95, 733, 589, volume::SIZE.0, volume::SIZE.1, screen), (x, y));
        }
        out.push(Case { name, file, regions: vec![whole(&ours, (x, y))], ours, masks: vec![], vga: false });
    }
    // Windows 98's (95's colors too): from the speaker, the pointer's tip at (710, 586), on the
    // screen's bottom edge with its left edge at the pointer (it fits to the right there), at
    // half volume, then Mute checked with Alt+M. The pointer covers its bottom left corner.
    let pointer98 = Rect::new(710, 586, 11, 14);
    let popups98 = [
        ("vm-win98-volume-from-tray", "vm/win98-truecolor-volume-from-tray.png", base95),
        (
            "vm-win98-volume-muted",
            "vm/win98-truecolor-volume-muted.png",
            VolumeControl { muted: true, focus: Focus::Mute, ..base95 },
        ),
    ];
    for (name, file, v) in popups98 {
        let mut ours = Framebuffer::new(volume::SIZE.0, volume::SIZE.1, 0);
        volume::paint(&mut ours, &WIN98, f95.regular, &v);
        let at = volume::place(Era::Win98, 710, 586, volume::SIZE.0, volume::SIZE.1, screen);
        out.push(Case { name, file, regions: vec![whole(&ours, at)], ours, masks: vec![pointer98], vga: false });
    }
    // The speaker's menu: in Windows 95 on the screen's bottom edge like its volume control (the
    // pointer at (733, 589)); in Windows 98 at the pointer (710, 586), flipped up and left.
    for (name, file, theme, era, (x, y)) in [
        ("vm-win95-volume-menu", "vm/win95-truecolor-volume-menu.png", &WIN95, Era::Win95, (733, 589)),
        ("vm-win98-volume-menu", "vm/win98-truecolor-volume-menu.png", &WIN98, Era::Win98, (710, 586)),
    ] {
        let menu = volume::speaker_menu(true);
        let layout = paint::layout_menu(theme, f95, &menu, MenuStyle::Popup, 600);
        let mut ours = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut ours, theme, f95, &menu, MenuStyle::Popup, &layout);
        let at = volume::place_menu(era, x, y, layout.width, layout.height, screen);
        out.push(Case { name, file, regions: vec![whole(&ours, at)], ours, masks: vec![], vga: false });
    }
    for (name, file, (x, y), v) in popups {
        let mut ours = Framebuffer::new(volume::SIZE.0, volume::SIZE.1, 0);
        volume::paint(&mut ours, &WIN2000, f95.regular, &v);
        // The mouse pointer's shadow touches the bottom left of the ones opened from the tray.
        let masks = if x == 715 { vec![Rect::new(715, 585, 8, 2)] } else { Vec::new() };
        out.push(Case { name, file, regions: vec![whole(&ours, (x, y))], ours, masks, vga: false });
    }
    // (The speaker's and the taskbar's right-click menus were captured too, vm/win2000-
    // truecolor-volume-menu.png and -taskbar-menu.png, and measured for theme.rs; they are not
    // compared here: that Windows 2000's Tahoma differs from today's in a few letters' widths.)
    // The speaker in the notification area, normal and muted: Windows 2000's, and Windows 95's
    // (the same picture: its light gray pixels look like the bar's gray there).
    for (name, file, theme, fonts, muted, clock) in [
        ("vm-win2000-tray-volume", "vm/win2000-truecolor-tray-volume.png", &WIN2000, f2k, false, "8:20 AM"),
        ("vm-win2000-tray-volume-muted", "vm/win2000-truecolor-tray-volume-muted.png", &WIN2000, f2k, true, "8:19 AM"),
        ("vm-win95-tray-volume", "vm/win95-truecolor-tray-volume.png", &WIN95, f95, false, "9:34 AM"),
        ("vm-win95-tray-volume-muted", "vm/win95-truecolor-tray-volume-muted.png", &WIN95, f95, true, "9:35 AM"),
    ] {
        let mut ours = Framebuffer::new(800, theme.bar.height, 0);
        let state = Bar {
            tray: vec![Some(bevel_core::icons::speaker(muted))],
            clock: clock.to_string(),
            clock_widest: WIDEST.to_string(),
            ..Bar::default()
        };
        let layout = paint::paint_bar(&mut ours, theme, fonts, &state);
        let x = layout.well.x - 2;
        out.push(Case { name, file, regions: vec![region((x, 0), Rect::new(x, 572, 800 - x, 28))], ours, masks: vec![], vga: false });
    }
    // Windows 98 with its sound card: the display settings' icon, the speaker, Task Scheduler's
    // (in the order they came; the other two masked).
    let invisible = Icon { width: 16, height: 16, pixels: vec![0; 256] };
    let mut ours = Framebuffer::new(800, WIN98.bar.height, 0);
    let state = Bar {
        tray: vec![Some(invisible.clone()), Some(bevel_core::icons::speaker(false)), Some(invisible)],
        clock: "10:40 AM".to_string(),
        clock_widest: WIDEST.to_string(),
        ..Bar::default()
    };
    let layout = paint::paint_bar(&mut ours, &WIN98, f95, &state);
    let x = layout.well.x - 2;
    let masks = [layout.tray[0], layout.tray[2]].iter().map(|r| Rect::new(r.x, 572 + r.y, r.w, r.h)).collect();
    out.push(Case {
        name: "vm-win98-tray-volume",
        file: "vm/win98-truecolor-tray-volume.png",
        regions: vec![region((x, 0), Rect::new(x, 572, 800 - x, 28))],
        ours,
        masks,
        vga: false,
    });
}

/// Windows 98's Start menu with "Show small icons in Start menu" on, in the v86 emulator (16
/// colors; its user is called "Hi"). Its icons were cut from this same picture, so this checks
/// the layout: rows, columns, separators, arrows, the frame.
fn small_start_case(out: &mut Vec<Case>, f98: &Fonts) {
    use bevel_core::config::StartOptions;
    let menu = bevel_core::start::menu(Era::Win98, StartOptions::default(), "Hi", |e| {
        bevel_core::icons::start_icon_small(Era::Win98, e)
    });
    let menu = Menu { show_keys: true, ..menu };
    let layout = paint::layout_menu(&WIN98, f98, &menu, MenuStyle::StartSmall, 480);
    let mut ours = Framebuffer::new(layout.width, layout.height, 0);
    paint::paint_menu(&mut ours, &WIN98, f98, &menu, MenuStyle::StartSmall, &layout);
    out.push(Case {
        name: "win98-start-menu-small",
        file: "v86/w98-startmenu-small.png",
        regions: vec![whole(&ours, (2, 54))],
        ours,
        masks: Vec::new(),
        vga: true,
    });
}

/// Windows 2000's Start menu with small icons, in the v86 emulator. Its menu starts with one
/// more entry ("Set Program Access and Defaults", from a service pack) and is wider for it, so
/// our menu's rows are compared one row down, from the left edge to past the labels.
fn small_start_case_2000(out: &mut Vec<Case>, f2k: &Fonts) {
    use bevel_core::config::StartOptions;
    let opts = StartOptions { favorites: false, log_off: false, scroll_programs: false };
    let menu = bevel_core::start::menu(Era::Win2000, opts, "", |e| bevel_core::icons::start_icon_small(Era::Win2000, e));
    // Opened with Ctrl+Esc, so even Windows 2000 underlines the access keys.
    let menu = Menu { show_keys: true, ..menu };
    let layout = paint::layout_menu(&WIN2000, f2k, &menu, MenuStyle::StartSmall, 768);
    let mut ours = Framebuffer::new(layout.width, layout.height, 0);
    paint::paint_menu(&mut ours, &WIN2000, f2k, &menu, MenuStyle::StartSmall, &layout);
    let rows = layout.height - 6;
    out.push(Case {
        name: "win2000-start-menu-small",
        file: "v86/w2k-startmenu-small.png",
        regions: vec![region((0, 3), Rect::new(2, 124, 110, rows))],
        ours,
        masks: Vec::new(),
        vga: false,
    });
}

/// The Reference PCs' Start menus that show the last pictures cut from the owner's own Windows
/// (`vm/own-pictures/`, 2026-11-18; docs/decisions.md, "The last pictures from the owner's own
/// Windows"): 98's and 2000's with Log Off (98 with user profiles on, its user "Bevel95"; 2000
/// with Display Favorites and Display Logoff ticked), 2000's and 95's with small icons. Each was
/// opened with Ctrl+Esc (so the access keys show) and captured as it opened, then with each entry
/// selected in turn (`-sel-<n>`, the Down key). Every picture is compared, see-through pixels
/// included. 2000's menus begin with "Set Program Access and Defaults" (a service pack's), which
/// Bevel95 has not: its menu is one row taller and wider, so ours is compared from its first entry
/// down (the bottoms meet at the Start button), from the left edge to past the labels. 95's has
/// "Suspend" (its power management) over Shut Down: ours is compared in two parts around it.
fn own_start_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::config::StartOptions;
    use bevel_core::start::StartEntry;
    let screen_rect = Rect::new(0, 0, 800, 600);
    let draw = |theme: &Theme, fonts: &Fonts, era: Era, user: &str, small: bool, hover: Option<usize>| {
        let opts = StartOptions { favorites: true, log_off: true, scroll_programs: false };
        let icon = |e: StartEntry| {
            if small { bevel_core::icons::start_icon_small(era, e) } else { bevel_core::icons::start_icon(era, e) }
        };
        let menu = Menu { hover, show_keys: true, ..bevel_core::start::menu(era, opts, user, icon) };
        let style = if small { MenuStyle::StartSmall } else { MenuStyle::Start };
        let layout = paint::layout_menu(theme, fonts, &menu, style, 600);
        let mut fb = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut fb, theme, fonts, &menu, style, &layout);
        let (_, b) = pressed_start(theme, fonts);
        let button = Rect::new(b.x, 572 + b.y, b.w, b.h);
        let at = place_start_menu(button, layout.width, layout.height, screen_rect);
        (fb, layout, at)
    };
    // Windows 98: the same entries as Bevel95's, compared whole. The PC's Shut Down, Log Off and
    // the others as it opened, then with Log Off (9) and Shut Down (10) selected.
    for (suffix, hover) in [("", None), ("-sel-9", Some(10)), ("-sel-10", Some(11))] {
        let (ours, _, at) = draw(&WIN98, f95, Era::Win98, "Bevel95", false, hover);
        out.push(Case {
            name: format!("own-win98-start-logoff{suffix}").leak(),
            file: format!("vm/own-pictures/win98-truecolor-start-logoff{suffix}.png").leak(),
            regions: vec![whole(&ours, at)],
            ours,
            masks: vec![],
            vga: false,
        });
    }
    // Windows 2000, large and small icons: as it opened, then Favorites (4) and Log Off (10)
    // selected (our entries are one fewer: Favorites is our 3rd entry, Log Off our 9th).
    for (small, capture) in [(false, "start-logoff"), (true, "start-small")] {
        for (suffix, hover) in [("", None), ("-sel-4", Some(3)), ("-sel-10", Some(10))] {
            let (ours, layout, at) = draw(&WIN2000, f2k, Era::Win2000, "Administrator", small, hover);
            // From the left edge to past the longest label ("Log Off Administrator..."), short of
            // our submenu arrows; from our first entry's top (the frame's top rows are the PC's
            // first entry) to the bottom.
            let arrow_room = 22;
            let top = layout.rects[0].y;
            let w = ours.width - arrow_room;
            out.push(Case {
                name: format!("own-win2000-{capture}{suffix}").leak(),
                file: format!("vm/own-pictures/win2000-truecolor-{capture}{suffix}.png").leak(),
                regions: vec![region((0, top), Rect::new(at.0, at.1 + top, w, ours.height - top))],
                ours,
                masks: vec![],
                vga: false,
            });
        }
    }
    // Windows 95 with small icons: as it opened, then Documents (2) and Find (4) selected.
    for (suffix, hover) in [("", None), ("-sel-2", Some(1)), ("-sel-4", Some(3))] {
        let (ours, layout, at) = draw(&WIN95, f95, Era::Win95, "", true, hover);
        let shut_down = *layout.rects.last().unwrap();
        let suspend = shut_down.h;
        // The PC's menu is 127 pixels wide, its arrows 9 from the edge; ours is 121 (Bevel95's 95
        // small-menu width is a PROVISIONAL guess: theme-spec.md, private/todo.md): compared from the
        // left edge to past the labels, short of the submenu arrows.
        let w = ours.width - 22;
        let above = Rect::new(at.0, at.1 - suspend, w, shut_down.y);
        let below = Rect::new(at.0, at.1 + shut_down.y, w, ours.height - shut_down.y);
        out.push(Case {
            name: format!("own-win95-start-small{suffix}").leak(),
            file: format!("vm/own-pictures/win95-truecolor-start-small{suffix}.png").leak(),
            regions: vec![region((0, 0), above), region((0, shut_down.y), below)],
            ours,
            masks: vec![],
            vga: false,
        });
    }
}

/// The Reference PCs' Programs menus longer than the screen (brief 91; survey part 1,
/// `private/probes/start-long`), opened with the keyboard on an 800x600 screen (the first entry
/// chosen), each laid out by `layout_long_menu` as the bar lays it out and compared whole where
/// the PC put it. The entries are the survey's: the PC's own folders and programs and the test
/// entries "Entry nnn" (copies of Notepad's shortcut); every picture cell is masked (the PC's
/// program pictures, which the core does not draw).
/// - Windows 98, 37 entries: one column the screen's height, scrolled to its top (its own order:
///   later entries at the end), at x 165.
/// - Windows 95, 50 entries: two columns at x 160 beside the Start menu, each as wide as its
///   widest entry, the etched line between.
/// - Windows 2000, 59 entries: three columns at x 250.
fn long_menu_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    let entries = |first: usize, n: usize| (first..first + n).map(|i| format!("Entry {i:03}")).collect::<Vec<_>>();
    let with = |head: &[&str], tail: Vec<String>, own: &[&str]| {
        let mut l: Vec<String> = head.iter().map(|s| s.to_string()).collect();
        l.extend(tail);
        l.extend(own.iter().map(|s| s.to_string()));
        l
    };
    // 98: its own order, then what was added later, in the order it was made.
    let h98 = ["Accessories>", "Internet Explorer>", "Online Services>", "StartUp>", "MS-DOS Prompt", "Windows Explorer", "Long Folder>"];
    // 95: folders, then the entries, then the PC's own programs, by name.
    let h95 = ["Accessories>", "Long Folder>", "Online Services>", "StartUp>"];
    let o95 = ["Internet Explorer", "Internet Mail", "Internet News", "Microsoft NetMeeting", "MS-DOS Prompt", "Windows Explorer"];
    // 2000: the same rule, its entries numbered from 100.
    let h2k = ["Accessories>", "Long Folder>", "Startup>"];
    let o2k = ["Internet Explorer", "Outlook Express"];
    let scroll98 = Long::Scroll { flat: false };
    let scroll2k = Long::Scroll { flat: true };
    let cols95 = Long::Columns { own_widths: true };
    // (name, capture, theme, fonts, labels, rule, x (None: the screen's left edge), screen height,
    // the last entry chosen (else the first), scrolled to the end)
    let cases = vec![
        ("own-win98-long-programs-37", "vm/win98-truecolor-startlong-800x600-programs-36entries-open.png", &WIN98, f95, with(&h98, entries(0, 30), &[]), scroll98, Some(165), 600, false, false),
        ("own-win98-long-programs-57-end", "vm/win98-truecolor-startlong-800x600-programs-57entries-end-last-selected.png", &WIN98, f95, with(&h98, entries(0, 50), &[]), scroll98, Some(165), 600, true, true),
        ("own-win98-long-programs-117-end", "vm/win98-truecolor-startlong-800x600-programs-117entries-end-last-selected.png", &WIN98, f95, with(&h98, entries(0, 110), &[]), scroll98, Some(165), 600, true, true),
        ("own-win98-long-programs-117-640-end", "vm/win98-truecolor-startlong-640x480-programs-117entries-end-last-selected.png", &WIN98, f95, with(&h98, entries(0, 110), &[]), scroll98, Some(165), 480, true, true),
        ("own-win95-long-programs-50", "vm/win95-truecolor-startlong-800x600-programs-50entries-open.png", &WIN95, f95, with(&h95, entries(0, 40), &o95), cols95, Some(160), 600, false, false),
        ("own-win95-long-programs-80-640", "vm/win95-truecolor-startlong-640x480-programs-80entries-open.png", &WIN95, f95, with(&h95, entries(0, 70), &o95), cols95, None, 480, false, false),
        ("own-win2000-long-programs-59-scroll", "vm/win2000-truecolor-startlong-800x600-programs-59entries-scroll-on-open-2.png", &WIN2000, f2k, with(&h2k, entries(100, 54), &o2k), scroll2k, Some(250), 600, false, false),
        ("own-win2000-long-programs-59-scroll-end", "vm/win2000-truecolor-startlong-800x600-programs-59entries-scroll-on-scrolled-to-end.png", &WIN2000, f2k, with(&h2k, entries(100, 54), &o2k), scroll2k, Some(250), 600, false, true),
    ];
    // Windows 2000's columns are not compared: their widths at 800x600 with 59 entries (130, 129
    // and 126; the lines at 384 and 517) follow neither the menu's widest entry nor each column's
    // own. Tried in brief 92: each column's own widest (the reviewer's idea; the arrow's room is
    // the same with and without arrows in 2000) gives a first column of about 105; the widest of a
    // column and every later one, a pixel more where it opens a submenu (ours) gives 132, 131 and
    // 128, lines at 386 and 521. Bevel95 draws them all as wide as the menu's widest (PROVISIONAL).
    for (name, file, theme, fonts, labels, long, x, h, last, end) in cases {
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let hover = if last { labels.len() - 1 } else { 0 };
        let m = menu(&labels, Some(hover), true);
        let layout = paint::layout_long_menu(theme, fonts, &m, MenuStyle::Cascade, h, long, if end { i32::MAX } else { 0 });
        let mut ours = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut ours, theme, fonts, &m, MenuStyle::Cascade, &layout);
        let (x, y) = (x.unwrap_or(0), h - layout.height);
        let items = &theme.start_menu.small;
        let (top, bottom) = layout.view();
        let mut masks: Vec<Rect> = (0..m.entries.len())
            .filter_map(|i| layout.shown_rect(i))
            .map(|r| Rect::new(x + r.x + items.icon_x, y + r.y + (r.h - items.icon) / 2, items.icon, items.icon))
            .map(|r| {
                // A picture cell cut by a strip: only its part in view.
                let t = r.y.max(y + top);
                Rect::new(r.x, t, r.w, (r.bottom().min(y + bottom) - t).max(0))
            })
            .collect();
        // The PC's mouse pointer where it rested (95's capture: in the second column's empty part).
        if name == "own-win95-long-programs-50" {
            masks.push(Rect::new(400, 300, 11, 19));
        }
        // 2000's capture scrolled to the end: the pointer rests on the last entry's label (its tip
        // at 346, 593), and the label's three rows just above it.
        if name == "own-win2000-long-programs-59-scroll-end" {
            masks.push(Rect::new(346, 590, 14, 10));
        }
        out.push(Case { name, file, regions: vec![whole(&ours, (x, y))], ours, masks, vga: false });
    }
    start_scroll_cases(out, f95);
}

/// A text field's right-click menu (brief 95; survey part 3), drawn by the look's popup menu from
/// `edit::Edit::menu`, against the Reference PCs' captures of the Run box's menu. The menu's place
/// on the screen is the capture's (found by its frame).
fn field_menu_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::edit::{Edit, Op};
    // (name, capture, look, fonts, the field's text, characters selected from its start, the
    // clipboard holds text, where the menu's top left corner is)
    type Row<'a> = (&'static str, &'static str, &'a Theme, &'a Fonts<'a>, &'a str, usize, bool, (i32, i32));
    let cases: [Row; 6] = [
        // 98: "exp" of "explorer /e,c:\windows\media" selected; the clipboard held text.
        ("own-win98-field-menu-run-part-selected", "vm/win98-truecolor-address-run-box-right-click-menu-exp-selected.png", &WIN98, f95, "explorer /e,c:\\windows\\media", 3, true, (200, 370)),
        // 98's address field, all of it selected (Select All grey), then nothing selected.
        ("own-win98-field-menu-address-all-selected", "vm/win98-truecolor-address-folder-field-right-click-menu-all-selected.png", &WIN98, f95, "C:\\WINDOWS\\MEDIA", 16, true, (273, 233)),
        ("own-win98-field-menu-address-nothing-selected", "vm/win98-truecolor-address-folder-field-right-click-menu-nothing-selected.png", &WIN98, f95, "C:\\WINDOWS\\MEDIA", 0, true, (273, 233)),
        // 2000, Shift+F10: "c:" of "c:\winnt\help" selected; the clipboard held text.
        ("own-win2000-field-menu-run-part-selected", "vm/win2000-truecolor-address-run-box-shift-f10-menu-part-selected.png", &WIN2000, f2k, "c:\\winnt\\help", 2, true, (191, 355)),
        // 2000's address field ("Media"): part of it selected, by the mouse; and Shift+F10 with
        // all of it selected and the clipboard empty (Paste and Select All grey).
        ("own-win2000-field-menu-address-part-selected", "vm/win2000-truecolor-address-folder-field-mouse-right-click-part-selected-3.png", &WIN2000, f2k, "Media", 2, true, (204, 189)),
        ("own-win2000-field-menu-address-all-selected-clipboard-empty", "vm/win2000-truecolor-address-folder-field-shift-f10-menu-all-selected-clipboard-empty.png", &WIN2000, f2k, "Media", 5, false, (454, 195)),
    ];
    for (name, file, theme, fonts, text, selected, clipboard, (x, y)) in cases {
        let mut edit = Edit::new(text);
        edit.apply(Op::Home, false);
        for _ in 0..selected {
            edit.apply(Op::Right, true);
        }
        let mut menu = edit.menu(clipboard);
        // Opened by the mouse, Windows 2000 underlines no access keys (as its other menus).
        menu.show_keys = !name.ends_with("address-part-selected");
        let layout = paint::layout_menu(theme, fonts, &menu, MenuStyle::Popup, 600);
        let mut ours = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut ours, theme, fonts, &menu, MenuStyle::Popup, &layout);
        // 98's address-field menus: the mouse pointer lies over the menu's top left corner.
        let masks = match name {
            n if n.starts_with("own-win98-field-menu-address") => vec![Rect::new(272, 232, 14, 22)],
            // The mouse pointer at the menu's corner.
            "own-win2000-field-menu-address-part-selected" => vec![Rect::new(203, 188, 18, 26)],
            // Here the PC drew the grey Undo without its dotted access-key underline, which the
            // Run box's menu (above, exact) has: not understood, the 6 pixels masked.
            "own-win2000-field-menu-address-all-selected-clipboard-empty" => vec![Rect::new(475, 211, 6, 2)],
            _ => Vec::new(),
        };
        out.push(Case { name, file, regions: vec![whole(&ours, (x, y))], ours, masks, vga: false });
    }
}

/// Windows 98's own Start menu when it scrolls (brief 92): the PC's menu after MS-DOS Prompt was
/// dropped on top of it. It kept its height (347, from y 229 down to the bar), so its entries no
/// longer fit and it scrolls, shown at its top: the banner stands still and whole, the bottom strip
/// covers the entries' part only. Laid out by `layout_long_menu` for a "screen" of the menu's own
/// height (Bevel95 meets the same layout on a short screen). The pictures are masked (the dropped
/// entry's is the PC's). The PC's menu has Windows 98's plain Log Off... (no user name: with one,
/// as on the PC's other captures, the menu is 4 pixels wider than this one).
fn start_scroll_cases(out: &mut Vec<Case>, f95: &Fonts) {
    {
        let (name, user, log_off) = ("own-win98-start-scrolls", "", false);
        let opts = bevel_core::config::StartOptions { favorites: true, log_off, scroll_programs: false };
        let mut m = bevel_core::start::menu(Era::Win98, opts, user, |e| bevel_core::icons::start_icon(Era::Win98, e));
        m.entries.insert(0, MenuEntry::item("MS-DOS Prompt", None));
        m.show_keys = true;
        let layout = paint::layout_long_menu(&WIN98, f95, &m, MenuStyle::Start, 347, Long::Scroll { flat: false }, 0);
        let mut ours = Framebuffer::new(layout.width, layout.height, 0);
        paint::paint_menu(&mut ours, &WIN98, f95, &m, MenuStyle::Start, &layout);
        let (x, y) = (2, 229);
        let items = &WIN98.start_menu.large;
        let (top, bottom) = layout.view();
        let masks: Vec<Rect> = (0..m.entries.len())
            .filter(|&i| m.entries[i].kind != EntryKind::Separator)
            .filter_map(|i| layout.shown_rect(i))
            .map(|r| Rect::new(x + r.x + items.icon_x, y + r.y + (r.h - items.icon) / 2, items.icon, items.icon))
            .map(|r| {
                let t = r.y.max(y + top);
                Rect::new(r.x, t, r.w, (r.bottom().min(y + bottom) - t).max(0))
            })
            .collect();
        // The PC's pressed Start button lies over the menu's bottom left corner (x 2..53, y 572..575).
        let mut masks = masks;
        masks.push(Rect::new(2, 572, 52, 4));
        let file = "vm/win98-truecolor-startdrag-800x600-startmenu-with-dropped-msdosprompt-on-top-scrolls.png";
        out.push(Case { name, file, regions: vec![whole(&ours, (x, y))], ours, masks, vga: false });
    }
}

/// Taskbar Properties on the Reference PCs (`vm/own-pictures/<pc>-truecolor-props-<state>.png`,
/// 2026-11-18), opened from Start > Settings with the keyboard (the access keys show), at (2, 168)
/// on the screen: as it opened, each check box changed in turn with its access key (the preview
/// shows that option; Apply is on from then), and the second page (Ctrl+Tab; the focus on Add...).
/// The whole window is compared, the preview's pieces included: 95's own pictures in the 95 look,
/// Internet Explorer 4's in 98's, 2000's.
fn own_properties_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::config::{StartOptions, TaskbarOptions};
    use bevel_core::properties::{self, Check, Part, Properties, Sheet, Values};
    let o = TaskbarOptions::default();
    let states: [(&str, usize, TaskbarOptions, Part, bool); 6] = [
        ("options", 0, o, Part::Check(Check::AlwaysOnTop), false),
        ("small", 0, TaskbarOptions { small_icons: true, ..o }, Part::Check(Check::SmallIcons), true),
        ("noclock", 0, TaskbarOptions { show_clock: false, ..o }, Part::Check(Check::ShowClock), true),
        ("autohide", 0, TaskbarOptions { auto_hide: true, ..o }, Part::Check(Check::AutoHide), true),
        ("nottop", 0, TaskbarOptions { always_on_top: false, ..o }, Part::Check(Check::AlwaysOnTop), true),
        ("smp", 1, o, Part::Add, true),
    ];
    for era in Era::ALL {
        let (theme, fonts, pc) = match era {
            Era::Win95 => (&WIN95, f95, "win95"),
            Era::Win98 => (&WIN98, f95, "win98"),
            Era::Win2000 => (&WIN2000, f2k, "win2000"),
        };
        for (state, page, options, focus, changed) in states {
            // 2000 calls its pages General and Advanced.
            let capture = match (era, state) {
                (Era::Win2000, "options") => "general",
                (Era::Win2000, "smp") => "advanced",
                _ => state,
            };
            let values = Values {
                options,
                // The PCs' settings when captured: 2000's Display Favorites and Display Logoff off.
                start: StartOptions { favorites: false, log_off: era != Era::Win2000, scroll_programs: false },
                era,
                colors: bevel_core::theme::Depth::TrueColor,
                scale: bevel_core::scale::Scale::whole(2),
                start_with_windows: false,
                clock_offset: 0,
                desktop: true,
                programs: bevel_core::programs::Shown::Everything,
                windows_key: bevel_core::config::WindowsKey::Find,
                quick_search_all: false,
                battery_percent: false,
                windows_display: false,
                monitor_lines: bevel_core::monitor_lines::Strength::Off,
                quick_launch: false,
                group: true,
            };
            let mut p = Properties::new(Sheet::Taskbar, era, values);
            p.page = page;
            p.focus = focus;
            p.changed = changed;
            p.show_keys = true;
            let l = properties::layout(&p, fonts);
            let mut ours = Framebuffer::new(l.width, l.height, 0);
            properties::paint(&mut ours, theme, fonts, &p, &l);
            let mut masks = Vec::new();
            match era {
                // Windows 2000: "Use Personalized Menus" (page 1) and the Start Menu Settings
                // list (page 2) are drawn differently by Bevel95 (as in the v86 cases).
                Era::Win2000 if page == 0 => masks.push(Rect::new(33, 323, 140, 19)),
                Era::Win2000 => masks.extend([Rect::new(26, 274, 277, 16), Rect::new(26, 322, 277, 21)]),
                // 95 and 98: under the selected tab the PCs leave the face color where Bevel95
                // draws the page's light edge (DFDFDF; the v86 captures were 16 colors, where it is
                // the face color too). Not a picture: private/todo.md.
                _ if page == 0 => masks.push(Rect::new(11, 50, 92, 1)),
                _ => masks.push(Rect::new(99, 50, 115, 1)),
            }
            if era == Era::Win95 {
                // Plain Windows 95's words, where Bevel95's 95 look has Internet Explorer 4's
                // (private/todo.md): "Show Clock"; "Customize Start Menu", "You may customize your
                // Start Menu by", "Documents Menu" and its two lines.
                if page == 0 {
                    masks.push(Rect::new(52, 316, 70, 18));
                } else {
                    masks.extend([
                        Rect::new(108, 62, 32, 12),
                        Rect::new(112, 92, 166, 14),
                        Rect::new(89, 209, 33, 12),
                        Rect::new(90, 227, 183, 40),
                    ]);
                }
            }
            // In the window's coordinates above; the window is at (2, 168) on the capture.
            let masks = masks.into_iter().map(|m| Rect::new(m.x + 2, m.y + 168, m.w, m.h)).collect();
            out.push(Case {
                name: format!("own-{pc}-properties-{state}").leak(),
                file: format!("vm/own-pictures/{pc}-truecolor-props-{capture}.png").leak(),
                regions: vec![whole(&ours, (2, 168))],
                ours,
                masks,
                vga: false,
            });
        }
    }
}

/// Windows' "Remove Shortcuts/Folders" window (Taskbar Properties, Remove...) on the Reference
/// PCs: as it opens, and as left by the keyboard with folders opened (Down, Right). The whole
/// window is compared: frame and title bar, the text, the tree (lines, plus and minus boxes,
/// the folders' pictures, the names, the selection with its focus rectangle), the scroll bar
/// and the buttons. Masked: the shortcuts' pictures, which are each PC's own programs'.
fn remove_window_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::kit::Form;
    use bevel_core::programs::{self, Row, RowKind};
    use bevel_core::remove::{Part, RemoveWindow};
    use bevel_core::tree::{self, TreeStyle};
    /// What a row is: a folder with its rows showing, with them put away or with none, or a
    /// program.
    #[derive(Clone, Copy, PartialEq)]
    enum What {
        Open,
        Closed,
        Empty,
        Program,
    }
    use What::*;
    struct Capture {
        name: &'static str,
        file: &'static str,
        era: Era,
        /// The rows that show, from the tree's first: how many folders each is in, its name,
        /// what it is.
        lines: Vec<(usize, &'static str, What)>,
        /// Which of them is selected, and how many are scrolled out above the tree.
        selected: usize,
        top: usize,
        /// What has the focus: the tree, or (after Tab) one of the buttons.
        focus: Part,
    }
    let captures = [
        // Windows 95 as the window opens: Programs selected, so Remove is grayed out.
        Capture {
            name: "vm-win95-remove-opened",
            file: "vm/win95-truecolor-remove-tree-1.png",
            era: Era::Win95,
            lines: vec![
                (0, "Start Menu", Open),
                (1, "Programs", Open),
                (2, "Accessories", Closed),
                (2, "Online Services", Closed),
                (2, "StartUp", Empty),
                (2, "Internet Explorer", Program),
                (2, "Internet Mail", Program),
                (2, "Internet News", Program),
                (2, "Microsoft NetMeeting", Program),
                (2, "MS-DOS Prompt", Program),
                (2, "Windows Explorer", Program),
            ],
            selected: 1,
            top: 0,
            focus: Part::Tree,
        },
        // Windows 95, Accessories open and selected: 22 rows, and the tree has moved one row
        // up to show what the folder holds ("Start Menu" is out of sight).
        Capture {
            name: "vm-win95-remove-accessories",
            file: "vm/win95-truecolor-remove-tree-2.png",
            era: Era::Win95,
            lines: vec![
                (0, "Start Menu", Open),
                (1, "Programs", Open),
                (2, "Accessories", Open),
                (3, "Internet Tools", Closed),
                (3, "Multimedia", Closed),
                (3, "System Tools", Closed),
                (3, "Calculator", Program),
                (3, "HyperTerminal", Program),
                (3, "Imaging", Program),
                (3, "Notepad", Program),
                (3, "Online Registration", Program),
                (3, "Paint", Program),
                (3, "Phone Dialer", Program),
                (3, "WordPad", Program),
                (2, "Online Services", Closed),
                (2, "StartUp", Empty),
                (2, "Internet Explorer", Program),
                (2, "Internet Mail", Program),
                (2, "Internet News", Program),
                (2, "Microsoft NetMeeting", Program),
                (2, "MS-DOS Prompt", Program),
                (2, "Windows Explorer", Program),
            ],
            selected: 2,
            top: 1,
            focus: Part::Tree,
        },
        // Windows 98 as the window opens. Its Start menu has a program of its own beside
        // Programs (Windows Update), so a line goes on down from Programs.
        Capture {
            name: "vm-win98-remove-opened",
            file: "vm/win98-truecolor-remove-tree-1.png",
            era: Era::Win98,
            lines: vec![
                (0, "Start Menu", Open),
                (1, "Programs", Open),
                (2, "Accessories", Closed),
                (2, "Internet Explorer", Closed),
                (2, "Online Services", Closed),
                (2, "StartUp", Empty),
                (2, "MS-DOS Prompt", Program),
                (2, "Windows Explorer", Program),
                (1, "Windows Update", Program),
            ],
            selected: 1,
            top: 0,
            focus: Part::Tree,
        },
        // Windows 98, Accessories open and selected: 19 rows, the scroll bar appears.
        Capture {
            name: "vm-win98-remove-accessories",
            file: "vm/win98-truecolor-remove-tree-2.png",
            era: Era::Win98,
            lines: vec![
                (0, "Start Menu", Open),
                (1, "Programs", Open),
                (2, "Accessories", Open),
                (3, "Accessibility", Closed),
                (3, "Communications", Closed),
                (3, "Entertainment", Closed),
                (3, "Games", Closed),
                (3, "System Tools", Closed),
                (3, "Calculator", Program),
                (3, "Imaging", Program),
                (3, "Notepad", Program),
                (3, "Paint", Program),
                (3, "WordPad", Program),
                (2, "Internet Explorer", Closed),
                (2, "Online Services", Closed),
                (2, "StartUp", Empty),
                (2, "MS-DOS Prompt", Program),
                (2, "Windows Explorer", Program),
                (1, "Windows Update", Program),
            ],
            selected: 2,
            top: 0,
            focus: Part::Tree,
        },
        // Windows 98, Accessories and its Games open, Games selected: four levels of lines.
        Capture {
            name: "vm-win98-remove-games",
            file: "vm/win98-truecolor-remove-tree-3.png",
            era: Era::Win98,
            lines: vec![
                (0, "Start Menu", Open),
                (1, "Programs", Open),
                (2, "Accessories", Open),
                (3, "Accessibility", Closed),
                (3, "Communications", Closed),
                (3, "Entertainment", Closed),
                (3, "Games", Open),
                (4, "FreeCell", Program),
                (4, "Hearts", Program),
                (4, "Minesweeper", Program),
                (4, "Solitaire", Program),
                (3, "System Tools", Closed),
                (3, "Calculator", Program),
                (3, "Imaging", Program),
                (3, "Notepad", Program),
                (3, "Paint", Program),
                (3, "WordPad", Program),
                (2, "Internet Explorer", Closed),
                (2, "Online Services", Closed),
                (2, "StartUp", Empty),
                (2, "MS-DOS Prompt", Program),
                (2, "Windows Explorer", Program),
                (1, "Windows Update", Program),
            ],
            selected: 6,
            top: 0,
            focus: Part::Tree,
        },
        // Windows 2000 as the window opens (Tahoma; its own colors).
        Capture {
            name: "vm-win2000-remove-opened",
            file: "vm/win2000-truecolor-remove-tree-1.png",
            era: Era::Win2000,
            lines: vec![
                (0, "Start Menu", Open),
                (1, "Programs", Open),
                (2, "Accessories", Closed),
                (2, "Startup", Empty),
                (2, "Internet Explorer", Program),
                (2, "Outlook Express", Program),
            ],
            selected: 1,
            top: 0,
            focus: Part::Tree,
        },
        // Windows 2000, Accessories open and selected: 15 rows.
        Capture {
            name: "vm-win2000-remove-accessories",
            file: "vm/win2000-truecolor-remove-tree-2.png",
            era: Era::Win2000,
            lines: vec![
                (0, "Start Menu", Open),
                (1, "Programs", Open),
                (2, "Accessories", Open),
                (3, "Accessibility", Closed),
                (3, "Entertainment", Empty),
                (3, "System Tools", Closed),
                (3, "Address Book", Program),
                (3, "Command Prompt", Program),
                (3, "Imaging", Program),
                (3, "Notepad", Program),
                (3, "Synchronize", Program),
                (3, "Windows Explorer", Program),
                (2, "Startup", Empty),
                (2, "Internet Explorer", Program),
                (2, "Outlook Express", Program),
            ],
            selected: 2,
            top: 0,
            focus: Part::Tree,
        },
    ];
    // Each window as it opens, after Down (Accessories selected, so Remove can be used), then
    // after Tab (the focus on Remove: the tree's selection stays, on gray; Remove is the framed
    // button), then after a second Tab (the focus on Close).
    let mut captures = Vec::from(captures);
    for (pc, era, opened) in [("win95", Era::Win95, 0), ("win98", Era::Win98, 2), ("win2000", Era::Win2000, 5)] {
        let states = [("tree", Part::Tree), ("remove", Part::Remove), ("close", Part::Close)];
        for (n, (state, focus)) in states.into_iter().enumerate() {
            captures.push(Capture {
                name: format!("vm-{pc}-remove-focus-{state}").leak(),
                file: format!("vm/{pc}-truecolor-remove-focus-{n}.png").leak(),
                era,
                lines: captures[opened].lines.clone(),
                selected: 2,
                top: 0,
                focus,
            });
        }
    }
    // Where the window stands on the Reference PCs' screens: (33, 50) into Taskbar Properties.
    let at = (35, 218);
    for capture in captures {
        let (theme, fonts) = match capture.era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        let mut rows = Vec::new();
        let mut selected = 0;
        for (i, &(depth, name, what)) in capture.lines.iter().enumerate() {
            if i == capture.selected {
                selected = rows.len();
            }
            let row = |depth: usize, kind: RowKind, name: &str| Row {
                depth,
                kind,
                name: name.to_string(),
                place: name.to_string(),
                ticked: false,
                own: false,
                icon: None,
                open: what == Open,
                last: false,
            };
            let kind = match what {
                Program => RowKind::Item,
                _ if i < 2 => RowKind::Root,
                _ => RowKind::Folder,
            };
            rows.push(row(depth, kind, name));
            // A closed folder's plus box says that it has rows: one stands for them here.
            if what == Closed {
                rows.push(row(depth + 1, RowKind::Item, "put away"));
            }
        }
        programs::mark_last(&mut rows);
        let mut window = RemoveWindow::new(capture.era, rows);
        (window.sel, window.top, window.focus) = (selected, capture.top, capture.focus);
        // Opened from the keyboard, so even Windows 2000 shows the access keys.
        window.show_keys = true;
        window.set_screen(at.0, at.1);
        window.layout(fonts);
        let (w, h) = window.size();
        let mut ours = Framebuffer::new(w, h, 0);
        window.paint(&mut ours, theme, fonts);
        // The shortcuts' pictures, but for the arrow in their lower left corner (7Ã—7), which is
        // compared. The tree's inside begins (20, 76) into the window and is 220 high: the 14th
        // row shows only its upper part.
        let style = TreeStyle::of(capture.era);
        let mut masks = Vec::new();
        for (n, &(depth, _, what)) in capture.lines.iter().enumerate().skip(capture.top).take(14) {
            let y = (n - capture.top) as i32 * tree::ROW_H;
            if what == Program {
                let (x, y, h) = (at.0 + 20 + style.content_x(depth), at.1 + 76 + y, (220 - y).min(16));
                masks.extend([Rect::new(x, y, 16, h.min(9)), Rect::new(x + 7, y + 9, 9, (h - 9).max(0))]);
            }
        }
        let regions = vec![whole(&ours, at)];
        out.push(Case { name: capture.name, file: capture.file, regions, ours, masks, vga: false });
    }
}

/// The check boxes that Bevel95 Settings' tree of programs adds to Windows' tree, each where a
/// capture has one. Windows 98's tree in Folder Options (View) has the flat box of 13, ticked
/// and empty. Windows 95's Windows Setup list has the box of 11: ticked, shaded (only part of
/// the component is installed) and empty.
fn tree_cases(out: &mut Vec<Case>) {
    use bevel_core::programs::Tick;
    use bevel_core::tree::{self, BoxKind};
    type Boxes = (&'static str, &'static str, &'static Theme, BoxKind, i32, Vec<(Tick, (i32, i32))>);
    let boxes: [Boxes; 2] = [
        (
            "vm-win98-tree-check-boxes",
            "vm/win98-truecolor-folder-options-view.png",
            &WIN98,
            BoxKind::Flat,
            13,
            vec![(Tick::On, (73, 223)), (Tick::Off, (73, 239))],
        ),
        (
            "vm-win95-list-check-boxes",
            "vm/win95-truecolor-windows-setup-list.png",
            &WIN95,
            BoxKind::Setup,
            11,
            vec![(Tick::On, (72, 172)), (Tick::Mixed, (72, 190)), (Tick::Off, (72, 226))],
        ),
    ];
    for (name, file, theme, kind, size, states) in boxes {
        let mut ours = Framebuffer::new(size, 16 * states.len() as i32, rgb(0xFFFFFF));
        let mut regions = Vec::new();
        for (n, &(tick, at)) in states.iter().enumerate() {
            let y = 16 * n as i32;
            tree::paint_box(&mut ours, &theme.colors, kind, 0, y, tick);
            regions.push(region((0, y), Rect::new(at.0, at.1, size, size)));
        }
        out.push(Case { name, file, regions, ours, masks: vec![], vga: false });
    }
}

/// Bevel95's desktop is one plain color, each look's own: the reference PCs' desktops as first
/// installed (the same captures as the notification area's; their right part, which has no
/// icons and no pointer).
fn desktop_cases(out: &mut Vec<Case>) {
    let captures: [(&'static str, &'static str, &Theme); 3] = [
        ("vm-win95-desktop-color", "vm/win95-truecolor-desktop.png", &WIN95),
        ("vm-win98-desktop-color", "vm/win98-truecolor-desktop.png", &WIN98),
        ("vm-win2000-desktop-color", "vm/win2000-truecolor-desktop.png", &WIN2000),
    ];
    for (name, file, theme) in captures {
        let area = Rect::new(480, 20, 300, 540);
        let ours = Framebuffer::new(area.w, area.h, theme.colors.desktop);
        out.push(Case { name, file, regions: vec![region((0, 0), area)], ours, masks: vec![], vga: false });
    }
}

/// The classic desktop with a wallpaper or a pattern against the Reference PCs' desktops after
/// Apply (`vm/<pc>-truecolor-display-desk-*`, private/probes/display/survey.md, section 4): the
/// screen right of the icons (x 100 to 799, down to the taskbar), drawn by `display::background`
/// over the whole 800x600 screen, and each of Bevel95's own icons in its 75x75 cell where the PC
/// shows it, label box and selection included, over the same picture. Left out: the PCs' other
/// icons (Inbox, My Network Places, ...), which Bevel95 does not have.
fn wallpaper_desktop_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::desktop_icons::Desktop;
    use bevel_core::display::{self, Choice, Show};
    // (capture, look, wallpaper, how, pattern, selected icon (ours), focus frame shown)
    type Shot = (&'static str, Era, Option<&'static str>, Show, Option<&'static str>, Option<usize>);
    let shots: &[Shot] = &[
        ("win95-truecolor-display-desk-bubbles-tile", Era::Win95, Some("Bubbles"), Show::Tile, None, None),
        ("win95-truecolor-display-desk-strawmat-tile", Era::Win95, Some("Straw Mat"), Show::Tile, None, None),
        ("win95-truecolor-display-desk-setup-tile", Era::Win95, Some("Setup"), Show::Tile, None, None),
        ("win95-truecolor-display-desk-pattern-plaid", Era::Win95, None, Show::Center, Some("Plaid"), None),
        ("win95-truecolor-display-desk-pattern-bricks", Era::Win95, None, Show::Center, Some("Bricks"), None),
        ("win95-truecolor-display-desk-pattern-waffles-revenge", Era::Win95, None, Show::Center, Some("Waffle's Revenge"), None),
        ("win95-truecolor-display-desk-bubbles-center-pattern-plaid", Era::Win95, Some("Bubbles"), Show::Center, Some("Plaid"), None),
        ("win95-truecolor-display-desk-setup-center-pattern-plaid", Era::Win95, Some("Setup"), Show::Center, Some("Plaid"), None),
        ("win98-truecolor-display-desk-bubbles-tile", Era::Win98, Some("Bubbles"), Show::Tile, None, None),
        ("win98-truecolor-display-desk-clouds-tile", Era::Win98, Some("Clouds"), Show::Tile, None, None),
        ("win98-truecolor-display-desk-forest-tile", Era::Win98, Some("Forest"), Show::Tile, None, None),
        ("win98-truecolor-display-desk-forest-tile-selected", Era::Win98, Some("Forest"), Show::Tile, None, Some(0)),
        ("win98-truecolor-display-desk-clouds-stretch", Era::Win98, Some("Clouds"), Show::Stretch, None, None),
        ("win98-truecolor-display-desk-pattern-bricks-2", Era::Win98, None, Show::Center, Some("Bricks"), None),
        ("win98-truecolor-display-desk-pattern-bricks-selected-2", Era::Win98, None, Show::Center, Some("Bricks"), Some(0)),
        ("win98-truecolor-display-desk-pattern-daisies", Era::Win98, None, Show::Center, Some("Daisies"), None),
        ("win98-truecolor-display-desk-pattern-tile", Era::Win98, None, Show::Center, Some("Tile"), None),
        ("win98-truecolor-display-desk-pattern-waffles-revenge", Era::Win98, None, Show::Center, Some("Waffle's Revenge"), None),
        ("win98-truecolor-display-desk-redblocks-center", Era::Win98, Some("Red Blocks"), Show::Center, None, None),
        ("win98-truecolor-display-desk-redblocks-center-pattern-waffles-revenge", Era::Win98, Some("Red Blocks"), Show::Center, Some("Waffle's Revenge"), None),
        ("win98-truecolor-display-desk-redblocks-center-pattern-waffles-revenge-selected", Era::Win98, Some("Red Blocks"), Show::Center, Some("Waffle's Revenge"), Some(0)),
        ("win98-truecolor-display-desk-redblocks-tile-after-activedesktop-off", Era::Win98, Some("Red Blocks"), Show::Tile, None, None),
        ("win2000-truecolor-display-desk-pattern-boxes", Era::Win2000, None, Show::Center, Some("Boxes"), None),
        ("win2000-truecolor-display-desk-pattern-scottie", Era::Win2000, None, Show::Center, Some("Scottie"), None),
        ("win2000-truecolor-display-desk-pattern-scottie-selected", Era::Win2000, None, Show::Center, Some("Scottie"), Some(0)),
        ("win2000-truecolor-display-desk-pattern-tulip", Era::Win2000, None, Show::Center, Some("Tulip"), None),
        ("win2000-truecolor-display-desk-prairiewind-tile", Era::Win2000, Some("Prairie Wind"), Show::Tile, None, None),
        ("win2000-truecolor-display-desk-zapotec-center", Era::Win2000, Some("Zapotec"), Show::Center, None, None),
        ("win2000-truecolor-display-desk-zapotec-center-selected", Era::Win2000, Some("Zapotec"), Show::Center, None, Some(0)),
        ("win2000-truecolor-display-desk-zapotec-center-pattern-tulip", Era::Win2000, Some("Zapotec"), Show::Center, Some("Tulip"), None),
        ("win2000-truecolor-display-desk-zapotec-center-pattern-tulip-selected", Era::Win2000, Some("Zapotec"), Show::Center, Some("Tulip"), Some(0)),
        ("win2000-truecolor-display-desk-zapotec-tile", Era::Win2000, Some("Zapotec"), Show::Tile, None, None),
        ("win2000-truecolor-display-desk-zapotec-tile-selected", Era::Win2000, Some("Zapotec"), Show::Tile, None, Some(0)),
        ("win2000-truecolor-display-desk-zapotec-stretch-after-activedesktop-off", Era::Win2000, Some("Zapotec"), Show::Stretch, None, None),
    ];
    for &(file, era, wallpaper, show, pattern, selected) in shots {
        let (theme, fonts) = look(era, f95, f2k);
        let choice = Choice { wallpaper: wallpaper.map(str::to_string), show, pattern: pattern.map(str::to_string) };
        let background = display::background(theme, &choice, 800, 600).expect("a wallpaper or a pattern");
        let mut ours = background.clone();
        let background = std::sync::Arc::new(background);
        // The row of each of our icons on the Reference PC's desktop.
        let rows: &[i32] = match era {
            Era::Win95 => &[0, 2],
            Era::Win98 => &[0, 1, 2],
            Era::Win2000 => &[0, 1, 3],
        };
        let mut regions = vec![region((100, 0), Rect::new(100, 0, 700, 571))];
        for (i, &row) in rows.iter().enumerate() {
            let mut d = Desktop::new(era, Rect::new(0, (row - i as i32) * 75, 800, 572));
            d.set_background(Some(background.clone()));
            // 98's desktop kept the keyboard after Display Properties closed: My Computer's focus
            // frame shows with nothing selected; 95 and 2000 showed none.
            let focus = if era == Era::Win98 { selected.or(Some(0)) } else { selected };
            d.active = focus.is_some();
            d.select_only(selected);
            d.focus = focus;
            let cell = d.cell(i);
            d.paint(&mut ours, theme, fonts, cell);
            regions.push(region((cell.x, cell.y), cell));
        }
        let name = file.replace("-truecolor-display-desk-", "-wallpaper-");
        // The mouse pointer, left in the screen's top right corner.
        let masks = vec![Rect::new(799, 0, 1, 17)];
        out.push(Case { name: name.leak(), file: format!("vm/{file}.png").leak(), regions, ours, masks, vga: false });
    }
}

/// Display Properties' Background page and the Pattern window against the Reference PCs
/// (`vm/<pc>-truecolor-display-background`, `-bg-*`, `-pattern-window*`; survey.md, sections 1 to
/// 3): the whole window where the PC shows it. Masked, each for its reason: the other tabs
/// (Bevel95 shows only Background for now); in 98
/// the list's scroll bar (the PC's list has the HTML page as a 22nd entry, so its thumb differs);
/// in 2000 the list's scroll bar (the PC's list has Sample, the PC user's own picture, as a 25th
/// entry, so its thumb differs; every row shown is the same); in 2000's JPEG cases the monitor's
/// screen (Windows 2000 drew a JPEG there through its web view, not by the BMPs' rule; the core
/// cannot decode a JPEG either).
fn display_window_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::display::form::{DisplayProperties, Part, Which};
    use bevel_core::display::pattern::PatternWindow;
    use bevel_core::display::{Choice, Show};
    // (case, capture, look, window's place, wallpaper, how, pattern, focus)
    type Shot = (&'static str, &'static str, Era, (i32, i32), Option<&'static str>, Show, Option<&'static str>, Option<Part>);
    let list = Some(Part::List(Which::Wallpaper));
    let shots: &[Shot] = &[
        ("display-win95-background", "win95-truecolor-display-background", Era::Win95, (44, 44), None, Show::Center, None, None),
        ("display-win95-bubbles-center", "win95-truecolor-display-bg-bubbles-center", Era::Win95, (44, 44), Some("Bubbles"), Show::Center, None, list),
        ("display-win95-pattern-bricks", "win95-truecolor-display-bg-none-pattern-bricks", Era::Win95, (66, 66), None, Show::Center, Some("Bricks"), Some(Part::List(Which::Pattern))),
        ("display-win98-background", "win98-truecolor-display-background", Era::Win98, (0, 0), None, Show::Center, None, None),
        ("display-win98-forest-center", "win98-truecolor-display-bg-forest-center", Era::Win98, (0, 0), Some("Forest"), Show::Center, None, list),
        ("display-win98-pattern-bricks", "win98-truecolor-display-bg-none-pattern-bricks-2", Era::Win98, (110, 110), None, Show::Center, Some("Bricks"), list),
        ("display-win2000-background", "win2000-truecolor-display-background", Era::Win2000, (110, 110), None, Show::Center, None, None),
        ("display-win2000-bluelace16-center", "win2000-truecolor-display-bg-bluelace16-center", Era::Win2000, (110, 110), Some("Blue Lace 16"), Show::Center, None, list),
        ("display-win2000-pattern-scottie", "win2000-truecolor-display-bg-none-pattern-scottie", Era::Win2000, (44, 44), None, Show::Center, Some("Scottie"), list),
        // Tiled and stretched: the monitor tiles the scaled picture or fills its screen.
        ("display-win95-bubbles-tile", "win95-truecolor-display-bg-bubbles-tile", Era::Win95, (132, 120), Some("Bubbles"), Show::Tile, None, Some(Part::Tile)),
        ("display-win98-forest-tile", "win98-truecolor-display-bg-forest-tile", Era::Win98, (0, 0), Some("Forest"), Show::Tile, None, Some(Part::ShowList)),
        ("display-win98-clouds-stretch", "win98-truecolor-display-bg-clouds-stretch", Era::Win98, (66, 66), Some("Clouds"), Show::Stretch, None, Some(Part::ShowList)),
        ("display-win2000-zapotec-tile", "win2000-truecolor-display-bg-zapotec-tile", Era::Win2000, (110, 110), Some("Zapotec"), Show::Tile, None, Some(Part::ShowList)),
        ("display-win2000-zapotec-stretch", "win2000-truecolor-display-bg-zapotec-stretch", Era::Win2000, (110, 110), Some("Zapotec"), Show::Stretch, None, Some(Part::ShowList)),
        // 2000's JPEG entries in its list, one chosen.
        ("display-win2000-windows2000-center", "win2000-truecolor-display-bg-windows2000-center", Era::Win2000, (110, 110), Some("Windows 2000"), Show::Center, None, list),
    ];
    for &(name, file, era, at, wallpaper, show, pattern, focus) in shots {
        let (theme, fonts) = look(era, f95, f2k);
        let mut p = DisplayProperties::new(era, Choice::plain());
        p.set_shown(Choice { wallpaper: wallpaper.map(str::to_string), show, pattern: pattern.map(str::to_string) });
        if let Some(f) = focus {
            p.focus = f;
        }
        p.show_keys = true;
        // 95's capture shows its Pattern list scrolled to Bricks, the chosen one, at the top.
        if name == "display-win95-pattern-bricks" {
            p.scroll(Which::Pattern, 1);
        }
        let mut ours = Framebuffer::new(404, 448, 0);
        p.paint(&mut ours, theme, fonts);
        // Browse... is usable as on the PCs (brief 73): no mask.
        let mut masks = vec![Rect::new(83, 29, 312, 20)];
        // With a pattern chosen 95's Edit Pattern... is usable on the PC, grayed in Bevel95 for now.
        if era == Era::Win95 && pattern.is_some() {
            masks.push(Rect::new(96, 335, 92, 23));
        }
        // 95's tiled capture still had a pattern of an earlier step chosen, out of sight in its list
        // and hidden under the wallpaper: its Pattern list and Edit Pattern... are left out.
        if name == "display-win95-bubbles-tile" {
            masks.extend([Rect::new(33, 262, 155, 68), Rect::new(96, 335, 92, 23)]);
        }
        // A JPEG in the monitor: Windows 2000's web view drew it (see above).
        if name == "display-win2000-windows2000-center" {
            masks.push(Rect::new(126, 81, 152, 112));
        }
        match era {
            Era::Win98 => masks.push(Rect::new(265, 287, 16, 92)),
            Era::Win2000 => masks.push(Rect::new(278, 282, 16, 110)),
            Era::Win95 => {}
        }
        let masks = masks.into_iter().map(|m| Rect::new(m.x + at.0, m.y + at.1, m.w, m.h)).collect();
        out.push(Case { name, file: format!("vm/{file}.png").leak(), regions: vec![whole(&ours, at)], ours, masks, vga: false });
    }
    // The Pattern window as it opens on "(None)", and with a pattern chosen.
    type Window = (&'static str, &'static str, Era, (i32, i32), Option<&'static str>);
    let windows: &[Window] = &[
        ("display-win98-pattern-window", "win98-truecolor-display-pattern-window", Era::Win98, (113, 132), None),
        ("display-win98-pattern-window-bricks", "win98-truecolor-display-pattern-window-bricks", Era::Win98, (113, 132), Some("Bricks")),
        ("display-win2000-pattern-window", "win2000-truecolor-display-pattern-window", Era::Win2000, (47, 66), None),
        ("display-win2000-pattern-window-weave", "win2000-truecolor-display-pattern-window-weave", Era::Win2000, (47, 66), Some("Weave")),
    ];
    for &(name, file, era, at, chosen) in windows {
        let (theme, fonts) = look(era, f95, f2k);
        let mut w = PatternWindow::new(era, chosen);
        w.show_keys = true;
        let mut ours = Framebuffer::new(318, 205, 0);
        w.paint(&mut ours, theme, fonts);
        // With a pattern chosen the PCs' Edit Pattern... is usable; Bevel95's is grayed for now.
        let masks = if chosen.is_some() { vec![Rect::new(at.0 + 230, at.1 + 92, 75, 23)] } else { vec![] };
        out.push(Case { name, file: format!("vm/{file}.png").leak(), regions: vec![whole(&ours, at)], ours, masks, vga: false });
    }
}

/// The desktop's own icons with their labels, plain and selected, against the Reference PCs'
/// desktops (`vm/<era>-truecolor-desk-*`): each icon's 75Ã—75 cell where the PC shows it (the PCs
/// have icons Bevel95 leaves out, such as Inbox and My Network Places, so a cell's row there can
/// differ from ours). The state of each capture: which icon is selected, which has the focus,
/// whether the desktop had the keyboard (`*-sel-inactive`: a click on the taskbar took it).
fn desktop_icon_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::desktop_icons::Desktop;
    struct Shot {
        name: &'static str,
        file: &'static str,
        era: Era,
        active: bool,
        selected: Option<usize>,
        focus: Option<usize>,
        full: bool,
    }
    let shot = |name, file, era, active, selected, focus, full| Shot { name, file, era, active, selected, focus, full };
    let shots = [
        shot("vm-win95-desk-fresh", "vm/win95-truecolor-desk-fresh.png", Era::Win95, true, None, Some(0), false),
        shot("vm-win95-desk-sel-mycomputer", "vm/win95-truecolor-desk-sel-mycomputer.png", Era::Win95, true, Some(0), Some(0), false),
        shot("vm-win95-desk-sel-recycle", "vm/win95-truecolor-desk-sel-recycle.png", Era::Win95, true, Some(1), Some(1), false),
        shot("vm-win95-desk-sel-inactive", "vm/win95-truecolor-desk-sel-inactive.png", Era::Win95, false, Some(0), Some(0), false),
        shot("vm-win98-desk-fresh", "vm/win98-truecolor-desk-fresh.png", Era::Win98, false, None, None, false),
        shot("vm-win98-desk-sel-mycomputer", "vm/win98-truecolor-desk-sel-mycomputer.png", Era::Win98, true, Some(0), Some(0), false),
        shot("vm-win98-desk-sel-mydocs", "vm/win98-truecolor-desk-sel-mydocs.png", Era::Win98, true, Some(1), Some(1), false),
        shot("vm-win98-desk-sel-recycle", "vm/win98-truecolor-desk-sel-recycle.png", Era::Win98, true, Some(2), Some(2), false),
        shot("vm-win98-desk-none", "vm/win98-truecolor-desk-none.png", Era::Win98, true, None, Some(1), false),
        shot("vm-win98-desk-sel-inactive", "vm/win98-truecolor-desk-sel-inactive.png", Era::Win98, false, Some(0), Some(0), false),
        shot("vm-win98-desk-full", "vm/win98-truecolor-desk-full-plain.png", Era::Win98, true, None, Some(1), true),
        shot("vm-win98-desk-full-sel-2", "vm/win98-truecolor-desk-full2.png", Era::Win98, true, Some(2), Some(2), true),
        shot("vm-win98-desk-full-sel", "vm/win98-truecolor-desk-full-sel.png", Era::Win98, true, Some(2), Some(2), true),
        shot("vm-win2000-desk-fresh", "vm/win2000-truecolor-desk-fresh.png", Era::Win2000, true, None, Some(0), false),
        shot("vm-win2000-desk-sel-mydocs", "vm/win2000-truecolor-desk-sel-mydocs.png", Era::Win2000, true, Some(0), Some(0), false),
        shot("vm-win2000-desk-sel-mycomputer", "vm/win2000-truecolor-desk-sel-mycomputer.png", Era::Win2000, true, Some(1), Some(1), false),
        shot("vm-win2000-desk-sel-recycle", "vm/win2000-truecolor-desk-sel-recycle.png", Era::Win2000, true, Some(2), Some(2), false),
        shot("vm-win2000-desk-none", "vm/win2000-truecolor-desk-none.png", Era::Win2000, true, None, Some(0), false),
        shot("vm-win2000-desk-sel-inactive", "vm/win2000-truecolor-desk-sel-inactive.png", Era::Win2000, false, Some(0), Some(0), false),
        // No focus frame: only the mouse had been used (Windows 2000 hides them until a key).
        shot("vm-win2000-desk-full-sel", "vm/win2000-truecolor-desk-full-sel.png", Era::Win2000, true, Some(2), None, true),
    ];
    for s in shots {
        let (theme, fonts) = match s.era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        let mut ours = Framebuffer::new(160, 600, theme.colors.desktop);
        // The row of each of our icons on the Reference PC's desktop.
        let rows: &[i32] = match s.era {
            Era::Win95 => &[0, 2],
            Era::Win98 => &[0, 1, 2],
            Era::Win2000 => &[0, 1, 3],
        };
        let mut regions = Vec::new();
        for (i, &row) in rows.iter().enumerate() {
            // Each icon painted in the PC's place for it (the focus frame's dots follow the
            // screen's pixels, so the place matters).
            let area = Rect::new(0, (row - i as i32) * 75, 160, 572);
            let mut d = Desktop::new(s.era, area);
            d.active = s.active;
            d.select_only(s.selected);
            d.focus = s.focus;
            d.recycle_full = s.full;
            // Windows 2000 had seen keys (the logon) everywhere but in -full-sel.
            d.keys_used = s.focus.is_some();
            let cell = d.cell(i);
            d.paint(&mut ours, theme, fonts, cell);
            regions.push(region((cell.x, cell.y), cell));
        }
        out.push(Case { name: s.name, file: s.file, regions, ours, masks: vec![], vga: false });
    }
}

/// Moved icons against the Reference PCs (`vm/<era>-truecolor-move-*`): My Computer dropped at a
/// crooked place (Windows 98, selected, the pointer still on its picture, so only the picture's
/// left part and the label are compared), and after Line up Icons on the grid cell nearest to
/// where it lay (the focus frame on, nothing selected after the right-click on the desktop).
/// Painted by `Desktop` from a places file, so the place itself is checked too.
fn desktop_move_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::desktop_icons::Desktop;
    type Shot = (&'static str, &'static str, Era, (i32, i32), bool, Vec<Rect>);
    let shots: [Shot; 4] = [
        ("vm-win98-move-crooked", "vm/win98-truecolor-move-crooked.png", Era::Win98, (153, 97), true,
            vec![Rect::new(174, 99, 9, 32), Rect::new(153, 135, 75, 20)]),
        ("vm-win98-move-lineup", "vm/win98-truecolor-move-lineup.png", Era::Win98, (150, 75), false, vec![Rect::new(150, 75, 75, 60)]),
        ("vm-win95-move-lineup", "vm/win95-truecolor-move-lineup.png", Era::Win95, (225, 150), false, vec![Rect::new(225, 150, 75, 60)]),
        ("vm-win2000-move-lineup", "vm/win2000-truecolor-move-lineup.png", Era::Win2000, (225, 150), false, vec![Rect::new(225, 150, 75, 60)]),
    ];
    for (name, file, era, (x, y), selected, rects) in shots {
        let (theme, fonts) = match era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        let area = Rect::new(0, 0, 800, 572);
        let mut d = Desktop::new(era, area);
        d.load_places(&format!("{x} {y} ::my-computer\n"));
        let i = d.index_of("::my-computer").unwrap();
        d.active = true;
        d.select_only(Some(i));
        if !selected {
            d.select_only(None);
        }
        d.keys_used = true;
        let mut ours = Framebuffer::new(800, 600, theme.colors.desktop);
        d.paint(&mut ours, theme, fonts, Rect::new(x - 10, y - 10, 95, 95));
        let regions = rects.iter().map(|&r| region((r.x, r.y), r)).collect();
        out.push(Case { name, file, regions, ours, masks: vec![], vga: false });
    }
}

/// The desktop's right-click menu and its Arrange Icons submenu against the Reference PCs'
/// (`vm/<era>-truecolor-desk-menu*`). Bevel95's menus leave out what does not work yet, so each
/// of our entries is compared with the same entry's row of Windows' menu (laid out from Windows'
/// full list of entries), from the menu's left edge to 20 pixels short of the narrower menu's
/// right edge (the submenu arrows sit at the right edge, which depends on the widest label); the
/// menus' top-left corners too.
fn desktop_menu_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::desktop;
    type Shot = (&'static str, &'static str, Era, (i32, i32), &'static [&'static str], Option<usize>);
    let shots: [Shot; 9] = [
        ("vm-win95-desk-menu", "vm/win95-truecolor-desk-menu-left.png", Era::Win95, (672, 164),
            &["Arrange &Icons>", "Lin&e up Icons", "-", "&Paste", "Paste &Shortcut", "-", "Ne&w>", "-", "P&roperties"], None),
        ("vm-win98-desk-menu", "vm/win98-truecolor-desk-menu.png", Era::Win98, (300, 200),
            &["&Active Desktop>", "-", "Arrange &Icons>", "Line &Up Icons", "-", "Re&fresh", "-", "&Paste", "Paste &Shortcut", "-", "&New>", "-", "P&roperties"], None),
        ("vm-win2000-desk-menu", "vm/win2000-truecolor-desk-menu-left.png", Era::Win2000, (674, 150),
            &["Active Desktop>", "-", "Arrange Icons>", "Line Up Icons", "Refresh", "-", "Paste", "Paste Shortcut", "-", "New>", "-", "Properties"], None),
        ("vm-win95-desk-arrange", "vm/win95-truecolor-desk-menu-arrange-left.png", Era::Win95, (567, 178),
            &["by &Name", "by &Type", "by Si&ze", "by &Date", "-", "&Auto Arrange"], Some(0)),
        ("vm-win98-desk-arrange", "vm/win98-truecolor-desk-menu-arrange-left.png", Era::Win98, (562, 176),
            &["by &Name", "by &Type", "by Si&ze", "by &Date", "-", "&Auto Arrange"], Some(0)),
        ("vm-win2000-desk-arrange", "vm/win2000-truecolor-desk-menu-arrange-left.png", Era::Win2000, (568, 176),
            &["by &Name", "by &Type", "by Si&ze", "by &Date", "-", "&Auto Arrange"], Some(0)),
        // Auto Arrange on: its tick (the Reference PCs, after Arrange Icons > Auto Arrange).
        ("vm-win95-desk-arrange-auto", "vm/win95-truecolor-move-menu-auto-on.png", Era::Win95, (493, 450),
            &["by &Name", "by &Type", "by Si&ze", "by &Date", "-", "&Auto Arrange"], Some(0)),
        ("vm-win98-desk-arrange-auto", "vm/win98-truecolor-move-menu-auto-on.png", Era::Win98, (615, 326),
            &["by &Name", "by &Type", "by Si&ze", "by &Date", "-", "&Auto Arrange"], Some(0)),
        ("vm-win2000-desk-arrange-auto", "vm/win2000-truecolor-move-menu-auto-on.png", Era::Win2000, (493, 299),
            &["by &Name", "by &Type", "by Si&ze", "by &Date", "-", "&Auto Arrange"], Some(0)),
    ];
    for (name, file, era, at, windows_labels, hover) in shots {
        let auto = name.ends_with("-auto");
        let (theme, fonts) = match era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        let arrange = hover.is_some();
        // Windows 2000 underlined no access keys in these captures (opened by the mouse, the
        // submenus by their access key).
        let keys = era.always_shows_keys();
        let ours_menu = if arrange { desktop::arrange_menu(auto) } else { desktop::menu(era, false) };
        let ours_menu = Menu { hover, show_keys: keys, ..ours_menu };
        let windows_menu = menu(windows_labels, hover, keys);
        let mut s = Screen::new(800, 600, theme.colors.desktop);
        let (ours_rect, ours_layout) = s.menu(theme, fonts, &ours_menu, MenuStyle::Popup, |_, _| at);
        let win_layout = paint::layout_menu(theme, fonts, &windows_menu, MenuStyle::Popup, 600);
        let width = ours_layout.width.min(win_layout.width) - 20;
        let plain = |l: &str| l.replace('&', "").trim_end_matches('>').to_string();
        let mut regions = vec![region((at.0, at.1), Rect::new(at.0, at.1, width, 3))];
        for (i, e) in ours_menu.entries.iter().enumerate() {
            if e.kind == bevel_core::paint::EntryKind::Separator {
                continue;
            }
            let Some(j) = windows_labels.iter().position(|w| plain(w) == plain(&e.label)) else {
                continue;
            };
            let (o, w) = (entry(ours_rect, &ours_layout, i), win_layout.rects[j]);
            regions.push(region((o.x - 3, o.y), Rect::new(at.0 + w.x - 3, at.1 + w.y, width, w.h)));
        }
        out.push(Case { name, file, regions, ours: s.fb, masks: vec![], vga: false });
    }
}

/// The reference folder (as the test itself finds it), for the few cases built from another
/// capture's pixels.
fn refs_dir() -> PathBuf {
    std::env::var_os("BEVEL95_REFS").map(PathBuf::from).unwrap_or_else(|| private_dir().join("reference").join("refs"))
}

/// The folder of material that is not part of the public project (the reference captures and
/// the Reference PCs are in its `reference` folder): `BEVEL95_PRIVATE`, else the checkout's
/// untracked `private` folder.
fn private_dir() -> PathBuf {
    std::env::var_os("BEVEL95_PRIVATE").map(PathBuf::from).unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../private"))
}

/// The theme and fonts of a look.
fn look<'a>(era: Era, f95: &'a Fonts<'a>, f2k: &'a Fonts<'a>) -> (&'static Theme, &'a Fonts<'a>) {
    match era {
        Era::Win95 => (&WIN95, f95),
        Era::Win98 => (&WIN98, f95),
        Era::Win2000 => (&WIN2000, f2k),
    }
}

/// The menu on an icon, the confirmation windows, the rename box and the cut picture against the
/// Reference PCs (`vm/<era>-truecolor-item-*`, taken with `private/probes/desktop-menu/k.ps1`).
/// Menus: each of our rows with the same entry's row of Windows' fuller menu (as
/// `desktop_menu_cases`), opened by Shift+F10, so Windows 2000 underlined the access keys.
/// Windows: the whole window, where Windows put it. Rename: the edit box over a label (Windows 95
/// and 98 between two blinks of the caret, 2000 with it). Cut: the plain picture from another
/// capture of the same PC, faded by `cut_picture`, with its label.
fn desktop_item_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::delete_confirm::{ConfirmWindow, Question};
    use bevel_core::desktop::{self, ItemKind};
    use bevel_core::desktop_icons::{self, Desktop, Item, Picture, System};
    use bevel_core::kit::Form;
    const FILE: &[&str] =
        &["&Open", "&Print", "Open &With...", "-", "Se&nd To>", "-", "Cu&t", "&Copy", "-", "Create &Shortcut", "&Delete", "Rena&me", "-", "P&roperties"];
    const FILE98: &[&str] = &[
        "&Open", "&Print", "Open &with...", "&Quick View", "-", "Send &To>", "-", "Cu&t", "&Copy", "-", "Create &Shortcut", "&Delete",
        "Rena&me", "-", "P&roperties",
    ];
    const FILE2K: &[&str] =
        &["&Open", "&Print", "Open Wit&h...", "-", "Se&nd To>", "-", "Cu&t", "&Copy", "-", "Create &Shortcut", "&Delete", "Rena&me", "-", "P&roperties"];
    const FOLDER95: &[&str] =
        &["&Open", "&Explore", "&Find...", "-", "Se&nd To>", "-", "Cu&t", "&Copy", "-", "Create &Shortcut", "&Delete", "Rena&me", "-", "P&roperties"];
    const FOLDER98: &[&str] =
        &["&Open", "&Explore", "&Find...", "-", "Send &To>", "-", "Cu&t", "&Copy", "-", "Create &Shortcut", "&Delete", "Rena&me", "-", "P&roperties"];
    const FOLDER2K: &[&str] = &[
        "&Open", "E&xplore", "Se&arch...", "-", "S&haring...", "-", "Se&nd To>", "-", "Cu&t", "&Copy", "-", "Create &Shortcut", "&Delete",
        "Rena&me", "-", "P&roperties",
    ];
    const MYCOMPUTER: &[&str] = &["&Open", "&Explore", "&Find...", "-", "Create &Shortcut", "Rena&me", "-", "P&roperties"];
    const MYCOMPUTER2K: &[&str] = &[
        "&Open", "E&xplore", "Se&arch...", "Mana&ge", "-", "Map &Network Drive...", "Di&sconnect Network Drive...", "-", "Create &Shortcut",
        "Rena&me", "-", "P&roperties",
    ];
    const MYDOCS98: &[&str] = &["&Open", "&Explore", "-", "Remo&ve from Desktop", "-", "Create &Shortcut", "Rena&me", "-", "P&roperties"];
    const MYDOCS2K: &[&str] = &["&Open", "E&xplore", "Se&arch...", "-", "Create &Shortcut", "&Delete", "Rena&me", "-", "P&roperties"];
    const BIN: &[&str] = &["&Open", "&Explore", "Empty Recycle &Bin", "-", "Create &Shortcut", "-", "P&roperties"];
    // Full, with a file cut onto the clipboard before it was deleted: the Recycle Bin took a Paste.
    const BIN_PASTE: &[&str] = &["&Open", "&Explore", "Empty Recycle &Bin", "-", "&Paste", "-", "Create &Shortcut", "-", "P&roperties"];
    const BIN_PASTE2K: &[&str] = &["&Open", "E&xplore", "Empty Recycle &Bin", "-", "&Paste", "-", "Create &Shortcut", "-", "P&roperties"];
    const BIN2K: &[&str] = &["&Open", "E&xplore", "Empty Recycle &Bin", "-", "Create &Shortcut", "-", "P&roperties"];
    // The Recycle Bin and a folder selected together.
    const SEVERAL: &[&str] = &["&Open", "&Explore", "&Find...", "-", "&Paste", "-", "Create &Shortcut", "-", "P&roperties"];
    const SEVERAL2K: &[&str] = &["&Open", "E&xplore", "Empty Recycle &Bin", "-", "&Paste", "-", "Create &Shortcut", "-", "P&roperties"];
    type Shot = (&'static str, &'static str, Era, (i32, i32), &'static [&'static str], &'static [ItemKind], bool);
    let menus: [Shot; 22] = [
        ("vm-win95-item-menu-mycomputer", "vm/win95-truecolor-item-menu-mycomputer.png", Era::Win95, (37, 18), MYCOMPUTER, &[ItemKind::System(System::MyComputer)], false),
        ("vm-win95-item-menu-recycle-empty", "vm/win95-truecolor-item-menu-recycle-empty.png", Era::Win95, (37, 168), BIN, &[ItemKind::System(System::RecycleBin)], false),
        ("vm-win95-item-menu-recycle-full", "vm/win95-truecolor-item-menu-recycle-full.png", Era::Win95, (37, 168), BIN_PASTE, &[ItemKind::System(System::RecycleBin)], true),
        ("vm-win95-item-menu-folder", "vm/win95-truecolor-item-menu-folder.png", Era::Win95, (37, 388), FOLDER95, &[ItemKind::Folder], false),
        ("vm-win95-item-menu-text", "vm/win95-truecolor-item-menu-text.png", Era::Win95, (37, 388), FILE, &[ItemKind::File], false),
        ("vm-win95-item-menu-shortcut", "vm/win95-truecolor-item-menu-shortcut.png", Era::Win95, (112, 168), FILE, &[ItemKind::File], false),
        ("vm-win95-item-menu-several", "vm/win95-truecolor-item-menu-several.png", Era::Win95, (37, 393), SEVERAL, &[ItemKind::System(System::RecycleBin), ItemKind::Folder], true),
        ("vm-win98-item-menu-mydocs", "vm/win98-truecolor-item-menu-mydocs.png", Era::Win98, (37, 93), MYDOCS98, &[ItemKind::System(System::MyDocuments)], false),
        ("vm-win98-item-menu-recycle-empty", "vm/win98-truecolor-item-menu-recycle-empty.png", Era::Win98, (37, 168), BIN, &[ItemKind::System(System::RecycleBin)], false),
        ("vm-win98-item-menu-recycle-full", "vm/win98-truecolor-item-menu-recycle-full.png", Era::Win98, (37, 168), BIN_PASTE, &[ItemKind::System(System::RecycleBin)], true),
        ("vm-win98-item-menu-folder", "vm/win98-truecolor-item-menu-folder.png", Era::Win98, (112, 18), FOLDER98, &[ItemKind::Folder], false),
        ("vm-win98-item-menu-text", "vm/win98-truecolor-item-menu-text.png", Era::Win98, (112, 93), FILE98, &[ItemKind::File], false),
        ("vm-win98-item-menu-shortcut", "vm/win98-truecolor-item-menu-shortcut.png", Era::Win98, (112, 168), FILE98, &[ItemKind::File], false),
        ("vm-win98-item-menu-several", "vm/win98-truecolor-item-menu-several.png", Era::Win98, (112, 18), SEVERAL, &[ItemKind::System(System::RecycleBin), ItemKind::Folder], true),
        ("vm-win2000-item-menu-mydocs", "vm/win2000-truecolor-item-menu-mydocs.png", Era::Win2000, (37, 18), MYDOCS2K, &[ItemKind::System(System::MyDocuments)], false),
        ("vm-win2000-item-menu-mycomputer", "vm/win2000-truecolor-item-menu-mycomputer.png", Era::Win2000, (37, 93), MYCOMPUTER2K, &[ItemKind::System(System::MyComputer)], false),
        ("vm-win2000-item-menu-recycle-empty", "vm/win2000-truecolor-item-menu-recycle-empty.png", Era::Win2000, (37, 243), BIN2K, &[ItemKind::System(System::RecycleBin)], false),
        ("vm-win2000-item-menu-recycle-full", "vm/win2000-truecolor-item-menu-recycle-full.png", Era::Win2000, (37, 243), BIN_PASTE2K, &[ItemKind::System(System::RecycleBin)], true),
        ("vm-win2000-item-menu-folder", "vm/win2000-truecolor-item-menu-folder.png", Era::Win2000, (37, 155), FOLDER2K, &[ItemKind::Folder], false),
        ("vm-win2000-item-menu-text", "vm/win2000-truecolor-item-menu-text.png", Era::Win2000, (112, 18), FILE2K, &[ItemKind::File], false),
        ("vm-win2000-item-menu-shortcut", "vm/win2000-truecolor-item-menu-shortcut.png", Era::Win2000, (112, 93), FILE2K, &[ItemKind::File], false),
        ("vm-win2000-item-menu-several", "vm/win2000-truecolor-item-menu-several.png", Era::Win2000, (37, 393), SEVERAL2K, &[ItemKind::System(System::RecycleBin), ItemKind::Folder], true),
    ];
    for (name, file, era, at, windows_labels, kinds, full) in menus {
        let (theme, fonts) = look(era, f95, f2k);
        // The several menu's mix is Windows' own (see desktop::item_entries); the rows we share
        // with it are compared all the same.
        let full = full || name.ends_with("-several");
        let ours_menu = Menu { show_keys: true, ..desktop::item_menu(era, kinds, full) };
        let windows_menu = menu(windows_labels, None, true);
        let mut s = Screen::new(800, 600, theme.colors.desktop);
        let (ours_rect, ours_layout) = s.menu(theme, fonts, &ours_menu, MenuStyle::Popup, |_, _| at);
        let win_layout = paint::layout_menu(theme, fonts, &windows_menu, MenuStyle::Popup, 600);
        let width = ours_layout.width.min(win_layout.width) - 20;
        let plain = |l: &str| l.replace('&', "").trim_end_matches('>').to_string();
        let mut regions = vec![region((at.0, at.1), Rect::new(at.0, at.1, width, 3))];
        for (i, e) in ours_menu.entries.iter().enumerate() {
            if e.kind == bevel_core::paint::EntryKind::Separator {
                continue;
            }
            let Some(j) = windows_labels.iter().position(|w| plain(w) == plain(&e.label)) else { continue };
            let (o, w) = (entry(ours_rect, &ours_layout, i), win_layout.rects[j]);
            regions.push(region((o.x - 3, o.y), Rect::new(at.0 + w.x - 3, at.1 + w.y, width, w.h)));
        }
        out.push(Case { name, file, regions, ours: s.fb, masks: vec![], vga: false });
    }

    // The confirmation windows: (question, the capture's name, the window's top).
    let windows: [(Question, &str, i32); 6] = [
        (Question::File("Test text".into()), "file", 204),
        (Question::File("Shortcut to Test text".into()), "shortcut", 204),
        (Question::Folder("Test folder with a long name".into()), "folder", 204),
        (Question::Several(2), "multiple", 220),
        (Question::EmptyOne("Test text".into()), "empty-one", 204),
        (Question::EmptySeveral(2), "empty-multiple", 220),
    ];
    for era in Era::ALL {
        let (theme, fonts) = look(era, f95, f2k);
        for (question, shot, top) in windows.iter().cloned() {
            if era == Era::Win2000 && shot == "empty-one" {
                continue; // not captured
            }
            let mut w = ConfirmWindow::new(era, question, true);
            w.layout(fonts);
            let (ww, wh) = w.size();
            let mut fb = Framebuffer::new(ww, wh, 0);
            w.paint(&mut fb, theme, fonts);
            let name: &'static str = Box::leak(format!("vm-{}-item-confirm-{shot}", era_name(era)).into_boxed_str());
            let file: &'static str = Box::leak(format!("vm/{}-truecolor-item-confirm-{shot}.png", era_name(era)).into_boxed_str());
            out.push(Case { name, file, regions: vec![region((0, 0), Rect::new(190, top, ww, wh))], ours: fb, masks: vec![], vga: false });
        }
    }

    // The rename box: (capture, name typed, the icon's cell, caret shown).
    let item = |name: &str, folder: bool| Item {
        id: format!("C:\\Desktop\\{name}"),
        name: name.to_string(),
        picture: if folder { Picture::Folder } else { Picture::Own(None) },
        shortcut: false,
        folder,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    type Rename = (Era, &'static str, &'static str, (i32, i32), bool);
    let renames: [Rename; 6] = [
        (Era::Win95, "rename-newfolder", "New Folder", (229, 41), true),
        (Era::Win98, "rename-newfolder", "New Folder", (229, 41), false),
        (Era::Win2000, "rename-newfolder", "New Folder", (228, 42), false),
        (Era::Win95, "rename-text", "Test text", (0, 453), false),
        (Era::Win98, "rename-text", "Test text", (75, 78), false),
        (Era::Win2000, "rename-text", "Test text", (75, 3), true),
    ];
    for (era, shot, label, (x, y), caret) in renames {
        let (theme, fonts) = look(era, f95, f2k);
        let mut d = Desktop::new(era, Rect::new(0, 0, 800, 572));
        let it = item(label, label == "New Folder");
        d.load_places(&format!("{x} {y} {}\n", it.id));
        d.set_files(vec![it.clone()]);
        let i = d.index_of(&it.id).unwrap();
        d.active = true;
        d.select_only(Some(i));
        d.begin_rename(i);
        d.blink(caret);
        let (b, _) = d.rename_box(fonts.regular).unwrap();
        let mut ours = Framebuffer::new(800, 600, theme.colors.desktop);
        d.paint(&mut ours, theme, fonts, Rect::new(0, 0, 800, 572));
        let name: &'static str = Box::leak(format!("vm-{}-item-{shot}", era_name(era)).into_boxed_str());
        let file: &'static str = Box::leak(format!("vm/{}-truecolor-item-{shot}.png", era_name(era)).into_boxed_str());
        out.push(Case { name, file, regions: vec![region((b.x, b.y), b)], ours, masks: vec![], vga: false });
    }

    // The extension warning (`vm/<era>-truecolor-renext-*`, taken with
    // `private/probes/rename-ext/r.ps1`): the whole message box where Windows put it (Enter typed,
    // so 2000 underlines too); the rename box under it while it is open ("a.dat", no selection,
    // no caret) and after No (the name typed, all selected), on the text file made at 600,300:
    // 95 and 98 only. Windows 2000's box under the window touches it, and after No (the old
    // two-line name back) it stood 4 pixels lower than a freshly opened box, a move of its edit
    // control that Bevel95 does not copy (docs/decisions.md, "The extension warning").
    {
        use bevel_core::rename_warning::{self, RenameWarning};
        for era in Era::ALL {
            let (theme, fonts) = look(era, f95, f2k);
            let mut w = RenameWarning::new(era, true);
            w.layout(fonts);
            let (ww, wh) = w.size();
            let (x, y) = rename_warning::place((800, 600), (ww, wh));
            let mut fb = Framebuffer::new(ww, wh, 0);
            w.paint(&mut fb, theme, fonts);
            let name: &'static str = Box::leak(format!("vm-{}-renext-warning", era_name(era)).into_boxed_str());
            let file: &'static str = Box::leak(format!("vm/{}-truecolor-renext-changed.png", era_name(era)).into_boxed_str());
            out.push(Case { name, file, regions: vec![region((0, 0), Rect::new(x, y, ww, wh))], ours: fb, masks: vec![], vga: false });

            let it = item("New Text Document.txt", false);
            for state in ["changed", "after-no"] {
                if era == Era::Win2000 {
                    continue;
                }
                let mut d = Desktop::new(era, Rect::new(0, 0, 800, 572));
                d.load_places(&format!("579 298 {}\n", it.id));
                d.set_files(vec![it.clone()]);
                let i = d.index_of(&it.id).unwrap();
                d.active = true;
                d.select_only(Some(i));
                d.begin_rename(i);
                d.rename_paste("a.dat");
                d.rename_asking(true);
                if state == "after-no" {
                    d.rename_declined(rename_warning::keeps_typed_name(era));
                    d.blink(false);
                }
                let (b, _) = d.rename_box(fonts.regular).unwrap();
                let mut ours = Framebuffer::new(800, 600, theme.colors.desktop);
                d.paint(&mut ours, theme, fonts, Rect::new(0, 0, 800, 572));
                let name: &'static str = Box::leak(format!("vm-{}-renext-box-{state}", era_name(era)).into_boxed_str());
                let file: &'static str = Box::leak(format!("vm/{}-truecolor-renext-{state}.png", era_name(era)).into_boxed_str());
                out.push(Case { name, file, regions: vec![region((b.x, b.y), b)], ours, masks: vec![], vga: false });
            }
        }
    }

    // The cut picture: the text file's plain picture where the PC showed it after Create
    // Shortcut, faded; with its label as the PC showed it (not selected, the focus frame on).
    let dir = refs_dir();
    let cuts: [(Era, (i32, i32)); 3] = [(Era::Win95, (21, 455)), (Era::Win98, (96, 80)), (Era::Win2000, (96, 5))];
    for (era, (ix, iy)) in cuts {
        let (theme, _) = look(era, f95, f2k);
        let plain = dir.join(format!("vm/{}-truecolor-item-shortcut-made.png", era_name(era)));
        if !plain.is_file() {
            continue;
        }
        let shot = read_png(&plain);
        let mut icon = Icon { width: 32, height: 32, pixels: Vec::with_capacity(1024) };
        for y in 0..32 {
            for x in 0..32 {
                // Where the desktop shows through, the picture is see-through.
                let p = shot.get(ix + x, iy + y);
                icon.pixels.push(if p == theme.colors.desktop { 0 } else { p });
            }
        }
        let faded = desktop_icons::cut_picture(era, &icon, theme.colors.desktop);
        let mut ours = Framebuffer::new(800, 600, theme.colors.desktop);
        ours.draw_icon(&faded, ix, iy);
        let name: &'static str = Box::leak(format!("vm-{}-item-cut", era_name(era)).into_boxed_str());
        let file: &'static str = Box::leak(format!("vm/{}-truecolor-item-cut-unselected.png", era_name(era)).into_boxed_str());
        out.push(Case { name, file, regions: vec![region((ix, iy), Rect::new(ix, iy, 32, 32))], ours, masks: vec![], vga: false });
    }
}

fn era_name(era: Era) -> &'static str {
    match era {
        Era::Win95 => "win95",
        Era::Win98 => "win98",
        Era::Win2000 => "win2000",
    }
}

/// The Taskbar Properties window, read pixel for pixel from Windows 95 (with Internet
/// Explorer 4; 256 colors), Windows 98 (16 colors) and Windows 2000 in the v86 emulator:
/// `v86/` in the reference folder (see docs/theme-spec.md). Each state as captured: the focus
/// where the keyboard left it, Apply on once something changed. The window has Windows' two
/// tabs and nothing of ours (Bevel95's own settings have their own window, Bevel95 Settings),
/// so the row of tabs is compared in full. Masked: a grayed-out Apply's label in 95 and 98
/// (with so few colors Windows draws it flat gray; in full color it is etched, as GUIdebook's
/// Run dialogs show and as we draw it); in Windows 98 the pointer and the title bar (16 colors
/// show no gradient); in Windows 2000 what Bevel95 does not do and so grays out (Personalized
/// Menus, most Start Menu Settings).
fn properties_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::config::{StartOptions, TaskbarOptions};
    use bevel_core::properties::{self, Check, Part, Properties, Sheet, Values};
    let draw = |era: Era, page: usize, options: TaskbarOptions, focus: Part, changed: bool| {
        let (theme, fonts) = match era {
            Era::Win95 => (&WIN95, f95),
            Era::Win98 => (&WIN98, f95),
            Era::Win2000 => (&WIN2000, f2k),
        };
        // Windows 2000's emulator had Display Logoff off.
        let start = StartOptions { favorites: false, log_off: era != Era::Win2000, scroll_programs: false };
        let values = Values {
            options,
            start,
            era,
            colors: bevel_core::theme::Depth::TrueColor,
            scale: bevel_core::scale::Scale::whole(2),
            start_with_windows: false,
            clock_offset: 0,
            desktop: true,
            programs: bevel_core::programs::Shown::Everything,
            windows_key: bevel_core::config::WindowsKey::Find,
            quick_search_all: false,
            battery_percent: false,
            windows_display: false,
            monitor_lines: bevel_core::monitor_lines::Strength::Off,
            quick_launch: false,
            group: true,
        };
        let mut p = Properties::new(Sheet::Taskbar, era, values);
        p.page = page;
        p.focus = focus;
        p.changed = changed;
        // The window was opened from the keyboard, so even Windows 2000 shows the access keys.
        p.show_keys = true;
        let l = properties::layout(&p, fonts);
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        properties::paint(&mut fb, theme, fonts, &p, &l);
        fb
    };
    let o = TaskbarOptions::default();
    let states: [(&str, &str, usize, TaskbarOptions, Part, bool); 7] = [
        ("general", "general", 0, o, Part::Check(Check::AlwaysOnTop), false),
        ("small-icons", "small", 0, TaskbarOptions { small_icons: true, ..o }, Part::Check(Check::SmallIcons), true),
        ("no-clock", "noclock", 0, TaskbarOptions { show_clock: false, ..o }, Part::Check(Check::ShowClock), true),
        ("auto-hide", "autohide", 0, TaskbarOptions { auto_hide: true, ..o }, Part::Check(Check::AutoHide), true),
        ("not-on-top", "nottop", 0, TaskbarOptions { always_on_top: false, ..o }, Part::Check(Check::AlwaysOnTop), true),
        ("tab-focused", "shifttab", 0, o, Part::Tab(0), true),
        ("advanced", "advanced", 1, o, Part::Tab(1), true),
    ];
    for (state, cap, page, options, focus, changed) in states {
        let mut masks = Vec::new();
        if page == 0 {
            masks.push(Rect::new(33, 323, 140, 19));
        } else {
            masks.extend([Rect::new(26, 274, 277, 16), Rect::new(26, 322, 277, 21)]);
        }
        let ours = draw(Era::Win2000, page, options, focus, changed);
        out.push(Case {
            name: format!("win2000-properties-{state}").leak(),
            file: format!("v86/w2k-{cap}.png").leak(),
            regions: vec![whole(&ours, (0, 0))],
            ours,
            masks,
            vga: false,
        });
    }
    properties_cases_95_98(out, &draw);
}

fn properties_cases_95_98(
    out: &mut Vec<Case>,
    draw: &dyn Fn(Era, usize, bevel_core::config::TaskbarOptions, bevel_core::properties::Part, bool) -> Framebuffer,
) {
    use bevel_core::config::TaskbarOptions;
    use bevel_core::properties::{Check, Part};
    let o = TaskbarOptions::default();
    let states: [(&str, &str, usize, TaskbarOptions, Part, bool); 6] = [
        ("options", "opts", 0, o, Part::Check(Check::AlwaysOnTop), false),
        ("small-icons", "opts-small", 0, TaskbarOptions { small_icons: true, ..o }, Part::Check(Check::SmallIcons), true),
        ("no-clock", "opts-noclock", 0, TaskbarOptions { show_clock: false, ..o }, Part::Check(Check::ShowClock), true),
        ("auto-hide", "opts-autohide", 0, TaskbarOptions { auto_hide: true, ..o }, Part::Check(Check::AutoHide), true),
        (
            "not-on-top",
            "opts-nottop",
            0,
            TaskbarOptions { always_on_top: false, ..o },
            Part::Check(Check::AlwaysOnTop),
            true,
        ),
        ("start-menu-programs", "smp", 1, o, Part::Add, true),
    ];
    let name = |era: &str, state: &str| -> &'static str { format!("{era}-properties-{state}").leak() };
    let file = |era: &str, cap: &str| -> &'static str { format!("v86/{era}-{cap}.png").leak() };
    for (state, cap, page, options, focus, changed) in states {
        let mut masks = Vec::new();
        if !changed {
            masks.push(Rect::new(262, 369, 71, 19));
        }
        let ours = draw(Era::Win95, page, options, focus, changed);
        // The emulator's Windows 95 had Internet Explorer 4's shell: its preview is 98's. Since
        // 2026-11-18 the 95 look shows plain Windows 95's (its Reference PC's, checked whole by the
        // `own-win95-properties-*` cases): the preview is left out here.
        let mut masks95 = masks.clone();
        if page == 0 {
            masks95.push(Rect::new(34, 74, 276, 132));
        }
        out.push(Case {
            name: name("win95", state),
            file: file("w95", cap),
            regions: vec![whole(&ours, (0, 0))],
            ours,
            masks: masks95,
            vga: true,
        });
        masks.extend([Rect::new(186, 175, 11, 19), Rect::new(3, 3, 338, 18)]);
        let ours = draw(Era::Win98, page, options, focus, changed);
        out.push(Case {
            name: name("win98", state),
            file: file("w98", cap),
            regions: vec![whole(&ours, (0, 0))],
            ours,
            masks,
            vga: true,
        });
    }
}

/// Windows 98 (it uses 95's font). Its taskbar screenshot has Quick Launch (not in v1), which
/// squeezes the task buttons to 121 pixels, so our bar is drawn just wide enough to give the
/// same width, and each part is compared on its own.
fn win98_cases(out: &mut Vec<Case>, f98: &Fonts) {
    let tasks = || {
        vec![
            task("Scheduled Tasks", TaskState::Normal),
            // The Drive Converter's window has no icon in this screenshot.
            Task { icon: None, ..task("Drive Converter (FAT32)", TaskState::Normal) },
            task("Calculator", TaskState::Active),
        ]
    };
    let width = (300..900)
        .find(|&w| {
            let layout = bevel_core::layout::bar(&WIN98, f98, w, 3, 1, None, "3:15 PM", WIDEST, 0);
            layout.tasks.iter().all(|t| t.w == 121)
        })
        .expect("a bar width that gives 121-pixel buttons");
    let (ours, layout) = bar(&WIN98, f98, width, false, tasks(), 1, "3:15 PM");
    let full = |x: i32, w: i32| Rect::new(x, 0, w, 28);
    out.push(Case {
        name: "win98-taskbar",
        file: "win98-taskbar-quicklaunch-3tasks-1active-tray-640x28.png",
        // Quick Launch's handle starts at x = 58, right after the Start button and its gap. The
        // notification area holds Task Scheduler's icon (masked).
        regions: vec![
            region((0, 0), full(0, 58)),
            region((layout.tasks[0].x, 0), full(176, 121)),
            region((layout.tasks[1].x, 0), full(300, 121)),
            region((layout.tasks[2].x, 0), full(424, 121)),
            region((layout.well.x - 2, 0), full(555, 85)),
        ],
        ours,
        masks: vec![Rect::new(180, 7, 16, 16), Rect::new(428, 8, 16, 16), Rect::new(560, 7, 16, 16)],
        vga: false,
    });

    // The Start menu on a 640Ã—480 screen, Start button at (2, 456): Favorites > Channels open
    // (a tooltip covers part of the screen; it is masked), then the 16-color cut-outs: Programs >
    // Accessories > Communications, and the Start menu alone.
    let (button_fb, b) = pressed_start(&WIN98, f98);
    let button = Rect::new(b.x, 452 + b.y, b.w, b.h);
    let screen_rect = Rect::new(0, 0, 640, 480);
    let rule = WIN98.start_menu.submenu;
    let start98 = |user: &str, hover: Option<usize>| {
        let log_off = if user.is_empty() { "&Log Off...".to_string() } else { format!("&Log Off {user}...") };
        let labels = [
            "Windows Update",
            "-",
            "&Programs>",
            "F&avorites>",
            "&Documents>",
            "&Settings>",
            "&Find>",
            "&Help",
            "&Run...",
            "-",
            log_off.as_str(),
            "Sh&ut Down...",
        ];
        menu(&labels, hover, true)
    };
    {
        let mut s = Screen::new(640, 480, rgb(0xFFFFFF));
        s.fb.blit(&button_fb, button.x, button.y);
        let start = start98("Midnite", Some(3));
        let (start_rect, start_layout) =
            s.menu(&WIN98, f98, &start, MenuStyle::Start, |w, h| place_start_menu(button, w, h, screen_rect));
        let favorites = menu(&["Channels>", "Links>", "Software Updates>"], Some(0), true);
        let at = entry(start_rect, &start_layout, 3);
        let (f_rect, f_layout) = s.menu(&WIN98, f98, &favorites, MenuStyle::Cascade, |w, h| {
            place_submenu(start_rect, at, w, h, 3, rule, screen_rect)
        });
        let channels = menu(&["Microsoft Channel Guide"], Some(0), true);
        let at = entry(f_rect, &f_layout, 0);
        let (c_rect, _) = s.menu(&WIN98, f98, &channels, MenuStyle::Cascade, |w, h| {
            place_submenu(f_rect, at, w, h, 3, rule, screen_rect)
        });
        s.masks.push(Rect::new(343, 178, 297, 59)); // the Channel Guide's tooltip
        out.push(screen_case(
            "win98-start-menu",
            "win98-desktop-startmenu-favorites-channels-taskbar-640x480.png",
            s,
            (0, 0),
            &[start_rect, button, f_rect, c_rect],
        ));
    }
    {
        let mut s = Screen::new(640, 480, rgb(0xFFFFFF));
        s.fb.blit(&button_fb, button.x, button.y);
        let start = start98("", Some(2));
        let (start_rect, start_layout) =
            s.menu(&WIN98, f98, &start, MenuStyle::Start, |w, h| place_start_menu(button, w, h, screen_rect));
        let programs = menu(
            &[
                "Accessories>",
                "Internet Explorer>",
                "Online Services>",
                "StartUp>",
                "MS-DOS Prompt",
                "Windows Explorer",
            ],
            Some(0),
            true,
        );
        let at = entry(start_rect, &start_layout, 2);
        let (p_rect, p_layout) = s.menu(&WIN98, f98, &programs, MenuStyle::Cascade, |w, h| {
            place_submenu(start_rect, at, w, h, 3, rule, screen_rect)
        });
        let accessories = menu(
            &[
                "Accessibility>",
                "Communications>",
                "Entertainment>",
                "Games>",
                "System Tools>",
                "Calculator",
                "Imaging",
                "Notepad",
                "Paint",
                "WordPad",
            ],
            Some(1),
            true,
        );
        let at = entry(p_rect, &p_layout, 0);
        let (a_rect, a_layout) = s.menu(&WIN98, f98, &accessories, MenuStyle::Cascade, |w, h| {
            place_submenu(p_rect, at, w, h, 3, rule, screen_rect)
        });
        let communications = menu(
            &[
                "Dial-Up Networking",
                "Direct Cable Connection",
                "HyperTerminal",
                "ISDN Configuration Wizard",
                "Phone Dialer",
            ],
            None,
            true,
        );
        let at = entry(a_rect, &a_layout, 1);
        let (c_rect, _) = s.menu(&WIN98, f98, &communications, MenuStyle::Cascade, |w, h| {
            place_submenu(a_rect, at, w, h, 3, rule, screen_rect)
        });
        let mut case = screen_case(
            "win98-start-cascade",
            "win98-startmenu-programs-accessories-communications-cutout-564x405.png",
            s,
            (2, 73),
            &[start_rect, button, p_rect, a_rect, c_rect],
        );
        case.vga = true;
        out.push(case);
    }
    {
        let mut s = Screen::new(640, 480, rgb(0xFFFFFF));
        s.fb.blit(&button_fb, button.x, button.y);
        let start = start98("", None);
        let (start_rect, _) =
            s.menu(&WIN98, f98, &start, MenuStyle::Start, |w, h| place_start_menu(button, w, h, screen_rect));
        let mut case = screen_case(
            "win98-start-menu-alone",
            "win98-startmenu-nohighlight-cutout-168x405.png",
            s,
            (2, 73),
            &[start_rect, button],
        );
        case.vga = true;
        out.push(case);
    }

    // The dialogs. (Windows 98's title bar gradient depends only on the window: with the rule
    // measured on the reference PC, `window.rs`, all three match whole, the title bars too.)
    let dialogs: [(&'static str, &'static str, DialogKind); 3] = [
        ("win98-run", "dialogs/win98-run-dialog-347x163.png", DialogKind::Run),
        ("win98-shut-down", "dialogs/win98-shutdown-dialog-323x192.png", DialogKind::ShutDown),
        ("win98-log-off", "dialogs/win98-logoff-dialog-288x123.png", DialogKind::LogOff),
    ];
    for (name, file, kind) in dialogs {
        let mut d = Dialog::new(kind, Era::Win98);
        d.caret_on = false; // the Run screenshot caught the caret off
        if kind == DialogKind::ShutDown {
            d.choice = 0;
            d.focus = dialog::Part::Option(0);
        }
        let l = dialog::layout(&d);
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        dialog::paint_dialog(&mut fb, &WIN98, f98, &d, &l);
        out.push(Case { name, file, regions: vec![whole(&fb, (0, 0))], ours: fb, masks: Vec::new(), vga: false });
    }
}

/// The dialogs, each as GUIdebook shows it: nothing typed yet in Run, the first choice of
/// Shut Down, Yes in Log Off. (The dialog icons were cut from these same screenshots.)
fn dialog_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    let dialogs: [(&'static str, &'static str, DialogKind, Era); 5] = [
        ("win95-run", "dialogs/win95-run-dialog-347x163.png", DialogKind::Run, Era::Win95),
        ("win95-shut-down", "dialogs/win95-shutdown-dialog-347x222.png", DialogKind::ShutDown, Era::Win95),
        ("win2000-run", "dialogs/win2000-run-dialog-347x179.png", DialogKind::Run, Era::Win2000),
        ("win2000-shut-down", "dialogs/win2000-shutdown-dialog-417x269.png", DialogKind::ShutDown, Era::Win2000),
        ("win2000-log-off", "dialogs/win2000-logoff-dialog-288x123.png", DialogKind::LogOff, Era::Win2000),
    ];
    for (name, file, kind, era) in dialogs {
        let (theme, fonts) = if era == Era::Win95 { (&WIN95, f95) } else { (&WIN2000, f2k) };
        let d = Dialog::new(kind, era);
        let l = dialog::layout(&d);
        let mut fb = Framebuffer::new(l.width, l.height, 0);
        dialog::paint_dialog(&mut fb, theme, fonts, &d, &l);
        out.push(Case { name, file, regions: vec![whole(&fb, (0, 0))], ours: fb, masks: Vec::new(), vga: false });
    }
}

impl HelpScrollbar {
    fn source(&self) -> (&'static str, i32, i32, i32) {
        match (self.era, self.thumb_height != 0) {
            (Era::Win98, true) => ("vm/win98-truecolor-mine-help-topics.png", 553, 65, 294),
            (Era::Win98, false) => ("vm/win98-truecolor-mine-help-article-object.png", 553, 65, 0),
            (Era::Win2000, true) => ("vm/win2000-truecolor-mine-using-help.png", 758, 95, 308),
            (Era::Win2000, false) => ("vm/win2000-truecolor-mine-help-topics.png", 758, 95, 0),
            _ => unreachable!("HTML Help is used on Windows 98 and 2000"),
        }
    }

    fn expected(&self, dir: &Path) -> Framebuffer {
        let (capture, x, y, source_thumb) = self.source();
        let enabled = self.thumb_height != 0;
        let source = read_png(&dir.join(capture));
        let mut bar = Framebuffer::new(16, 379, 0);
        for dy in 0..bar.height {
            let ty = dy - 16;
            let source_y = if !enabled || !(16..363).contains(&dy) || ty < 2 {
                dy // intact buttons and upper two thumb bevels
            } else if ty < self.thumb_height - 2 {
                18 // middle thumb row, including both side bevels
            } else if ty < self.thumb_height {
                16 + source_thumb - (self.thumb_height - ty)
            } else {
                360 + dy % 2 // track stipple, preserving scanline parity
            };
            for dx in 0..bar.width {
                bar.set(dx, dy, source.get(x + dx, y + source_y));
            }
        }
        bar
    }
}

/// Compares the measured regions. The return values distinguish mismatches from the
/// authored words and document-driven scrollbar/icon changes in the historical screenshot.
fn compare(case: &Case, dir: &Path) -> (usize, usize, usize) {
    // 16-color captures in Windows' own colors (`sixteen_cases`); any other as it is.
    let reference = sixteen_cases::read_capture(&dir.join(case.file));
    // Our pixels pasted over a copy of the screenshot, so both share coordinates.
    let mut ours = reference.clone();
    let mut compared = vec![false; reference.pixels.len()];
    let mut authored_ink = 0;
    let mut reflow_pixels = 0;
    let mut expected_render = reference.clone();
    for r in &case.regions {
        if let Some(bar) = &r.reflowed_scrollbar {
            let expected = bar.expected(dir);
            for dy in 0..expected.height {
                for dx in 0..expected.width {
                    if reference.get(bar.at.0 + dx, bar.at.1 + dy) != expected.get(dx, dy) {
                        reflow_pixels += 1;
                    }
                }
            }
            expected_render.blit(&expected, bar.at.0, bar.at.1);
        }
        if let Some((from, to)) = r.reflowed_note {
            expected_render.fill(from, rgb(0xffffff));
            for y in 0..from.h {
                for x in 0..from.w {
                    expected_render.set(to.x + x, to.y + y, reference.get(from.x + x, from.y + y));
                }
            }
            for bounds in [from, to] {
                for y in bounds.y..bounds.bottom() {
                    for x in bounds.x..bounds.right() {
                        reflow_pixels += usize::from(reference.get(x, y) != expected_render.get(x, y));
                    }
                }
            }
        }
        for y in 0..r.reference.h {
            for x in 0..r.reference.w {
                let (rx, ry) = (r.reference.x + x, r.reference.y + y);
                if case.masks.iter().any(|m| m.contains(rx, ry)) {
                    continue;
                }
                let p = case.ours.get(r.ours.0 + x, r.ours.1 + y);
                let expected = expected_render.get(rx, ry);
                let note = r.reflowed_note.is_some_and(|(_, to)| to.contains(rx, ry));
                if let Some((_, paper)) = r.authored_text.filter(|(bounds, _)| p != expected && !note && bounds.contains(rx, ry)) {
                    // The guest Help viewers rasterize black type, blue links, and green
                    // glossary links without antialiasing. Keep comparing visible paper.
                    let ink = |pixel: u32| matches!(pixel & 0x00ff_ffff, 0 | 0x0000cc | 0x0000ff | 0x006600)
                        || (matches!(paper, HelpPaper::Solid(0xffffe2)) && pixel == rgb(0x008000));
                    if (ink(p) && (ink(expected) || paper.contains(expected))) || (ink(expected) && paper.contains(p)) {
                        authored_ink += 1;
                        ours.set(rx, ry, p);
                        continue;
                    }
                }
                ours.set(rx, ry, if case.vga { nearest_vga(p) } else { p });
                compared[(ry * reference.width + rx) as usize] = true;
            }
        }
    }
    let mut count = 0;
    let mut marked = Framebuffer::new(reference.width, reference.height, 0);
    for (i, (&e, &a)) in expected_render.pixels.iter().zip(&ours.pixels).enumerate() {
        let fade = |p: u32, shift: u32| (((p >> shift) & 0xFF) + 3 * 255) / 4;
        marked.pixels[i] = if !compared[i] {
            rgb(0xB0E0FF)
        } else if (e ^ a) & 0x00FF_FFFF != 0 {
            count += 1;
            rgb(0xFF0000)
        } else {
            rgb((fade(e, 16) << 16) | (fade(e, 8) << 8) | fade(e, 0))
        };
    }
    // Screenshot, ours, differences: stacked, each enlarged.
    let zoom = if reference.width > 400 { 2 } else { 4 };
    let gap = 4;
    let mut sheet = Framebuffer::new(reference.width * zoom, (reference.height * zoom + gap) * 3, rgb(0xFF00FF));
    for (i, fb) in [&reference, &ours, &marked].into_iter().enumerate() {
        let big = bevel_core::scale::upscale(fb, zoom);
        sheet.blit(&big, 0, i as i32 * (reference.height * zoom + gap));
    }
    // BEVEL95_OUT names another folder for the pictures (the captures' folder stays untouched).
    let out = std::env::var_os("BEVEL95_OUT").map(PathBuf::from).unwrap_or_else(|| dir.join("out"));
    std::fs::create_dir_all(&out).unwrap();
    write_png(&out.join(format!("{}.png", case.name)), &sheet);
    write_png(&out.join(format!("{}.ours.png", case.name)), &ours);
    (count, authored_ink, reflow_pixels)
}

#[test]
#[ignore = "needs the reference screenshots; run it with `cargo compare-refs` (see the module docs)"]
fn compare_with_reference_screenshots() {
    let dir = refs_dir();
    assert!(
        dir.is_dir(),
        "no reference folder at {}: the reference captures are not part of the public project. \
         tools/fetch-refs.ps1 downloads the published screenshots there; the Reference PCs' captures \
         (reference/refs/vm) exist only where BEVEL95_PRIVATE (default: private) points, or set BEVEL95_REFS",
        dir.display()
    );
    let only = std::env::var("BEVEL95_CASE").ok();
    let mut report = Vec::new();
    let mut total = 0;
    for case in cases().iter().filter(|c| only.as_deref().is_none_or(|o| c.name.contains(o))) {
        // The emulator's captures cannot be downloaded like the screenshots (see
        // docs/theme-spec.md, "Sources"): without them those cases are skipped, not failed.
        let missing_scrollbar = case.regions.iter().filter_map(|r| r.reflowed_scrollbar.as_ref())
            .any(|bar| !dir.join(bar.source().0).is_file());
        if (case.file.starts_with("v86/") || case.file.starts_with("vm/") || case.file.starts_with("solitaire/"))
            && (!dir.join(case.file).exists() || missing_scrollbar) {
            report.push(format!("{:<28} skipped (no capture)", case.name));
            continue;
        }
        let (n, authored_ink, reflow) = compare(case, &dir);
        total += n;
        let prose = if authored_ink == 0 { String::new() } else { format!(" ({authored_ink} changed Help text pixels)") };
        let layout = if reflow == 0 { String::new() } else { format!(" ({reflow} document-reflow pixels)") };
        report.push(format!("{:<28} {n} pixels differ{prose}{layout}", case.name));
    }
    println!("\n{}\n(pictures in {})", report.join("\n"), std::env::var_os("BEVEL95_OUT").map(PathBuf::from).unwrap_or_else(|| dir.join("out")).display());
    assert_eq!(total, 0, "some drawings differ from the screenshots");
}

/// A Windows 95 folder window as the Reference PC showed one: its items as Bevel95 reads them
/// (names, pictures; the PC's own files' pictures, which Bevel95 reads from the shell, are masked
/// by the case), painted by `FolderWindow` at its place on the screen.
fn folder_window(
    fonts: &Fonts,
    title: &str,
    files: bool,
    at: (i32, i32),
    size: (i32, i32),
    items: Vec<bevel_core::desktop_icons::Item>,
    state: impl FnOnce(&mut bevel_core::folder::FolderWindow),
) -> Framebuffer {
    use bevel_core::folder::{FolderWindow, art};
    use bevel_core::kit::Form;
    let small = match title {
        "My Computer" => art::Small::MyComputer,
        "Win95 (C:)" => art::Small::Drive(art::DriveKind::Fixed),
        _ => art::Small::Folder,
    };
    let mut w = FolderWindow::new(Era::Win95, title, art::small(Era::Win95, small), files, size);
    w.set_screen(at.0, at.1);
    w.layout(fonts);
    w.set_items(items);
    state(&mut w);
    w.layout(fonts);
    let mut fb = Framebuffer::new(w.size_now().0, w.size_now().1, 0);
    w.paint(&mut fb, &WIN95, fonts);
    fb
}

/// The items of the Reference PC's folders (`kind`: 'f' a folder, 'p' a file whose picture is the
/// PC's own, masked; sizes as its status bar added them up).
fn folder_items(list: &[(&str, char, u64)]) -> Vec<bevel_core::desktop_icons::Item> {
    use bevel_core::desktop_icons::{Item, Picture};
    list.iter()
        .map(|&(name, kind, size)| Item {
            id: format!("C:\\x\\{name}"),
            name: name.to_string(),
            picture: if kind == 'f' { Picture::Folder } else { Picture::Own(None) },
            shortcut: false,
            folder: kind == 'f',
            type_name: String::new(),
            size,
            modified: 0,
        })
        .collect()
}

/// My Computer on the Reference PC: its floppy and hard disk, Control Panel and Printers.
fn my_computer_items() -> Vec<bevel_core::desktop_icons::Item> {
    use bevel_core::desktop_art::Art;
    use bevel_core::desktop_icons::{Item, Picture};
    use bevel_core::folder::art::DriveKind;
    let item = |id: &str, name: &str, art: Art| Item {
        id: id.to_string(),
        name: name.to_string(),
        picture: Picture::Art(art),
        shortcut: false,
        folder: false,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    vec![
        item("A:\\", "3\u{bd} Floppy (A:)", Art::Drive(DriveKind::Floppy)),
        item("C:\\", "Win95 (C:)", Art::Drive(DriveKind::Fixed)),
        item("::control-panel", "Control Panel", Art::ControlPanel),
        item("::printers", "Printers", Art::Printers),
    ]
}

/// The cells of a folder window's items whose pictures the case leaves out: the PC's own pictures
/// of its files, and folders the virtual PC drew gray (its drawing, not Windows 95's look).
fn picture_masks(at: (i32, i32), list_at: (i32, i32), cols: i32, which: &[usize]) -> Vec<Rect> {
    which
        .iter()
        .map(|&i| {
            let (c, r) = (i as i32 % cols, i as i32 / cols);
            Rect::new(at.0 + list_at.0 + c * 75 + 21, at.1 + list_at.1 + r * 75 + 2, 32, 32)
        })
        .collect()
}

/// Whole Windows 95 folder windows against the Reference PC (`vm/win95-truecolor-folder-*`): the
/// frame, title, menu bar, the icons in rows with their labels, focus and selection, the scroll
/// bars and the status bar's parts and words. Masked: the mouse pointer where it was, the PC's
/// pictures of its own files, and folder pictures the virtual PC drew gray.
fn folder_window_cases(out: &mut Vec<Case>, f95: &Fonts) {
    use bevel_core::folder::Space;
    let pointer = |x: i32, y: i32| Rect::new(x, y, 13, 21);
    // The Help menu's title is left out of ours (nothing in it works yet): its place is masked.
    let help = bevel_core::window::menu_bar_layout(f95, &["&File", "&Edit", "&View", "&Help"])[3];
    let whole_window = |fb: &Framebuffer, at: (i32, i32)| {
        vec![region((0, 0), Rect::new(at.0, at.1, fb.width, fb.height))]
    };
    let first = out.len();
    let space = |w: &mut bevel_core::folder::FolderWindow| {
        w.spaces.insert("C:\\".into(), Space { free: 284 << 20, capacity: 511 << 20 });
    };

    // My Computer as it opens: nothing selected, the focus on the first icon.
    let at = (44, 44);
    let ours = folder_window(f95, "My Computer", false, at, (210, 219), my_computer_items(), |w| w.view.focus = Some(0));
    out.push(Case { name: "vm-win95-folder-mycomputer", file: "vm/win95-truecolor-folder-mycomputer.png", regions: whole_window(&ours, at), ours, masks: vec![], vga: false });
    // Win95 (C:) selected: its free space.
    let ours = folder_window(f95, "My Computer", false, at, (210, 219), my_computer_items(), |w| {
        space(w);
        w.view.select_only(Some(1));
    });
    out.push(Case {
        name: "vm-win95-folder-mc-sel-drive",
        file: "vm/win95-truecolor-folder-mc-sel-drive.png",
        regions: whole_window(&ours, at),
        ours,
        masks: vec![pointer(161, 110)],
        vga: false,
    });
    // Another window in front: no selection shows.
    let ours = folder_window(f95, "My Computer", false, at, (210, 219), my_computer_items(), |w| {
        space(w);
        w.view.select_only(Some(1));
        bevel_core::kit::Form::set_active(w, false);
    });
    out.push(Case { name: "vm-win95-folder-mc-inactive", file: "vm/win95-truecolor-folder-mc-inactive-sel.png", regions: whole_window(&ours, at), ours, masks: vec![], vga: false });

    // Drive C: (gray Program Files and Windows: the virtual PC's drawing).
    let c_items = || folder_items(&[("Program Files", 'f', 0), ("Win95", 'f', 0), ("Windows", 'f', 0), ("Command", 'p', 93_880), ("Netlog", 'p', 489)]);
    let at = (66, 66);
    let c_state = |w: &mut bevel_core::folder::FolderWindow| {
        w.hidden = 13;
        w.total_size = 939 * 1024 + 300;
    };
    let ours = folder_window(f95, "Win95 (C:)", true, at, (265, 219), c_items(), |w| {
        c_state(w);
        w.view.focus = Some(0);
    });
    let mut masks = picture_masks(at, (6, 44), 3, &[0, 2, 3, 4]);
    masks.push(pointer(161, 110));
    out.push(Case { name: "vm-win95-folder-drive-c", file: "vm/win95-truecolor-folder-drive-c.png", regions: whole_window(&ours, at), ours, masks, vga: false });
    let ours = folder_window(f95, "Win95 (C:)", true, at, (265, 219), c_items(), |w| {
        c_state(w);
        w.view.select_only(Some(4));
    });
    let mut masks = picture_masks(at, (6, 44), 3, &[0, 2, 3, 4]);
    masks.push(pointer(183, 203));
    out.push(Case { name: "vm-win95-folder-c-sel-file", file: "vm/win95-truecolor-folder-c-sel-file.png", regions: whole_window(&ours, at), ours, masks, vga: false });

    // Program Files: long names on several lines.
    let pf = || {
        folder_items(&[
            ("Accessories", 'f', 0),
            ("ICW-Internet Connection Wizard", 'f', 0),
            ("Internet Explorer", 'f', 0),
            ("NetMeeting", 'f', 0),
            ("Online Services", 'f', 0),
            ("The Microsoft Network", 'f', 0),
            ("Windows Messaging", 'f', 0),
        ])
    };
    let at = (88, 88);
    let ours = folder_window(f95, "Program Files", true, at, (265, 294), pf(), |w| w.view.focus = Some(0));
    let mut masks = picture_masks(at, (6, 44), 3, &[0, 2, 4, 6]);
    masks.push(pointer(110, 128));
    out.push(Case { name: "vm-win95-folder-programfiles", file: "vm/win95-truecolor-folder-programfiles.png", regions: whole_window(&ours, at), ours, masks, vga: false });
    // The same window made narrower: the icons stay, scroll bars come.
    let ours = folder_window(f95, "Program Files", true, at, (265, 294), pf(), |w| {
        w.view.select_only(Some(0));
        w.resize((175, 294), false);
    });
    let mut masks = picture_masks(at, (6, 44), 3, &[0, 2, 4, 6]);
    masks.push(pointer(250, 100));
    out.push(Case { name: "vm-win95-folder-pf-narrow", file: "vm/win95-truecolor-folder-pf-narrow.png", regions: whole_window(&ours, at), ours, masks, vga: false });

    // 26 folders: taller than the largest first window, a scroll bar down.
    let names: Vec<String> = ('a'..='z').map(|c| c.to_string()).collect();
    let list: Vec<(&str, char, u64)> = names.iter().map(|n| (n.as_str(), 'f', 0)).collect();
    let at = (22, 22);
    let ours = folder_window(f95, "t26", true, at, (490, 412), folder_items(&list), |w| w.view.focus = Some(0));
    let mut masks = picture_masks(at, (6, 44), 6, &(0..26).collect::<Vec<_>>());
    masks.push(pointer(250, 100));
    out.push(Case { name: "vm-win95-folder-t26", file: "vm/win95-truecolor-folder-t26.png", regions: whole_window(&ours, at), ours, masks, vga: false });

    // A menu entry highlighted: its help text across the status bar (My Computer's File menu).
    let at = (44, 44);
    let ours = folder_window(f95, "My Computer", false, at, (210, 219), my_computer_items(), |w| {
        w.view.focus = Some(0);
        w.help = Some("Creates shortcuts to the selected items.".into());
    });
    for case in &mut out[first..] {
        let at = (case.regions[0].reference.x, case.regions[0].reference.y);
        case.masks.push(Rect::new(at.0 + help.x + 1, at.1 + help.y + 1, help.w, help.h));
    }
    let status = Rect::new(0, 219 - 4 - 19, 210, 19);
    out.push(Case {
        name: "vm-win95-folder-menu-help",
        file: "vm/win95-truecolor-folder-mc-menu-file.png",
        regions: vec![region((status.x, status.y), Rect::new(at.0 + status.x, at.1 + status.y, status.w, status.h))],
        ours,
        masks: vec![],
        vga: false,
    });
}

/// The folder window's menus against the Reference PC's (`vm/win95-truecolor-folder-*menu*`): each
/// of our rows with the same entry's row of Windows' fuller menu (laid out from Windows' whole
/// list), from the menu's left edge to 20 pixels short of the narrower menu's right edge, and the
/// menus' top-left corners, as the desktop's menus are compared.
fn folder_menu_cases(out: &mut Vec<Case>, f95: &Fonts) {
    use bevel_core::desktop::{self, ItemKind};
    use bevel_core::folder::menus::{self, State, Sub};
    let state = |kinds: &[ItemKind], files: bool| State { era: Era::Win95, kinds: kinds.to_vec(), files, can_paste: false, status_bar: true, auto_arrange: false, nav: Default::default(), tree: None, mode: Default::default(), web: None, undo: None };
    const FILE_NONE_MC: &[&str] = &["Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "&Close"];
    const FILE_DRIVE: &[&str] =
        &["&Open", "&Explore", "&Find...", "-", "F&ormat...", "-", "Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "&Close"];
    const FILE_FOLDER: &[&str] = &[
        "&Open", "&Explore", "&Find...", "-", "Se&nd To>", "-", "Ne&w>", "-", "Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "&Close",
    ];
    const FILE_FILE: &[&str] =
        &["&Open", "&Print", "-", "Se&nd To>", "-", "Ne&w>", "-", "Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "&Close"];
    const FILE_NONE: &[&str] = &["Ne&w>", "-", "Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "&Close"];
    const EDIT: &[&str] =
        &["&Undo\tCtrl+Z", "-", "Cu&t\tCtrl+X", "&Copy\tCtrl+C", "&Paste\tCtrl+V", "Paste &Shortcut", "-", "Select &All\tCtrl+A", "&Invert Selection"];
    const VIEW: &[&str] =
        &["&Toolbar", "Status &Bar", "-", "Lar&ge Icons", "S&mall Icons", "&List", "&Details", "-", "Arrange &Icons>", "Lin&e up Icons", "-", "&Refresh", "&Options..."];
    const BACK_C: &[&str] = &["&View>", "-", "Arrange &Icons>", "Lin&e up Icons", "-", "&Paste", "Paste &Shortcut", "-", "Ne&w>", "-", "P&roperties"];
    const BACK_MC: &[&str] = &["&View>", "-", "Arrange &Icons>", "Lin&e up Icons", "-", "&Paste", "Paste &Shortcut", "-", "P&roperties"];
    const NEW: &[&str] = &["&Folder", "&Shortcut", "-", "Wave Sound", "Text Document", "WordPad Document", "Bitmap Image"];
    const ITEM_DRIVE: &[&str] = &["&Open", "&Explore", "&Find...", "-", "F&ormat...", "-", "Create &Shortcut", "-", "P&roperties"];
    const ITEM_FILE: &[&str] = &["&Open", "&Print", "-", "Se&nd To>", "-", "Cu&t", "&Copy", "-", "Create &Shortcut", "&Delete", "Rena&me", "-", "P&roperties"];
    enum Ours {
        Bar(usize, State),
        Back(State),
        Sub(Sub, State),
        Item(ItemKind),
    }
    type Shot<'a> = (&'static str, &'static str, (i32, i32), &'a [&'a str], Option<usize>, Ours);
    let shots: Vec<Shot> = vec![
        ("vm-win95-folder-menu-file-mc", "vm/win95-truecolor-folder-mc-menu-file.png", (48, 85), FILE_NONE_MC, Some(0), Ours::Bar(0, state(&[], false))),
        ("vm-win95-folder-menu-file-drive", "vm/win95-truecolor-folder-mc-menu-file-drive.png", (48, 85), FILE_DRIVE, Some(0), Ours::Bar(0, state(&[ItemKind::Drive], false))),
        ("vm-win95-folder-menu-file-folder", "vm/win95-truecolor-folder-c-menu-file-folder.png", (70, 107), FILE_FOLDER, Some(0), Ours::Bar(0, state(&[ItemKind::Folder], true))),
        ("vm-win95-folder-menu-file-file", "vm/win95-truecolor-folder-c-menu-file-file.png", (70, 107), FILE_FILE, Some(0), Ours::Bar(0, state(&[ItemKind::File], true))),
        ("vm-win95-folder-menu-file-none", "vm/win95-truecolor-folder-c-menu-file-none.png", (70, 107), FILE_NONE, Some(0), Ours::Bar(0, state(&[], true))),
        ("vm-win95-folder-menu-edit-mc", "vm/win95-truecolor-folder-mc-menu-edit.png", (76, 85), EDIT, Some(0), Ours::Bar(1, state(&[], false))),
        ("vm-win95-folder-menu-edit-file", "vm/win95-truecolor-folder-c-menu-edit-file.png", (98, 107), EDIT, Some(0), Ours::Bar(1, state(&[ItemKind::File], true))),
        ("vm-win95-folder-menu-view-mc", "vm/win95-truecolor-folder-mc-menu-view.png", (106, 85), VIEW, Some(0), Ours::Bar(2, state(&[], false))),
        ("vm-win95-folder-menu-back-c", "vm/win95-truecolor-folder-c-rmenu-empty.png", (300, 245), BACK_C, None, Ours::Back(state(&[], true))),
        ("vm-win95-folder-menu-back-mc", "vm/win95-truecolor-folder-mc-rmenu-empty.png", (220, 226), BACK_MC, None, Ours::Back(state(&[], false))),
        ("vm-win95-folder-menu-new", "vm/win95-truecolor-folder-c-menu-file-new.png", (186, 107), NEW, Some(0), Ours::Sub(Sub::New, state(&[], true))),
        ("vm-win95-folder-menu-item-drive", "vm/win95-truecolor-folder-mc-rmenu-drive.png", (161, 110), ITEM_DRIVE, None, Ours::Item(ItemKind::Drive)),
        ("vm-win95-folder-menu-item-file", "vm/win95-truecolor-folder-c-rmenu-file.png", (183, 202), ITEM_FILE, None, Ours::Item(ItemKind::File)),
    ];
    for (name, file, at, windows_labels, hover, which) in shots {
        let ours_menu = match &which {
            Ours::Bar(title, s) => menus::menu(&menus::bar_rows(s, *title)),
            Ours::Back(s) => menus::menu(&menus::background_rows(s)),
            Ours::Sub(sub, s) => menus::menu(&menus::sub_rows(s, *sub)),
            Ours::Item(kind) => desktop::item_menu(Era::Win95, &[*kind], false),
        };
        let plain = |l: &str| l.replace('&', "").split('\t').next().unwrap_or("").trim_end_matches('>').to_string();
        // Windows' highlighted entry, where ours has it (Undo and Toolbar are not ours).
        let ours_hover = hover.and_then(|h| ours_menu.entries.iter().position(|e| plain(&e.label) == plain(windows_labels[h])));
        let ours_menu = Menu { hover: ours_hover, show_keys: true, ..ours_menu };
        let mut windows_menu = menu(windows_labels, hover, true);
        windows_menu.explorer_bold = true;
        if let Some(e) = windows_menu.entries.first_mut().filter(|e| e.label == "&Open") {
            e.bold = true;
        }
        let mut s = Screen::new(800, 600, WIN95.colors.desktop);
        let (ours_rect, ours_layout) = s.menu(&WIN95, f95, &ours_menu, MenuStyle::Popup, |_, _| at);
        let win_layout = paint::layout_menu(&WIN95, f95, &windows_menu, MenuStyle::Popup, 600);
        let width = ours_layout.width.min(win_layout.width) - 20;
        let mut regions = vec![region((at.0, at.1), Rect::new(at.0, at.1, width, 3))];
        for (i, e) in ours_menu.entries.iter().enumerate() {
            // Windows 95 drew the New menu's kinds of file (shell items) 2 pixels further left and
            // 1 higher than its other entries; Bevel95 does not copy that yet (todo.md).
            if e.kind == EntryKind::Separator || e.label == "Text Document" {
                continue;
            }
            let Some(j) = windows_labels.iter().position(|w| plain(w) == plain(&e.label)) else { continue };
            let (o, w) = (entry(ours_rect, &ours_layout, i), win_layout.rects[j]);
            regions.push(region((o.x - 3, o.y), Rect::new(at.0 + w.x - 3, at.1 + w.y, width, w.h)));
        }
        let mut masks = vec![Rect::new(at.0, at.1, 13, 21)];
        if name == "vm-win95-folder-menu-edit-file" {
            // The pointer, still where it clicked on Netlog.
            masks.push(Rect::new(183, 202, 13, 21));
        }
        out.push(Case { name, file, regions, ours: s.fb, masks, vga: false });
    }
}
/// A Windows 98 or 2000 folder window as the Reference PC showed one (web view off): painted by
/// `FolderWindow` at its place on the screen.
#[allow(clippy::too_many_arguments)]
fn explorer_window(
    fonts: &Fonts,
    era: Era,
    title: &str,
    small: bevel_core::folder::art::Small,
    files: bool,
    at: (i32, i32),
    size: (i32, i32),
    items: Vec<bevel_core::desktop_icons::Item>,
    address: &str,
    nav: bevel_core::folder::menus::Nav,
    state: impl FnOnce(&mut bevel_core::folder::FolderWindow),
) -> Framebuffer {
    use bevel_core::folder::{FolderWindow, art};
    use bevel_core::kit::Form;
    let icon = art::small(era, if small == art::Small::ClosedFolder { art::Small::Folder } else { small });
    let mut w = FolderWindow::new(era, title, icon.clone(), files, size);
    w.set_screen(at.0, at.1);
    w.layout(fonts);
    w.show_place(title, icon, files, address, art::listed(era, small));
    w.set_nav(nav);
    w.set_items(items);
    state(&mut w);
    w.layout(fonts);
    let mut fb = Framebuffer::new(w.size_now().0, w.size_now().1, 0);
    w.paint(&mut fb, bevel_core::theme::theme_in(era, REF_DEPTH.get()), fonts);
    fb
}

thread_local! {
    /// The colors `explorer_window` draws in (the 16-color cases set it while they draw).
    static REF_DEPTH: std::cell::Cell<bevel_core::theme::Depth> = const { std::cell::Cell::new(bevel_core::theme::Depth::TrueColor) };
}

/// My Computer's items as each look's Reference PC showed them (98: floppy, (C:), (D:), Printers,
/// Control Panel; 2000: its drive names and Control Panel).
fn explorer_my_computer(era: Era) -> Vec<bevel_core::desktop_icons::Item> {
    use bevel_core::desktop_art::Art;
    use bevel_core::desktop_icons::{Item, Picture};
    use bevel_core::folder::art::DriveKind;
    let item = |id: &str, name: &str, art: Art| Item {
        id: id.to_string(),
        name: name.to_string(),
        picture: Picture::Art(art),
        shortcut: false,
        folder: false,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    if era == Era::Win98 {
        vec![
            item("A:\\", "3\u{bd} Floppy (A:)", Art::Drive(DriveKind::Floppy)),
            item("C:\\", " (C:)", Art::Drive(DriveKind::Fixed)),
            item("D:\\", " (D:)", Art::Drive(DriveKind::CdRom)),
            item("::printers", "Printers", Art::Printers),
            item("::control-panel", "Control Panel", Art::ControlPanel),
        ]
    } else {
        vec![
            item("A:\\", "3\u{bd} Floppy (A:)", Art::Drive(DriveKind::Floppy)),
            item("C:\\", "Local Disk (C:)", Art::Drive(DriveKind::Fixed)),
            item("D:\\", "Compact Disc (D:)", Art::Drive(DriveKind::CdRom)),
            item("::control-panel", "Control Panel", Art::ControlPanel),
        ]
    }
}

/// Whole Windows 98 and 2000 folder windows against the Reference PCs
/// (`vm/<era>-truecolor-folder-*`): the frame, the rebar with its menu titles, throbber, toolbar
/// and address bar, the icons and the status bar's three parts. Masked: the pointer, the PCs'
/// pictures of their own files, and what Bevel95 leaves out (98's Favorites and Help titles,
/// 2000's buttons after Up, the items My Computer has here only).
fn explorer_window_cases(out: &mut Vec<Case>, f98: &Fonts, f2k: &Fonts) {
    use bevel_core::folder::art::{DriveKind, Small};
    use bevel_core::folder::menus::Nav;
    let pointer = |x: i32, y: i32| Rect::new(x, y, 13, 21);
    let whole = |fb: &Framebuffer, at: (i32, i32)| vec![region((0, 0), Rect::new(at.0, at.1, fb.width, fb.height))];
    let cells = |at: (i32, i32), list: (i32, i32), cols: i32, which: &[usize]| picture_masks(at, list, cols, which);

    // Windows 98: My Computer (at 0, 0, 512 x 461), nothing selected, the focus on the floppy.
    let at = (0, 0);
    let ours = explorer_window(f98, Era::Win98, "My Computer", Small::MyComputer, false, at, (512, 461), explorer_my_computer(Era::Win98), "My Computer", Nav::default(), |w| {
        w.view.focus = Some(0);
    });
    let mut masks = vec![
        Rect::new(158, 25, 308, 22), // Favorites, Help
        Rect::new(5, 440, 200, 15),  // "7 object(s)": Dial-Up Networking and Scheduled Tasks
        Rect::new(381, 120, 75, 75),
        Rect::new(6, 195, 75, 75),
        pointer(700, 300),
    ];
    masks.extend(cells(at, (6, 120), 6, &[5, 6]));
    out.push(Case { name: "vm-win98-folder-x-mycomputer", file: "vm/win98-truecolor-folder-mycomputer.png", regions: whole(&ours, at), ours, masks, vga: false });

    // (C:) after a double-click: the same window, Back and Up lit.
    let c_items = folder_items(&[
        ("My Documents", 'f', 0),
        ("Program Files", 'f', 0),
        ("Windows", 'f', 0),
        ("Autoexec", 'p', 0),
        ("Command", 'p', 0),
        ("Frunlog", 'p', 0),
        ("Netlog", 'p', 0),
        ("Scandisk.log", 'p', 0),
    ]);
    let nav = Nav { back: true, up: true, ..Nav::default() };
    let ours = explorer_window(f98, Era::Win98, " (C:)", Small::Drive(DriveKind::Fixed), true, at, (512, 461), c_items, "C:\\", nav, |w| {
        w.hidden = 12;
        w.total_size = 1_070_000;
        w.view.focus = Some(0);
    });
    let mut masks = vec![Rect::new(158, 25, 308, 22), pointer(700, 300)];
    masks.extend(cells(at, (6, 120), 6, &[0, 3, 4, 5, 6, 7]));
    out.push(Case { name: "vm-win98-folder-x-c", file: "vm/win98-truecolor-folder-c-after-dblclick.png", regions: whole(&ours, at), ours, masks, vga: false });

    // Windows 2000: My Computer after Backspace (at 110, 110, 680 x 458), the focus on the floppy.
    let at = (110, 110);
    let nav = Nav { back: true, up: true, at_my_computer: true, ..Nav::default() };
    let ours = explorer_window(f2k, Era::Win2000, "My Computer", Small::MyComputer, false, at, (680, 458), explorer_my_computer(Era::Win2000), "My Computer", nav, |w| {
        w.view.focus = Some(0);
        w.view.keys_used = true;
        w.bar.show_keys = true;
    });
    let masks = vec![
        Rect::new(110 + 139, 159, 680 - 139 - 6, 22), // Search and what follows it
        Rect::new(230, 135, 514, 22),                  // Favorites, Tools, Help
        Rect::new(110 + 58 + 2, 110 + 75, 18, 18),     // the drive's picture 2000 left in the address bar
        pointer(60, 500),
    ];
    out.push(Case { name: "vm-win2000-folder-x-mycomputer", file: "vm/win2000-truecolor-folder-mc-after-backspace.png", regions: whole(&ours, at), ours, masks, vga: false });

    // Local Disk (C:) after a double-click.
    let c_items = folder_items(&[("Documents and Settings", 'f', 0), ("Program Files", 'f', 0), ("WINNT", 'f', 0)]);
    let nav = Nav { back: true, up: true, ..Nav::default() };
    let ours = explorer_window(f2k, Era::Win2000, "Local Disk (C:)", Small::Drive(DriveKind::Fixed), true, at, (680, 458), c_items, "Local Disk (C:)", nav, |w| {
        w.hidden = 2;
        w.view.focus = Some(0);
        // 2000 showed the focus frame in a window that had gone to another place.
        w.view.keys_used = true;
    });
    let masks = vec![Rect::new(110 + 139, 159, 680 - 139 - 6, 22), Rect::new(230, 135, 514, 22), pointer(60, 500)];
    out.push(Case { name: "vm-win2000-folder-x-c", file: "vm/win2000-truecolor-folder-c-after-dblclick.png", regions: whole(&ours, at), ours, masks, vga: false });
}

/// Windows 2000's Undo button, plain, against the Reference PC's (`win2000-truecolor-folder-mycomputer`):
/// only the button, as 2000's toolbar holds buttons Bevel95 leaves out before it (Search, History,
/// Move To, Copy To), so it stands elsewhere in ours. No True Color capture shows it under the
/// pointer or held down (its hot picture is 2000's own, `explorer_art`).
fn explorer_undo_cases(out: &mut Vec<Case>, f2k: &Fonts) {
    use bevel_core::folder::art::Small;
    use bevel_core::folder::menus::Nav;
    use bevel_core::folder::{Part, Tool};
    use bevel_core::kit::Form;
    let at = (110, 110);
    let mut button = None;
    let nav = Nav { up: true, ..Nav::default() };
    let ours = explorer_window(f2k, Era::Win2000, "My Computer", Small::MyComputer, false, at, (680, 458), Vec::new(), "My Computer", nav, |w| {
        button = w.rect(Part::Tool(Tool::Undo));
    });
    let r = button.expect("2000's toolbar has Undo");
    // On the PC: right after Delete, whose button began at x 504 of the screen (`delete_cold`).
    out.push(Case {
        name: "vm-win2000-folder-x-undo",
        file: "vm/win2000-truecolor-folder-mycomputer.png",
        regions: vec![region((r.x, r.y), Rect::new(504 + 24, at.1 + r.y, r.w, r.h))],
        ours,
        masks: Vec::new(),
        vga: false,
    });
}

/// The rebar of Windows 98 and 2000 folder windows in each state of its flat buttons, against the
/// Reference PCs' captures of the same window with the pointer on a button or pressing it (only
/// the rebar is compared: the window's own state is the case's base).
fn explorer_toolbar_cases(out: &mut Vec<Case>, f98: &Fonts, f2k: &Fonts) {
    use bevel_core::folder::art::{DriveKind, Small};
    use bevel_core::folder::menus::Nav;
    use bevel_core::folder::{Part, Tool};
    use bevel_core::kit::Form;
    let pointer = |x: i32, y: i32| Rect::new(x - 1, y - 1, 14, 22);
    #[derive(Clone, Copy)]
    enum S {
        Hot(Tool, bool),
        Pressed(Tool),
        HotTitle(usize),
        HotGo,
        Plain,
    }
    // (name, capture, era, at, size, in (C:) with Back lit, state, pointer)
    type Shot = (&'static str, &'static str, Era, (i32, i32), (i32, i32), u8, S, (i32, i32));
    let shots: Vec<Shot> = vec![
        ("vm-win98-folder-x-hot-cut", "vm/win98-truecolor-folder-hot-cut.png", Era::Win98, (0, 0), (512, 461), 0, S::Hot(Tool::Cut, false), (213, 51)),
        // In 16 colors (its 16-color toolbar: black line art). The pointer had left Cut: nothing hot.
        ("win98-16-folder-toolbar", "vm/win98-16color-folder-hot-cut.png", Era::Win98, (0, 0), (512, 461), 0, S::Plain, (263, 91)),
        ("vm-win98-folder-x-hot-copy", "vm/win98-truecolor-folder-hot-copy.png", Era::Win98, (0, 0), (512, 461), 0, S::Hot(Tool::Copy, false), (268, 51)),
        ("vm-win98-folder-x-hot-paste", "vm/win98-truecolor-folder-hot-paste.png", Era::Win98, (0, 0), (512, 461), 0, S::Hot(Tool::Paste, false), (322, 51)),
        ("vm-win98-folder-x-hot-undo", "vm/win98-truecolor-folder-hot-undo.png", Era::Win98, (0, 0), (512, 461), 0, S::Hot(Tool::Undo, false), (384, 50)),
        ("vm-win98-folder-x-pressed-cut", "vm/win98-truecolor-folder-pressed-cut.png", Era::Win98, (0, 0), (512, 461), 0, S::Pressed(Tool::Cut), (214, 51)),
        ("vm-win98-folder-x-hot-back", "vm/win98-truecolor-folder-hot-back-arrow.png", Era::Win98, (0, 0), (512, 461), 1, S::Hot(Tool::Back, true), (74, 51)),
        ("vm-win98-folder-x-pressed-back", "vm/win98-truecolor-folder-pressed-back.png", Era::Win98, (0, 0), (512, 461), 1, S::Pressed(Tool::Back), (15, 52)),
        ("vm-win98-folder-x-hot-forward", "vm/win98-truecolor-folder-hot-forward-arrow.png", Era::Win98, (0, 0), (512, 461), 2, S::Hot(Tool::Forward, true), (140, 51)),
        ("vm-win98-folder-x-hot-title", "vm/win98-truecolor-folder-hot-title-edit.png", Era::Win98, (0, 0), (512, 461), 3, S::HotTitle(1), (56, 40)),
        ("vm-win2000-folder-x-hot-back", "vm/win2000-truecolor-folder-hot-back.png", Era::Win2000, (110, 110), (680, 458), 1, S::Hot(Tool::Back, false), (172, 179)),
        ("vm-win2000-folder-x-hot-back-arrow", "vm/win2000-truecolor-folder-hot-back-arrow.png", Era::Win2000, (110, 110), (680, 458), 1, S::Hot(Tool::Back, true), (186, 179)),
        ("vm-win2000-folder-x-pressed-back", "vm/win2000-truecolor-folder-pressed-back.png", Era::Win2000, (110, 110), (680, 458), 1, S::Pressed(Tool::Back), (172, 178)),
        ("vm-win2000-folder-x-hot-up", "vm/win2000-truecolor-folder-hot-up.png", Era::Win2000, (110, 110), (680, 458), 1, S::Hot(Tool::Up, false), (240, 179)),
        ("vm-win2000-folder-x-hot-forward", "vm/win2000-truecolor-folder-hot-forward.png", Era::Win2000, (110, 110), (680, 458), 2, S::Hot(Tool::Forward, false), (208, 179)),
        ("vm-win2000-folder-x-hot-delete", "vm/win2000-truecolor-folder-hot-delete.png", Era::Win2000, (110, 110), (680, 458), 0, S::Hot(Tool::Delete, false), (525, 179)),
        ("vm-win2000-folder-x-pressed-delete", "vm/win2000-truecolor-folder-pressed-delete.png", Era::Win2000, (110, 110), (680, 458), 0, S::Pressed(Tool::Delete), (524, 178)),
        ("vm-win2000-folder-x-hot-go", "vm/win2000-truecolor-folder-hot-go.png", Era::Win2000, (110, 110), (680, 458), 1, S::HotGo, (770, 203)),
    ];
    for (name, file, era, at, size, place, s, ptr) in shots {
        REF_DEPTH.set(if file.contains("-16color-") { bevel_core::theme::Depth::Sixteen } else { bevel_core::theme::Depth::TrueColor });
        let fonts = if era == Era::Win98 { f98 } else { f2k };
        let (title, small, address) = match (era, place) {
            (_, 0 | 3) => ("My Computer", Small::MyComputer, "My Computer"),
            (Era::Win98, _) => (" (C:)", Small::Drive(DriveKind::Fixed), "C:\\"),
            _ => ("Local Disk (C:)", Small::Drive(DriveKind::Fixed), "Local Disk (C:)"),
        };
        let nav = match place {
            0 => Nav { up: era == Era::Win2000, ..Nav::default() },
            1 => Nav { back: true, up: true, ..Nav::default() },
            3 => Nav { back: true, forward: true, ..Nav::default() },
            _ => Nav { back: true, forward: true, up: true, ..Nav::default() },
        };
        let ours = explorer_window(fonts, era, title, small, place == 1 || place == 2, at, size, Vec::new(), address, nav, |w| match s {
            S::Hot(t, arrow) => {
                w.bar.hot_tool = Some((t, arrow));
                w.bar.show_keys = era == Era::Win2000 && place == 2;
            }
            S::Pressed(t) => w.set_pressed(Some(Part::Tool(t))),
            S::HotTitle(i) => {
                // After typing into the address bar and going back: the field kept the keyboard.
                w.bar.hot_title = Some(i);
                w.field.focus();
            }
            S::HotGo => {
                // After keys were used (the address bar holds 2000's own leftover, masked).
                w.bar.hot_go = true;
                w.bar.show_keys = true;
            }
            S::Plain => {}
        });
        let rebar = Rect::new(0, 22, size.0, if era == Era::Win98 { 94 } else { 76 });
        // The pointer, and the tooltip that came under it (not built yet).
        let mut masks = vec![pointer(ptr.0, ptr.1), Rect::new(ptr.0 - 40, ptr.1 + 7, 220, 34)];
        if era == Era::Win98 {
            masks.push(Rect::new(at.0 + 158, at.1 + 25, 308, 22));
        } else {
            masks.extend([Rect::new(at.0 + 139, at.1 + 49, size.0 - 145, 22), Rect::new(at.0 + 120, at.1 + 25, 514, 22)]);
            // The address bar's picture: 2000 showed My Computer's with the first window's address.
            masks.push(Rect::new(at.0 + 60, at.1 + 75, 18, 18));
            if matches!(s, S::HotGo) {
                masks.push(Rect::new(at.0 + 60, at.1 + 75, 540, 18));
            }
        }
        out.push(Case {
            name,
            file,
            regions: vec![region((rebar.x, rebar.y), Rect::new(at.0 + rebar.x, at.1 + rebar.y, rebar.w, rebar.h))],
            ours,
            masks,
            vga: false,
        });
    }
}

/// Windows 98's and 2000's folder window menus against their Reference PCs' fuller menus, row by
/// row as `folder_menu_cases` compares Windows 95's.
fn explorer_menu_cases(out: &mut Vec<Case>, f98: &Fonts, f2k: &Fonts) {
    use bevel_core::desktop::ItemKind;
    use bevel_core::folder::menus::{self, Nav, State};
    let state = |era: Era, kinds: &[ItemKind], files: bool, nav: Nav| State { era, kinds: kinds.to_vec(), files, can_paste: false, status_bar: true, auto_arrange: false, nav, tree: None, mode: Default::default(), web: None, undo: None };
    const FILE98: &[&str] = &[
        "&Open", "&Explore", "&Find...", "&Backup", "-", "F&ormat...", "-", "Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "My Computer", "-",
        "&Work Offline", "&Close",
    ];
    const VIEW98: &[&str] = &[
        "&Toolbars>", "&Status Bar", "&Explorer Bar>", "-", "as &Web Page", "-", "Lar&ge Icons", "S&mall Icons", "&List", "&Details", "-",
        "&Customize this Folder...", "-", "Arrange &Icons>", "Line &Up Icons", "-", "&Refresh", "Folder &Options...",
    ];
    const FILE2K: &[&str] = &["Create &Shortcut", "&Delete", "Rena&me", "P&roperties", "-", "&Close"];
    const VIEW2K: &[&str] = &[
        "&Toolbars>", "Status &Bar", "&Explorer Bar>", "-", "Lar&ge Icons", "S&mall Icons", "&List", "&Details", "-", "Arrange &Icons>", "Line &Up Icons", "-",
        "G&o To>", "&Refresh",
    ];
    let mc98 = Nav { chain: vec!["My Computer".into()], ..Nav::default() };
    type Shot<'a> = (&'static str, &'static str, Era, (i32, i32), &'a [&'a str], usize, State);
    let shots: Vec<Shot> = vec![
        ("vm-win98-folder-x-menu-file-drive", "vm/win98-truecolor-folder-mc-menu-file-drive.png", Era::Win98, (15, 45), FILE98, 0, state(Era::Win98, &[ItemKind::Drive], false, mc98)),
        ("vm-win98-folder-x-menu-view", "vm/win98-truecolor-folder-c-menu-view.png", Era::Win98, (81, 45), VIEW98, 2, state(Era::Win98, &[], true, Nav::default())),
        ("vm-win2000-folder-x-menu-file", "vm/win2000-truecolor-folder-mc-menu-file-none.png", Era::Win2000, (125, 155), FILE2K, 0, state(Era::Win2000, &[], false, Nav::default())),
        ("vm-win2000-folder-x-menu-view", "vm/win2000-truecolor-folder-mc-menu-view.png", Era::Win2000, (191, 155), VIEW2K, 2, state(Era::Win2000, &[], false, Nav::default())),
    ];
    for (name, file, era, at, windows_labels, title, s) in shots {
        let fonts = if era == Era::Win98 { f98 } else { f2k };
        let theme = bevel_core::theme::theme(era);
        let ours_menu = menus::menu(&menus::bar_rows(&s, title));
        let plain = |l: &str| l.replace('&', "").split('\t').next().unwrap_or("").trim_end_matches('>').to_string();
        let ours_hover = ours_menu.entries.iter().position(|e| plain(&e.label) == plain(windows_labels[0]));
        let ours_menu = Menu { hover: ours_hover, show_keys: true, ..ours_menu };
        let mut windows_menu = menu(windows_labels, Some(0), true);
        windows_menu.explorer_bold = true;
        if let Some(e) = windows_menu.entries.first_mut().filter(|e| e.label == "&Open") {
            e.bold = true;
        }
        let mut sc = Screen::new(800, 600, theme.colors.desktop);
        let (ours_rect, ours_layout) = sc.menu(theme, fonts, &ours_menu, MenuStyle::Popup, |_, _| at);
        let win_layout = paint::layout_menu(theme, fonts, &windows_menu, MenuStyle::Popup, 600);
        let width = ours_layout.width.min(win_layout.width) - 20;
        let mut regions = vec![region((at.0, at.1), Rect::new(at.0, at.1, width, 3))];
        for (i, e) in ours_menu.entries.iter().enumerate() {
            if e.kind == EntryKind::Separator {
                continue;
            }
            let Some(j) = windows_labels.iter().position(|w| plain(w) == plain(&e.label)) else { continue };
            let (o, w) = (entry(ours_rect, &ours_layout, i), win_layout.rects[j]);
            regions.push(region((o.x - 3, o.y), Rect::new(at.0 + w.x - 3, at.1 + w.y, width, w.h)));
        }
        out.push(Case { name, file, regions, ours: sc.fb, masks: vec![], vga: false });
    }
}

/// Brief 103: a folder window's Arrange Icons submenu, whole, against the survey's captures
/// (`*-truecolor-arrange-*`, `private/probes/folder-arrange/survey.md` part 1): My Computer's (by
/// Drive Letter, by Type, by Size, by Free Space) and a folder's, from the View menu and from the
/// empty area's menu, its first entry highlighted as Windows had it. The menus' corners were found
/// by `private/probes/folder-arrange/corners.ps1` (2000's by eye and a pixel probe). Left out:
/// 2000's other three (their corners were not measured), and the View and empty area's menus
/// around them, whose other rows (Toolbar, Customize this Folder, Folder Options) Bevel95 has not:
/// their Arrange and Line up rows are in `folder_menu_cases` and `explorer_menu_cases`. Also left
/// out: whole windows with a moved icon, after Line up and after 98's reopening. The survey's
/// windows are laid out with the PCs' own pictures of drives and text files, 98's and 2000's web
/// view panels (pie charts, the PC's own descriptions) and windows of sizes the PCs chose, and
/// the moved icon's place is a drag of the survey's pointer: a scene built of Bevel95's pictures
/// would differ in every picture and could only compare places, which the core's tests
/// (`folder::arrange_tests`) prove with the survey's own numbers (the drag by 171,173; the grid of
/// 75 from the first icon; the order with the moved icon last).
fn arrange_menu_cases(out: &mut Vec<Case>, f98: &Fonts, f2k: &Fonts) {
    use bevel_core::folder::menus::{self, State, Sub};
    let state = |era: Era, files: bool| State { era, kinds: Vec::new(), files, can_paste: false, status_bar: true, auto_arrange: false, nav: Default::default(), tree: None, mode: Default::default(), web: None, undo: None };
    type Shot = (&'static str, &'static str, Era, (i32, i32), bool);
    let shots: [Shot; 9] = [
        ("vm-win95-arrange-mc-view", "vm/win95-truecolor-arrange-mc-large-menu-view-arrange.png", Era::Win95, (166, 157), false),
        ("vm-win95-arrange-fa-view", "vm/win95-truecolor-arrange-fa-large-menu-view-arrange.png", Era::Win95, (170, 161), true),
        ("vm-win95-arrange-mc-rmenu", "vm/win95-truecolor-arrange-mc-large-rmenu-arrange.png", Era::Win95, (612, 426), false),
        ("vm-win95-arrange-fa-rmenu", "vm/win95-truecolor-arrange-fa-large-rmenu-arrange.png", Era::Win95, (432, 366), true),
        ("vm-win98-arrange-mc-view", "vm/win98-truecolor-arrange-mc-large-menu-view-arrange.png", Era::Win98, (222, 230), false),
        ("vm-win98-arrange-fa-view", "vm/win98-truecolor-arrange-fa-large-menu-view-arrange.png", Era::Win98, (341, 344), true),
        ("vm-win98-arrange-mc-rmenu", "vm/win98-truecolor-arrange-mc-large-rmenu-arrange.png", Era::Win98, (572, 406), false),
        ("vm-win98-arrange-fa-rmenu", "vm/win98-truecolor-arrange-fa-large-rmenu-arrange.png", Era::Win98, (453, 339), true),
        ("vm-win2000-arrange-mc-view", "vm/win2000-truecolor-arrange-mc-large-menu-view-arrange.png", Era::Win2000, (319, 314), false),
    ];
    for (name, file, era, at, files) in shots {
        let fonts = if era == Era::Win2000 { f2k } else { f98 };
        let theme = bevel_core::theme::theme(era);
        let rows = menus::sub_rows(&state(era, files), Sub::Arrange);
        // 2000's View menu was opened with the keys here: it underlines too.
        let ours = Menu { hover: Some(0), show_keys: true, ..menus::menu(&rows) };
        let mut sc = Screen::new(800, 600, theme.colors.desktop);
        let (r, _) = sc.menu(theme, fonts, &ours, MenuStyle::Popup, |_, _| at);
        out.push(Case { name, file, regions: vec![region(at, r)], ours: sc.fb, masks: vec![], vga: false });
    }
}

/// The address bar's open list in Windows 98 and 2000 (`*-truecolor-folder-address-list`): its
/// frame's left edge and the rows both have (the Desktop, My Computer and its items), with their
/// pictures, levels and names, 2000's row of the place shown highlighted. The rows after them
/// (Dial-Up Networking, My Network Places, My Briefcase...) and the list's width are not compared.
fn explorer_list_cases(out: &mut Vec<Case>, f98: &Fonts, f2k: &Fonts) {
    use bevel_core::folder::art::{self, DriveKind, Small};
    use bevel_core::folder::menus::Nav;
    use bevel_core::folder::ListRow;
    for era in [Era::Win98, Era::Win2000] {
        let fonts = if era == Era::Win98 { f98 } else { f2k };
        let (at, size) = if era == Era::Win98 { ((0, 0), (512, 461)) } else { ((110, 110), (680, 458)) };
        let row = |level: i32, small: Small, name: &str| ListRow { level, icon: art::listed(era, small), name: name.to_string() };
        let (rows, hot, title, address, file, name) = if era == Era::Win98 {
            (
                vec![
                    row(0, Small::Desktop, "Desktop"),
                    row(1, Small::MyComputer, "My Computer"),
                    row(2, Small::Drive(DriveKind::Floppy), "3\u{bd} Floppy (A:)"),
                    row(2, Small::Drive(DriveKind::Fixed), " (C:)"),
                    row(2, Small::Drive(DriveKind::CdRom), " (D:)"),
                    row(2, Small::Printers, "Printers"),
                    row(2, Small::ControlPanel, "Control Panel"),
                ],
                None,
                " (C:)",
                "C:\\",
                "vm/win98-truecolor-folder-address-list.png",
                "vm-win98-folder-x-address-list",
            )
        } else {
            (
                vec![
                    row(0, Small::Desktop, "Desktop"),
                    row(1, Small::MyDocuments, "My Documents"),
                    row(1, Small::MyComputer, "My Computer"),
                    row(2, Small::Drive(DriveKind::Floppy), "3\u{bd} Floppy (A:)"),
                    row(2, Small::Drive(DriveKind::Fixed), "Local Disk (C:)"),
                    row(2, Small::Drive(DriveKind::CdRom), "Compact Disc (D:)"),
                    row(2, Small::ControlPanel, "Control Panel"),
                ],
                Some(4),
                "Local Disk (C:)",
                "Local Disk (C:)",
                "vm/win2000-truecolor-folder-address-list.png",
                "vm-win2000-folder-x-address-list",
            )
        };
        let n = rows.len() as i32;
        let nav = Nav { back: true, up: true, ..Nav::default() };
        let ours = explorer_window(fonts, era, title, Small::Drive(DriveKind::Fixed), true, at, size, Vec::new(), address, nav, |w| {
            w.field.focus();
            w.set_list(Some((rows, hot)));
        });
        // The combo box's bottom edge and the list's left edge and rows (ours: the first rows of a
        // list of the same frame).
        let (lx, ly) = if era == Era::Win98 { (57, 113) } else { (168 - 110, 205 - 110) };
        let rect = Rect::new(lx, ly, 200, 1 + n * 16);
        out.push(Case {
            name,
            file,
            regions: vec![region((rect.x, rect.y), Rect::new(at.0 + rect.x, at.1 + rect.y, rect.w, rect.h))],
            ours,
            // 2000 tinted the highlighted row's picture in a way not copied yet.
            masks: hot.map(|h| vec![Rect::new(at.0 + lx + 4 + 20, at.1 + ly + 1 + 16 * h as i32, 16, 16)]).unwrap_or_default(),
            vga: false,
        });
    }
}

/// The Reference PCs' trees as Explorer showed them, built as the folders program builds them:
/// row by row as each folder's rows come (`pane::Tree::set_rows`), then opened down to the place
/// shown. Only what Bevel95's tree holds (no network places, Dial-Up Networking, Scheduled Tasks,
/// Internet Explorer); the cases mask the rows the PCs had beyond them.
fn reference_tree(era: Era) -> bevel_core::folder::pane::Tree {
    use bevel_core::folder::art::DriveKind;
    use bevel_core::folder::pane::{NodeKind as K, NodeSpec, Tree};
    let spec = |id: &str, name: &str, kind: K, has_rows: bool| NodeSpec { id: id.into(), name: name.into(), kind, has_rows, hidden: false };
    let mut t = Tree::new();
    let _ = t.wanted();
    let computer = spec("::my-computer", "My Computer", K::MyComputer, true);
    let documents = spec("::my-documents", "My Documents", K::MyDocuments, true);
    let recycle = spec("::recycle-bin", "Recycle Bin", K::RecycleBin, false);
    let desktop = match era {
        Era::Win95 => vec![computer, recycle, spec("D:\\d\\Online Services", "Online Services", K::Folder, false)],
        Era::Win98 => vec![
            computer,
            documents,
            recycle,
            spec("D:\\d\\My Briefcase", "My Briefcase", K::Folder, false),
            spec("D:\\d\\Online Services", "Online Services", K::Folder, false),
        ],
        Era::Win2000 => vec![documents, computer, recycle],
    };
    t.set_rows("::desktop", desktop);
    let (c, d) = match era {
        Era::Win95 => ("Win95 (C:)", None),
        Era::Win98 => (" (C:)", Some(" (D:)")),
        Era::Win2000 => ("Local Disk (C:)", Some("Compact Disc (D:)")),
    };
    let mut drives = vec![spec("A:\\", "3\u{bd} Floppy (A:)", K::Drive(DriveKind::Floppy), true), spec("C:\\", c, K::Drive(DriveKind::Fixed), true)];
    if let Some(d) = d {
        drives.push(spec("D:\\", d, K::Drive(DriveKind::CdRom), true));
    }
    let cp = spec("::control-panel", "Control Panel", K::ControlPanel, false);
    let printers = spec("::printers", "Printers", K::Printers, false);
    match era {
        Era::Win95 => drives.extend([cp, printers]),
        Era::Win98 => drives.extend([printers, cp]),
        Era::Win2000 => drives.push(cp),
    }
    t.set_rows("::my-computer", drives);
    let folders: Vec<NodeSpec> = match era {
        Era::Win95 => vec![
            spec("C:\\Program Files", "Program Files", K::Folder, true),
            spec("C:\\Win95", "Win95", K::Folder, false),
            spec("C:\\Windows", "Windows", K::Folder, true),
        ],
        Era::Win98 => vec![
            spec("C:\\My Documents", "My Documents", K::Folder, true),
            spec("C:\\Program Files", "Program Files", K::Folder, true),
            spec("C:\\Windows", "Windows", K::Folder, true),
        ],
        Era::Win2000 => vec![
            spec("C:\\Documents and Settings", "Documents and Settings", K::Folder, true),
            spec("C:\\Program Files", "Program Files", K::Folder, true),
            spec("C:\\WINNT", "WINNT", K::Folder, true),
        ],
    };
    t.set_rows("C:\\", folders);
    t.set_rows("::my-documents", vec![spec("C:\\docs\\My Pictures", "My Pictures", K::Folder, false)]);
    if era == Era::Win2000 {
        t.follow(vec!["::my-documents".into()], 20);
    } else {
        t.follow(vec!["::my-computer".into(), "C:\\".into()], 20);
    }
    t
}

/// A window with Explorer's tree as a Reference PC showed one: painted by `FolderWindow` at its
/// place on the screen.
#[allow(clippy::too_many_arguments)]
fn tree_window(
    fonts: &Fonts,
    era: Era,
    title: &str,
    place: &str,
    small: bevel_core::folder::art::Small,
    files: bool,
    at: (i32, i32),
    size: (i32, i32),
    items: Vec<bevel_core::desktop_icons::Item>,
    address: &str,
    nav: bevel_core::folder::menus::Nav,
    state: impl FnOnce(&mut bevel_core::folder::FolderWindow),
) -> Framebuffer {
    use bevel_core::folder::{FolderWindow, art, tree_art};
    use bevel_core::kit::Form;
    let icon = tree_art::explorer(era);
    let mut w = FolderWindow::new(era, title, icon.clone(), files, size);
    w.set_screen(at.0, at.1);
    w.layout(fonts);
    if era != Era::Win95 {
        w.show_place(place, icon.clone(), files, address, art::listed(era, small));
        w.set_nav(nav);
    }
    w.title = title.to_string();
    w.place_name = place.to_string();
    w.icon = icon;
    w.exploring = true;
    w.tree = reference_tree(era);
    w.show_tree(true);
    w.set_items(items);
    state(&mut w);
    w.layout(fonts);
    let mut fb = Framebuffer::new(w.size_now().0, w.size_now().1, 0);
    w.paint(&mut fb, bevel_core::theme::theme(era), fonts);
    fb
}

/// Whole windows with Explorer's tree against the Reference PCs (`vm/<era>-truecolor-tree-*`):
/// the frame and title ("Exploring - ..." in 95 and 98, the Explorer's picture), the pane's
/// caption(s) and frame, the tree's rows with their lines, boxes and pictures, the selection, the
/// bar between the panes, the icons and the status bar. Masked, each with its reason: the PCs'
/// pictures of their own files and of folders with pictures of their own, folders the virtual PC
/// drew gray, the menu titles and toolbar buttons Bevel95 leaves out, and the tree rows the PCs
/// had beyond Bevel95's (Dial-Up Networking, Scheduled Tasks, network places, Internet Explorer).
fn explorer_tree_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::folder::art::{DriveKind, Small};
    use bevel_core::folder::menus::Nav;
    let whole = |fb: &Framebuffer, at: (i32, i32)| vec![region((0, 0), Rect::new(at.0, at.1, fb.width, fb.height))];
    let pointer = |x: i32, y: i32| Rect::new(x, y, 13, 21);

    // Windows 95, from Start > Programs > Windows Explorer, after View > Large Icons: drive C:
    // shown, the icons with the keyboard (the focus frame on Program Files).
    let at = (44, 44);
    let c95 = || folder_items(&[("Program Files", 'f', 0), ("Win95", 'f', 0), ("Windows", 'f', 0), ("Command", 'p', 93_880), ("Netlog", 'p', 489)]);
    let state95 = |w: &mut bevel_core::folder::FolderWindow| {
        w.hidden = 13;
        w.total_size = 939 * 1024 + 300;
        w.disk_free = Some(284 << 20);
    };
    let help = bevel_core::window::menu_bar_layout(f95, &["&File", "&Edit", "&View", "&Tools", "&Help"]);
    let tools_help = Rect::new(at.0 + 4 + help[3].x - 3, at.1 + 4 + help[3].y - 3, help[4].right() - help[3].x + 2, help[3].h);
    for (name, file, tree_focus) in [("vm-win95-tree-large", "vm/win95-truecolor-tree-large.png", false), ("vm-win95-tree-tab", "vm/win95-truecolor-tree-tab.png", true)] {
        let ours = tree_window(f95, Era::Win95, "Exploring - Win95 (C:)", "Win95 (C:)", Small::Drive(DriveKind::Fixed), true, at, (600, 411), c95(), "", Nav::default(), |w| {
            state95(w);
            w.view.focus = Some(0);
            w.tree_focus = tree_focus;
        });
        let mut masks = vec![
            tools_help,
            // The icons' list: the files' own pictures, Win95 drawn gray by the virtual PC.
            Rect::new(at.0 + 199 + 2 + 75 + 21, at.1 + 66, 32, 32),
            Rect::new(at.0 + 199 + 2 + 225 + 21, at.1 + 66, 64 + 43, 32),
            // The tree: Win95 and Online Services drawn gray by the virtual PC.
            Rect::new(at.0 + 6 + 57, at.1 + 64 + 6 * 16, 16, 16),
            Rect::new(at.0 + 6 + 19, at.1 + 64 + 10 * 16, 16, 16),
            pointer(790, 352),
        ];
        if tree_focus {
            masks.clear();
            masks.extend([tools_help, Rect::new(at.0 + 199 + 2 + 75 + 21, at.1 + 66, 32, 32), Rect::new(at.0 + 199 + 2 + 225 + 21, at.1 + 66, 64 + 43, 32)]);
            masks.extend([Rect::new(at.0 + 6 + 57, at.1 + 64 + 6 * 16, 16, 16), Rect::new(at.0 + 6 + 19, at.1 + 64 + 10 * 16, 16, 16), pointer(790, 352)]);
        }
        out.push(Case { name, file, regions: whole(&ours, at), ours, masks, vga: false });
    }

    // Windows 98, the same entry, the web view off and Large Icons: (C:) selected in the tree,
    // which has the keyboard; no status bar (98's Explorer started without one).
    let at = (0, 0);
    let c98 = folder_items(&[
        ("My Documents", 'f', 0),
        ("Program Files", 'f', 0),
        ("Windows", 'f', 0),
        ("Autoexec", 'p', 0),
        ("Command", 'p', 0),
        ("Frunlog", 'p', 0),
        ("Netlog", 'p', 0),
        ("Scandisk.log", 'p', 0),
    ]);
    let nav = Nav { up: true, ..Nav::default() };
    let ours = tree_window(f95, Era::Win98, "Exploring -  (C:)", " (C:)", Small::Drive(DriveKind::Fixed), true, at, (600, 411), c98, "C:\\", nav, |w| {
        w.status_bar = false;
        w.tree_focus = true;
        let (size, max) = (w.size_now(), w.maximized);
        w.resize(size, max);
    });
    let masks = vec![
        Rect::new(158, 25, 400, 22),  // Favorites, Tools, Help
        Rect::new(373, 49, 222, 40),  // Undo and what follows it
        Rect::new(204 + 2 + 21, 122, 32, 32), // My Documents' own picture
        Rect::new(204 + 2 + 225 + 21, 122, 107, 32),
        Rect::new(204 + 2 + 21, 197, 182, 32),
        Rect::new(4 + 2 + 38 + 19, 118 + 24 + 4 * 16, 16, 16), // C:\My Documents' own picture
        Rect::new(4 + 2, 118 + 24 + 9 * 16, 192, 289 - 24 - 9 * 16), // Dial-Up Networking and below
        pointer(790, 300),
    ];
    out.push(Case { name: "vm-win98-tree-large", file: "vm/win98-truecolor-tree-large.png", regions: whole(&ours, at), ours, masks, vga: false });

    // Windows 2000, Start > Programs > Accessories > Windows Explorer, classic folders: My
    // Documents, the tree with the keyboard.
    let at = (110, 110);
    let docs = folder_items(&[("My Pictures", 'p', 0)]);
    let ours = tree_window(f2k, Era::Win2000, "My Documents", "My Documents", Small::MyDocuments, true, at, (680, 458), docs, "My Documents", Nav { up: true, ..Nav::default() }, |w| {
        w.tree_focus = true;
        w.disk_free = Some(2_205_000_000);
    });
    let masks = vec![
        Rect::new(110 + 139, 159, 680 - 139 - 6, 22), // Search and what follows it
        Rect::new(230, 135, 514, 22),                  // Favorites, Tools, Help
        Rect::new(110 + 4 + 2 + 19 + 19, 110 + 100 + 24 + 2 * 16, 16, 16), // My Pictures' own picture
        Rect::new(110 + 4 + 2, 110 + 100 + 24 + 4 * 16, 196, 120), // My Network Places and below
        Rect::new(110 + 231, 110 + 106, 32, 32), // My Pictures' own picture
        pointer(790, 300),
    ];
    out.push(Case { name: "vm-win2000-tree-start", file: "vm/win2000-truecolor-tree-start.png", regions: whole(&ours, at), ours, masks, vga: false });
}
/// Windows 2000's Folders button in each state, against the Reference PC (its place differs:
/// Bevel95 leaves out Search, so ours stands where Search stood; each case pairs the two).
fn folders_button_cases(out: &mut Vec<Case>, f2k: &Fonts) {
    use bevel_core::folder::art::Small;
    use bevel_core::folder::menus::Nav;
    use bevel_core::folder::{Part, Tool};
    use bevel_core::kit::Form;
    let at = (110, 110);
    for (name, file, shown, hot) in [
        ("vm-win2000-tree-folders-on", "vm/win2000-truecolor-tree-start.png", true, false),
        ("vm-win2000-tree-folders-hot-on", "vm/win2000-truecolor-tree-folders-hot-on.png", true, true),
        ("vm-win2000-tree-folders-hot-off", "vm/win2000-truecolor-tree-folders-hot-off.png", false, true),
        ("vm-win2000-tree-folders-off", "vm/win2000-truecolor-tree-folders-off-cold.png", false, false),
    ] {
        let mut button = None;
        let ours = tree_window(f2k, Era::Win2000, "My Documents", "My Documents", Small::MyDocuments, true, at, (680, 458), Vec::new(), "My Documents", Nav { up: true, ..Nav::default() }, |w| {
            w.show_tree(shown);
            if hot {
                w.bar.hot_tool = Some((Tool::Folders, false));
            }
            button = w.rect(Part::Tool(Tool::Folders));
        });
        let b = button.expect("the Folders button");
        out.push(Case {
            name,
            file,
            regions: vec![region((b.x, b.y), Rect::new(at.0 + 209, at.1 + b.y, b.w, b.h))],
            ours,
            // The pointer and its shadow, where it rested on the button.
            masks: if hot { vec![Rect::new(349, 169, 19, 23)] } else { vec![] },
            vga: false,
        });
    }
}
/// The tree bar's close button under the pointer and pressed (98 and 2000), against the Reference
/// PCs: the button alone (the windows around it showed other places); the pointer masked.
fn tree_close_cases(out: &mut Vec<Case>, f98: &Fonts, f2k: &Fonts) {
    use bevel_core::folder::art::Small;
    use bevel_core::folder::menus::Nav;
    use bevel_core::folder::Part;
    use bevel_core::kit::Form;
    for (era, fonts, at, size, pointer) in [
        (Era::Win98, f98, (0, 0), (600, 411), Rect::new(184, 130, 14, 22)),
        (Era::Win2000, f2k, (110, 110), (680, 458), Rect::new(299, 221, 19, 23)),
    ] {
        for (state, pressed) in [("hot", false), ("pressed", true)] {
            let mut button = Rect::new(0, 0, 0, 0);
            let ours = tree_window(fonts, era, "t", "t", Small::Folder, true, at, size, Vec::new(), "t", Nav::default(), |w| {
                let b = w.pane_layout().and_then(|l| l.close).expect("a close button");
                w.hover(b.x + 5, b.y + 5);
                if pressed {
                    w.set_pressed(Some(Part::TreeClose));
                }
                button = b;
            });
            let name: &'static str = Box::leak(format!("vm-{}-tree-close-{state}", if era == Era::Win98 { "win98" } else { "win2000" }).into_boxed_str());
            let file: &'static str = Box::leak(format!("vm/{}-truecolor-tree-close-{state}.png", if era == Era::Win98 { "win98" } else { "win2000" }).into_boxed_str());
            out.push(Case {
                name,
                file,
                regions: vec![region((button.x, button.y), Rect::new(at.0 + button.x, at.1 + button.y, button.w, button.h))],
                ours,
                masks: vec![pointer],
                vga: false,
            });
        }
    }
}
/// What a folder window says when a drive cannot be read (`core::disk_box`): Windows 95's "not
/// accessible" box and 2000's "Insert disk" box, whole, against the Reference PCs (2000's picture
/// masked: the shell's own image of the drive).
fn disk_box_cases(out: &mut Vec<Case>, f95: &Fonts, f2k: &Fonts) {
    use bevel_core::disk_box::DiskBox;
    use bevel_core::folder::art::{self, DriveKind};
    use bevel_core::kit::Form;
    for (era, fonts, at, name, file) in [
        (Era::Win95, f95, (308, 245), "vm-win95-tree-floppy-error", "vm/win95-truecolor-tree-floppy-error.png"),
        (Era::Win2000, f2k, (226, 217), "vm-win2000-tree-floppy-insert", "vm/win2000-truecolor-tree-floppy-error.png"),
    ] {
        let w = DiskBox::new(era, "Exploring - 3\u{bd} Floppy (A:)", 'A', art::drive(era, DriveKind::Floppy), true, |l| fonts.regular.measure(l));
        let (ww, wh) = w.size();
        let mut fb = Framebuffer::new(ww, wh, 0);
        w.paint(&mut fb, bevel_core::theme::theme(era), fonts);
        let masks = if era == Era::Win2000 { vec![Rect::new(at.0 + 10, at.1 + 22, 40, 40)] } else { vec![] };
        out.push(Case { name, file, regions: vec![region((0, 0), Rect::new(at.0, at.1, ww, wh))], ours: fb, masks, vga: false });
    }
}
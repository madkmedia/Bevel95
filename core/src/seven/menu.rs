//! Windows 7 Basic's Start menu: its layout and its painting at 1x, from the pictures in
//! bevel-art (`seven/menu-...`, cut from ONE published screenshot: Windows 7 SP1, 1440x900, the
//! Start menu open; survey.md section 2 and its round 2 corrections, section 5).
//!
//! The menu is 411 wide and stands on the taskbar at the screen's left edge. Left to right: the
//! frame (border, light line, 5 frame columns, light and dark line), the white left pane (250:
//! program rows 38 apart, a line, All Programs, the search band), two columns between the panes,
//! the right column (148: the user's picture tile above, entries 35 apart and 3 more per
//! separator, the Shut down button with its arrow part), the right edge (2).
//!
//! PROVISIONAL: the menu's height (measured: 485 rows; for another height the measured gradients
//! are kept at the top and the bottom and the row between them repeated or left out, because one
//! screenshot cannot show whether Windows stretches them), the item under the pointer and the one
//! chosen by keys (a place in the data, nothing painted: no picture of it exists), All Programs
//! opened (in the data, not painted), the letters (plain Tahoma 8 pt standing in for Segoe UI 9 pt
//! with ClearType; the search hint upright, as the core has no italic), their colors, the row and
//! entry rectangles used for pointing (only the pictures' and letters' places were measured), the
//! user's picture (the screenshot's own flower picture in its frame).

use crate::art::Look;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::text::Font;

/// The menu's width and its measured height.
pub const W: i32 = 411;
pub const MEASURED_H: i32 = 485;
/// The left pane: x 9..=258.
pub const PANE_X: i32 = 9;
pub const PANE_W: i32 = 250;
/// The two columns between the panes: x 259, 260.
pub const EDGE_X: i32 = 259;
/// The right column: x 261..=408.
pub const RIGHT_X: i32 = 261;
pub const RIGHT_W: i32 = 148;
/// The right edge: x 409, 410.
pub const RIGHT_EDGE_X: i32 = 409;
/// The top rows painted from one picture (corners, the panes' tops).
pub const TOP_H: i32 = 13;
/// Program rows: the first picture's top, the step, the picture's x, the name's x and the rows
/// from a picture's top to its name's capital letters' top.
pub const FIRST_ROW: i32 = 13;
pub const ROW_STEP: i32 = 38;
pub const ROW_ICON_X: i32 = 13;
pub const ROW_NAME_X: i32 = 51;
pub const ROW_NAME_DOWN: i32 = 11;
/// A program row's submenu arrow: x, and rows down from the picture's top.
pub const SUBMENU_ARROW_X: i32 = 242;
pub const SUBMENU_ARROW_DOWN: i32 = 12;
/// Counted from the menu's bottom (its height h): the line above All Programs (h - 85, x 20..=251),
/// All Programs' arrow (x 19, h - 76) and name (x 49, capitals' top h - 74), the search band
/// (h - 54, 45 rows), its box (x 21..=242, h - 42..=h - 21), its hint (x 30, h - 36), the pane's
/// bottom rows (h - 9, 8 rows), the Shut down button (x 267, h - 42, 94x22; its arrow part from
/// x 338) and its name (x 274, h - 35).
pub const LINE_UP: i32 = 85;
pub const ALL_ARROW_X: i32 = 19;
pub const ALL_ARROW_UP: i32 = 76;
pub const ALL_NAME_X: i32 = 49;
pub const ALL_NAME_UP: i32 = 74;
pub const BAND_UP: i32 = 54;
pub const BOX_X: i32 = 21;
pub const BOX_UP: i32 = 42;
pub const BOX_W: i32 = 222;
pub const BOX_H: i32 = 22;
pub const HINT_X: i32 = 30;
pub const HINT_UP: i32 = 36;
pub const BOTTOM_UP: i32 = 9;
pub const SHUT_X: i32 = 267;
pub const SHUT_UP: i32 = 42;
pub const SHUT_W: i32 = 94;
pub const SHUT_H: i32 = 22;
pub const SHUT_ARROW_X: i32 = 338;
pub const SHUT_NAME_X: i32 = 274;
pub const SHUT_NAME_UP: i32 = 35;
/// The user's picture tile.
pub const TILE: Rect = Rect::new(300, 0, 72, 70);
/// Right column entries: the first one's capitals' top (the user's name), the step, 3 more per
/// separator, a separator 12 rows above where the next entry would have been, the name's x.
pub const FIRST_ENTRY: i32 = 87;
pub const ENTRY_STEP: i32 = 35;
pub const SEPARATOR_EXTRA: i32 = 3;
pub const SEPARATOR_UP: i32 = 12;
pub const ENTRY_X: i32 = 276;
/// The separator's place across the column.
pub const SEPARATOR_X: i32 = 276;
/// Colors: the line above All Programs, the bottom border, the letters (PROVISIONAL: Windows
/// smooths them; these are the plain colors they are drawn in).
pub const LINE: u32 = 0xFFD6_E5F5;
pub const BORDER: u32 = 0xFF3A_434D;
pub const WHITE: u32 = 0xFFFF_FFFF;
pub const PANE_TEXT: u32 = 0xFF00_0000;
pub const RIGHT_TEXT: u32 = 0xFFFF_FFFF;
pub const HINT_TEXT: u32 = 0xFF6D_6D6D;
pub const SHUT_TEXT: u32 = 0xFFFF_FFFF;
/// The measured gradients' rows kept at the top (0..240) and at the bottom (the last 245), and the
/// row repeated between them for a taller menu (PROVISIONAL).
const KEEP_TOP: i32 = 240;
const KEEP_BOTTOM: i32 = MEASURED_H - KEEP_TOP;

/// A program row of the left pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramRow {
    pub name: String,
    /// Its 32x32 picture.
    pub icon: Option<Icon>,
    /// It has a submenu (its recent files: the arrow at the right).
    pub submenu: bool,
}

/// An entry of the right column, below the user's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RightEntry {
    Item(String),
    Separator,
}

/// The part under the pointer or chosen by keys (PROVISIONAL: kept, not painted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hot {
    Program(usize),
    AllPrograms,
    Search,
    /// The user's name is entry 0; the right column's entries follow, separators counted.
    Right(usize),
    ShutDown,
    ShutDownArrow,
}

/// What the menu holds: everything the platform fills.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SevenMenu {
    pub programs: Vec<ProgramRow>,
    pub all_programs: String,
    /// All Programs opened in the left pane (not painted in this version).
    pub all_programs_open: bool,
    pub search_hint: String,
    pub user: String,
    pub right: Vec<RightEntry>,
    pub shut_down: String,
    pub hot: Option<Hot>,
    /// The menu's height in rows (the screenshot's: [`MEASURED_H`]).
    pub height: i32,
}

impl Default for SevenMenu {
    fn default() -> SevenMenu {
        SevenMenu {
            programs: Vec::new(),
            all_programs: "All Programs".into(),
            all_programs_open: false,
            search_hint: "Search programs and files".into(),
            user: String::new(),
            right: Vec::new(),
            shut_down: "Shut down".into(),
            hot: None,
            height: MEASURED_H,
        }
    }
}

/// Where the menu and its parts are. Parts are in the menu's own coordinates; `place` is the
/// menu on the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuLayout {
    pub place: Rect,
    /// Each program row's picture (32x32).
    pub program_icons: Vec<Rect>,
    /// Each program row for pointing (PROVISIONAL: 38 rows from 3 above its picture).
    pub program_rows: Vec<Rect>,
    pub line_row: i32,
    pub all_programs: Rect,
    pub search_box: Rect,
    /// The user's name and each entry: the top of its capitals.
    pub entry_tops: Vec<i32>,
    /// Each entry for pointing (PROVISIONAL), the user's name first.
    pub entry_rows: Vec<Rect>,
    /// Each separator's first row.
    pub separators: Vec<i32>,
    pub tile: Rect,
    pub shut_down: Rect,
    pub shut_down_arrow: Rect,
}

/// The layout of `menu` standing on a taskbar whose top row is `bar_top`.
pub fn layout(menu: &SevenMenu, bar_top: i32) -> MenuLayout {
    let h = menu.height;
    let program_icons: Vec<Rect> =
        (0..menu.programs.len() as i32).map(|k| Rect::new(ROW_ICON_X, FIRST_ROW + ROW_STEP * k, 32, 32)).collect();
    let program_rows = program_icons.iter().map(|r| Rect::new(PANE_X, r.y - 3, PANE_W, ROW_STEP)).collect();
    let mut entry_tops = vec![FIRST_ENTRY];
    let mut separators = Vec::new();
    let mut next = FIRST_ENTRY + ENTRY_STEP;
    for e in &menu.right {
        match e {
            RightEntry::Item(_) => {
                entry_tops.push(next);
                next += ENTRY_STEP;
            }
            RightEntry::Separator => {
                separators.push(next - SEPARATOR_UP);
                next += SEPARATOR_EXTRA;
            }
        }
    }
    let entry_rows = entry_tops.iter().map(|&t| Rect::new(RIGHT_X + 5, t - 12, RIGHT_W - 10, ENTRY_STEP)).collect();
    MenuLayout {
        place: Rect::new(0, bar_top - h, W, h),
        program_icons,
        program_rows,
        line_row: h - LINE_UP,
        all_programs: Rect::new(PANE_X, h - ALL_ARROW_UP - 5, PANE_W, 22),
        search_box: Rect::new(BOX_X, h - BOX_UP, BOX_W, BOX_H),
        entry_tops,
        entry_rows,
        separators,
        tile: TILE,
        shut_down: Rect::new(SHUT_X, h - SHUT_UP, SHUT_W, SHUT_H),
        shut_down_arrow: Rect::new(SHUT_ARROW_X, h - SHUT_UP, SHUT_X + SHUT_W - SHUT_ARROW_X, SHUT_H),
    }
}

/// The measured row that row `y` of a menu `h` rows high is painted from (PROVISIONAL for
/// h != 485: the top 240 and the bottom 245 rows kept, row 240 repeated or rows left out between).
pub fn source_row(y: i32, h: i32) -> i32 {
    if y >= h - KEEP_BOTTOM {
        y + MEASURED_H - h
    } else if y < KEEP_TOP {
        y
    } else {
        KEEP_TOP
    }
}

// ---- the pictures ----

const TOP_PIC: &[u8; 411 * 13 * 4] = crate::art::exactly("seven/menu-top", Look::Win7);
const FRAME_PIC: &[u8; 9 * 485 * 4] = crate::art::exactly("seven/menu-frame-left", Look::Win7);
const EDGE_PIC: &[u8; 2 * 485 * 4] = crate::art::exactly("seven/menu-pane-edge", Look::Win7);
const COLUMN_PIC: &[u8; 485 * 4] = crate::art::exactly("seven/menu-right-column", Look::Win7);
const RIGHT_EDGE_PIC: &[u8; 2 * 485 * 4] = crate::art::exactly("seven/menu-right-edge", Look::Win7);
const BOTTOM_PIC: &[u8; 250 * 8 * 4] = crate::art::exactly("seven/menu-pane-bottom", Look::Win7);
const BAND_PIC: &[u8; 250 * 45 * 4] = crate::art::exactly("seven/menu-search-band", Look::Win7);
const SHUT_PIC: &[u8; 94 * 22 * 4] = crate::art::exactly("seven/menu-shutdown", Look::Win7);
const SEPARATOR_PIC: &[u8; 124 * 2 * 4] = crate::art::exactly("seven/menu-separator", Look::Win7);
const TILE_PIC: &[u8; 72 * 70 * 4] = crate::art::exactly("seven/menu-user-tile", Look::Win7);
const SUBMENU_PIC: &[u8; 4 * 7 * 4] = crate::art::exactly("seven/menu-submenu-arrow", Look::Win7);
const ALL_ARROW_PIC: &[u8; 7 * 11 * 4] = crate::art::exactly("seven/menu-all-programs-arrow", Look::Win7);

fn px(rgba: &[u8], w: i32, x: i32, y: i32) -> u32 {
    let i = ((y * w + x) * 4) as usize;
    u32::from_be_bytes([rgba[i + 3], rgba[i], rgba[i + 1], rgba[i + 2]])
}

fn picture(rgba: &[u8], w: i32, h: i32) -> Icon {
    Icon {
        width: w,
        height: h,
        pixels: (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| px(rgba, w, x, y)).collect(),
    }
}

/// The menu behind its letters and pictures at (x, y) of a menu `h` rows high.
pub fn background(x: i32, y: i32, h: i32) -> u32 {
    if y == h - 1 {
        return BORDER;
    }
    if y < TOP_H {
        return px(TOP_PIC, W, x, y);
    }
    let sy = source_row(y, h);
    match x {
        0..PANE_X => px(FRAME_PIC, 9, x, sy),
        PANE_X..EDGE_X => {
            let up = h - y;
            if up <= BOTTOM_UP {
                px(BOTTOM_PIC, PANE_W, x - PANE_X, BOTTOM_UP - up)
            } else if up <= BAND_UP {
                px(BAND_PIC, PANE_W, x - PANE_X, BAND_UP - up)
            } else {
                WHITE
            }
        }
        EDGE_X..RIGHT_X => px(EDGE_PIC, 2, x - EDGE_X, sy),
        RIGHT_X..RIGHT_EDGE_X => px(COLUMN_PIC, 1, 0, sy),
        _ => px(RIGHT_EDGE_PIC, 2, x - RIGHT_EDGE_X, sy),
    }
}

/// The row in a font's cell where capital letters start.
fn capital_top(font: &Font) -> i32 {
    let g = font.glyph('H');
    (0..font.height).find(|&row| (0..g.ink_w).any(|c| g.ink[(row * g.ink_w + c) as usize])).unwrap_or(0)
}

/// Paints the menu into `fb` (at least 411 x `menu.height`) with its top left at (0, 0).
pub fn paint(fb: &mut Framebuffer, menu: &SevenMenu, l: &MenuLayout, font: &Font) {
    let h = menu.height;
    for y in 0..h {
        for x in 0..W {
            fb.set(x, y, background(x, y, h));
        }
    }
    fb.hline(20, l.line_row, 232, LINE);
    let separator = picture(SEPARATOR_PIC, 124, 2);
    for &row in &l.separators {
        fb.draw_icon(&separator, SEPARATOR_X, row);
    }
    fb.draw_icon(&picture(TILE_PIC, 72, 70), l.tile.x, l.tile.y);
    let cap = capital_top(font);
    let text = |fb: &mut Framebuffer, x: i32, top: i32, s: &str, color: u32| {
        font.draw(fb, x, top - cap, s, color);
    };
    let submenu = picture(SUBMENU_PIC, 4, 7);
    for (row, r) in menu.programs.iter().zip(&l.program_icons) {
        if let Some(icon) = &row.icon {
            fb.draw_icon(icon, r.x, r.y);
        }
        text(fb, ROW_NAME_X, r.y + ROW_NAME_DOWN, &row.name, PANE_TEXT);
        if row.submenu {
            fb.draw_icon(&submenu, SUBMENU_ARROW_X, r.y + SUBMENU_ARROW_DOWN);
        }
    }
    fb.draw_icon(&picture(ALL_ARROW_PIC, 7, 11), ALL_ARROW_X, h - ALL_ARROW_UP);
    text(fb, ALL_NAME_X, h - ALL_NAME_UP, &menu.all_programs, PANE_TEXT);
    text(fb, HINT_X, h - HINT_UP, &menu.search_hint, HINT_TEXT);
    let names = std::iter::once(menu.user.as_str()).chain(menu.right.iter().filter_map(|e| match e {
        RightEntry::Item(s) => Some(s.as_str()),
        RightEntry::Separator => None,
    }));
    for (name, &top) in names.zip(&l.entry_tops) {
        text(fb, ENTRY_X, top, name, RIGHT_TEXT);
    }
    fb.draw_icon(&picture(SHUT_PIC, SHUT_W, SHUT_H), l.shut_down.x, l.shut_down.y);
    text(fb, SHUT_NAME_X, h - SHUT_NAME_UP, &menu.shut_down, SHUT_TEXT);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screenshot_menu() -> SevenMenu {
        let item = |s: &str| RightEntry::Item(s.into());
        SevenMenu {
            programs: (0..9).map(|k| ProgramRow { name: format!("P{k}"), icon: None, submenu: k == 0 }).collect(),
            user: "User".into(),
            right: vec![
                item("Documents"),
                item("Pictures"),
                item("Music"),
                RightEntry::Separator,
                item("Games"),
                item("Computer"),
                RightEntry::Separator,
                item("Control Panel"),
                item("Devices and Printers"),
                item("Default Programs"),
                item("Help and Support"),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn program_rows_are_38_apart() {
        let l = layout(&screenshot_menu(), 860);
        let tops: Vec<i32> = l.program_icons.iter().map(|r| r.y).collect();
        assert_eq!(tops, [13, 51, 89, 127, 165, 203, 241, 279, 317]);
        assert!(l.program_icons.iter().all(|r| (r.x, r.w, r.h) == (13, 32, 32)));
    }

    #[test]
    fn right_entries_and_separators_follow_the_screenshot() {
        let l = layout(&screenshot_menu(), 860);
        assert_eq!(l.entry_tops, [87, 122, 157, 192, 230, 265, 303, 338, 373, 408]);
        assert_eq!(l.separators, [215, 288]);
    }

    #[test]
    fn the_bottom_parts_hang_from_the_bottom() {
        let l = layout(&screenshot_menu(), 860);
        assert_eq!(l.line_row, 400);
        assert_eq!(l.search_box, Rect::new(21, 443, 222, 22));
        assert_eq!(l.shut_down, Rect::new(267, 443, 94, 22));
        assert_eq!(l.shut_down_arrow, Rect::new(338, 443, 23, 22));
        let mut taller = screenshot_menu();
        taller.height = 520;
        let t = layout(&taller, 860);
        assert_eq!(t.shut_down.y, 443 + 35);
        assert_eq!(t.line_row, 435);
    }

    #[test]
    fn the_menu_stands_on_the_bar_at_the_left() {
        let m = screenshot_menu();
        assert_eq!(layout(&m, 860).place, Rect::new(0, 375, 411, 485)); // 1440x900
        assert_eq!(layout(&m, 1040).place, Rect::new(0, 555, 411, 485)); // 1920x1080
    }

    #[test]
    fn another_height_keeps_both_ends_of_the_gradients() {
        assert_eq!(source_row(10, 485), 10);
        assert_eq!(source_row(300, 485), 300);
        // taller: the top and the bottom kept, row 240 repeated between
        assert_eq!(source_row(239, 520), 239);
        assert_eq!(source_row(260, 520), 240);
        assert_eq!(source_row(519, 520), 484);
        assert_eq!(source_row(275, 520), 240);
        assert_eq!(source_row(276, 520), 241);
        // shorter: rows left out in the middle
        assert_eq!(source_row(100, 400), 100);
        assert_eq!(source_row(399, 400), 484);
        assert_eq!(source_row(155, 400), 240);
        for h in [400, 485, 520] {
            assert_eq!(background(5, h - 1, h), BORDER);
            assert_eq!(background(300, h - 20, h), background(300, 465, 485), "the right column's bottom at {h}");
        }
    }
}

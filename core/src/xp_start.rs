//! Windows XP's Start menu (Luna, blue), Bevel95's XP bar style (brief 112): a first version,
//! painted from pictures cut from published screenshots (`xp::picture`; the survey's section 2:
//! `private/probes/xp/survey.md`). 380 wide and 478 high as the screenshots: the user band (64
//! rows), its dark and orange lines (3), the body (370: the white left column, the blue right one),
//! the bottom band (41 with its border row).
//!
//! It is an ordinary `Menu` of the bar's menu machinery (`MenuStyle::XpStart`): its entries in a
//! fixed order (`menu`), laid out (`layout`) and painted (`paint`) here, so that the pointer, the
//! keys and the cascades (All Programs, My Recent Documents) work as the classic Start menu's do.
//!
//! The entries, in order: the pinned programs (Quick Launch's shortcuts), a separator, the
//! programs started most often, a separator, All Programs, the right column's ten with its two
//! separators, Log Off, Turn Off Computer. A separator always stands where it belongs, also after
//! an empty list (it is then not drawn): `item_at` counts on it.
//!
//! PROVISIONAL, and listed in decisions.md: the item under the pointer is filled 316AC5 with white
//! letters (the survey's guess); the bottom band's buttons show a white frame under the pointer (no
//! screenshot shows them); the corners are square (the rounded corners' outside is the band's
//! color); the user's name is Tahoma 17px bold (the nearest stored face to the survey's ~15 px)
//! with a one-pixel shadow; the user picture is the one cut from the screenshot.

use crate::fb::{Framebuffer, Icon, Rect};
use crate::paint::{EntryKind, Menu, MenuEntry, MenuLayout};
use crate::text::{Font, Fonts};
use crate::theme::rgb;
use crate::xp::{paint_sliced, picture};

/// The right column's entries, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Right {
    MyDocuments,
    MyRecentDocuments,
    MyPictures,
    MyMusic,
    MyComputer,
    ControlPanel,
    Printers,
    Help,
    Search,
    Run,
}

/// What an entry of the XP Start menu is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Pinned(usize),
    Often(usize),
    AllPrograms,
    Right(Right),
    LogOff,
    TurnOff,
}

/// The right column: entry, label (access key marked), bold, picture, its icon's row in the body.
const RIGHT: [(Right, &str, bool, &str, i32); 10] = [
    (Right::MyDocuments, "My &Documents", true, "icon_my_documents", 8),
    (Right::MyRecentDocuments, "My &Recent Documents", true, "icon_my_recent_documents", 40),
    (Right::MyPictures, "My &Pictures", true, "icon_my_pictures", 72),
    (Right::MyMusic, "My &Music", true, "icon_my_music", 104),
    (Right::MyComputer, "My &Computer", true, "icon_my_computer", 136),
    (Right::ControlPanel, "Control P&anel", false, "icon_control_panel", 176),
    (Right::Printers, "Printers and &Faxes", false, "icon_printers", 208),
    (Right::Help, "&Help and Support", false, "icon_help", 248),
    (Right::Search, "&Search", false, "icon_search", 280),
    (Right::Run, "&Run...", false, "icon_run", 312),
];

pub const WIDTH: i32 = 380;
pub const HEIGHT: i32 = 478;
const BAND_H: i32 = 64;
const BODY_Y: i32 = 67;
const BODY_H: i32 = 370;
const BOTTOM_Y: i32 = BODY_Y + BODY_H;
/// Left column items: 40 high from 4 into the body, x 2 to 189; icons 32×32 6 in and 4 down, the
/// label 44 in (screen x 46), its capitals 15 down.
const LEFT: (i32, i32) = (2, 188);
const LEFT_TOP: i32 = 4;
const ITEM_H: i32 = 40;
const SEP_H: i32 = 8;
/// The list's room, down to the line over All Programs (body row 333: two pinned, their line and six used fit, as in the screenshots).
const LEFT_ROOM: i32 = 329;
/// All Programs: its picture 188×35 at body row 335.
const ALL_PROGRAMS_Y: i32 = 335;
/// The right column: x 191 to 372, items 32 high, icons 24×24 7 in, labels 34 in.
const RIGHT_X: i32 = 191;
const RIGHT_W: i32 = 182;
/// The fill under the pointer (the survey's guess: XP's highlight color).
pub const HOT: u32 = rgb(0x316AC5);
const LEFT_TEXT: u32 = rgb(0x373738);
const RIGHT_TEXT: u32 = rgb(0x0A246A);
/// The bottom band's two buttons (their pictures are part of the band's): Log Off and Turn Off.
const LOG_OFF: Rect = Rect::new(170, BOTTOM_Y + 7, 72, 28);
const TURN_OFF: Rect = Rect::new(245, BOTTOM_Y + 7, 130, 28);

/// How many programs stand in the left column: all pinned that fit, then up to six of the most
/// often used (brief 112), as many as fit above All Programs.
pub fn fitting(pinned: usize, often: usize) -> (usize, usize) {
    let p = pinned.min((LEFT_ROOM / ITEM_H) as usize);
    let left = LEFT_ROOM - p as i32 * ITEM_H - if p > 0 { SEP_H } else { 0 };
    let o = often.min(6).min((left.max(0) / ITEM_H) as usize);
    (p, o)
}

/// The XP Start menu for the Windows user `user`, the pinned programs (names and 32×32 pictures)
/// and the most often used ones.
pub fn menu(user: &str, pinned: &[(String, Option<Icon>)], often: &[(String, Option<Icon>)]) -> Menu {
    let (p, o) = fitting(pinned.len(), often.len());
    let mut entries: Vec<MenuEntry> = Vec::new();
    for (name, icon) in &pinned[..p] {
        let mut e = MenuEntry::item(name, icon.clone());
        e.bold = true;
        entries.push(e);
    }
    entries.push(MenuEntry::separator());
    for (name, icon) in &often[..o] {
        entries.push(MenuEntry::item(name, icon.clone()));
    }
    entries.push(MenuEntry::separator());
    let mut all = MenuEntry::submenu("All &Programs", None);
    all.bold = true;
    entries.push(all);
    for (k, &(which, label, bold, ..)) in RIGHT.iter().enumerate() {
        if k == 5 || k == 7 {
            entries.push(MenuEntry::separator());
        }
        let mut e = if which == Right::MyRecentDocuments { MenuEntry::submenu(label, None) } else { MenuEntry::item(label, None) };
        e.bold = bold;
        entries.push(e);
    }
    entries.push(MenuEntry::item("&Log Off", None));
    entries.push(MenuEntry::item("T&urn Off Computer", None));
    Menu { entries, user: user.to_string(), ..Menu::default() }
}

/// The pinned and often-used counts of a menu made by `menu` (from its first two separators).
fn counts(menu: &Menu) -> (usize, usize) {
    let mut seps = menu.entries.iter().enumerate().filter(|(_, e)| e.kind == EntryKind::Separator).map(|(i, _)| i);
    let first = seps.next().unwrap_or(0);
    let second = seps.next().unwrap_or(first + 1);
    (first, second - first - 1)
}

/// What entry `index` of a menu made by `menu` is.
pub fn item_at(menu: &Menu, index: usize) -> Option<Item> {
    let (p, o) = counts(menu);
    if index < p {
        return Some(Item::Pinned(index));
    }
    if index > p && index <= p + o {
        return Some(Item::Often(index - p - 1));
    }
    let tail = index.checked_sub(p + o + 2)?;
    // All Programs, then the right column with its separators after its 5th and 7th, then the two.
    match tail {
        0 => Some(Item::AllPrograms),
        1..=5 => Some(Item::Right(RIGHT[tail - 1].0)),
        7..=8 => Some(Item::Right(RIGHT[tail - 2].0)),
        10..=12 => Some(Item::Right(RIGHT[tail - 3].0)),
        13 => Some(Item::LogOff),
        14 => Some(Item::TurnOff),
        _ => None,
    }
}

/// Where each entry goes (`MenuLayout`, one rectangle per entry: the part filled under the pointer;
/// a separator's is its line's place, empty when its list is).
pub fn layout(menu: &Menu) -> MenuLayout {
    let (p, o) = counts(menu);
    let mut rects = Vec::with_capacity(menu.entries.len());
    let mut y = BODY_Y + LEFT_TOP;
    for _ in 0..p {
        rects.push(Rect::new(LEFT.0, y, LEFT.1, ITEM_H));
        y += ITEM_H;
    }
    rects.push(if p > 0 { Rect::new(0, y, 191, SEP_H) } else { Rect::new(0, y, 0, 0) });
    if p > 0 {
        y += SEP_H;
    }
    for _ in 0..o {
        rects.push(Rect::new(LEFT.0, y, LEFT.1, ITEM_H));
        y += ITEM_H;
    }
    // The line over All Programs (its picture's 3 rows from body row 333, the last under All
    // Programs' picture).
    rects.push(Rect::new(0, BODY_Y + ALL_PROGRAMS_Y - 2, 191, 2));
    rects.push(Rect::new(LEFT.0, BODY_Y + ALL_PROGRAMS_Y, LEFT.1, 35));
    for (k, &(.., icon_y)) in RIGHT.iter().enumerate() {
        if k == 5 || k == 7 {
            rects.push(Rect::new(RIGHT_X - 1, BODY_Y + icon_y - 8, 190, 3));
        }
        rects.push(Rect::new(RIGHT_X, BODY_Y + icon_y - 4, RIGHT_W, 32));
    }
    rects.push(LOG_OFF);
    rects.push(TURN_OFF);
    MenuLayout { width: WIDTH, height: HEIGHT, rects, shortcut_x: 0, frame: 0, columns: vec![(0, WIDTH)], lines: Vec::new(), scroll: None }
}

/// One picture of several side by side (all as tall as the first), for slicing.
fn joined(names: &[&str]) -> Option<Icon> {
    let parts: Vec<Icon> = names.iter().map(|n| picture(n)).collect::<Option<_>>()?;
    let h = parts[0].height;
    let mut whole = Icon { width: parts.iter().map(|p| p.width).sum(), height: h, pixels: Vec::new() };
    for y in 0..h {
        for p in &parts {
            whole.pixels.extend_from_slice(&p.pixels[(y * p.width) as usize..((y + 1) * p.width) as usize]);
        }
    }
    Some(whole)
}

/// Draws `text` (its access key underlined when `keys`) with its capitals' top at `cap_top`.
fn text(fb: &mut Framebuffer, font: &Font, x: i32, cap_top: i32, label: &str, color: u32, keys: bool) {
    // Tahoma 8's capitals are 8 rows tall, standing on the baseline.
    let y = cap_top + 8 - font.ascent;
    font.draw_label(fb, (x, y), label, i32::MAX, color, keys);
}

/// Paints the XP Start menu (`fb` `WIDTH`×`HEIGHT`).
pub fn paint(fb: &mut Framebuffer, fonts: &Fonts, menu: &Menu, layout: &MenuLayout) {
    // The user band: 10 left columns, the column stretched, 35 right ones; its lines; the corners.
    if let Some(band) = joined(&["menu_band_left", "menu_band_column", "menu_band_right"]) {
        // The band's top rows hold the corners' outside (see-through in the pictures): the band's
        // own column under them, so the corners are square (PROVISIONAL).
        if let Some(column) = picture("menu_band_column") {
            paint_sliced(fb, &column, Rect::new(0, 0, WIDTH, column.height), 0, 0);
        }
        paint_sliced(fb, &band, Rect::new(0, 0, WIDTH, band.height), 10, 35);
    }
    if let Some(lines) = picture("menu_lines") {
        fb.draw_icon(&lines, 0, BAND_H);
    }
    for (name, x) in [("menu_corner_left", 0), ("menu_corner_right", WIDTH - 12)] {
        if let Some(corner) = picture(name) {
            fb.draw_icon(&corner, x, 0);
        }
    }
    if let Some(frame) = picture("menu_user_frame") {
        fb.draw_icon(&frame, 4, 3);
    }
    if !menu.user.is_empty() {
        let big = crate::text::stored("Tahoma", 17, true);
        let font = big.as_ref().unwrap_or(fonts.bold);
        let y = 21;
        font.draw(fb, 68, y + 1, &menu.user, rgb(0x0F3D8C));
        font.draw(fb, 67, y, &menu.user, rgb(0xFFFFFF));
    }
    // The body: one row of the left column, its line, the right column and its shade, the frame.
    if let Some(row) = picture("menu_body_row") {
        for y in BODY_Y..BOTTOM_Y {
            fb.draw_icon(&row, 0, y);
        }
        // Beside All Programs the frame's inner column is 738BD0 (the survey's round 2).
        for y in BODY_Y + 341..BOTTOM_Y {
            fb.set(1, y, rgb(0x738BD0));
        }
    }
    if let Some(bottom) = picture("menu_bottom") {
        fb.draw_icon(&bottom, 0, BOTTOM_Y);
    }
    let keys = menu.show_keys;
    for (i, entry) in menu.entries.iter().enumerate() {
        let Some(r) = layout.rects.get(i).copied() else { continue };
        let hot = menu.hover == Some(i);
        let Some(item) = item_at(menu, i) else {
            // A separator: the left column's or the right one's line.
            if entry.kind == EntryKind::Separator && r.w > 0 {
                // The left line 4 into its gap (body row 88 after two pinned), or where its rectangle
                // starts (over All Programs); the right one where its rectangle starts.
                let (name, x, y) = if r.x < RIGHT_X - 1 {
                    ("menu_sep_left", 0, if r.h == SEP_H { r.y + 4 } else { r.y })
                } else {
                    ("menu_sep_right", RIGHT_X - 1, r.y)
                };
                if let Some(sep) = picture(name) {
                    fb.draw_icon(&sep, x, y);
                }
            }
            continue;
        };
        match item {
            Item::Pinned(_) | Item::Often(_) => {
                if hot {
                    fb.fill(r, HOT);
                }
                if let Some(icon) = &entry.icon {
                    fb.draw_icon(icon, r.x + 6, r.y + 4);
                }
                let font = if entry.bold { fonts.bold } else { fonts.regular };
                let label = font.elide(&entry.label, r.right() - 4 - (r.x + 44));
                text(fb, font, r.x + 44, r.y + 15, &label, if hot { rgb(0xFFFFFF) } else { LEFT_TEXT }, keys);
            }
            Item::AllPrograms => {
                if let Some(all) = picture("menu_all_programs") {
                    fb.draw_icon(&all, r.x, r.y);
                }
                if hot {
                    fb.fill(r.inset(0, 2, 0, 2), HOT);
                    let w = fonts.bold.measure("All Programs");
                    text(fb, fonts.bold, 128 - 6 - w, r.y + 13, &entry.label, rgb(0xFFFFFF), keys);
                    if let Some(arrow) = picture("menu_all_programs_arrow") {
                        fb.draw_icon(&arrow, 128, r.y + 9);
                    }
                } else if keys {
                    // The picture's label has no underline: the key's line under "P" (as G2's).
                    let w = fonts.bold.measure("All Programs");
                    let x = 128 - 6 - w + fonts.bold.measure("All ");
                    fb.hline(x, r.y + 22, fonts.bold.measure("P"), LEFT_TEXT);
                }
            }
            Item::Right(which) => {
                let &(_, _, bold, icon, _) = RIGHT.iter().find(|r| r.0 == which).unwrap_or(&RIGHT[0]);
                let color = if hot {
                    fb.fill(r, HOT);
                    rgb(0xFFFFFF)
                } else {
                    RIGHT_TEXT
                };
                if let Some(icon) = picture(icon) {
                    fb.draw_icon(&icon, r.x + 7, r.y + 4);
                }
                let font = if bold { fonts.bold } else { fonts.regular };
                text(fb, font, r.x + 34, r.y + 12, &entry.label, color, keys);
                if which == Right::MyRecentDocuments {
                    // Its arrow, as the classic menus' arrow: 4 rows pointing right.
                    let (ax, ay) = (r.x + 174, r.y + 12);
                    for k in 0..4 {
                        fb.vline(ax + k, ay + k, 7 - 2 * k, color);
                    }
                }
            }
            Item::LogOff | Item::TurnOff => {
                if hot {
                    fb.frame(r, rgb(0xFFFFFF), rgb(0xFFFFFF));
                }
            }
        }
    }
}

/// The programs started most often (brief 112), as their file keeps them: a line each, how many
/// times, the name shown, the program's shortcut or file ("3\tNotepad\tC:\...\Notepad.lnk").
/// Lines that do not read are left out.
fn often_lines(text: &str) -> Vec<(u32, String, String)> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let n = parts.next()?.trim().parse().ok()?;
            let name = parts.next()?.to_string();
            let target = parts.next()?.to_string();
            (!name.is_empty() && !target.is_empty()).then_some((n, name, target))
        })
        .collect()
}

/// The file's text after `target` (shown as `name`) was started once more. At most 50 programs are
/// kept (the least used go).
pub fn often_counted(text: &str, name: &str, target: &str) -> String {
    let mut lines = often_lines(text);
    match lines.iter_mut().find(|(_, _, t)| t.eq_ignore_ascii_case(target)) {
        Some(line) => {
            line.0 = line.0.saturating_add(1);
            line.1 = name.to_string();
        }
        None => lines.push((1, name.to_string(), target.to_string())),
    }
    lines.sort_by_key(|l| std::cmp::Reverse(l.0));
    lines.truncate(50);
    lines.iter().map(|(n, name, target)| format!("{n}\t{name}\t{target}\n")).collect()
}

/// The `n` programs started most often (name, target), the most first; equal counts in the file's
/// order.
pub fn often_top(text: &str, n: usize) -> Vec<(String, String)> {
    let mut lines = often_lines(text);
    lines.sort_by_key(|l| std::cmp::Reverse(l.0));
    lines.into_iter().take(n).map(|(_, name, target)| (name, target)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The often-used list: counted, the most first, at most the six asked for; a broken line is
    /// left out; a program counted again keeps one line.
    #[test]
    fn the_programs_used_most_come_first() {
        let mut text = String::new();
        for (name, target, times) in [("Notes", "C:\\p\\notes.lnk", 2), ("Paint", "C:\\p\\paint.lnk", 5), ("Games", "C:\\p\\games.lnk", 1)] {
            for _ in 0..times {
                text = often_counted(&text, name, target);
            }
        }
        assert_eq!(often_top(&text, 6), [("Paint".into(), "C:\\p\\paint.lnk".into()), ("Notes".into(), "C:\\p\\notes.lnk".into()), ("Games".into(), "C:\\p\\games.lnk".into())]);
        assert_eq!(often_top(&text, 1).len(), 1);
        assert_eq!(text.lines().count(), 3);
        assert_eq!(often_top("x\tbroken\n2\tNotes\tC:\\n.lnk\n", 6), [("Notes".into(), "C:\\n.lnk".into())]);
        assert!(often_top("", 6).is_empty(), "it starts empty");
    }

    fn names(n: usize) -> Vec<(String, Option<Icon>)> {
        (0..n).map(|i| (format!("Program {i}"), None)).collect()
    }

    /// The entries and what each is, with and without lists; separators always where they belong.
    #[test]
    fn every_entry_is_known_by_its_place() {
        let m = menu("User", &names(2), &names(6));
        assert_eq!(item_at(&m, 0), Some(Item::Pinned(0)));
        assert_eq!(item_at(&m, 1), Some(Item::Pinned(1)));
        assert_eq!(item_at(&m, 2), None, "a separator");
        assert_eq!(item_at(&m, 3), Some(Item::Often(0)));
        assert_eq!(item_at(&m, 8), Some(Item::Often(5)));
        assert_eq!(item_at(&m, 10), Some(Item::AllPrograms));
        assert_eq!(item_at(&m, 11), Some(Item::Right(Right::MyDocuments)));
        assert_eq!(item_at(&m, 12), Some(Item::Right(Right::MyRecentDocuments)));
        assert_eq!(item_at(&m, 16), None, "the right column's first line");
        assert_eq!(item_at(&m, 17), Some(Item::Right(Right::ControlPanel)));
        assert_eq!(item_at(&m, 21), Some(Item::Right(Right::Search)));
        assert_eq!(item_at(&m, 22), Some(Item::Right(Right::Run)));
        assert_eq!(item_at(&m, 23), Some(Item::LogOff));
        assert_eq!(item_at(&m, 24), Some(Item::TurnOff));
        assert_eq!(item_at(&m, 25), None);
        assert_eq!(m.entries.len(), 25);
        assert_eq!(m.entries[12].kind, EntryKind::Submenu, "My Recent Documents opens the documents");
        assert_eq!(m.entries[10].kind, EntryKind::Submenu, "All Programs opens Programs");
        // Nothing pinned, nothing used yet: All Programs is the first entry that is a program's.
        let m = menu("User", &[], &[]);
        assert_eq!(item_at(&m, 2), Some(Item::AllPrograms));
        assert_eq!(item_at(&m, 15), Some(Item::LogOff));
        assert_eq!(item_at(&m, 16), Some(Item::TurnOff));
        let l = layout(&m);
        assert_eq!((l.rects[0].w, l.rects[1].w), (0, 191), "the empty list's line is not drawn; the one over All Programs is");
    }

    /// The keys: the menu machinery's arrows go through the entries that can be highlighted, in
    /// the order left column, right column, bottom band; the separators never; the pointer finds
    /// an entry by its place.
    #[test]
    fn the_keys_and_the_pointer_reach_every_entry() {
        use crate::paint::MenuStyle;
        let m = menu("User", &names(2), &names(6));
        let reachable: Vec<usize> = (0..m.entries.len()).filter(|&i| m.selectable(i, MenuStyle::XpStart)).collect();
        assert_eq!(reachable.len(), 2 + 6 + 1 + 10 + 2);
        assert!(reachable.iter().all(|&i| item_at(&m, i).is_some()));
        let l = layout(&m);
        let at = |i: usize| (l.rects[i].x + 5, l.rects[i].y + 5);
        for &i in &reachable {
            let (x, y) = at(i);
            assert_eq!(l.hit(&m, MenuStyle::XpStart, x, y), Some(i), "{i}");
        }
        assert_eq!(l.hit(&m, MenuStyle::XpStart, 30, 30), None, "the user band is no entry");
    }

    /// The layout: 380×478 as the screenshots; the left column's items 40 high from 4 into the
    /// body, All Programs at body row 335, the right column's items around their icons; as many
    /// programs as fit (pinned first, at most six used).
    #[test]
    fn the_menu_is_laid_out_as_the_screenshots() {
        let m = menu("User", &names(2), &names(6));
        let l = layout(&m);
        assert_eq!((l.width, l.height), (380, 478));
        assert_eq!(l.rects[0], Rect::new(2, 71, 188, 40));
        assert_eq!(l.rects[3], Rect::new(2, 159, 188, 40), "after the pinned two and their line");
        assert_eq!(l.rects[10], Rect::new(2, 402, 188, 35), "All Programs");
        assert_eq!(l.rects[11], Rect::new(191, 71, 182, 32), "My Documents");
        assert!(l.rects[8].bottom() <= 67 + 333, "the six fit above the line");
        assert_eq!(fitting(2, 9), (2, 6));
        assert_eq!(fitting(0, 9), (0, 6));
        assert_eq!(fitting(5, 9), (5, 3));
        assert_eq!(fitting(12, 3), (8, 0));
        // Every entry's place lies inside the menu, none over another.
        for (i, a) in l.rects.iter().enumerate() {
            assert!(a.x >= 0 && a.right() <= 380 && a.y >= 0 && a.bottom() <= 478, "{i}");
            for b in l.rects.iter().skip(i + 1) {
                assert!(a.w == 0 || b.w == 0 || !(a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()), "{a:?} {b:?}");
            }
        }
    }
}

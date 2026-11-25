//! Windows 7 Basic's Start menu in the bar (brief 114): an ordinary `Menu` of the bar's menu
//! machinery (`MenuStyle::SevenStart`), its entries in a fixed order (`menu`), laid out from
//! `seven::menu::layout` and painted by `seven::menu::paint`, so the pointer, the keys and the
//! cascades work as the classic Start menu's do.
//!
//! The entries, in order: the programs started most often (often.txt), a separator, All Programs
//! (the Programs cascade, 2000's look: PROVISIONAL), the search box (a click opens Quick Search:
//! PROVISIONAL), the user's name (their profile folder), Documents, Pictures, Music, a separator,
//! Computer, a separator, Control Panel, Devices and Printers, Default Programs, Help and Support,
//! Shut down and its arrow (both the classic Shut Down window: PROVISIONAL). "Games" is left out:
//! today's Windows has no such place. A separator stands where it belongs also after an empty
//! list: `item_at` counts on it.
//!
//! The item under the pointer or chosen by the keys: no picture exists; a one-pixel frame in
//! `HOT_FRAME` (PROVISIONAL, Bevel95's choice) marks it so the menu can be used with the keys.

use crate::fb::{Framebuffer, Icon, Rect};
use crate::paint::{EntryKind, Menu, MenuEntry, MenuLayout};
use crate::seven::menu::{self as sm, ProgramRow, RightEntry, SevenMenu};
use crate::text::Fonts;

/// The right column's entries below the user's name, top to bottom (`None`: a separator).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Right {
    Documents,
    Pictures,
    Music,
    Computer,
    ControlPanel,
    Devices,
    DefaultPrograms,
    Help,
}

const RIGHT: [Option<(Right, &str)>; 10] = [
    Some((Right::Documents, "Documents")),
    Some((Right::Pictures, "Pictures")),
    Some((Right::Music, "Music")),
    None,
    Some((Right::Computer, "Computer")),
    None,
    Some((Right::ControlPanel, "Control Panel")),
    Some((Right::Devices, "Devices and Printers")),
    Some((Right::DefaultPrograms, "Default Programs")),
    Some((Right::Help, "Help and Support")),
];

/// What an entry of the 7 Start menu is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Program(usize),
    AllPrograms,
    Search,
    User,
    Right(Right),
    ShutDown,
}

/// The mark of the entry under the pointer (PROVISIONAL: no picture exists).
pub const HOT_FRAME: u32 = 0xFF7D_A2CE;

/// The menu's height and how many programs and right-column lines it keeps in a room `max_h`
/// rows high (the screen above the bar, in the look's pixels): the measured 485 while it fits;
/// shorter, the programs go first, then the right column's lines from its bottom; All Programs,
/// the search box and Shut down always stay (the XP menu's rule, brief 112's review).
pub fn fit(programs: usize, max_h: i32) -> (i32, usize, usize) {
    let h = max_h.clamp(sm::FIRST_ENTRY + sm::LINE_UP + 10, sm::MEASURED_H);
    let room = h - sm::LINE_UP - 4 - sm::FIRST_ROW;
    let p = programs.min((room.max(0) / sm::ROW_STEP) as usize);
    // The right column's lines: each item's row ends 23 below its top; they stay above Shut down.
    let limit = h - sm::SHUT_UP - 6;
    let mut r = RIGHT.len();
    while r > 0 {
        let probe = SevenMenu { right: right_entries(r), height: h, ..SevenMenu::default() };
        let l = sm::layout(&probe, h);
        if l.entry_rows.last().is_none_or(|e| e.bottom() <= limit) {
            break;
        }
        r -= 1;
    }
    (h, p, r)
}

fn right_entries(r: usize) -> Vec<RightEntry> {
    let mut list: Vec<RightEntry> = RIGHT.iter().take(r).map(|e| e.map_or(RightEntry::Separator, |(_, l)| RightEntry::Item(l.into()))).collect();
    while list.last() == Some(&RightEntry::Separator) {
        list.pop();
    }
    list
}

/// The 7 Start menu for the Windows user `user` and the programs started most often (names,
/// 32×32 pictures), for a room `max_h` rows high.
pub fn menu(user: &str, programs: &[(String, Option<Icon>)], max_h: i32) -> Menu {
    let (h, p, r) = fit(programs.len(), max_h);
    let mut entries: Vec<MenuEntry> = programs[..p].iter().map(|(n, i)| MenuEntry::item(n, i.clone())).collect();
    entries.push(MenuEntry::separator());
    entries.push(MenuEntry::submenu("All Programs", None));
    entries.push(MenuEntry::item("Search programs and files", None));
    entries.push(MenuEntry::item(user, None));
    for e in right_entries(r) {
        entries.push(match e {
            RightEntry::Item(label) => MenuEntry::item(&label, None),
            RightEntry::Separator => MenuEntry::separator(),
        });
    }
    entries.push(MenuEntry::item("Shut down", None));
    entries.push(MenuEntry::item("Shut down options", None));
    Menu { entries, user: user.to_string(), xp_fit: Some((h, r)), ..Menu::default() }
}

fn shape(menu: &Menu) -> (i32, usize) {
    menu.xp_fit.unwrap_or((sm::MEASURED_H, RIGHT.len()))
}

fn programs_of(menu: &Menu) -> usize {
    menu.entries.iter().position(|e| e.kind == EntryKind::Separator).unwrap_or(0)
}

/// What entry `index` of a menu made by `menu` is (`None`: a separator).
pub fn item_at(menu: &Menu, index: usize) -> Option<Item> {
    let p = programs_of(menu);
    if index < p {
        return Some(Item::Program(index));
    }
    let mut tail: Vec<Option<Item>> = vec![None, Some(Item::AllPrograms), Some(Item::Search), Some(Item::User)];
    // The right column's entries are `RIGHT`'s first ones, in its order (only trailing lines left out).
    let shown = right_entries(shape(menu).1).len();
    tail.extend(RIGHT.iter().take(shown).map(|e| e.map(|(r, _)| Item::Right(r))));
    tail.extend([Some(Item::ShutDown), Some(Item::ShutDown)]);
    tail.get(index - p).copied().flatten()
}

/// The `SevenMenu` the core paints for `menu`.
fn seven_of(menu: &Menu) -> SevenMenu {
    let (h, r) = shape(menu);
    let p = programs_of(menu);
    let programs = menu.entries[..p].iter().map(|e| ProgramRow { name: e.label.clone(), icon: e.icon.clone(), submenu: false }).collect();
    SevenMenu { programs, user: menu.user.clone(), right: right_entries(r), height: h, ..SevenMenu::default() }
}

/// Where each entry goes (one rectangle per entry, the part for pointing; a separator's empty).
pub fn layout(menu: &Menu) -> MenuLayout {
    let seven = seven_of(menu);
    let l = sm::layout(&seven, seven.height);
    let p = seven.programs.len();
    let none = Rect::new(0, 0, 0, 0);
    let mut rects: Vec<Rect> = l.program_rows.clone();
    rects.extend([none, l.all_programs, l.search_box]);
    let mut rows = l.entry_rows.iter();
    rects.push(rows.next().copied().unwrap_or(none));
    for e in &seven.right {
        rects.push(match e {
            RightEntry::Item(_) => rows.next().copied().unwrap_or(none),
            RightEntry::Separator => none,
        });
    }
    let shut = Rect { w: l.shut_down_arrow.x - l.shut_down.x, ..l.shut_down };
    rects.extend([shut, l.shut_down_arrow]);
    debug_assert_eq!(rects.len(), menu.entries.len(), "{p} programs");
    MenuLayout { width: sm::W, height: seven.height, rects, shortcut_x: 0, frame: 0, columns: vec![(0, sm::W)], lines: Vec::new(), scroll: None }
}

/// Paints the 7 Start menu (`fb` as `layout` makes it).
pub fn paint(fb: &mut Framebuffer, fonts: &Fonts, menu: &Menu, layout: &MenuLayout) {
    let seven = seven_of(menu);
    let l = sm::layout(&seven, seven.height);
    sm::paint(fb, &seven, &l, fonts.regular);
    if let Some(r) = menu.hover.and_then(|i| layout.rects.get(i)).copied().filter(|r| r.w > 0) {
        fb.frame(r, HOT_FRAME, HOT_FRAME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(n: usize) -> Vec<(String, Option<Icon>)> {
        (0..n).map(|i| (format!("Program {i}"), None)).collect()
    }

    /// Every entry by its place, with and without programs; the layout gives each a place inside
    /// the menu, found again by the pointer; separators are none.
    #[test]
    fn every_entry_is_known_and_found() {
        use crate::paint::MenuStyle;
        for n in [0, 3, 9] {
            let m = menu("User", &names(n), 485);
            let l = layout(&m);
            assert_eq!(l.rects.len(), m.entries.len());
            let kinds: Vec<Option<Item>> = (0..m.entries.len()).map(|i| item_at(&m, i)).collect();
            assert!(kinds.contains(&Some(Item::AllPrograms)) && kinds.contains(&Some(Item::Search)) && kinds.contains(&Some(Item::User)));
            assert!(kinds.contains(&Some(Item::Right(Right::Help))) && kinds.contains(&Some(Item::ShutDown)));
            for (i, e) in m.entries.iter().enumerate() {
                if m.selectable(i, MenuStyle::SevenStart) {
                    let r = l.rects[i];
                    assert!(r.w > 0 && r.bottom() <= l.height && r.right() <= l.width, "{n} {i} {:?}", e.label);
                    assert_eq!(l.hit(&m, MenuStyle::SevenStart, r.x + 3, r.y + 3), Some(i), "{n} {i}");
                }
            }
        }
        let m = menu("User", &names(2), 485);
        assert_eq!(item_at(&m, 0), Some(Item::Program(0)));
        assert_eq!(item_at(&m, 2), None);
        assert_eq!(item_at(&m, 6), Some(Item::Right(Right::Documents)));
        assert_eq!(item_at(&m, 9), None, "the separator after Music");
        assert_eq!(item_at(&m, 10), Some(Item::Right(Right::Computer)));
    }

    /// A short screen: the menu shortens to its room, programs first, then the right column's
    /// lines from its bottom; All Programs, the search box and Shut down stay.
    #[test]
    fn the_menu_shortens_to_a_short_screen() {
        assert_eq!(fit(9, 900), (485, fit(9, 485).1, 10));
        let mut last = (usize::MAX, usize::MAX);
        for room in [485, 420, 354, 300, 250] {
            let (h, p, r) = fit(9, room);
            assert!(h <= room.max(sm::FIRST_ENTRY + sm::LINE_UP + 10));
            assert!(p <= last.0 && r <= last.1);
            last = (p, r);
            let m = menu("User", &names(9), room);
            let kinds: Vec<Option<Item>> = (0..m.entries.len()).map(|i| item_at(&m, i)).collect();
            assert!(kinds.contains(&Some(Item::AllPrograms)) && kinds.contains(&Some(Item::ShutDown)), "{room}");
        }
    }
}

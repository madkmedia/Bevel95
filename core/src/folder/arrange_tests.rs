//! Brief 103 (GitHub issue 9): moved icons and Arrange Icons in folder windows, each rule of the
//! survey on the three Reference PCs (`private/probes/folder-arrange/survey.md`) as a test.

use super::saved::{Kept, Relaid, Saved, kept};
use super::*;
use crate::desktop_icons::{Arrange, Item, Picture, ViewMode};
use crate::fb::Icon;

fn fonts() -> (Font, Font) {
    let regular = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
    let bold = Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px-bold.font")).unwrap();
    (regular, bold)
}

/// The survey's folder `fa`: twelve text files.
const FA: [&str; 12] = ["apple", "berry", "cherry", "date", "elder", "fig", "grape", "honey", "iris", "juniper", "kiwi", "lemon"];

fn fa() -> Vec<Item> {
    FA.iter()
        .map(|n| Item {
            id: format!("C:\\fa\\{n}.txt"),
            name: format!("{n}.txt"),
            picture: Picture::Own(None),
            shortcut: false,
            folder: false,
            type_name: "Text Document".into(),
            size: 4,
            modified: 0,
        })
        .collect()
}

fn window(era: Era, files: bool, items: Vec<Item>) -> FolderWindow {
    let (r, b) = fonts();
    let mut w = FolderWindow::new(era, "t", Icon { width: 16, height: 16, pixels: vec![0; 256] }, files, (640, 420));
    w.layout(&Fonts { regular: &r, bold: &b });
    w.set_items(items);
    w
}

fn id(n: &str) -> String {
    format!("C:\\fa\\{n}.txt")
}

/// Where item `id`'s cell is (the view's pixels).
fn at(w: &FolderWindow, id: &str) -> (i32, i32) {
    let i = w.view.index_of(id).expect("there");
    let c = w.view.cell(i);
    (c.x, c.y)
}

fn all(w: &FolderWindow) -> Vec<(i32, i32)> {
    (0..w.view.items.len()).map(|i| (w.view.cell(i).x, w.view.cell(i).y)).collect()
}

fn names(w: &FolderWindow) -> Vec<String> {
    w.view.items.iter().map(|i| i.name.trim_end_matches(".txt").to_string()).collect()
}

/// Decision 1: a dropped icon lies exactly where it is dropped (no grid), and going to Details
/// or List and back moves nothing, in Large and Small Icons, in every look
/// (`*-arrange-mc-c-moved-then-details`, `-details-back-to-large`).
#[test]
fn a_dropped_icon_stays_where_it_is_dropped_through_details_and_back() {
    for era in Era::ALL {
        for free in [ViewMode::Large, ViewMode::Small] {
            let mut w = window(era, true, fa());
            w.set_view_mode(free);
            let before = at(&w, &id("fig"));
            w.view.move_ids(&[id("fig")], (171, 173));
            let dropped = at(&w, &id("fig"));
            assert_eq!(dropped, (before.0 + 171, before.1 + 173), "{era:?} {free:?}: no grid");
            let everything = all(&w);
            for other in [ViewMode::Details, ViewMode::List] {
                w.set_view_mode(other);
                w.set_view_mode(free);
                assert_eq!(all(&w), everything, "{era:?} {free:?} via {other:?}: nothing moved");
            }
            assert_eq!(w.view.moved_places(), vec![(dropped.0 - w.view.area().x, dropped.1 - w.view.area().y, id("fig"))]);
        }
    }
}

/// Decision 7: only the icons really moved are kept (never every icon of the folder); kept for
/// good where the look keeps them: the window opened again shows them where they were left, the
/// others in order around them.
#[test]
fn only_moved_icons_are_kept_and_put_back_when_the_window_opens_again() {
    let mut w = window(Era::Win95, true, fa());
    w.view.move_ids(&[id("fig")], (286, 237));
    w.view.move_ids(&[id("apple")], (0, 300));
    let kept = w.view.moved_places();
    assert_eq!(kept.iter().map(|k| k.2.clone()).collect::<Vec<_>>(), [id("fig"), id("apple")], "in the order moved, nothing else");
    let fig = at(&w, &id("fig"));
    let apple = at(&w, &id("apple"));
    // Opened again: a new window of the same folder.
    let mut again = window(Era::Win95, true, fa());
    assert!(again.view.load_moved(&kept, ViewMode::Large));
    assert_eq!((at(&again, &id("fig")), at(&again, &id("apple"))), (fig, apple));
    assert_eq!(again.view.moved_ids(), [id("fig"), id("apple")]);
    // Brief 105, 4: the others stand where they stood, each at its own slot in the order, and the
    // cells apple and fig left stay empty (`95 mc-c-moved-closed-reopened-2`: "The other icons do
    // not move"); berry is NOT where apple was.
    let fresh = window(Era::Win95, true, fa());
    for n in FA.iter().filter(|n| !["apple", "fig"].contains(n)) {
        assert_eq!(at(&again, &id(n)), at(&fresh, &id(n)), "{n} where it stood");
    }
    let shown = all(&again);
    for gone in ["apple", "fig"] {
        assert!(!shown.contains(&at(&fresh, &id(gone))), "{gone}'s cell stays empty");
    }
}

/// Brief 105, 4: an icon whose own slot a moved icon now covers takes the next free cell; the
/// others still stand at their own slots.
#[test]
fn an_icon_whose_slot_is_covered_takes_the_next_free_cell() {
    let mut w = window(Era::Win95, true, fa());
    let fresh = all(&w);
    // lemon (the last) dropped exactly on berry's slot.
    let (berry, lemon) = (at(&w, &id("berry")), at(&w, &id("lemon")));
    w.view.move_ids(&[id("lemon")], (berry.0 - lemon.0, berry.1 - lemon.1));
    let kept = w.view.moved_places();
    let mut again = window(Era::Win95, true, fa());
    assert!(again.view.load_moved(&kept, ViewMode::Large));
    assert_eq!(at(&again, &id("lemon")), berry);
    // berry's slot is covered: it takes the first free cell from there on (lemon's old one, the
    // only free one), every other icon at its own slot.
    assert_eq!(at(&again, &id("berry")), fresh[11]);
    for (k, n) in FA.iter().enumerate().filter(|(_, n)| !["berry", "lemon"].contains(n)) {
        assert_eq!(at(&again, &id(n)), fresh[k], "{n}");
    }
}

/// A window opened again is laid out before its items come, and a small one then gets a scroll
/// bar and puts its icons in rows for the narrower list: the moved icon still stays where it was
/// left (found by the live try on a scratch folder, `try-win95-5-reopened`).
#[test]
fn a_moved_icon_stays_when_a_small_window_gets_its_scroll_bar() {
    let (r, b) = fonts();
    let mut w = FolderWindow::new(Era::Win95, "t", Icon { width: 16, height: 16, pixels: vec![0; 256] }, true, (340, 290));
    w.layout(&Fonts { regular: &r, bold: &b });
    w.set_items(fa());
    w.view.move_ids(&[id("cherry")], (171, 173));
    let kept = w.view.moved_places();
    let mut again = FolderWindow::new(Era::Win95, "t", Icon { width: 16, height: 16, pixels: vec![0; 256] }, true, (340, 290));
    again.layout(&Fonts { regular: &r, bold: &b });
    assert!(again.view.load_moved(&kept, ViewMode::Large));
    again.set_items(fa());
    assert_eq!(again.view.moved_places(), kept);
    assert_eq!(names(&again)[2], "cherry", "still in its order, only its place is its own");
}

/// Decision 2, the survey's table of steps per look.
#[test]
fn each_look_keeps_a_moved_icon_as_measured() {
    use Kept::*;
    use Relaid::*;
    for when in [Refresh, Returned, Reopened] {
        // 95 for good; 98 without its web view for good; 2000 classic for good.
        assert_eq!(kept(Era::Win95, false, true, when), Places);
        assert_eq!(kept(Era::Win95, false, false, when), Places);
        assert_eq!(kept(Era::Win98, false, true, when), Places);
        assert_eq!(kept(Era::Win98, false, false, when), Places);
        assert_eq!(kept(Era::Win2000, false, true, when), Places);
        // 98 with its web view: an ordinary folder packed with the moved icon last at every step.
        assert_eq!(kept(Era::Win98, true, false, when), MovedLast);
    }
    // 98 with its web view, My Computer: kept while the window stays open, packed when opened again.
    assert_eq!(kept(Era::Win98, true, true, Refresh), Places, "PROVISIONAL: seen once");
    assert_eq!(kept(Era::Win98, true, true, Returned), Places);
    assert_eq!(kept(Era::Win98, true, true, Reopened), MovedLast);
    // 2000 with its web view: F5 puts every icon back in its order; Back, Up, a reopen keep it.
    for mc in [true, false] {
        assert_eq!(kept(Era::Win2000, true, mc, Refresh), InOrder);
        assert_eq!(kept(Era::Win2000, true, mc, Returned), Places);
        assert_eq!(kept(Era::Win2000, true, mc, Reopened), Places);
    }
}

/// Windows 98 with its web view: packed, the moved icon LAST, and it stays last when the folder is
/// read again (F5) until Arrange Icons by ... (`win98-truecolor-arrange-fa-fig-moved-after-f5`,
/// `-mc-c-moved-closed-reopened`).
#[test]
fn windows_98_packs_with_the_moved_icon_last() {
    let mut w = window(Era::Win98, true, fa());
    let packed = all(&w);
    w.view.move_ids(&[id("fig")], (171, 173));
    let moved = w.view.moved_ids().to_vec();
    w.view.pack_moved_last(&moved);
    let order = names(&w);
    assert_eq!(order.last().map(String::as_str), Some("fig"));
    assert_eq!(&order[..5], ["apple", "berry", "cherry", "date", "elder"]);
    assert_eq!(order[5], "grape");
    assert_eq!(all(&w), packed, "packed into the order's places, nothing where it was dropped");
    assert!(w.view.moved_ids().is_empty());
    // Read again: still last.
    w.set_items(fa());
    assert_eq!(names(&w).last().map(String::as_str), Some("fig"));
    assert_eq!(w.view.last_order(), [id("fig")]);
    // Opened again from the order kept (Saved.last): the same.
    let mut again = window(Era::Win98, true, fa());
    again.view.pack_moved_last(&[id("fig")]);
    assert_eq!(names(&again), order);
    // Arrange Icons by Name: every icon back in order.
    w.arrange_by(Arrange::Name);
    assert_eq!(names(&w), FA);
    assert!(w.view.last_order().is_empty());
}

/// 2000's F5 with its web view: every icon back in its order (not "moved one last"), a kept
/// "moved last" order included (brief 105, 9).
#[test]
fn windows_2000_refresh_puts_icons_back_in_order() {
    let mut w = window(Era::Win2000, true, fa());
    let packed = all(&w);
    w.view.move_ids(&[id("fig")], (60, 192));
    assert_ne!(all(&w), packed);
    w.view.back_in_order();
    assert_eq!(all(&w), packed);
    assert_eq!(names(&w), FA, "in order, fig in its own place");
    assert!(w.view.moved_places().is_empty());
    // An order with fig last (kept from another step): back in order too.
    w.view.pack_moved_last(&[id("fig")]);
    assert_eq!(names(&w).last().map(String::as_str), Some("fig"));
    w.view.back_in_order();
    assert_eq!(names(&w), FA);
    assert!(w.view.last_order().is_empty());
}

/// Decision 4: Line up Icons moves an icon off the grid onto the nearest grid place (75 x 75 from
/// the first icon's place), not back into its order; icons on the grid stay
/// (`*-arrange-mc-c-moved-crooked-after-line-up`).
#[test]
fn line_up_puts_a_crooked_icon_on_the_nearest_grid_place() {
    for era in Era::ALL {
        let mut w = window(era, true, fa());
        let origin = at(&w, &id("apple"));
        let others: Vec<(i32, i32)> = FA.iter().filter(|n| **n != "lemon").map(|n| at(&w, &id(n))).collect();
        // lemon dropped crooked below the rows: 3 columns across and 31 down from apple's corner,
        // 7 off the grid each way.
        let lemon = at(&w, &id("lemon"));
        let target = (origin.0 + 3 * 75 + 7, origin.1 + 5 * 75 - 31);
        w.view.move_ids(&[id("lemon")], (target.0 - lemon.0, target.1 - lemon.1));
        assert_eq!(at(&w, &id("lemon")), target);
        w.line_up();
        assert_eq!(at(&w, &id("lemon")), (origin.0 + 3 * 75, origin.1 + 5 * 75), "{era:?}: the nearest grid place");
        let after: Vec<(i32, i32)> = FA.iter().filter(|n| **n != "lemon").map(|n| at(&w, &id(n))).collect();
        assert_eq!(after, others, "{era:?}: the others stay");
    }
}

/// Decision 5: Auto Arrange on packs in the present order, a dragged icon last; a drop between two
/// others takes that place in the order; beyond the last it becomes the last; off, they stay.
#[test]
fn auto_arrange_as_measured() {
    let mut w = window(Era::Win95, true, fa());
    let packed = all(&w);
    w.view.move_ids(&[id("cherry")], (300, 300));
    w.set_auto_arrange(true);
    let order = names(&w);
    assert_eq!(order.last().map(String::as_str), Some("cherry"), "the dragged icon last (`95 fa-auto-arrange-on`)");
    assert_eq!(all(&w), packed);
    // apple dropped between fig and grape (one row): it takes that place in the order.
    let (fig, grape) = (at(&w, &id("fig")), at(&w, &id("grape")));
    assert_eq!(fig.1, grape.1);
    let apple = at(&w, &id("apple"));
    let between = ((fig.0 + grape.0) / 2 + 2, fig.1);
    w.view.move_ids(&[id("apple")], (between.0 - apple.0, between.1 - apple.1));
    let shown = |w: &FolderWindow| {
        let mut v: Vec<(i32, i32, String)> = FA.iter().map(|n| (at(w, &id(n)).1, at(w, &id(n)).0, n.to_string())).collect();
        v.sort();
        v.into_iter().map(|t| t.2).collect::<Vec<_>>()
    };
    let o = shown(&w);
    let k = o.iter().position(|n| n == "apple").unwrap();
    assert_eq!((o[k - 1].as_str(), o[k + 1].as_str()), ("fig", "grape"), "{o:?}");
    assert_eq!(all(&w).iter().filter(|c| packed.contains(c)).count(), 12, "still packed");
    // Beyond the last one: the last.
    let date = at(&w, &id("date"));
    w.view.move_ids(&[id("date")], (520 - date.0, 330 - date.1));
    assert_eq!(shown(&w).last().map(String::as_str), Some("date"));
    // Off: everything stays.
    let before = all(&w);
    w.set_auto_arrange(false);
    assert_eq!(all(&w), before);
}

/// Decision 6: Arrange Icons by ... puts every icon back in order; My Computer by Drive Letter and
/// by Free Space with the facts the window already has (`spaces`, Details' own).
#[test]
fn my_computer_arranges_by_drive_letter_and_free_space() {
    let item = |id: &str, name: &str| Item {
        id: id.into(),
        name: name.into(),
        picture: Picture::Own(None),
        shortcut: false,
        folder: false,
        type_name: String::new(),
        size: 0,
        modified: 0,
    };
    // The order My Computer is read in: the drives by letter, then its places.
    let mc = vec![
        item("A:\\", "3\u{bd} Floppy (A:)"),
        item("C:\\", "Win2000 (C:)"),
        item("D:\\", "(D:)"),
        item("::control-panel", "Control Panel"),
        item("::printers", "Printers"),
    ];
    let mut w = window(Era::Win2000, false, mc.clone());
    w.spaces.insert("C:\\".into(), Space { free: 500, capacity: 2000 });
    w.spaces.insert("D:\\".into(), Space { free: 0, capacity: 600 });
    w.set_items(mc.clone());
    let order = |w: &FolderWindow| w.view.items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(order(&w), ["A:\\", "C:\\", "D:\\", "::control-panel", "::printers"]);
    w.view.move_ids(&["C:\\".to_string()], (60, 192));
    w.arrange_by(Arrange::Date);
    // `win2000-truecolor-arrange-mc-after-arrange-by-free-space`: the floppy (no disk: 0), the
    // compact disc (0), C:, then Control Panel.
    assert_eq!(order(&w), ["A:\\", "D:\\", "C:\\", "::control-panel", "::printers"]);
    assert!(w.view.moved_places().is_empty(), "back in order");
    w.arrange_by(Arrange::Size);
    assert_eq!(order(&w), ["A:\\", "D:\\", "C:\\", "::control-panel", "::printers"]);
    w.arrange_by(Arrange::Name);
    assert_eq!(order(&w), ["A:\\", "C:\\", "D:\\", "::control-panel", "::printers"], "by Drive Letter");
    // Read again: kept by drive letter.
    w.set_items(mc);
    assert_eq!(order(&w), ["A:\\", "C:\\", "D:\\", "::control-panel", "::printers"]);
}

/// Decision 7: the record read, written and pruned: only moved icons, by name within the folder,
/// the oldest folders going beyond the limit, an older file's every-icon line read no more.
#[test]
fn the_record_is_read_written_and_pruned() {
    use saved::arranged_key;
    let e = Era::Win95;
    let mut s = Saved::default();
    assert!(s.remember_places(e, "c:\\fa", ViewMode::Large, vec![(446, 332, "C:\\fa\\fig.txt".into())]));
    assert!(s.remember_last(e, "c:\\fb", vec!["C:\\fb\\win.com".into()]));
    let back = Saved::parse(&s.text());
    assert_eq!(back.places_of(e, "c:\\fa"), Some((ViewMode::Large, vec![(446, 332, "C:\\fa\\fig.txt".to_string())])));
    assert_eq!(back.last_of(e, "c:\\fb"), ["C:\\fb\\win.com"]);
    assert_eq!(back.arranged, [arranged_key(e, "c:\\fa"), arranged_key(e, "c:\\fb")]);
    // Brief 105, 7: the same again is no change (nothing to write).
    let mut same = back.clone();
    assert!(!same.remember_places(e, "c:\\fa", ViewMode::Large, vec![(446, 332, "C:\\fa\\fig.txt".into())]));
    assert!(!same.remember_last(e, "c:\\fb", vec!["C:\\fb\\win.com".into()]));
    assert!(!same.remember_last(e, "c:\\nothing", Vec::new()));
    // The view is part of the record: the same places in another view are a change.
    assert!(same.remember_places(e, "c:\\fa", ViewMode::Small, vec![(446, 332, "C:\\fa\\fig.txt".into())]));
    // An empty list forgets.
    let mut t = back.clone();
    assert!(t.remember_places(e, "c:\\fa", ViewMode::Large, Vec::new()));
    assert!(t.places_of(e, "c:\\fa").is_none() && t.arranged == [arranged_key(e, "c:\\fb")]);
    // At most MOST_ARRANGED folders: the one changed longest ago goes; a change makes it new.
    let mut u = Saved::default();
    for k in 0..saved::MOST_ARRANGED + 5 {
        u.remember_places(e, &format!("c:\\f{k}"), ViewMode::Large, vec![(0, 0, "x".into())]);
        if k == 10 {
            u.remember_last(e, "c:\\f0", vec!["y".into()]);
        }
    }
    assert_eq!(u.arranged.len(), saved::MOST_ARRANGED);
    assert!(u.places_of(e, "c:\\f1").is_none() && u.places_of(e, "c:\\f3").is_none(), "the oldest went");
    assert_eq!(u.last_of(e, "c:\\f0"), ["y"], "changed later: kept");
    let reread = Saved::parse(&u.text());
    assert_eq!(reread.arranged, u.arranged, "the age survives the file");
    // At most MOST_MOVED icons for one folder: the last moved.
    let mut v = Saved::default();
    v.remember_places(e, "c:\\big", ViewMode::Large, (0..saved::MOST_MOVED as i32 + 10).map(|i| (i, i, format!("{i}"))).collect());
    let (_, big) = v.places_of(e, "c:\\big").unwrap();
    assert_eq!(big.len(), saved::MOST_MOVED);
    assert_eq!(big[0].2, "10");
    // A line of before brief 103 (every icon's place) is read no more, nor brief 103's own lines
    // without a look and a view (brief 105, 8: dropped; those folders open in order once).
    assert!(Saved::parse("places\tc:\\old\t1 2 a\t3 4 b\n").places.is_empty());
    let old = Saved::parse("moved\tc:\\fa\t446 332 C:\\fa\\fig.txt\nlast\tc:\\fb\tC:\\fb\\win.com\n");
    assert!(old.places.is_empty() && old.last.is_empty() && old.arranged.is_empty());
}

/// Brief 105, 6: the limits hold for a file as it is read too: at most MOST_MOVED icons a folder
/// (the last moved), MOST_ARRANGED folders (the last written), no id or folder longer than
/// LONGEST, no view but Large or Small Icons.
#[test]
fn the_limits_hold_when_the_file_is_read() {
    let word = Era::Win98.setting();
    let mut text = String::new();
    // A folder with 100 moved icons.
    text += &format!("moved\t{word}\tlarge\tc:\\big");
    for i in 0..100 {
        text += &format!("\t{i} {i} id{i}");
    }
    text += "\n";
    // 250 more folders.
    for k in 0..250 {
        text += &format!("moved\t{word}\tsmall\tc:\\f{k}\t1 2 a\n");
    }
    let s = Saved::parse(&text);
    assert_eq!(s.arranged.len(), saved::MOST_ARRANGED);
    assert!(s.places_of(Era::Win98, "c:\\big").is_none(), "the oldest went");
    assert!(s.places_of(Era::Win98, "c:\\f249").is_some());
    let one = Saved::parse(text.lines().next().unwrap());
    let (view, big) = one.places_of(Era::Win98, "c:\\big").unwrap();
    assert_eq!(view, ViewMode::Large);
    assert_eq!(big.len(), saved::MOST_MOVED);
    assert_eq!(big[0].2, "id36", "the last moved");
    // Too long, or a view that holds no places.
    let long = "x".repeat(saved::LONGEST + 1);
    let odd = Saved::parse(&format!(
        "moved\t{word}\tlarge\tc:\\{long}\t1 2 a\nmoved\t{word}\tlarge\tc:\\fa\t1 2 {long}\nmoved\t{word}\tdetails\tc:\\fb\t1 2 a\nlast\t{word}\tc:\\fc\t{long}\n"
    ));
    assert!(odd.places.is_empty() && odd.last.is_empty() && odd.arranged.is_empty());
}

/// Brief 105, 8: a record belongs to the look it was made in; another look never sees it.
#[test]
fn a_record_belongs_to_its_look() {
    let mut s = Saved::default();
    s.remember_places(Era::Win95, "c:\\fa", ViewMode::Large, vec![(446, 332, id("fig"))]);
    s.remember_last(Era::Win98, "c:\\fa", vec![id("fig")]);
    let back = Saved::parse(&s.text());
    assert!(back.places_of(Era::Win95, "c:\\fa").is_some());
    assert!(back.places_of(Era::Win98, "c:\\fa").is_none() && back.places_of(Era::Win2000, "c:\\fa").is_none());
    assert_eq!(back.last_of(Era::Win98, "c:\\fa"), [id("fig")]);
    assert!(back.last_of(Era::Win95, "c:\\fa").is_empty() && back.last_of(Era::Win2000, "c:\\fa").is_empty());
    // Forgetting one look's record leaves the other's.
    let mut t = back.clone();
    t.remember_places(Era::Win95, "c:\\fa", ViewMode::Large, Vec::new());
    assert_eq!(t.last_of(Era::Win98, "c:\\fa"), [id("fig")]);
    assert_eq!(t.arranged, [saved::arranged_key(Era::Win98, "c:\\fa")]);
}

/// Brief 105, 1: a place belongs to the view it was made in. A change of view moves the moved
/// icon as the view's own step does, and what is kept then is in the new view; a window opened in
/// the other view than the record's: 95 and 98 move it as that step does, 2000 does not use it
/// (its own step puts the icons back in order).
#[test]
fn a_place_belongs_to_the_view_it_was_made_in() {
    for era in Era::ALL {
        let mut w = window(era, true, fa());
        w.view.move_ids(&[id("fig")], (171, 173));
        let large = w.view.moved_places();
        assert_eq!(w.view.moved_view(), ViewMode::Large);
        // Large to Small in the window itself.
        w.set_view_mode(ViewMode::Small);
        if era == Era::Win2000 {
            assert!(w.view.moved_places().is_empty(), "2000: back in order");
        } else {
            let small = w.view.moved_places();
            assert_eq!(w.view.moved_view(), ViewMode::Small);
            assert_eq!(small.len(), 1);
            assert_ne!((small[0].0, small[0].1), (large[0].0, large[0].1), "{era:?}: stepped");
            // Opened again in Small Icons from the Small record: the same place.
            let mut again = window(era, true, fa());
            again.set_view_mode(ViewMode::Small);
            assert!(again.view.load_moved(&small, ViewMode::Small));
            assert_eq!(again.view.moved_places(), small);
        }
        // Opened in Small Icons from a Large record.
        let mut other = window(era, true, fa());
        other.set_view_mode(ViewMode::Small);
        let used = other.view.load_moved(&large, ViewMode::Large);
        if era == Era::Win2000 {
            assert!(!used, "2000: not used, the record goes");
            assert!(other.view.moved_places().is_empty());
        } else {
            assert!(used);
            let mut stepped = window(era, true, fa());
            stepped.view.move_ids(&[id("fig")], (171, 173));
            stepped.set_view_mode(ViewMode::Small);
            assert_eq!(other.view.moved_places(), stepped.view.moved_places(), "{era:?}: as the view's own step");
            assert_eq!(other.view.moved_view(), ViewMode::Small);
        }
        // Opened in Details from a Large record: kept for the way back to Large, in Large.
        for list in [ViewMode::Details, ViewMode::List] {
            let mut d = window(era, true, fa());
            d.set_view_mode(list);
            assert!(d.view.load_moved(&large, ViewMode::Large));
            assert_eq!(d.view.moved_view(), ViewMode::Large, "{era:?} {list:?}");
            assert_eq!(d.view.moved_places(), large);
            d.set_view_mode(ViewMode::Large);
            assert_eq!(d.view.moved_places(), large, "{era:?} {list:?}: back where it was left");
        }
    }
}

/// A window opened again in Small Icons is laid out before its items come; their first measuring
/// (the columns' width) then must not put the moved icon back in order (found by the live try of
/// brief 105, `try-win95-7-small-reopened`).
#[test]
fn a_moved_icon_stays_when_a_small_icons_window_measures_its_items() {
    let (r, b) = fonts();
    for era in Era::ALL {
        let mut w = window(era, true, fa());
        w.set_view_mode(ViewMode::Small);
        let c = at(&w, &id("cherry"));
        w.view.move_ids(&[id("cherry")], (448 - c.0, 34 - c.1));
        let kept = w.view.moved_places();
        let mut again = FolderWindow::new(era, "t", Icon { width: 16, height: 16, pixels: vec![0; 256] }, true, (640, 420));
        again.layout(&Fonts { regular: &r, bold: &b });
        again.set_view_mode(ViewMode::Small);
        assert!(again.view.load_moved(&kept, ViewMode::Small));
        again.set_items(fa());
        assert_eq!(again.view.moved_places(), kept, "{era:?}");
        assert_eq!(at(&again, &id("cherry")), at(&w, &id("cherry")), "{era:?}");
    }
}

/// Brief 105, 2: in List or Details, showing or hiding the tree (any repack) leaves the kept
/// places alone.
#[test]
fn the_tree_shown_in_details_leaves_the_places_alone() {
    for era in Era::ALL {
        let mut w = window(era, true, fa());
        w.view.move_ids(&[id("fig")], (171, 173));
        let large = w.view.moved_places();
        let everything = all(&w);
        w.set_view_mode(ViewMode::Details);
        w.view.repack();
        w.view.repack();
        assert_eq!(w.view.moved_places(), large, "{era:?}");
        w.set_view_mode(ViewMode::Large);
        assert_eq!(all(&w), everything, "{era:?}: nothing moved");
    }
}

/// Brief 105, 3: with Auto Arrange on, the order is what is kept: an icon dropped between two
/// others is still between them after the window is opened again and after a repack.
#[test]
fn auto_arrange_keeps_the_order_after_a_reopen() {
    let mut w = window(Era::Win95, true, fa());
    w.set_auto_arrange(true);
    let (fig, grape) = (at(&w, &id("fig")), at(&w, &id("grape")));
    let apple = at(&w, &id("apple"));
    let between = ((fig.0 + grape.0) / 2 + 2, fig.1);
    w.view.move_ids(&[id("apple")], (between.0 - apple.0, between.1 - apple.1));
    let order = |w: &FolderWindow| {
        let mut v: Vec<(i32, i32, String)> = FA.iter().map(|n| (at(w, &id(n)).1, at(w, &id(n)).0, n.to_string())).collect();
        v.sort();
        v.into_iter().map(|t| t.2).collect::<Vec<_>>()
    };
    let shown = order(&w);
    let k = shown.iter().position(|n| n == "apple").unwrap();
    assert_eq!((shown[k - 1].as_str(), shown[k + 1].as_str()), ("fig", "grape"));
    let kept = w.view.moved_places();
    let mut again = window(Era::Win95, true, fa());
    again.set_auto_arrange(true);
    assert!(again.view.load_moved(&kept, ViewMode::Large));
    assert_eq!(order(&again), shown, "after a reopen");
    again.view.repack();
    assert_eq!(order(&again), shown, "after a repack");
}

/// Brief 105, 10: a rename carries the icon's place along (what is kept names it by its new
/// name, in the window and for a reopen).
#[test]
fn a_rename_carries_the_place_along() {
    let mut w = window(Era::Win95, true, fa());
    w.view.move_ids(&[id("fig")], (171, 173));
    let before = w.view.moved_places();
    let fig = at(&w, &id("fig"));
    w.view.renamed(&id("fig"), &id("figs"), "figs.txt");
    let after = w.view.moved_places();
    assert_eq!(after, vec![(before[0].0, before[0].1, id("figs"))]);
    assert_eq!(at(&w, &id("figs")), fig);
    assert_eq!(w.view.moved_ids(), [id("figs")]);
    // Also while waiting in Details.
    w.set_view_mode(ViewMode::Details);
    w.view.renamed(&id("figs"), &id("fig"), "fig.txt");
    assert_eq!(w.view.moved_places(), vec![(before[0].0, before[0].1, id("fig"))]);
}

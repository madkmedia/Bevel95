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
    again.view.load_moved(&kept);
    assert_eq!((at(&again, &id("fig")), at(&again, &id("apple"))), (fig, apple));
    assert_eq!(again.view.moved_ids(), [id("fig"), id("apple")]);
    // The others stand in order in the free places: berry where apple was.
    let fresh = window(Era::Win95, true, fa());
    assert_eq!(at(&again, &id("berry")), at(&fresh, &id("apple")));
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

/// 2000's F5 with its web view: every icon back in its order (not "moved one last").
#[test]
fn windows_2000_refresh_puts_icons_back_in_order() {
    let mut w = window(Era::Win2000, true, fa());
    let packed = all(&w);
    w.view.move_ids(&[id("fig")], (60, 192));
    assert_ne!(all(&w), packed);
    let by = w.view.arrangement();
    w.view.set_arrange(by);
    assert_eq!(all(&w), packed);
    assert_eq!(names(&w), FA, "in order, fig in its own place");
    assert!(w.view.moved_places().is_empty());
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
    let mut s = Saved::default();
    s.remember_places("c:\\fa", vec![(446, 332, "C:\\fa\\fig.txt".into())]);
    s.remember_last("c:\\fb", vec!["C:\\fb\\win.com".into()]);
    let back = Saved::parse(&s.text());
    assert_eq!(back.places_of("c:\\fa"), [(446, 332, "C:\\fa\\fig.txt".to_string())]);
    assert_eq!(back.last_of("c:\\fb"), ["C:\\fb\\win.com"]);
    assert_eq!(back.arranged, ["c:\\fa", "c:\\fb"]);
    // An empty list forgets.
    let mut t = back.clone();
    t.remember_places("c:\\fa", Vec::new());
    assert!(t.places_of("c:\\fa").is_empty() && t.arranged == ["c:\\fb"]);
    // At most MOST_ARRANGED folders: the one changed longest ago goes; a change makes it new.
    let mut u = Saved::default();
    for k in 0..saved::MOST_ARRANGED + 5 {
        u.remember_places(&format!("c:\\f{k}"), vec![(0, 0, "x".into())]);
        if k == 10 {
            u.remember_last("c:\\f0", vec!["y".into()]);
        }
    }
    assert_eq!(u.arranged.len(), saved::MOST_ARRANGED);
    assert!(u.places_of("c:\\f1").is_empty() && u.places_of("c:\\f3").is_empty(), "the oldest went");
    assert_eq!(u.last_of("c:\\f0"), ["y"], "changed later: kept");
    let reread = Saved::parse(&u.text());
    assert_eq!(reread.arranged, u.arranged, "the age survives the file");
    // At most MOST_MOVED icons for one folder: the last moved.
    let mut v = Saved::default();
    v.remember_places("c:\\big", (0..saved::MOST_MOVED as i32 + 10).map(|i| (i, i, format!("{i}"))).collect());
    assert_eq!(v.places_of("c:\\big").len(), saved::MOST_MOVED);
    assert_eq!(v.places_of("c:\\big")[0].2, "10");
    // A line of before brief 103 (every icon's place) is read no more.
    assert!(Saved::parse("places\tc:\\old\t1 2 a\t3 4 b\n").places.is_empty());
}

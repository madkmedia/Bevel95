//! Quick Launch: the band of small one-click icons right of the Start button that Internet
//! Explorer 4 brought to Windows 95 and that Windows 98 and 2000 had from the start
//! (docs/decisions.md, "Quick Launch").
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/quick_launch.rs`); this
//! module chooses them.

use crate::art::Look;
use crate::fb::{Icon, Rect};
use crate::icons::ClassicIcon;
use crate::paint::{Menu, MenuEntry};
use crate::theme::Era;

/// The icons Bevel95 puts on the band itself (the person's own shortcuts follow them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    /// Starts the person's web browser (98's and 2000's "Launch Internet Explorer Browser").
    InternetExplorer,
    /// Starts the person's mail program ("Launch Outlook Express").
    OutlookExpress,
    /// Windows' own Show Desktop: every window away, and back at the next click.
    ShowDesktop,
    /// Bevel95's own addition: its Windows Explorer, as the Programs menu's entry starts it.
    WindowsExplorer,
}

impl Builtin {
    /// Every built-in icon.
    pub const ALL: [Builtin; 4] = [Builtin::InternetExplorer, Builtin::OutlookExpress, Builtin::ShowDesktop, Builtin::WindowsExplorer];

    /// Its name in quick-launch.txt (the built-in icons the person took off the band).
    pub fn key(self) -> &'static str {
        match self {
            Builtin::InternetExplorer => "internet-explorer",
            Builtin::OutlookExpress => "outlook-express",
            Builtin::ShowDesktop => "show-desktop",
            Builtin::WindowsExplorer => "windows-explorer",
        }
    }

    pub fn from_key(key: &str) -> Option<Builtin> {
        Builtin::ALL.into_iter().find(|b| b.key().eq_ignore_ascii_case(key.trim()))
    }

    /// Its tooltip in `era`: the originals' words (the Reference PCs: the names of the files in their
    /// Quick Launch folders; Windows 2000 adds a shortcut's description as a second line, which only
    /// its Internet Explorer shortcut has); Windows Explorer's is Bevel95's own.
    pub fn tooltip(self, era: Era) -> &'static str {
        match self {
            Builtin::InternetExplorer if era == Era::Win2000 => {
                "Launch Internet Explorer Browser\nFinds and displays information and Web sites on the Internet."
            }
            Builtin::InternetExplorer => "Launch Internet Explorer Browser",
            Builtin::OutlookExpress => "Launch Outlook Express",
            Builtin::ShowDesktop => "Show Desktop",
            Builtin::WindowsExplorer => "Windows Explorer",
        }
    }
}

/// The built-in icons of `era`, left to right: each look's own order (Windows 98's Internet
/// Explorer, Outlook Express, Show Desktop; Windows 2000's Show Desktop first), then Windows
/// Explorer. Windows 95 had no Quick Launch: switched on, it shows the icons 95's own files have a
/// picture for (its globe for the browser, its desktop for Show Desktop; no mail picture).
pub fn builtins(era: Era) -> &'static [Builtin] {
    use Builtin::*;
    match era {
        Era::Win95 => &[InternetExplorer, ShowDesktop, WindowsExplorer],
        Era::Win98 => &[InternetExplorer, OutlookExpress, ShowDesktop, WindowsExplorer],
        Era::Win2000 => &[ShowDesktop, InternetExplorer, OutlookExpress, WindowsExplorer],
    }
}

/// `builtin`'s picture in `era` (True Color; `colors16` finds its 16-color drawing), `None` where
/// that Windows had none.
pub fn picture(era: Era, builtin: Builtin) -> Option<Icon> {
    use Builtin::*;
    let art = match (era, builtin) {
        (_, WindowsExplorer) => return Some(crate::folder::tree_art::explorer(era)),
        (Era::Win95, ShowDesktop) => return Some(crate::folder::tree_art::w95(crate::folder::tree_art::W95::Desktop)),
        (Era::Win95, InternetExplorer) => &W95_INTERNET,
        (Era::Win95, OutlookExpress) => return None,
        (Era::Win98, InternetExplorer) => &W98_INTERNET,
        (Era::Win98, OutlookExpress) => &W98_OUTLOOK,
        (Era::Win98, ShowDesktop) => &W98_SHOW_DESKTOP,
        (Era::Win2000, InternetExplorer) => &W2K_INTERNET,
        (Era::Win2000, OutlookExpress) => &W2K_OUTLOOK,
        (Era::Win2000, ShowDesktop) => &W2K_SHOW_DESKTOP,
    };
    Some(art.icon())
}

/// Whether Quick Launch shows in `era` while settings.txt does not say (`quick_launch`): on in
/// 98 and 2000 (every capture of them has it), off in 95 (plain Windows 95 had none).
pub fn on_by_default(era: Era) -> bool {
    era != Era::Win95
}

/// At most this many icons show; more are left out (the log says how many). Dragging the band
/// wider, the chevron and the arrows for icons that do not fit are not built.
pub const MAX_ICONS: usize = 10;

/// One icon on the band.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    Builtin(Builtin),
    /// The person's own shortcut: an index into the list of their files (`shown_files`).
    Shortcut(usize),
}

/// The band's icons, left to right: `era`'s built-in icons (those not taken off, `removed`, and
/// those `available` says can be started: no mail program, no Outlook Express), then the person's
/// `shortcuts` (already in their order). At most `MAX_ICONS`; the second value is how many were
/// left out.
pub fn items(era: Era, removed: &[Builtin], available: impl Fn(Builtin) -> bool, shortcuts: usize) -> (Vec<Item>, usize) {
    let all: Vec<Item> = builtins(era)
        .iter()
        .copied()
        .filter(|b| !removed.contains(b) && available(*b))
        .map(Item::Builtin)
        .chain((0..shortcuts).map(Item::Shortcut))
        .collect();
    let left_out = all.len().saturating_sub(MAX_ICONS);
    (all.into_iter().take(MAX_ICONS).collect(), left_out)
}

/// One entry of Windows' own Quick Launch folder, as the platform read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderEntry {
    pub name: String,
    pub hidden: bool,
    pub folder: bool,
}

/// The two shortcuts today's Windows keeps in the Quick Launch folder for its own taskbar
/// ("Shows Desktop.lnk" and "Window Switcher.lnk": today's pictures, not classic ones; Show Desktop
/// is on the band already).
const WINDOWS_OWN: [&str; 2] = ["Shows Desktop.lnk", "Window Switcher.lnk"];

/// Which entries of the folder show on the band, A to Z by name (as Windows compares file names):
/// the files directly in it, without Windows' own two, `desktop.ini`, hidden files and folders.
pub fn shown_files(entries: &[FolderEntry]) -> Vec<String> {
    let mut names: Vec<String> = entries
        .iter()
        .filter(|e| !e.folder && !e.hidden)
        .filter(|e| !e.name.eq_ignore_ascii_case("desktop.ini"))
        .filter(|e| !WINDOWS_OWN.iter().any(|own| crate::text::same_name(own, &e.name)))
        .map(|e| e.name.clone())
        .collect();
    names.sort_by_key(|n| n.chars().flat_map(char::to_uppercase).collect::<String>());
    names
}

/// What a shortcut's tooltip says: its name without the extension (as Windows' band showed the
/// file names: "Launch Internet Explorer Browser").
pub fn shortcut_tooltip(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && (1..=4).contains(&ext.len()) => stem,
        _ => name,
    }
}

/// The first line of quick-launch.txt.
pub const STATE_HEAD: &str = "# Bevel95: the built-in Quick Launch icons taken off the band (one per line).";

/// The built-in icons taken off the band, read from quick-launch.txt (lines it does not know are
/// skipped).
pub fn parse_removed(text: &str) -> Vec<Builtin> {
    let mut removed = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some(b) = Builtin::from_key(line)
            && !removed.contains(&b)
        {
            removed.push(b);
        }
    }
    removed
}

/// quick-launch.txt for `removed`.
pub fn removed_text(removed: &[Builtin]) -> String {
    let mut text = format!("{STATE_HEAD}\r\n");
    for b in Builtin::ALL.iter().filter(|b| removed.contains(b)) {
        text.push_str(b.key());
        text.push_str("\r\n");
    }
    text
}

/// What the band shows (in `paint::Bar`): its icons' pictures (`None`: a shortcut without one
/// yet, its button empty), the icon under the pointer (raised) and the one held down (sunken).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Band {
    pub icons: Vec<Option<Icon>>,
    pub hot: Option<usize>,
    pub pressed: Option<usize>,
}

/// Where the band's parts are on the bar (1x pixels), measured on the Reference PCs' captures
/// (docs/theme-spec.md, "Quick Launch"): an etched line after the Start button, a grip, the icons'
/// buttons, a second etched line and grip, then the task buttons; on Windows 98's bar also an
/// etched line before the notification area.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The band from its first line to where the task buttons begin (the band's own part: its
    /// menu opens on it).
    pub rect: Rect,
    /// The etched line after the Start button (its dark column) and the grip's left column.
    pub line: i32,
    pub grip: i32,
    /// One 23x22 button per icon, side by side.
    pub buttons: Vec<Rect>,
    /// The etched line and grip in front of the task buttons.
    pub end_line: i32,
    pub end_grip: i32,
    /// Where the task buttons may go: from `rect`'s right edge to here.
    pub tasks_right: i32,
    /// Windows 98's etched line before the notification area (the 95 look takes 98's rule);
    /// Windows 2000 has none.
    pub tray_line: Option<i32>,
}

impl Layout {
    /// The icon under (x, y).
    pub fn button_at(&self, x: i32, y: i32) -> Option<usize> {
        self.buttons.iter().position(|r| r.contains(x, y))
    }
}

/// From the Start button's right edge to the band's first etched line: 98 (and the 95 look) 2,
/// 2000 3 (its Start button is one wider; the line is at 58 and 60 on both PCs).
fn line_after_start(era: Era) -> i32 {
    if era == Era::Win2000 { 3 } else { 2 }
}
/// The grip from the line; the first button from the line; a button's width (the icons' step);
/// the icon in its button; the line after the last button; the task buttons after that line.
const GRIP_FROM_LINE: i32 = 4;
const FIRST_BUTTON_FROM_LINE: i32 = 11;
pub const BUTTON_W: i32 = 23;
pub const ICON_IN: i32 = 3;
const END_LINE_AFTER_BUTTONS: i32 = 4;
const TASKS_FROM_END_LINE: i32 = 11;
/// The grip: 3 wide, 2 rows below the row's top, 18 tall.
pub const GRIP_W: i32 = 3;
pub const GRIP_DY: i32 = 2;
pub const GRIP_H: i32 = 18;

/// The band for `icons` icons on a bar whose Start button is `start` and whose notification area
/// is `well`; `y`, `h`: the row of buttons (the bar's `item_y`, `item_h`).
pub fn layout(era: Era, start: Rect, well: Rect, icons: usize, y: i32, h: i32) -> Layout {
    let line = start.right() + line_after_start(era);
    let first = line + FIRST_BUTTON_FROM_LINE;
    let buttons: Vec<Rect> = (0..icons).map(|i| Rect::new(first + i as i32 * BUTTON_W, y, BUTTON_W, h)).collect();
    let end_line = first + icons as i32 * BUTTON_W + END_LINE_AFTER_BUTTONS;
    let tasks_left = end_line + TASKS_FROM_END_LINE;
    // The task area ends 7 before the well on 98 (its line 4 before it), 6 before it on 2000.
    let (tasks_right, tray_line) = if era == Era::Win2000 { (well.x - 6, None) } else { (well.x - 7, Some(well.x - 4)) };
    Layout {
        rect: Rect::new(line, y, tasks_left - line, h),
        line,
        grip: line + GRIP_FROM_LINE,
        buttons,
        end_line,
        end_grip: end_line + GRIP_FROM_LINE,
        tasks_right,
        tray_line,
    }
}

/// An entry of the band's own menu (a right-click on its grips, lines and gaps).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BandEntry {
    View,
    ShowText,
    Refresh,
    Open,
    ShowTitle,
    Toolbars,
    AdjustDateTime,
    Cascade,
    TileHorizontally,
    TileVertically,
    MinimizeAll,
    TaskManager,
    Properties,
    Close,
}

/// The band's menu as the Reference PCs show it (98: `win98-truecolor-ql-bandmenu-grip`; 2000:
/// `-bandmenu-grip`, which adds Adjust Date/Time after Toolbars and Task Manager... before
/// Properties). `None`: a line.
fn band_entries(era: Era) -> Vec<Option<(&'static str, BandEntry)>> {
    use BandEntry::*;
    let mut e = vec![
        Some(("&View", View)),
        Some(("Show Te&xt", ShowText)),
        Some(("&Refresh", Refresh)),
        Some(("&Open", Open)),
        None,
        Some(("Sho&w Title", ShowTitle)),
        None,
        Some(("&Toolbars", Toolbars)),
        None,
    ];
    if era == Era::Win2000 {
        e.push(Some(("&Adjust Date/Time", AdjustDateTime)));
    }
    e.extend([
        Some(("Ca&scade Windows", Cascade)),
        Some(("Tile Windows &Horizontally", TileHorizontally)),
        Some(("Tile Windows &Vertically", TileVertically)),
        None,
        Some(("&Minimize All Windows", MinimizeAll)),
        None,
    ]);
    if era == Era::Win2000 {
        e.extend([Some(("Tas&k Manager...", TaskManager)), None]);
    }
    e.extend([Some(("P&roperties", Properties)), None, Some(("&Close", Close))]);
    e
}

impl BandEntry {
    /// What Bevel95 carries out: Refresh, Open, the Toolbars submenu, Adjust Date/Time, Properties
    /// and Close. The others (the band's View, Show Text, Show Title; the window arrangements and
    /// Task Manager, which Bevel95's bar menu leaves out) are shown grayed.
    pub fn works(self) -> bool {
        use BandEntry::*;
        matches!(self, Refresh | Open | Toolbars | AdjustDateTime | Properties | Close)
    }
}

/// The band's menu in `era`, nothing highlighted.
pub fn band_menu(era: Era) -> Menu {
    let entries = band_entries(era)
        .into_iter()
        .map(|e| match e {
            None => MenuEntry::separator(),
            Some((label, entry)) => {
                let mut m = if matches!(entry, BandEntry::View | BandEntry::Toolbars) {
                    MenuEntry::submenu(label, None)
                } else {
                    MenuEntry::item(label, None)
                };
                m.disabled = !entry.works();
                // Windows 2000 draws its Adjust Date/Time bold (the default of the clock's menu).
                m.bold = entry == BandEntry::AdjustDateTime;
                m
            }
        })
        .collect();
    Menu::new(entries)
}

/// What entry `index` of `band_menu(era)` is.
pub fn band_entry(era: Era, index: usize) -> Option<BandEntry> {
    band_entries(era).get(index).copied().flatten().map(|(_, e)| e)
}

/// An entry of the menu on an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconEntry {
    Open,
    Delete,
    Properties,
}

/// The menu on an icon (first pass): Open (bold, the default), Delete, and for the person's
/// shortcut its Properties (Windows' own window). The originals' other entries (Quick View, Send
/// To, Cut, Copy, Create Shortcut; 2000's Rename) are left out for now. `None`: a line.
fn icon_entries(shortcut: bool) -> Vec<Option<(&'static str, IconEntry)>> {
    let mut e = vec![Some(("&Open", IconEntry::Open)), None, Some(("&Delete", IconEntry::Delete))];
    if shortcut {
        e.extend([None, Some(("P&roperties", IconEntry::Properties))]);
    }
    e
}

/// The menu on an icon (`shortcut`: one of the person's own), nothing highlighted.
pub fn icon_menu(shortcut: bool) -> Menu {
    let entries = icon_entries(shortcut)
        .into_iter()
        .map(|e| match e {
            None => MenuEntry::separator(),
            Some((label, entry)) => MenuEntry { bold: entry == IconEntry::Open, ..MenuEntry::item(label, None) },
        })
        .collect();
    Menu::new(entries)
}

/// What entry `index` of `icon_menu(shortcut)` is.
pub fn icon_entry(shortcut: bool, index: usize) -> Option<IconEntry> {
    icon_entries(shortcut).get(index).copied().flatten().map(|(_, e)| e)
}

/// Windows 98's and 2000's bar with Quick Launch switched off (the Reference PCs'
/// `<pc>-truecolor-ql-off-<n>`): the etched line after the Start button and one grip stay (now the
/// task band's own), the task buttons start 7 after the grip (L+11: x 69 on 98, 71 on 2000), the
/// task area ends as with the band (7 before the well on 98, its line 4 before it; 6 on 2000).
/// Windows 95 had no bands: `None`, and its bar stays as it always was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskBand {
    pub line: i32,
    pub grip: i32,
    /// Where the task buttons go: from `left` to `right`.
    pub left: i32,
    pub right: i32,
    pub tray_line: Option<i32>,
}

/// The task band of `era`'s bar without Quick Launch (see `TaskBand`).
pub fn task_band(era: Era, start: Rect, well: Rect) -> Option<TaskBand> {
    if era == Era::Win95 {
        return None;
    }
    // The same rules as with the band (`layout`), with no band between the line and the tasks.
    let band = layout(era, start, well, 0, 0, 0);
    Some(TaskBand { line: band.line, grip: band.grip, left: band.line + FIRST_BUTTON_FROM_LINE, right: band.tasks_right, tray_line: band.tray_line })
}

static W95_INTERNET: ClassicIcon = ClassicIcon::art("quick-launch/internet-explorer", Look::Win95);
static W98_INTERNET: ClassicIcon = ClassicIcon::art("quick-launch/internet-explorer", Look::Win98);
static W98_OUTLOOK: ClassicIcon = ClassicIcon::art("quick-launch/outlook-express", Look::Win98);
static W98_SHOW_DESKTOP: ClassicIcon = ClassicIcon::art("quick-launch/show-desktop", Look::Win98);
static W2K_INTERNET: ClassicIcon = ClassicIcon::art("quick-launch/internet-explorer", Look::Win2000);
static W2K_OUTLOOK: ClassicIcon = ClassicIcon::art("quick-launch/outlook-express", Look::Win2000);
static W2K_SHOW_DESKTOP: ClassicIcon = ClassicIcon::art("quick-launch/show-desktop", Look::Win2000);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Depth, theme_in};

    #[test]
    fn every_icon_of_a_look_has_its_pictures() {
        for era in Era::ALL {
            for &b in builtins(era) {
                let icon = picture(era, b).unwrap_or_else(|| panic!("{era:?} {b:?}"));
                assert_eq!((icon.width, icon.height), (16, 16), "{era:?} {b:?}");
                assert!(icon.pixels.iter().any(|p| p >> 24 == 0), "{era:?} {b:?} has see-through pixels");
                // 98 and 2000 have their own 16-color drawing of each of their pictures.
                if era != Era::Win95 {
                    let sixteen = crate::colors16_art::own(era, &icon);
                    assert!(sixteen.is_some(), "{era:?} {b:?} has its 16-color drawing");
                    let _ = theme_in(era, Depth::Sixteen);
                }
            }
        }
        assert!(picture(Era::Win95, Builtin::OutlookExpress).is_none(), "95's files have no mail picture");
    }

    /// The Reference PCs' bars (800 wide, the bar's row from y 4): Windows 98's default band of four
    /// icons (`win98-truecolor-tasks-1`: line 58/59, grip 62..64, icons at 72, 95, 118, 141, line
    /// 165/166, grip 169..171, the first task button at 176, the task area 517 wide with the well at
    /// 700, its line at 696), Windows 2000's of three (line 60, grip 64, icons 74, 97, 120, line
    /// 144, grip 148, tasks from 155, 566 wide with the well at 727), and the hot and pressed button
    /// of `win98-truecolor-ql-hover-ie-at-once` / `-pressed-ie` (x 69..91, y 576..597).
    #[test]
    fn the_band_sits_where_the_reference_pcs_have_it() {
        let b = layout(Era::Win98, Rect::new(2, 4, 54, 22), Rect::new(700, 4, 98, 22), 4, 4, 22);
        assert_eq!((b.line, b.grip, b.end_line, b.end_grip, b.rect.right()), (58, 62, 165, 169, 176));
        let icons: Vec<i32> = b.buttons.iter().map(|r| r.x + ICON_IN).collect();
        assert_eq!(icons, [72, 95, 118, 141]);
        assert_eq!(b.buttons[0], Rect::new(69, 4, 23, 22));
        assert_eq!((b.tasks_right - b.rect.right(), b.tray_line), (517, Some(696)));
        let b = layout(Era::Win2000, Rect::new(2, 4, 55, 22), Rect::new(727, 4, 71, 22), 3, 4, 22);
        assert_eq!((b.line, b.grip, b.end_line, b.end_grip, b.rect.right()), (60, 64, 144, 148, 155));
        let icons: Vec<i32> = b.buttons.iter().map(|r| r.x + ICON_IN).collect();
        assert_eq!(icons, [74, 97, 120]);
        assert_eq!((b.tasks_right - b.rect.right(), b.tray_line), (566, None));
        // The 95 look by 98's rule.
        assert_eq!(layout(Era::Win95, Rect::new(2, 4, 54, 22), Rect::new(700, 4, 98, 22), 4, 4, 22).line, 58);
        assert_eq!(b.button_at(97, 10), Some(1));
        assert_eq!(b.button_at(70, 10), None, "the gap before the first button");
    }

    #[test]
    fn the_items_are_the_looks_own_then_the_persons() {
        use Builtin::*;
        let all = |_: Builtin| true;
        let (items, left_out) = items(Era::Win2000, &[], all, 2);
        assert_eq!(items, [Item::Builtin(ShowDesktop), Item::Builtin(InternetExplorer), Item::Builtin(OutlookExpress), Item::Builtin(WindowsExplorer), Item::Shortcut(0), Item::Shortcut(1)]);
        assert_eq!(left_out, 0);
        // Taken off, or no mail program: left out.
        let (items, _) = super::items(Era::Win98, &[InternetExplorer], |b| b != OutlookExpress, 0);
        assert_eq!(items, [Item::Builtin(ShowDesktop), Item::Builtin(WindowsExplorer)]);
        // At most ten.
        let (items, left_out) = super::items(Era::Win98, &[], all, 9);
        assert_eq!((items.len(), left_out), (MAX_ICONS, 3));
        assert_eq!(items[MAX_ICONS - 1], Item::Shortcut(5));
    }

    #[test]
    fn the_folder_shows_the_persons_files_a_to_z() {
        let e = |name: &str, hidden: bool, folder: bool| FolderEntry { name: name.into(), hidden, folder };
        let entries = [
            e("zebra.lnk", false, false),
            e("desktop.ini", true, false),
            e("Desktop.ini", false, false),
            e("Shows Desktop.lnk", false, false),
            e("window switcher.LNK", false, false),
            e("User Pinned", false, true),
            e("secret.lnk", true, false),
            e("apple.lnk", false, false),
            e("Banana.url", false, false),
        ];
        assert_eq!(shown_files(&entries), ["apple.lnk", "Banana.url", "zebra.lnk"]);
        assert_eq!(shortcut_tooltip("Banana.url"), "Banana");
        assert_eq!(shortcut_tooltip("no extension"), "no extension");
    }

    #[test]
    fn the_icons_taken_off_are_remembered() {
        use Builtin::*;
        let text = removed_text(&[WindowsExplorer, InternetExplorer]);
        assert!(text.starts_with(STATE_HEAD));
        assert_eq!(parse_removed(&text), [InternetExplorer, WindowsExplorer]);
        assert_eq!(parse_removed("show-desktop\r\nnonsense\r\nshow-desktop # twice\r\n"), [ShowDesktop]);
        assert_eq!(parse_removed(""), []);
    }

    #[test]
    fn the_menus_are_the_originals_with_what_is_built_working() {
        let labels = |m: &Menu| m.entries.iter().map(|e| if e.label.is_empty() { "-".to_string() } else { e.label.clone() }).collect::<Vec<_>>();
        let m98 = band_menu(Era::Win98);
        assert_eq!(
            labels(&m98),
            ["&View", "Show Te&xt", "&Refresh", "&Open", "-", "Sho&w Title", "-", "&Toolbars", "-", "Ca&scade Windows", "Tile Windows &Horizontally", "Tile Windows &Vertically", "-", "&Minimize All Windows", "-", "P&roperties", "-", "&Close"]
        );
        let working: Vec<_> = (0..m98.entries.len()).filter(|&i| m98.entries[i].choosable()).filter_map(|i| band_entry(Era::Win98, i)).collect();
        use BandEntry::*;
        assert_eq!(working, [Refresh, Open, Toolbars, Properties, Close]);
        let m2k = band_menu(Era::Win2000);
        assert_eq!(band_entry(Era::Win2000, 9), Some(AdjustDateTime));
        assert!(m2k.entries[9].bold);
        assert!(labels(&m2k).contains(&"Tas&k Manager...".to_string()));
        assert_eq!(labels(&band_menu(Era::Win95)), labels(&m98), "the 95 look takes 98's");
        assert_eq!(labels(&icon_menu(false)), ["&Open", "-", "&Delete"]);
        assert_eq!(labels(&icon_menu(true)), ["&Open", "-", "&Delete", "-", "P&roperties"]);
        assert!(icon_menu(true).entries[0].bold);
        assert_eq!((icon_entry(true, 2), icon_entry(true, 4), icon_entry(false, 1)), (Some(IconEntry::Delete), Some(IconEntry::Properties), None));
    }

    #[test]
    fn each_look_keeps_its_own_order() {
        use Builtin::*;
        assert_eq!(builtins(Era::Win98), [InternetExplorer, OutlookExpress, ShowDesktop, WindowsExplorer]);
        assert_eq!(builtins(Era::Win2000), [ShowDesktop, InternetExplorer, OutlookExpress, WindowsExplorer]);
        for b in Builtin::ALL {
            assert_eq!(Builtin::from_key(b.key()), Some(b));
        }
    }
}

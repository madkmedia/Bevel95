//! The classic pictures Bevel95 shows on task buttons instead of some of today's windows' own
//! icons, as the owner asked: Notepad's, and Explorer's for its windows (My Documents for the
//! Documents folder, My Computer for This PC and Home, the Recycle Bin's, Control Panel's, and
//! the folder window's for every other folder, Downloads included). Explorer's folder windows
//! also get their classic names (`classic_title`).
//!
//! Cut from the reference PCs' taskbars (real Windows 95, 98 and 2000, True Color; see
//! docs/theme-spec.md, "Classic icons for today's windows"): each window opened, its button
//! captured raised and pressed, the see-through pixels proven by the pressed button's
//! checkerboard. Windows 95 had no My Documents: there it is the folder window's picture.
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/taskbar.rs`); this
//! module chooses and draws them.

use crate::art::Look;
use crate::fb::Icon;
use crate::icons::ClassicIcon;
use crate::theme::Era;

/// Which classic picture a window gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassicWindow {
    Notepad,
    /// A folder window, for every folder without a picture of its own.
    Folder,
    MyDocuments,
    MyComputer,
    RecycleBin,
    ControlPanel,
}

impl ClassicWindow {
    /// The picture of a button holding several windows of this kind: Explorer's show a folder.
    pub fn for_group(self) -> ClassicWindow {
        match self {
            ClassicWindow::Notepad => ClassicWindow::Notepad,
            _ => ClassicWindow::Folder,
        }
    }
}

/// The names this Windows shows for the places that have a picture of their own (they depend
/// on its language and on where the folders are), as the platform layer read them: display
/// names, and the Documents folder's full path too (for Explorer's "full path in the title
/// bar" option).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Places {
    pub documents: Vec<String>,
    /// This PC, and Windows 11's Home (what Win+E opens, as My Computer was).
    pub my_computer: Vec<String>,
    pub recycle_bin: Vec<String>,
    pub control_panel: Vec<String>,
}

/// What today's Explorer adds to its folder windows' titles ("Documents - File Explorer").
/// Its Control Panel windows have none ("Programs and Features").
const EXPLORER_SUFFIX: &str = " - File Explorer";

/// The classic picture for a window of program `exe` (file name only), window class `class`
/// and title `title`, if Bevel95 has one for it.
pub fn classify(exe: &str, class: &str, title: &str, places: &Places) -> Option<ClassicWindow> {
    if exe.eq_ignore_ascii_case("notepad.exe") {
        return Some(ClassicWindow::Notepad);
    }
    // Explorer's other windows (the Run box, copy progress, ...) keep their own icons.
    if !exe.eq_ignore_ascii_case("explorer.exe") || class != "CabinetWClass" {
        return None;
    }
    let Some(place) = title.strip_suffix(EXPLORER_SUFFIX) else {
        return Some(ClassicWindow::ControlPanel);
    };
    let named = |names: &[String]| names.iter().any(|n| n.eq_ignore_ascii_case(place));
    Some(if named(&places.documents) {
        ClassicWindow::MyDocuments
    } else if named(&places.my_computer) {
        ClassicWindow::MyComputer
    } else if named(&places.recycle_bin) {
        ClassicWindow::RecycleBin
    } else if named(&places.control_panel) {
        ClassicWindow::ControlPanel
    } else {
        ClassicWindow::Folder
    })
}

/// The title Bevel95 shows for a window (same arguments as `classify`): Explorer's folder
/// windows without today's " - File Explorer", and with the classic names where the place had
/// one (My Computer for This PC and Home, as Win+E opened My Computer; My Documents; Control
/// Panel for "All Control Panel Items"). A full path (Explorer's "Display the full path in the
/// title bar") stays a path. Every other window keeps its title.
pub fn classic_title(exe: &str, class: &str, title: &str, places: &Places) -> String {
    if !exe.eq_ignore_ascii_case("explorer.exe") || class != "CabinetWClass" {
        return title.to_string();
    }
    let Some(place) = title.strip_suffix(EXPLORER_SUFFIX) else {
        // Control Panel opened as control.exe has no suffix ("Control Panel", or "All Control
        // Panel Items" in its icon view); opened as a folder it has one.
        let control_panel = places.control_panel.iter().any(|n| n.eq_ignore_ascii_case(title));
        return if control_panel { "Control Panel".to_string() } else { title.to_string() };
    };
    if place.contains('\\') {
        return place.to_string();
    }
    match classify(exe, class, title, places) {
        Some(ClassicWindow::MyComputer) => "My Computer".to_string(),
        Some(ClassicWindow::MyDocuments) => "My Documents".to_string(),
        Some(ClassicWindow::ControlPanel) => "Control Panel".to_string(),
        _ => place.to_string(),
    }
}

/// The 16×16 picture of `kind` as `era` drew it on a task button.
pub fn icon(era: Era, kind: ClassicWindow) -> Icon {
    use ClassicWindow::*;
    let picture = match (era, kind) {
        (Era::Win95, Notepad) => &WIN95_NOTEPAD,
        // Windows 95 had no My Documents picture (Office's My Documents was a plain folder).
        // The owner wants the 95 look kept pure: no borrowing 98's.
        (Era::Win95, Folder | MyDocuments) => &WIN95_FOLDER_WINDOW,
        (Era::Win95, MyComputer) => &WIN95_MY_COMPUTER,
        (Era::Win95, RecycleBin) => &WIN95_RECYCLE_BIN,
        (Era::Win95, ControlPanel) => &WIN95_CONTROL_PANEL,
        // Windows 2000's Notepad is Windows 98's, pixel for pixel.
        (Era::Win98 | Era::Win2000, Notepad) => &WIN98_NOTEPAD,
        (Era::Win98, Folder) => &WIN98_FOLDER_WINDOW,
        (Era::Win98, MyDocuments) => &WIN98_MY_DOCUMENTS,
        (Era::Win98, MyComputer) => &WIN98_MY_COMPUTER,
        (Era::Win98, RecycleBin) => &WIN98_RECYCLE_BIN,
        (Era::Win98, ControlPanel) => &WIN98_CONTROL_PANEL,
        (Era::Win2000, Folder) => &WIN2000_FOLDER_WINDOW,
        (Era::Win2000, MyDocuments) => &WIN2000_MY_DOCUMENTS,
        (Era::Win2000, MyComputer) => &WIN2000_MY_COMPUTER,
        (Era::Win2000, RecycleBin) => &WIN2000_RECYCLE_BIN,
        (Era::Win2000, ControlPanel) => &WIN2000_CONTROL_PANEL,
    };
    picture.icon()
}

static WIN95_MY_COMPUTER: ClassicIcon = ClassicIcon::art("task/my-computer", Look::Win95);

static WIN95_RECYCLE_BIN: ClassicIcon = ClassicIcon::art("task/recycle-bin", Look::Win95);

static WIN95_NOTEPAD: ClassicIcon = ClassicIcon::art("task/notepad", Look::Win95);

static WIN95_CONTROL_PANEL: ClassicIcon = ClassicIcon::art("task/control-panel", Look::Win95);

static WIN95_FOLDER_WINDOW: ClassicIcon = ClassicIcon::art("task/folder-window", Look::Win95);

static WIN98_MY_DOCUMENTS: ClassicIcon = ClassicIcon::art("task/my-documents", Look::Win98);

static WIN98_MY_COMPUTER: ClassicIcon = ClassicIcon::art("task/my-computer", Look::Win98);

static WIN98_RECYCLE_BIN: ClassicIcon = ClassicIcon::art("task/recycle-bin", Look::Win98);

static WIN98_NOTEPAD: ClassicIcon = ClassicIcon::art("task/notepad", Look::Win98);

static WIN98_CONTROL_PANEL: ClassicIcon = ClassicIcon::art("task/control-panel", Look::Win98);

static WIN98_FOLDER_WINDOW: ClassicIcon = ClassicIcon::art("task/folder-window", Look::Win98);

static WIN2000_MY_COMPUTER: ClassicIcon = ClassicIcon::art("task/my-computer", Look::Win2000);

static WIN2000_MY_DOCUMENTS: ClassicIcon = ClassicIcon::art("task/my-documents", Look::Win2000);

static WIN2000_RECYCLE_BIN: ClassicIcon = ClassicIcon::art("task/recycle-bin", Look::Win2000);

static WIN2000_CONTROL_PANEL: ClassicIcon = ClassicIcon::art("task/control-panel", Look::Win2000);

static WIN2000_FOLDER_WINDOW: ClassicIcon = ClassicIcon::art("task/folder-window", Look::Win2000);

#[cfg(test)]
mod tests {
    use super::ClassicWindow::*;
    use super::*;

    fn places() -> Places {
        Places {
            documents: vec!["Documents".into(), r"C:\Users\someone\OneDrive\Documents".into()],
            my_computer: vec!["This PC".into(), "Home".into()],
            recycle_bin: vec!["Recycle Bin".into()],
            control_panel: vec!["All Control Panel Items".into(), "Control Panel".into()],
        }
    }

    #[test]
    fn notepad_and_explorer_windows_get_classic_pictures() {
        let p = places();
        let explorer = |title: &str| classify("explorer.exe", "CabinetWClass", title, &p);
        assert_eq!(classify("Notepad.exe", "Notepad", "*Untitled - Notepad", &p), Some(Notepad));
        assert_eq!(explorer("Documents - File Explorer"), Some(MyDocuments));
        assert_eq!(explorer(r"C:\Users\someone\OneDrive\Documents - File Explorer"), Some(MyDocuments));
        assert_eq!(explorer("This PC - File Explorer"), Some(MyComputer));
        assert_eq!(explorer("Home - File Explorer"), Some(MyComputer));
        assert_eq!(explorer("Recycle Bin - File Explorer"), Some(RecycleBin));
        assert_eq!(explorer("Downloads - File Explorer"), Some(Folder));
        assert_eq!(explorer("tmp - File Explorer"), Some(Folder));
        // Control Panel's pages carry no " - File Explorer".
        assert_eq!(explorer("Programs and Features"), Some(ControlPanel));
        // Explorer's other windows, and other programs, keep their own icons.
        assert_eq!(classify("explorer.exe", "#32770", "Run", &p), None);
        assert_eq!(classify("chrome.exe", "Chrome_WidgetWin_1", "Documents - File Explorer", &p), None);
    }

    #[test]
    fn explorer_windows_get_classic_names() {
        let p = places();
        let name = |title: &str| classic_title("explorer.exe", "CabinetWClass", title, &p);
        assert_eq!(name("This PC - File Explorer"), "My Computer");
        assert_eq!(name("Home - File Explorer"), "My Computer");
        assert_eq!(name("Documents - File Explorer"), "My Documents");
        assert_eq!(name("All Control Panel Items - File Explorer"), "Control Panel");
        assert_eq!(name("Recycle Bin - File Explorer"), "Recycle Bin");
        assert_eq!(name("Downloads - File Explorer"), "Downloads");
        assert_eq!(name("Photos - 2024 - File Explorer"), "Photos - 2024");
        // A full path stays a path, and Control Panel's pages keep their titles.
        assert_eq!(name(r"C:\Users\someone\OneDrive\Documents - File Explorer"), r"C:\Users\someone\OneDrive\Documents");
        assert_eq!(name("Programs and Features"), "Programs and Features");
        assert_eq!(name("All Control Panel Items"), "Control Panel");
        // Only Explorer's folder windows: Notepad and other programs keep their titles.
        assert_eq!(classic_title("notepad.exe", "Notepad", "Untitled - Notepad", &p), "Untitled - Notepad");
        assert_eq!(classic_title("chrome.exe", "Chrome_WidgetWin_1", "Home - File Explorer", &p), "Home - File Explorer");
    }

    #[test]
    fn a_button_of_several_explorer_windows_shows_a_folder() {
        assert_eq!(MyDocuments.for_group(), Folder);
        assert_eq!(ControlPanel.for_group(), Folder);
        assert_eq!(Notepad.for_group(), Notepad);
    }

    #[test]
    fn every_picture_is_16_square_and_windows_95_has_no_my_documents() {
        for era in [Era::Win95, Era::Win98, Era::Win2000] {
            for kind in [Notepad, Folder, MyDocuments, MyComputer, RecycleBin, ControlPanel] {
                let i = icon(era, kind);
                assert_eq!((i.width, i.height, i.pixels.len()), (16, 16, 256), "{era:?} {kind:?}");
            }
        }
        assert_eq!(icon(Era::Win95, MyDocuments), icon(Era::Win95, Folder));
        assert_eq!(icon(Era::Win2000, Notepad), icon(Era::Win98, Notepad));
    }
}

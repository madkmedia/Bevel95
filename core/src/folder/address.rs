//! The address bar of Windows 98's and 2000's folder windows: what it says for each place (and
//! what the title bar says), editing what is typed into it, and the places its list holds
//! (measured on the Reference PCs: `*-truecolor-folder-c-after-dblclick`, `-address-list`,
//! `-winnt-after-dblclick`, `-desktop-view`).
//!
//! - The address: "My Computer", "Desktop"; in 98 a folder's path ("C:\WINDOWS", "C:\"), in 2000
//!   its name as the title bar shows it ("WINNT", "Local Disk (C:)"; 2000 showed the full path
//!   only with a Folder Options setting that was off at first).
//! - The list: the Desktop, My Computer and its items (one level in), the places down to the one
//!   shown (under its drive), then the Desktop's other items: 2000 puts My Documents first, 98
//!   after My Computer; the Recycle Bin and the folders on the desktop. Each level is 10 pixels
//!   further in.

use super::history::Place;
use crate::theme::Era;

/// What the address bar says for `place` (`name`: the place's name as the title bar shows it).
pub fn wording(era: Era, place: &Place, name: &str) -> String {
    match place {
        Place::Desktop => "Desktop".into(),
        Place::MyComputer => "My Computer".into(),
        Place::Folder(_) if era == Era::Win2000 && !name.trim().is_empty() => name.trim().to_string(),
        Place::Folder(p) => p.clone(),
    }
}

/// What was typed, read as a place: "My Computer" and "Desktop" (any case), "C:" for a drive's
/// root, and a drive's name as the list shows it; anything else is a path (`None`: empty). A
/// relative name ("Logs", "..\Other", "\Windows") means one in `shown`, the folder the window
/// shows (the second safety review of 2026-11-18, C2: it was looked up in the program's working
/// folder, Bevel95's own); without a folder shown it stays relative, and the caller never takes a
/// relative path for a folder. Anything with a colon (an address, `shell:`) is left as typed.
pub fn typed_place(text: &str, drives: &[(String, String)], shown: Option<&str>) -> Option<Place> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if t.eq_ignore_ascii_case("my computer") {
        return Some(Place::MyComputer);
    }
    if t.eq_ignore_ascii_case("desktop") {
        return Some(Place::Desktop);
    }
    if let Some((root, _)) = drives.iter().find(|(_, name)| crate::text::same_name(name.trim(), t)) {
        return Some(Place::Folder(root.clone()));
    }
    let b = t.as_bytes();
    if b.len() == 2 && b[1] == b':' {
        return Some(Place::Folder(format!("{t}\\")));
    }
    let relative = !t.contains(':') && !t.starts_with("\\\\") && !t.starts_with("//");
    if relative && let Some(dir) = shown {
        return Some(Place::Folder(join_relative(dir, t)));
    }
    let path = if t.len() > 3 { t.trim_end_matches('\\') } else { t };
    Some(Place::Folder(path.to_string()))
}

/// `rel` (a relative name, or one from the drive's root: "\Windows") in the folder `dir`, with
/// "." and ".." taken away as Windows does (never above the root).
fn join_relative(dir: &str, rel: &str) -> String {
    let dir = dir.trim_end_matches(['\\', '/']);
    // The root: "C:" of "C:\Users", "\\server\share" of a network folder.
    let root_len = if let Some(unc) = dir.strip_prefix("\\\\") {
        let mut slashes = unc.match_indices('\\').map(|(i, _)| i);
        slashes.nth(1).map_or(dir.len(), |i| i + 2)
    } else if dir.get(1..2) == Some(":") {
        2
    } else {
        0
    };
    let (root, rest) = dir.split_at(root_len);
    let mut parts: Vec<&str> = if rel.starts_with(['\\', '/']) { Vec::new() } else { rest.split('\\').filter(|s| !s.is_empty()).collect() };
    for part in rel.split(['\\', '/']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            name => parts.push(name),
        }
    }
    format!("{root}\\{}", parts.join("\\"))
}

/// The text field: one line of editable text (`edit::Edit`, brief 95: the text, the caret, the
/// selection, the scroll, Undo; reached through `Deref`), and what the address bar adds to it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Field {
    pub edit: crate::edit::Edit,
    /// The field has the keyboard.
    pub focused: bool,
    /// Something was typed since the place was shown (Esc gives the place's address back).
    pub edited: bool,
    pub caret_on: bool,
    /// The address of the place shown (Esc gives it back).
    pub place_text: String,
    /// The text of the last Cut or Copy, for the clipboard (`take_copy`).
    pub copied: Option<String>,
}

impl std::ops::Deref for Field {
    type Target = crate::edit::Edit;
    fn deref(&self) -> &crate::edit::Edit {
        &self.edit
    }
}

impl std::ops::DerefMut for Field {
    fn deref_mut(&mut self) -> &mut crate::edit::Edit {
        &mut self.edit
    }
}

/// The keys the field acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKey {
    Char(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    SelectAll,
}

impl Field {
    /// The place shown changed: its address, nothing typed (no Undo back to what was typed).
    pub fn show(&mut self, text: &str) {
        self.edit.set(text);
        self.place_text = text.to_string();
        self.edited = false;
        if self.focused {
            self.edit.select_all();
        } else {
            self.edit.place(0);
        }
    }

    /// The place shown is the same, its address is better known now (a late reading): what Esc
    /// gives back changes; the text shown only while nothing was typed (a typed address stays).
    pub fn place_known(&mut self, text: &str) {
        if self.place_text == text {
            return;
        }
        if self.edited {
            self.place_text = text.to_string();
        } else {
            self.show(text);
        }
    }

    /// The field takes the keyboard: everything selected (98's `address-clicked`).
    pub fn focus(&mut self) {
        self.focused = true;
        self.edit.select_all();
        self.caret_on = true;
    }

    pub fn blur(&mut self) {
        self.focused = false;
        let c = self.edit.caret;
        self.edit.place(c);
    }

    /// A key (`shift`: the selection grows with the caret keys).
    pub fn key(&mut self, key: EditKey, shift: bool) {
        use crate::edit::Op;
        self.caret_on = true;
        let op = match key {
            EditKey::Char(ch) => {
                if self.edit.type_char(ch) {
                    self.edited = true;
                }
                return;
            }
            EditKey::Backspace => Op::Backspace,
            EditKey::Delete => Op::Delete,
            EditKey::Left => Op::Left,
            EditKey::Right => Op::Right,
            EditKey::Home => Op::Home,
            EditKey::End => Op::End,
            EditKey::SelectAll => Op::SelectAll,
        };
        if self.edit.apply(op, shift) == crate::edit::Done::Changed {
            self.edited = true;
        }
    }

    /// A key of Windows' edit box (`kit::field_key`: caret keys, words, Cut, Copy, Paste, Undo,
    /// Ctrl+A, Shift+F10). `None`: not the field's.
    pub fn edit_key<P>(&mut self, key: crate::kit::Key, mods: crate::kit::Mods) -> Option<Vec<crate::kit::Effect<P>>> {
        self.caret_on = true;
        let done = crate::kit::field_key(&mut self.edit, key, mods, &mut self.copied)?;
        if done.changed {
            self.edited = true;
        }
        Some(done.effects)
    }

    /// Entry `index` of the field's right-click menu chosen.
    pub fn menu_chosen<P>(&mut self, index: usize) -> Vec<crate::kit::Effect<P>> {
        self.caret_on = true;
        let done = crate::kit::field_menu_chosen(&mut self.edit, index, &mut self.copied);
        if done.changed {
            self.edited = true;
        }
        done.effects
    }

    /// Text pasted in (Ctrl+V): the first line only (`edit::Edit::paste`).
    pub fn paste(&mut self, text: &str) {
        if self.edit.paste(text) {
            self.edited = true;
        }
    }

    /// How far the text is moved left to keep the caret inside a box `width` pixels wide (0 while
    /// the field has no keyboard: the start of the address shows).
    pub fn fit(&self, font: &crate::text::Font, width: i32) -> i32 {
        if !self.focused {
            self.edit.scroll.set(0);
            return 0;
        }
        self.edit.fit(&|s| font.measure(s), width)
    }

    /// The character nearest `x` pixels into the text (as shown: `x` counts from the text's first
    /// pixel in the box, the scroll is added here).
    pub fn index_at(&self, font: &crate::text::Font, x: i32) -> usize {
        self.edit.index_at(&|s| font.measure(s), x)
    }

    /// The caret goes to the character nearest `x` (Shift: the selection grows to there).
    pub fn click(&mut self, font: &crate::text::Font, x: i32, shift: bool) {
        let at = self.index_at(font, x);
        self.edit.press(at, shift);
        self.caret_on = true;
    }
}

/// What the address list is built from: My Computer's items (id, name, picture), the place shown,
/// and the folders on the desktop (path, name).
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub computer: Vec<(String, String, super::art::Small)>,
    pub desktop_folders: Vec<(String, String)>,
}

/// One entry of the list: where it goes, how far in, its picture and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub place: Option<Place>,
    /// An item that is not a place of a folder window (Control Panel, Printers, the Recycle Bin):
    /// opened as Windows' own, by this id.
    pub open_id: Option<String>,
    pub level: i32,
    pub small: super::art::Small,
    pub name: String,
}

/// The list's entries for a window showing `now` (`names`: a folder's name by its path).
pub fn entries(era: Era, tree: &Tree, now: &Place, names: &dyn Fn(&str) -> String) -> Vec<Entry> {
    use super::art::Small;
    let mut out = vec![Entry { place: Some(Place::Desktop), open_id: None, level: 0, small: Small::Desktop, name: "Desktop".into() }];
    let my_docs = Entry { place: None, open_id: Some("::my-documents".into()), level: 1, small: Small::MyDocuments, name: "My Documents".into() };
    if era == Era::Win2000 {
        out.push(my_docs.clone());
    }
    out.push(Entry { place: Some(Place::MyComputer), open_id: None, level: 1, small: Small::MyComputer, name: "My Computer".into() });
    let chain = now.chain();
    for (id, name, small) in &tree.computer {
        let is_place = matches!(small, Small::Drive(_));
        out.push(Entry {
            place: is_place.then(|| Place::Folder(id.clone())),
            open_id: (!is_place).then(|| id.clone()),
            level: 2,
            small: *small,
            name: name.clone(),
        });
        // The places down to the one shown, under its drive.
        if chain.get(1) == Some(&Place::Folder(id.clone())) {
            for (k, p) in chain.iter().enumerate().skip(2) {
                if let Place::Folder(path) = p {
                    out.push(Entry { place: Some(p.clone()), open_id: None, level: 1 + k as i32, small: Small::ClosedFolder, name: names(path) });
                }
            }
        }
    }
    if era != Era::Win2000 {
        out.push(my_docs);
    }
    out.push(Entry { place: None, open_id: Some("::recycle-bin".into()), level: 1, small: Small::RecycleBin, name: "Recycle Bin".into() });
    for (path, name) in &tree.desktop_folders {
        out.push(Entry { place: Some(Place::Folder(path.clone())), open_id: None, level: 1, small: Small::ClosedFolder, name: name.clone() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::art::{DriveKind, Small};
    use super::*;

    #[test]
    fn the_address_says_what_each_windows_said() {
        let c = Place::Folder("C:\\".into());
        assert_eq!(wording(Era::Win98, &c, "(C:)"), "C:\\");
        assert_eq!(wording(Era::Win2000, &c, "Local Disk (C:)"), "Local Disk (C:)");
        assert_eq!(wording(Era::Win98, &Place::MyComputer, ""), "My Computer");
        assert_eq!(wording(Era::Win2000, &Place::Desktop, ""), "Desktop");
        assert_eq!(wording(Era::Win2000, &Place::Folder("C:\\WINNT".into()), "WINNT"), "WINNT");
    }

    #[test]
    fn typed_addresses_become_places() {
        let drives = vec![("C:\\".to_string(), "Local Disk (C:)".to_string())];
        for shown in [None, Some("D:\\Projects")] {
            assert_eq!(typed_place("my computer", &drives, shown), Some(Place::MyComputer));
            assert_eq!(typed_place(" Desktop ", &drives, shown), Some(Place::Desktop));
            assert_eq!(typed_place("local disk (c:)", &drives, shown), Some(Place::Folder("C:\\".into())));
            assert_eq!(typed_place("d:", &drives, shown), Some(Place::Folder("d:\\".into())));
            assert_eq!(typed_place("C:\\Windows\\", &drives, shown), Some(Place::Folder("C:\\Windows".into())));
            assert_eq!(typed_place("\\\\server\\share\\x", &drives, shown), Some(Place::Folder("\\\\server\\share\\x".into())));
            assert_eq!(typed_place("https://example.com/a", &drives, shown), Some(Place::Folder("https://example.com/a".into())));
            assert_eq!(typed_place("  ", &drives, shown), None);
        }
    }

    /// A relative name means one in the folder shown (the second safety review of 2026-11-18,
    /// C2): before, "programs" in a window showing D:\Projects opened the program's working
    /// folder's "programs", Bevel95's own.
    #[test]
    fn a_relative_name_is_looked_up_in_the_folder_shown() {
        let f = |t: &str, shown: &str| typed_place(t, &[], Some(shown));
        let folder = |p: &str| Some(Place::Folder(p.into()));
        assert_eq!(f("programs", "D:\\Projects"), folder("D:\\Projects\\programs"));
        assert_eq!(f("Logs\\2026\\", "D:\\Projects\\"), folder("D:\\Projects\\Logs\\2026"));
        assert_eq!(f("settings.txt", "C:\\"), folder("C:\\settings.txt"));
        assert_eq!(f("..", "D:\\Projects\\Old"), folder("D:\\Projects"));
        assert_eq!(f("..\\..\\..", "D:\\Projects"), folder("D:\\"), "never above the root");
        assert_eq!(f(".\\a/b", "D:\\x"), folder("D:\\x\\a\\b"));
        assert_eq!(f("\\Windows", "C:\\Users\\someone"), folder("C:\\Windows"), "from the drive's root");
        assert_eq!(f("docs", "\\\\server\\share\\team"), folder("\\\\server\\share\\team\\docs"));
        assert_eq!(f("..", "\\\\server\\share"), folder("\\\\server\\share\\"));
        // Without a folder shown (My Computer): left relative, for the caller to refuse.
        assert_eq!(typed_place("programs", &[], None), folder("programs"));
    }

    #[test]
    fn the_field_edits_as_an_edit_box() {
        let mut f = Field::default();
        f.show("C:\\");
        f.focus();
        assert_eq!(f.selection(), (0, 3), "everything selected");
        f.key(EditKey::Char('d'), false);
        assert_eq!((f.text.as_str(), f.caret), ("d", 1));
        f.paste(":\\x\nmore");
        assert_eq!(f.text, "d:\\x");
        f.key(EditKey::Backspace, false);
        f.key(EditKey::Home, false);
        f.key(EditKey::Right, true);
        assert_eq!(f.selection(), (0, 1));
        f.key(EditKey::Delete, false);
        assert_eq!(f.text, ":\\");
        assert!(f.edited);
        f.show("My Computer");
        assert!(!f.edited);
    }

    #[test]
    fn a_long_address_keeps_the_caret_in_sight() {
        let font = crate::text::Font::parse(include_str!("../../tests/fixtures/ms-sans-serif-11px.font")).unwrap();
        let mut f = Field::default();
        f.show("C:\\");
        assert_eq!(f.fit(&font, 60), 0, "without the keyboard: the start shows");
        f.focus();
        f.key(EditKey::End, false);
        for ch in "\\a rather long folder name typed in".chars() {
            f.key(EditKey::Char(ch), false);
        }
        let total = font.measure(&f.text);
        let s = f.fit(&font, 60);
        assert_eq!(s, total - 59, "the caret at the box's right edge");
        // A step left keeps the text where it is; Home brings the start back.
        f.key(EditKey::Left, false);
        assert_eq!(f.fit(&font, 60), s);
        f.key(EditKey::Home, false);
        assert_eq!(f.fit(&font, 60), 0);
        // A click counts from what shows.
        f.key(EditKey::End, false);
        f.fit(&font, 60);
        f.click(&font, 59, false);
        assert_eq!(f.caret, f.text.chars().count());
        f.show("My Computer");
        assert_eq!(f.scroll.get(), 0);
    }

    #[test]
    fn the_list_holds_the_desktop_my_computer_and_the_way_down() {
        let tree = Tree {
            computer: vec![
                ("A:\\".into(), "3\u{bd} Floppy (A:)".into(), Small::Drive(DriveKind::Floppy)),
                ("C:\\".into(), "(C:)".into(), Small::Drive(DriveKind::Fixed)),
                ("::printers".into(), "Printers".into(), Small::Printers),
            ],
            desktop_folders: vec![("C:\\Users\\x\\Desktop\\Online Services".into(), "Online Services".into())],
        };
        let now = Place::Folder("C:\\WINDOWS\\SYSTEM".into());
        let names = |p: &str| p.rsplit('\\').next().unwrap_or("").to_string();
        let e = entries(Era::Win98, &tree, &now, &names);
        let shown: Vec<(i32, &str)> = e.iter().map(|e| (e.level, e.name.as_str())).collect();
        assert_eq!(
            shown,
            [
                (0, "Desktop"),
                (1, "My Computer"),
                (2, "3\u{bd} Floppy (A:)"),
                (2, "(C:)"),
                (3, "WINDOWS"),
                (4, "SYSTEM"),
                (2, "Printers"),
                (1, "My Documents"),
                (1, "Recycle Bin"),
                (1, "Online Services")
            ]
        );
        assert_eq!(e[2].place, Some(Place::Folder("A:\\".into())));
        assert_eq!(e[6].open_id.as_deref(), Some("::printers"));
        let e = entries(Era::Win2000, &tree, &Place::MyComputer, &names);
        assert_eq!(e[1].name, "My Documents", "2000: My Documents before My Computer");
    }
}

//! Where a Windows 98 or 2000 folder window has been: the places Back and Forward go to, what Up
//! goes to, and when each is grayed (measured on the Reference PCs: `win98-truecolor-folder-*`,
//! `win2000-truecolor-folder-*`).
//!
//! - A new place (a double-click, Up, Backspace, the address bar, the Go menu) goes after the one
//!   shown and forgets everything Forward held (98's `mc-after-backspace`: Forward grayed again).
//! - Back and Forward step through the list without changing it; Back is grayed at its start,
//!   Forward at its end. Their little arrows list the places that way, the nearest first
//!   (`back-menu`: "(C:)", "My Computer" from Windows).
//! - Up: a folder's parent, a drive's is My Computer; My Computer's is the Desktop in Windows 2000
//!   (`mc-after-up`), nothing in 98 (Up grayed there); the Desktop has none.

use crate::theme::Era;

/// A place a folder window shows.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// The desktop as a folder: the look's own icons, then what the Desktop folders hold.
    Desktop,
    MyComputer,
    /// A folder or a drive's root ("C:\"), by its path.
    Folder(String),
}

impl Place {
    /// A drive's root ("C:\"): its parent is My Computer.
    pub fn is_drive(&self) -> bool {
        matches!(self, Place::Folder(p) if is_root(p))
    }

    /// The place one level up, as Up and Backspace go (`None`: Up is grayed).
    pub fn parent(&self, era: Era) -> Option<Place> {
        match self {
            Place::Desktop => None,
            Place::MyComputer => (era == Era::Win2000).then_some(Place::Desktop),
            Place::Folder(p) if is_root(p) => Some(Place::MyComputer),
            Place::Folder(p) => {
                let trimmed = p.trim_end_matches('\\');
                let cut = trimmed.rfind('\\')?;
                let parent = &trimmed[..cut];
                // "C:" is the drive's root: "C:\".
                Some(Place::Folder(if parent.len() == 2 && parent.ends_with(':') { format!("{parent}\\") } else { parent.to_string() }))
            }
        }
    }

    /// The places from the top down to this one (the Desktop left out): My Computer, the drive,
    /// each folder (Windows 98's File menu lists them, the address list opens them).
    pub fn chain(&self) -> Vec<Place> {
        let mut out = vec![self.clone()];
        let mut at = self.clone();
        while let Some(up) = at.parent(Era::Win98) {
            out.push(up.clone());
            at = up;
        }
        out.reverse();
        out
    }
}

/// "C:\" (or "C:"): a drive's root.
pub fn is_root(path: &str) -> bool {
    let b = path.as_bytes();
    (b.len() == 3 && b[1] == b':' && b[2] == b'\\') || (b.len() == 2 && b[1] == b':')
}

/// How many places Back's and Forward's lists show at most.
pub const LIST_MOST: usize = 9;
/// How many places are kept each way (older ones are forgotten).
const KEEP: usize = 100;

/// The places a window has shown, and which one it shows now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    back: Vec<Place>,
    now: Place,
    forward: Vec<Place>,
}

impl History {
    pub fn new(now: Place) -> History {
        History { back: Vec::new(), now, forward: Vec::new() }
    }

    pub fn now(&self) -> &Place {
        &self.now
    }

    pub fn can_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// A new place: the one shown goes to Back's list, Forward's is forgotten. Going to the place
    /// shown changes nothing.
    pub fn go(&mut self, place: Place) {
        if place == self.now {
            return;
        }
        let was = std::mem::replace(&mut self.now, place);
        self.back.push(was);
        if self.back.len() > KEEP {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    /// `steps` places back (1: Back); `None` if there are not that many.
    pub fn back(&mut self, steps: usize) -> Option<Place> {
        if steps == 0 || steps > self.back.len() {
            return None;
        }
        for _ in 0..steps {
            let p = self.back.pop()?;
            let was = std::mem::replace(&mut self.now, p);
            self.forward.push(was);
        }
        Some(self.now.clone())
    }

    /// `steps` places forward (1: Forward).
    pub fn forward(&mut self, steps: usize) -> Option<Place> {
        if steps == 0 || steps > self.forward.len() {
            return None;
        }
        for _ in 0..steps {
            let p = self.forward.pop()?;
            let was = std::mem::replace(&mut self.now, p);
            self.back.push(was);
        }
        Some(self.now.clone())
    }

    /// Back's list, the nearest first (at most `LIST_MOST`).
    pub fn back_list(&self) -> Vec<Place> {
        self.back.iter().rev().take(LIST_MOST).cloned().collect()
    }

    /// Forward's list, the nearest first.
    pub fn forward_list(&self) -> Vec<Place> {
        self.forward.iter().rev().take(LIST_MOST).cloned().collect()
    }

    /// The place shown was renamed or went away: the window shows `place` instead, nothing else
    /// changes.
    pub fn replace_now(&mut self, place: Place) {
        self.now = place;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(p: &str) -> Place {
        Place::Folder(p.to_string())
    }

    #[test]
    fn back_and_forward_as_the_reference_pcs_went() {
        // 98: My Computer, double-click (C:), double-click Windows; Back's list "(C:)", "My
        // Computer"; Alt+Left: (C:) with Forward lit; Backspace: My Computer, Forward grayed.
        let mut h = History::new(Place::MyComputer);
        assert!(!h.can_back() && !h.can_forward());
        h.go(f("C:\\"));
        h.go(f("C:\\WINDOWS"));
        assert_eq!(h.back_list(), [f("C:\\"), Place::MyComputer]);
        assert_eq!(h.back(1), Some(f("C:\\")));
        assert!(h.can_back() && h.can_forward());
        assert_eq!(h.forward_list(), [f("C:\\WINDOWS")]);
        h.go(f("C:\\").parent(Era::Win98).unwrap());
        assert_eq!(h.now(), &Place::MyComputer);
        assert!(!h.can_forward(), "a new place forgets Forward's list");
        assert_eq!(h.back_list(), [f("C:\\"), Place::MyComputer]);
        assert_eq!(h.back(2), Some(Place::MyComputer));
        assert_eq!(h.forward(2), Some(Place::MyComputer));
        assert_eq!(h.back(5), None);
        h.go(Place::MyComputer);
        assert_eq!(h.back_list().len(), 2, "going to the place shown changes nothing");
    }

    #[test]
    fn up_goes_one_level_as_each_windows_did() {
        assert_eq!(f("C:\\WINDOWS\\SYSTEM").parent(Era::Win98), Some(f("C:\\WINDOWS")));
        assert_eq!(f("C:\\WINDOWS").parent(Era::Win98), Some(f("C:\\")));
        assert_eq!(f("C:\\").parent(Era::Win98), Some(Place::MyComputer));
        assert_eq!(Place::MyComputer.parent(Era::Win98), None, "98: Up grayed in My Computer");
        assert_eq!(Place::MyComputer.parent(Era::Win2000), Some(Place::Desktop));
        assert_eq!(Place::Desktop.parent(Era::Win2000), None);
        assert_eq!(f("C:\\WINDOWS").chain(), [Place::MyComputer, f("C:\\"), f("C:\\WINDOWS")]);
        assert!(f("D:\\").is_drive() && !f("D:\\x").is_drive());
    }
}

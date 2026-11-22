//! Which files and folders a listing shows: Windows' own two switches decide (File Explorer's
//! "Hidden items", and Folder Options' "Hide protected operating system files"), until Bevel95
//! has each look's own Folder Options page (GitHub issue 4). Everything that lists a folder goes
//! through `shown`: the folder windows, the tree of folders, the classic desktop's items.

/// A file's attributes as Windows keeps them (FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM).
pub const HIDDEN: u32 = 0x2;
pub const SYSTEM: u32 = 0x4;

/// Windows' two switches, as the person set them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Switches {
    /// File Explorer > View > Show > Hidden items.
    pub show_hidden: bool,
    /// Folder Options: "Hide protected operating system files" switched off.
    pub show_protected: bool,
}

impl Switches {
    /// Nothing hidden shows (Windows' defaults, and what is used when the switches cannot be read).
    pub const NONE: Switches = Switches { show_hidden: false, show_protected: false };
}

/// Whether an item with `attributes` shows:
/// - neither hidden nor a system file: always;
/// - hidden, not a system file: when Windows shows hidden items;
/// - hidden and a system file (desktop.ini and the like): only when Windows shows hidden items
///   and does not hide protected operating system files;
/// - a system file that is not hidden: never, as before (File Explorer shows such a file).
pub fn shown(attributes: u32, switches: Switches) -> bool {
    match (attributes & HIDDEN != 0, attributes & SYSTEM != 0) {
        (false, false) => true,
        (true, false) => switches.show_hidden,
        (true, true) => switches.show_hidden && switches.show_protected,
        (false, true) => false,
    }
}

/// Whether a shown item's picture is drawn faded (as a cut item's: the old Windows drew hidden
/// files so; PROVISIONAL, not measured on the Reference PCs): it is hidden.
pub fn faded(attributes: u32) -> bool {
    attributes & HIDDEN != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_switches_decide() {
        const PLAIN: u32 = 0x20; // archive: no matter
        let none = Switches::NONE;
        let hidden_only = Switches { show_hidden: true, show_protected: false };
        let both = Switches { show_hidden: true, show_protected: true };
        let protected_only = Switches { show_hidden: false, show_protected: true };
        // (attributes, shown with: none, hidden items, both, protected only)
        let table = [
            (0, [true, true, true, true]),
            (PLAIN, [true, true, true, true]),
            (HIDDEN, [false, true, true, false]),
            (HIDDEN | PLAIN, [false, true, true, false]),
            (HIDDEN | SYSTEM, [false, false, true, false]),
            (SYSTEM, [false, false, false, false]),
            (SYSTEM | 0x10, [false, false, false, false]),
            (HIDDEN | 0x10, [false, true, true, false]),
        ];
        for (attributes, want) in table {
            let got = [none, hidden_only, both, protected_only].map(|s| shown(attributes, s));
            assert_eq!(got, want, "attributes {attributes:#x}");
        }
        assert!(faded(HIDDEN) && faded(HIDDEN | SYSTEM) && !faded(SYSTEM) && !faded(PLAIN));
    }
}

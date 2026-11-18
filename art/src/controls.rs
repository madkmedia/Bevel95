//! The small pictures of Windows' controls (used by `core::window`, `core::dialog` and
//! `core::disk_box`): the title bar buttons' glyphs ("#" ink, as Windows draws them from its Marlett
//! font, measured on the reference captures), Windows 95's 12×12 option buttons (their letters are
//! the colors the core names) and the system's error icon of the disk boxes.

use crate::{Art, Depth, Entry, Look, Rows};

/// The glyphs of the title bar buttons ("#" = a pixel), drawn 2px in from the button's corner.
#[rustfmt::skip]
const HELP_GLYPH: Rows = Rows { w: 12, h: 9, palette: &[], rows: &[
    "....####....",
    "...##..##...",
    "...##..##...",
    "......##....",
    ".....##.....",
    ".....##.....",
    "............",
    ".....##.....",
    ".....##.....",
] };

#[rustfmt::skip]
const CLOSE_GLYPH: Rows = Rows { w: 12, h: 8, palette: &[], rows: &[
    "............",
    "..##....##..",
    "...##..##...",
    "....####....",
    ".....##.....",
    "....####....",
    "...##..##...",
    "..##....##..",
] };

#[rustfmt::skip]
const MINIMIZE_GLYPH: Rows = Rows { w: 12, h: 9, palette: &[], rows: &["", "", "", "", "", "", "", "..######....", "..######...."
] };

#[rustfmt::skip]
const MAXIMIZE_GLYPH: Rows = Rows { w: 12, h: 9, palette: &[], rows: &[
    ".#########..",
    ".#########..",
    ".#.......#..",
    ".#.......#..",
    ".#.......#..",
    ".#.......#..",
    ".#.......#..",
    ".#.......#..",
    ".#########..",
] };

/// Windows 95's 12×12 option buttons, checked and not.
#[rustfmt::skip]
const OPTION_ON: Rows = Rows { w: 12, h: 12, palette: &[], rows: &[
    "....gggg....",
    "..gg####gg..",
    ".g##WWWW##W.",
    ".g#WWWWWWlW.",
    "g#WWW##WWWlW",
    "g#WW####WWlW",
    "g#WW####WWlW",
    "g#WWW##WWWlW",
    ".g#WWWWWWlW.",
    ".gllWWWWllW.",
    "..WWllllWW..",
    "....WWWW....",
] };

#[rustfmt::skip]
const OPTION_OFF: Rows = Rows { w: 12, h: 12, palette: &[], rows: &[
    "....gggg....",
    "..gg####gg..",
    ".g##WWWW##W.",
    ".g#WWWWWWlW.",
    "g#WWWWWWWWlW",
    "g#WWWWWWWWlW",
    "g#WWWWWWWWlW",
    "g#WWWWWWWWlW",
    ".g#WWWWWWlW.",
    ".gllWWWWllW.",
    "..WWllllWW..",
    "....WWWW....",
] };

/// Windows 95's and 98's error picture (USER's hand: the red circle with the white cross), cut
#[rustfmt::skip]
const ERROR: Rows = Rows {
    w: 32,
    h: 32,
    palette: &[(b'a', 0x800000), (b'b', 0xFF0000), (b'c', 0x808080), (b'd', 0xFFFFFF)],
    rows: &[
        "...........aaaaaaaa.............",
        "........aaabbbbbbbbaaa..........",
        ".......abbbbbbbbbbbbbba.........",
        ".....aabbbbbbbbbbbbbbbbaa.......",
        "....abbbbbbbbbbbbbbbbbbbba......",
        "...abbbbbbbbbbbbbbbbbbbbbba.....",
        "...abbbbbbbbbbbbbbbbbbbbbbac....",
        "..abbbbbbdbbbbbbbbbbdbbbbbbac...",
        ".abbbbbbdddbbbbbbbbdddbbbbbba...",
        ".abbbbbdddddbbbbbbdddddbbbbbac..",
        ".abbbbbbdddddbbbbdddddbbbbbbacc.",
        "abbbbbbbbdddddbbdddddbbbbbbbbac.",
        "abbbbbbbbbddddddddddbbbbbbbbbac.",
        "abbbbbbbbbbddddddddbbbbbbbbbbacc",
        "abbbbbbbbbbbddddddbbbbbbbbbbbacc",
        "abbbbbbbbbbbddddddbbbbbbbbbbbacc",
        "abbbbbbbbbbddddddddbbbbbbbbbbacc",
        "abbbbbbbbbddddddddddbbbbbbbbbacc",
        "abbbbbbbbdddddbbdddddbbbbbbbbacc",
        ".abbbbbbdddddbbbbdddddbbbbbbaccc",
        ".abbbbbdddddbbbbbbdddddbbbbbaccc",
        ".abbbbbbdddbbbbbbbbdddbbbbbbacc.",
        "..abbbbbbdbbbbbbbbbbdbbbbbbaccc.",
        "...abbbbbbbbbbbbbbbbbbbbbbacccc.",
        "...abbbbbbbbbbbbbbbbbbbbbbaccc..",
        "....abbbbbbbbbbbbbbbbbbbbaccc...",
        ".....aabbbbbbbbbbbbbbbbaacccc...",
        "......cabbbbbbbbbbbbbbaccccc....",
        ".......caaabbbbbbbbaaaccccc.....",
        ".........ccaaaaaaaacccccc.......",
        "..........cccccccccccccc........",
        ".............cccccccc...........",
    ],
};

// The classic menu check mark (7×7) and option bullet (6×6: Calculator's View menu on the reference
// PCs, 98 and 2000), "#" ink.
#[rustfmt::skip]
const MENU_CHECK: Rows = Rows { w: 7, h: 7, palette: &[], rows: &[
    "......#", ".....##", "#...###", "##.###.", "#####..", ".###...", "..#....",
] };

#[rustfmt::skip]
const MENU_BULLET: Rows = Rows { w: 6, h: 6, palette: &[], rows: &[
    ".####.", "######", "######", "######", "######", ".####.",
] };

// The window menu's pictures in Windows 98, "#" ink: first read in the v86 emulator; since
// 2026-11-18 cut from the Reference PC's Calculator window menu (`vm/win98-truecolor-calc-sysmenu-item.png`:
// Restore and Maximize grayed, Minimize and Close black, each at its measured place) and found
// identical.
#[rustfmt::skip]
const MENU_RESTORE: Rows = Rows { w: 10, h: 9, palette: &[], rows: &[
    "..########",
    "..########",
    "..#......#",
    "########.#",
    "########.#",
    "#......###",
    "#......#..",
    "#......#..",
    "########..",
] };

#[rustfmt::skip]
const MENU_MINIMIZE: Rows = Rows { w: 7, h: 2, palette: &[], rows: &["#######", "#######"] };

#[rustfmt::skip]
const MENU_MAXIMIZE: Rows = Rows { w: 10, h: 10, palette: &[], rows: &[
    "##########",
    "##########",
    "#........#",
    "#........#",
    "#........#",
    "#........#",
    "#........#",
    "#........#",
    "#........#",
    "##########",
] };

#[rustfmt::skip]
const MENU_CLOSE: Rows = Rows { w: 9, h: 9, palette: &[], rows: &[
    "##.....##",
    "###...###",
    ".###.###.",
    "..#####..",
    "...###...",
    "..#####..",
    ".###.###.",
    "###...###",
    "##.....##",
] };

/// Windows 2000 gives these dialogs the system's default small flag icon.
#[rustfmt::skip]
const DIALOG_ICON: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'K', 0), (b'R', 0xFF0000), (b'M', 0x800000), (b'G', 0x00FF00),
               (b'B', 0x0000FF), (b'N', 0x000080), (b'Y', 0xFFFF00)],
    rows: &[
        "................", "................", ".........KKKK...", "K.......KKKKKKK.",
        ".K.KK.K.KRRKGGK.", "R...K...KRRKGGK.", ".R.RR.R.KMKKKGK.", "K...R...KKKKKKK.",
        ".K.KK.K.KBBKYYK.", "B...K...KBBKYYK.", ".B.BB.B.KNKKKYK.", "K...B...KKKKKKK.",
        ".K.KK.K.K.....K.", "....K...........", "................", "................",
    ],
};

#[rustfmt::skip]
pub(crate) const ENTRIES: &[Entry] = &[
    Entry { what: "window/help-glyph", looks: crate::EVERY_LOOK, depth: Depth::TrueColor, art: Art::Rows(&HELP_GLYPH) },
    Entry { what: "window/close-glyph", looks: crate::EVERY_LOOK, depth: Depth::TrueColor, art: Art::Rows(&CLOSE_GLYPH) },
    Entry { what: "window/minimize-glyph", looks: crate::EVERY_LOOK, depth: Depth::TrueColor, art: Art::Rows(&MINIMIZE_GLYPH) },
    Entry { what: "window/maximize-glyph", looks: crate::EVERY_LOOK, depth: Depth::TrueColor, art: Art::Rows(&MAXIMIZE_GLYPH) },
    Entry { what: "dialog/option-on", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&OPTION_ON) },
    Entry { what: "dialog/option-off", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&OPTION_OFF) },
    Entry { what: "message-box/error", looks: crate::EVERY_LOOK, depth: Depth::TrueColor, art: Art::Rows(&ERROR) },
    Entry { what: "menu/check", looks: crate::EVERY_LOOK, depth: Depth::TrueColor, art: Art::Rows(&MENU_CHECK) },
    Entry { what: "menu/bullet", looks: crate::EVERY_LOOK, depth: Depth::TrueColor, art: Art::Rows(&MENU_BULLET) },
    Entry { what: "menu/restore", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&MENU_RESTORE) },
    Entry { what: "menu/minimize", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&MENU_MINIMIZE) },
    Entry { what: "menu/maximize", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&MENU_MAXIMIZE) },
    Entry { what: "menu/close", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&MENU_CLOSE) },
    Entry { what: "dialog/flag", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&DIALOG_ICON) },
];

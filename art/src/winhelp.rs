//! Windows 95's WinHelp pictures (used by `core::help::winhelp`): the three Contents images from
//! WINHLP32.EXE bitmap 110 (a closed book, an open book, a page). The rows' letters are colors the
//! core names.

use crate::{Art, Depth, Entry, Look, Rows};

// The three Contents images from Windows 95 WINHLP32.EXE bitmap 110.
#[rustfmt::skip]
const BOOK: Rows = Rows { w: 20, h: 17, palette: &[], rows: &[
    "...........KK.......",
    ".........KKPPK......",
    ".......KKPPPPPK.....",
    ".....KKPPPPPPPPK....",
    "....KGPPPPPPPPPPK...",
    "....KPGPPPPPPPPPPK..",
    "....KPPGPPPPPPPPPPK.",
    "....KPPPGPPPPPPPPPKK",
    "....KPPPPGPPPPPPKKS.",
    ".....KPPPPGPPPKKS.S.",
    "......KPPPPGKKSG.GKK",
    ".......KPPPKS.G.GKK.",
    "........KPPK.G.KK...",
    ".........KPKSKK.....",
    "..........KKK.......",
    "....................",
    "....................",
] };

#[rustfmt::skip]
const OPEN_BOOK: Rows = Rows { w: 20, h: 17, palette: &[], rows: &[
    ".......S............",
    "......SGS...........",
    ".....KP.GS....SSS...",
    ".....KP..S..SS..S...",
    ".....KP..GSS...YSKK.",
    ".....KP...KG.Y..S.PK",
    ".....KP...KG...YS.PK",
    ".....KP...KG.Y..S.PK",
    ".....KP...KG...YS.PK",
    ".....KP...KG.Y..S.PK",
    ".....KPP..KG...YS.PK",
    "......KPP.KG.SKKK.PK",
    ".......KP.KKKSGG..PK",
    ".......KPKKPPPPPPPPK",
    ".......KPPPKKKKKKKKK",
    "........KKK.........",
    "....................",
] };

#[rustfmt::skip]
const PAGE: Rows = Rows { w: 20, h: 17, palette: &[], rows: &[
    "...SSSSSSSSSS.......",
    "...S...Y...YSS......",
    "...S.Y...Y..KKK.....",
    "...S...BBBBY.GK.....",
    "...S.YBB.YBB.GK.....",
    "...S..BB..BB.GK.....",
    "...S.Y...YBB.GK.....",
    "...S...Y.BBY.GK.....",
    "...S.Y..BB...GK.....",
    "...S...Y...Y.GK.....",
    "...S.Y..BB...GK.....",
    "...S...YBB.Y.GK.....",
    "...S.Y...Y...GK.....",
    "...SGGGGGGGGGGK.....",
    "...KKKKKKKKKKKK.....",
    "....................",
    "....................",
] };

// The WinHelp document icon in the title bar of the topic window.
#[rustfmt::skip]
const DOCUMENT: Rows = Rows { w: 16, h: 16, palette: &[
    (b'K', 0x000000), (b'G', 0x808080), (b'L', 0xC0C0C0), (b'W', 0xFFFFFF), (b'Y', 0xFFFF00), (b'B', 0x0000FF),
], rows: &[
    "..GGGGGGGGGG....",
    "..GWWWYWWWYGG...",
    "..GWYWWWYWWKKK..",
    "..GWWWYWWWYWLK..",
    "..GWYWBBBBWWLK..",
    "..GWWBBWWBBWLK..",
    "..GWYBBWYBBWLK..",
    "..GWWWYWWBBWLK..",
    "..GWYWWWBBWWLK..",
    "..GWWWYBBWYWLK..",
    "..GWYWWWYWWWLK..",
    "..GWWWYBBWYWLK..",
    "..GWYWWBBWWWLK..",
    "..GWWWYWWWYWLK..",
    "..GLLLLLLLLLLK..",
    "..KKKKKKKKKKKK..",
] };

#[rustfmt::skip]
pub(crate) const ENTRIES: &[Entry] = &[
    Entry { what: "winhelp/book", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&BOOK) },
    Entry { what: "winhelp/open-book", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&OPEN_BOOK) },
    Entry { what: "winhelp/page", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&PAGE) },
    Entry { what: "winhelp/document", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&DOCUMENT) },
];

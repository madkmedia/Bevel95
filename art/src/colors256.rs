//! The pictures Windows drew differently at 256 colors that are written as text (the others are
//! files in art/assets: web/*-256.pic, explorer/*-256.pic). Cut from the Reference PCs' 256-color
//! screens by private/probes/colors256/cut256.ps1.

use crate::{Art, Depth, Entry, Look, Rows};

// win2000: Minesweeper's title-bar picture (cut from vm/win2000-256color-mine-new.png at 85,84;
// see-through where the caption shows).
#[rustfmt::skip]
const W2K_MINESWEEPER_16: Rows = Rows { w: 16, h: 16, palette: &[(b'#', 0x000000), (b'a', 0x808080), (b'c', 0xC0C0C0), (b'b', 0xFFFFFF)], rows: &[
    "................",
    ".......##.......",
    "......####......",
    "..###abbcca###..",
    "..##cccccaa###..",
    "...accaaaaaa#...",
    "..#bcaa#aaaa##..",
    ".##caa#b#aa####.",
    ".##ccaa###a####.",
    "..#acaa#######..",
    "...#####a####...",
    "...##########...",
    "...##########...",
    "...##.####.##...",
    ".......##.......",
    "................",
] };

#[rustfmt::skip]
pub(crate) const ENTRIES: &[Entry] = &[
    Entry { what: "minesweeper/icon-16", looks: &[Look::Win2000], depth: Depth::TwoFiftySix, art: Art::Rows(&W2K_MINESWEEPER_16) },
];
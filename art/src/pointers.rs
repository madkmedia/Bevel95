//! Windows' mouse pointers that Bevel95 draws itself (used by `core::drag` and `core::kit`): 98's and
//! 2000's drag pointers (move, copy, link) and the "no" pointer, measured on the Reference PCs, and
//! Windows 95's split pointer of Explorer's tree. "#" black, "o" white (the outline), "." see-through.

use crate::{Art, Depth, Entry, Look, Rows};

#[rustfmt::skip]
const MOVE_98: Rows = Rows { w: 14, h: 19, palette: &[], rows: &[
    "#.............",
    "##............",
    "#o#...........",
    "#oo#..........",
    "#ooo#.........",
    "#oooo#........",
    "#ooooo#.......",
    "#oooooo#......",
    "#ooooooo#.....",
    "#oooooooo#....",
    "#ooooo#####...",
    "#oo#oo#.......",
    "#o#.#oo#......",
    "##..#oo#......",
    "#....#oo#.....",
    ".....#oo#.....",
    "......#oo#....",
    "......#oo#....",
    ".......##.....",
] };

#[rustfmt::skip]
const COPY_98: Rows = Rows { w: 26, h: 29, palette: &[], rows: &[
    "#.........................",
    "##........................",
    "#o#.......................",
    "#oo#......................",
    "#ooo#.....................",
    "#oooo#....................",
    "#ooooo#...................",
    "#oooooo#..................",
    "#ooooooo#.................",
    "#oooooooo#................",
    "#ooooo#####...............",
    "#oo#oo#...................",
    "#o#.#oo#..................",
    "##..#oo#..................",
    "#....#oo#.................",
    ".....#oo#.................",
    "......#oo#................",
    "......#oo#................",
    ".......##..aaaaaaaaaa#....",
    "...........aooooooooo#....",
    "...........aooooooooo#....",
    "...........aoooo#oooo#....",
    "...........aoooo#oooo#....",
    "...........aoo#####oo#....",
    "...........aoooo#oooo#....",
    "...........aoooo#oooo#....",
    "...........aooooooooo#....",
    "...........aooooooooo#....",
    "...........###########....",
] };

#[rustfmt::skip]
const LINK_98: Rows = Rows { w: 26, h: 29, palette: &[], rows: &[
    "#.........................",
    "##........................",
    "#o#.......................",
    "#oo#......................",
    "#ooo#.....................",
    "#oooo#....................",
    "#ooooo#...................",
    "#oooooo#..................",
    "#ooooooo#.................",
    "#oooooooo#................",
    "#ooooo#####...............",
    "#oo#oo#...................",
    "#o#.#oo#..................",
    "##..#oo#..................",
    "#....#oo#.................",
    ".....#oo#.................",
    "......#oo#................",
    "......#oo#................",
    ".......##..aaaaaaaaaa#....",
    "...........aooooooooo#....",
    "...........aooo####oo#....",
    "...........aoooo###oo#....",
    "...........aooo####oo#....",
    "...........aoo###o#oo#....",
    "...........aoo##ooooo#....",
    "...........aoo#oooooo#....",
    "...........aooo#ooooo#....",
    "...........aooooooooo#....",
    "...........###########....",
] };

#[rustfmt::skip]
const MOVE_2000: Rows = Rows { w: 14, h: 21, palette: &[], rows: &[
    "#.............",
    "##............",
    "#o#...........",
    "#oo#..........",
    "#ooo#.........",
    "#oooo#........",
    "#ooooo#.......",
    "#oooooo#......",
    "#ooooooo#.....",
    "#oooooooo#....",
    "#ooooooooo#...",
    "#oooooo#####..",
    "#ooo#oo#......",
    "#oo##oo#......",
    "#o#..#oo#.....",
    "##...#oo#.....",
    "#.....#oo#....",
    "......#oo#....",
    ".......#oo#...",
    ".......#oo#...",
    "........##....",
] };

#[rustfmt::skip]
const COPY_2000: Rows = Rows { w: 23, h: 22, palette: &[], rows: &[
    "#......................",
    "##.....................",
    "#o#....................",
    "#oo#...................",
    "#ooo#..................",
    "#oooo#.................",
    "#ooooo#................",
    "#oooooo#...............",
    "#ooooooo#..............",
    "#oooooooo#.............",
    "#ooooooooo#............",
    "#oooooo####aaaaaaaaaa#.",
    "#ooo#oo#...aooooooooo#.",
    "#oo##oo#...aooooooooo#.",
    "#o#..#oo#..aoooo#oooo#.",
    "##...#oo#..aoooo#oooo#.",
    "#.....#oo#.aoo#####oo#.",
    "......#oo#.aoooo#oooo#.",
    ".......#oo#aoooo#oooo#.",
    ".......#oo#aooooooooo#.",
    "........##.aooooooooo#.",
    "...........###########.",
] };

#[rustfmt::skip]
const LINK_2000: Rows = Rows { w: 23, h: 22, palette: &[], rows: &[
    "#......................",
    "##.....................",
    "#o#....................",
    "#oo#...................",
    "#ooo#..................",
    "#oooo#.................",
    "#ooooo#................",
    "#oooooo#...............",
    "#ooooooo#..............",
    "#oooooooo#.............",
    "#ooooooooo#............",
    "#oooooo####aaaaaaaaaa#.",
    "#ooo#oo#...aooooooooo#.",
    "#oo##oo#...aooo####oo#.",
    "#o#..#oo#..aoooo###oo#.",
    "##...#oo#..aooo####oo#.",
    "#.....#oo#.aoo###o#oo#.",
    "......#oo#.aoo##ooooo#.",
    ".......#oo#aoo#oooooo#.",
    ".......#oo#aooo#ooooo#.",
    "........##.aooooooooo#.",
    "...........###########.",
] };

/// The "no" sign: a black ring with a slash from top left to bottom right and a white outline
/// (2000's capture; 98's black pixels are the same). Its hot spot was not measured: the middle.
#[rustfmt::skip]
const NO: Rows = Rows { w: 20, h: 20, palette: &[], rows: &[
    ".......oooooo.......",
    ".....oo######oo.....",
    "....o##########o....",
    "...o####oooo####o...",
    "..o###oo....oo###o..",
    ".o#####o......o###o.",
    ".o##o###o......o##o.",
    "o###oo###o.....o###o",
    "o##o..o###o.....o##o",
    "o##o...o###o....o##o",
    "o##o....o###o...o##o",
    "o##o.....o###o..o##o",
    "o###o.....o###oo###o",
    ".o##o......o###o##o.",
    ".o###o......o#####o.",
    "..o###oo.....o###o..",
    "...o####ooooo###o...",
    "....o##########o....",
    ".....oo######oo.....",
    ".......oooooo.......",
] };

/// Windows 95 Explorer's split pointer, read off the 95 Reference PC with the pointer over the
/// bar between the panes (`win95-truecolor-m17-tree-splitter-pointer`): a 1-pixel bar 15 rows
/// high crossed by two arrows with 3-row shafts, the right one a pixel longer. No pointer of
/// today's Windows has this shape. Its outline could not be seen on the white tree behind it.
#[rustfmt::skip]
const SPLIT95: Rows = Rows { w: 22, h: 17, palette: &[], rows: &[
        "......................",
        "..........#...........",
        "..........#...........",
        "..........#...........",
        "..........#...........",
        "....#.....#......#....",
        "...##.....#......##...",
        "..#######.#.########..",
        ".########.#.#########.",
        "..#######.#.########..",
        "...##.....#......##...",
        "....#.....#......#....",
        "..........#...........",
        "..........#...........",
        "..........#...........",
        "..........#...........",
        "......................",
] };

#[rustfmt::skip]
pub(crate) const ENTRIES: &[Entry] = &[
    Entry { what: "drag/move", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&MOVE_98) },
    Entry { what: "drag/copy", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&COPY_98) },
    Entry { what: "drag/link", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&LINK_98) },
    Entry { what: "drag/move", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&MOVE_2000) },
    Entry { what: "drag/copy", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&COPY_2000) },
    Entry { what: "drag/link", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&LINK_2000) },
    Entry { what: "drag/no", looks: &[Look::Win98, Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&NO) },
    Entry { what: "pointer/split", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&SPLIT95) },
];

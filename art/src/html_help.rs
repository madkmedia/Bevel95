//! HTML Help's pictures (used by `core::html_help`): the viewer's small icon, its Contents images
//! and toolbar pictures. The color and monochrome toolbar strips are resources 513 and 512 in each
//! edition's HHCTRL.OCX (cells 18×16 in 98, 20×20 in 2000). The etched 2000 history arrows were
//! measured on the initially empty history. And Internet Explorer's document picture of 2000's
//! Find bar. The rows' letters are colors the core names.

use crate::{Art, Depth, Entry, Look, Rows};

#[rustfmt::skip]
const CAPTION: Rows = Rows { w: 16, h: 16, palette: &[], rows: &[
    "........ooooo...",
    ".......oYYYYLo..",
    "......oYYYYYYLS.",
    ".....oYYYooYYYK.",
    "SSSSSKYYLSSLYYK.",
    "SWWWLKKKoSKYYYK.",
    "SWWWWWWWnKYYYLK.",
    "SWWWWWSKoYYYLK..",
    "SWWSSSSWoYYLK...",
    "SWWWWWWWoSSSS...",
    "SWWSSSSWWK......",
    "SWWWWWWWoKKKo...",
    "SWWSSSSWKYYYK...",
    "SWWWWWWWKYYYK...",
    "SWWWWWWWoKKKo...",
    "SKKKKKKKKK......",
] };

#[rustfmt::skip]
const TREE: Rows = Rows { w: 64, h: 16, palette: &[], rows: &[
    "....SSSSSSSSSS.....S...................KK...........SSSSSSSSS...",
    "....SWWWYWWWYSS...SLS................KKppK..........SWWWYWWWLS..",
    "....SWYWWWYWWKKK.KpWLS....SSS......KKpppppK.........SWYWWWYWLWS.",
    "....SWWWBBBBYWLK.KpWWS..SSWWS....KKppppppppK........S.W.Y..WKKKK",
    "....SWYBBWYBBWLK.KpWWLSSWWWYSKK.KLppppppppppK....KSSKK.KSSK..WLK",
    "....SWWBBWWBBWLK.KpWWWKLWYWWSWpKKpLppppppppppK..KKWWLSKWWWLKpWLK",
    "....SWYWWWYBBWLK.KpWWWKLWWWYSWpKKppLppppppppppK.KSWWLSSWWWLKpWLK",
    "....SWWWYWBBYWLK.KpWWWKLWYWWSWpKKpppLpppppppppKKKSWWLSSWWWLKpWLK",
    "....SWYWWBBWWWLK.KpWWWKLWWWYSWpKKppppLppppppKKS.KSWWLSKWWWLKpWLK",
    "....SWWWYWWWYWLK.KpWWWKLWYWWSWpK.KppppLpppKKSWS.KSWWLSKWWWLKpWLK",
    "....SWYWWBBWWWLK.KppWWKLWWWSLWpK..KppppLKKSLWLKKKSKKKLSLKKKKpWLK",
    "....SWWWYBBWYWLK..KppWKLWSSLWWpK...KpppKSWLWLKK.KKLLLKKKLLLSKWLK",
    "....SWYWWWYWWWLK...KpWKSSLWWWWpK....KppKWLWKK...ppppppSppppppWLK",
    "....SLLLLLLLLLLK....KppppppppppK.....KpKSKK.........SLLLLLLLLLLK",
    "....KKKKKKKKKKKK.....KKKKKKKKKKK......KKK...........KKKKKKKKKKKK",
    "................................................................",
] };

#[rustfmt::skip]
const NORMAL_98: Rows = Rows { w: 108, h: 16, palette: &[], rows: &[
    "............................................................................................................",
    "................................................................................................KKKKK.......",
    "................................................................................SKKKKKKKK..KK..KKWK.KK......",
    ".......KKKKKKKKKK........KKKKKKKKKK........K....................K..............S.WWWWWKSS....KKKWWK..K......",
    ".......KSSSSSSSSK........KSSSSSSSSK.......KK....................KK............S.WKWWWWKSS..SS..KWKKK.K......",
    "...K...KWWWWWWWWK.....K..KWWWWWWWWK......KWK....................KWK..........S.WKW.WWWKSS....SSKKSKWKKSS....",
    "...KK..KWWWWWWWWK....KK..KWWWWWWWWK.....KWWKKKKKKKK......KKKKKKKKWWK.....SSSSSWKW.S.WKKSS..SS..KSSKWWK..S...",
    "...KWK.KWWSSSSWWK...KWK..KWWSSSSWWK....KWWWWWWWWWWK......KWWWWWWWWWWK....SWWKWKS.S.WK.KKK....SSKSKKKWKSS.K..",
    "KKKKWWKSWWWWWWWWK..KWWKKKKSWWWWWWWK....KWWWWWWWWWWK......KWWWWWWWWWWK....SWKWKWWS.WK.......KK..KKKKKKKS...K.",
    "KWWWWWWK.WSSSWWWK.KWWWWWWWSWSSSWWWK.....KWWKKKKKKKK......KKKKKKKKWWK.....SWSKWWWWSK..........KKKKKSSKK..SSK.",
    "KKKKWWKSWWWWWWWWK..KWWKKKKSWWWWWWWK......KWK....................KWK......SWWWW..WWK.KKKKK........S.SS.SSSKK.",
    "...KWK.KWWSSSSWWK...KWK..KWWSSSSWWK.......KK....................KK.......SWSSWWWWWK..KKK.........S.S.SSSKSK.",
    "...KK..KWWWWWWWWK....KK..KWWWWWWWWK........K....................K........SWSSWS.SWK...K...........SSSSSKSK..",
    "...K...KWWWWWWWWK.....K..KWWWWWWWWK......................................SWWWWWWWWK................KSKKKK...",
    ".......KKKKKKKKKK........KKKKKKKKKK......................................KKKKKKKKKK.................KKKK....",
    "............................................................................................................",
] };

#[rustfmt::skip]
const HOT_98: Rows = Rows { w: 108, h: 16, palette: &[], rows: &[
    "............................................................................................................",
    "................................................................................SKKKKKKKK.......KKKKK.......",
    "...............................................................................SLWWWWWKBB..KK..KKRKgKK......",
    ".......KKKKKKKKKK........KKKKKKKKKK........K....................K.............SLWKWWWWKBB....KKKRRKggK......",
    ".......KBBBBBBBBK........KBBBBBBBBK.......KK....................KK...........SLWKWLWWWKBB..RRLLKRKKKgK......",
    "...K...KWWWWWWWWK.....K..KWWWWWWWWK......KCK....................KCK......BBBBSWKWLSLWKKBB....RRKKBKYKKKK....",
    "...KK..KWWWWWWWWK....KK..KWWWWWWWWK.....KCCKKKKKKKK......KKKKKKKKCCK.....BWWKWKSLSLWK.KKK..BBLLKBBKYYKggK...",
    "...KYK.KWWSSSSWWK...KYK..KWWSSSSWWK....KCCCCCCCCCCK......KCCCCCCCCCCK....BWKWKWWSLWK.........BBKBKKKYKttgK..",
    "KKKKYYKSWWWWWWWWK..KYYKKKKSWWWWWWWK....KCCCCCCCCCCK......KCCCCCCCCCCK....BWSKWWWWSK........KKLLKKKKKKKCtngK.",
    "KYYYYYYKLWSSSWWWK.KYYYYYYYSWSSSWWWK.....KCCKKKKKKKK......KKKKKKKKCCK.....BWWWWLLWWK.KKKKK....KKKKKttKKttngK.",
    "KKKKYYKSWWWWWWWWK..KYYKKKKSWWWWWWWK......KCK....................KCK......BWSSWWWWWK..KKK.........KBtgtBBtnK.",
    "...KYK.KWWSSSSWWK...KYK..KWWSSSSWWK.......KK....................KK.......BWSSWSLSWK...K..........KBBtgtBBnK.",
    "...KK..KWWWWWWWWK....KK..KWWWWWWWWK........K....................K........BWWWWWWWWK...............KBBgBgnK..",
    "...K...KWWWWWWWWK.....K..KWWWWWWWWK......................................KKKKKKKKKK................KnBnnK...",
    ".......KKKKKKKKKK........KKKKKKKKKK.................................................................KKKK....",
    "............................................................................................................",
] };

#[rustfmt::skip]
const NORMAL_2000: Rows = Rows { w: 120, h: 20, palette: &[], rows: &[
    "..........................................................................................................KKKKKKK.......",
    "..moKKKKKKKKKKKKKKKK.........KKKKKKKKKKK................................................KKKKKK.KK.......KKLWLWLWKKK.....",
    "...KLLLLLLLLLLLLLLLK.........KLLLLLLLLLK...............................................KWWWWWLKSS......KWLWLWLWLKL.Y....",
    "..ooKKKKKKKKKKKKKKKK.........KKKKKKKKKKK..............................................KLKWLWLWLSS.....KWLWLWLWLKLSLSK...",
    "...KLSLLLKWWWWWWWWWK.....S...KWWWWWWWWWK........K......................K........KKKKKKLKLKWLWLWSS....KWLWLWLWKKKKKKLSK..",
    "...KLSKLLKWWWWWWWWWK....KS...KWWWWWWWWWK.......KK......................KK.......KSSSKLKLKLKWLWKSS....KKWLWLKKLWWWWLKKK..",
    "...KLSWKLKWWSSSSSLWK...KWS...KWWSSSSSWWK......KWK......................KWK......KKKKLKKKLKLKKKKKK...KLSKWLKLWWSSSSWWLK..",
    "SSSSSSWWKKWWWWWWWWWK..KWWSSSSKWWWWWWWWWK.....KWWK......................KWWK.....KWKKKWWWKLKWK.......KSLSKKLWSSWWWSSSWLK.",
    "SWWWWWWWWKWWSSSSLWWK.KWWWWWWSKWWSSSSWWWK....KWWWKKKKKKKKK......KKKKKKKKKWWWK....KWKWWWWWKKWWK.......KLSLSKWSSWWLWWSSSWK.",
    "RgWWWWWWWWKWWWWWWWWKKWWWWWWWSKWWWWWWWWWK..mmWWWWWWWWWWWWK......KWWWWWWWWWWWWK...KWKWSSSWKWWWK.......KKKKKLWSSWSSLWSSSWLK",
    "SWWWWWWWWKWWSSSSSLWK.KWWWWWWSKWWSSSSSWWK..GmWWWWWWWWWWWWK......KWWWWWWWWWWWWWK..KWKWWWWWKWWWK.......KWLWKWSSSSSSLWSSSSWK",
    "KKKKKKWWKKWWWWWWWWWK..KWWKKKKKWWWWWWWWWK..omWWWWWWWWWWWWK......KWWWWWWWWWWWWK...KWKWSSSWKWWWK..KKKKKKLWLKWSSSSSLWSSSSSWK",
    "...KLKWKLKWWSSSSLWWK...KWK...KWWSSSSWWWK....KWWWKKKKKKKKK......KKKKKKKKKWWWK....KWKWWWWWKWWWK...KKK.KWLWKWSSSSSWLSSSSSWK",
    "...KLKKLLKWWWWWWWWWK....KK...KWWWWWWWWWK.....KWWK......................KWWK.....KWKWSSSWKWWWK....K...KWLKWSSSSSWLSSSSSWK",
    "...KLKLLLKWWSSSSSLWK.....K...KWWSSSSSWWK......KWK......................KWK......KWKWWWWWKWWWK........KLWKLWSSSSSSSSSSWLK",
    "...KLLLLLKWWWWWWWWWK.........KWWWWWWWWWK.......KK......................KK.......KWKKKKKKKWWWK.........KLWKWSSSSWLSSSSWK.",
    "...KLLLLLKWWWWWWWWWK.........KWWWWWWWWWK........K......................K........tnWWWWWWWWWWK..........KKKLWSSSLLSSSWLK.",
    "...KLLLLLKWWWWWWWWWK.........KWWWWWWWWWK........................................WnWWWWWWWWWWK............KKLWWSSSSWWLK..",
    "..GoKKKKKKKKKKKKKKKK.........KKKKKKKKKKK........................................KKKKKKKKKKKKK..............KKLWWWWLKK...",
    ".............................................................................................................KKKKKK.....",
] };

#[rustfmt::skip]
const HOT_2000: Rows = Rows { w: 120, h: 20, palette: &[], rows: &[
    "..........................................................................................................KKKKKKK.......",
    "..moKKKKKKKKKKKKKKKK.........KKKKKKKKKKK................................................KKKKKK.KK.......KKgGgGgGKKK.....",
    "...KBBBBBBBBBBBBBBBK.........KBBBBBBBBBK...............................................KWWWWWLKBB......KGgGgGgGgKC.Y....",
    "..ooKKKKKKKKKKKKKKKK.........KKKKKKKKKKK..............................................KLKWWWLWLBB.....KGgGgGgGgKCBCBK...",
    "...KLSLLLKWWWWWWWWWK.....S...KWWWWWWWWWK........K......................K........KKKKKKLKLKWLWLWBB....KGgGgGgGKKKKKKCBK..",
    "...KLSKLLKWWWWWWWWWK....KS...KWWWWWWWWWK.......KK......................KK.......KBBBKLKLKLKWLWKBB....KKGgGgKKLWWWWLKKK..",
    "...KLSYKLKWWSSSSSLWK...KYS...KWWSSSSSWWK......KCK......................KCK......KKKKLKKKLKLKKKKKK...KCBKGgKLWWBBBBWWLK..",
    "SSSSSSYYKKWWWWWWWWWK..KYYSSSSKWWWWWWWWWK.....KCCK......................KCCK.....KWKKKWWWKLKWK.......KBCBKKLWBBWWWBBBWLK.",
    "SYWYWYYYYKWWSSSSLWWK.KYYYYWYSKWWSSSSWWWK....KCCCKKKKKKKKK......KKKKKKKKKCCCK....KWKWWWWWKKWWK.......KCBCBKWBBWWLWWBBBWK.",
    "SWYWYWYYYYKWWWWWWWWKKYYYYWYWSKWWWWWWWWWK..mmCCCCCCCCCCCCK......KCCCCCCCCCCCCK...KWKWSSSWKWWWK.......KKKKKLWBBWBBLWBBBWLK",
    "SYWYYYYYYKWWSSSSSLWK.KYYYYYYSKWWSSSSSWWK..GmCCCCCCCCCCCCK......KCCCCCCCCCCCCCK..KWKWWWWWKWWWK.......KgGgKWBBBBBBLWBBBBWK",
    "KKKKKKYYKKWWWWWWWWWK..KYYKKKKKWWWWWWWWWK..omCCCCCCCCCCCCK......KCCCCCCCCCCCCK...KWKWSSSWKWWWK..KKKKKKGgGKWBBBBBLWBBBBBWK",
    "...KLKYKLKWWSSSSLWWK...KYK...KWWSSSSWWWK....KCCCKKKKKKKKK......KKKKKKKKKCCCK....KWKWWWWWKWWWK...KKK.KgGgKWBBBBBWLBBBBBWK",
    "...KLKKLLKWWWWWWWWWK....KK...KWWWWWWWWWK.....KCCK......................KCCK.....KWKWSSSWKWWWK....K...KgGKWBBBBBWLBBBBBWK",
    "...KLKLLLKWWSSSSSLWK.....K...KWWSSSSSWWK......KCK......................KCK......KWKWWWWWKWWWK........KGgKLWBBBBBBBBBBWLK",
    "...KLLLLLKWWWWWWWWWK.........KWWWWWWWWWK.......KK......................KK.......KWKKKKKKKWWWK.........KGgKWBBBBWLBBBBWK.",
    "...KLLLLLKWWWWWWWWWK.........KWWWWWWWWWK........K......................K........tnWWWWWWWWWWK..........KKKLWBBBLLBBBWLK.",
    "...KLLLLLKWWWWWWWWWK.........KWWWWWWWWWK........................................WnWWWWWWWWWWK............KKLWWBBBBWWLK..",
    "..GoKKKKKKKKKKKKKKKK.........KKKKKKKKKKK........................................KKKKKKKKKKKKK..............KKLWWWWLKK...",
    ".............................................................................................................KKKKKK.....",
] };

#[rustfmt::skip]
const DISABLED_ARROWS: Rows = Rows { w: 40, h: 20, palette: &[], rows: &[
    "........................................",
    "........................................",
    "........................................",
    "........................................",
    "........S......................S........",
    ".......SSW.....................SS.......",
    "......S.SW.....................SWS......",
    ".....S.WSW.....................SW.S.....",
    "....S.W.SSSSSSSSS......SSSSSSSSSW..S....",
    "..SS.W...WWWWWWWSW.....SWWWWWWWWW...S...",
    "..SSW...........SW.....SW............S..",
    "..SSW...........SW.....SW...........S.W.",
    "...WS...SSSSSSSSSW.....SSSSSSSSS...S.W..",
    ".....S..SWWWWWWWWW......WWWWWWWSW.S.W...",
    "......S.SW.....................SWS.W....",
    ".......SSW.....................SS.W.....",
    "........SW.....................SWW......",
    ".........W......................W.......",
    "........................................",
    "........................................",
] };

// Internet Explorer’s HTML document icon.
#[rustfmt::skip]
const IE_DOCUMENT: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[
        (b'A', 0x555555), (b'B', 0xfffbf0), (b'C', 0xe7e7d6), (b'D', 0xcccccc),
        (b'E', 0xeaeaea), (b'F', 0x0066cc), (b'G', 0x777777), (b'H', 0xffffff),
        (b'I', 0x3366ff), (b'J', 0x868686), (b'K', 0xf1f1f1), (b'L', 0x3333cc),
        (b'M', 0x006699), (b'N', 0x999999), (b'O', 0x333333), (b'P', 0x000080),
        (b'Q', 0x999966), (b'R', 0xc0c0c0), (b'S', 0x040404), (b'T', 0x669999),
        (b'U', 0x99cccc), (b'V', 0x66ffff), (b'W', 0xffff99), (b'X', 0x00ccff),
        (b'Y', 0xffcc99), (b'Z', 0xcc9966), (b'a', 0x3366cc), (b'b', 0x5f5f5f),
        (b'c', 0x003399), (b'd', 0x808000), (b'e', 0x333366),
    ],
    rows: &[
        "...GJJJJJJJJJ...",
        "...GHHHHHHHHNA..",
        "...GHBBBBBBBNBA.",
        "...GHBBFFBBBAOOO",
        "...PFFFFBIKBDNGA",
        "..PTULLVFKBKEDNA",
        ".PQWFBBLXFKEECDA",
        ".GYIFFFFFFEECCDA",
        "GZaIFBBKBEKCECDA",
        "bQcIIRRLIMEECCDA",
        "dLeLIIIIMEECECDA",
        ".L.SSMMMEECECCDA",
        "..LGHKBKECECCCDA",
        "...GEEECCECCCCDA",
        "...GDDDDDDDDDDDA",
        "...GAAAAAAAAAAAA",
    ],
};

// The small note-and-pencil image that precedes a Windows 2000 note in its Help articles.
#[rustfmt::skip]
const NOTE: Rows = Rows { w: 10, h: 10, palette: &[
    (b'0', 0x020202), (b'1', 0x858585), (b'2', 0x8d8c8d), (b'3', 0xc0c0c0), (b'4', 0xd3d3d3),
    (b'5', 0xff0505), (b'6', 0xff5d5d), (b'7', 0xffcc00), (b'8', 0xffffcc), (b'9', 0xffffff),
], rows: &[
    "2222224365",
    "2888888075",
    "2888880709",
    "2888807099",
    "2888170499",
    "2884018199",
    "2881488099",
    "2888888099",
    "2888888099",
    "0000000099",
] };

// The colors of 2000's Web Help heading (assets/html-help/support-heading.i4, two cropped 4-bit
// sprites: the title 199×41 and the arrow beneath it 35×51).
const SUPPORT_HEADING_COLORS: &[u32] = &[0, 0x333333, 0x666666, 0x999999, 0xfefefe, 0xff3300, 0xffffff];

#[rustfmt::skip]
pub(crate) const ENTRIES: &[Entry] = &[
    Entry { what: "html-help/caption", looks: &[Look::Win98, Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&CAPTION) },
    Entry { what: "html-help/tree", looks: &[Look::Win98, Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&TREE) },
    Entry { what: "html-help/normal", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&NORMAL_98) },
    Entry { what: "html-help/hot", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&HOT_98) },
    Entry { what: "html-help/normal", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&NORMAL_2000) },
    Entry { what: "html-help/hot", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&HOT_2000) },
    Entry { what: "html-help/disabled-arrows", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&DISABLED_ARROWS) },
    Entry { what: "html-help/ie-document", looks: &[Look::Win98, Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&IE_DOCUMENT) },
    Entry { what: "html-help/note", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&NOTE) },
    Entry { what: "html-help/support-heading-colors", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Colors(SUPPORT_HEADING_COLORS) },
];

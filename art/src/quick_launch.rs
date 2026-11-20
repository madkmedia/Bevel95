//! The Quick Launch band's own pictures: Internet Explorer, Outlook Express and Show Desktop,
//! 16x16, as Windows 98 and 2000 showed them on their bar, and Windows 95's own globe.
//!
//! True Color: cut from the Reference PCs' captures of the default band (98's and 2000's shell
//! draws a file's icon through its image list, which changes colors slightly), the see-through
//! pixels from the icon in Windows' own file, checked against the bar's face in the capture.
//! 16 colors: the files' 4-bit images, each color as its number among Windows' 16
//! (`core::colors16::VGA`) in hexadecimal, '.' see-through. Written by
//! private/probes/quick-launch/build/cut.ps1.

use crate::{Art, Depth, Entry, Look, Rows};
/// Windows 98: Internet Explorer (IEXPLORE.EXE group 32528), as its Quick Launch band drew it.
#[rustfmt::skip]
const W98_INTERNET: Rows = Rows { w: 16, h: 16, palette: &[(b'a', 0x0060CF), (b'b', 0x3060FF), (b'c', 0x309FCF), (b'd', 0x009FFF), (b'e', 0x3030CF), (b'f', 0x609FCF), (b'g', 0x9FCFCF), (b'h', 0x30309F), (b'i', 0x609F9F), (b'j', 0xC0C0C0), (b'k', 0x00CFFF), (b'l', 0x60FFFF), (b'm', 0xFFFF9F), (b'n', 0x9FFFFF), (b'o', 0x9F9F60), (b'p', 0xFFCF9F), (b'q', 0x707070), (b'r', 0xCFCF9F), (b's', 0x6060CF), (b't', 0x3060CF), (b'u', 0xCF9F60), (b'v', 0x30609F), (b'w', 0x5F5F5F), (b'x', 0x00609F), (b'y', 0x00309F), (b'z', 0x000080), (b'A', 0x303060), (b'B', 0x101010), (b'C', 0x00009F)], rows: &[
    "...........aaaa.",
    "......aaaaaa...a",
    "....aabbbbcda..b",
    "...eafgaacdcda.b",
    "..ehijaaeeaklda.",
    "..himbaa..ecnla.",
    ".eopbba....elkda",
    ".qrstbbbbccdddda",
    "qrutvbaaaaaaaaaa",
    "qubevba.........",
    "wothvab....ebbbx",
    "wtyzevba..ebbbbx",
    "we.Ahevbabbbbbx.",
    "qe..BBevvabbxx..",
    ".ex..yBCevxx....",
    "..eee...........",
]};

/// Windows 98: Outlook Express (MSIMN.EXE group 2), as its Quick Launch band drew it.
#[rustfmt::skip]
const W98_OUTLOOK: Rows = Rows { w: 16, h: 16, palette: &[(b'a', 0x0060CF), (b'b', 0x000080), (b'c', 0x3060FF), (b'd', 0x609F9F), (b'e', 0x9FCFCF), (b'f', 0x3030CF), (b'g', 0x60FFFF), (b'h', 0x9F9F60), (b'i', 0xFFFF9F), (b'j', 0x00CFFF), (b'k', 0x707070), (b'l', 0xFFCF9F), (b'm', 0xCF9F60), (b'n', 0x3060CF), (b'o', 0x5F5F5F), (b'p', 0x00309F), (b'q', 0xC0C0C0), (b'r', 0x00609F), (b's', 0x808000), (b't', 0x303060), (b'u', 0xAFAF90), (b'v', 0x000000), (b'w', 0xCFCF9F), (b'x', 0xDFDFDF), (b'y', 0xFFFFFF), (b'z', 0xFF5050), (b'A', 0x9F3030), (b'B', 0x30609F)], rows: &[
    ".......aa.......",
    "...baaaa.c......",
    "..bdeffga.......",
    ".bhia..fja......",
    ".klcaaaaaa......",
    "kmnca...........",
    "ohpccqqfcr......",
    "sftfccccrssuuuuv",
    ".f.vvffrhwxyyiwv",
    "..fshhhhwxyyzAwv",
    "...swwwwxiyyAAwv",
    "...uxxyBBBByiywv",
    "...uyyiyiyiiyiwv",
    "...uiyyBBBByiywv",
    "...uwwwwwwwwwwwv",
    "...vvvvvvvvvvvvv",
]};

/// Windows 98: Show Desktop (EXPLORER.EXE group 103), as its Quick Launch band drew it.
#[rustfmt::skip]
const W98_SHOW_DESKTOP: Rows = Rows { w: 16, h: 16, palette: &[(b'a', 0xFF5050), (b'b', 0xCF3000), (b'c', 0xCF9F00), (b'd', 0xFF9FCF), (b'e', 0x9F0000), (b'f', 0x009FCF), (b'g', 0x00609F), (b'h', 0xAFAF90), (b'i', 0xFFCF00), (b'j', 0x9F6000), (b'k', 0x000000), (b'l', 0x60CFFF), (b'm', 0xCFCF9F), (b'n', 0xFFFFFF), (b'o', 0xFFFFCF), (b'p', 0xFFFF9F), (b'q', 0x30609F), (b'r', 0x909090), (b's', 0x9F9F60), (b't', 0x707070), (b'u', 0x404040)], rows: &[
    "............ab..",
    "...........cde..",
    "ffffghhhhhcijkhk",
    "flfgmnnoncpjkfgk",
    "ffgmnnoocijkqlgk",
    "fgmnnorcpjkhoqgk",
    "gmnnorcijkhommqk",
    "hnnornsjkhomohrk",
    "hnornnknnkmohmrk",
    "hotnnnnnkhommhrk",
    "hoounnnkhommhmrk",
    "fooounkhommhmhqk",
    "ffoookhommhmhqqk",
    "flfoomommhmhqlqk",
    "gggfrrrrrrrqqqqk",
    "kkkkkkkkkkkkkkkk",
]};

/// Windows 2000: Show Desktop (EXPLORER.EXE group 103), as its Quick Launch band drew it.
#[rustfmt::skip]
const W2K_SHOW_DESKTOP: Rows = Rows { w: 16, h: 16, palette: &[(b'a', 0xFF5252), (b'b', 0xCE3100), (b'c', 0xCE9C00), (b'd', 0xFF9CCE), (b'e', 0x9C0000), (b'f', 0x009CCE), (b'g', 0x00639C), (b'h', 0xADAD94), (b'i', 0xFFCE00), (b'j', 0x9C6300), (b'k', 0x000000), (b'l', 0x63CEFF), (b'm', 0xCECE9C), (b'n', 0xFFFFFF), (b'o', 0xFFFFCE), (b'p', 0xFFFF9C), (b'q', 0x31639C), (b'r', 0x949494), (b's', 0x9C9C63)], rows: &[
    "............ab..",
    "...........cde..",
    "ffffghhhhhcijkhk",
    "flfgmnnoncpjkfgk",
    "ffgmnnoocijkqlgk",
    "fgmnrrrcpjkhoqgk",
    "gmnnrncijkhommqk",
    "hnnornsjkkomohrk",
    "hnoornknokmohmrk",
    "hooornnnokommhrk",
    "hooornnnokmmhmrk",
    "fooorooookmhmhqk",
    "ffookkkkkkhmhqqk",
    "flfoomommhmhqlqk",
    "gggfrrrrrrrqqqqk",
    "kkkkkkkkkkkkkkkk",
]};

/// Windows 2000: Internet Explorer (IEXPLORE.EXE group 32528), as its Quick Launch band drew it.
#[rustfmt::skip]
const W2K_INTERNET: Rows = Rows { w: 16, h: 16, palette: &[(b'a', 0x0063CE), (b'b', 0x3163FF), (b'c', 0x319CCE), (b'd', 0x009CFF), (b'e', 0x3131CE), (b'f', 0x639CCE), (b'g', 0x9CCECE), (b'h', 0x31319C), (b'i', 0x639C9C), (b'j', 0xC6C6C6), (b'k', 0x00CEFF), (b'l', 0x63FFFF), (b'm', 0xFFFF9C), (b'n', 0x9CFFFF), (b'o', 0x9C9C63), (b'p', 0xFFCE9C), (b'q', 0x737373), (b'r', 0xCECE9C), (b's', 0x6363CE), (b't', 0x3163CE), (b'u', 0xCE9C63), (b'v', 0x31639C), (b'w', 0x5A5A5A), (b'x', 0x00639C), (b'y', 0x00319C), (b'z', 0x000084), (b'A', 0x313163), (b'B', 0x101010), (b'C', 0x00009C)], rows: &[
    "...........aaaa.",
    "......aaaaaa...a",
    "....aabbbbcda..b",
    "...eafgaacdcda..",
    "..ehijaaeeaklda.",
    "..himbaa..ecnla.",
    ".eopbba....elkda",
    ".qrstbbbbccdddda",
    "qrutvbaaaaaaaaaa",
    "qubevba.........",
    "wothvab....ebbbx",
    "wtyzevba..ebbbbx",
    "we.Ahevbabbbbbx.",
    "qe..BBevvabbxx..",
    ".ex..yBCevxx....",
    "..eee...........",
]};

/// Windows 2000: Outlook Express 5 (MSIMN.EXE group 2), as its Quick Launch band drew it.
#[rustfmt::skip]
const W2K_OUTLOOK: Rows = Rows { w: 16, h: 16, palette: &[(b'a', 0x639CFF), (b'b', 0x63CEFF), (b'c', 0x63FFFF), (b'd', 0x3163FF), (b'e', 0x31009C), (b'f', 0x3100CE), (b'g', 0x636363), (b'h', 0x009CCE), (b'i', 0x000000), (b'j', 0xFFFFFF), (b'k', 0xFFFFCE), (b'l', 0x000084), (b'm', 0xFF0063), (b'n', 0xCE0000), (b'o', 0x9C0000), (b'p', 0x6363FF)], rows: &[
    "......abcc......",
    ".....abcdea.....",
    "..f.dbcd.ae.....",
    "ggffhbfggggggggi",
    "gjfhhfkkkkkkkkki",
    "gjfdhlkkkkkkmnki",
    "gjlllllkkkkknoki",
    "gjjjkkkkkkkkkkki",
    "gjjkkkkkkkkkkkki",
    "gjjkkkkkkfdddlki",
    "gjkkkkkkkkfcclki",
    "gkkkkkkkkkfcclki",
    "giiiiiiiifbcllii",
    ".....ea.fabd.l..",
    ".....pedaad.....",
    "......elfd......",
]};

/// Windows 95: its own globe (SHELL32.DLL group 14), the Internet picture its files have (plain 95 had no Internet Explorer).
#[rustfmt::skip]
const W95_INTERNET: Rows = Rows { w: 16, h: 16, palette: &[(b'a', 0xC0C0C0), (b'b', 0x008080), (b'c', 0x808000), (b'd', 0x00FFFF), (b'e', 0xFF00FF), (b'f', 0x000000), (b'g', 0xFFFF00), (b'h', 0x00FF00), (b'i', 0xFFFFFF), (b'j', 0x800080), (b'k', 0x808080), (b'l', 0x008000)], rows: &[
    "......abbacc....",
    ".....bdeebfgc...",
    "....bdeddebfgc..",
    "...bheiidjefgc..",
    "...bedeiebkfac..",
    "...bkddedjbfcf..",
    "...eddedelkfcf..",
    "...cflddaefccf..",
    "...cgfbbbfccf...",
    "....cgcffccf....",
    ".....ccccff.....",
    ".......cf.......",
    "......cccf......",
    "....ccggccff....",
    "...cigccccccf...",
    "....ffffffff....",
]};

/// Windows 98 in 16 colors: Internet Explorer (the 4-bit image).
#[rustfmt::skip]
const W98_INTERNET_16: Rows = Rows { w: 16, h: 16, palette: &[], rows: &[
    "...........CCCC.",
    "......CCCCCC...C",
    "....4CCCCCCC4..C",
    "...4CCBCCCCCC4.C",
    "..443BCC44CCCC4.",
    "..43BCCC..0CCC4.",
    ".43BCCC....0CCC4",
    ".3BCCCCCCCCCCCC4",
    "3B3CCCC444444444",
    "37C4CCC.........",
    "33C4CCC....0CCC4",
    "3CC44CCC880CCCC4",
    "3C.044CCCCCCCC4.",
    "3C..0044CCCC44..",
    ".CC..C000000....",
    "F.CCC...........",
]};

/// Windows 98 in 16 colors: Outlook Express (the 4-bit image).
#[rustfmt::skip]
const W98_OUTLOOK_16: Rows = Rows { w: 16, h: 16, palette: &[], rows: &[
    ".......CC.......",
    "...4CCC4.C......",
    "..43BCCC4.......",
    ".43BC..4C4......",
    ".3BCCCCCC4......",
    "374C8...........",
    "334CC778C4......",
    "3C04CCCC48888880",
    ".C.0000887FFFF70",
    "..C888877FFF9170",
    "...87777FFFF1170",
    "...8FFFCCCCFFF70",
    "...8FFFFFFFFFF70",
    "...8FFFCCCCFFF70",
    "...8777777777770",
    "...0000000000000",
]};

/// Windows 98 in 16 colors: Show Desktop (the 4-bit image).
#[rustfmt::skip]
const W98_SHOW_DESKTOP_16: Rows = Rows { w: 16, h: 16, palette: &[], rows: &[
    "............11..",
    "...........391..",
    "66666888883B3080",
    "6E6C7FFFF3B30CC0",
    "66C7FF7F3B3047C0",
    "6C7FF783B30874C0",
    "67FF783B30877740",
    "8FF78F3308777780",
    "8F78FF0F70777780",
    "8F07FFF708777780",
    "8F707F7087777780",
    "67F70708777777C0",
    "6C77708777777C60",
    "6EC777777777C7C0",
    "666C8888888C6CC0",
    "0000000000000000",
]};

/// Windows 2000 in 16 colors: Show Desktop (the 4-bit image).
#[rustfmt::skip]
const W2K_SHOW_DESKTOP_16: Rows = Rows { w: 16, h: 16, palette: &[], rows: &[
    "............11..",
    "...........391..",
    "66666888883B3080",
    "6E6C7FFFF3B30CC0",
    "66C7FF7F3B3047C0",
    "6C7F8883B30874C0",
    "67FF8F3B30877740",
    "8FF78F3300777780",
    "8F7F8F0F70777780",
    "8FF78FFF70777780",
    "8F7F8FFF70777780",
    "67F78777707777C0",
    "6C77000000777C60",
    "6EC777777777C7C0",
    "666C8888888C6CC0",
    "0000000000000000",
]};

/// Windows 2000 in 16 colors: Internet Explorer (the 4-bit image).
#[rustfmt::skip]
const W2K_INTERNET_16: Rows = Rows { w: 16, h: 16, palette: &[], rows: &[
    "...........CCCC.",
    "......CCCCCC...C",
    "....4CCCCCCC4..C",
    "...4CCBCCCCCC4..",
    "..443BCC44CCCC4.",
    "..43BCCC..0CCC4.",
    ".43BCCC....0CCC4",
    ".3BCCCCCCCCCCCC4",
    "3B3CCCC444444444",
    "37C4CCC.........",
    "33C4CCC....0CCC4",
    "3CC44CCC880CCCC4",
    "3C.044CCCCCCCC4.",
    "3C..0044CCCC44..",
    ".CC..C000000....",
    "..CCC...........",
]};

/// Windows 2000 in 16 colors: Outlook Express 5 (the 4-bit image).
#[rustfmt::skip]
const W2K_OUTLOOK_16: Rows = Rows { w: 16, h: 16, palette: &[], rows: &[
    "......6EEE......",
    ".....CEE448.....",
    "..C.C6E4.64.....",
    "88CC664888888880",
    "8FCC64FBFFBFBFB0",
    "8FCCC4FFFFFF99F0",
    "8F44444BFBFB91B0",
    "8FFFFFFFFBFFBFF0",
    "8FFFFBFBFBFBFFB0",
    "8FFFFFFFFCCCC4F0",
    "8FFFFBFBFBCEE4B0",
    "8FFBFFBFFFCEE4F0",
    "800000000C6E4400",
    ".....46.C66C.4..",
    ".....C4CC6C.....",
    "......C4CC......",
]};

pub(crate) const ENTRIES: &[Entry] = &[
    Entry { what: "quick-launch/internet-explorer", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&W98_INTERNET) },
    Entry { what: "quick-launch/outlook-express", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&W98_OUTLOOK) },
    Entry { what: "quick-launch/show-desktop", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&W98_SHOW_DESKTOP) },
    Entry { what: "quick-launch/show-desktop", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&W2K_SHOW_DESKTOP) },
    Entry { what: "quick-launch/internet-explorer", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&W2K_INTERNET) },
    Entry { what: "quick-launch/outlook-express", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&W2K_OUTLOOK) },
    Entry { what: "quick-launch/internet-explorer", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&W95_INTERNET) },
    Entry { what: "16-colors/quick-launch-internet-explorer", looks: &[Look::Win98], depth: Depth::Sixteen, art: Art::Rows(&W98_INTERNET_16) },
    Entry { what: "16-colors/quick-launch-outlook-express", looks: &[Look::Win98], depth: Depth::Sixteen, art: Art::Rows(&W98_OUTLOOK_16) },
    Entry { what: "16-colors/quick-launch-show-desktop", looks: &[Look::Win98], depth: Depth::Sixteen, art: Art::Rows(&W98_SHOW_DESKTOP_16) },
    Entry { what: "16-colors/quick-launch-show-desktop", looks: &[Look::Win2000], depth: Depth::Sixteen, art: Art::Rows(&W2K_SHOW_DESKTOP_16) },
    Entry { what: "16-colors/quick-launch-internet-explorer", looks: &[Look::Win2000], depth: Depth::Sixteen, art: Art::Rows(&W2K_INTERNET_16) },
    Entry { what: "16-colors/quick-launch-outlook-express", looks: &[Look::Win2000], depth: Depth::Sixteen, art: Art::Rows(&W2K_OUTLOOK_16) },
];

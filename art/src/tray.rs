//! The notification area's network and battery pictures (used by `core::status`), 16×16, from each
//! Windows' own files, read from the Reference PCs' disks (95's Dial-Up Networking from its CD) with
//! `tools/dev/icons.ps1`: the network from Dial-Up Networking's RNAAPP.EXE (95 and 98, icons 110 to
//! 114) and Windows 2000's NETSHELL.DLL (190 to 193, 195); the battery from 95's SYSTRAY.EXE, 98's
//! and 2000's POWERCFG.CPL (the plug), BATMETER.DLL (its 16-pixel strip of batteries) and 2000's
//! STOBJECT.DLL (charging).

use crate::{Art, Depth, Entry, Look, Rows};

#[rustfmt::skip]
const WIN95_NET_IDLE: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0xFFFFFF), (b'c', 0x000000), (b'd', 0xC0C0C0), (b'e', 0x000080), (b'f', 0x008000), (b'g', 0x00FF00)],
    rows: &[
        "..aaaaaa........",
        ".abbbbbbc.......",
        ".accccadceeee...",
        ".acfffadc...e...",
        ".acfffadc...e...",
        ".addddddc...e...",
        ".cccccccaaaaaa..",
        "abbbbbbabbbbbbc.",
        "adgddddaccccadc.",
        ".ccccccacfffadc.",
        ".......acfffadc.",
        ".......addddddc.",
        ".......cccccccdc",
        "......abbbbbbbac",
        "......adgdddddac",
        ".......cccccccc.",
    ],
};

#[rustfmt::skip]
const WIN95_NET_SEND: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0xFFFFFF), (b'c', 0x000000), (b'd', 0x008000), (b'e', 0xC0C0C0), (b'f', 0x000080), (b'g', 0x00FF00)],
    rows: &[
        "..aaaaaa........",
        ".abbbbbbc.......",
        ".accccdecffff...",
        ".acgggdec...f...",
        ".acgggdec...f...",
        ".aeeeeeec...f...",
        ".cccccccaaaaaa..",
        "abbbbbbabbbbbbc.",
        "aegeeeeaccccaec.",
        ".ccccccacdddaec.",
        ".......acdddaec.",
        ".......aeeeeeec.",
        ".......cccccccec",
        "......abbbbbbbac",
        "......aegeeeeeac",
        ".......cccccccc.",
    ],
};

#[rustfmt::skip]
const WIN95_NET_RECEIVE: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0xFFFFFF), (b'c', 0x000000), (b'd', 0xC0C0C0), (b'e', 0x000080), (b'f', 0x008000), (b'g', 0x00FF00)],
    rows: &[
        "..aaaaaa........",
        ".abbbbbbc.......",
        ".accccadceeee...",
        ".acfffadc...e...",
        ".acfffadc...e...",
        ".addddddc...e...",
        ".cccccccaaaaaa..",
        "abbbbbbabbbbbbc.",
        "adgddddaccccfdc.",
        ".ccccccacgggfdc.",
        ".......acgggfdc.",
        ".......addddddc.",
        ".......cccccccdc",
        "......abbbbbbbac",
        "......adgdddddac",
        ".......cccccccc.",
    ],
};

#[rustfmt::skip]
const WIN95_NET_BOTH: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0xFFFFFF), (b'c', 0x000000), (b'd', 0x008000), (b'e', 0xC0C0C0), (b'f', 0x000080), (b'g', 0x00FF00)],
    rows: &[
        "..aaaaaa........",
        ".abbbbbbc.......",
        ".accccdecffff...",
        ".acgggdec...f...",
        ".acgggdec...f...",
        ".aeeeeeec...f...",
        ".cccccccaaaaaa..",
        "abbbbbbabbbbbbc.",
        "aegeeeeaccccdec.",
        ".ccccccacgggdec.",
        ".......acgggdec.",
        ".......aeeeeeec.",
        ".......cccccccec",
        "......abbbbbbbac",
        "......aegeeeeeac",
        ".......cccccccc.",
    ],
};

#[rustfmt::skip]
const WIN95_NET_OFF: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0xFFFFFF), (b'c', 0x000000), (b'd', 0xC0C0C0), (b'e', 0x000080), (b'f', 0x008000), (b'g', 0x00FF00), (b'h', 0xFF0000)],
    rows: &[
        "..aaaaaa........",
        ".abbbbbbc.......",
        ".accccadceeee...",
        ".acfffadc...e...",
        ".acfffadc...e...",
        ".addddddc...e...",
        ".cccccccaaaaaa..",
        "abbbbbbabbbbbbc.",
        "adgddddaccccfdc.",
        ".ccccccacffhhhc.",
        ".......acfhhhhh.",
        ".......adhdbhbdh",
        ".......cchhbbbhh",
        "......abbhdbhbdh",
        "......adgdhhhhhc",
        ".......cccchhhc.",
    ],
};

#[rustfmt::skip]
const WIN98_NET_IDLE: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0x008000), (b'd', 0x000080), (b'e', 0xFFFFFF), (b'f', 0xC0C0C0), (b'g', 0x00FF00)],
    rows: &[
        "..aaaaaa........",
        ".abbbbbab.......",
        ".abccccabdddd...",
        ".abccccab...d...",
        ".abccccab...d...",
        ".aaaaaaaaaaaaa..",
        ".bbbbbabbbbbbab.",
        "aeeeeeabcccccab.",
        "afgfffabcccccab.",
        ".bbbbbabcccccab.",
        "......abcccccab.",
        "......aaaaaaaab.",
        "......bbbbbbbbfb",
        ".....aeeeeeeeeab",
        ".....afgffffffab",
        "......bbbbbbbbb.",
    ],
};

#[rustfmt::skip]
const WIN98_NET_SEND: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0x00FF00), (b'd', 0xFFFFFF), (b'e', 0x000080), (b'f', 0x008000), (b'g', 0xC0C0C0)],
    rows: &[
        "..aaaaaa........",
        ".abbbbbab.......",
        ".abcdcdabeeee...",
        ".abdcdcab...e...",
        ".abcdcdab...e...",
        ".aaaaaaaaaaaaa..",
        ".bbbbbabbbbbbab.",
        "adddddabfffffab.",
        "agcgggabfffffab.",
        ".bbbbbabfffffab.",
        "......abfffffab.",
        "......aaaaaaaab.",
        "......bbbbbbbbgb",
        ".....addddddddab",
        ".....agcggggggab",
        "......bbbbbbbbb.",
    ],
};

#[rustfmt::skip]
const WIN98_NET_RECEIVE: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0x008000), (b'd', 0x000080), (b'e', 0xFFFFFF), (b'f', 0x00FF00), (b'g', 0xC0C0C0)],
    rows: &[
        "..aaaaaa........",
        ".abbbbbab.......",
        ".abccccabdddd...",
        ".abccccab...d...",
        ".abccccab...d...",
        ".aaaaaaaaaaaaa..",
        ".bbbbbabbbbbbab.",
        "aeeeeeabfefefab.",
        "agfgggabefefeab.",
        ".bbbbbabfefefab.",
        "......abefefeab.",
        "......aaaaaaaab.",
        "......bbbbbbbbgb",
        ".....aeeeeeeeeab",
        ".....agfggggggab",
        "......bbbbbbbbb.",
    ],
};

#[rustfmt::skip]
const WIN98_NET_BOTH: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0x00FF00), (b'd', 0xFFFFFF), (b'e', 0x000080), (b'f', 0xC0C0C0)],
    rows: &[
        "..aaaaaa........",
        ".abbbbbab.......",
        ".abcdcdabeeee...",
        ".abdcdcab...e...",
        ".abcdcdab...e...",
        ".aaaaaaaaaaaaa..",
        ".bbbbbabbbbbbab.",
        "adddddabcdcdcab.",
        "afcfffabdcdcdab.",
        ".bbbbbabcdcdcab.",
        "......abdcdcdab.",
        "......aaaaaaaab.",
        "......bbbbbbbbfb",
        ".....addddddddab",
        ".....afcffffffab",
        "......bbbbbbbbb.",
    ],
};

#[rustfmt::skip]
const WIN98_NET_OFF: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0x008000), (b'd', 0x000080), (b'e', 0xFFFFFF), (b'f', 0xC0C0C0), (b'g', 0x00FF00), (b'h', 0xFF0000)],
    rows: &[
        "..aaaaaa........",
        ".abbbbbab.......",
        ".abccccabdddd...",
        ".abccccab...d...",
        ".abccccab...d...",
        ".aaaaaaaaaaaaa..",
        ".bbbbbabbbbbbab.",
        "aeeeeeabcccccab.",
        "afgfffabcccccab.",
        ".bbbbbabccchhhb.",
        "......abcchhhhh.",
        "......aaahfehefh",
        "......bbbhheeehh",
        ".....aeeehfehefh",
        ".....afgffhhhhhb",
        "......bbbbbhhhb.",
    ],
};

#[rustfmt::skip]
const WIN2000_NET_IDLE: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0xC0C0C0), (b'd', 0xFFFFFF)],
    rows: &[
        "................",
        ".......aaaaaaa..",
        "......abbbbbbcb.",
        "......abbdbbbcb.",
        "..aaaaaaadbbbcb.",
        ".abbbbbbcbbbbcb.",
        ".abdbbbbcbbbbcb.",
        ".abdbbbbcbaaacb.",
        ".abbbbbbcbccccb.",
        ".abbbbbbcbbbbbb.",
        ".abaaaaacbcaacab",
        ".acccccccbaaaaab",
        ".abbbbbbbbbbbbb.",
        "adccccaacab.....",
        "aaaaaaaaaab.....",
        ".bbbbbbbbb......",
    ],
};

#[rustfmt::skip]
const WIN2000_NET_SEND: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0xC0C0C0), (b'd', 0x00FFFF), (b'e', 0xFFFFFF)],
    rows: &[
        "................",
        ".......aaaaaaa..",
        "......abbbbbbcb.",
        "......abdedddcb.",
        "..aaaaaaaedddcb.",
        ".abbbbbbcbdddcb.",
        ".abebbbbcbdddcb.",
        ".abebbbbcbaaacb.",
        ".abbbbbbcbccccb.",
        ".abbbbbbcbbbbbb.",
        ".abaaaaacbcaacab",
        ".acccccccbaaaaab",
        ".abbbbbbbbbbbbb.",
        "aeccccaacab.....",
        "aaaaaaaaaab.....",
        ".bbbbbbbbb......",
    ],
};

#[rustfmt::skip]
const WIN2000_NET_RECEIVE: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0xC0C0C0), (b'd', 0xFFFFFF), (b'e', 0x00FFFF)],
    rows: &[
        "................",
        ".......aaaaaaa..",
        "......abbbbbbcb.",
        "......abbdbbbcb.",
        "..aaaaaaadbbbcb.",
        ".abbbbbbcbbbbcb.",
        ".abdeeeecbbbbcb.",
        ".abdeeeecbaaacb.",
        ".abeeeeecbccccb.",
        ".abeeeeecbbbbbb.",
        ".abaaaaacbcaacab",
        ".acccccccbaaaaab",
        ".abbbbbbbbbbbbb.",
        "adccccaacab.....",
        "aaaaaaaaaab.....",
        ".bbbbbbbbb......",
    ],
};

#[rustfmt::skip]
const WIN2000_NET_BOTH: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0xC0C0C0), (b'd', 0x00FFFF), (b'e', 0xFFFFFF)],
    rows: &[
        "................",
        ".......aaaaaaa..",
        "......abbbbbbcb.",
        "......abdedddcb.",
        "..aaaaaaaedddcb.",
        ".abbbbbbcbdddcb.",
        ".abeddddcbdddcb.",
        ".abeddddcbaaacb.",
        ".abdddddcbccccb.",
        ".abdddddcbbbbbb.",
        ".abaaaaacbcaacab",
        ".acccccccbaaaaab",
        ".abbbbbbbbbbbbb.",
        "aeccccaacab.....",
        "aaaaaaaaaab.....",
        ".bbbbbbbbb......",
    ],
};

#[rustfmt::skip]
const WIN2000_NET_OFF: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x008080), (b'c', 0xC0C0C0), (b'd', 0x000000), (b'e', 0xFFFFFF), (b'f', 0xFF0000)],
    rows: &[
        "..aaaaaaa.......",
        ".abbbbbbcd......",
        ".abebbbbcd......",
        ".abebbbbcd......",
        ".abbbbbbcd......",
        ".abbbbbbcd......",
        ".acccccccd......",
        ".adddddddd......",
        "aeccccaacad.....",
        "aaaaaaaaaff...ff",
        ".ddddddddfff.fff",
        "....d.....fffff.",
        "....addddddfffdd",
        "..........fffff.",
        ".........fff.fff",
        ".........ff...ff",
    ],
};

#[rustfmt::skip]
const WIN95_POWER_AC: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0x808000), (b'd', 0xFFFF00), (b'e', 0xFFFFFF), (b'f', 0xC0C0C0)],
    rows: &[
        ".......abc.abc..",
        ".......cdc.cdc..",
        ".......cdc.cdc..",
        ".......cdaacdab.",
        "......aaaaaaaaab",
        ".....aefffffffab",
        "......abbbbbbbb.",
        ".......aeeeffab.",
        ".......aeffffab.",
        ".......aeffffab.",
        "........beffab..",
        ".........bbbb...",
        "..aaaa...aeb....",
        "aaeeeeaaaefb....",
        "eebbbfeeefb.....",
        "bb...bbbbb......",
    ],
};

#[rustfmt::skip]
const WIN95_POWER_CHARGING: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0xFFFFFF), (b'd', 0xC0C0C0), (b'e', 0x808000), (b'f', 0xFFFF00), (b'g', 0xFF0000)],
    rows: &[
        ".........aaab...",
        "......aaacdabbb.",
        ".abeabecaddabdab",
        ".efeefeaaaaaaaab",
        ".efaefabcdddddab",
        "aaaaaaaabcgdddab",
        "acdddddabgggddab",
        "abbbbbbbcdgdddab",
        ".acccdabcdddddab",
        ".acdddabfdfdfafb",
        "..bddabfdfdfdfab",
        "...bbbfcfdfdfafb",
        "...acbcfdfdfdfab",
        "aaacdbfcfgggfafb",
        "accdb.bfdfdfdfb.",
        "bbbb...bbbbbbb..",
    ],
};

#[rustfmt::skip]
const WIN95_BATTERY_HIGH: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0xFFFFFF), (b'd', 0xC0C0C0), (b'e', 0x000080), (b'f', 0x0000FF), (b'g', 0xFF0000)],
    rows: &[
        "......aaab......",
        "...aaacdabbb....",
        "..aacaddabdab...",
        "..adaaaaaaaeb...",
        "..adefefefeeb...",
        "..adfefgfefeb...",
        "..adefgggfeeb...",
        "..adfefgfefeb...",
        "..adefefefeeb...",
        "..adfefefefeb...",
        "..adefefefeeb...",
        "..adfefefefeb...",
        "..adefefefeeb...",
        "..aafegggefeb...",
        "...beeeeeeeb....",
        "....bbbbbbb.....",
    ],
};

#[rustfmt::skip]
const WIN95_BATTERY_MEDIUM: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0xFFFFFF), (b'd', 0xC0C0C0), (b'e', 0xFF0000), (b'f', 0x000080), (b'g', 0x0000FF)],
    rows: &[
        "......aaab......",
        "...aaacdabbb....",
        "..aacaddabdab...",
        "..acaaaaaaaab...",
        "..acdddddddab...",
        "..acdddedddab...",
        "..acddeeeddab...",
        "..acdddedddab...",
        "..ffdddddddfb...",
        "..ffffffffffb...",
        "..fgfgfgfgffb...",
        "..ffgfgfgfgfb...",
        "..fgfgfgfgffb...",
        "..ffgfeeefgfb...",
        "...bfffffffb....",
        "....bbbbbbb.....",
    ],
};

#[rustfmt::skip]
const WIN95_BATTERY_LOW: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0xFFFFFF), (b'd', 0xC0C0C0), (b'e', 0x000080), (b'f', 0x0000FF)],
    rows: &[
        "......aaab......",
        "...aaacdabbb....",
        "..aacaddabdab...",
        "..acaaaaaaaab...",
        "..acdddddddab...",
        "..acdddbdddab...",
        "..acddbbbddab...",
        "..acdddbdddab...",
        "..acdddddddab...",
        "..acdddddddab...",
        "..acdddddddab...",
        "..acddbbbddab...",
        "..eeadddddaeb...",
        "..beeeeeeeeeb...",
        "...befefefeb....",
        "....bbbbbbb.....",
    ],
};

#[rustfmt::skip]
const WIN95_BATTERY_UNKNOWN: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0xFFFFFF), (b'd', 0xC0C0C0), (b'e', 0x808000), (b'f', 0xFFFF00), (b'g', 0xFF0000)],
    rows: &[
        "......aaab......",
        "...aaacdabbb....",
        "..eadaddabdeb...",
        "..efaaaaeeeab...",
        "..ecfdfdfdfab...",
        "..efcggggfafb...",
        "..ecggfdggfab...",
        "..efggdfggafb...",
        "..ecfdfggdfab...",
        "..efcfggdfafb...",
        "..ecfdfdfdfab...",
        "..efcfggdfafb...",
        "..ecfdggfdfab...",
        "..efcfdfdfafb...",
        "...bfafafafb....",
        "....bbbbbbb.....",
    ],
};

#[rustfmt::skip]
const WIN98_POWER_AC: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x000000), (b'b', 0xFFFFFF), (b'c', 0x808080), (b'd', 0xC0C0C0), (b'e', 0x808000), (b'f', 0xFFFF00)],
    rows: &[
        "a...............",
        "ba..............",
        "cba.............",
        "cba.............",
        "cba.............",
        "cba.............",
        "cba.............",
        "cbda............",
        "ccbdaaaaaaaa....",
        ".ccdbdbdbdbca...",
        "..cddbdbdbaaa...",
        "..cddddccaabee..",
        "..cccddcabeedfee",
        "...aacccaddfedfa",
        ".....aacaaadfeaa",
        ".......aaa.aaa..",
    ],
};

#[rustfmt::skip]
const WIN98_POWER_CHARGING: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x000000), (b'b', 0x868686), (b'c', 0xFFFFFF), (b'd', 0x040404), (b'e', 0x808080), (b'f', 0xFFFF66), (b'g', 0xC0C0C0), (b'h', 0xFF0000), (b'i', 0xFFFF00), (b'j', 0x808000), (b'k', 0xFFCC66), (b'l', 0x800000), (b'm', 0x777777), (b'n', 0xE3E3E3), (b'o', 0xB2B2B2), (b'p', 0x4D4D4D), (b'q', 0xEAEAEA), (b'r', 0xCCCCCC), (b's', 0xD7D7D7), (b't', 0xFFFF99), (b'u', 0xA0A0A4), (b'v', 0x999999), (b'w', 0x666600)],
    rows: &[
        "a......bbb......",
        "ca...bbcccbd....",
        "eca..bfghijd....",
        "eca..bkhlljd....",
        "eca..bfihijd....",
        "eca..mngobpd....",
        "eca..bqggbpd....",
        "ecga.bnggbpd....",
        "eecgaddddddd....",
        ".eegcnnnnnrbd...",
        "..eggsrsrsddd...",
        "..eggggmbddtjj..",
        "..eeeuvvdtwjgtjj",
        "...aabbvdgttjotd",
        ".....ddbdddoijdd",
        ".......ddd.ddd..",
    ],
};

#[rustfmt::skip]
const WIN2000_POWER_CHARGING: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0x000000), (b'c', 0xFFFFFF), (b'd', 0xC0C0C0), (b'e', 0x808000), (b'f', 0xFFFF00), (b'g', 0xFF0000)],
    rows: &[
        ".........aaab...",
        "......aaacdabbb.",
        ".abeabecaddabdab",
        ".efeefeaaaaaaaab",
        ".efaefabcdddddab",
        "aaaaaaaabcgdddab",
        "acdddddabgggddab",
        "abbbbbbbcdgdddab",
        ".acccdabcdddddab",
        ".acdddabfdfdfafb",
        "..bddabfdfdfdfab",
        "...bbbfcfdfdfafb",
        "...acbcfdfdfdfab",
        "aaacdbfcfgggfafb",
        "accdb.bfdfdfdfb.",
        "bbbb...bbbbbbb..",
    ],
};

#[rustfmt::skip]
const WIN98_BATTERY_HIGH: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0xFFFFFF), (b'c', 0x000000), (b'd', 0xC0C0C0), (b'e', 0x000080), (b'f', 0x0000FF), (b'g', 0x008080), (b'h', 0xFF0000)],
    rows: &[
        "......aa........",
        "....aabcac......",
        "...abdddddec....",
        "...fggfffeec....",
        "...ffgfhffec....",
        "...fgghhheec....",
        "...ffgfhffec....",
        "...fggfffeec....",
        "...ffgffffec....",
        "...fggfffeec....",
        "...ffgffffec....",
        "...fggfffeec....",
        "...ffghhhfec....",
        "...cggfffec.....",
        "....cccccc......",
        "................",
    ],
};

#[rustfmt::skip]
const WIN98_BATTERY_MEDIUM: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0xFFFFFF), (b'c', 0x000000), (b'd', 0xC0C0C0), (b'e', 0xFF0000), (b'f', 0x0000FF), (b'g', 0x008080), (b'h', 0x000080)],
    rows: &[
        ".......aa.......",
        ".....aabcac.....",
        "...aabdddddcc...",
        "...adbbdddaac...",
        "...addbdedaac...",
        "...adbbeeeaac...",
        "...addbdedaac...",
        "...afgbdddahc...",
        "...affgffffhc...",
        "...afggfffhhc...",
        "...affgffffhc...",
        "...afggfffhhc...",
        "...affgeeefhc...",
        "....cggfffhc....",
        ".....cccccc.....",
        "................",
    ],
};

#[rustfmt::skip]
const WIN98_BATTERY_LOW: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0xFFFFFF), (b'c', 0x000000), (b'd', 0xC0C0C0), (b'e', 0x0000FF), (b'f', 0x000080), (b'g', 0x008080)],
    rows: &[
        ".......aa.......",
        ".....aabcac.....",
        "...aabdddddcc...",
        "...adbbdddaac...",
        "...addbdcdaac...",
        "...adbbcccaac...",
        "...addbdcdaac...",
        "...adbbdddaac...",
        "...addbdddaac...",
        "...adbbdddaac...",
        "...addbdddaac...",
        "...aebbdddafc...",
        "...aeegcccefc...",
        "....cggeeefc....",
        ".....cccccc.....",
        "................",
    ],
};

#[rustfmt::skip]
const WIN98_BATTERY_UNKNOWN: Rows = Rows {
    w: 16,
    h: 16,
    palette: &[(b'a', 0x808080), (b'b', 0xFFFFFF), (b'c', 0x000000), (b'd', 0xC0C0C0), (b'e', 0xFF0000), (b'f', 0x808000), (b'g', 0xFFFF00)],
    rows: &[
        ".......aa.......",
        ".....aabcac.....",
        "...aabdddddcc...",
        "...adbeeeedfc...",
        "...adeedgeefc...",
        "...adeegdeefc...",
        "...addedgeefc...",
        "...adbbgeedfc...",
        "...addbeeddfc...",
        "...adbbeegdfc...",
        "...addbdgddfc...",
        "...adbbeegdfc...",
        "...afdbeeddfc...",
        "....cffffffc....",
        ".....cccccc.....",
        "................",
    ],
};

#[rustfmt::skip]
pub(crate) const ENTRIES: &[Entry] = &[
    Entry { what: "tray/net-idle", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_NET_IDLE) },
    Entry { what: "tray/net-send", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_NET_SEND) },
    Entry { what: "tray/net-receive", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_NET_RECEIVE) },
    Entry { what: "tray/net-both", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_NET_BOTH) },
    Entry { what: "tray/net-off", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_NET_OFF) },
    Entry { what: "tray/net-idle", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_NET_IDLE) },
    Entry { what: "tray/net-send", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_NET_SEND) },
    Entry { what: "tray/net-receive", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_NET_RECEIVE) },
    Entry { what: "tray/net-both", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_NET_BOTH) },
    Entry { what: "tray/net-off", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_NET_OFF) },
    Entry { what: "tray/net-idle", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&WIN2000_NET_IDLE) },
    Entry { what: "tray/net-send", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&WIN2000_NET_SEND) },
    Entry { what: "tray/net-receive", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&WIN2000_NET_RECEIVE) },
    Entry { what: "tray/net-both", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&WIN2000_NET_BOTH) },
    Entry { what: "tray/net-off", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&WIN2000_NET_OFF) },
    Entry { what: "tray/power-ac", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_POWER_AC) },
    Entry { what: "tray/power-charging", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_POWER_CHARGING) },
    Entry { what: "tray/battery-high", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_BATTERY_HIGH) },
    Entry { what: "tray/battery-medium", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_BATTERY_MEDIUM) },
    Entry { what: "tray/battery-low", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_BATTERY_LOW) },
    Entry { what: "tray/battery-unknown", looks: &[Look::Win95], depth: Depth::TrueColor, art: Art::Rows(&WIN95_BATTERY_UNKNOWN) },
    Entry { what: "tray/power-ac", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_POWER_AC) },
    Entry { what: "tray/power-charging", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_POWER_CHARGING) },
    Entry { what: "tray/power-charging", looks: &[Look::Win2000], depth: Depth::TrueColor, art: Art::Rows(&WIN2000_POWER_CHARGING) },
    Entry { what: "tray/battery-high", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_BATTERY_HIGH) },
    Entry { what: "tray/battery-medium", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_BATTERY_MEDIUM) },
    Entry { what: "tray/battery-low", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_BATTERY_LOW) },
    Entry { what: "tray/battery-unknown", looks: &[Look::Win98], depth: Depth::TrueColor, art: Art::Rows(&WIN98_BATTERY_UNKNOWN) },
];

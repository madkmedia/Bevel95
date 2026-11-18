//! Bevel95's silver B, drawn independently at both classic Windows icon sizes.

use crate::fb::Icon;
use crate::icons::ClassicIcon;

// The highlights face up and left. The navy keyline falls below and to the right.
const PALETTE: &[(u8, u32)] =
    &[(b'W', 0xFFFFFF), (b'S', 0xC0C0C0), (b'G', 0x808080), (b'K', 0x000000), (b'N', 0x000080)];

const SMALL: ClassicIcon = ClassicIcon {
    size: 16,
    palette: PALETTE,
    rows: &[
        "..WWWWWWWWWN....",
        ".WSSSSSSSSSGN...",
        ".WSSSSSSSSSSGN..",
        ".WSSGKKKKGSSSGN.",
        ".WSSK....WSSSGN.",
        ".WSSK....WSSSGN.",
        ".WSSWWWWWSSSGN..",
        ".WSSSSSSSSSGN...",
        ".WSSSSSSSSSSGN..",
        ".WSSGKKKKGSSSGN.",
        ".WSSK....WSSSGN.",
        ".WSSK....WSSSGN.",
        ".WSSWWWWWSSSSGN.",
        ".WSSSSSSSSSSGN..",
        ".GGGGGGGGGGGN...",
        "..NNNNNNNNNN....",
    ],
};

const LARGE: ClassicIcon = ClassicIcon {
    size: 32,
    palette: PALETTE,
    rows: &[
        "................................",
        "...WWWWWWWWWWWWWWWWWWWWN........",
        "..WWWWWWWWWWWWWWWWWWWWWGN.......",
        "..WWSSSSSSSSSSSSSSSSSSSGGN......",
        "..WWSSSSSSSSSSSSSSSSSSSSSGN.....",
        "..WWSSSSSGGGGGGGGGSSSSSSSSGN....",
        "..WWSSSSSGKKKKKKKKGSSSSSSSGGN...",
        "..WWSSSSSGK.......KGSSSSSSSGN...",
        "..WWSSSSSGK........WSSSSSSSGN...",
        "..WWSSSSSGK........WSSSSSSSGN...",
        "..WWSSSSSGK........WSSSSSSSGN...",
        "..WWSSSSSGK.......WWSSSSSSSGN...",
        "..WWSSSSSGK.......WSSSSSSSGGN...",
        "..WWSSSSSSWWWWWWWWSSSSSSSGGN....",
        "..WWSSSSSSSSSSSSSSSSSSSSGGN.....",
        "..WWSSSSSSSSSSSSSSSSSSSGGN......",
        "..WWSSSSSSSSSSSSSSSSSSSSGN......",
        "..WWSSSSSGGGGGGGGGSSSSSSSGN.....",
        "..WWSSSSSGKKKKKKKKGSSSSSSSGN....",
        "..WWSSSSSGK.......KGSSSSSSSGN...",
        "..WWSSSSSGK........WSSSSSSSGGN..",
        "..WWSSSSSGK........WSSSSSSSSGN..",
        "..WWSSSSSGK........WSSSSSSSSGN..",
        "..WWSSSSSGK........WSSSSSSSSGN..",
        "..WWSSSSSGK.......WWSSSSSSSGGN..",
        "..WWSSSSSSWWWWWWWWWSSSSSSSSGN...",
        "..WWSSSSSSSSSSSSSSSSSSSSSSGGN...",
        "..WWSSSSSSSSSSSSSSSSSSSSSGGN....",
        "..WWSSSSSSSSSSSSSSSSSSSSGGN.....",
        "..WGGGGGGGGGGGGGGGGGGGGGGN......",
        "...NNNNNNNNNNNNNNNNNNNNNNN......",
        "................................",
    ],
};

/// The application's small (16) or large (32) Windows icon.
pub fn icon(size: i32) -> Icon {
    if size == 32 { LARGE.icon() } else { SMALL.icon() }
}

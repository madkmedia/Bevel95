//! Display Properties' Background page and the classic desktop's wallpapers: each look's BMP
//! wallpapers as its Reference PC had them in its Windows folder, its patterns, the little
//! monitor and the wallpaper list's small pictures.
//!
//! The wallpapers (`art/assets/wallpapers/*.wlp`) are the PCs' own files
//! (`refs/files/<pc>/wallpapers`), pixel for pixel, kept in a compact form: the colors the picture
//! uses, then its pixels packed at the fewest bits that hold that many colors, run-length coded
//! where that is shorter (`private/probes/display/build72/convert.ps1`, which checks each one back
//! against the file). The eleven pictures that Windows 95 and 98 both had are the same bytes on
//! both disks and answer for both looks. Windows 2000's twelve JPEG pictures are here as the
//! files they are (decoded by Windows when chosen); 98's HTML page and its Desktop Themes
//! pictures are not (Active Desktop), nor 2000's Sample (the PC user's own picture).
//!
//! Each entry's `what` is `wallpaper/<the name in the look's list>`; the core keeps each look's
//! list and its order.

use crate::Depth::TrueColor;
use crate::Look::{Win95, Win98, Win2000};
use crate::{Art, EVERY_LOOK, Entry, Look};

const NOT_2000: &[Look] = &[Win95, Win98];

macro_rules! file {
    ($what:literal, $looks:expr, $file:literal) => {
        Entry { what: $what, looks: $looks, depth: TrueColor, art: Art::Bytes(include_bytes!(concat!("../assets/", $file))) }
    };
}

/// The patterns of Windows 95 and 98 (the same 19 in both: `[Patterns]` of CONTROL.INI on both
/// disks), one per line: the name, a tab, the eight numbers (rows top to bottom, bit 7 the left
/// pixel). In the list's order, without "(None)".
const PATTERNS_9X: &str = "\
Bricks\t187 95 174 93 186 117 234 245
Buttons\t170 125 198 71 198 127 190 85
Cargo Net\t120 49 19 135 225 200 140 30
Circuits\t82 41 132 66 148 41 66 132
Cobblestones\t40 68 146 171 214 108 56 16
Colosseum\t130 1 1 1 171 85 170 85
Daisies\t30 140 216 253 191 27 49 120
Dizzy\t62 7 225 7 62 112 195 112
Field Effect\t86 89 166 154 101 149 106 169
Key\t254 2 250 138 186 162 190 128
Live Wire\t239 239 14 254 254 254 224 239
Plaid\t240 240 240 240 170 85 170 85
Rounder\t215 147 40 215 40 147 213 215
Scales\t225 42 37 146 85 152 62 247
Stone\t174 77 239 255 8 77 174 77
Thatches\t248 116 34 71 143 23 34 113
Tile\t69 130 1 0 1 130 69 170
Triangles\t135 7 6 4 0 247 231 199
Waffle's Revenge\t77 154 8 85 239 154 77 154
";

/// Windows 2000's 13 patterns (`HKCU\Control Panel\Patterns` of its Administrator and its Default
/// User, the same in both), as above.
const PATTERNS_2000: &str = "\
50% Gray\t170 85 170 85 170 85 170 85
Boxes\t127 65 65 65 65 65 127 0
Critters\t0 80 114 32 0 5 39 2
Diamonds\t32 80 136 80 32 0 0 0
Paisley\t2 7 7 2 32 80 80 32
Pattern\t224 128 142 136 234 10 14 0
Quilt\t130 68 40 17 40 68 130 1
Scottie\t64 192 200 120 120 72 0 0
Spinner\t20 12 200 121 158 19 48 40
Thatches\t248 116 34 71 143 23 34 113
Tulip\t0 0 84 124 124 56 146 124
Waffle\t0 0 0 0 128 128 128 240
Weave\t136 84 34 69 136 21 34 81
";

#[rustfmt::skip]
pub(crate) const ENTRIES: &[Entry] = &[
    Entry { what: "display/patterns", looks: NOT_2000, depth: TrueColor, art: Art::Text(PATTERNS_9X) },
    Entry { what: "display/patterns", looks: &[Win2000], depth: TrueColor, art: Art::Text(PATTERNS_2000) },
    // The Background page's little monitor, 184x170: the same picture in all three (98's capture,
    // `vm/win98-truecolor-display-background` at 110,60; see-through where 98 shows its face and
    // 2000 its own at the same place). Its screen (152x112 at 16,17) shows 98's desktop color.
    file!("display/monitor", EVERY_LOOK, "display/monitor.wlp"),
    // The wallpaper list's small pictures (98 and 2000 list views), 16x16 RGB on the list's white,
    // plain and as drawn selected in the list with the keyboard: "(None)"'s sign and a BMP file's
    // picture (98's `display-background` and the walk `w98walk-02`; 2000's `display-background`,
    // `w2kwalk-01` and `-02`; each list's inside at its PC's place, the picture 2 in).
    file!("display/none", &[Win98], "display/win98-none.rgb"),
    file!("display/none-selected", &[Win98], "display/win98-none-selected.rgb"),
    file!("display/bitmap", &[Win98], "display/win98-bitmap.rgb"),
    file!("display/bitmap-selected", &[Win98], "display/win98-bitmap-selected.rgb"),
    file!("display/none", &[Win2000], "display/win2000-none.rgb"),
    file!("display/none-selected", &[Win2000], "display/win2000-none-selected.rgb"),
    file!("display/bitmap", &[Win2000], "display/win2000-bitmap.rgb"),
    file!("display/bitmap-selected", &[Win2000], "display/win2000-bitmap-selected.rgb"),
    // Windows 95's and 98's wallpapers both had (WINDOWS\<name>.bmp, the same bytes on both disks).
    file!("wallpaper/Black Thatch", NOT_2000, "wallpapers/black-thatch.wlp"),
    file!("wallpaper/Blue Rivets", NOT_2000, "wallpapers/blue-rivets.wlp"),
    file!("wallpaper/Bubbles", NOT_2000, "wallpapers/bubbles.wlp"),
    file!("wallpaper/Carved Stone", NOT_2000, "wallpapers/carved-stone.wlp"),
    file!("wallpaper/Circles", NOT_2000, "wallpapers/circles.wlp"),
    file!("wallpaper/Houndstooth", NOT_2000, "wallpapers/houndstooth.wlp"),
    file!("wallpaper/Pinstripe", NOT_2000, "wallpapers/pinstripe.wlp"),
    file!("wallpaper/Straw Mat", NOT_2000, "wallpapers/straw-mat.wlp"),
    file!("wallpaper/Tiles", NOT_2000, "wallpapers/tiles.wlp"),
    file!("wallpaper/Triangles", NOT_2000, "wallpapers/triangles.wlp"),
    file!("wallpaper/Waves", NOT_2000, "wallpapers/waves.wlp"),
    // Windows 95's own: its Setup (640x480, 1 bit), another picture than 98's of the same name.
    file!("wallpaper/Setup", &[Win95], "wallpapers/win95-setup.wlp"),
    // Windows 98's own.
    file!("wallpaper/1stboot", &[Win98], "wallpapers/win98-1stboot.wlp"),
    file!("wallpaper/Clouds", &[Win98], "wallpapers/win98-clouds.wlp"),
    file!("wallpaper/Forest", &[Win98], "wallpapers/win98-forest.wlp"),
    file!("wallpaper/Gold Weave", &[Win98], "wallpapers/win98-gold-weave.wlp"),
    file!("wallpaper/Metal Links", &[Win98], "wallpapers/win98-metal-links.wlp"),
    file!("wallpaper/Red Blocks", &[Win98], "wallpapers/win98-red-blocks.wlp"),
    file!("wallpaper/Sandstone", &[Win98], "wallpapers/win98-sandstone.wlp"),
    file!("wallpaper/Setup", &[Win98], "wallpapers/win98-setup.wlp"),
    file!("wallpaper/Stitches", &[Win98], "wallpapers/win98-stitches.wlp"),
    // Windows 2000's BMP wallpapers (WINNT\<name>.bmp).
    file!("wallpaper/Blue Lace 16", &[Win2000], "wallpapers/win2000-blue-lace-16.wlp"),
    file!("wallpaper/Coffee Bean", &[Win2000], "wallpapers/win2000-coffee-bean.wlp"),
    file!("wallpaper/FeatherTexture", &[Win2000], "wallpapers/win2000-feathertexture.wlp"),
    file!("wallpaper/Gone Fishing", &[Win2000], "wallpapers/win2000-gone-fishing.wlp"),
    file!("wallpaper/Greenstone", &[Win2000], "wallpapers/win2000-greenstone.wlp"),
    file!("wallpaper/Prairie Wind", &[Win2000], "wallpapers/win2000-prairie-wind.wlp"),
    file!("wallpaper/Rhododendron", &[Win2000], "wallpapers/win2000-rhododendron.wlp"),
    file!("wallpaper/River Sumida", &[Win2000], "wallpapers/win2000-river-sumida.wlp"),
    file!("wallpaper/Santa Fe Stucco", &[Win2000], "wallpapers/win2000-santa-fe-stucco.wlp"),
    file!("wallpaper/Soap Bubbles", &[Win2000], "wallpapers/win2000-soap-bubbles.wlp"),
    file!("wallpaper/Zapotec", &[Win2000], "wallpapers/win2000-zapotec.wlp"),
    // Windows 2000's JPEG wallpapers (WINNT\Web\Wallpaper\<name>.jpg), the files as they are:
    // the bar decodes them through Windows when one is chosen. Its Sample (the PC user's own
    // picture in My Pictures) is not Windows' and stays out.
    file!("wallpaper/Boiling Point", &[Win2000], "wallpapers/win2000-boiling-point.jpg"),
    file!("wallpaper/Chateau", &[Win2000], "wallpapers/win2000-chateau.jpg"),
    file!("wallpaper/Fall Memories", &[Win2000], "wallpapers/win2000-fall-memories.jpg"),
    file!("wallpaper/Fly Away", &[Win2000], "wallpapers/win2000-fly-away.jpg"),
    file!("wallpaper/Gold Petals", &[Win2000], "wallpapers/win2000-gold-petals.jpg"),
    file!("wallpaper/Ocean Wave", &[Win2000], "wallpapers/win2000-ocean-wave.jpg"),
    file!("wallpaper/Paradise", &[Win2000], "wallpapers/win2000-paradise.jpg"),
    file!("wallpaper/Purple Sponge", &[Win2000], "wallpapers/win2000-purple-sponge.jpg"),
    file!("wallpaper/Snow Trees", &[Win2000], "wallpapers/win2000-snow-trees.jpg"),
    file!("wallpaper/Solar Eclipse", &[Win2000], "wallpapers/win2000-solar-eclipse.jpg"),
    file!("wallpaper/Water Color", &[Win2000], "wallpapers/win2000-water-color.jpg"),
    file!("wallpaper/Windows 2000", &[Win2000], "wallpapers/win2000-windows-2000.jpg"),
    // A picture file's small picture in the wallpaper list (a JPEG of Windows 2000's own; on 98 a
    // JPEG chosen with Browse...): plain and selected. 98's from `display-bg-browsed-jpg1024-tile`
    // (plain: the list without the keyboard under the Active Desktop question) and `-center`
    // (selected) at 104,421; 2000's from `display-bg-windows2000-center` at 138,409 (Snow Trees;
    // Solar Eclipse and Water Color the same) and 138,477 (Windows 2000, selected).
    file!("display/picture", &[Win98], "display/win98-picture.rgb"),
    file!("display/picture-selected", &[Win98], "display/win98-picture-selected.rgb"),
    file!("display/picture", &[Win2000], "display/win2000-picture.rgb"),
    file!("display/picture-selected", &[Win2000], "display/win2000-picture-selected.rgb"),
];

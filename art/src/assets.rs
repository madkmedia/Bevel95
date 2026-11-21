//! The pictures, sounds and fonts kept as files (`art/assets`): cut from the Reference PCs'
//! screens and from the files of Windows and its programs.

use crate::Depth::{Sixteen, TrueColor, TwoFiftySix};
use crate::Look::{Win95, Win98, Win2000};
use crate::{Art, EVERY_LOOK, Entry, Look};

const NOT_2000: &[Look] = &[Win95, Win98];
const HTML_HELP: &[Look] = &[Win98, Win2000];

macro_rules! file {
    ($what:literal, $looks:expr, $depth:expr, $file:literal) => {
        Entry { what: $what, looks: $looks, depth: $depth, art: Art::Bytes(include_bytes!(concat!("../assets/", $file))) }
    };
}

#[rustfmt::skip]
pub(crate) const ENTRIES: &[Entry] = &[
    // Calculator: CALC.EXE's 16- and 32-pixel icon (the same bytes in all three).
    file!("calculator/icons", EVERY_LOOK, TrueColor, "calculator/icons.rgba"),
    // HTML Help (98, 2000): the viewer's fonts as Windows drew them, its pages' pictures.
    file!("html-help/font-arial-11", HTML_HELP, TrueColor, "html-help/arial-11.glyphs"),
    file!("html-help/font-verdana-11", HTML_HELP, TrueColor, "html-help/verdana-11.glyphs"),
    file!("html-help/font-verdana-11-bold", HTML_HELP, TrueColor, "html-help/verdana-11-bold.glyphs"),
    file!("html-help/font-verdana-11-bold-italic", HTML_HELP, TrueColor, "html-help/verdana-11-bold-italic.glyphs"),
    file!("html-help/font-verdana-13-bold", HTML_HELP, TrueColor, "html-help/verdana-13-bold.glyphs"),
    file!("html-help/font-verdana-14-bold", HTML_HELP, TrueColor, "html-help/verdana-14-bold.glyphs"),
    file!("html-help/support-heading", &[Win2000], TrueColor, "html-help/support-heading.i4"),
    file!("html-help/welcome", &[Win98], TrueColor, "html-help/welcome.palrle"),
    // Taskbar Properties: the preview's pieces and the pictures beside its options. First read
    // in the v86 emulator (Windows 95 with Internet Explorer 4, 98, 2000); since 2026-11-18 every
    // one is the Reference PCs': each piece is where the PCs' own Taskbar Properties shows it
    // (`vm/own-pictures/<pc>-truecolor-props-*.png`, the preview's inside at (37, 243)), and the
    // same as their EXPLORER.EXE's bitmaps: the Start menus large and small are bitmaps 150 and 149
    // (from x 2), the taskbar 151 (2000's with the 3-D grays C0C0C0 and DFDFDF drawn as its D4D0C8,
    // as Windows draws them), the window bitmap 149's top 100 rows from x 212 over bitmap 152 (149
    // holds the auto-hide outline there); the taskbar without clock is 151 with the clock painted
    // away by Windows (cut from the `-props-noclock` captures); Customize, Documents and Clear are
    // icons 107, 108 (on the face color) and the Advanced page's picture. The "ie4-" files are
    // Windows 98's (Internet Explorer 4's shell, as the v86 emulator's Windows 95 had it); plain
    // Windows 95 has its own Start menus and taskbar ("w95-"; its window, Customize and Documents
    // are the same as 98's).
    file!("properties/preview-window", NOT_2000, TrueColor, "ie4-preview-window.rgb"),
    file!("properties/preview-start-large", &[Win95], TrueColor, "w95-preview-start-large.rgb"),
    file!("properties/preview-start-small", &[Win95], TrueColor, "w95-preview-start-small.rgb"),
    file!("properties/preview-taskbar", &[Win95], TrueColor, "w95-preview-taskbar.rgb"),
    file!("properties/preview-taskbar-no-clock", &[Win95], TrueColor, "w95-preview-taskbar-noclock.rgb"),
    file!("properties/preview-start-large", &[Win98], TrueColor, "ie4-preview-start-large.rgb"),
    file!("properties/preview-start-small", &[Win98], TrueColor, "ie4-preview-start-small.rgb"),
    file!("properties/preview-taskbar", &[Win98], TrueColor, "ie4-preview-taskbar.rgb"),
    file!("properties/preview-taskbar-no-clock", &[Win98], TrueColor, "ie4-preview-taskbar-noclock.rgb"),
    file!("properties/customize-start", NOT_2000, TrueColor, "ie4-customize-start-icon.rgb"),
    file!("properties/documents", NOT_2000, TrueColor, "ie4-documents-icon.rgb"),
    file!("properties/preview-window", &[Win2000], TrueColor, "w2k-preview-window.rgb"),
    file!("properties/preview-start-large", &[Win2000], TrueColor, "w2k-preview-start-large.rgb"),
    file!("properties/preview-start-small", &[Win2000], TrueColor, "w2k-preview-start-small.rgb"),
    file!("properties/preview-taskbar", &[Win2000], TrueColor, "w2k-preview-taskbar.rgb"),
    file!("properties/preview-taskbar-no-clock", &[Win2000], TrueColor, "w2k-preview-taskbar-noclock.rgb"),
    file!("properties/customize-start", &[Win2000], TrueColor, "w2k-customize-start-icon.rgb"),
    file!("properties/clear", &[Win2000], TrueColor, "w2k-clear-icon.rgb"),
    // Minesweeper: the board's squares, faces and digits (each sheet holds every look's), the
    // program's icons, Windows 2000's three sounds.
    file!("minesweeper/tiles", EVERY_LOOK, TrueColor, "minefield/tiles.idx"),
    file!("minesweeper/faces", EVERY_LOOK, TrueColor, "minefield/faces.idx"),
    file!("minesweeper/digits", EVERY_LOOK, TrueColor, "minefield/digits.idx"),
    file!("minesweeper/icons", NOT_2000, TrueColor, "minefield/win95-icons.rgba"),
    file!("minesweeper/icons", &[Win2000], TrueColor, "minefield/win2000-icons.rgba"),
    file!("minesweeper/sound-tick", &[Win2000], TrueColor, "minefield/win2000-tick.pcm"),
    file!("minesweeper/sound-win", &[Win2000], TrueColor, "minefield/win2000-win.pcm"),
    file!("minesweeper/sound-lose", &[Win2000], TrueColor, "minefield/win2000-lose.pcm"),
    // Solitaire: the cards (spots, court cards, backs, empty places, the backs' animations), the
    // program's icons, the System font of its status line.
    file!("solitaire/spots", EVERY_LOOK, TrueColor, "solitaire/spots.bits"),
    file!("solitaire/court", EVERY_LOOK, TrueColor, "solitaire/court.idx4"),
    file!("solitaire/backs", EVERY_LOOK, TrueColor, "solitaire/backs.idx4"),
    file!("solitaire/slots", EVERY_LOOK, TrueColor, "solitaire/slots.idx4"),
    file!("solitaire/animations", EVERY_LOOK, TrueColor, "solitaire/animations.idx4"),
    file!("solitaire/icons", EVERY_LOOK, TrueColor, "solitaire/icons.rgba"),
    file!("solitaire/font-system", EVERY_LOOK, TrueColor, "solitaire/status-font.bin"),
    // Date/Time Properties: the Time Zone page's world maps (2000 shares 98's).
    file!("time-zone/world", &[Win95], TrueColor, "w95-world.bits"),
    file!("time-zone/world", &[Win98, Win2000], TrueColor, "w98-world.bits"),
    // Folder windows: the web view's pictures, 2000's Folder Options pictures (98's and 2000's
    // sheets of toolbar, address bar and drive pictures are in explorer.rs).
    file!("web/left", &[Win98], TrueColor, "web/win98-left.pic"),
    file!("web/logo", &[Win98], TrueColor, "web/win98-logo.pic"),
    file!("web/left", &[Win2000], TrueColor, "web/win2000-left.pic"),
    file!("web/logo", &[Win2000], TrueColor, "web/win2000-logo.pic"),
    file!("folder-options/pictures", &[Win2000], TrueColor, "web/win2000-fo.rgb"),
    // The same at 256 colors, cut from the Reference PCs' 256-color screens (98's watercolor
    // dithered in the browser's halftone colors, 2000's corner picture with its pale colors
    // dithered; where text or the folder's picture covered it, or the panel ended, filled:
    // private/probes/colors256/cut256.ps1), and the throbbers as those Windows drew them there.
    file!("web/left", &[Win98], TwoFiftySix, "web/win98-left-256.pic"),
    file!("web/left", &[Win2000], TwoFiftySix, "web/win2000-left-256.pic"),
    file!("explorer/throbber", &[Win98], TwoFiftySix, "explorer/win98-throbber-256.pic"),
    file!("explorer/throbber", &[Win2000], TwoFiftySix, "explorer/win2000-throbber-256.pic"),
    // The Confirm File Delete windows' pictures.
    file!("confirm/delete-icons", &[Win95], TrueColor, "win95-delete-icons.rgb"),
    file!("confirm/delete-icons", &[Win98], TrueColor, "win98-delete-icons.rgb"),
    file!("confirm/delete-icons", &[Win2000], TrueColor, "win2000-delete-icons.rgb"),
    // Windows 2000's banners: Shut Down Windows (True Color and its own 16-color drawing), the
    // shell's About box (Minesweeper's and Solitaire's About). The True Color Shut Down banner was
    // first cut from GUIdebook's screenshot; since 2026-11-18 it is the Reference PC's
    // (`vm/win2000-truecolor-shutdown.png` at 194, 132), found identical.
    file!("dialog/shut-down-banner", &[Win2000], TrueColor, "win2000-shutdown-banner.rgb"),
    file!("dialog/shut-down-banner", &[Win2000], Sixteen, "win2000-shutdown-banner-16.rgb"),
    file!("about/banner", &[Win2000], TrueColor, "win2000-mine-about-banner.rgb"),
];

//! 256 Colors: Windows 95, 98 and 2000 as they looked on a 256-color screen (after the restart
//! each asked for), beside True Color and 16 Colors.
//!
//! Measured on the three Reference PCs at 256 colors (private/probes/colors256/survey.md, the
//! captures `vm/<pc>-256color-*`; docs/theme-spec.md, "256 Colors"). The shell drew its 16-color
//! look there: the 16-color drawings of every icon and toolbar picture, the navy checkerboard on
//! a selected icon, plain title bars, the silver light edge of 95 and 98, 2000's own 16-color Shut
//! Down banner. So a 256-color theme is its look's 16-color theme (`theme::theme_in`), every
//! painter that asks `Depth::reduced` draws it the 16-color way, and `colors16` converts a
//! picture without a drawing of its own by the look's 16-color rule (Windows drew icons in 16
//! colors at this depth too). What this module adds is where 256 colors showed more:
//!
//! - Windows 2000 keeps its own desktop and face colors (3A6EA5, D4D0C8): an area or line in one
//!   of them stays (`kept`); every other color of its theme as in 16 colors.
//! - Windows 2000 dots its pale yellow (FFFFE1: tooltips, the web view's infotip) white and
//!   yellow, two of 16 pixels, as 95 and 98 do in 16 colors but in its own phase; Windows 95 dots
//!   its Help topics' pale yellow (FFFFE2) the same way (`dotted`).
//! - The pictures the browser and the folder windows drew from more colors: 98's web view
//!   watercolor and 2000's corner picture (halftone colors, dithered), both throbbers, the web
//!   view's colored line (True Color's own): `web_picture`, `throbber`, `true_line`.
//! - Windows 2000's focus rectangle inverts the palette's number: on its desktop the dots are
//!   money green (C0DCC0), on its face black (`invert`).

use crate::fb::Icon;
use crate::theme::{Depth, Era, Theme, rgb};

/// Windows 2000's desktop and face colors, which it kept at 256 colors.
pub const KEPT_2000: [u32; 2] = [0x3A6EA5, 0xD4D0C8];

/// True when `theme` is a 256-color one.
pub fn is_256(theme: &Theme) -> bool {
    theme.depth == Depth::TwoFiftySix
}

/// True when `color` stays as it is on `theme`'s screen although it is not one of the 16: Windows
/// 2000's desktop and face at 256 colors.
pub fn kept(theme: &Theme, color: u32) -> bool {
    is_256(theme) && theme.era == Era::Win2000 && KEPT_2000.contains(&(color & 0x00FF_FFFF))
}

/// One pixel of an area or line on `theme`'s screen that is not a picture: kept (`kept`), else by
/// the look's 16-color rule (`colors16::convert`), else (True Color) unchanged.
pub fn area_pixel(theme: &Theme, pixel: u32) -> u32 {
    if !theme.depth.reduced() || kept(theme, pixel) {
        return pixel;
    }
    crate::colors16::convert(theme.era, pixel)
}

/// How a 256-color screen dots an area outside its colors: `base` with `dot` at the two places of
/// every 4×4 block (x mod 4, y mod 4) listed, counted from the corner of what is painted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dots {
    pub base: u32,
    pub dot: u32,
    pub at: [(i32, i32); 2],
}

/// The dotted areas measured at 256 colors (None: as 16 colors, `colors16::fill`).
/// - Windows 2000's FFFFE1: white, yellow at (0, 0) and (2, 2) from the area's corner (the web
///   view's infotip, `vm/win2000-256color-folder-mycomputer-2`; its clock tooltip was not caught:
///   PROVISIONAL there).
/// - Windows 95's FFFFE2 (WinHelp's topic): white, yellow at (0, 1) and (2, 3) from the topic
///   area's corner (`vm/win95-256color-calc-help-topic`).
pub fn dotted(theme: &Theme, color: u32) -> Option<Dots> {
    if !is_256(theme) {
        return None;
    }
    match (theme.era, color & 0x00FF_FFFF) {
        (Era::Win2000, 0xFFFFE1) => Some(Dots { base: rgb(0xFFFFFF), dot: rgb(0xFFFF00), at: [(0, 0), (2, 2)] }),
        (Era::Win95, 0xFFFFE2) => Some(Dots { base: rgb(0xFFFFFF), dot: rgb(0xFFFF00), at: [(0, 1), (2, 3)] }),
        // 98's clock tooltip at 256 colors (`vm/win98-256color-tooltip-clock`); 95's tooltips the
        // same (PROVISIONAL: none caught on its PC, the pointer is not in its captures).
        (Era::Win98 | Era::Win95, 0xFFFFE1) => Some(Dots { base: rgb(0xFFFFFF), dot: rgb(0xFFFF00), at: [(1, 1), (3, 3)] }),
        _ => None,
    }
}

impl Dots {
    /// The color at (x, y) counted from the area's corner.
    pub fn at(&self, x: i32, y: i32) -> u32 {
        let p = (x.rem_euclid(4), y.rem_euclid(4));
        if self.at.contains(&p) { self.dot } else { self.base }
    }
}

/// A focus rectangle's inverted pixel on Windows 2000's 256-color screen, where it differs from 16
/// colors' i -> 15 - i: the desktop's 3A6EA5 becomes money green C0DCC0 (`vm/win2000-256color-
/// desktop-2`, the focus on My Documents), the face D4D0C8 black (`-msgbox-not-found`, its OK).
pub fn invert(theme: &Theme, pixel: u32) -> Option<u32> {
    if !is_256(theme) || theme.era != Era::Win2000 {
        return None;
    }
    match pixel & 0x00FF_FFFF {
        0x3A6EA5 => Some(rgb(0xC0DCC0)),
        0xD4D0C8 => Some(rgb(0x000000)),
        _ => None,
    }
}

/// The web view's corner picture as `era` showed it at 256 colors: 98's watercolor and 2000's
/// picture cut from the Reference PCs' screens.
pub fn web_picture_bytes(era: Era) -> &'static [u8] {
    match era {
        Era::Win2000 => const { crate::art::bytes_at("web/left", crate::art::Look::Win2000, bevel_art::Depth::TwoFiftySix) },
        _ => const { crate::art::bytes_at("web/left", crate::art::Look::Win98, bevel_art::Depth::TwoFiftySix) },
    }
}

/// The throbber at rest as `theme` shows it at 256 colors (98's and 2000's, cut from their
/// 256-color screens), or None (another depth or look: the usual way).
pub fn throbber(theme: &Theme) -> Option<Icon> {
    if !is_256(theme) {
        return None;
    }
    let bytes = match theme.era {
        Era::Win98 => const { crate::art::bytes_at("explorer/throbber", crate::art::Look::Win98, bevel_art::Depth::TwoFiftySix) },
        Era::Win2000 => const { crate::art::bytes_at("explorer/throbber", crate::art::Look::Win2000, bevel_art::Depth::TwoFiftySix) },
        Era::Win95 => return None,
    };
    let fb = crate::folder::web::load_pic(bytes);
    Some(Icon { width: fb.width, height: fb.height, pixels: fb.pixels })
}

/// The pictures a Windows drew at 256 colors with a drawing of its own (neither True Color's nor
/// its 16-color one): the True Color picture (by the function that draws it) and that drawing.
type Own = (Era, fn() -> Option<Icon>, &'static crate::art::Rows);

/// Windows 2000's Minesweeper title-bar picture (`vm/win2000-256color-mine-new`).
static OWN: &[Own] = &[(
    Era::Win2000,
    || Some(crate::minefield::art::icon(Era::Win2000, 16)),
    const { crate::art::rows_at("minesweeper/icon-16", crate::art::Look::Win2000, bevel_art::Depth::TwoFiftySix) },
)];

/// `icon` as `theme` draws it at 256 colors where that Windows had a drawing of its own there.
pub fn own(theme: &Theme, icon: &Icon) -> Option<Icon> {
    if !is_256(theme) {
        return None;
    }
    let (_, _, rows) = OWN.iter().find(|(era, tc, _)| *era == theme.era && tc().is_some_and(|t| t.width == icon.width && t.height == icon.height && t.pixels == icon.pixels))?;
    let pixels = rows
        .rows
        .iter()
        .flat_map(|r| r.bytes())
        .map(|b| rows.palette.iter().find(|(c, _)| *c == b).map_or(0, |&(_, color)| rgb(color)))
        .collect();
    Some(Icon { width: rows.w, height: rows.h, pixels })
}

/// True when the web view's colored line is True Color's own on `theme`'s screen (98 and 2000 at
/// 256 colors: its colors are the browser's halftone colors already; both PCs showed it so).
pub fn true_line(theme: &Theme) -> bool {
    is_256(theme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::theme_in;

    #[test]
    fn windows_2000_keeps_its_desktop_and_face() {
        let t = theme_in(Era::Win2000, Depth::TwoFiftySix);
        assert!(kept(t, rgb(0x3A6EA5)) && kept(t, rgb(0xD4D0C8)));
        assert!(!kept(t, rgb(0x404040)) && !kept(t, rgb(0x0A246A)));
        assert!(!kept(theme_in(Era::Win2000, Depth::Sixteen), rgb(0xD4D0C8)));
        assert!(!kept(theme_in(Era::Win98, Depth::TwoFiftySix), rgb(0xD4D0C8)));
        assert_eq!(area_pixel(t, rgb(0xD4D0C8)), rgb(0xD4D0C8));
        assert_eq!(area_pixel(t, rgb(0x404040)), rgb(0x000000));
        assert_eq!(area_pixel(theme_in(Era::Win2000, Depth::TrueColor), rgb(0x404040)), rgb(0x404040));
    }

    #[test]
    fn the_dots_are_two_in_every_four_by_four() {
        let t = theme_in(Era::Win2000, Depth::TwoFiftySix);
        let d = dotted(t, rgb(0xFFFFE1)).expect("dotted");
        let yellow: Vec<(i32, i32)> = (0..4).flat_map(|y| (0..4).map(move |x| (x, y))).filter(|&(x, y)| d.at(x, y) == rgb(0xFFFF00)).collect();
        assert_eq!(yellow, [(0, 0), (2, 2)]);
        assert!(dotted(theme_in(Era::Win2000, Depth::Sixteen), rgb(0xFFFFE1)).is_none());
        assert!(dotted(theme_in(Era::Win95, Depth::TwoFiftySix), rgb(0xFFFFE2)).is_some());
        let d98 = dotted(theme_in(Era::Win98, Depth::TwoFiftySix), rgb(0xFFFFE1)).expect("98's tooltip");
        assert_eq!((d98.at(1, 1), d98.at(3, 3), d98.at(0, 0)), (rgb(0xFFFF00), rgb(0xFFFF00), rgb(0xFFFFFF)));
    }

    #[test]
    fn the_256_color_pictures_are_whole() {
        for era in [Era::Win98, Era::Win2000] {
            let t = theme_in(era, Depth::TwoFiftySix);
            let th = throbber(t).expect("a throbber");
            assert_eq!((th.width, th.height), (38, 22));
            let web = crate::folder::web::load_pic(web_picture_bytes(era));
            let tc = crate::folder::web::corner_picture(era);
            assert_eq!((web.width, web.height), (tc.width, tc.height));
        }
        assert!(throbber(theme_in(Era::Win98, Depth::Sixteen)).is_none());
        assert!(invert(theme_in(Era::Win2000, Depth::TwoFiftySix), rgb(0x3A6EA5)) == Some(rgb(0xC0DCC0)));
    }
}

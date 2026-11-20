//! Quick Launch: the band of small one-click icons right of the Start button that Internet
//! Explorer 4 brought to Windows 95 and that Windows 98 and 2000 had from the start
//! (docs/decisions.md, "Quick Launch").
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/quick_launch.rs`); this
//! module chooses them.

use crate::art::Look;
use crate::fb::Icon;
use crate::icons::ClassicIcon;
use crate::theme::Era;

/// The icons Bevel95 puts on the band itself (the person's own shortcuts follow them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    /// Starts the person's web browser (98's and 2000's "Launch Internet Explorer Browser").
    InternetExplorer,
    /// Starts the person's mail program ("Launch Outlook Express").
    OutlookExpress,
    /// Windows' own Show Desktop: every window away, and back at the next click.
    ShowDesktop,
    /// Bevel95's own addition: its Windows Explorer, as the Programs menu's entry starts it.
    WindowsExplorer,
}

impl Builtin {
    /// Every built-in icon.
    pub const ALL: [Builtin; 4] = [Builtin::InternetExplorer, Builtin::OutlookExpress, Builtin::ShowDesktop, Builtin::WindowsExplorer];

    /// Its name in quick-launch.txt (the built-in icons the person took off the band).
    pub fn key(self) -> &'static str {
        match self {
            Builtin::InternetExplorer => "internet-explorer",
            Builtin::OutlookExpress => "outlook-express",
            Builtin::ShowDesktop => "show-desktop",
            Builtin::WindowsExplorer => "windows-explorer",
        }
    }

    pub fn from_key(key: &str) -> Option<Builtin> {
        Builtin::ALL.into_iter().find(|b| b.key().eq_ignore_ascii_case(key.trim()))
    }

    /// Its tooltip: the originals' words (Windows 98 on the Reference PC: the names of the files
    /// in its Quick Launch folder); Windows Explorer's is Bevel95's own (PROVISIONAL).
    pub fn tooltip(self) -> &'static str {
        match self {
            Builtin::InternetExplorer => "Launch Internet Explorer Browser",
            Builtin::OutlookExpress => "Launch Outlook Express",
            Builtin::ShowDesktop => "Show Desktop",
            Builtin::WindowsExplorer => "Windows Explorer",
        }
    }
}

/// The built-in icons of `era`, left to right: each look's own order (Windows 98's Internet
/// Explorer, Outlook Express, Show Desktop; Windows 2000's Show Desktop first), then Windows
/// Explorer. Windows 95 had no Quick Launch: switched on, it shows the icons 95's own files have a
/// picture for (its globe for the browser, its desktop for Show Desktop; no mail picture).
pub fn builtins(era: Era) -> &'static [Builtin] {
    use Builtin::*;
    match era {
        Era::Win95 => &[InternetExplorer, ShowDesktop, WindowsExplorer],
        Era::Win98 => &[InternetExplorer, OutlookExpress, ShowDesktop, WindowsExplorer],
        Era::Win2000 => &[ShowDesktop, InternetExplorer, OutlookExpress, WindowsExplorer],
    }
}

/// `builtin`'s picture in `era` (True Color; `colors16` finds its 16-color drawing), `None` where
/// that Windows had none.
pub fn picture(era: Era, builtin: Builtin) -> Option<Icon> {
    use Builtin::*;
    let art = match (era, builtin) {
        (_, WindowsExplorer) => return Some(crate::folder::tree_art::explorer(era)),
        (Era::Win95, ShowDesktop) => return Some(crate::folder::tree_art::w95(crate::folder::tree_art::W95::Desktop)),
        (Era::Win95, InternetExplorer) => &W95_INTERNET,
        (Era::Win95, OutlookExpress) => return None,
        (Era::Win98, InternetExplorer) => &W98_INTERNET,
        (Era::Win98, OutlookExpress) => &W98_OUTLOOK,
        (Era::Win98, ShowDesktop) => &W98_SHOW_DESKTOP,
        (Era::Win2000, InternetExplorer) => &W2K_INTERNET,
        (Era::Win2000, OutlookExpress) => &W2K_OUTLOOK,
        (Era::Win2000, ShowDesktop) => &W2K_SHOW_DESKTOP,
    };
    Some(art.icon())
}

static W95_INTERNET: ClassicIcon = ClassicIcon::art("quick-launch/internet-explorer", Look::Win95);
static W98_INTERNET: ClassicIcon = ClassicIcon::art("quick-launch/internet-explorer", Look::Win98);
static W98_OUTLOOK: ClassicIcon = ClassicIcon::art("quick-launch/outlook-express", Look::Win98);
static W98_SHOW_DESKTOP: ClassicIcon = ClassicIcon::art("quick-launch/show-desktop", Look::Win98);
static W2K_INTERNET: ClassicIcon = ClassicIcon::art("quick-launch/internet-explorer", Look::Win2000);
static W2K_OUTLOOK: ClassicIcon = ClassicIcon::art("quick-launch/outlook-express", Look::Win2000);
static W2K_SHOW_DESKTOP: ClassicIcon = ClassicIcon::art("quick-launch/show-desktop", Look::Win2000);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Depth, theme_in};

    #[test]
    fn every_icon_of_a_look_has_its_pictures() {
        for era in Era::ALL {
            for &b in builtins(era) {
                let icon = picture(era, b).unwrap_or_else(|| panic!("{era:?} {b:?}"));
                assert_eq!((icon.width, icon.height), (16, 16), "{era:?} {b:?}");
                assert!(icon.pixels.iter().any(|p| p >> 24 == 0), "{era:?} {b:?} has see-through pixels");
                // 98 and 2000 have their own 16-color drawing of each of their pictures.
                if era != Era::Win95 {
                    let sixteen = crate::colors16_art::own(era, &icon);
                    assert!(sixteen.is_some(), "{era:?} {b:?} has its 16-color drawing");
                    let _ = theme_in(era, Depth::Sixteen);
                }
            }
        }
        assert!(picture(Era::Win95, Builtin::OutlookExpress).is_none(), "95's files have no mail picture");
    }

    #[test]
    fn each_look_keeps_its_own_order() {
        use Builtin::*;
        assert_eq!(builtins(Era::Win98), [InternetExplorer, OutlookExpress, ShowDesktop, WindowsExplorer]);
        assert_eq!(builtins(Era::Win2000), [ShowDesktop, InternetExplorer, OutlookExpress, WindowsExplorer]);
        for b in Builtin::ALL {
            assert_eq!(Builtin::from_key(b.key()), Some(b));
        }
    }
}

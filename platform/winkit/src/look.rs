//! What a program paints with, as the bar is set up: the look (Windows 95, 98 or 2000), its
//! colors (True Color, 256 Colors or 16 Colors) and the size from Bevel95's settings.txt, and that look's
//! letters (stored in the program: `font::letters`). A program reads it when it
//! starts and again when the bar says its look or size changed (`kit::Hosted::look_changed`).


use bevel_core::config;
use bevel_core::scale::Scale;
use bevel_core::theme::{self, Depth, Era};
use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI, SetProcessDpiAwarenessContext,
};

use crate::font;
use crate::kit::Look;
use crate::log::data_dir;

/// Makes this process draw in real screen pixels (the pictures are enlarged by whole numbers
/// here, never smoothed by Windows). Once, before any window.
pub fn dpi_aware() {
    // SAFETY: a plain call with a predefined constant (failing because it is already set is fine).
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

/// The main screen without the taskbar's strip, and its DPI.
pub fn main_screen() -> (RECT, u32) {
    // SAFETY: valid out-pointers.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(monitor, &mut info);
        let (mut dpi_x, mut dpi_y) = (96, 96);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        (info.rcWork, dpi_x)
    }
}

/// The look and size Bevel95 is set to (settings.txt in Bevel95's folder; the defaults without
/// one), with the look's letters.
pub fn load() -> (Era, Look) {
    let settings = settings();
    let scale = settings.scale_at(main_screen().1);
    (settings.era, for_look(settings.era, settings.colors, scale))
}

fn settings() -> config::Config {
    // Missing or unreadable: the defaults (only read here; the bar keeps the file).
    let text = crate::state_file::read(&data_dir().join("settings.txt")).text().unwrap_or_default();
    xp_as_2000(config::parse(&text).0)
}

/// Under Windows XP's bar style (brief 112) every program shows the 2000 look in True Color.
fn xp_as_2000(mut settings: config::Config) -> config::Config {
    if settings.bar_style == bevel_core::theme::BarStyle::Xp {
        settings.era = Era::Win2000;
        settings.colors = Depth::TrueColor;
    }
    settings
}

/// Tabs in folder windows (brief 110): where the row goes and where its X goes.
pub type FolderTabs = (bevel_core::folder::tabs::Place, bevel_core::folder::tabs::CloseAt);

/// The look and size as `load` gives them, and the folder windows' tabs, from ONE read of
/// settings.txt (the second review's N1). The tabs are None when the file is there but cannot
/// be read, or is empty for a moment (a save caught half way): then nothing is known of them,
/// and a window keeps its tabs as they are; an unreadable file is never taken for "off". (The
/// look itself still falls back to the defaults on such a file, as it always did.)
pub fn load_with_tabs() -> (Era, Look, Option<FolderTabs>) {
    let (settings, tabs) = settings_and_tabs(crate::state_file::read(&data_dir().join("settings.txt")));
    let scale = settings.scale_at(main_screen().1);
    (settings.era, for_look(settings.era, settings.colors, scale), tabs)
}

/// The settings a reading gives, and the tabs when the reading says what they are.
fn settings_and_tabs(read: crate::state_file::Read) -> (config::Config, Option<FolderTabs>) {
    use crate::state_file::Read;
    let known = match &read {
        Read::Missing => true,
        Read::Text(text) => !text.trim().is_empty(),
        Read::Unreadable(_) => false,
    };
    let settings = xp_as_2000(config::parse(&read.text().unwrap_or_default()).0);
    let tabs = known.then_some((settings.folder_tabs, settings.folder_tabs_close));
    (settings, tabs)
}

/// The colors Bevel95 is set to (True Color, 256 Colors or 16 Colors; settings.txt).
pub fn colors() -> Depth {
    settings().colors
}

/// The same letters and theme for a caller that supplies its own era and logical scale (in the
/// colors Bevel95 is set to).
pub fn for_era(era: Era, scale: Scale) -> Look {
    for_look(era, colors(), scale)
}

/// The stored letters and theme of `era` in `depth` colors at `scale`. Nothing here asks Windows:
/// since 0.1.5 (brief 89) the letters are stored in the program, so a look can always be made.
pub fn for_look(era: Era, depth: Depth, scale: Scale) -> Look {
    let theme = theme::theme_in(era, depth);
    let regular = font::letters(theme.font_face, theme.font_px, false).with_period_glyphs();
    let bold = font::letters(theme.font_face, theme.font_px, true).with_period_glyphs();
    Look { theme, regular, bold, scale }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_file::Read;
    use bevel_core::folder::tabs::{CloseAt, Place};

    /// N1 of the second review: a settings.txt that cannot be read (or is empty for a moment)
    /// never says "tabs off"; a missing one is the defaults; a readable one says what it holds,
    /// and the look comes from the same read.
    #[test]
    fn an_unreadable_settings_file_says_nothing_of_the_tabs() {
        let (s, tabs) = settings_and_tabs(Read::Unreadable("not UTF-8".into()));
        assert_eq!(tabs, None);
        assert_eq!(s.era, config::Config::default().era, "the look falls back to the defaults, as before");
        assert_eq!(settings_and_tabs(Read::Text("  \r\n".into())).1, None);
        assert_eq!(settings_and_tabs(Read::Missing).1, Some((Place::Off, CloseAt::End)));
        let text = "era = win2000\nfolder_tabs = above_menu\nfolder_tabs_close = tab\n";
        let (s, tabs) = settings_and_tabs(Read::Text(text.into()));
        assert_eq!(tabs, Some((Place::AboveMenu, CloseAt::Tab)));
        assert_eq!(s.era, Era::Win2000);
        let (_, tabs) = settings_and_tabs(Read::Text("folder_tabs = off\n".into()));
        assert_eq!(tabs, Some((Place::Off, CloseAt::End)), "a file that says off is off");
    }
}

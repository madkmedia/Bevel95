//! The settings file: a few `name = value` lines that a person can edit in Notepad.

use crate::programs::Shown;
use crate::theme::{Depth, Era};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub era: Era,
    /// True Color or 16 Colors (Bevel95 Settings, General page; the bar menu's Bevel95 >
    /// Colors). A file without the line: True Color, as everything was drawn before.
    pub colors: Depth,
    /// A whole-number scale, or `None` for automatic: the nearest whole number to the
    /// Windows display scale (150% -> 2).
    pub scale: Option<i32>,
    pub options: TaskbarOptions,
    pub start: StartOptions,
    /// How many seconds Bevel95's own clock runs ahead of the PC's (behind if negative): what
    /// Date/Time Properties sets. The PC's clock is never changed.
    pub clock_offset: i64,
    /// The time zone Bevel95's clock keeps (a Windows zone name from the Time Zone page's
    /// lists), or `None` for the PC's own; and whether it follows daylight saving time.
    pub clock_zone: Option<&'static str>,
    pub clock_dst: bool,
    /// Bevel95's own desktop is shown: a plain desktop in the look's color over Windows' own
    /// (Bevel95 Settings, Desktop page; the desktop's right-click menu switches it off).
    pub desktop: bool,
    /// What the Programs menu shows (Bevel95 Settings, Programs page).
    pub programs: Shown,
    /// What a lone press of the Windows key opens (Bevel95 Settings, Taskbar page).
    pub windows_key: WindowsKey,
    /// A program's windows share one task button (Bevel95 Settings, Taskbar page: Windows XP's
    /// "Group similar taskbar buttons"); off, one button per window, as in 95, 98 and 2000.
    pub group: bool,
}

/// What a lone press of the Windows key opens while Bevel95's bar shows (`windows_key` in
/// settings.txt). Ctrl+Esc always opens the Start menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowsKey {
    /// Quick Search, Bevel95's own (the setting's word is still `find`, its name until 2026-11-16;
    /// the owner's choice, 2026-11-11: "default should be
    /// windows key").
    #[default]
    Find,
    /// The Start menu, as in Windows 95, 98 and 2000.
    Start,
}

impl WindowsKey {
    pub const ALL: [WindowsKey; 2] = [WindowsKey::Find, WindowsKey::Start];

    /// The name in settings.txt.
    pub fn setting(self) -> &'static str {
        match self {
            WindowsKey::Find => "find",
            WindowsKey::Start => "start",
        }
    }

    pub fn from_setting(value: &str) -> Option<WindowsKey> {
        WindowsKey::ALL.into_iter().find(|key| key.setting().eq_ignore_ascii_case(value.trim()))
    }
}

/// The two Start Menu Settings of Windows 2000 that Bevel95 carries out (its Advanced page).
/// Only the Windows 2000 look uses them: Windows 98's Start menu always has both entries,
/// Windows 95's neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartOptions {
    /// "Display Favorites": a Favorites entry under Programs.
    pub favorites: bool,
    /// "Display Logoff": the "Log Off <user>..." entry.
    pub log_off: bool,
}

impl Default for StartOptions {
    fn default() -> StartOptions {
        StartOptions { favorites: false, log_off: true }
    }
}

/// The four check boxes of Windows' Taskbar Options page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskbarOptions {
    /// Other windows cannot cover the bar, and maximized windows stop above it.
    pub always_on_top: bool,
    /// The bar slides off the screen edge until the pointer touches that edge.
    pub auto_hide: bool,
    /// The Start menu with 16×16 icons and no banner.
    pub small_icons: bool,
    pub show_clock: bool,
}

impl Default for TaskbarOptions {
    fn default() -> TaskbarOptions {
        TaskbarOptions { always_on_top: true, auto_hide: false, small_icons: false, show_clock: true }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            era: Era::Win95,
            colors: Depth::TrueColor,
            scale: Some(DEFAULT_SCALE),
            options: TaskbarOptions::default(),
            start: StartOptions::default(),
            clock_offset: 0,
            clock_zone: None,
            clock_dst: true,
            desktop: true,
            programs: Shown::Everything,
            windows_key: WindowsKey::Find,
            group: true,
        }
    }
}

/// The size used unless the settings say otherwise: double, crisp on most screens.
pub const DEFAULT_SCALE: i32 = 2;

/// The sizes offered in the bar's right-click menu and the Properties window (the file
/// accepts 1 to 8, or auto).
pub const MENU_SCALES: [i32; 4] = [1, 2, 3, 4];

/// What a new settings file contains.
pub const DEFAULT_FILE: &str = "\
# Bevel95 settings. Lines starting with # are ignored.
# Properties and Bevel95 Settings (right-click the bar) change all of these.
# Changes made here take effect when Bevel95 restarts (right-click the bar, Restart).

# Look: win95, win98 or win2000
era = win95

# Colors: true (True Color) or 16 (16 Colors: each look as its Windows looked on a 16-color
# screen, as on a PC without its display driver).
colors = true

# Size: a whole number (1 = original pixel size, 2 = double, ...), or auto
# (the whole number nearest your Windows display scale, e.g. 150% -> 2).
scale = 2

# The Taskbar Options of Windows 95: yes or no.
always_on_top = yes
auto_hide = no
small_icons = no
show_clock = yes

# Windows 2000's Start menu: show Favorites, show Log Off (yes or no).
favorites = no
log_off = yes

# Bevel95's own clock: seconds ahead of the PC's clock (negative: behind). Double-click the
# clock to set it; 0 shows the PC's time. The PC's own clock is never changed.
clock_offset = 0

# The time zone Bevel95's clock keeps (Date/Time Properties, Time Zone page): pc (the PC's own)
# or a Windows zone name such as Eastern Standard Time; and whether it follows daylight saving
# time (yes or no).
clock_zone = pc
clock_dst = yes

# Bevel95's own desktop: a plain desktop in the look's color, over Windows' own (yes or no).
# Your own desktop icons stay underneath, untouched.
desktop = yes

# What the Programs menu shows (Bevel95 Settings, Programs page): everything (every program
# found on this PC), classic (only the classic programs that come with Bevel95) or ticked
# (only the programs ticked on that page; they are listed in programs.txt).
programs = everything

# What a press of the Windows key opens: find (Bevel95's Quick Search: type a few letters, Enter
# starts the program or does the thing found) or start (the Start menu). Ctrl+Esc always opens the Start
# menu.
windows_key = find

# Task buttons: yes gives a program's windows one button (as Windows XP's \"Group similar
# taskbar buttons\"), no gives every window a button of its own (as Windows 95, 98 and 2000).
group = yes
";

/// Where in a `Config` a yes/no setting lives.
type Flag = fn(&mut Config) -> &mut bool;

/// The settings-file names of the yes/no settings, with their field.
const FLAG_KEYS: [(&str, Flag); 8] = [
    ("always_on_top", |c| &mut c.options.always_on_top),
    ("auto_hide", |c| &mut c.options.auto_hide),
    ("small_icons", |c| &mut c.options.small_icons),
    ("show_clock", |c| &mut c.options.show_clock),
    ("favorites", |c| &mut c.start.favorites),
    ("log_off", |c| &mut c.start.log_off),
    ("desktop", |c| &mut c.desktop),
    ("group", |c| &mut c.group),
];

/// The settings-file lines of the yes/no settings in `config`, as (name, value) pairs.
pub fn flag_settings(config: &Config) -> [(&'static str, &'static str); 8] {
    let mut config = *config;
    FLAG_KEYS.map(|(key, field)| (key, if *field(&mut config) { "yes" } else { "no" }))
}

/// Reads a settings file. Lines it doesn't understand are skipped and described in the
/// returned warnings; anything not set keeps its default.
pub fn parse(text: &str) -> (Config, Vec<String>) {
    // A file saved "UTF-8 with BOM" (Notepad, PowerShell's Set-Content -Encoding utf8): the mark
    // is not part of the first setting (the safety review of 2026-11-18, suspected 4).
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut config = Config::default();
    let mut warnings = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut warn = |what: &str| warnings.push(format!("line {}: {what}", n + 1));
        let Some((key, value)) = line.split_once('=') else {
            warn("expected `name = value`");
            continue;
        };
        let (key, value) = (key.trim().to_ascii_lowercase(), value.trim().to_ascii_lowercase());
        if let Some((_, field)) = FLAG_KEYS.iter().find(|(k, _)| *k == key) {
            match value.as_str() {
                "yes" | "true" | "on" | "1" => *field(&mut config) = true,
                "no" | "false" | "off" | "0" => *field(&mut config) = false,
                _ => warn(&format!("{key} must be yes or no")),
            }
            continue;
        }
        match key.as_str() {
            "era" => match value.as_str() {
                "win95" | "95" => config.era = Era::Win95,
                "win98" | "98" => config.era = Era::Win98,
                "win2000" | "2000" => config.era = Era::Win2000,
                _ => warn("era must be win95, win98 or win2000"),
            },
            "colors" => match Depth::from_setting(&value) {
                Some(depth) => config.colors = depth,
                None => warn("colors must be true or 16"),
            },
            "clock_offset" => match value.parse::<i64>() {
                Ok(seconds) => config.clock_offset = seconds,
                Err(_) => warn("clock_offset must be a whole number of seconds"),
            },
            "clock_zone" => match value.as_str() {
                "pc" | "" => config.clock_zone = None,
                name => match crate::timezone::known_key(name) {
                    Some(key) => config.clock_zone = Some(key),
                    None => warn("clock_zone must be pc or a zone of the Time Zone page"),
                },
            },
            "clock_dst" => match value.as_str() {
                "yes" | "true" | "on" | "1" => config.clock_dst = true,
                "no" | "false" | "off" | "0" => config.clock_dst = false,
                _ => warn("clock_dst must be yes or no"),
            },
            "programs" => match Shown::from_setting(&value) {
                Some(shown) => config.programs = shown,
                None => warn("programs must be everything, classic or ticked"),
            },
            "windows_key" => match WindowsKey::from_setting(&value) {
                Some(key) => config.windows_key = key,
                None => warn("windows_key must be find or start"),
            },
            "scale" => match value.as_str() {
                "auto" => config.scale = None,
                number => match number.parse::<i32>() {
                    Ok(s) if (1..=8).contains(&s) => config.scale = Some(s),
                    _ => warn("scale must be auto or a whole number from 1 to 8"),
                },
            },
            _ => warn(&format!("unknown setting `{key}`")),
        }
    }
    (config, warnings)
}

/// `text` with its `key` line set to `value` (added if missing); every other line,
/// comments included, is kept as it was.
pub fn with_setting(text: &str, key: &str, value: &str) -> String {
    let mut found = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| {
            let setting = line.split('#').next().unwrap_or("");
            let is_key = setting.split_once('=').is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(key));
            if is_key && !found {
                found = true;
                format!("{key} = {value}")
            } else {
                line.to_string()
            }
        })
        .collect();
    if !found {
        lines.push(format!("{key} = {value}"));
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_gives_default_config() {
        assert_eq!(parse(DEFAULT_FILE), (Config::default(), vec![]));
        assert_eq!(Config::default().era, Era::Win95);
        assert_eq!(Config::default().scale, Some(2));
        let o = Config::default().options;
        assert!(o.always_on_top && o.show_clock && !o.auto_hide && !o.small_icons);
        assert!(Config::default().desktop, "the desktop is shown unless switched off");
        // A settings file from before the desktop existed has no line for it: it is shown.
        assert!(parse("era = win98\n").0.desktop);
        assert!(!parse("desktop = no\n").0.desktop);
    }

    #[test]
    fn reads_what_the_programs_menu_shows() {
        // A settings file from before the choice existed: everything, as the menu showed.
        assert_eq!(parse("era = win98\n").0.programs, Shown::Everything);
        assert_eq!(parse("programs = Classic\n").0.programs, Shown::Classic);
        assert_eq!(parse("programs = ticked  # my own list\n").0.programs, Shown::Ticked);
        let (c, w) = parse("programs = some\n");
        assert_eq!((c.programs, w.len()), (Shown::Everything, 1));
        for shown in Shown::ALL {
            assert_eq!(parse(&with_setting(DEFAULT_FILE, "programs", shown.setting())).0.programs, shown);
        }
    }

    #[test]
    fn reads_what_the_windows_key_opens() {
        // A settings file from before the setting existed: Find, the owner's default.
        assert_eq!(parse("era = win98\n").0.windows_key, WindowsKey::Find);
        assert_eq!(parse("windows_key = Start\n").0.windows_key, WindowsKey::Start);
        let (c, w) = parse("windows_key = menu\n");
        assert_eq!((c.windows_key, w.len()), (WindowsKey::Find, 1));
        for key in WindowsKey::ALL {
            assert_eq!(parse(&with_setting(DEFAULT_FILE, "windows_key", key.setting())).0.windows_key, key);
        }
    }

    #[test]
    fn reads_the_colors() {
        // A settings file from before the setting existed: True Color, as everything was drawn.
        assert_eq!(parse("era = win98\n").0.colors, Depth::TrueColor);
        assert_eq!(parse("colors = 16\n").0.colors, Depth::Sixteen);
        assert_eq!(parse("COLORS = True  # as today\n").0.colors, Depth::TrueColor);
        let (c, w) = parse("colors = 256\n");
        assert_eq!((c.colors, w.len()), (Depth::TrueColor, 1));
        for depth in Depth::ALL {
            assert_eq!(parse(&with_setting(DEFAULT_FILE, "colors", depth.setting())).0.colors, depth);
        }
    }

    #[test]
    fn reads_whether_buttons_are_grouped() {
        // A settings file from before the setting existed: grouped, as the bar always was.
        assert!(parse("era = win98\n").0.group);
        assert!(!parse("group = no\n").0.group);
        assert!(parse("GROUP = Yes  # one per program\n").0.group);
        let (c, w) = parse("group = sometimes\n");
        assert!(c.group && w.len() == 1);
        assert_eq!(flag_settings(&Config { group: false, ..Config::default() })[7], ("group", "no"));
    }

    #[test]
    fn reads_values_case_and_space_insensitively() {
        let (c, w) = parse("  ERA = Win2000  # my choice\r\nscale=auto\nAuto_Hide = Yes\nshow_clock=off\n");
        assert_eq!((c.era, c.scale), (Era::Win2000, None));
        assert!(c.options.auto_hide && !c.options.show_clock && c.options.always_on_top);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(parse("era = 98").0.era, Era::Win98);
    }

    #[test]
    fn reads_the_clock_zone() {
        let (c, w) = parse("clock_zone = Eastern Standard Time\nclock_dst = no\n");
        assert_eq!((c.clock_zone, c.clock_dst), (Some("Eastern Standard Time"), false));
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(parse("clock_zone = PC").0.clock_zone, None);
        let (c, w) = parse("clock_zone = Mars Standard Time");
        assert_eq!((c.clock_zone, w.len()), (None, 1));
    }

    #[test]
    fn a_byte_order_mark_is_not_part_of_the_first_setting() {
        let (c, w) = parse("\u{feff}era = 98\nscale = 3\n");
        assert_eq!((c.era, c.scale), (Era::Win98, Some(3)));
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn bad_lines_warn_and_keep_defaults() {
        let (c, w) = parse("era = winxp\nscale = 1.5\ncolor = teal\njust words\nsmall_icons = maybe\n");
        assert_eq!(c, Config::default());
        assert_eq!(w.len(), 5, "{w:?}");
        assert!(w[0].starts_with("line 1:") && w[4].starts_with("line 5:"));
    }

    #[test]
    fn with_setting_changes_only_that_line() {
        let text = "# Size: scale = something in a comment\nera = win2000\nscale = auto  # mine\n";
        let changed = with_setting(text, "scale", "3");
        assert_eq!(changed, "# Size: scale = something in a comment\nera = win2000\nscale = 3\n");
        assert_eq!((parse(&changed).0.era, parse(&changed).0.scale), (Era::Win2000, Some(3)));
        assert_eq!(with_setting("era = win95\n", "scale", "1"), "era = win95\nscale = 1\n");
        assert_eq!(parse(&with_setting(DEFAULT_FILE, "scale", "4")).0.scale, Some(4));
        for era in Era::ALL {
            assert_eq!(parse(&with_setting(DEFAULT_FILE, "era", era.setting())).0.era, era);
        }
    }

    #[test]
    fn options_round_trip_through_the_file() {
        let config = Config {
            options: TaskbarOptions { always_on_top: false, auto_hide: true, small_icons: true, show_clock: false },
            start: StartOptions { favorites: true, log_off: false },
            desktop: false,
            ..Config::default()
        };
        let mut text = DEFAULT_FILE.to_string();
        for (key, value) in flag_settings(&config) {
            text = with_setting(&text, key, value);
        }
        assert_eq!(parse(&text), (config, vec![]));
        assert_eq!(flag_settings(&Config::default())[0], ("always_on_top", "yes"));
        assert_eq!(flag_settings(&Config::default())[5], ("log_off", "yes"));
        assert_eq!(flag_settings(&config)[6], ("desktop", "no"));
    }
}

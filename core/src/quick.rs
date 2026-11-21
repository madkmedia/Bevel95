//! Quick Search's things to do (the owner, 2026-11-16: "Eventually I think the search we have
//! should become a kind of 'do anything' modern tool for bevel95"): a table of commands, made to
//! grow, each with its name, the other words it is found by, a small picture and what it does.
//! Quick Search (`find`) lists them beside the programs, in the folder "Bevel95", once something
//! typed matches them. PROVISIONAL: every name and word here.
//!
//! A command that cannot apply right now (the look already in use, the desktop already on) is
//! still listed and does nothing harmful: the bar's own handling of the menu's entries already
//! leaves such a choice alone.

use crate::bar_menu::BarCommand;
use crate::config::MENU_SCALES;
use crate::fb::Icon;
use crate::start::{StartEntry, Target};
use crate::theme::{Depth, Era};
use crate::window_icons::{self, ClassicWindow};

/// What a command does. The platform layer carries it out as the bar's menus do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// A command of the bar (the menu's own, and those only Quick Search offers).
    Bar(BarCommand),
    /// The Theme menu's choice.
    Look(Era),
    /// The Size menu's choice (its way of drawing is the bar's own, not this one's).
    Size(crate::scale::Scale),
    /// The Colors menu's choice: True Color or 16 Colors.
    Colors(Depth),
}

/// One thing Quick Search can do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    /// Other words it is found by (lowercase). Every command is also found by "Bevel95".
    pub words: &'static [&'static str],
    pub action: Action,
    /// What the row's target says (unique, never a file: "bevel95:<what>").
    pub key: String,
}

/// The folder the list shows for a command.
pub const FOLDER: &str = "Bevel95";

/// The words of a look and of a size.
const LOOK_WORDS: &[&str] = &["theme", "look", "style", "classic", "era"];
const SIZE_WORDS: &[&str] = &["size", "scale", "zoom", "bigger", "smaller", "dpi"];
const COLORS_WORDS: &[&str] = &["color", "colors", "colour", "colours", "vga"];

/// Every command, in the table's order (the list ranks them; this order only matters as the
/// last tie-break, which never comes: no two have the same name).
pub fn commands() -> Vec<Command> {
    use BarCommand::*;
    let bar = |name: &str, words: &'static [&'static str], c: BarCommand, key: &str| Command {
        name: name.to_string(),
        words,
        action: Action::Bar(c),
        key: format!("bevel95:{key}"),
    };
    let mut out = vec![
        bar("Bevel95 Settings", &["settings", "options", "preferences", "setup"], Settings, "settings"),
        bar("Taskbar Properties", &["taskbar", "start menu", "properties", "options", "clock"], Properties, "taskbar-properties"),
        bar("Date/Time Properties", &["date", "time", "clock", "time zone", "calendar"], AdjustDateTime, "date-time"),
    ];
    for era in Era::ALL {
        out.push(Command {
            name: format!("{} look", era.title()),
            words: LOOK_WORDS,
            action: Action::Look(era),
            key: format!("bevel95:look-{}", era.setting()),
        });
    }
    for scale in MENU_SCALES {
        out.push(Command {
            name: format!("Size {scale}\u{d7}"),
            words: SIZE_WORDS,
            action: Action::Size(scale),
            key: format!("bevel95:size-{scale}"),
        });
    }
    for depth in Depth::ALL {
        out.push(Command {
            name: depth.title().to_string(),
            words: COLORS_WORDS,
            action: Action::Colors(depth),
            key: format!("bevel95:colors-{}", depth.setting()),
        });
    }
    out.extend([
        bar("Switch back to Windows", &["taskbar", "windows 11", "switch", "restore"], SwitchToWindowsTaskbar, "windows-taskbar"),
        bar("Show the classic desktop", &["desktop", "classic", "on", "wallpaper"], DesktopOn, "desktop-on"),
        bar("Switch to Windows desktop", &["desktop", "classic", "off", "wallpaper"], DesktopOff, "desktop-off"),
        bar("Run...", &["run", "command", "open"], Run, "run"),
        bar("Shut Down...", &["shut down", "turn off", "restart", "log off", "stand by", "power"], ShutDown, "shut-down"),
        bar("Windows Explorer", &["explorer", "files", "folders"], Explorer, "explorer"),
        bar("My Computer", &["computer", "drives", "files", "this pc"], MyComputer, "my-computer"),
        bar("Restart Bevel95", &["restart", "reload"], Restart, "restart"),
        bar("Exit Bevel95", &["exit", "quit", "close"], Exit, "exit"),
        bar(concat!("About ", crate::product_name!()), &["about", "version", "build", "help", "licence", "license"], About, "about"),
    ]);
    out
}

/// The command's small picture in `era`: that look's own picture of the same thing where it had
/// one (Run, Shut Down, the Taskbar, Windows Explorer, My Computer, Control Panel for Date/Time),
/// Bevel95's own silver B for what is Bevel95's own.
pub fn picture(era: Era, action: Action) -> Option<Icon> {
    use BarCommand::*;
    match action {
        Action::Bar(Run) => crate::icons::start_icon_small(era, StartEntry::Run),
        Action::Bar(ShutDown) => crate::icons::start_icon_small(era, StartEntry::ShutDown),
        Action::Bar(Properties | SwitchToWindowsTaskbar) => crate::icons::target_icon(era, Target::BarSettings),
        Action::Bar(AdjustDateTime) => Some(window_icons::icon(era, ClassicWindow::ControlPanel)),
        Action::Bar(Explorer) => Some(crate::folder::tree_art::explorer(era)),
        Action::Bar(MyComputer) => Some(window_icons::icon(era, ClassicWindow::MyComputer)),
        _ => Some(crate::bevel_icon::icon(16)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_holds_each_thing_to_do_once() {
        let all = commands();
        let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Bevel95 Settings",
                "Taskbar Properties",
                "Date/Time Properties",
                "Windows 95 look",
                "Windows 98 look",
                "Windows 2000 look",
                "Size 1\u{d7}",
                "Size 2\u{d7}",
                "Size 3\u{d7}",
                "Size 4\u{d7}",
                "16 Colors",
                "256 Colors",
                "True Color",
                "Switch back to Windows",
                "Show the classic desktop",
                "Switch to Windows desktop",
                "Run...",
                "Shut Down...",
                "Windows Explorer",
                "My Computer",
                "Restart Bevel95",
                "Exit Bevel95",
                "About Bevel95",
            ]
        );
        // Names and keys are unique; keys are never a file.
        let mut keys: Vec<&str> = all.iter().map(|c| c.key.as_str()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), all.len());
        assert!(all.iter().all(|c| c.key.starts_with("bevel95:")));
        // Each look and size the menus offer.
        for era in Era::ALL {
            assert!(all.iter().any(|c| c.action == Action::Look(era)));
        }
        for s in MENU_SCALES {
            assert!(all.iter().any(|c| c.action == Action::Size(s)));
        }
        for d in Depth::ALL {
            assert!(all.iter().any(|c| c.action == Action::Colors(d) && c.words.contains(&"color") && c.words.contains(&"colors")));
        }
        assert!(all.iter().all(|c| c.words.iter().all(|w| *w == w.to_lowercase())));
        // Every command has a picture in every look.
        for era in Era::ALL {
            for c in &all {
                assert!(picture(era, c.action).is_some_and(|i| i.width == 16 && i.height == 16), "{era:?} {}", c.name);
            }
        }
    }
}

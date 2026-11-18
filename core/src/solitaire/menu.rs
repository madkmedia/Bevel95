//! Solitaire's Game and Help menus. Labels, shortcuts and status text come from each
//! edition of SOL.EXE's MENU, ACCELERATOR and STRINGTABLE resources.

use crate::fb::Rect;
use crate::kit::{Key, Mods};
use crate::paint::{Menu, MenuEntry};
use crate::text::Fonts;
use crate::theme::Era;
use crate::window::{self, TitleLook};

pub const MENUS: [&str; 2] = ["&Game", "&Help"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Deal,
    Undo,
    Deck,
    Options,
    Exit,
    HelpTopics,
    SearchHelp,
    UsingHelp,
    About,
}

impl Command {
    /// The text at the left of the status bar while this entry is highlighted.
    pub fn status(self, _era: Era) -> &'static str {
        match self {
            Self::Deal => "Deal a new game",
            Self::Undo => "Undo last action",
            Self::Deck => "Choose new deck back",
            Self::Options => "Change Solitaire options",
            Self::Exit => "Exit Solitaire",
            Self::HelpTopics => "Index of Solitaire help topics",
            Self::SearchHelp => "Search the Help Engine for a specific topic",
            Self::UsingHelp => "Help using help",
            Self::About => "About Solitaire",
        }
    }

    fn label(self, era: Era) -> &'static str {
        match self {
            Self::Deal if era == Era::Win2000 => "&Deal\t F2",
            Self::Deal => "&Deal",
            Self::Undo => "&Undo",
            Self::Deck => "De&ck...",
            Self::Options => "&Options...",
            Self::Exit => "E&xit",
            Self::HelpTopics if era == Era::Win2000 => "&Contents\t F1",
            Self::HelpTopics => "&Help Topics",
            Self::SearchHelp => "&Search for Help on...",
            Self::UsingHelp => "&How to Use Help",
            Self::About => "&About Solitaire",
        }
    }
}

use Command::*;
const GAME: &[Option<Command>] = &[Some(Deal), None, Some(Undo), Some(Deck), Some(Options), None, Some(Exit)];
const HELP_9X: &[Option<Command>] = &[Some(HelpTopics), None, Some(About)];
const HELP_2000: &[Option<Command>] = &[Some(HelpTopics), Some(SearchHelp), Some(UsingHelp), None, Some(About)];

fn entries(era: Era, menu: usize) -> &'static [Option<Command>] {
    match menu {
        0 => GAME,
        1 if era == Era::Win2000 => HELP_2000,
        1 => HELP_9X,
        _ => &[],
    }
}

pub fn command_at(era: Era, menu: usize, row: usize) -> Option<Command> {
    entries(era, menu).get(row).copied().flatten()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// The menu bar consumed the key (for example Left selected the previous title).
    Handled,
    Open(usize),
    Run(Command),
    SystemMenu,
}

/// The two titles belong to the game window; the open popup is tracked by the host.
#[derive(Clone, Debug, Default)]
pub struct MenuBar {
    titles: Vec<Rect>,
    hot: Option<usize>,
    chosen: Option<usize>,
    open: bool,
    keyboard: bool,
    keys: bool,
}

impl MenuBar {
    /// Normal and maximized Solitaire frames have different insets. Translate the shared
    /// menu metrics by the same offsets that the caller uses when painting its caption.
    pub fn layout(&mut self, fonts: &Fonts, dx: i32, dy: i32) {
        self.titles = window::menu_bar_layout(fonts, &MENUS);
        for r in &mut self.titles {
            r.x += dx;
            r.y += dy;
        }
    }

    pub fn title_rect(&self, menu: usize) -> Option<Rect> {
        self.titles.get(menu).copied()
    }

    pub fn hit(&self, x: i32, y: i32) -> Option<usize> {
        self.titles.iter().position(|r| r.contains(x, y))
    }

    pub fn looks(&self) -> [TitleLook; 2] {
        std::array::from_fn(|i| match (self.chosen, self.hot) {
            (Some(chosen), _) if chosen == i => TitleLook::Chosen,
            (None, Some(hot)) if hot == i => TitleLook::Hot,
            _ => TitleLook::Plain,
        })
    }

    pub fn show_keys(&self, era: Era) -> bool {
        era != Era::Win2000 || self.keys
    }

    pub fn menu(&self, era: Era, index: usize, can_undo: bool) -> Menu {
        let entries = entries(era, index)
            .iter()
            .map(|entry| match entry {
                Some(cmd) => MenuEntry { disabled: *cmd == Undo && !can_undo, ..MenuEntry::item(cmd.label(era), None) },
                None => MenuEntry::separator(),
            })
            .collect();
        let mut menu = Menu::new(entries);
        menu.show_keys = era != Era::Win2000 || self.keyboard;
        if self.keyboard && !menu.entries.is_empty() {
            menu.hover = Some(0);
        }
        menu
    }

    pub fn hover(&mut self, x: i32, y: i32, era: Era) -> bool {
        let hot = self.hit(x, y);
        std::mem::replace(&mut self.hot, hot) != hot && era == Era::Win2000
    }

    pub fn pointer_left(&mut self, era: Era) -> bool {
        self.hot.take().is_some() && era == Era::Win2000
    }

    pub fn leave(&mut self) {
        self.chosen = None;
        self.open = false;
        self.keyboard = false;
        self.keys = false;
    }

    pub fn show_menu(&mut self, menu: Option<usize>) {
        if let Some(i) = menu {
            self.chosen = Some(i);
            self.open = true;
        } else {
            self.leave();
        }
    }

    /// Escape from a popup returns keyboard control to its title without reopening it.
    pub fn back_to_bar(&mut self, menu: usize) {
        self.chosen = Some(menu);
        self.open = false;
        self.keyboard = true;
        self.keys = true;
    }

    pub fn reached_by_keyboard(&mut self, keyboard: bool) {
        self.keyboard = keyboard;
        self.keys |= keyboard;
    }

    pub fn press(&mut self, menu: usize) -> Option<MenuAction> {
        self.leave();
        (menu < MENUS.len()).then_some(MenuAction::Open(menu))
    }

    pub fn menu_key(&mut self) {
        if self.chosen.is_some() {
            self.leave();
        } else {
            self.back_to_bar(0);
        }
    }

    /// The OS turns Alt's release into the menu-key event.
    pub fn alt_key(&mut self, key: Key) -> bool {
        if key == Key::Modifier {
            self.keys = true;
        }
        false
    }

    fn open_by_keyboard(&mut self, menu: usize) -> MenuAction {
        self.reached_by_keyboard(true);
        MenuAction::Open(menu)
    }

    pub fn access(&mut self, ch: char) -> Option<MenuAction> {
        match ch.to_ascii_lowercase() {
            'g' => Some(self.open_by_keyboard(0)),
            'h' => Some(self.open_by_keyboard(1)),
            ' ' => Some(MenuAction::SystemMenu),
            _ => None,
        }
    }

    pub fn typed(&mut self, ch: char) -> Option<MenuAction> {
        if self.chosen.is_some() && !self.open && ch.is_ascii_alphabetic() { self.access(ch) } else { None }
    }

    pub fn key(&mut self, era: Era, key: Key, mods: Mods) -> Option<MenuAction> {
        if self.open {
            return Some(MenuAction::Handled);
        }
        if let Some(i) = self.chosen {
            match key {
                Key::Left | Key::Right => self.chosen = Some((i + 1) % MENUS.len()),
                Key::Up | Key::Down | Key::Enter => return Some(self.open_by_keyboard(i)),
                Key::Modifier => {}
                Key::Other(vk) if (u16::from(b'A')..=u16::from(b'Z')).contains(&vk) => {}
                _ => self.leave(),
            }
            return Some(MenuAction::Handled);
        }
        if key != Key::Modifier {
            self.keys = false;
        }
        match (key, mods.ctrl, mods.shift) {
            (Key::Other(0x70), false, false) if era == Era::Win2000 => Some(MenuAction::Run(HelpTopics)),
            (Key::Other(0x71), false, false) if era == Era::Win2000 => Some(MenuAction::Run(Deal)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn era_specific_commands_match_the_painted_rows() {
        let bar = MenuBar::default();
        for era in [Era::Win95, Era::Win98, Era::Win2000] {
            let menu = bar.menu(era, 0, false);
            assert_eq!(menu.entries[0].label, if era == Era::Win2000 { "&Deal\t F2" } else { "&Deal" });
            assert!(menu.entries[2].disabled);
            assert!(!bar.menu(era, 0, true).entries[2].disabled);
            assert_eq!(command_at(era, 0, 1), None);
            assert_eq!(command_at(era, 0, 2), Some(Undo));
            assert_eq!(command_at(era, 1, 0), Some(HelpTopics));
            assert_eq!(command_at(era, 1, 1), (era == Era::Win2000).then_some(SearchHelp));
            assert_eq!(command_at(era, 1, bar.menu(era, 1, true).entries.len() - 1), Some(About));
        }
    }

    #[test]
    fn keyboard_selection_returns_to_the_title_after_escape() {
        let mut bar = MenuBar::default();
        let era = Era::Win2000;
        assert!(!bar.show_keys(era));
        bar.menu_key();
        assert_eq!(bar.looks(), [TitleLook::Chosen, TitleLook::Plain]);
        assert_eq!(bar.key(era, Key::Right, Mods::default()), Some(MenuAction::Handled));
        assert_eq!(bar.key(era, Key::Down, Mods::default()), Some(MenuAction::Open(1)));
        bar.show_menu(Some(1));
        assert_eq!(bar.menu(era, 1, false).hover, Some(0));
        bar.back_to_bar(1);
        assert_eq!(bar.typed('g'), Some(MenuAction::Open(0)));
        bar.show_menu(None);
        assert!(!bar.show_keys(era));
        assert_eq!(bar.press(0), Some(MenuAction::Open(0)));
        assert_eq!(bar.menu(era, 0, false).hover, None);
    }

    #[test]
    fn the_function_keys_belong_to_the_windows_2000_game() {
        let mut bar = MenuBar::default();
        for (key, command) in [(0x70, HelpTopics), (0x71, Deal)] {
            for era in [Era::Win95, Era::Win98] {
                assert_eq!(bar.key(era, Key::Other(key), Mods::default()), None);
            }
            assert_eq!(bar.key(Era::Win2000, Key::Other(key), Mods::default()), Some(MenuAction::Run(command)));
        }
    }
}

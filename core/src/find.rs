//! Quick Search (until 2026-11-16 "Find: Programs", "Search: Programs" in the 2000 look): a small window of Bevel95's
//! own in which a few letters typed find a program and Enter starts it (the owner, 2026-11-11:
//! "just to start maybe it can just search programs"). No classic Windows had it, so it is built
//! from the control kit's parts in the look's style and matched to no capture: its wording and
//! its numbers are PROVISIONAL (docs/theme-spec.md, "Find (programs)").
//!
//! Top to bottom: "Named:" with the Run dialog's text field; a list with a header of two
//! columns, Name and In Folder (the columns of Windows' own Find results), each row a program's
//! picture, its name and its folder in the Programs menu; a status bar ("N program(s) found",
//! as Windows' Find wrote "N file(s) found"). The text field keeps the focus: the arrows and
//! Page Up and Page Down move the selection, Enter starts the selected program, Esc closes.
//!
//! What it finds (`programs`) is every program of the Programs menu with "everything" chosen;
//! how it finds (`find`) is unit-tested here. The platform layer reads the programs, fetches
//! the pictures of the rows that show (`wanted_icons`) and starts the chosen one.

use std::collections::{HashMap, HashSet};

use crate::dialog;
use crate::fb::{Framebuffer, Icon, Rect};
use crate::kit::{self, Effect, Key, Mods, Press};
use crate::programs::Folder;
use crate::quick::{self, Action, Command};
use crate::scroll::ScrollBar;
use crate::start::StartEntry;
use crate::text::{Fonts, fold};
use crate::theme::{Era, Theme, rgb};
use crate::window::{self, ButtonLook, Frame, TitleButton};

/// One program the window can find: its name, its folder as the window shows it ("Programs",
/// "Programs\Accessories", ...) and what starts it (the Programs menu entry's target).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub name: String,
    pub folder: String,
    pub target: String,
    /// One of Bevel95's own programs (the host says so: `programs` cannot tell), or a command.
    pub own: bool,
    /// A thing to do instead of a program (`quick`): `target` is then its key, `folder` "Bevel95".
    pub command: Option<Action>,
    /// A command's other words it is found by (none for a program).
    pub words: &'static [&'static str],
    /// What the entry really opens, as the host found out (`Target::Unknown` until it has).
    pub kind: Target,
}

impl Program {
    /// The row of a command.
    pub fn of_command(c: &Command) -> Program {
        Program {
            name: c.name.clone(),
            folder: quick::FOLDER.to_string(),
            target: c.key.clone(),
            own: true,
            command: Some(c.action),
            words: c.words,
            kind: Target::Program,
        }
    }
}

/// What a Programs menu entry really opens (the host reads a shortcut's target; `classify`
/// tells the kind from it). By default Quick Search hides only what is known to be no program
/// (`hidden`: a web page, a document of a known kind, a folder), unless the setting
/// `quick_search = everything` says otherwise (`listed`). A hidden program is the costly
/// mistake, a listed document the cheap one, so whatever is not known is listed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Target {
    /// Not known to be anything else: a file of a kind neither in `PROGRAM_EXTENSIONS` nor in
    /// `DOCUMENT_EXTENSIONS` (a script, a Java program, a launcher of any kind, a file without
    /// an extension), a shortcut that could not be read, or one to a thing of the shell's own
    /// with no file, such as a Control Panel item. Listed.
    #[default]
    Unknown,
    /// A program's file (`PROGRAM_EXTENSIONS`), an installer-advertised shortcut, or one of
    /// Windows' Store-style apps.
    Program,
    /// A web address, or an Internet shortcut (.url).
    Web,
    /// A file of a known document kind (`DOCUMENT_EXTENSIONS`): a text, a Help book, a page,
    /// a picture.
    Document,
    /// A folder (the host tells it from what the shortcut recorded, never by asking the disk).
    Folder,
}

/// The files that are programs: they start something rather than open a document. .pif (an
/// MS-DOS program's settings) and .appref-ms (an app installed from the web by ClickOnce) start
/// a program too.
pub const PROGRAM_EXTENSIONS: [&str; 8] = ["exe", "com", "bat", "cmd", "msc", "cpl", "pif", "appref-ms"];

/// The files known to be documents: they open something to read or look at. Only these are
/// hidden as documents; any other kind is listed (it may start a program).
pub const DOCUMENT_EXTENSIONS: [&str; 28] = [
    "txt", "rtf", "doc", "docx", "odt", "pdf", "chm", "hlp", "htm", "html", "mht", "mhtml", "md", "log", "nfo", "ini", "xml",
    "json", "csv", "xls", "xlsx", "ppt", "pptx", "bmp", "gif", "jpg", "jpeg", "png",
];

/// The kind of thing `target` is: a path (a shortcut's target, or the entry's own file when it
/// is no shortcut), a web address, or a Store-style app (`programs::APP_PREFIX`). An empty
/// target is `Unknown`. Folders are told by the host (from what the shortcut recorded).
pub fn classify(target: &str) -> Target {
    let target = target.trim().trim_matches('"');
    if target.is_empty() {
        return Target::Unknown;
    }
    // Compared as bytes: a path's 17th byte may fall inside a letter of another alphabet, where
    // cutting the text would panic.
    let prefix = crate::programs::APP_PREFIX.as_bytes();
    if target.as_bytes().get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix)) {
        return Target::Program;
    }
    let lower = target.to_lowercase();
    if ["http:", "https:", "ftp:", "mailto:", "www."].iter().any(|scheme| lower.starts_with(scheme)) {
        return Target::Web;
    }
    // The file's extension: after the last dot of its last part.
    let file = lower.rsplit(['\\', '/']).next().unwrap_or(&lower);
    match file.rsplit_once('.') {
        Some((_, "url" | "website")) => Target::Web,
        Some((_, ext)) if PROGRAM_EXTENSIONS.contains(&ext) => Target::Program,
        Some((_, ext)) if DOCUMENT_EXTENSIONS.contains(&ext) => Target::Document,
        _ => Target::Unknown,
    }
}

/// The first words of a name that hide the entry whatever it opens: an uninstaller or an About
/// box is never what a person searches for to start a program.
const HIDDEN_NAMES: [&str; 2] = ["uninstall", "about"];

/// Why an entry is not listed (`None`: it is listed). Bevel95's own programs and commands are
/// always listed; other entries unless what they open is known to be no program, or their
/// name's first word is "Uninstall" or "About" (case does not matter).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hidden {
    /// What it opens is known to be no program (`Target::Web`, `Document` or `Folder`).
    ByTarget,
    /// Its name's first word is one of `HIDDEN_NAMES`.
    ByName,
}

pub fn hidden(name: &str, target: Target) -> Option<Hidden> {
    if matches!(target, Target::Web | Target::Document | Target::Folder) {
        return Some(Hidden::ByTarget);
    }
    // The first word: the letters and digits up to the first other mark ("Uninstall Tool",
    // "About: Tool"), so "Aboutface" and "Uninstaller" are names of their own.
    let first: String = name.trim_start().chars().take_while(|c| c.is_alphanumeric()).collect();
    let first = first.to_lowercase();
    HIDDEN_NAMES.contains(&first.as_str()).then_some(Hidden::ByName)
}

/// The entries Quick Search lists: with `everything`, all of them; else only those `hidden`
/// does not hide, and always Bevel95's own and the commands.
pub fn listed(programs: Vec<Program>, everything: bool) -> Vec<Program> {
    if everything {
        return programs;
    }
    programs.into_iter().filter(|p| p.own || p.command.is_some() || hidden(&p.name, p.kind).is_none()).collect()
}

/// The programs and, after them, every command of `quick::commands` but those that only repeat
/// one of Bevel95's own programs of the same name (Windows Explorer: the program's row starts
/// the very same window).
pub fn with_commands(mut programs: Vec<Program>) -> Vec<Program> {
    let own: Vec<String> = programs.iter().filter(|p| p.own && p.command.is_none()).map(|p| p.name.to_lowercase()).collect();
    programs.extend(quick::commands().iter().filter(|c| !own.contains(&c.name.to_lowercase())).map(Program::of_command));
    programs
}

/// The name of the Programs menu itself, the first part of every folder shown.
pub const ROOT: &str = "Programs";

/// Every program of the Programs menu `tree`, folder by folder in the menu's order. One row
/// per program: the same name in the same folder shows once (compared without regard to case,
/// as the menu merges the two Start Menu folders).
pub fn programs(tree: &Folder) -> Vec<Program> {
    fn walk(folder: &Folder, path: &str, out: &mut Vec<Program>) {
        for item in &folder.items {
            out.push(Program {
                name: item.name.clone(),
                folder: path.to_string(),
                target: item.target.clone(),
                own: false,
                command: None,
                words: &[],
                kind: Target::Unknown,
            });
        }
        for inner in &folder.folders {
            walk(inner, &format!("{path}\\{}", inner.name), out);
        }
    }
    let mut out = Vec::new();
    walk(tree, ROOT, &mut out);
    let mut seen = HashSet::new();
    out.retain(|p| seen.insert((fold(&p.name), fold(&p.folder))));
    out
}

/// Whether a word of `name` (after a space or other mark that is neither letter nor digit)
/// begins with `text`. Both are lowercase.
fn a_word_begins(name: &str, text: &str) -> bool {
    let mut before: Option<char> = None;
    for (i, ch) in name.char_indices() {
        if before.is_some_and(|b| !b.is_alphanumeric()) && ch.is_alphanumeric() && name[i..].starts_with(text) {
            return true;
        }
        before = Some(ch);
    }
    false
}

/// The rows that `text` finds, best first, as indices into `programs`. Case does not matter;
/// the text is split at spaces and every word must be in the name (a command's: in its name,
/// its other words or "Bevel95"). They rank by (a) the name begins with the text, (b) a word of
/// the name, or one of a command's other words, begins with it, (c) the rest (the name has it,
/// or has each of its words); alphabetical within a rank, then Bevel95's own (its programs and
/// the commands) before another of the same name (so "calc" and Enter start Bevel95's
/// Calculator, not today's), then by folder. An empty text finds every program, A to Z, and no
/// command.
pub fn find(programs: &[Program], text: &str) -> Vec<usize> {
    let words: Vec<String> = text.split_whitespace().map(fold).collect();
    let whole = words.join(" ");
    let mut found: Vec<(u8, String, bool, String, usize)> = programs
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let name = fold(&p.name);
            let rank = if p.command.is_some() {
                if words.is_empty() {
                    return None;
                }
                let mut hay = format!("{name} {}", quick::FOLDER.to_lowercase());
                for w in p.words {
                    hay.push(' ');
                    hay.push_str(w);
                }
                if !words.iter().all(|w| hay.contains(w.as_str())) {
                    return None;
                }
                if name.starts_with(&whole) {
                    0
                } else if a_word_begins(&name, &whole) || p.words.iter().any(|w| w.starts_with(&whole)) {
                    1
                } else {
                    2
                }
            } else {
                if !words.iter().all(|w| name.contains(w.as_str())) {
                    return None;
                }
                if name.starts_with(&whole) {
                    0
                } else if a_word_begins(&name, &whole) {
                    1
                } else {
                    2
                }
            };
            Some((rank, name, !p.own, fold(&p.folder), i))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(.., i)| i).collect()
}

/// The window's title: the same in every look, as it is Bevel95's own tool, not something a
/// Windows had (PROVISIONAL, the owner's working name of 2026-11-16).
pub fn title(_era: Era) -> &'static str {
    "Quick Search"
}

/// The status bar's text (programs and commands alike are items; PROVISIONAL).
pub fn status(found: usize) -> String {
    format!("{found} item(s) found")
}

// The window at 1× (ours, PROVISIONAL; docs/theme-spec.md, "Find (programs)"): 330×264, not
// resizable. "Named:" 9 in from the frame's outer edge, 4 below the field's top (Run's "Open:"
// sits 3 below), the field Run's 21 high from x 58 to 9 short of the right edge; the list's
// sunken frame 7 below it, as wide as the field reaches; inside it a header of 17 and 10 rows
// of 16 (a 16×16 picture each); the status bar's 20 rows at the bottom inside the frame, 3
// below the list.
const SIZE: (i32, i32) = (330, 264);
const LABEL: &str = "&Named:";
const LABEL_AT: (i32, i32) = (9, 33);
const FIELD: Rect = Rect::new(58, 29, 263, 21);
const LIST: Rect = Rect::new(9, 57, 312, 181);
const HEADER_H: i32 = 17;
const ROW_H: i32 = 16;
/// Rows the list shows (the scroll bar's page).
const PAGE: usize = 10;
/// The Name column's width as it opens the first time; In Folder takes the rest. The line
/// between them can be dragged (as a folder window's Details header: `desktop_icons::header_hit`)
/// and the width is remembered (`FindWindow::name_w`, `state_text`).
pub const NAME_W: i32 = 150;
/// The narrowest the two columns may be made: Name keeps its picture and a few letters, In
/// Folder a few letters (so neither can vanish under the other).
const MIN_NAME_W: i32 = NAME_X + 16;
const MIN_FOLDER_W: i32 = 40;
const STATUS: Rect = Rect::new(3, 241, 324, 20);
const COLUMNS: [&str; 2] = ["Name", "In Folder"];
/// What the list says until the programs are read.
const WAIT: &str = "Looking for programs...";
/// In a row: the picture 2 in, the name 4 after it; In Folder's text 6 into its column (as
/// the header's labels).
const ICON_X: i32 = 2;
const NAME_X: i32 = 22;
const CELL_PAD: i32 = 6;

/// The parts of the window that the mouse and the keyboard act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    TitleClose,
    /// The text field (it always has the focus).
    Field,
    /// The list (its frame) and a row of it (its place among the programs found).
    List,
    Row(usize),
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollThumb,
    /// The line between the header's Name and In Folder: dragged, it sets Name's width; a
    /// double-click fits Name to the longest name found. The host hears of a new width by
    /// `Effect::Activate(Part::Divider)` and remembers it.
    Divider,
    /// Starting the selected program (Enter, a double-click): no place of its own.
    Open,
}

/// The window as it is now.
#[derive(Clone, Debug)]
pub struct FindWindow {
    pub era: Era,
    /// Every program it can find (`programs`).
    pub programs: Vec<Program>,
    /// The programs have been read (until then the list says it is looking).
    pub loaded: bool,
    /// Each program's picture once fetched (`None`: not fetched yet).
    icons: Vec<Option<Option<Icon>>>,
    /// What was typed, and the caret's place in it (in characters).
    pub text: String,
    pub caret: usize,
    pub caret_on: bool,
    /// The programs found (indices into `programs`, best first), the selected one (a place in
    /// this list) and how many are scrolled out above the list.
    pub found: Vec<usize>,
    pub sel: usize,
    /// A new reading took away the selected program (or brought rows where none was selected):
    /// nothing is selected until the person moves, types or clicks, so Enter never starts a
    /// row that was not shown selected (an Enter may wait in the queue before the new list is
    /// painted).
    pub selection_lost: bool,
    pub top: usize,
    pub pressed: Option<Part>,
    /// Underline the access key: Windows 95 and 98 always do; Windows 2000 once the keyboard
    /// has been used.
    pub show_keys: bool,
    /// The window is the one in front (its title bar shows it).
    pub active: bool,
    /// The left edge of the text's first character in the window, and each character's width
    /// (for clicks in the field).
    field_x: i32,
    advances: Vec<i32>,
    parts: Vec<(Part, Rect)>,
    /// The Name column's width as chosen (`NAME_W` until the line is dragged; the host sets the
    /// remembered one). It is kept within the limits where it is used (`name_width`).
    pub name_w: i32,
    /// The line being dragged: the pointer's x where it was grabbed and where it is now.
    divider_drag: Option<(i32, i32)>,
    /// The font the names are drawn in (from the first layout), to fit Name to them.
    font: Option<crate::text::Font>,
}

impl FindWindow {
    /// The window as it opens: the field empty, every program listed, the first selected. The
    /// commands (`with_commands`) join the programs, their pictures already known.
    pub fn new(era: Era, programs: Vec<Program>, loaded: bool) -> FindWindow {
        let programs = with_commands(programs);
        let mut w = FindWindow {
            era,
            icons: programs.iter().map(|p| p.command.map(|a| quick::picture(era, a))).collect(),
            programs,
            loaded,
            text: String::new(),
            caret: 0,
            caret_on: true,
            found: Vec::new(),
            sel: 0,
            selection_lost: false,
            top: 0,
            pressed: None,
            show_keys: era.always_shows_keys(),
            active: true,
            field_x: 0,
            advances: Vec::new(),
            parts: Vec::new(),
            name_w: NAME_W,
            divider_drag: None,
            font: None,
        };
        w.refind();
        w
    }

    /// Whether no program is known yet (only the commands).
    fn no_programs(&self) -> bool {
        self.programs.iter().all(|p| p.command.is_some())
    }

    /// The programs were read (again; without the commands, which join them here): pictures
    /// already fetched are kept, the text stays, and the selection stays on its program if it
    /// is still found. Else nothing is selected (`selection_lost`) until the person moves,
    /// types or clicks: a row that comes under the selection was never shown selected.
    pub fn set_programs(&mut self, programs: Vec<Program>) {
        let mut known: HashMap<String, Option<Icon>> = HashMap::new();
        for (p, icon) in self.programs.iter().zip(&self.icons) {
            if let Some(icon) = icon {
                known.insert(p.target.clone(), icon.clone());
            }
        }
        let selected = self.selected().map(|p| (p.target.clone(), p.folder.clone()));
        let programs = with_commands(programs);
        let era = self.era;
        self.icons = programs
            .iter()
            .map(|p| p.command.map(|a| quick::picture(era, a)).or_else(|| known.get(&p.target).cloned()))
            .collect();
        self.programs = programs;
        self.loaded = true;
        self.refind();
        if let Some((target, folder)) = selected
            && let Some(at) = self.found.iter().position(|&i| self.programs[i].target == target && self.programs[i].folder == folder)
        {
            self.sel = at;
            self.show_selected();
        } else {
            self.selection_lost = true;
        }
        self.lay_out();
    }

    /// The selected program, if anything was found and is shown selected.
    pub fn selected(&self) -> Option<&Program> {
        if self.selection_lost {
            return None;
        }
        self.found.get(self.sel).map(|&i| &self.programs[i])
    }

    /// The programs (indices into `programs`) of the rows that show and whose pictures have
    /// not been fetched yet.
    pub fn wanted_icons(&self) -> Vec<usize> {
        self.found.iter().skip(self.top).take(PAGE).copied().filter(|&i| self.icons[i].is_none()).collect()
    }

    /// The picture of program `i` (`None`: it has none).
    pub fn set_icon(&mut self, i: usize, icon: Option<Icon>) {
        if let Some(slot) = self.icons.get_mut(i) {
            *slot = Some(icon);
        }
    }

    /// The text changed: the programs are found afresh, the first selected and in view.
    fn refind(&mut self) {
        self.found = find(&self.programs, &self.text);
        self.sel = 0;
        self.selection_lost = false;
        self.top = 0;
        self.lay_out();
    }

    /// Edits the text: `f` gets it and the caret (in characters).
    fn edit(&mut self, f: impl FnOnce(&mut String, &mut usize)) {
        let (mut text, mut caret) = (self.text.clone(), self.caret);
        f(&mut text, &mut caret);
        let changed = text != self.text;
        self.text = text;
        self.caret = caret.min(self.text.chars().count());
        self.caret_on = true;
        if changed {
            self.refind();
        }
    }

    /// Moves the selection by `delta` rows, scrolling to keep it in view.
    fn move_by(&mut self, delta: i32) {
        if self.found.is_empty() {
            return;
        }
        if self.selection_lost {
            // Nothing selected: a move selects the first row shown, as a list without a
            // selection does.
            self.selection_lost = false;
            self.sel = self.top.min(self.found.len() - 1);
        } else {
            self.sel = (self.sel as i32 + delta).clamp(0, self.found.len() as i32 - 1) as usize;
        }
        self.show_selected();
    }

    fn show_selected(&mut self) {
        if self.sel < self.top {
            self.top = self.sel;
        } else if self.sel >= self.top + PAGE {
            self.top = self.sel + 1 - PAGE;
        }
        self.lay_out();
    }

    fn scroll_by(&mut self, delta: i32) {
        let max = self.found.len().saturating_sub(PAGE) as i32;
        self.top = (self.top as i32 + delta).clamp(0, max) as usize;
        self.lay_out();
    }

    fn inside() -> Rect {
        LIST.inset(2, 2, 2, 2)
    }

    /// The list's scroll bar, beside the header and the rows: none while everything found fits.
    fn scroll(&self) -> Option<ScrollBar> {
        let inside = Self::inside();
        let bar = Rect::new(inside.right() - 16, inside.y, 16, inside.h);
        (self.found.len() > PAGE).then_some(ScrollBar { bar, total: self.found.len(), page: PAGE })
    }

    /// The width of the header and the rows: the list's inside, less the scroll bar.
    fn rows_width(&self) -> i32 {
        Self::inside().w - if self.scroll().is_some() { 16 } else { 0 }
    }

    /// `width` kept within the limits: neither column narrower than its least.
    fn clamp_name(&self, width: i32) -> i32 {
        width.clamp(MIN_NAME_W, (self.rows_width() - MIN_FOLDER_W).max(MIN_NAME_W))
    }

    /// The Name column's width as shown (`name_w` within the limits).
    pub fn name_width(&self) -> i32 {
        self.clamp_name(self.name_w)
    }

    /// Where the line being dragged is drawn (x in the list's inside), if one is.
    fn drag_line(&self) -> Option<i32> {
        self.divider_drag.map(|(from, to)| self.clamp_name(self.name_width() + to - from))
    }

    /// The width a double-click on the line gives Name: the longest name found, its picture
    /// and the room after it (as a folder window's Details fits its Name column).
    fn fitted_width(&self) -> Option<i32> {
        let font = self.font.as_ref()?;
        let widest = self.found.iter().map(|&i| font.measure(&self.programs[i].name)).max()?;
        Some(self.clamp_name(widest + NAME_X + CELL_PAD))
    }

    fn lay_out(&mut self) {
        let (inside, w) = (Self::inside(), self.rows_width());
        // The line between the columns takes the mouse 8 left to 7 right of In Folder's first
        // pixel, as a Details header's divider does (`desktop_icons::header_hit`); first, so
        // the list under it does not take the press.
        let edge = inside.x + self.name_width();
        let left = edge - crate::desktop_icons::DIVIDER_LEFT;
        let right = (edge + crate::desktop_icons::DIVIDER_RIGHT).min(inside.x + w - 1);
        let mut parts = vec![(Part::Divider, Rect::new(left, inside.y, right - left + 1, HEADER_H))];
        for at in self.top..self.found.len().min(self.top + PAGE) {
            let y = inside.y + HEADER_H + (at - self.top) as i32 * ROW_H;
            parts.push((Part::Row(at), Rect::new(inside.x, y, w, ROW_H)));
        }
        if let Some(scroll) = self.scroll() {
            let r = scroll.rects(self.top);
            parts.extend([
                (Part::ScrollUp, r.up),
                (Part::ScrollDown, r.down),
                (Part::ScrollThumb, r.thumb),
                (Part::ScrollPageUp, r.page_up),
                (Part::ScrollPageDown, r.page_down),
            ]);
        }
        let close = window::title_button_rect(SIZE.0, TitleButton::Close);
        parts.extend([(Part::List, LIST), (Part::Field, FIELD), (Part::TitleClose, close)]);
        self.parts = parts;
    }

    fn part_rect(&self, part: Part) -> Option<Rect> {
        self.parts.iter().find(|(p, _)| *p == part).map(|&(_, r)| r)
    }

    /// The text field's inside and where its text starts (Run's field: 1 in, 4 in Windows 2000).
    fn text_x(&self) -> i32 {
        FIELD.x + 2 + if self.era == Era::Win2000 { 4 } else { 1 }
    }
}

fn byte_at(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(i, _)| i)
}

impl kit::Form for FindWindow {
    type Part = Part;

    fn title(&self) -> &str {
        title(self.era)
    }
    fn size(&self) -> (i32, i32) {
        SIZE
    }
    fn layout(&mut self, fonts: &Fonts) {
        if self.font.as_ref() != Some(fonts.regular) {
            self.font = Some(fonts.regular.clone());
        }
        self.field_x = self.text_x();
        self.advances = self.text.chars().map(|c| fonts.regular.glyph(c).advance).collect();
        self.lay_out();
    }

    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        let (c, font) = (&theme.colors, fonts.regular);
        let held = |part: Part| self.pressed == Some(part);
        let picture = crate::icons::start_icon_small(self.era, StartEntry::Find);
        let close = ButtonLook {
            button: TitleButton::Close,
            rect: window::title_button_rect(SIZE.0, TitleButton::Close),
            pressed: held(Part::TitleClose),
            enabled: true,
        };
        let frame = Frame { era: self.era, title: title(self.era), icon: picture.as_ref(), active: self.active, buttons: &[close] };
        window::paint_frame(fb, theme, fonts, &frame);

        // "Named:" and the field (the Run dialog's), its text clipped to the field.
        font.draw_label(fb, LABEL_AT, LABEL, i32::MAX, c.text, self.show_keys);
        let inside = dialog::well(fb, FIELD, c);
        let mut field = Framebuffer::new(inside.w, inside.h, rgb(0xFFFFFF));
        let x0 = self.text_x() - inside.x;
        font.draw(&mut field, x0, 1, &self.text, c.text);
        if self.caret_on {
            let before: String = self.text.chars().take(self.caret).collect();
            field.vline(x0 + font.measure(&before), 1, font.height, c.text);
        }
        fb.blit(&field, inside.x, inside.y);

        // The list: its sunken frame, the header's raised cells, the rows.
        fb.frame(LIST, c.shadow, c.highlight);
        fb.frame(LIST.inset(1, 1, 1, 1), c.dark_shadow, c.light);
        let (inside, w) = (Self::inside(), self.rows_width());
        let name_w = self.name_width();
        let mut list = Framebuffer::new(w, inside.h, rgb(0xFFFFFF));
        let cells = [Rect::new(0, 0, name_w, HEADER_H), Rect::new(name_w, 0, w - name_w, HEADER_H)];
        for (cell, label) in cells.iter().zip(COLUMNS) {
            list.fill(*cell, c.face);
            dialog::raised(&mut list, *cell, c);
            font.draw(&mut list, cell.x + CELL_PAD, (HEADER_H - font.height) / 2, &font.elide(label, cell.w - 2 * CELL_PAD), c.text);
        }
        if !self.loaded && self.no_programs() {
            font.draw(&mut list, 4, HEADER_H + 2, WAIT, c.shadow);
        }
        let text_y = (ROW_H - font.height) / 2;
        for (row, &i) in self.found.iter().enumerate().skip(self.top).take(PAGE) {
            let (p, y) = (&self.programs[i], HEADER_H + (row - self.top) as i32 * ROW_H);
            if let Some(Some(icon)) = &self.icons[i] {
                crate::colors16::draw_picture(&mut list, theme, icon, ICON_X, y);
            }
            // The name, highlighted when selected (as a list's chosen label is).
            let name = font.elide(&p.name, name_w - NAME_X - CELL_PAD);
            let mut color = c.text;
            if row == self.sel && !self.selection_lost {
                list.fill(Rect::new(NAME_X - 2, y, font.measure(&name) + 4, ROW_H), c.menu_highlight);
                color = c.menu_highlight_text;
            }
            font.draw(&mut list, NAME_X, y + text_y, &name, color);
            let folder = font.elide(&p.folder, w - name_w - 2 * CELL_PAD);
            font.draw(&mut list, name_w + CELL_PAD, y + text_y, &folder, c.text);
        }
        // The line being dragged: inverted, from the header's top down the list, as a folder
        // window's Details draws it (`folder`, `*-view-v1-hdr-divider-drag`).
        if let Some(lx) = self.drag_line() {
            for y in 0..inside.h {
                let p = list.get(lx, y);
                list.set(lx, y, (p & 0xFF00_0000) | (!p & 0x00FF_FFFF));
            }
        }
        fb.blit(&list, inside.x, inside.y);
        if let Some(scroll) = self.scroll() {
            scroll.paint(fb, c, self.top, held(Part::ScrollUp), held(Part::ScrollDown));
        }

        // The status bar: one sunken panel the whole width, below a row of the face.
        let panel = Rect::new(STATUS.x, STATUS.y + 2, STATUS.w, STATUS.h - 2);
        fb.frame(panel, c.shadow, c.highlight);
        let said = if self.loaded || !self.no_programs() { status(self.found.len()) } else { WAIT.to_string() };
        font.draw(fb, panel.x + 3, panel.y + (panel.h - font.height) / 2, &said, c.text);
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.parts.iter().find(|(_, r)| r.contains(x, y)).map(|&(p, _)| p)
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        self.part_rect(part)
    }
    /// Enter starts something only while something is shown selected.
    fn enabled(&self, part: Part) -> bool {
        part != Part::Open || self.selected().is_some()
    }
    fn focus(&self) -> Part {
        Part::Field
    }
    /// The text field keeps the focus.
    fn set_focus(&mut self, _part: Part) {}
    fn tab_stops(&self) -> Vec<Part> {
        vec![Part::Field]
    }
    fn set_pressed(&mut self, part: Option<Part>) {
        self.pressed = part;
    }
    fn show_keys(&mut self) {
        self.show_keys = true;
    }
    fn enter(&self) -> Part {
        Part::Open
    }
    fn escape(&self) -> Part {
        Part::TitleClose
    }

    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        match key {
            Key::Up | Key::Down => self.move_by(if key == Key::Down { 1 } else { -1 }),
            Key::PageUp | Key::PageDown => {
                let page = PAGE as i32 - 1;
                self.move_by(if key == Key::PageDown { page } else { -page });
            }
            Key::Left => self.edit(|_, c| *c = c.saturating_sub(1)),
            Key::Right => self.edit(|_, c| *c += 1),
            Key::Home => self.edit(|_, c| *c = 0),
            Key::End => self.edit(|t, c| *c = t.chars().count()),
            Key::Delete => self.edit(|t, c| {
                if *c < t.chars().count() {
                    let at = byte_at(t, *c);
                    t.remove(at);
                }
            }),
            // One control: Tab stays in the field.
            Key::Tab => {}
            Key::Other(vk) if mods.ctrl && vk == u16::from(b'V') => return Some(vec![Effect::Paste]),
            _ => return None,
        }
        Some(Vec::new())
    }
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        match ch {
            '\u{8}' => self.edit(|t, c| {
                if *c > 0 {
                    *c -= 1;
                    let at = byte_at(t, *c);
                    t.remove(at);
                }
            }),
            ch if !ch.is_control() => self.edit(|t, at| {
                t.insert(byte_at(t, *at), ch);
                *at += 1;
            }),
            _ => {}
        }
        Vec::new()
    }
    /// Alt+N: the field, which has the focus anyway.
    fn access(&mut self, _ch: char) -> Vec<Effect<Part>> {
        Vec::new()
    }
    /// Space is typed into the field (it comes as a character too).
    fn space(&mut self) -> Vec<Effect<Part>> {
        Vec::new()
    }
    fn press(&mut self, part: Option<Part>, x: i32, _y: i32) -> Press<Part> {
        match part {
            Some(Part::Row(at)) => {
                self.sel = at;
                self.selection_lost = false;
                self.show_selected();
            }
            Some(Part::Field) => {
                // The caret goes between the characters nearest the click.
                let mut w = self.field_x;
                self.caret = self
                    .advances
                    .iter()
                    .take_while(|&&a| {
                        let before = w + a / 2 < x;
                        w += a;
                        before
                    })
                    .count();
                self.caret_on = true;
            }
            Some(part @ (Part::ScrollUp | Part::ScrollDown)) => {
                self.scroll_by(if part == Part::ScrollUp { -1 } else { 1 });
                return Press::Hold(part);
            }
            Some(part @ (Part::ScrollPageUp | Part::ScrollPageDown)) => {
                self.scroll_by(if part == Part::ScrollPageUp { -(PAGE as i32) } else { PAGE as i32 });
            }
            Some(Part::ScrollThumb) => return Press::Grab(Part::ScrollThumb, self.top as i32),
            Some(Part::Divider) => {
                self.divider_drag = Some((x, x));
                return Press::Grab(Part::Divider, 0);
            }
            Some(Part::TitleClose) => return Press::Hold(Part::TitleClose),
            Some(_) => {}
            None => return Press::Nothing,
        }
        Press::Done
    }
    /// A double-click on a row starts its program; on the line between the columns it fits Name
    /// to the longest name found.
    fn double_click(&mut self, x: i32, y: i32) -> Option<Vec<Effect<Part>>> {
        match self.hit(x, y) {
            Some(Part::Row(at)) => {
                self.sel = at;
                self.selection_lost = false;
                self.show_selected();
                Some(vec![Effect::Activate(Part::Open)])
            }
            Some(Part::Divider) => {
                self.divider_drag = None;
                let width = self.fitted_width()?;
                self.name_w = width;
                self.lay_out();
                Some(vec![Effect::Activate(Part::Divider)])
            }
            _ => None,
        }
    }
    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        match part {
            Part::TitleClose => vec![Effect::Activate(part)],
            _ => Vec::new(),
        }
    }
    fn drag(&mut self, part: Part, grab_y: i32, grab_top: i32, x: i32, y: i32) -> bool {
        if part == Part::Divider {
            let before = self.drag_line();
            if let Some(d) = self.divider_drag.as_mut() {
                d.1 = x;
            }
            return self.drag_line() != before;
        }
        let Some(scroll) = self.scroll() else { return false };
        let top = scroll.dragged(grab_top, y - grab_y);
        let changed = top != self.top;
        self.top = top;
        self.lay_out();
        changed
    }
    /// The line let go: Name takes the width the line shows (as a folder window's Details).
    fn dropped(&mut self, part: Part, _x: i32, _y: i32) -> Vec<Effect<Part>> {
        if part != Part::Divider {
            return Vec::new();
        }
        let Some(width) = self.drag_line() else { return Vec::new() };
        self.divider_drag = None;
        if width == self.name_width() {
            return Vec::new();
        }
        self.name_w = width;
        self.lay_out();
        vec![Effect::Activate(Part::Divider)]
    }
    fn pointer(&self, x: i32, y: i32) -> kit::Pointer {
        match self.hit(x, y) {
            Some(Part::Divider) => kit::Pointer::HeaderDivider,
            _ => kit::Pointer::Arrow,
        }
    }
    fn wheel(&mut self, notches: i32) {
        self.scroll_by(-3 * notches);
    }
    fn paste(&mut self, text: &str) {
        let paste: String = text.chars().filter(|c| !c.is_control()).collect();
        self.edit(|t, c| {
            t.insert_str(byte_at(t, *c), &paste);
            *c += paste.chars().count();
        });
    }
    fn has_caret(&self) -> bool {
        true
    }
    fn blink(&mut self, on: bool) {
        self.caret_on = on;
    }
}

/// What Quick Search keeps between runs (the host's `quick-search.txt`): the Name column's width.
pub fn state_text(name_w: i32) -> String {
    format!("# Quick Search: the width of its Name column (the line between Name and In Folder).\nname_width = {name_w}\n")
}

/// The Name column's width kept in `text` (`state_text`), if it holds a sensible one.
pub fn read_state(text: &str) -> Option<i32> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "name_width")
        .and_then(|(_, value)| value.trim().parse::<i32>().ok())
        .filter(|w| (MIN_NAME_W..=SIZE.0).contains(w))
}

/// A held scroll bar button scrolls on (the window's host calls this at each tick of its timer
/// once the button has been held a moment). True if the list moved.
pub fn scroll_on(window: &mut FindWindow) -> bool {
    let before = window.top;
    match window.pressed {
        Some(Part::ScrollUp) => window.scroll_by(-1),
        Some(Part::ScrollDown) => window.scroll_by(1),
        _ => {}
    }
    window.top != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::{Controller, Form};
    use crate::programs::Item;

    fn menu() -> Folder {
        let item = |name: &str| Item { name: name.into(), target: format!("C:\\{name}.lnk") };
        let mut tree = Folder::default();
        for name in ["Minesweeper", "Solitaire"] {
            tree.add(&["Accessories", "Games"], item(name));
        }
        for name in ["Calculator", "Notepad", "Paint"] {
            tree.add(&["Accessories"], item(name));
        }
        for name in ["MS-DOS Prompt", "Windows Explorer", "Internet Explorer"] {
            tree.add(&[], item(name));
        }
        tree.sort();
        tree
    }

    fn names<'a>(programs: &'a [Program], found: &[usize]) -> Vec<&'a str> {
        found.iter().map(|&i| programs[i].name.as_str()).collect()
    }

    #[test]
    fn every_program_with_its_folder_once() {
        let mut tree = menu();
        // The same shortcut in both Start Menu folders, another case: the menu shows it once,
        // and so does Find. A program of the same name in another folder is another row.
        tree.folders[0].items.push(Item { name: "notepad".into(), target: "C:\\other.lnk".into() });
        tree.add(&[], Item { name: "Notepad".into(), target: "C:\\top.lnk".into() });
        tree.sort();
        let all = programs(&tree);
        let rows: Vec<(&str, &str)> = all.iter().map(|p| (p.name.as_str(), p.folder.as_str())).collect();
        assert_eq!(
            rows,
            [
                ("Internet Explorer", "Programs"),
                ("MS-DOS Prompt", "Programs"),
                ("Notepad", "Programs"),
                ("Windows Explorer", "Programs"),
                ("Calculator", "Programs\\Accessories"),
                ("Notepad", "Programs\\Accessories"),
                ("Paint", "Programs\\Accessories"),
                ("Minesweeper", "Programs\\Accessories\\Games"),
                ("Solitaire", "Programs\\Accessories\\Games"),
            ]
        );
        assert_eq!(all[5].target, "C:\\Notepad.lnk", "the first of the two is kept");
    }

    #[test]
    fn finds_by_beginning_then_word_then_anywhere() {
        let all = programs(&menu());
        // "p": Paint begins with it, MS-DOS Prompt has a word that does, the rest have it.
        assert_eq!(
            names(&all, &find(&all, "p")),
            ["Paint", "MS-DOS Prompt", "Internet Explorer", "Minesweeper", "Notepad", "Windows Explorer"]
        );
        // Case does not matter; "ex" begins a word of both Explorers.
        assert_eq!(names(&all, &find(&all, "EX")), ["Internet Explorer", "Windows Explorer"]);
        assert_eq!(names(&all, &find(&all, "dos")), ["MS-DOS Prompt"], "after the dash is a word too");
        // Every word must be in the name, in any order.
        assert_eq!(names(&all, &find(&all, "explorer win")), ["Windows Explorer"]);
        assert_eq!(names(&all, &find(&all, "  windows   explorer ")), ["Windows Explorer"]);
        assert!(find(&all, "xyz").is_empty());
        // Nothing typed: everything, A to Z.
        assert_eq!(
            names(&all, &find(&all, "")),
            [
                "Calculator",
                "Internet Explorer",
                "Minesweeper",
                "MS-DOS Prompt",
                "Notepad",
                "Paint",
                "Solitaire",
                "Windows Explorer"
            ]
        );
    }

    #[test]
    fn bevel95s_own_program_comes_first_among_equals() {
        // Today's Calculator (an app, in Programs) and Bevel95's (in Accessories): both begin
        // with "calc", and the same name; Bevel95's own goes first, though its folder sorts later.
        let mut tree = Folder::default();
        tree.add(&[], Item { name: "Calculator".into(), target: "shell:AppsFolder\\Calc!App".into() });
        tree.add(&["Accessories"], Item { name: "Calculator".into(), target: "C:\\Bevel95\\calculator.exe".into() });
        tree.add(&[], Item { name: "Calendar".into(), target: "shell:AppsFolder\\Cal!App".into() });
        tree.sort();
        let mut all = programs(&tree);
        let before = find(&all, "calc");
        assert_eq!(all[before[0]].folder, "Programs", "without the rule, by folder");
        for p in &mut all {
            p.own = p.target.ends_with("calculator.exe");
        }
        let found = find(&all, "calc");
        assert_eq!(found.len(), 2);
        assert_eq!(all[found[0]].folder, "Programs\\Accessories");
        // Only among equals: a better match still ranks first.
        assert_eq!(names(&all, &find(&all, "cal")), ["Calculator", "Calculator", "Calendar"]);
        assert!(all[find(&all, "cal")[0]].own);
        all.iter_mut().for_each(|p| p.own = p.name == "Calendar");
        assert_eq!(names(&all, &find(&all, "cal")), ["Calculator", "Calculator", "Calendar"]);
    }

    /// The rows `text` finds in a window: name and folder.
    fn rows(w: &FindWindow) -> Vec<(String, String)> {
        w.found.iter().map(|&i| (w.programs[i].name.clone(), w.programs[i].folder.clone())).collect()
    }

    fn typed(era: Era, text: &str) -> FindWindow {
        let mut w = FindWindow::new(era, programs(&menu()), true);
        for ch in text.chars() {
            w.typed(ch);
        }
        w
    }

    #[test]
    fn commands_show_once_something_typed_matches_them() {
        // Nothing typed: the programs only, as before.
        let w = FindWindow::new(Era::Win98, programs(&menu()), true);
        assert_eq!(w.found.len(), 8);
        assert!(w.found.iter().all(|&i| w.programs[i].command.is_none()));
        // "theme" and "look": the three looks, found by their other words.
        let names = |w: &FindWindow| rows(w).into_iter().map(|(n, _)| n).collect::<Vec<_>>();
        assert_eq!(names(&typed(Era::Win98, "theme")), ["Windows 2000 look", "Windows 95 look", "Windows 98 look"]);
        assert_eq!(names(&typed(Era::Win98, "look")), ["Windows 2000 look", "Windows 95 look", "Windows 98 look"]);
        // "size" and "scale": each size the Size menu offers; names beginning with it come first.
        assert_eq!(names(&typed(Era::Win98, "size")), ["Size 1\u{d7}", "Size 2\u{d7}", "Size 3\u{d7}", "Size 4\u{d7}"]);
        assert_eq!(typed(Era::Win98, "scale").found.len(), 4);
        // "sett": Bevel95 Settings, a word of its name beginning with it (and its own word
        // "settings"); nothing else has it.
        assert_eq!(names(&typed(Era::Win98, "sett")), ["Bevel95 Settings"]);
        // "bevel" shows them all, in the folder Bevel95; Enter does the first.
        let w = typed(Era::Win98, "bevel");
        assert_eq!(w.found.len(), quick::commands().len());
        assert!(rows(&w).iter().all(|(_, f)| f == "Bevel95"));
        assert_eq!(rows(&w)[0].0, "Bevel95 Settings", "its name begins with it");
        assert_eq!(w.selected().unwrap().command, Some(Action::Bar(crate::bar_menu::BarCommand::Settings)));
        assert_eq!(status(w.found.len()), "22 item(s) found");
    }

    #[test]
    fn programs_and_commands_rank_together() {
        // "ex": both Explorers' names have a word beginning with it, and Exit Bevel95's name
        // begins with it; the Windows Explorer command repeats no own program here, so it shows.
        let w = typed(Era::Win2000, "ex");
        assert_eq!(
            rows(&w),
            [
                ("Exit Bevel95".into(), "Bevel95".into()),
                ("Internet Explorer".into(), "Programs".into()),
                ("Windows Explorer".into(), "Bevel95".into()),
                ("Windows Explorer".into(), "Programs".into()),
            ],
            "Bevel95's own first among equals"
        );
        // With Bevel95's own Windows Explorer among the programs, the command would only repeat it.
        let mut all = programs(&menu());
        all.iter_mut().filter(|p| p.name == "Windows Explorer").for_each(|p| p.own = true);
        let w = FindWindow::new(Era::Win2000, all, true);
        assert_eq!(w.programs.iter().filter(|p| p.name == "Windows Explorer").count(), 1);
        // Every typed word must be found: "windows 95" finds the look; "run" Run... first.
        assert_eq!(rows(&typed(Era::Win95, "windows 95"))[0].0, "Windows 95 look");
        assert_eq!(rows(&typed(Era::Win95, "run"))[0].0, "Run...");
        assert!(typed(Era::Win95, "xyz").found.is_empty());
        // The commands' pictures are there at once; programs' still wanted.
        let w = typed(Era::Win95, "p");
        let wanted = w.wanted_icons();
        assert!(wanted.iter().all(|&i| w.programs[i].command.is_none()));
        assert!(w.found.iter().any(|&i| w.programs[i].command.is_some()));
    }

    #[test]
    fn a_new_reading_keeps_the_commands_and_the_selection() {
        let mut w = typed(Era::Win98, "size 3");
        assert_eq!(w.selected().unwrap().command, Some(Action::Size(3)));
        w.set_programs(programs(&menu()));
        assert_eq!(w.selected().unwrap().command, Some(Action::Size(3)));
        assert_eq!(w.programs.len(), 8 + quick::commands().len());
        assert!(w.wanted_icons().is_empty(), "the command's picture stays");
    }

    #[test]
    fn the_kind_of_target_is_told_by_what_it_is() {
        for program in [
            "C:\\Program Files\\Tool\\tool.exe",
            "C:\\WINDOWS\\system32\\CMD.EXE",
            "C:\\old\\edit.com",
            "C:\\scripts\\go.bat",
            "\"C:\\scripts\\go.cmd\"",
            "C:\\WINDOWS\\system32\\compmgmt.msc",
            "C:\\WINDOWS\\system32\\main.cpl",
            "C:\\old\\game.pif",
            "C:\\Users\\x\\Start Menu\\App.appref-ms",
            "shell:AppsFolder\\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
            "SHELL:APPSFOLDER\\Some.App!App",
        ] {
            assert_eq!(classify(program), Target::Program, "{program}");
        }
        for web in ["https://example.com/help", "HTTP://example.com", "ftp://x", "mailto:a@b", "www.example.com", "C:\\Links\\Site.url", "C:\\x\\Pinned.website"] {
            assert_eq!(classify(web), Target::Web, "{web}");
        }
        for document in [
            "C:\\Program Files\\Tool\\readme.txt",
            "C:\\Program Files\\Tool\\tool.chm",
            "C:\\Program Files\\Tool\\manual.pdf",
            "C:\\Program Files\\Tool\\help.hlp",
            "C:\\Program Files\\Tool\\index.html",
            "C:\\Program Files\\Tool\\Notes.MD",
            "C:\\Program Files\\Tool\\settings.ini",
            "C:\\Program Files\\Tool\\picture.jpeg",
            "C:\\Program Files\\Tool\\slides.pptx",
        ] {
            assert_eq!(classify(document), Target::Document, "{document}");
        }
        // Whatever is not known to be a document is listed: scripts and other launchers, an
        // installer, a file without an extension.
        for unknown in [
            "C:\\Tools\\go.vbs",
            "C:\\Tools\\go.js",
            "C:\\Tools\\go.wsf",
            "C:\\Tools\\go.ps1",
            "C:\\Tools\\go.hta",
            "C:\\Tools\\tool.jar",
            "C:\\Tools\\tool.py",
            "C:\\Tools\\keys.ahk",
            "C:\\Windows\\System32\\Bubbles.scr",
            "C:\\Users\\x\\Tool.application",
            "C:\\Program Files\\Tool\\setup.msi",
            "C:\\Program Files\\Tool\\LICENSE",
            "C:\\Program Files\\Tool.d\\notes",
            "",
            "  ",
        ] {
            assert_eq!(classify(unknown), Target::Unknown, "{unknown}");
        }
    }

    /// Paths in other alphabets whose 17th byte (the length of `programs::APP_PREFIX`) falls
    /// inside a letter are classed, not cut inside the letter (which would panic).
    #[test]
    fn paths_in_other_alphabets_are_classed() {
        let prefix = crate::programs::APP_PREFIX.len();
        let cases = [
            // Cyrillic: "а" takes bytes 16 and 17.
            ("C:\\Users\\Anna\\Рабочий стол\\x.exe", Target::Program),
            // Greek: "ά" takes bytes 16 and 17 (after "C:\Users\Jo\" and two letters).
            ("C:\\Users\\Jo\\Επάνω\\οδηγός.pdf", Target::Document),
            // Three-byte letters: the profile name's third letter takes bytes 15 to 17.
            ("C:\\Users\\山田太郎\\AppData\\Local\\Programs\\a.exe", Target::Program),
            ("C:\\Users\\山田太郎\\a.exe", Target::Program),
            ("C:\\Игры\\Ведьмак 3\\witcher3.exe", Target::Program),
            ("C:\\Users\\Jo\\Документы\\справка.chm", Target::Document),
        ];
        for (path, kind) in cases {
            assert!(!path.is_char_boundary(prefix), "the test's own case: byte {prefix} inside a letter in {path}");
            assert_eq!(classify(path), kind, "{path}");
        }
        // Shorter than the prefix, and ending inside it.
        assert_eq!(classify("Блокнот.exe"), Target::Program);
        assert_eq!(classify("шелл:AppsFolder\\x"), Target::Unknown);
    }

    #[test]
    fn only_programs_are_listed_unless_everything_is_chosen() {
        // The target rule.
        for kind in [Target::Program, Target::Unknown] {
            assert_eq!(hidden("Tool", kind), None, "{kind:?}");
        }
        for kind in [Target::Web, Target::Document, Target::Folder] {
            assert_eq!(hidden("Tool", kind), Some(Hidden::ByTarget), "{kind:?}");
        }
        // The name rule, whatever the target; case does not matter; only the first word, whole.
        for name in ["Uninstall Tool", "uninstall", "UNINSTALL TOOL", "About Tool", "  about", "About: Tool", "Uninstall-Tool"] {
            assert_eq!(hidden(name, Target::Program), Some(Hidden::ByName), "{name}");
            assert_eq!(hidden(name, Target::Unknown), Some(Hidden::ByName), "{name}");
        }
        for name in ["Tool Uninstaller", "All about Tool", "Abacus", "Unicode Tool", "Aboutface", "Uninstaller", "Abouts"] {
            assert_eq!(hidden(name, Target::Program), None, "{name}");
        }
        assert_eq!(hidden("About Tool", Target::Web), Some(Hidden::ByTarget), "the target rule first");

        let entry = |name: &str, kind: Target, own: bool| Program {
            name: name.into(),
            folder: ROOT.into(),
            target: format!("C:\\{name}.lnk"),
            own,
            command: None,
            words: &[],
            kind,
        };
        let all = vec![
            entry("Tool", Target::Program, false),
            entry("Tool Help", Target::Document, false),
            entry("Tool Website", Target::Web, false),
            entry("Uninstall Tool", Target::Program, false),
            entry("About Bevel95", Target::Program, true),
            entry("Shell thing", Target::Unknown, false),
        ];
        let names = |list: &[Program]| list.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&listed(all.clone(), false)), ["Tool", "About Bevel95", "Shell thing"], "Bevel95's own always");
        assert_eq!(names(&listed(all.clone(), true)), names(&all));
        // A hidden entry is not found by typing either; the commands are always there.
        let w = FindWindow::new(Era::Win98, listed(all, false), true);
        let found = |text: &str| find(&w.programs, text).iter().map(|&i| w.programs[i].name.clone()).collect::<Vec<_>>();
        assert_eq!(found("tool"), ["Tool"]);
        assert!(found("uninstall").is_empty());
        assert_eq!(found("bevel95 settings"), ["Bevel95 Settings"]);
    }

    /// A reading that takes away the selected program leaves nothing selected: an Enter that
    /// was waiting starts nothing, as no row was shown selected; a move, typing or a click
    /// selects again.
    #[test]
    fn enter_starts_only_a_row_shown_selected() {
        let entry = |name: &str, kind: Target| Program {
            name: name.into(),
            folder: ROOT.into(),
            target: format!("C:\\{name}.lnk"),
            own: false,
            command: None,
            words: &[],
            kind,
        };
        let before = vec![entry("Manual", Target::Unknown), entry("Mandelbrot", Target::Unknown), entry("Mangle", Target::Unknown)];
        let mut w = FindWindow::new(Era::Win98, listed(before.clone(), false), true);
        let mut c = Controller::default();
        for ch in "man".chars() {
            w.typed(ch);
        }
        assert_eq!(w.selected().unwrap().name, "Mandelbrot", "A to Z: the first row");
        // The reading finds that Mandelbrot is a document: it goes, and nothing is selected.
        let mut after = before.clone();
        after[1].kind = Target::Document;
        w.set_programs(listed(after.clone(), false));
        assert!(w.selection_lost && w.selected().is_none());
        assert!(!w.enabled(Part::Open));
        assert_eq!(c.key(&mut w, Key::Enter, Mods::default()), vec![], "the waiting Enter starts nothing");
        // Down selects the first row shown; Enter then starts it.
        c.key(&mut w, Key::Down, Mods::default());
        assert_eq!((w.selected().unwrap().name.as_str(), w.sel), ("Mangle", 0));
        assert_eq!(c.key(&mut w, Key::Enter, Mods::default()), vec![Effect::Activate(Part::Open)]);
        // Typing selects again too.
        w.set_programs(listed(before.clone(), false));
        assert_eq!(w.selected().unwrap().name, "Mangle", "still found: it stays selected");
        w.set_programs(listed(after.iter().filter(|p| p.name != "Mangle").cloned().collect(), false));
        assert!(w.selected().is_none());
        w.typed('u');
        assert_eq!(w.selected().unwrap().name, "Manual");
        // So does a click on a row.
        w.typed('\u{8}');
        w.set_programs(listed(vec![entry("Manual", Target::Document), entry("Mangle", Target::Unknown)], false));
        assert!(w.selected().is_none());
        let r = w.rect(Part::Row(0)).unwrap();
        c.mouse_down(&mut w, r.x + 40, r.y + 4);
        c.mouse_up(&mut w, r.x + 40, r.y + 4);
        assert_eq!(w.selected().unwrap().name, "Mangle");
        // Rows that come where nothing was found were never shown selected either.
        w.typed('z');
        assert!(w.found.is_empty());
        w.set_programs(listed(vec![entry("Manzanita", Target::Unknown)], false));
        assert_eq!(w.found.len(), 1);
        assert_eq!(c.key(&mut w, Key::Enter, Mods::default()), vec![]);
    }

    #[test]
    fn same_names_rank_by_folder() {
        let mut tree = Folder::default();
        tree.add(&["Tools"], Item { name: "Paint".into(), target: "b".into() });
        tree.add(&[], Item { name: "Paint".into(), target: "a".into() });
        let all = programs(&tree);
        let found = find(&all, "pa");
        assert_eq!(found.iter().map(|&i| all[i].folder.as_str()).collect::<Vec<_>>(), ["Programs", "Programs\\Tools"]);
    }

    #[test]
    fn other_alphabets_are_found_whatever_their_case() {
        let mut tree = Folder::default();
        tree.add(&["Стандартные"], Item { name: "Блокнот".into(), target: "a.exe".into() });
        tree.add(&[], Item { name: "Ελληνικός Επεξεργαστής".into(), target: "b.exe".into() });
        tree.add(&[], Item { name: "İstanbul Haritası".into(), target: "c.exe".into() });
        tree.add(&[], Item { name: "Łódź".into(), target: "d.exe".into() });
        let all = programs(&tree);
        let names = |text: &str| find(&all, text).into_iter().map(|i| all[i].name.clone()).collect::<Vec<_>>();
        assert_eq!(names("БЛОК"), ["Блокнот"]);
        assert_eq!(names("блокнот"), ["Блокнот"]);
        assert_eq!(names("ΕΠΕΞ"), ["Ελληνικός Επεξεργαστής"], "a word of the name begins with it");
        assert_eq!(names("ελληνικόσ"), ["Ελληνικός Επεξεργαστής"], "final sigma is sigma");
        assert_eq!(names("istanbul"), ["İstanbul Haritası"]);
        assert_eq!(names("HARITASI"), ["İstanbul Haritası"]);
        assert_eq!(names("łó"), ["Łódź"]);
        assert_eq!(names("ŁÓDŹ"), ["Łódź"]);
    }

    #[test]
    fn typing_finds_and_selects_the_first() {
        let mut w = FindWindow::new(Era::Win98, programs(&menu()), true);
        let mut c = Controller::default();
        assert_eq!((w.found.len(), w.sel), (8, 0));
        assert_eq!(w.title(), "Quick Search");
        assert_eq!(FindWindow::new(Era::Win2000, Vec::new(), true).title(), "Quick Search", "one name in every look");
        c.key(&mut w, Key::Down, Mods::default());
        c.key(&mut w, Key::Down, Mods::default());
        assert_eq!(w.sel, 2);
        for ch in "min".chars() {
            w.typed(ch);
        }
        assert_eq!((w.selected().unwrap().name.as_str(), w.sel), ("Minesweeper", 0), "every change selects the first");
        assert_eq!(c.key(&mut w, Key::Enter, Mods::default()), vec![Effect::Activate(Part::Open)]);
        assert_eq!(c.key(&mut w, Key::Escape, Mods::default()), vec![Effect::Activate(Part::TitleClose)]);
        // Backspace, the caret keys, Delete, paste.
        w.typed('\u{8}');
        assert_eq!((w.text.as_str(), w.found.len()), ("mi", 1));
        c.key(&mut w, Key::Home, Mods::default());
        c.key(&mut w, Key::Delete, Mods::default());
        assert_eq!((w.text.as_str(), w.caret), ("i", 0));
        assert_eq!(c.key(&mut w, Key::Other(u16::from(b'V')), Mods { ctrl: true, shift: false }), vec![Effect::Paste]);
        w.paste("pa\n");
        assert_eq!((w.text.as_str(), w.caret), ("pai", 2));
        assert_eq!(w.selected().unwrap().name, "Paint");
        // Tab and Space stay in the field (the space comes as a character).
        c.key(&mut w, Key::Tab, Mods::default());
        assert_eq!(c.key(&mut w, Key::Space, Mods::default()), vec![]);
        assert_eq!(w.focus(), Part::Field);
        // Nothing found: Enter does nothing; the status bar says so.
        w.typed('q');
        assert!(w.found.is_empty() && w.selected().is_none());
        assert_eq!(c.key(&mut w, Key::Enter, Mods::default()), vec![]);
        assert_eq!(status(w.found.len()), "0 item(s) found");
        c.key(&mut w, Key::Down, Mods::default());
        assert_eq!(w.sel, 0);
    }

    #[test]
    fn a_long_list_pages_and_scrolls() {
        let item = |name: String| Item { target: format!("C:\\{name}.lnk"), name };
        let mut tree = Folder::default();
        for i in 0..25 {
            tree.add(&[], item(format!("Program {i:02}")));
        }
        let mut w = FindWindow::new(Era::Win95, programs(&tree), true);
        let mut c = Controller::default();
        // 25 rows, 10 show; the header and rows leave room for the scroll bar.
        assert_eq!(w.rect(Part::Row(0)), Some(Rect::new(11, 59 + 17, 292, 16)));
        assert!(w.rect(Part::ScrollThumb).is_some());
        assert_eq!(w.rect(Part::Row(10)), None);
        c.key(&mut w, Key::PageDown, Mods::default());
        assert_eq!((w.sel, w.top), (9, 0));
        c.key(&mut w, Key::PageDown, Mods::default());
        assert_eq!((w.sel, w.top), (18, 9));
        c.key(&mut w, Key::PageDown, Mods::default());
        assert_eq!((w.sel, w.top), (24, 15));
        c.key(&mut w, Key::PageUp, Mods::default());
        assert_eq!((w.sel, w.top), (15, 15));
        w.wheel(1);
        assert_eq!(w.top, 12);
        w.wheel(-10);
        assert_eq!(w.top, 15);
        // A click selects; a double-click starts.
        let r = w.rect(Part::Row(17)).unwrap();
        assert_eq!(c.mouse_down(&mut w, r.x + 40, r.y + 4), vec![]);
        assert_eq!(w.sel, 17);
        let r = w.rect(Part::Row(20)).unwrap();
        assert_eq!(w.double_click(r.x + 40, r.y + 4), Some(vec![Effect::Activate(Part::Open)]));
        assert_eq!(w.selected().unwrap().name, "Program 20");
        // The scroll bar's buttons, held, scroll on.
        let up = w.rect(Part::ScrollUp).unwrap();
        assert_eq!(c.mouse_down(&mut w, up.x + 4, up.y + 4), vec![Effect::Capture]);
        assert_eq!(w.top, 14);
        assert!(scroll_on(&mut w) && w.top == 13);
        c.mouse_up(&mut w, up.x + 4, up.y + 4);
        // Typing narrows the list: no scroll bar, the rows as wide as the list's inside.
        for ch in "program 2".chars() {
            w.typed(ch);
        }
        // 02, 12 and 20 to 24.
        assert_eq!((w.found.len(), w.top, w.sel), (7, 0, 0));
        assert_eq!(w.rect(Part::Row(0)).unwrap().w, 308);
        assert!(w.rect(Part::ScrollThumb).is_none());
    }

    /// Every character 6 pixels wide.
    fn test_font() -> crate::text::Font {
        let glyph = crate::text::Glyph { ch: '?', advance: 6, ink_x: 0, ink_w: 0, ink: Vec::new() };
        crate::text::Font::new("test".into(), false, 13, 11, vec![glyph]).unwrap()
    }

    #[test]
    fn the_line_between_the_columns_drags_and_fits() {
        let mut w = FindWindow::new(Era::Win98, programs(&menu()), true);
        let f = test_font();
        w.layout(&Fonts { regular: &f, bold: &f });
        let mut c = Controller::default();
        // 8 rows, no scroll bar: the rows 308 wide from x 11; In Folder starts at 11 + 150.
        let (edge, y) = (11 + NAME_W, 59 + 5);
        assert_eq!(w.name_width(), NAME_W);
        // It takes the mouse 8 left to 7 right of In Folder's first pixel, with the double bar.
        for x in [edge - 8, edge, edge + 7] {
            assert_eq!(w.hit(x, y), Some(Part::Divider), "{x}");
            assert_eq!(w.pointer(x, y), kit::Pointer::HeaderDivider);
        }
        assert_eq!(w.hit(edge - 9, y), Some(Part::List));
        assert_eq!(w.pointer(edge - 9, y), kit::Pointer::Arrow);
        assert_eq!(w.hit(edge, 59 + 17 + 4), Some(Part::Row(0)), "only in the header");
        // Dragged 30 right: the line moves, the column only when let go.
        assert_eq!(c.mouse_down(&mut w, edge + 2, y), vec![Effect::Capture]);
        assert!(c.mouse_move(&mut w, edge + 32, y));
        assert_eq!(w.name_width(), NAME_W);
        assert_eq!(w.drag_line(), Some(NAME_W + 30));
        assert_eq!(c.mouse_up(&mut w, edge + 32, y), vec![Effect::Release, Effect::Activate(Part::Divider)]);
        assert_eq!((w.name_width(), w.drag_line()), (NAME_W + 30, None));
        assert_eq!(w.hit(edge + 30, y), Some(Part::Divider), "the line's place follows");
        // Neither column can vanish: far right leaves In Folder 40, far left leaves Name 38.
        let edge = 11 + w.name_width();
        c.mouse_down(&mut w, edge, y);
        c.mouse_move(&mut w, 2000, y);
        c.mouse_up(&mut w, 2000, y);
        assert_eq!(w.name_width(), 308 - 40);
        let edge = 11 + w.name_width();
        c.mouse_down(&mut w, edge, y);
        c.mouse_move(&mut w, -2000, y);
        c.mouse_up(&mut w, -2000, y);
        assert_eq!(w.name_width(), NAME_X + 16);
        // A press let go where it was changes nothing (and tells nothing).
        let edge = 11 + w.name_width();
        c.mouse_down(&mut w, edge, y);
        assert_eq!(c.mouse_up(&mut w, edge, y), vec![Effect::Release]);
        // A double-click fits Name to the longest name found (6 pixels a letter here).
        let longest = w.found.iter().map(|&i| w.programs[i].name.chars().count() as i32).max().unwrap();
        assert_eq!(w.double_click(edge, y), Some(vec![Effect::Activate(Part::Divider)]));
        assert_eq!(w.name_width(), longest * 6 + NAME_X + CELL_PAD);
        // A remembered width out of reach (the list grew a scroll bar) is shown within reach.
        w.name_w = 400;
        assert_eq!(w.name_width(), 308 - 40);
    }

    #[test]
    fn the_name_width_is_kept_as_text() {
        assert_eq!(read_state(&state_text(212)), Some(212));
        assert_eq!(read_state("name_width=90\n"), Some(90));
        for bad in ["", "name_width = wide", "name_width = 5", "name_width = 99999", "other = 120"] {
            assert_eq!(read_state(bad), None, "{bad}");
        }
    }

    #[test]
    fn pictures_are_wanted_for_the_rows_that_show_and_kept() {
        let item = |name: String| Item { target: format!("C:\\{name}.lnk"), name };
        let mut tree = Folder::default();
        for i in 0..15 {
            tree.add(&[], item(format!("Program {i:02}")));
        }
        let mut w = FindWindow::new(Era::Win2000, programs(&tree), false);
        assert_eq!(w.wanted_icons(), (0..10).collect::<Vec<_>>());
        for i in w.wanted_icons() {
            w.set_icon(i, None);
        }
        assert!(w.wanted_icons().is_empty());
        w.wheel(-1);
        assert_eq!(w.wanted_icons(), [10, 11, 12]);
        // Read again with one more program: the pictures already fetched stay, the selection
        // stays on its program, the text stays.
        // "1" begins a word of 10 to 14 and is inside 01.
        w.typed('1');
        let found_programs: Vec<usize> = w.found.iter().copied().filter(|&i| w.programs[i].command.is_none()).collect();
        assert_eq!(names(&w.programs, &found_programs), ["Program 10", "Program 11", "Program 12", "Program 13", "Program 14", "Program 01"]);
        // The command "16 Colors" begins with it too, and Bevel95's own come first: two down.
        assert_eq!(w.selected().unwrap().name, "16 Colors");
        w.key(Key::Down, Mods::default());
        w.key(Key::Down, Mods::default());
        assert_eq!(w.selected().unwrap().name, "Program 11");
        tree.add(&[], item("Program 1A".into()));
        tree.sort();
        w.set_programs(programs(&tree));
        assert!(w.loaded);
        assert_eq!((w.text.as_str(), w.selected().unwrap().name.as_str()), ("1", "Program 11"));
        assert_eq!(w.wanted_icons(), [10, 11, 12, 13, 14, 15], "Program 1A is new; 01 was fetched");
    }
}

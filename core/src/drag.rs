//! Dragging files and folders: what a drop does in each look, the right-button drag's menu, the
//! pointers with their little marks, and the list of what Undo can take back. The rules were
//! measured on the three Reference PCs (`private/probes/drag18`, captures
//! `private/reference/refs/vm/<era>-truecolor-drag-*`):
//! - A drag with the left button moves within one drive and copies to another; a program file
//!   (.exe) makes a shortcut in Windows 95 and 98 (2000 moved it like any file). Ctrl copies,
//!   Ctrl+Shift makes a shortcut, Shift moves; the mark changes as soon as a key goes down or up.
//! - A drag with the right button asks at the drop: Move Here, Copy Here, Create Shortcut(s)
//!   Here, Cancel (2000: Copy Here first), what a left drag would do in bold.
//! - Undo: "Undo Move", "Undo Copy", "Undo Rename", "Undo Delete", several steps back; a shortcut
//!   made by a drag is not recorded (98).
//!
//! The platform does the work with Windows' own file machinery; this module only decides.

use crate::art::Look;
use crate::paint::{Menu, MenuEntry};
use crate::theme::Era;

/// What a drop does with the files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    Move,
    Copy,
    /// A shortcut to each file at the drop place.
    Link,
}

/// What the program a drag comes from allows a drop to do (OLE's offered effects). Bevel95's own
/// drags and Windows' Explorer offer all three; a browser's list of downloads only a copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offered {
    pub moves: bool,
    pub copy: bool,
    pub link: bool,
}

impl Offered {
    pub const ALL: Offered = Offered { moves: true, copy: true, link: true };

    pub fn has(self, e: Effect) -> bool {
        match e {
            Effect::Move => self.moves,
            Effect::Copy => self.copy,
            Effect::Link => self.link,
        }
    }
}

/// The one rule for what a drop does (the safety review of 2026-11-18, A1): the look's wish
/// (`effect`, or a move for the Recycle Bin) narrowed by what the source offers, decided before
/// anything is done, for the pointer and the drop alike. A wish that is offered stays; else a
/// copy if offered, else a move if offered, else nothing (the "no" sign). A shortcut only where
/// one was wished for. The Recycle Bin (`recycle_bin`) takes only what may be moved: anything
/// else is refused, never turned into a copy.
pub fn narrow(wish: Option<Effect>, offered: Offered, recycle_bin: bool) -> Option<Effect> {
    let wish = wish?;
    if recycle_bin {
        return (wish == Effect::Move && offered.moves).then_some(Effect::Move);
    }
    if offered.has(wish) {
        return Some(wish);
    }
    if wish == Effect::Link {
        return None;
    }
    [Effect::Copy, Effect::Move].into_iter().find(|&e| offered.has(e))
}

/// What the right button's menu at a drop offers: the three (or only Move, for the Recycle Bin
/// and an icon moved about in its own window), each only if the source offers it.
pub fn menu_effects(offered: Offered, only_move: bool) -> Vec<Effect> {
    let all: &[Effect] = if only_move { &[Effect::Move] } else { &[Effect::Move, Effect::Copy, Effect::Link] };
    all.iter().copied().filter(|&e| offered.has(e)).collect()
}

/// The keys held during a drag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    pub ctrl: bool,
    pub shift: bool,
}

/// The drive (or network share) a path lies on, in lower case: "c:" or "\\server\share".
pub fn drive_of(path: &str) -> String {
    let p = path.replace('/', "\\");
    if let Some(rest) = p.strip_prefix("\\\\") {
        let mut parts = rest.splitn(3, '\\');
        let server = parts.next().unwrap_or("");
        let share = parts.next().unwrap_or("");
        return format!("\\\\{server}\\{share}").to_lowercase();
    }
    p.chars().take(2).collect::<String>().to_lowercase()
}

/// Whether a file is a program in the sense of the 95 and 98 shell's drag rule (it made a
/// shortcut of a dragged program instead of moving it; only .exe was measured).
pub fn is_program(path: &str) -> bool {
    path.to_lowercase().ends_with(".exe")
}

/// What a left-button drag does without keys: move within one drive, copy to another; in 95 and
/// 98 a shortcut when every dragged file is a program.
pub fn default_effect(era: Era, same_drive: bool, all_programs: bool) -> Effect {
    if era != Era::Win2000 && all_programs {
        return Effect::Link;
    }
    if same_drive { Effect::Move } else { Effect::Copy }
}

/// What a drop does with `keys` held (the same in all three looks): Ctrl+Shift a shortcut, Ctrl
/// a copy, Shift a move, else the default.
pub fn effect(era: Era, keys: Keys, same_drive: bool, all_programs: bool) -> Effect {
    match (keys.ctrl, keys.shift) {
        (true, true) => Effect::Link,
        (true, false) => Effect::Copy,
        (false, true) => Effect::Move,
        (false, false) => default_effect(era, same_drive, all_programs),
    }
}

/// Whether the folder `dest` is one of `sources` or lies inside one of them (a folder cannot go
/// into itself).
pub fn into_itself(sources: &[String], dest: &str) -> bool {
    let d = dest.trim_end_matches('\\').to_lowercase();
    sources.iter().any(|s| {
        let s = s.trim_end_matches('\\').to_lowercase();
        d == s || d.starts_with(&format!("{s}\\"))
    })
}

/// Whether every one of `sources` already lies directly in the folder `dest` (a move there
/// changes nothing; in the same window it only moves the icons).
pub fn all_in(sources: &[String], dest: &str) -> bool {
    let d = dest.trim_end_matches('\\').to_lowercase();
    !sources.is_empty()
        && sources.iter().all(|s| {
            let s = s.to_lowercase();
            s.rsplit_once('\\').is_some_and(|(parent, _)| parent.trim_end_matches('\\') == d)
        })
}

/// The right-button drag's menu: (the menu, what each row does; `None` for the separator and
/// Cancel). `default` is drawn bold. 95 and 98: Move Here, Copy Here, Create Shortcut(s) Here,
/// a line, Cancel; 2000 the same with Copy Here first. Only the entries in `offered` show (a drop
/// on the Recycle Bin can only move).
pub fn drop_menu(era: Era, default: Effect, offered: &[Effect]) -> (Menu, Vec<Option<Effect>>) {
    let order: [Effect; 3] = if era == Era::Win2000 {
        [Effect::Copy, Effect::Move, Effect::Link]
    } else {
        [Effect::Move, Effect::Copy, Effect::Link]
    };
    let mut entries = Vec::new();
    let mut does = Vec::new();
    for e in order.into_iter().filter(|e| offered.contains(e)) {
        let label = match e {
            Effect::Move => "&Move Here",
            Effect::Copy => "&Copy Here",
            Effect::Link => "Create &Shortcut(s) Here",
        };
        let mut entry = MenuEntry::item(label, None);
        entry.bold = e == default;
        entries.push(entry);
        does.push(Some(e));
    }
    entries.push(MenuEntry::separator());
    does.push(None);
    entries.push(MenuEntry::item("Cancel", None));
    does.push(None);
    (Menu { show_keys: era.always_shows_keys(), ..Menu::new(entries) }, does)
}

/// One thing Undo can take back. Paths are whole paths as Windows names them.
///
/// Undo takes back only what it can prove (the outside review of 2026-11-17): what a move or a
/// copy made is decided before the operation (`plan`: each item's place at the destination and
/// whether it was free) and checked after it, item by item (`moved`, `copied`); nothing else of
/// the destination folder is ever looked at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Items moved: (where each is now, where it was).
    Move(Vec<(String, String)>),
    /// Items copied: each copy made, with what it was like when made.
    Copy(Vec<Made>),
    /// An item renamed: (its name now, its name before), whole paths.
    Rename(String, String),
    /// Items sent to the Recycle Bin: where they were, and the operation's start and end
    /// (seconds since 1970), so the Recycle Bin's own records of them can be told from older and
    /// newer ones of the same name.
    Delete(Vec<String>, u64, u64),
}

/// What an item was like: a file's size and time of last change; a folder's number of items,
/// their total size and the newest time of change in it (read through the whole folder).
/// Times are nanoseconds since 1970.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Proof {
    pub folder: bool,
    pub items: u64,
    pub bytes: u64,
    pub newest: u64,
}

impl Proof {
    /// Whether a copy just made is like its source: a file of the same size and time of change
    /// (Windows' copy keeps the time), a folder of the same number of items and total size.
    pub fn copy_of(&self, source: &Proof) -> bool {
        self.folder == source.folder
            && self.items == source.items
            && self.bytes == source.bytes
            && (self.folder || self.newest == source.newest)
    }
}

/// One copy Undo Copy may take back: where it is and what it was like when made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Made {
    pub path: String,
    pub proof: Proof,
}

/// One item of a move or a copy, decided before the operation: where it is, where it goes
/// (whole paths), and whether that place was free.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub from: String,
    pub to: String,
    pub free: bool,
}

/// The whole path of `name` in the folder `dir`.
pub fn join(dir: &str, name: &str) -> String {
    format!("{}\\{name}", dir.trim_end_matches('\\'))
}

/// The name Windows (today's, with FOF_RENAMEONCOLLISION) gives a copy of `name` in its own
/// folder: "a - Copy.txt", then "a - Copy (2).txt" and so on; a folder's dot is no extension
/// ("v1.2 - Copy"), a file's last dot is ("arc.tar - Copy.gz"); measured on Windows 11.
pub fn copy_name(name: &str, folder: bool, taken: impl Fn(&str) -> bool) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if !folder && i > 0 => name.split_at(i),
        _ => (name, ""),
    };
    crate::desktop::new_name(&format!("{stem} - Copy"), ext, taken)
}

/// Where each of `sources` goes in the folder `dir`, decided before a move or copy. A copy of an
/// item of `dir` itself (`own_folder`) gets the copy's name, chosen free here (so it is known
/// exactly, not read off the folder afterwards); everything else keeps its name, and its place
/// is free only if nothing is there (`exists`) and no earlier item of the same operation goes
/// there.
pub fn plan(
    sources: &[String],
    dir: &str,
    copy: bool,
    own_folder: impl Fn(&str) -> bool,
    is_folder: impl Fn(&str) -> bool,
    exists: impl Fn(&str) -> bool,
) -> Vec<Planned> {
    let mut out: Vec<Planned> = Vec::new();
    for from in sources {
        let name = name_of(from).to_string();
        let planned = |p: &str, out: &[Planned]| out.iter().any(|o| o.to.eq_ignore_ascii_case(p));
        let to = if copy && own_folder(from) {
            join(dir, &copy_name(&name, is_folder(from), |n| exists(&join(dir, n)) || planned(&join(dir, n), &out)))
        } else {
            join(dir, &name)
        };
        let free = !exists(&to) && !planned(&to, &out);
        out.push(Planned { from: from.clone(), to, free });
    }
    out
}

/// What a move did that Undo can take back: each item whose place was free before, that is
/// there now and gone from where it was, as (now, was). An item that merged into or replaced
/// something already there is not among them.
pub fn moved(plan: &[Planned], exists: impl Fn(&str) -> bool) -> Vec<(String, String)> {
    plan.iter().filter(|p| p.free && exists(&p.to) && !exists(&p.from)).map(|p| (p.to.clone(), p.from.clone())).collect()
}

/// What a copy made that Undo can take back: each item whose place was free before and that is
/// there now, like its source (`proof_of` reads an item as it is now), with its proof.
pub fn copied(plan: &[Planned], proof_of: impl Fn(&str) -> Option<Proof>) -> Vec<Made> {
    plan.iter()
        .filter(|p| p.free)
        .filter_map(|p| {
            let proof = proof_of(&p.to)?;
            let source = proof_of(&p.from)?;
            proof.copy_of(&source).then(|| Made { path: p.to.clone(), proof })
        })
        .collect()
}

/// What a move or copy leaves for Undo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recorded {
    /// What it provably made (all of it or a part): a new step.
    Step(Step),
    /// It provably did nothing: the list stays as it was.
    Nothing,
    /// Something may have happened that cannot be taken back: the list is emptied.
    Unproven,
}

/// What an operation planned as `plan` leaves for Undo, given the step of what it provably made
/// (`moved`, `copied`; None: nothing of it). Without a step the list is left as it is only when
/// the operation provably did nothing: every planned place was free before and is still free
/// (`free_now`), and every source is still where it was (`still_there`); nothing was made,
/// nothing went. In every other case, and whenever in doubt (a place or source that cannot be
/// read counts as changed, so the callbacks answer true only when sure), the list is emptied.
pub fn recorded(
    plan: &[Planned],
    step: Option<Step>,
    free_now: impl Fn(&str) -> bool,
    still_there: impl Fn(&str) -> bool,
) -> Recorded {
    match step {
        Some(step) => Recorded::Step(step),
        None if !plan.is_empty() && plan.iter().all(|p| p.free && free_now(&p.to) && still_there(&p.from)) => Recorded::Nothing,
        None => Recorded::Unproven,
    }
}

/// Undo Copy's choice: the copies still exactly as they were made (to the Recycle Bin, after the
/// look's question) and those changed since (left, with the look's message); a copy that is gone
/// is in neither.
pub fn undo_copy(made: &[Made], now: impl Fn(&str) -> Option<Proof>) -> (Vec<Made>, Vec<String>) {
    let mut take = Vec::new();
    let mut left = Vec::new();
    for m in made {
        match now(&m.path) {
            Some(p) if p == m.proof => take.push(m.clone()),
            Some(_) => left.push(m.path.clone()),
            None => {}
        }
    }
    (take, left)
}

/// Edit's Undo while there is nothing to take back (grayed): "Undo" with "Ctrl+Z" in all three
/// (`win98-truecolor-drag-undo-empty-editmenu`, 95's `win95-truecolor-folder-mc-menu-edit`; 95
/// and 98 drop the Ctrl+Z once the entry names a step).
pub fn empty_undo_label(_era: Era) -> &'static str {
    "&Undo\tCtrl+Z"
}

/// How many steps back Undo goes (the Reference PCs went back several; their limit was not
/// reached: Bevel95's ten, PROVISIONAL).
pub const DEPTH: usize = 10;

fn name_of(path: &str) -> &str {
    path.trim_end_matches('\\').rsplit('\\').next().unwrap_or(path)
}

impl Step {
    /// Every path the step names (where items are now and where they were).
    pub fn paths(&self) -> Vec<&str> {
        match self {
            Step::Move(moves) => moves.iter().flat_map(|(now, was)| [now.as_str(), was.as_str()]).collect(),
            Step::Copy(made) => made.iter().map(|m| m.path.as_str()).collect(),
            Step::Rename(now, was) => vec![now.as_str(), was.as_str()],
            Step::Delete(paths, ..) => paths.iter().map(String::as_str).collect(),
        }
    }

    /// Whether the step is all of a live test's own items (each named "Bevel95 test ..."): only
    /// such a step is taken back by a test's Undo (the third safety review of 2026-11-18, C4).
    pub fn test_only(&self) -> bool {
        let paths = self.paths();
        !paths.is_empty()
            && paths.iter().all(|p| p.trim_end_matches('\\').rsplit('\\').next().is_some_and(|name| name.starts_with("Bevel95 test")))
    }

    /// The word after "Undo".
    pub fn word(&self) -> &'static str {
        match self {
            Step::Move(_) => "Move",
            Step::Copy(_) => "Copy",
            Step::Rename(..) => "Rename",
            Step::Delete(..) => "Delete",
        }
    }

    /// The Edit menu's entry (and the menus on the empty area and the desktop): "&Undo Move", with
    /// "Ctrl+Z" beside it in 2000 only (the Reference PCs).
    pub fn label(&self, era: Era) -> String {
        let key = if era == Era::Win2000 { "\tCtrl+Z" } else { "" };
        format!("&Undo {}{key}", self.word())
    }

    /// The status bar's words for the entry (98: "Undo Move of 'a3'", "Undo Rename of 'a2' to
    /// 'r2'"). Several items: "of N items" (not measured, PROVISIONAL).
    pub fn help(&self) -> String {
        let of = |paths: Vec<&str>| match paths.as_slice() {
            [one] => format!("'{}'", name_of(one)),
            many => format!("{} items", many.len()),
        };
        match self {
            Step::Move(pairs) => format!("Undo Move of {}", of(pairs.iter().map(|p| p.1.as_str()).collect())),
            Step::Copy(made) => format!("Undo Copy of {}", of(made.iter().map(|m| m.path.as_str()).collect())),
            Step::Rename(now, was) => format!("Undo Rename of '{}' to '{}'", name_of(was), name_of(now)),
            Step::Delete(was, ..) => format!("Undo Delete of {}", of(was.iter().map(String::as_str).collect())),
        }
    }

    /// One line of `undo.txt`: the kind, then the fields, tab-separated (no Windows file name holds
    /// a tab). A copy: per copy its path, "file" or "folder", items, bytes and newest change.
    fn to_line(&self) -> String {
        match self {
            Step::Move(pairs) => {
                let mut s = String::from("move");
                for (now, was) in pairs {
                    s.push_str(&format!("\t{now}\t{was}"));
                }
                s
            }
            Step::Copy(made) => {
                let mut s = String::from("copy");
                for m in made {
                    let p = &m.proof;
                    let kind = if p.folder { "folder" } else { "file" };
                    s.push_str(&format!("\t{}\t{kind}\t{}\t{}\t{}", m.path, p.items, p.bytes, p.newest));
                }
                s
            }
            Step::Rename(now, was) => format!("rename\t{now}\t{was}"),
            Step::Delete(was, from, until) => {
                std::iter::once(format!("delete\t{from}\t{until}")).chain(was.iter().cloned()).collect::<Vec<_>>().join("\t")
            }
        }
    }

    fn from_line(line: &str) -> Option<Step> {
        let mut f = line.split('\t');
        let kind = f.next()?;
        let rest: Vec<String> = f.map(str::to_string).collect();
        let step = match kind {
            "move" if !rest.is_empty() && rest.len().is_multiple_of(2) => {
                Step::Move(rest.chunks(2).map(|c| (c[0].clone(), c[1].clone())).collect())
            }
            "copy" if !rest.is_empty() && rest.len().is_multiple_of(5) => {
                let mut made = Vec::new();
                for c in rest.chunks(5) {
                    let folder = match c[1].as_str() {
                        "folder" => true,
                        "file" => false,
                        _ => return None,
                    };
                    let proof = Proof { folder, items: c[2].parse().ok()?, bytes: c[3].parse().ok()?, newest: c[4].parse().ok()? };
                    made.push(Made { path: c[0].clone(), proof });
                }
                Step::Copy(made)
            }
            "rename" if rest.len() == 2 => Step::Rename(rest[0].clone(), rest[1].clone()),
            "delete" if rest.len() >= 3 => Step::Delete(rest[2..].to_vec(), rest[0].parse().ok()?, rest[1].parse().ok()?),
            _ => return None,
        };
        Some(step)
    }
}

/// The first line of `undo.txt` in the format of 2026-11-17 (copies with their proof). A file
/// without it is of the older format, whose steps cannot be proven: they are dropped.
const UNDO_HEADER: &str = "bevel95 undo 2";

/// What Undo can take back, the newest last. Shared by the desktop and every folder window (the
/// shell's one list in Windows): kept in `undo.txt` in the data folder, which each program reads
/// before it shows or carries out Undo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UndoList {
    pub steps: Vec<Step>,
}

impl UndoList {
    pub fn parse(text: &str) -> UndoList {
        let mut lines = text.lines();
        if lines.next().map(str::trim_end) != Some(UNDO_HEADER) {
            return UndoList::default();
        }
        UndoList { steps: lines.filter_map(Step::from_line).collect() }
    }

    pub fn to_text(&self) -> String {
        std::iter::once(format!("{UNDO_HEADER}\n")).chain(self.steps.iter().map(|s| s.to_line() + "\n")).collect()
    }

    /// What an operation leaves for Undo: its step, or, when nothing of it could be recorded (a
    /// merge, a replacement, a drop of which nothing is provably there), an empty list, so that
    /// Undo never reaches past it to an older step.
    pub fn record(&mut self, step: Option<Step>) {
        match step {
            Some(step) => self.push(step),
            None => self.steps.clear(),
        }
    }

    /// What a move or copy leaves (`recorded`): its step, nothing at all (an operation that
    /// provably did nothing leaves the list alone), or an empty list.
    pub fn leave(&mut self, r: Recorded) {
        match r {
            Recorded::Step(step) => self.push(step),
            Recorded::Nothing => {}
            Recorded::Unproven => self.steps.clear(),
        }
    }

    /// A new step; the oldest goes beyond `DEPTH`.
    pub fn push(&mut self, step: Step) {
        self.steps.push(step);
        if self.steps.len() > DEPTH {
            let extra = self.steps.len() - DEPTH;
            self.steps.drain(..extra);
        }
    }

    pub fn last(&self) -> Option<&Step> {
        self.steps.last()
    }

    pub fn pop(&mut self) -> Option<Step> {
        self.steps.pop()
    }
}

/// The message when a step cannot be taken back (an item is gone or its old place is taken):
/// (title, text). 98: "Error Renaming File" / "Cannot rename file:  File system error (1026)."
/// (two spaces, as on the PC); 2000: "Error Renaming File or Folder" / "Cannot rename file:
/// Cannot read from the source file or disk." (both measured after an impossible Undo Rename).
/// 95 had 98's shell. A move or copy taken back says "move" and "delete" in the same pattern
/// (not measured, PROVISIONAL).
pub fn undo_failed(era: Era, step: &Step) -> (String, String) {
    let (verb, title98, title2000) = match step {
        Step::Rename(..) => ("rename", "Error Renaming File", "Error Renaming File or Folder"),
        Step::Move(_) | Step::Delete(..) => ("move", "Error Moving File", "Error Moving File or Folder"),
        Step::Copy(_) => ("delete", "Error Deleting File", "Error Deleting File or Folder"),
    };
    match era {
        Era::Win2000 => (title2000.into(), format!("Cannot {verb} file: Cannot read from the source file or disk.")),
        _ => (title98.into(), format!("Cannot {verb} file:  File system error (1026).")),
    }
}

/// The message when Undo Copy leaves copies that were changed after they were made: (title,
/// text), in the pattern of `undo_failed` (Bevel95's own words, PROVISIONAL: the Reference PCs
/// deleted a changed copy).
pub fn copies_left(era: Era, left: &[String]) -> (String, String) {
    let gap = if era == Era::Win2000 { " " } else { "  " };
    let title = if era == Era::Win2000 { "Error Deleting File or Folder" } else { "Error Deleting File" };
    let text = match left {
        [one] => format!("Cannot delete file:{gap}'{}' was changed after it was copied, so Undo left it.", name_of(one)),
        many => format!("Cannot delete {} files:{gap}they were changed after they were copied, so Undo left them.", many.len()),
    };
    (title.into(), text)
}

/// A drag pointer drawn from measured pixels: `#` black, `o` white, `a` gray (808080), `.`
/// see-through, and the pixel it points with.
pub struct PointerArt {
    pub rows: &'static [&'static str],
    pub hot: (i32, i32),
}

impl PointerArt {
    /// The pixel at (x, y) as 0xAARRGGBB (0 for see-through).
    pub fn pixel(&self, x: i32, y: i32) -> u32 {
        let c = (x >= 0 && y >= 0).then(|| self.rows.get(y as usize).and_then(|r| r.as_bytes().get(x as usize))).flatten();
        match c {
            Some(b'#') => 0xFF00_0000,
            Some(b'o') => 0xFFFF_FFFF,
            Some(b'a') => 0xFF80_8080,
            _ => 0,
        }
    }

    pub fn size(&self) -> (i32, i32) {
        (self.rows.iter().map(|r| r.len() as i32).max().unwrap_or(0), self.rows.len() as i32)
    }
}

/// The pointer during a drag in `era`: the arrow for a move, the arrow with a box holding a plus
/// (copy) or the shortcut arrow (link); `None` the "no" sign. Read off the Reference PCs
/// (`private/probes/drag18/pointers`): 95's are 98's pixel for pixel; 2000's arrow is a row longer
/// and its box sits 7 rows higher.
pub fn pointer(era: Era, effect: Option<Effect>) -> &'static PointerArt {
    let old = era != Era::Win2000;
    match (effect, old) {
        (None, _) => &NO,
        (Some(Effect::Move), true) => &MOVE_98,
        (Some(Effect::Copy), true) => &COPY_98,
        (Some(Effect::Link), true) => &LINK_98,
        (Some(Effect::Move), false) => &MOVE_2000,
        (Some(Effect::Copy), false) => &COPY_2000,
        (Some(Effect::Link), false) => &LINK_2000,
    }
}

const MOVE_98: PointerArt = PointerArt { rows: crate::art::rows("drag/move", Look::Win98).rows, hot: (0, 0) };

const COPY_98: PointerArt = PointerArt { rows: crate::art::rows("drag/copy", Look::Win98).rows, hot: (0, 0) };

const LINK_98: PointerArt = PointerArt { rows: crate::art::rows("drag/link", Look::Win98).rows, hot: (0, 0) };

const MOVE_2000: PointerArt = PointerArt { rows: crate::art::rows("drag/move", Look::Win2000).rows, hot: (0, 0) };

const COPY_2000: PointerArt = PointerArt { rows: crate::art::rows("drag/copy", Look::Win2000).rows, hot: (0, 0) };

const LINK_2000: PointerArt = PointerArt { rows: crate::art::rows("drag/link", Look::Win2000).rows, hot: (0, 0) };

const NO: PointerArt = PointerArt { rows: crate::art::rows("drag/no", Look::Win98).rows, hot: (9, 9) };

#[cfg(test)]
mod tests {
    use super::*;

    /// A live test's Undo takes back only a step of test items (the third safety review of
    /// 2026-11-18, C4): a move of a person's file into a test folder, or out of one, is not one.
    #[test]
    fn only_a_step_of_test_items_is_a_tests() {
        let made = |p: &str| Made { path: p.into(), proof: Proof::default() };
        assert!(Step::Move(vec![("C:\\t\\Bevel95 test b\\Bevel95 test x.txt".into(), "C:\\t\\Bevel95 test a\\Bevel95 test x.txt".into())]).test_only());
        assert!(Step::Copy(vec![made("C:\\Users\\someone\\Desktop\\Bevel95 test x - Copy.txt")]).test_only());
        assert!(Step::Rename("C:\\t\\Bevel95 test y".into(), "C:\\t\\Bevel95 test x".into()).test_only());
        assert!(!Step::Move(vec![("C:\\t\\Bevel95 test b\\Letter.docx".into(), "C:\\Users\\someone\\Letter.docx".into())]).test_only());
        assert!(!Step::Rename("C:\\t\\Bevel95 test y".into(), "C:\\t\\Taxes".into()).test_only());
        assert!(!Step::Delete(vec!["C:\\t\\Bevel95 test x".into(), "C:\\t\\Photos".into()], 0, 0).test_only());
        assert!(!Step::Copy(Vec::new()).test_only(), "an empty step is nobody's");
    }

    /// Every wish with every offer, into a folder and onto the Recycle Bin (the safety review of
    /// 2026-11-18, A1: a download offered only as a copy was moved, and binned on the Recycle Bin).
    #[test]
    fn a_drop_does_only_what_the_source_offers() {
        use Effect::{Copy, Link, Move};
        let offer = |m: bool, c: bool, l: bool| Offered { moves: m, copy: c, link: l };
        // (wish, offered moves/copy/link, into a folder, onto the Recycle Bin)
        type Row = (Option<Effect>, (bool, bool, bool), Option<Effect>, Option<Effect>);
        #[rustfmt::skip]
        let table: &[Row] = &[
            (Some(Move), (true, true, true),    Some(Move), Some(Move)),
            (Some(Move), (false, true, true),   Some(Copy), None),
            (Some(Move), (false, true, false),  Some(Copy), None),
            (Some(Move), (true, false, false),  Some(Move), Some(Move)),
            (Some(Move), (false, false, true),  None,       None),
            (Some(Move), (false, false, false), None,       None),
            (Some(Move), (true, true, false),   Some(Move), Some(Move)),
            (Some(Move), (true, false, true),   Some(Move), Some(Move)),
            (Some(Copy), (true, true, true),    Some(Copy), None),
            (Some(Copy), (true, false, false),  Some(Move), None),
            (Some(Copy), (true, false, true),   Some(Move), None),
            (Some(Copy), (false, true, false),  Some(Copy), None),
            (Some(Copy), (false, false, true),  None,       None),
            (Some(Copy), (false, false, false), None,       None),
            (Some(Copy), (true, true, false),   Some(Copy), None),
            (Some(Copy), (false, true, true),   Some(Copy), None),
            (Some(Link), (true, true, true),    Some(Link), None),
            (Some(Link), (true, true, false),   None,       None),
            (Some(Link), (false, false, true),  Some(Link), None),
            (Some(Link), (false, true, false),  None,       None),
            (Some(Link), (true, false, false),  None,       None),
            (Some(Link), (false, false, false), None,       None),
            (Some(Link), (true, false, true),   Some(Link), None),
            (Some(Link), (false, true, true),   Some(Link), None),
        ];
        for &(wish, (m, c, l), folder, bin) in table {
            assert_eq!(narrow(wish, offer(m, c, l), false), folder, "{wish:?} offered {m}/{c}/{l} into a folder");
            assert_eq!(narrow(wish, offer(m, c, l), true), bin, "{wish:?} offered {m}/{c}/{l} onto the Recycle Bin");
        }
        // Every combination: nothing refused stays refused, and what is done is always offered.
        for bits in 0..8u8 {
            let o = offer(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
            for wish in [None, Some(Move), Some(Copy), Some(Link)] {
                assert_eq!(table.iter().filter(|r| r.0 == wish && offer(r.1.0, r.1.1, r.1.2) == o).count(), usize::from(wish.is_some()));
                for bin in [false, true] {
                    let done = narrow(wish, o, bin);
                    assert!(done.is_none_or(|e| o.has(e)), "{wish:?} {o:?}: {done:?} was not offered");
                    if wish.is_none() {
                        assert_eq!(done, None);
                    }
                }
            }
        }
        // Bevel95's own drags and Explorer's offer everything: the look's wish as before.
        for wish in [Move, Copy, Link] {
            assert_eq!(narrow(Some(wish), Offered::ALL, false), Some(wish));
        }
    }

    #[test]
    fn the_right_buttons_menu_offers_only_what_the_source_offers() {
        use Effect::{Copy, Link, Move};
        assert_eq!(menu_effects(Offered::ALL, false), [Move, Copy, Link]);
        assert_eq!(menu_effects(Offered::ALL, true), [Move]);
        let copy_only = Offered { copy: true, ..Offered::default() };
        assert_eq!(menu_effects(copy_only, false), [Copy]);
        assert!(menu_effects(copy_only, true).is_empty(), "the Recycle Bin offers nothing for a copy-only source");
        assert_eq!(menu_effects(Offered { copy: true, link: true, moves: false }, false), [Copy, Link]);
    }

    #[test]
    fn a_left_drag_moves_within_a_drive_and_copies_to_another() {
        for era in Era::ALL {
            assert_eq!(default_effect(era, true, false), Effect::Move);
            assert_eq!(default_effect(era, false, false), Effect::Copy);
        }
        assert_eq!(default_effect(Era::Win95, true, true), Effect::Link, "95 made a shortcut of a program");
        assert_eq!(default_effect(Era::Win98, false, true), Effect::Link);
        assert_eq!(default_effect(Era::Win2000, true, true), Effect::Move, "2000 moved it");
    }

    #[test]
    fn keys_choose_the_same_way_in_every_look() {
        let k = |ctrl, shift| Keys { ctrl, shift };
        for era in Era::ALL {
            assert_eq!(effect(era, k(true, false), true, false), Effect::Copy);
            assert_eq!(effect(era, k(false, true), false, false), Effect::Move);
            assert_eq!(effect(era, k(true, true), true, false), Effect::Link);
            assert_eq!(effect(era, k(false, false), true, false), Effect::Move);
            assert_eq!(effect(era, k(true, false), true, true), Effect::Copy, "a key wins over the program rule");
        }
    }

    #[test]
    fn drives_and_folders() {
        assert_eq!(drive_of("C:\\Users\\x\\a.txt"), "c:");
        assert_eq!(drive_of("c:\\"), "c:");
        assert_eq!(drive_of("\\\\Server\\Share\\a\\b"), "\\\\server\\share");
        assert!(is_program("C:\\t\\Note.EXE"));
        assert!(!is_program("C:\\t\\exe.txt"));
        let src = vec!["C:\\t\\A".to_string()];
        assert!(into_itself(&src, "c:\\t\\a"));
        assert!(into_itself(&src, "C:\\t\\A\\Sub"));
        assert!(!into_itself(&src, "C:\\t\\AB"));
        assert!(all_in(&["C:\\t\\a.txt".into(), "C:\\t\\B".into()], "c:\\T\\"));
        assert!(!all_in(&["C:\\t\\a.txt".into(), "C:\\u\\b.txt".into()], "C:\\t"));
        assert!(all_in(&["C:\\a.txt".into()], "C:\\"));
    }

    #[test]
    fn the_right_drag_menu_in_each_look() {
        let all = [Effect::Move, Effect::Copy, Effect::Link];
        let labels = |m: &Menu| m.entries.iter().map(|e| e.label.clone()).collect::<Vec<_>>();
        let (m, does) = drop_menu(Era::Win98, Effect::Move, &all);
        assert_eq!(labels(&m), ["&Move Here", "&Copy Here", "Create &Shortcut(s) Here", "", "Cancel"]);
        assert!(m.entries[0].bold && !m.entries[1].bold);
        assert_eq!(does, [Some(Effect::Move), Some(Effect::Copy), Some(Effect::Link), None, None]);
        let (m, does) = drop_menu(Era::Win2000, Effect::Copy, &all);
        assert_eq!(labels(&m)[..2], ["&Copy Here", "&Move Here"]);
        assert!(m.entries[0].bold);
        assert_eq!(does[1], Some(Effect::Move));
        let (m, _) = drop_menu(Era::Win95, Effect::Move, &[Effect::Move]);
        assert_eq!(labels(&m), ["&Move Here", "", "Cancel"]);
    }

    #[test]
    fn undo_words_and_the_list() {
        let mv = Step::Move(vec![("C:\\B\\a3.txt".into(), "C:\\A\\a3.txt".into())]);
        assert_eq!(mv.label(Era::Win98), "&Undo Move");
        assert_eq!(mv.label(Era::Win2000), "&Undo Move\tCtrl+Z");
        assert_eq!(mv.help(), "Undo Move of 'a3.txt'");
        let rn = Step::Rename("C:\\A\\r2".into(), "C:\\A\\a2".into());
        assert_eq!(rn.help(), "Undo Rename of 'a2' to 'r2'");
        let made = |p: &str| Made { path: p.into(), proof: Proof { folder: p.ends_with('b'), items: 3, bytes: 40, newest: 7 } };
        let cp = Step::Copy(vec![made("D:\\a"), made("D:\\b")]);
        assert_eq!(cp.help(), "Undo Copy of 2 items");
        let del = Step::Delete(vec!["C:\\A\\x.txt".into()], 1_800_000_000, 1_800_000_003);
        let mut list = UndoList::default();
        for s in [mv.clone(), rn.clone(), cp.clone(), del.clone()] {
            list.push(s);
        }
        assert_eq!(UndoList::parse(&list.to_text()), list, "undo.txt reads back");
        assert_eq!(list.pop(), Some(del));
        assert_eq!(list.last(), Some(&cp));
        for _ in 0..20 {
            list.push(mv.clone());
        }
        assert_eq!(list.steps.len(), DEPTH);
        assert_eq!(UndoList::parse("bevel95 undo 2\nrubbish\nmove\tonly-one\n").steps, Vec::new());
        // The older format (no header, copies without proof) is dropped without an error.
        assert_eq!(UndoList::parse("copy\tC:\\t\\a - Copy.txt\nmove\tC:\\B\\a\tC:\\A\\a\n").steps, Vec::new());
    }

    /// A folder `C:\t` holding `a.txt` and the folder `F`; `D:\d` holding `F` (another `F`).
    fn on_disk(p: &str) -> bool {
        ["c:\\t\\a.txt", "c:\\t\\f", "d:\\d\\f", "d:\\d\\b.txt"].contains(&p.to_lowercase().as_str())
    }

    #[test]
    fn a_copy_into_its_own_folder_gets_a_name_chosen_before() {
        assert_eq!(copy_name("a.txt", false, |_| false), "a - Copy.txt");
        assert_eq!(copy_name("a.txt", false, |n| n == "a - Copy.txt"), "a - Copy (2).txt");
        assert_eq!(copy_name("arc.tar.gz", false, |_| false), "arc.tar - Copy.gz");
        assert_eq!(copy_name("v1.2", true, |_| false), "v1.2 - Copy", "a folder's dot is no extension");
        assert_eq!(copy_name("noext", false, |_| false), "noext - Copy");
        let own = |p: &str| p.to_lowercase().starts_with("c:\\t\\");
        let srcs = vec!["C:\\t\\a.txt".to_string(), "C:\\t\\a.txt".to_string()];
        let p = plan(&srcs, "C:\\t", true, own, |_| false, on_disk);
        assert_eq!(p[0].to, "C:\\t\\a - Copy.txt");
        assert_eq!(p[1].to, "C:\\t\\a - Copy (2).txt", "two copies in one operation get two names");
        assert!(p.iter().all(|i| i.free));
    }

    #[test]
    fn a_file_that_appears_during_a_copy_is_not_in_the_step() {
        let srcs = vec!["C:\\t\\a.txt".to_string()];
        let p = plan(&srcs, "D:\\d", true, |_| false, |_| false, on_disk);
        assert_eq!(p, [Planned { from: "C:\\t\\a.txt".into(), to: "D:\\d\\a.txt".into(), free: true }]);
        let file = Proof { folder: false, items: 1, bytes: 5, newest: 9 };
        // After the copy: a.txt is there, and so is b.txt (a download meanwhile).
        let after = |q: &str| ["c:\\t\\a.txt", "d:\\d\\a.txt", "d:\\d\\b.txt"].contains(&q.to_lowercase().as_str()).then_some(file);
        let made = copied(&p, after);
        assert_eq!(made, [Made { path: "D:\\d\\a.txt".into(), proof: file }], "only the planned copy, never b.txt");
        // A file that took the planned free name meanwhile is not like the source: not recorded.
        let other = |q: &str| match q.to_lowercase().as_str() {
            "c:\\t\\a.txt" => Some(file),
            "d:\\d\\a.txt" => Some(Proof { bytes: 77, ..file }),
            _ => None,
        };
        assert!(copied(&p, other).is_empty());
    }

    #[test]
    fn a_merged_folder_and_a_replaced_file_are_not_in_the_step() {
        // C:\t\F onto D:\d, which holds an F: merged. a.txt onto a D:\d that holds a.txt: replaced.
        let srcs = vec!["C:\\t\\F".to_string()];
        let p = plan(&srcs, "D:\\d", false, |_| false, |_| true, on_disk);
        assert!(!p[0].free);
        // After the move: D:\d\F is there, C:\t\F is gone.
        let after = |q: &str| q.eq_ignore_ascii_case("D:\\d\\F");
        assert!(moved(&p, after).is_empty(), "a merged folder is not carried back");
        let srcs = vec!["C:\\t\\b.txt".to_string()];
        let p = plan(&srcs, "D:\\d", true, |_| false, |_| false, on_disk);
        let file = Proof { folder: false, items: 1, bytes: 5, newest: 9 };
        assert!(copied(&p, |_| Some(file)).is_empty(), "a replaced file is not recorded");
        // A free one is: moved there, gone from where it was.
        let srcs = vec!["C:\\t\\a.txt".to_string()];
        let p = plan(&srcs, "D:\\d", false, |_| false, |_| false, on_disk);
        assert_eq!(moved(&p, |q| q.eq_ignore_ascii_case("D:\\d\\a.txt")), [("D:\\d\\a.txt".into(), "C:\\t\\a.txt".into())]);
        // Two items of one name from two folders: the second would merge into the first.
        let srcs = vec!["C:\\t\\x".to_string(), "C:\\u\\x".to_string()];
        let p = plan(&srcs, "D:\\d", false, |_| false, |_| true, |_| false);
        assert!(p[0].free && !p[1].free);
    }

    #[test]
    fn a_step_of_which_nothing_is_recorded_empties_the_list() {
        let mut list = UndoList::default();
        list.record(Some(Step::Rename("C:\\b".into(), "C:\\a".into())));
        assert_eq!(list.steps.len(), 1);
        list.record(None);
        assert_eq!(list.last(), None, "Undo does not reach past an operation it cannot take back");
    }

    #[test]
    fn an_operation_that_provably_did_nothing_leaves_the_list_alone() {
        let older = Step::Rename("C:\\t\\b".into(), "C:\\t\\a".into());
        let list = || UndoList { steps: vec![older.clone()] };
        let srcs = vec!["C:\\t\\a.txt".to_string(), "C:\\t\\F".to_string()];
        let planned = plan(&srcs, "D:\\e", false, |_| false, |_| false, on_disk);
        assert!(planned.iter().all(|p| p.free));
        let here = |p: &str| on_disk(p);
        let free = |p: &str| !on_disk(p);
        // Cancelled before anything happened: the places still free, the sources still there.
        let r = recorded(&planned, None, free, here);
        assert_eq!(r, Recorded::Nothing);
        let mut l = list();
        l.leave(r);
        assert_eq!(l, list(), "the older step stays");
        // One place taken now (something was made there, or appeared): emptied.
        let r = recorded(&planned, None, |p: &str| !p.eq_ignore_ascii_case("D:\\e\\F"), here);
        assert_eq!(r, Recorded::Unproven);
        let mut l = list();
        l.leave(r);
        assert!(l.steps.is_empty());
        // A source gone (something went): emptied.
        assert_eq!(recorded(&planned, None, free, |p: &str| !p.eq_ignore_ascii_case("C:\\t\\a.txt")), Recorded::Unproven);
        // A place that was taken before (a merge or a replacement Windows asked about): emptied,
        // even if it looks untouched.
        let taken = plan(&["C:\\t\\a.txt".to_string()], "C:\\t", false, |_| false, |_| false, on_disk);
        assert!(!taken[0].free);
        assert_eq!(recorded(&taken, None, |_| true, |_| true), Recorded::Unproven);
        // Nothing planned at all: in doubt, emptied.
        assert_eq!(recorded(&[], None, |_| true, |_| true), Recorded::Unproven);
        // A step: recorded as before.
        let mv = Step::Move(vec![("D:\\e\\a.txt".into(), "C:\\t\\a.txt".into())]);
        let mut l = list();
        l.leave(recorded(&planned, Some(mv.clone()), free, here));
        assert_eq!(l.steps, [older.clone(), mv]);
    }

    #[test]
    fn a_changed_copy_is_left_by_undo_copy() {
        let proof = Proof { folder: true, items: 4, bytes: 100, newest: 50 };
        let made = vec![
            Made { path: "D:\\d\\F".into(), proof },
            Made { path: "D:\\d\\G".into(), proof },
            Made { path: "D:\\d\\H".into(), proof },
        ];
        let now = |p: &str| match p {
            "D:\\d\\F" => Some(proof),
            "D:\\d\\G" => Some(Proof { items: 5, newest: 60, ..proof }),
            _ => None,
        };
        let (take, left) = undo_copy(&made, now);
        assert_eq!(take, [made[0].clone()]);
        assert_eq!(left, ["D:\\d\\G"]);
        let (title, text) = copies_left(Era::Win98, &left);
        assert_eq!(title, "Error Deleting File");
        assert_eq!(text, "Cannot delete file:  'G' was changed after it was copied, so Undo left it.");
    }

    #[test]
    fn an_undo_that_cannot_be_done_says_so() {
        let rn = Step::Rename("a".into(), "b".into());
        assert_eq!(undo_failed(Era::Win98, &rn), ("Error Renaming File".into(), "Cannot rename file:  File system error (1026).".into()));
        assert_eq!(undo_failed(Era::Win2000, &rn).0, "Error Renaming File or Folder");
    }

    #[test]
    fn pointers_have_their_marks_where_measured() {
        // The box's top left corner: (+11, +18) in 95 and 98, (+11, +11) in 2000.
        assert_eq!(pointer(Era::Win98, Some(Effect::Copy)).pixel(11, 18), 0xFF80_8080);
        assert_eq!(pointer(Era::Win2000, Some(Effect::Copy)).pixel(11, 11), 0xFF80_8080);
        assert_eq!(pointer(Era::Win95, Some(Effect::Link)).pixel(16, 21), 0xFF00_0000);
        assert_eq!(pointer(Era::Win98, Some(Effect::Move)).pixel(0, 0), 0xFF00_0000);
        assert_eq!(pointer(Era::Win98, Some(Effect::Move)).pixel(13, 0), 0);
        assert_eq!(pointer(Era::Win2000, None).size(), (20, 20));
    }
}

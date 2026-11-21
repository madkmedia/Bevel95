//! The web view panel of Windows 98's and 2000's folder windows: the white strip at the left of a
//! folder's contents with a picture in its corner, the folder's 32×32 picture, its name in large
//! letters, a colored line, and what is selected (a file's type, date and size; several items'
//! count, total size and names; a drive's capacity with a pie chart). In the real systems it was a
//! web page Internet Explorer drew from templates (98: `Windows\Web\folder.htt`, `mycomp.htt`;
//! 2000: `webvw.dll`'s own panel); Bevel95 draws it itself from what was measured on the Reference
//! PCs (`private/reference/refs/vm/<era>-truecolor-web-*`, the notes `private/probes/web/<era>-notes.txt`,
//! docs/theme-spec.md "The web view panel"):
//!
//! - 98: the panel is 30% of the contents' width (rounded); below 400 pixels it goes and a 32-row
//!   band with the title shows above the icons instead. Text in Verdana 8pt, the title Verdana
//!   16pt bold, a four-color line under it, lines 13 apart with blank lines between the parts.
//! - 2000: the panel is 200 wide; below 400 it goes, with nothing in its place. Text in the menu
//!   font (Tahoma 8pt), the title Tahoma 13pt bold, a blue line two rows high, parts 12 rows apart,
//!   "See also:" links, a yellow note for My Computer and My Documents.
//! - Both keep 16 columns at the panel's right for its scroll bar, which shows when the text is
//!   taller than the panel. The system folders (Windows, System, Program Files) show a warning page
//!   instead of their files until "Show Files" is clicked (`Guard`).
//!
//! Left out: the preview of a selected picture (it needed Internet Explorer's thumbnail control),
//! the panel's horizontal scroll bar (wider content is cut at the panel's edge), the hand pointer
//! and tooltip over a link.

use std::sync::{Arc, LazyLock};

use crate::art::{self, Look};
use crate::desktop_art::Art;
use crate::desktop_icons::{Item, Picture, date_text, kb_text};
use crate::fb::{Framebuffer, Icon, Rect};
use crate::folder::art::DriveKind;
use crate::folder::{Space, status};
use crate::text::Font;
use crate::theme::{Era, Theme};

const WHITE: u32 = 0xFFFF_FFFF;
const BLACK: u32 = 0xFF00_0000;
const LINK: u32 = 0xFF00_00FF;
const WARNING: u32 = 0xFFFF_0000;
/// Rows from one text line to the next (8pt text, 10pt or the menu font's line height).
const PITCH: i32 = 13;
/// Columns kept free at the panel's right for its scroll bar (Internet Explorer's).
pub const SCROLL_W: i32 = 16;
/// 98's band above the icons while the panel does not fit.
pub const BANNER_H: i32 = 32;

/// The panel's fonts: the text, its bold, the title.
#[derive(Clone, Debug)]
pub struct WebFonts {
    pub body: Font,
    pub bold: Font,
    pub title: Font,
}

/// What kind of place the window shows (what the panel says with nothing selected).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Folder,
    /// A drive's root (2000: its capacity and pie chart).
    Drive,
    MyComputer,
    MyDocuments,
}

/// The system folders whose web view warned before showing their files (98: `folder.htt` in
/// Windows, System, System32 and Program Files; 2000: WINNT, system32, Program Files).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guard {
    /// The Windows folder and its system folders: the files hide behind "Show Files".
    System,
    /// Program Files: 98 shows its files with a warning above the prompt; 2000 hides them too.
    ProgramFiles,
}

/// A link in the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    MyDocuments,
    NetworkPlaces,
    MyComputer,
    NetworkConnections,
    ShowFiles,
    AddRemove,
}

impl Link {
    pub fn label(self) -> &'static str {
        match self {
            Link::MyDocuments => "My Documents",
            Link::NetworkPlaces => "My Network Places",
            Link::MyComputer => "My Computer",
            Link::NetworkConnections => "Network and Dial-up Connections",
            Link::ShowFiles => "Show Files",
            Link::AddRemove => "Add/Remove Programs",
        }
    }

    /// The status bar's text while the pointer is on the link (98's ":Show Files", the link's own
    /// address; 2000's "See also" links their descriptions, read from its webvw.dll; none for 2000's
    /// Show Files).
    pub fn status(self, era: Era) -> Option<&'static str> {
        match (era, self) {
            (Era::Win98, Link::ShowFiles) => Some(":Show Files"),
            (_, Link::MyDocuments) => Some("My Documents contains your personal documents."),
            (_, Link::MyComputer) => Some("My Computer contains your various local drive and mapped network drives."),
            (_, Link::NetworkPlaces) => Some("My Network Places contains shortcuts to various locations on the corporate network and the Internet."),
            (_, Link::NetworkConnections) => Some("Connects to other computers, networks and the Internet"),
            _ => None,
        }
    }
}

/// The web view's state in a window.
#[derive(Clone, Debug, Default)]
pub struct Web {
    /// Switched on for this window (98: View > as Web Page for the folder; 2000: Folder Options).
    pub on: bool,
    pub kind: Kind,
    /// The place's 32×32 picture.
    pub art: Option<Icon>,
    pub fonts: Option<Arc<WebFonts>>,
    pub guard: Option<Guard>,
    /// "Show Files" was clicked (98 forgets it when the window leaves the folder; 2000 remembers).
    pub files_shown: bool,
    /// A drive's root: its free space and size (2000's pie with nothing selected).
    pub space: Option<Space>,
    /// The panel scrolled down by this many rows.
    pub scroll: i32,
    /// File attributes by item id (2000's "Attributes:" line).
    pub attributes: std::collections::HashMap<String, u32>,
}

impl Web {
    /// The files are hidden behind the warning page.
    pub fn barricaded(&self, era: Era) -> bool {
        self.on
            && !self.files_shown
            && match self.guard {
                Some(Guard::System) => true,
                Some(Guard::ProgramFiles) => era == Era::Win2000,
                None => false,
            }
    }
}

/// The widest the 98 panel grows: 30% of a 1600-wide window, the widest common screen of
/// Windows 98's time. The limit is Bevel95's own (GitHub issue 2: on a 2560-wide window 98's
/// 30% gave 768 pixels); every window up to 1600 wide is as Windows 98 made it.
pub const PANEL_98_MAX: i32 = 480;

/// The panel's width in a contents area `inside_w` wide, or `None` where it does not fit (98:
/// 30%, rounded, from 400 (450 on the warning page), never more than [`PANEL_98_MAX`]; 2000:
/// 200 from 400).
pub fn panel_width(era: Era, inside_w: i32, barricade: bool) -> Option<i32> {
    match era {
        Era::Win98 => {
            let threshold = if barricade { 450 } else { 400 };
            (inside_w >= threshold).then(|| ((inside_w * 3 + 5) / 10).min(PANEL_98_MAX))
        }
        Era::Win2000 => (inside_w >= 400).then_some(200),
        Era::Win95 => None,
    }
}

/// What the panel knows of a selected item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    /// As the list names it.
    pub name: String,
    /// The file's own name with its extension (98 names a single file so).
    pub file_name: String,
    pub type_name: String,
    pub size: u64,
    pub modified: u64,
    pub what: What,
    pub attributes: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    File,
    Folder,
    Drive(DriveKind, Option<Space>),
    ControlPanel,
    Printers,
    /// One of the look's own desktop icons (a window showing the Desktop).
    Own,
}

impl Info {
    pub fn of(item: &Item, space: Option<Space>, attributes: Option<u32>) -> Info {
        let what = match item.picture {
            Picture::Art(Art::Drive(kind)) => What::Drive(kind, space),
            Picture::Art(Art::ControlPanel) => What::ControlPanel,
            Picture::Art(Art::Printers) => What::Printers,
            Picture::Art(_) => What::Own,
            _ if item.folder => What::Folder,
            _ => What::File,
        };
        let file_name = item.id.rsplit(['\\', '/']).next().filter(|n| !n.is_empty()).unwrap_or(&item.name).to_string();
        Info { name: item.name.clone(), file_name, type_name: item.type_name.clone(), size: item.size, modified: item.modified, what, attributes }
    }
}

/// How a piece of text is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Body,
    Bold,
    Warning,
    Link(Link),
}

/// The panel's contents, top to bottom, before they are laid out.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Flow {
    Text(String, Style),
    /// One line that is not wrapped (2000's names of several items: cut at the edge).
    NoWrap(String),
    /// A line break (two in a row leave an empty line).
    Br,
    /// A paragraph: the line ends, then this many rows more.
    P(i32),
    /// The legend's box (used or free) at the line's start.
    Legend(bool),
    Pie(Space),
    /// 2000's note in a yellow box.
    Note(String),
    /// 2000's links, one per line.
    Links(Vec<Link>),
}

/// The types My Computer gave a drive.
fn drive_type(era: Era, kind: DriveKind) -> &'static str {
    match (era, kind) {
        (Era::Win2000, DriveKind::Floppy | DriveKind::Floppy525) => "3\u{bd}-Inch Floppy Disk",
        (_, DriveKind::Floppy | DriveKind::Floppy525) => "3\u{bd} Inch Floppy Disk",
        (Era::Win2000, DriveKind::CdRom) => "Compact Disc",
        (_, DriveKind::CdRom) => "CD-ROM Disc",
        (_, DriveKind::Removable) => "Removable Disk",
        (_, DriveKind::Network) => "Network Drive",
        (_, DriveKind::RamDisk) => "RAM Disk",
        (_, DriveKind::Fixed) => "Local Disk",
    }
}

/// A capacity as the panel wrote it: GB with two decimals, MB whole, a space before the unit.
fn capacity_text(bytes: u64) -> String {
    status::size_in(Era::Win2000, bytes)
}

/// "1,234,567" (the templates' FormatNumber).
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

const PROMPT: &str = "Select an item to view its description.";

/// The descriptions 98's My Computer showed under Control Panel and Printers (their InfoTips, read
/// from the Reference PC's registry).
const TIP_CONTROL_PANEL_98: &str = "Use the settings in Control Panel to personalize your computer. For example, you can specify how you want your desktop to look (Display icon), which events you want to hear sounds for (Sounds icon), the volume you prefer for audio recording (Multimedia icon), and much more.";
const TIP_PRINTERS_98: &str = "Use the Printers folder to add a new local or network printer, or to change settings for existing printers.";

fn text(s: impl Into<String>) -> Flow {
    Flow::Text(s.into(), Style::Body)
}

fn bold(s: impl Into<String>) -> Flow {
    Flow::Text(s.into(), Style::Bold)
}

/// What the panel says, below the line, for the window's place and selection (`selected`: the
/// focused item first, then the others in the list's order).
#[allow(clippy::too_many_arguments)]
fn flow(era: Era, kind: Kind, guard: Option<Guard>, files_shown: bool, space: Option<Space>, selected: &[Info], count: usize, in_my_computer: bool) -> Vec<Flow> {
    match era {
        Era::Win2000 => flow_2000(kind, guard, files_shown, space, selected, count, in_my_computer),
        _ => flow_98(guard, files_shown, selected, count, in_my_computer),
    }
}

fn flow_98(guard: Option<Guard>, files_shown: bool, selected: &[Info], count: usize, in_my_computer: bool) -> Vec<Flow> {
    let mut out = Vec::new();
    let warning = |out: &mut Vec<Flow>| {
        out.push(Flow::Text("Warning".into(), Style::Warning));
        out.push(Flow::Br);
        out.push(Flow::Br);
        out.push(text("Modifying the contents of this folder may cause your programs to stop working correctly."));
    };
    match (count, selected) {
        (0, _) | (_, []) => match guard {
            Some(Guard::System) => {
                warning(&mut out);
                out.push(Flow::Br);
                out.push(Flow::Br);
                if files_shown {
                    out.push(text(PROMPT));
                } else {
                    out.push(text("To view the contents of this folder, click "));
                    out.push(Flow::Text("Show Files".into(), Style::Link(Link::ShowFiles)));
                    out.push(text("."));
                }
            }
            Some(Guard::ProgramFiles) => {
                warning(&mut out);
                out.push(Flow::Br);
                out.push(Flow::Br);
                out.push(bold("To add or remove programs, click Start, point to Settings, click Control Panel, and then click Add/Remove Programs."));
                out.push(Flow::Br);
                out.push(Flow::Br);
                out.push(text(PROMPT));
            }
            None => out.push(text(PROMPT)),
        },
        (1, [one, ..]) => {
            let name = match one.what {
                What::File => one.file_name.clone(),
                _ => one.name.clone(),
            };
            out.push(bold(name));
            if in_my_computer {
                // mycomp.htt: the type, the description, a drive's capacity and pie.
                out.push(Flow::Br);
                out.push(text(match one.what {
                    What::Drive(kind, _) => drive_type(Era::Win98, kind).to_string(),
                    What::ControlPanel | What::Printers => "System Folder".into(),
                    _ => one.type_name.clone(),
                }));
                let tip = match one.what {
                    What::ControlPanel => Some(TIP_CONTROL_PANEL_98),
                    What::Printers => Some(TIP_PRINTERS_98),
                    _ => None,
                };
                if let Some(tip) = tip {
                    out.extend([Flow::Br, Flow::Br, text(tip)]);
                }
                if let What::Drive(_, Some(space)) = one.what {
                    let used = space.capacity.saturating_sub(space.free);
                    out.extend([Flow::Br, Flow::Br, Flow::Br, text(format!("Capacity: {}", capacity_text(space.capacity)))]);
                    out.extend([Flow::Br, Flow::Br, Flow::Legend(true), text(format!("Used: {}", capacity_text(used)))]);
                    out.extend([Flow::Br, Flow::Br, Flow::Legend(false), text(format!("Free: {}", capacity_text(space.free)))]);
                    out.push(Flow::Pie(space));
                }
                return out;
            }
            if !one.type_name.is_empty() {
                out.push(Flow::Br);
                out.push(text(one.type_name.clone()));
            }
            let date = date_text(Era::Win98, one.modified);
            if !date.is_empty() {
                out.extend([Flow::Br, Flow::Br, text("Modified:"), Flow::Br, text(date)]);
            }
            if one.what == What::File {
                let size = if one.size > 0 && one.size < 1000 { format!("Size: {}\u{a0}bytes", one.size) } else { format!("Size: {}", kb_text(Era::Win98, one.size)) };
                out.extend([Flow::Br, Flow::Br, text(size)]);
            }
        }
        (_, many) => {
            out.push(text(format!("{count} items selected.")));
            out.push(Flow::Br);
            if count <= 100 {
                let size: u64 = many.iter().map(|i| i.size).sum();
                if size > 0 && !in_my_computer {
                    out.extend([Flow::Br, text(format!("Total File Size: {}\u{a0}bytes", grouped(size))), Flow::Br]);
                }
                if count <= 16 {
                    for i in many {
                        out.extend([Flow::Br, text(i.name.clone())]);
                    }
                }
            }
        }
    }
    out
}

fn flow_2000(kind: Kind, guard: Option<Guard>, files_shown: bool, space: Option<Space>, selected: &[Info], count: usize, in_my_computer: bool) -> Vec<Flow> {
    let mut out = Vec::new();
    let intro = |out: &mut Vec<Flow>| {
        if guard.is_some() {
            out.push(text("This folder contains files that keep your system working properly. There is no need to modify its contents."));
            if guard == Some(Guard::ProgramFiles) {
                out.push(Flow::P(12));
                out.push(text("To manage your programs, open the "));
                out.push(Flow::Text(Link::AddRemove.label().into(), Style::Link(Link::AddRemove)));
                out.push(text(" applet from the Control Panel."));
                out.push(Flow::P(12));
            } else {
                out.push(Flow::Br);
                out.push(Flow::Br);
            }
        }
    };
    let drive = |out: &mut Vec<Flow>, space: Space| {
        let used = space.capacity.saturating_sub(space.free);
        out.extend([Flow::P(12), Flow::Br, text(format!("Capacity: {}", capacity_text(space.capacity)))]);
        out.extend([Flow::P(12), Flow::Legend(true), text(format!("Used: {}", capacity_text(used)))]);
        out.extend([Flow::P(12), Flow::Legend(false), text(format!("Free: {}", capacity_text(space.free)))]);
        out.push(Flow::Pie(space));
    };
    match (count, selected) {
        (0, _) | (_, []) => {
            intro(&mut out);
            if guard.is_some() && !files_shown {
                out.push(text("To view the contents of this folder, click: "));
                out.push(Flow::Text("Show Files".into(), Style::Link(Link::ShowFiles)));
                return out;
            }
            out.push(text(PROMPT));
            match kind {
                Kind::Drive => {
                    if let Some(space) = space {
                        drive(&mut out, space);
                    }
                    out.extend([Flow::P(12), text("See also:"), Flow::Links(vec![Link::MyDocuments, Link::NetworkPlaces, Link::MyComputer])]);
                }
                Kind::MyComputer => {
                    out.extend([Flow::P(12), Flow::Note("Displays the files and folders on your computer".into()), Flow::P(13), text("See also:")]);
                    out.push(Flow::Links(vec![Link::MyDocuments, Link::NetworkPlaces, Link::NetworkConnections]));
                }
                Kind::MyDocuments => {
                    out.extend([Flow::P(12), Flow::Note("Stores and manages documents".into()), Flow::P(13), text("See also:")]);
                    out.push(Flow::Links(vec![Link::NetworkPlaces, Link::MyComputer]));
                }
                Kind::Folder if guard.is_none() => {
                    out.extend([Flow::P(12), text("See also:"), Flow::Links(vec![Link::MyDocuments, Link::NetworkPlaces, Link::MyComputer])]);
                }
                Kind::Folder => {}
            }
        }
        (1, [one, ..]) => {
            out.push(bold(one.name.clone()));
            out.push(Flow::Br);
            let type_name = match one.what {
                What::Drive(kind, _) => drive_type(Era::Win2000, kind).to_string(),
                What::ControlPanel | What::Printers => "System Folder".into(),
                _ => one.type_name.clone(),
            };
            out.push(text(type_name));
            match one.what {
                What::Drive(_, Some(space)) => drive(&mut out, space),
                What::Drive(..) | What::Own => {}
                What::ControlPanel => out.extend([Flow::P(12), Flow::Note("Customizes the appearance of your desktop and configures your computer".into())]),
                What::Printers => {}
                What::File | What::Folder => {
                    let date = date_text(Era::Win2000, one.modified);
                    if !date.is_empty() {
                        out.extend([Flow::P(12), text(format!("Modified: {date}"))]);
                    }
                    if one.what == What::File && one.size > 0 {
                        out.extend([Flow::P(12), text(format!("Size: {}", status::size_in(Era::Win2000, one.size)))]);
                    }
                    if !in_my_computer {
                        out.extend([Flow::P(12), text(format!("Attributes: {}", attribute_words(one.attributes.unwrap_or(0))))]);
                    }
                }
            }
        }
        (_, many) => {
            out.push(text(format!("{count} items selected.")));
            if count <= 100 {
                let size: u64 = many.iter().map(|i| i.size).sum();
                if size > 0 {
                    out.extend([Flow::P(12), text(format!("Total File Size: {}", status::size_in(Era::Win2000, size)))]);
                }
                if count <= 16 {
                    out.extend([Flow::P(12), Flow::Br]);
                    for (k, i) in many.iter().enumerate() {
                        if k > 0 {
                            out.push(Flow::Br);
                        }
                        out.push(Flow::NoWrap(i.name.clone()));
                    }
                }
            }
        }
    }
    out
}

/// 2000's "Attributes:" words for Windows' attribute bits (read-only, hidden, system, compressed,
/// encrypted; the archive bit is left out, as the template did).
pub fn attribute_words(bits: u32) -> String {
    let names = [(0x1, "Read-only"), (0x2, "Hidden"), (0x4, "System"), (0x800, "Compressed"), (0x4000, "Encrypted")];
    let words: Vec<&str> = names.iter().filter(|(b, _)| bits & b != 0).map(|(_, n)| *n).collect();
    if words.is_empty() { "(normal)".into() } else { words.join(", ") }
}

/// One thing the panel draws (panel pixels, before scrolling).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Drawn {
    Text { x: i32, y: i32, text: String, style: Style },
    Legend { x: i32, y: i32, used: bool },
    Pie { x: i32, y: i32, space: Space },
    Note { rect: Rect, lines: Vec<String> },
}

/// Each look's measures (panel pixels).
struct Metrics {
    /// The text's left edge and the room it wraps in, past the panel's scroll bar column.
    left: i32,
    right_margin: i32,
    /// The picture's cell, the title's first cell top and line pitch.
    icon: (i32, i32),
    title_top: i32,
    title_pitch: i32,
    /// The line below the title's last line (from its cell top), its height; the text below it.
    line_below: i32,
    line_h: i32,
    text_below: i32,
}

fn metrics(era: Era) -> Metrics {
    match era {
        Era::Win2000 => Metrics { left: 12, right_margin: 0, icon: (12, 11), title_top: 49, title_pitch: 17, line_below: 23, line_h: 2, text_below: 17 },
        _ => Metrics { left: 15, right_margin: 15, icon: (15, 15), title_top: 52, title_pitch: 25, line_below: 30, line_h: 1, text_below: 23 },
    }
}

/// The font a piece of text is drawn in.
fn font_of(era: Era, fonts: &WebFonts, style: Style) -> &Font {
    match style {
        Style::Bold | Style::Warning => &fonts.bold,
        Style::Link(_) if era == Era::Win98 => &fonts.bold,
        _ => &fonts.body,
    }
}

/// Words of `text` with the spaces before them: (spaces, word).
fn words(text: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut space = false;
    let mut word = String::new();
    for c in text.chars() {
        if c == ' ' {
            if !word.is_empty() {
                out.push((space, std::mem::take(&mut word)));
            }
            space = true;
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() || space {
        out.push((space, word));
    }
    out
}

/// Lays `text` out in lines no wider than `width` (words wrap; a word wider than the room keeps
/// its own line), as Internet Explorer did.
pub fn wrap(font: &Font, text: &str, width: i32) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut w = 0;
    for (space, word) in words(text) {
        let line = lines.last_mut().expect("one line");
        let sp = if space && !line.is_empty() { font.measure(" ") } else { 0 };
        let ww = font.measure(&word);
        if !line.is_empty() && w + sp + ww > width {
            lines.push(word.clone());
            w = ww;
        } else {
            if sp > 0 {
                line.push(' ');
            }
            line.push_str(&word);
            w += sp + ww;
        }
    }
    lines
}

/// The panel laid out for a panel `width` wide: what is drawn and how tall it all is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Laid {
    pub title: Vec<String>,
    pub title_top: i32,
    pub line_y: i32,
    pub drawn: Vec<Drawn>,
    pub height: i32,
}

/// Lays the panel out.
#[allow(clippy::too_many_arguments)]
pub fn lay_out(era: Era, fonts: &WebFonts, width: i32, title: &str, web: &Web, selected: &[Info], count: usize, in_my_computer: bool) -> Laid {
    let m = metrics(era);
    let room = width - SCROLL_W;
    let text_right = room - m.right_margin;
    let title_lines = wrap(&fonts.title, title.trim(), text_right - m.left);
    let line_y = m.title_top + m.title_pitch * (title_lines.len() as i32 - 1) + m.line_below;
    let mut y = line_y + m.text_below;
    let mut drawn = Vec::new();
    let mut x = m.left;
    let mut content = false;
    let mut bottom = y;
    let left = m.left;
    let flows = flow(era, web.kind, web.guard, web.files_shown, web.space, selected, count, in_my_computer);
    let mut last_top = y;
    for f in flows {
        match f {
            Flow::Br => {
                y += PITCH;
                x = left;
                content = false;
            }
            Flow::P(gap) => {
                if content {
                    y += PITCH;
                }
                y += gap;
                x = left;
                content = false;
            }
            Flow::Legend(used) => {
                let (bx, tx) = if era == Era::Win2000 { (left + 16, left + 34) } else { (left, left + 19) };
                drawn.push(Drawn::Legend { x: bx, y, used });
                x = tx;
                content = true;
                last_top = y;
                bottom = bottom.max(y + PITCH);
            }
            Flow::Text(s, style) => {
                let font = font_of(era, fonts, style);
                for (space, word) in words(&s) {
                    let sp = if space && content { font.measure(" ") } else { 0 };
                    let ww = font.measure(&word);
                    if content && x + sp + ww > text_right {
                        y += PITCH;
                        x = left;
                        content = false;
                    }
                    let gap = if content { sp } else { 0 };
                    if !word.is_empty() {
                        // A link's underline runs under the spaces between its words too.
                        let linked = matches!(style, Style::Link(_)) && gap > 0 && drawn.last().is_some_and(|d| matches!(d, Drawn::Text { style: s, y: dy, .. } if *s == style && *dy == y));
                        let (dx, shown) = if linked { (x, format!(" {word}")) } else { (x + gap, word.clone()) };
                        drawn.push(Drawn::Text { x: dx, y, text: shown.replace('\u{a0}', " "), style });
                        x += gap + ww;
                        content = true;
                        last_top = y;
                        bottom = bottom.max(y + PITCH);
                    } else if content {
                        x += sp;
                    }
                }
            }
            Flow::NoWrap(s) => {
                drawn.push(Drawn::Text { x, y, text: s, style: Style::Body });
                content = true;
                last_top = y;
                bottom = bottom.max(y + PITCH);
            }
            Flow::Note(s) => {
                let lines = wrap(&fonts.body, &s, text_right - left - 2);
                let h = 2 + PITCH * lines.len() as i32;
                drawn.push(Drawn::Note { rect: Rect::new(left, y, text_right - left, h), lines });
                y += h;
                content = false;
                bottom = bottom.max(y);
            }
            Flow::Links(links) => {
                for l in links {
                    y += 17;
                    drawn.push(Drawn::Text { x: left, y, text: l.label().into(), style: Style::Link(l) });
                    last_top = y;
                    bottom = bottom.max(y + PITCH);
                }
                content = true;
            }
            Flow::Pie(space) => {
                let (px, top) = if era == Era::Win2000 { (left, last_top + 26) } else { (left, last_top + PITCH + 15 + 10) };
                let (box_h, offset) = if era == Era::Win2000 { (120, 30) } else { (PIE_98.1, 0) };
                drawn.push(Drawn::Pie { x: px, y: top + offset, space });
                y = top + box_h;
                content = false;
                bottom = bottom.max(y);
            }
        }
    }
    Laid { title: title_lines, title_top: m.title_top, line_y, drawn, height: bottom + if era == Era::Win2000 { 0 } else { 15 } }
}

/// The pie charts' sizes (98: 100 × 50, 2000: 120 × 59) and the depth of their side.
const PIE_98: (i32, i32, i32) = (100, 50, 8);
const PIE_2000: (i32, i32, i32) = (120, 59, 9);

/// A picture cut from the Reference PCs' webvw.dll (`core/assets/web/*.pic`, made by
/// `private/probes/web/cut.ps1`: size, palette, runs of color indexes).
pub(crate) fn load_pic(bytes: &[u8]) -> Framebuffer {
    let w = i32::from(u16::from_le_bytes([bytes[0], bytes[1]]));
    let h = i32::from(u16::from_le_bytes([bytes[2], bytes[3]]));
    let n = match bytes[4] {
        0 => 256,
        k => usize::from(k),
    };
    let palette: Vec<u32> = (0..n).map(|i| 0xFF00_0000 | u32::from(bytes[5 + 3 * i]) << 16 | u32::from(bytes[6 + 3 * i]) << 8 | u32::from(bytes[7 + 3 * i])).collect();
    let mut fb = Framebuffer::new(w, h, WHITE);
    let mut at = 0usize;
    for pair in bytes[5 + 3 * n..].as_chunks::<2>().0 {
        for _ in 0..pair[0] {
            if at < fb.pixels.len() {
                fb.pixels[at] = palette[usize::from(pair[1])];
            }
            at += 1;
        }
    }
    fb
}

/// The picture in the panel's corner as each Windows showed it at 256 colors
/// (`colors256::web_picture_bytes`).
pub fn corner_picture_256(era: Era) -> &'static Framebuffer {
    static P98: LazyLock<Framebuffer> = LazyLock::new(|| load_pic(crate::colors256::web_picture_bytes(Era::Win98)));
    static P2000: LazyLock<Framebuffer> = LazyLock::new(|| load_pic(crate::colors256::web_picture_bytes(Era::Win2000)));
    if era == Era::Win2000 { &P2000 } else { &P98 }
}

/// The picture in the panel's corner.
pub fn corner_picture(era: Era) -> &'static Framebuffer {
    static P98: LazyLock<Framebuffer> = LazyLock::new(|| load_pic(const { art::bytes("web/left", Look::Win98) }));
    static P2000: LazyLock<Framebuffer> = LazyLock::new(|| load_pic(const { art::bytes("web/left", Look::Win2000) }));
    if era == Era::Win2000 { &P2000 } else { &P98 }
}

/// The warning page's picture (the gears and the Windows logo) at the contents' bottom right.
pub fn logo_picture(era: Era) -> &'static Framebuffer {
    static P98: LazyLock<Framebuffer> = LazyLock::new(|| load_pic(const { art::bytes("web/logo", Look::Win98) }));
    static P2000: LazyLock<Framebuffer> = LazyLock::new(|| load_pic(const { art::bytes("web/logo", Look::Win2000) }));
    if era == Era::Win2000 { &P2000 } else { &P98 }
}

/// 98's line under the title: `wvline.gif` (168 × 1 in four colors) stretched to `w` columns by
/// nearest neighbor, as Internet Explorer stretched it.
fn line_98(fb: &mut Framebuffer, x: i32, y: i32, w: i32) {
    const LINE: &art::Rows = art::rows("web/line", Look::Win98);
    let row = LINE.rows[0].as_bytes();
    for k in 0..w.max(0) {
        // Rounded to the nearest source column (`v1-w*`: the color boundaries of five widths).
        let src = ((k * 168 + w / 2) / w.max(1)) as usize;
        let ch = row[src.min(row.len() - 1)];
        let color = LINE.palette.iter().find(|(c, _)| *c == ch).map_or(0, |&(_, hex)| crate::theme::rgb(hex));
        fb.set(x + k, y, color);
    }
}

/// Paints the laid-out panel into `fb` (as big as the panel) scrolled up by `scroll`.
pub fn paint(fb: &mut Framebuffer, theme: &Theme, fonts: &WebFonts, laid: &Laid, art: Option<&Icon>, scroll: i32, hot: Option<Link>) {
    let _ = hot;
    let era = theme.era;
    let sixteen = theme.depth.reduced();
    let m = metrics(era);
    fb.fill(Rect::new(0, 0, fb.width, fb.height), WHITE);
    let true_line = crate::colors256::true_line(theme);
    if crate::colors256::is_256(theme) {
        // 256 colors: the picture as each PC showed it there (98's watercolor and 2000's corner
        // picture, dithered in the browser's halftone colors).
        fb.blit(corner_picture_256(era), 0, -scroll);
    } else if sixteen {
        // 16 colors: 98 shows no watercolor at all (the Reference PC); 2000's picture by its rule.
        if era == Era::Win2000 {
            let mut pic = corner_picture(era).clone();
            for p in &mut pic.pixels {
                *p = crate::colors16::convert(era, *p);
            }
            fb.blit(&pic, 0, -scroll);
        }
    } else {
        fb.blit(corner_picture(era), 0, -scroll);
    }
    if let Some(icon) = art {
        crate::colors16::draw_picture(fb, theme, icon, m.icon.0, m.icon.1 - scroll);
    }
    for (k, line) in laid.title.iter().enumerate() {
        fonts.title.draw(fb, m.left, laid.title_top + k as i32 * m.title_pitch - scroll, line, BLACK);
    }
    let room = fb.width - SCROLL_W;
    if era == Era::Win2000 {
        // At 256 colors True Color's own line (6699CC is a halftone color).
        let blue = if true_line { 0xFF66_99CC } else { crate::colors16::line(theme, 0xFF66_99CC) };
        fb.fill(Rect::new(0, laid.line_y - scroll, room, m.line_h), blue);
    } else {
        line_98(fb, 0, laid.line_y - scroll, room);
        if sixteen && !true_line {
            // Each pixel of the stretched line by 98's rule: red, yellow, lime, aqua (the Reference PC).
            for x in 0..room {
                let (xx, yy) = (x, laid.line_y - scroll);
                if yy >= 0 && yy < fb.height && xx < fb.width {
                    let p = fb.get(xx, yy);
                    fb.set(xx, yy, crate::colors16::convert(era, p));
                }
            }
        }
    }
    for d in &laid.drawn {
        match d {
            Drawn::Text { x, y, text, style } => {
                let font = font_of(era, fonts, *style);
                let color = match style {
                    Style::Link(_) => LINK,
                    Style::Warning => WARNING,
                    _ => BLACK,
                };
                let w = font.draw(fb, *x, *y - scroll, text, color);
                if matches!(style, Style::Link(_)) {
                    fb.hline(*x, *y - scroll + 12, w, LINK);
                }
            }
            Drawn::Legend { x, y, used } => {
                let fill = match (era, used) {
                    (_, false) => WHITE,
                    (Era::Win2000, true) => 0xFFD4_D0C8,
                    _ => 0xFFC0_C0C0,
                };
                let r = Rect::new(*x, *y - scroll, 12, 12);
                fb.fill(r, crate::colors16::line(theme, fill));
                fb.frame(r, BLACK, BLACK);
            }
            Drawn::Pie { x, y, space } => {
                let (w, h, depth) = if era == Era::Win2000 { PIE_2000 } else { PIE_98 };
                let used_color = if era == Era::Win2000 { 0xFFD4_D0C8 } else { 0xFFDF_DFDF };
                let used = space.capacity.saturating_sub(space.free);
                let per_1000 = if space.capacity == 0 { 0 } else { (u128::from(used) * 1000 / u128::from(space.capacity)) as u32 };
                pie(fb, Rect::new(*x, *y - scroll, w, h), depth, per_1000, crate::colors16::line(theme, used_color));
            }
            Drawn::Note { rect, lines } => {
                let r = Rect::new(rect.x, rect.y - scroll, rect.w, rect.h);
                crate::colors16::fill(fb, theme, r, 0xFFFF_FFE1);
                // At 256 colors the browser's halftone gray nearest D3D3D3 (2000's PC: D7D7D7).
                let edge = if crate::colors256::is_256(theme) { 0xFFD7_D7D7 } else { crate::colors16::line(theme, 0xFFD3_D3D3) };
                fb.frame(r, edge, edge);
                for (k, l) in lines.iter().enumerate() {
                    fonts.body.draw(fb, r.x + 1, r.y + 1 + k as i32 * PITCH, l, BLACK);
                }
            }
        }
    }
}

/// Integer square root, rounded down.
fn isqrt(n: u64) -> i64 {
    if n == 0 {
        return 0;
    }
    let mut x = (n as f64).sqrt() as u64;
    while x * x > n {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= n {
        x += 1;
    }
    x as i64
}

/// Whether pixel (x, y) of a `w` × `h` box lies inside the ellipse that fills the box.
fn in_ellipse(x: i32, y: i32, w: i32, h: i32) -> bool {
    let (rx, ry) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
    let dx = (f64::from(x) + 0.5 - rx) / rx;
    let dy = (f64::from(y) + 0.5 - ry) / ry;
    dx * dx + dy * dy <= 1.0
}

/// The drive's pie chart (shell32's 3-D pie, as the panels drew it): the top an ellipse, its used
/// part from nine o'clock clockwise in `used_color`, the free part white, the side below it in the
/// used color, outlined in black. `per_1000`: the used share.
fn pie(fb: &mut Framebuffer, r: Rect, depth: i32, per_1000: u32, used_color: u32) {
    let w = r.w;
    let h = r.h - depth;
    let rx = w / 2;
    let ry = h / 2;
    let (cx, cy) = (rx - 1, ry - 1);
    // Where the used part ends on the rim (shell32's triangle-area approximation).
    let pct = per_1000.min(1000) as i64;
    let q = ((pct % 500) - 250).abs();
    let (rx64, ry64) = (rx as i64, ry as i64);
    let (mut ex, mut ey) = if q < 120 {
        let x = isqrt(((rx64 * rx64 * q * q) / (q * q + (250 - q) * (250 - q))) as u64);
        let y = isqrt((((rx64 * rx64 - x * x) * ry64 * ry64) / (rx64 * rx64)) as u64);
        (x, y)
    } else {
        let y = isqrt(((ry64 * ry64 * (250 - q) * (250 - q)) / (q * q + (250 - q) * (250 - q))) as u64);
        let x = isqrt((((ry64 * ry64 - y * y) * rx64 * rx64) / (ry64 * ry64)) as u64);
        (x, y)
    };
    match pct / 250 {
        1 => ey = -ey,
        2 => {}
        3 => ex = -ex,
        _ => {
            ex = -ex;
            ey = -ey;
        }
    }
    let end = (ex as f64, ey as f64);
    // Angles measured clockwise from nine o'clock (screen y down).
    let angle = |dx: f64, dy: f64| {
        let a = (-dy).atan2(-dx);
        let a = -a;
        if a < 0.0 { a + std::f64::consts::TAU } else { a }
    };
    let end_angle = if pct >= 1000 { std::f64::consts::TAU } else { angle(end.0, end.1) };
    let inside = |x: i32, y: i32| in_ellipse(x, y, 2 * rx, 2 * ry);
    // The side: the lower half of the ellipse moved down by `depth`, its left and right edges.
    for y in cy..2 * ry + depth {
        for x in 0..2 * rx {
            let below = inside(x, y - depth) && y - depth >= cy;
            let band = y <= cy + depth && y >= cy;
            if (below || band) && !inside(x, y) {
                fb.set(r.x + x, r.y + y, used_color);
            }
        }
    }
    for y in 0..2 * ry {
        for x in 0..2 * rx {
            if !inside(x, y) {
                continue;
            }
            let a = angle(f64::from(x - cx), f64::from(y - cy));
            let used = pct > 0 && a <= end_angle;
            fb.set(r.x + x, r.y + y, if used { used_color } else { WHITE });
        }
    }
    // Outlines: the top's rim, the side's lower rim and edges, the two radii of the used part.
    let edge = |x: i32, y: i32, f: &dyn Fn(i32, i32) -> bool| f(x, y) && (!f(x - 1, y) || !f(x + 1, y) || !f(x, y - 1) || !f(x, y + 1));
    for y in -1..2 * ry + depth + 1 {
        for x in -1..2 * rx + 1 {
            let top = edge(x, y, &|a, b| inside(a, b));
            let low = y - depth >= cy && edge(x, y - depth, &|a, b| inside(a, b)) && !inside(x, y);
            if top || low {
                fb.set(r.x + x, r.y + y, BLACK);
            }
        }
    }
    for y in cy..=cy + depth {
        fb.set(r.x, r.y + y, BLACK);
        fb.set(r.x + 2 * rx - 1, r.y + y, BLACK);
    }
    if pct > 0 && pct < 1000 {
        line(fb, r.x + cx, r.y + cy, r.x, r.y + cy);
        line(fb, r.x + cx, r.y + cy, r.x + cx + ex as i32, r.y + cy + ey as i32);
        if pct > 500 {
            fb.vline(r.x + cx + ex as i32, r.y + cy + ey as i32, depth + 1, BLACK);
        }
    }
}

/// A one-pixel black line (Bresenham, both ends drawn).
fn line(fb: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        fb.set(x, y, BLACK);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// The links' places (panel pixels, before scrolling).
pub fn link_rects(era: Era, fonts: &WebFonts, laid: &Laid) -> Vec<(Rect, Link)> {
    laid.drawn
        .iter()
        .filter_map(|d| match d {
            Drawn::Text { x, y, text, style: Style::Link(l) } => Some((Rect::new(*x, *y, font_of(era, fonts, Style::Link(*l)).measure(text), PITCH), *l)),
            _ => None,
        })
        .collect()
}

/// Paints 98's band above the icons (the panel did not fit): the title in bold Verdana.
pub fn paint_banner(fb: &mut Framebuffer, fonts: &WebFonts, r: Rect, title: &str) {
    fb.fill(r, WHITE);
    fonts.title.draw(fb, r.x + 18, r.y + 3, title.trim(), BLACK);
}

/// Paints the warning page's right side (or, where the panel does not fit, the whole contents with
/// the warning's words): white, the logo at the bottom right.
pub fn paint_brand(fb: &mut Framebuffer, era: Era, fonts: &WebFonts, r: Rect, words: Option<&WebWords>) {
    fb.fill(r, WHITE);
    let logo = logo_picture(era);
    // 98's at the area's bottom right; 2000's a row lower (its last row hidden), as captured.
    let below = i32::from(era == Era::Win2000);
    let (lx, ly) = (r.right() - logo.width, r.bottom() + below - logo.height);
    for y in 0..logo.height {
        let sy = ly + y;
        if sy < r.y || sy >= r.bottom() {
            continue;
        }
        for x in 0..logo.width {
            let sx = lx + x;
            if sx >= r.x && sx < r.right() {
                fb.set(sx, sy, logo.get(x, y));
            }
        }
    }
    if let Some(words) = words {
        for d in &words.drawn {
            if let Drawn::Text { x, y, text, style } = d {
                let font = font_of(era, fonts, *style);
                let color = match style {
                    Style::Link(_) => LINK,
                    Style::Warning => WARNING,
                    _ => BLACK,
                };
                let w = font.draw(fb, r.x + x, r.y + y, text, color);
                if matches!(style, Style::Link(_)) {
                    fb.hline(r.x + x, r.y + y + 12, w, LINK);
                }
            }
        }
    }
}

/// The warning's words laid across the whole contents (the panel did not fit).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebWords {
    pub drawn: Vec<Drawn>,
}

/// Lays the warning's words out across a contents area `width` wide (98: a paragraph 15 from the
/// edges; 2000: 12 from the left).
pub fn lay_out_words(era: Era, fonts: &WebFonts, width: i32, web: &Web) -> WebWords {
    let (left, top, right) = if era == Era::Win2000 { (12, 12, width - 12) } else { (15, 15, width - 15) };
    let flows = flow(era, web.kind, web.guard, false, None, &[], 0, false);
    let mut drawn = Vec::new();
    let (mut x, mut y, mut content) = (left, top, false);
    for f in flows {
        match f {
            Flow::Br => {
                y += PITCH;
                x = left;
                content = false;
            }
            Flow::P(gap) => {
                if content {
                    y += PITCH;
                }
                y += gap;
                x = left;
                content = false;
            }
            Flow::Text(s, style) => {
                let font = font_of(era, fonts, style);
                for (space, word) in words(&s) {
                    let sp = if space && content { font.measure(" ") } else { 0 };
                    let ww = font.measure(&word);
                    if content && x + sp + ww > right {
                        y += PITCH;
                        x = left;
                        content = false;
                    }
                    let gap = if content { sp } else { 0 };
                    if !word.is_empty() {
                        let linked = matches!(style, Style::Link(_)) && gap > 0 && drawn.last().is_some_and(|d| matches!(d, Drawn::Text { style: s, y: dy, .. } if *s == style && *dy == y));
                        let (dx, shown) = if linked { (x, format!(" {word}")) } else { (x + gap, word.clone()) };
                        drawn.push(Drawn::Text { x: dx, y, text: shown, style });
                        x += gap + ww;
                        content = true;
                    }
                }
            }
            _ => {}
        }
    }
    WebWords { drawn }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Font {
        let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        Font::parse(&std::fs::read_to_string(path).unwrap()).unwrap().with_period_glyphs()
    }

    pub fn fonts(era: Era) -> WebFonts {
        match era {
            Era::Win2000 => WebFonts { body: fixture("tahoma-11px.font"), bold: fixture("tahoma-11px-bold.font"), title: fixture("tahoma-17px-bold.font") },
            _ => WebFonts { body: fixture("verdana-11px.font"), bold: fixture("verdana-11px-bold.font"), title: fixture("verdana-21px-bold.font") },
        }
    }

    fn lines(laid: &Laid) -> Vec<String> {
        let mut rows: Vec<(i32, String)> = Vec::new();
        for d in &laid.drawn {
            if let Drawn::Text { y, text, .. } = d {
                match rows.last_mut() {
                    Some((ry, s)) if ry == y => {
                        s.push(' ');
                        s.push_str(text);
                    }
                    _ => rows.push((*y, text.clone())),
                }
            }
        }
        rows.into_iter().map(|(_, s)| s).collect()
    }

    fn file(name: &str, size: u64) -> Info {
        Info {
            name: name.trim_end_matches(".txt").into(),
            file_name: name.into(),
            type_name: "Text Document".into(),
            size,
            modified: crate::desktop_icons::local_filetime(2026, 11, 15, 13, 39),
            what: What::File,
            attributes: Some(0x20),
        }
    }

    #[test]
    fn the_panel_is_as_wide_as_each_look_made_it() {
        // 98 (`v1-w*`): 30% rounded, gone under 400; 2000: 200, gone under 400.
        for (inside, want) in [(760, Some(228)), (640, Some(192)), (500, Some(150)), (420, Some(126)), (404, Some(121)), (402, Some(121)), (400, Some(120)), (398, None)] {
            assert_eq!(panel_width(Era::Win98, inside, false), want, "98 at {inside}");
        }
        assert_eq!(panel_width(Era::Win98, 440, true), None, "the warning page needs 450");
        for (inside, want) in [(760, Some(200)), (400, Some(200)), (399, None)] {
            assert_eq!(panel_width(Era::Win2000, inside, false), want);
        }
        assert_eq!(panel_width(Era::Win95, 800, false), None);
    }

    #[test]
    fn the_98_panel_stops_growing_at_480() {
        // Up to 1600 wide as Windows 98 made it; wider windows keep 480 (GitHub issue 2).
        for (inside, want) in [(800, 240), (1024, 307), (1280, 384), (1590, 477), (1598, 479), (1600, 480), (1601, 480), (1602, 480), (1920, 480), (2560, 480)] {
            assert_eq!(panel_width(Era::Win98, inside, false), Some(want), "98 at {inside}");
            assert_eq!(panel_width(Era::Win98, inside, true), Some(want), "98's warning page at {inside}");
        }
        assert_eq!(panel_width(Era::Win2000, 2560, false), Some(200), "2000 keeps its 200");
    }

    #[test]
    fn the_98_panel_says_what_is_selected_as_folder_htt_did() {
        let f = fonts(Era::Win98);
        let web = Web { on: true, ..Web::default() };
        // `v1-none` at 500 (panel 150) and at 400 (panel 120), `w640`.
        assert_eq!(lines(&lay_out(Era::Win98, &f, 150, "v1", &web, &[], 0, false)), ["Select an item to", "view its", "description."]);
        assert_eq!(lines(&lay_out(Era::Win98, &f, 120, "v1", &web, &[], 0, false)), ["Select an", "item to view", "its", "description."]);
        assert_eq!(lines(&lay_out(Era::Win98, &f, 192, "v1", &web, &[], 0, false)), ["Select an item to view its", "description."]);
        // `v1-file`: the name with its extension, type, date, size; at 105, 13 apart.
        let laid = lay_out(Era::Win98, &f, 150, "v1", &web, &[file("small.txt", 51)], 1, false);
        assert_eq!(lines(&laid), ["small.txt", "Text Document", "Modified:", "11/15/26 1:39 PM", "Size: 51 bytes"]);
        let ys: Vec<i32> = laid.drawn.iter().filter_map(|d| if let Drawn::Text { y, x, .. } = d { (*x == 15).then_some(*y) } else { None }).collect();
        assert_eq!(ys, [105, 118, 144, 157, 183]);
        assert_eq!(laid.line_y, 82);
        // `v1-several`: the count, the total in bytes (wrapped before the number), the names.
        let several = [file("Beta", 0), file("medium.txt", 6644), file("small.txt", 51)];
        let several: Vec<Info> = several.into_iter().enumerate().map(|(k, mut i)| {
            if k == 0 {
                i.what = What::Folder;
                i.size = 0;
            }
            i
        }).collect();
        assert_eq!(lines(&lay_out(Era::Win98, &f, 150, "v1", &web, &several, 3, false)), ["3 items selected.", "Total File Size:", "6,695 bytes", "Beta", "medium", "small"]);
        // A long title wraps (`mc-none`: "My / Computer", the line and text 25 lower).
        let laid = lay_out(Era::Win98, &f, 150, "My Computer", &web, &[], 0, true);
        assert_eq!(laid.title, ["My", "Computer"]);
        assert_eq!(laid.line_y, 107);
    }

    #[test]
    fn the_2000_panel_says_what_is_selected_as_webvw_did() {
        let f = fonts(Era::Win2000);
        let web = Web { on: true, ..Web::default() };
        let laid = lay_out(Era::Win2000, &f, 200, "v1", &web, &[], 0, false);
        assert_eq!(lines(&laid), ["Select an item to view its", "description.", "See also:", "My Documents", "My Network Places", "My Computer"]);
        let ys: Vec<i32> = laid.drawn.iter().filter_map(|d| if let Drawn::Text { y, .. } = d { Some(*y) } else { None }).collect();
        assert_eq!(ys[0], 89);
        assert_eq!(*ys.last().unwrap(), 178, "`v1-none`: the links 17 apart from See also at 127");
        let laid = lay_out(Era::Win2000, &f, 200, "v1", &web, &[file("small.txt", 51)], 1, false);
        assert_eq!(lines(&laid), ["small", "Text Document", "Modified: 11/15/2026 1:39 PM", "Size: 51 bytes", "Attributes: (normal)"]);
        assert_eq!(attribute_words(0x1 | 0x4 | 0x20), "Read-only, System");
    }
}

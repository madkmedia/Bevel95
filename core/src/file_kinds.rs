//! Kinds of file as the look's own Windows knew them: a file whose ending that Windows knew by
//! itself shows that Windows' picture and Type name; every other file keeps today's (the shell's).
//!
//! What a file shows is decided in one place, [`decide`]: the look's picture (`Choice::Look`), the
//! file's own picture where it has one, else the look's picture of a program
//! (`Choice::OwnOrProgram`: programs and screen savers, as all three Windows drew a program without
//! an icon of its own), or today's (`Choice::Today`: every kind the look did not know, and the kinds
//! whose picture was the file's own: icons, cursors, shortcuts). Its Type name is the look's for
//! every kind the look knew, today's for the rest. (The other rule considered, every file the look
//! did not know showing the look's unknown page, would change only `decide`; it is not built: the
//! owner chose this one on 2026-11-23, "Things can't have no icons, so if no old icons, then
//! they'll need new ones".)
//!
//! A known kind needs no question to the shell: its picture is here, found by its ending while the
//! window paints, so a folder of thousands of text files shows its pictures with its names. The
//! tables are the Reference PCs' own (`tables.rs`); the pictures are Windows' and kept in bevel-art
//! (`art/src/file_icons.rs`); this module chooses and draws them.

use bevel_art::{Depth as ArtDepth, Place};

use crate::art::{self, Look};
use crate::colors16::Depth;
use crate::fb::Icon;
use crate::theme::Era;

mod tables;

/// What a look's table says of a kind's picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    /// The picture of that name in the look's sheet ("shell32-152": the file and its icon group).
    Fixed(&'static str),
    /// A program's own picture, or the look's program picture when it has none (.exe, .scr).
    Program,
    /// The file's own picture (icons, cursors, shortcuts): today's shell shows those.
    Own,
    /// The file's own picture where it names one, else the look's picture of that name (an
    /// Internet Shortcut with its own `IconFile`, as a Steam game's: issue 8, brief 100).
    OwnOrFixed(&'static str),
}

/// Which of the look's pictures an item shows (`desktop_icons::Picture::Kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Its kind's, found by the ending of its name (the item's id) when it is drawn.
    Ending,
    /// The look's picture of a program (shell32's program picture: the same group in all three).
    Program,
}

/// Where an item's picture comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// The look's own (no question to the shell).
    Look(Kind),
    /// The file's own picture if it has one (read from the shell off the window's thread), else
    /// the look's program picture.
    OwnOrProgram,
    /// The file's own picture if it has one (the shell's picture of the file is not the one it
    /// gives its kind), else the look's picture of its kind (brief 100: a file's own picture wins).
    OwnOrLook,
    /// Today's picture from the shell.
    Today,
}

/// What a file shows: its picture's source and, where the look knew its kind, the look's Type name
/// (`None`: today's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub picture: Choice,
    pub type_name: Option<&'static str>,
}

/// The one place where it is decided what a file of `file_name` shows in `era` (the rule: a kind
/// the look knew by itself shows the look's picture and name, every other file today's).
pub fn decide(era: Era, file_name: &str) -> Decision {
    let Some((type_name, shown)) = ending(file_name).and_then(|e| known(era, &e)) else {
        // A kind the look did not know: today's picture and name.
        return Decision { picture: Choice::Today, type_name: None };
    };
    let picture = match shown {
        Shown::Fixed(_) => Choice::Look(Kind::Ending),
        Shown::Program => Choice::OwnOrProgram,
        Shown::Own => Choice::Today,
        Shown::OwnOrFixed(_) => Choice::OwnOrLook,
    };
    Decision { picture, type_name: Some(type_name) }
}

/// Whether a file of a kind that may carry its own picture (`Choice::OwnOrLook`: an Internet
/// Shortcut) names one: its `IconFile` (the one line asked of the file; `None`: no such line) holds
/// something (brief 100, issue 8). The shell cannot tell: for an Internet Shortcut without a picture
/// of its own it answers with today's browser's picture, not its kind's.
pub fn names_own_picture(icon_file: Option<&str>) -> bool {
    icon_file.is_some_and(|f| !f.trim().is_empty())
}

/// A file name's ending as Windows found it (`PathFindExtension`): what follows the last dot, if
/// that holds no space; lower case. `None` without one ("noext", "a.", "a.b c").
pub fn ending(file_name: &str) -> Option<String> {
    let dot = file_name.rfind('.')?;
    let e = &file_name[dot + 1..];
    (!e.is_empty() && !e.contains(' ')).then(|| e.to_ascii_lowercase())
}

/// The last part of a path (an item's id), or the whole of it.
fn file_name_of(id: &str) -> &str {
    id.rsplit(['\\', '/']).next().unwrap_or(id)
}

fn table(era: Era) -> &'static [(&'static str, &'static str, Shown)] {
    match era {
        Era::Win95 => tables::WIN95,
        Era::Win98 => tables::WIN98,
        Era::Win2000 => tables::WIN2000,
    }
}

/// What `era`'s table says of `ending` (lower case): its Type name and its picture.
pub fn known(era: Era, ending: &str) -> Option<(&'static str, Shown)> {
    table(era).iter().find(|(e, ..)| *e == ending).map(|&(_, name, shown)| (name, shown))
}

/// The look's program picture (shell32's group 3 in all three Windows: a program without an icon
/// of its own, an MS-DOS program, a .com).
pub const PROGRAM: &str = "shell32-3";

/// The look's picture for an item (`id`: its path) of kind `kind`, 32×32 or (`small`) 16×16, as it
/// shows at `depth` (at 256 and 16 colors Windows 98 and 2000 showed the 16-color images). `None`
/// if the look does not know the kind (the look changed since the item was read: its reader reads
/// it again).
pub fn picture(era: Era, depth: Depth, kind: Kind, id: &str, small: bool) -> Option<Icon> {
    let name = match kind {
        Kind::Program => PROGRAM,
        Kind::Ending => match known(era, &ending(file_name_of(id))?)?.1 {
            Shown::Fixed(name) | Shown::OwnOrFixed(name) => name,
            _ => return None,
        },
    };
    named(era, depth, name, small)
}

const SHEET_95: &[u8] = art::bytes("file-icons/sheet", Look::Win95);
const SHEET_98: &[u8] = art::bytes("file-icons/sheet", Look::Win98);
const SHEET_2000: &[u8] = art::bytes("file-icons/sheet", Look::Win2000);
const TRUE_95: &[Place] = art::places("file-icons/places", Look::Win95);
const TRUE_98: &[Place] = art::places("file-icons/places", Look::Win98);
const TRUE_2000: &[Place] = art::places("file-icons/places", Look::Win2000);
const SIXTEEN_98: &[Place] = art::places_at("file-icons/places", Look::Win98, ArtDepth::Sixteen);
const SIXTEEN_2000: &[Place] = art::places_at("file-icons/places", Look::Win2000, ArtDepth::Sixteen);

fn sheet(era: Era, depth: Depth) -> (&'static [u8], &'static [Place]) {
    match (era, depth.reduced()) {
        (Era::Win95, _) => (SHEET_95, TRUE_95),
        (Era::Win98, false) => (SHEET_98, TRUE_98),
        (Era::Win98, true) => (SHEET_98, SIXTEEN_98),
        (Era::Win2000, false) => (SHEET_2000, TRUE_2000),
        (Era::Win2000, true) => (SHEET_2000, SIXTEEN_2000),
    }
}

/// The picture called `name` ("shell32-152") in `era`'s sheet at `depth`.
pub fn named(era: Era, depth: Depth, name: &str, small: bool) -> Option<Icon> {
    let (sheet, places) = sheet(era, depth);
    let size = if small { 16 } else { 32 };
    let &(_, at, w, h) = places.iter().find(|&&(n, _, w, _)| w == size && n.strip_prefix(name).is_some_and(|rest| rest.starts_with('/')))?;
    decode(sheet.get(at..)?, w, h)
}

/// One picture of a sheet (art/src/file_icons.rs says how it is kept).
fn decode(bytes: &[u8], w: i32, h: i32) -> Option<Icon> {
    let n = usize::from(*bytes.first()?);
    let colors = bytes.get(1..1 + 3 * n)?;
    let count = (w * h) as usize;
    let data = &bytes[1 + 3 * n..];
    let index = |k: usize| -> Option<usize> {
        Some(if n <= 15 {
            let b = *data.get(k / 2)?;
            usize::from(if k.is_multiple_of(2) { b >> 4 } else { b & 0x0F })
        } else {
            usize::from(*data.get(k)?)
        })
    };
    let mut pixels = Vec::with_capacity(count);
    for k in 0..count {
        let i = index(k)?;
        pixels.push(if i == 0 {
            0
        } else {
            let c = colors.get(3 * (i - 1)..3 * i)?;
            0xFF00_0000 | u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2])
        });
    }
    Some(Icon { width: w, height: h, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ERAS: [Era; 3] = [Era::Win95, Era::Win98, Era::Win2000];

    #[test]
    fn each_look_knows_its_own_kinds() {
        for era in ERAS {
            assert_eq!(known(era, "txt"), Some(("Text Document", Shown::Fixed("shell32-152"))), "{era:?}");
            assert_eq!(known(era, "ini"), Some(("Configuration Settings", Shown::Fixed("shell32-151"))));
            assert_eq!(known(era, "bat"), Some(("MS-DOS Batch File", Shown::Fixed("shell32-153"))));
            assert_eq!(known(era, "dll"), Some(("Application Extension", Shown::Fixed("shell32-154"))));
            assert_eq!(known(era, "com"), Some(("MS-DOS Application", Shown::Fixed(PROGRAM))));
            assert_eq!(known(era, "exe"), Some(("Application", Shown::Program)));
            assert_eq!(known(era, "ico"), Some(("Icon", Shown::Own)));
            assert_eq!(known(era, "lnk"), Some(("Shortcut", Shown::Own)));
            assert_eq!(known(era, "url"), Some(("Internet Shortcut", Shown::OwnOrFixed("url-102"))));
            // Not known to any of them (or only as a MIME type, with the unknown page).
            for e in ["pdf", "docx", "mp4", "zip", "json", "md", "7z"] {
                assert_eq!(known(era, e), None, "{era:?} {e}");
            }
        }
        // Each look its own pictures and names.
        assert_eq!(known(Era::Win95, "hlp").map(|k| k.1), Some(Shown::Fixed("winhlp32-4001")));
        assert_eq!(known(Era::Win2000, "hlp").map(|k| k.1), Some(Shown::Fixed("shell32-24")));
        assert_eq!(known(Era::Win95, "avi").map(|k| k.0), Some("Movie Clip (AVI)"));
        assert_eq!(known(Era::Win98, "avi").map(|k| k.0), Some("Video Clip"));
        assert_eq!(known(Era::Win98, "htm").map(|k| k.0), Some("Microsoft HTML Document 4.0"));
        assert_eq!(known(Era::Win2000, "htm").map(|k| k.0), Some("Microsoft HTML Document 5.0"));
        assert_eq!(known(Era::Win2000, "bmp").map(|k| k.1), Some(Shown::Fixed("mspaint-2")));
        // 2000's fonts show the file's own picture (its font handler).
        assert_eq!(known(Era::Win95, "ttf").map(|k| k.1), Some(Shown::Fixed("shell32-156")));
        assert_eq!(known(Era::Win2000, "ttf").map(|k| k.1), Some(Shown::Own));
    }

    #[test]
    fn an_ending_only_another_look_knew() {
        for (e, only) in [("log", Era::Win2000), ("cmd", Era::Win2000), ("ocx", Era::Win2000), ("msi", Era::Win2000), ("theme", Era::Win98)] {
            for era in ERAS {
                assert_eq!(known(era, e).is_some(), era == only, "{e} in {era:?}");
            }
        }
        for e in ["chm", "cab", "js", "vbs"] {
            assert!(known(Era::Win95, e).is_none() && known(Era::Win98, e).is_some() && known(Era::Win2000, e).is_some(), "{e}");
        }
        assert_eq!(decide(Era::Win95, "setup.log"), Decision { picture: Choice::Today, type_name: None });
        assert_eq!(decide(Era::Win2000, "setup.log"), Decision { picture: Choice::Look(Kind::Ending), type_name: Some("Text Document") });
    }

    #[test]
    fn endings_as_windows_found_them() {
        assert_eq!(ending("notes.txt").as_deref(), Some("txt"));
        assert_eq!(ending("NOTES.TXT").as_deref(), Some("txt"));
        assert_eq!(ending("Notes.TxT").as_deref(), Some("txt"));
        assert_eq!(ending("a.b.txt").as_deref(), Some("txt"), "the last dot");
        assert_eq!(ending("report.txt.zip").as_deref(), Some("zip"));
        assert_eq!(ending(".txt").as_deref(), Some("txt"), "a name that is only an ending");
        assert_eq!(ending("noext"), None);
        assert_eq!(ending("a."), None);
        assert_eq!(ending("version 1.2 notes"), None, "a space after the dot: no ending");
        assert_eq!(ending("caf\u{e9}.T\u{c9}X"), Some("t\u{c9}x".to_string()), "only ASCII letters change case");
        for era in ERAS {
            assert_eq!(decide(era, "NOTES.TXT"), decide(era, "notes.txt"));
            assert_eq!(decide(era, "noext"), Decision { picture: Choice::Today, type_name: None });
            assert_eq!(decide(era, "a.b.txt").type_name, Some("Text Document"));
            assert_eq!(decide(era, "report.txt.zip"), Decision { picture: Choice::Today, type_name: None });
        }
        assert_eq!(file_name_of("C:\\L\\a.txt"), "a.txt");
        assert_eq!(file_name_of("a.txt"), "a.txt");
    }

    #[test]
    fn the_one_deciding_place() {
        for era in ERAS {
            assert_eq!(decide(era, "a.txt"), Decision { picture: Choice::Look(Kind::Ending), type_name: Some("Text Document") });
            assert_eq!(decide(era, "a.exe"), Decision { picture: Choice::OwnOrProgram, type_name: Some("Application") });
            assert_eq!(decide(era, "a.scr"), Decision { picture: Choice::OwnOrProgram, type_name: Some("Screen Saver") });
            assert_eq!(decide(era, "a.ico"), Decision { picture: Choice::Today, type_name: Some("Icon") });
            assert_eq!(decide(era, "a.lnk"), Decision { picture: Choice::Today, type_name: Some("Shortcut") });
            // Issue 8 (brief 100): an Internet Shortcut keeps a picture of its own (a Steam game's),
            // and one without shows the old kind's picture; its Type name stays the look's.
            assert_eq!(decide(era, "Game.url"), Decision { picture: Choice::OwnOrLook, type_name: Some("Internet Shortcut") });
            assert!(names_own_picture(Some("C:\\Program Files (x86)\\Steam\\steam\\games\\game.ico")), "its own picture");
            assert!(!names_own_picture(None), "no IconFile line: the kind's picture");
            assert!(!names_own_picture(Some("  ")), "an empty IconFile: the kind's picture");
            assert!(picture(era, Depth::TrueColor, Kind::Ending, "C:\\Desktop\\Game.url", false).is_some(), "the kind's picture for one without");
            assert_eq!(decide(era, "a.pdf"), Decision { picture: Choice::Today, type_name: None });
        }
    }

    #[test]
    fn every_picture_is_there_at_every_depth() {
        let fives = |v: u32| (v & 0xF8) | (v >> 5) == v;
        let sixteen = [0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xC0C0C0, 0x808080, 0xFF0000, 0x00FF00, 0xFFFF00, 0x0000FF, 0xFF00FF, 0x00FFFF, 0xFFFFFF];
        for era in ERAS {
            let mut names: Vec<&str> = table(era).iter().filter_map(|&(_, _, s)| if let Shown::Fixed(n) = s { Some(n) } else { None }).collect();
            names.push(PROGRAM);
            names.push("shell32-1");
            for name in names {
                for depth in [Depth::TrueColor, Depth::TwoFiftySix, Depth::Sixteen] {
                    for small in [false, true] {
                        let icon = named(era, depth, name, small).unwrap_or_else(|| panic!("{era:?} {name} {depth:?} {small}"));
                        let size = if small { 16 } else { 32 };
                        assert_eq!((icon.width, icon.height, icon.pixels.len()), (size, size, (size * size) as usize));
                        assert!(icon.pixels.iter().any(|&p| p >> 24 == 0xFF), "{era:?} {name}: something is drawn");
                        assert!(icon.pixels.iter().all(|&p| p == 0 || p >> 24 == 0xFF), "see-through or not");
                        for &p in icon.pixels.iter().filter(|&&p| p != 0) {
                            let c = p & 0xFF_FFFF;
                            if era == Era::Win95 || depth.reduced() {
                                assert!(sixteen.contains(&c), "{era:?} {name} {depth:?}: {c:06X} is one of the 16");
                            } else if era == Era::Win2000 {
                                assert!(fives(c >> 16) && fives((c >> 8) & 0xFF) && fives(c & 0xFF), "{name}: {c:06X} through 5 bits");
                            }
                        }
                    }
                }
            }
        }
        // Every place lies inside its sheet.
        for (sheet, places) in [(SHEET_95, TRUE_95), (SHEET_98, TRUE_98), (SHEET_98, SIXTEEN_98), (SHEET_2000, TRUE_2000), (SHEET_2000, SIXTEEN_2000)] {
            for &(name, at, w, h) in places {
                assert!(at < sheet.len() && decode(&sheet[at..], w, h).is_some(), "{name}");
            }
        }
        assert!(named(Era::Win98, Depth::TrueColor, "no-such-picture", false).is_none());
        assert!(named(Era::Win98, Depth::TrueColor, "shell32-15", false).is_none(), "a name is matched whole");
    }

    #[test]
    fn an_items_picture_by_its_ending() {
        let t = picture(Era::Win98, Depth::TrueColor, Kind::Ending, "C:\\L\\Notes.TXT", false).unwrap();
        assert_eq!(Some(t), named(Era::Win98, Depth::TrueColor, "shell32-152", false));
        assert!(picture(Era::Win95, Depth::TrueColor, Kind::Ending, "C:\\L\\setup.log", false).is_none(), "95 did not know .log");
        assert!(picture(Era::Win95, Depth::TrueColor, Kind::Ending, "C:\\L\\a.exe", false).is_none(), "a program is Kind::Program");
        let p = picture(Era::Win2000, Depth::Sixteen, Kind::Program, "C:\\L\\a.exe", true).unwrap();
        assert_eq!(p.width, 16);
    }
}

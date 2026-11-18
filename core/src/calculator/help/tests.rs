use super::*;
use crate::calculator::engine::{Angle, Key as CalcKey, Radix, Word};
use crate::calculator::form::{CalculatorWindow, Command};
use crate::help::wire;
use crate::kit::{Form, Key, Mods};
use std::collections::BTreeSet;

const ERAS: [Era; 3] = [Era::Win95, Era::Win98, Era::Win2000];

fn distinct_keywords(book: &Catalog) -> usize {
    book.index.iter().map(|k| k.label.to_lowercase()).collect::<BTreeSet<_>>().len()
}

#[test]
fn each_book_is_well_formed_and_crosses_the_wire_unchanged() {
    for era in ERAS {
        let book = catalog(era);
        let n = book.topics.len();
        assert!(book.home < n, "{era:?} home");
        for entry in &book.contents {
            assert!(entry.topic.is_none_or(|t| t < n), "{era:?} contents entry {}", entry.label);
            assert_eq!(
                entry.topic.is_none(),
                entry.icon == EntryIcon::Book,
                "{era:?} {} is a book or a page",
                entry.label
            );
        }
        for keyword in &book.index {
            assert!(keyword.topic < n, "{era:?} keyword {}", keyword.label);
        }
        for (i, topic) in book.topics.iter().enumerate() {
            assert!(!topic.title.is_empty(), "{era:?} topic {i} has a title");
            assert!(
                matches!(topic.paragraphs[0].style, Title | Subhead),
                "{era:?} {} starts with its heading",
                topic.title
            );
            for p in &topic.paragraphs {
                match &p.style {
                    Link(t) => assert!(*t < n, "{era:?} {} link", topic.title),
                    Related(ts) => assert!(
                        !ts.is_empty() && ts.iter().all(|t| *t < n && *t != i),
                        "{era:?} {} related",
                        topic.title
                    ),
                    _ => {}
                }
            }
        }
        let mut packet = Vec::new();
        wire::write(&mut packet, &book).unwrap();
        assert_eq!(wire::read(&mut packet.as_slice()).unwrap(), book, "{era:?} round trip");
    }
}

#[test]
fn every_topic_can_be_reached_from_contents_the_index_or_a_link() {
    for era in ERAS {
        let book = catalog(era);
        let mut reached: BTreeSet<usize> = book.contents.iter().filter_map(|e| e.topic).collect();
        reached.extend(book.index.iter().map(|k| k.topic));
        reached.insert(book.home);
        for topic in &book.topics {
            for p in &topic.paragraphs {
                match &p.style {
                    Link(t) => {
                        reached.insert(*t);
                    }
                    Related(ts) => reached.extend(ts),
                    _ => {}
                }
            }
        }
        assert_eq!(reached.len(), book.topics.len(), "{era:?}");
    }
}

/// The originals' counts: 95's CALC.HLP has nine titled topics, CALC.CNT two books and seven
/// pages, 118 keywords; 98's calc.chm ten topics (and its welcome and legal pages), two books
/// and ten pages, 55 keywords and 16 under "numbers" and "values"; 2000's twelve topics, one
/// book and twelve pages, 87 index lines of which "calculations" and "memory" open no topic.
#[test]
fn each_look_has_its_originals_topics_contents_and_keywords() {
    for (era, topics, books, pages, keywords) in
        [(Era::Win95, 9, 2, 7, 118), (Era::Win98, 12, 2, 10, 71), (Era::Win2000, 12, 1, 12, 85)]
    {
        let book = catalog(era);
        assert_eq!(book.topics.len(), topics, "{era:?} topics");
        assert_eq!(book.contents.iter().filter(|e| e.topic.is_none()).count(), books, "{era:?} books");
        assert_eq!(book.contents.iter().filter(|e| e.topic.is_some()).count(), pages, "{era:?} pages");
        assert_eq!(distinct_keywords(&book), keywords, "{era:?} keywords");
    }
    let labels = |era| catalog(era).contents.iter().map(|e| (e.depth, e.label.clone())).collect::<Vec<_>>();
    let tree = |rows: &[(usize, &str)]| rows.iter().map(|&(d, l)| (d, l.to_string())).collect::<Vec<_>>();
    assert_eq!(
        labels(Era::Win95),
        tree(&[
            (0, "Performing Calculations"),
            (1, "Performing a simple calculation"),
            (1, "Performing a statistical calculation"),
            (1, "Performing a scientific calculation"),
            (1, "Converting values to other number systems"),
            (0, "Tips and Tricks"),
            (1, "Finding out what a Calculator button does"),
            (1, "Working with numbers stored in memory"),
            (1, "Using keyboard equivalents"),
        ])
    );
    assert_eq!(labels(Era::Win98)[5], (0, "Tips and Tricks".to_string()));
    assert_eq!(labels(Era::Win98)[11], (1, "Transfer numbers between standard and scientific view".to_string()));
    assert_eq!(labels(Era::Win2000)[0], (0, "Calculator".to_string()));
    assert_eq!(labels(Era::Win2000)[12], (1, "Performing calculations with large numbers".to_string()));
    // The opening topics: 95's picker, 98's welcome page, 2000's overview.
    assert_eq!(catalog(Era::Win98).topics[catalog(Era::Win98).home].title, "Welcome to Help");
    assert!(catalog(Era::Win98).topics[catalog(Era::Win98).home].welcome);
    assert_eq!(catalog(Era::Win2000).topics[0].title, "Calculator overview");
    assert_eq!(catalog(Era::Win2000).title, "Calculator");
    assert_eq!(catalog(Era::Win95).title, "Calculator Help");
}

/// FNV-1a of each original article's visible words, read directly from CALC.HLP
/// and the two decompiled CHMs. Ignore whitespace used for layout (including table
/// gutters), but retain case, punctuation, order and every cell in the printed book.
#[test]
fn each_article_matches_its_reference_text() {
    #[rustfmt::skip]
    let originals: [&[u64]; 3] = [
        &[0x9eb2f13df5a8981d, 0x19e5c684e094baac, 0xfebb16bd2bb4ed01, 0x918d6c1d50860432,
          0xe2f2b6a2ea0cf0e2, 0x8c260d9ab5dea6eb, 0xfa72743db0739eff, 0xca3c5ac2b73e8883,
          0x810cf33d61d558c1],
        &[0x1f65cdacb333f2d8, 0x5080f623698b9ca3, 0x48930cf88291e99, 0x0680fdeea6cff7b9,
          0xd7ea7bf3a4d28fe4, 0xb63dc777f2273b4e, 0xa9197692eef182be, 0x296c87c4e0635729,
          0xf054474523d44f2d, 0x5afbd2241f304491, 0x236ecd22d8dbe92b, 0x18f671643593b023],
        &[0xf4dfad171ec9b746, 0xe926d2b7cc5d77b5, 0x2896243bcc354ea4, 0x47577a34916e792,
          0xf54c61384e223968, 0x831962cf0c4b9f7d, 0xd5519d4f5ec7e5f1, 0x641b270eedc4021c,
          0xce5f3ad938623286, 0x657cfccf6bc7fc67, 0xb7500f30a9333bdd, 0x98d9392131b9b8d2],
    ];
    for (era, expected) in ERAS.into_iter().zip(originals) {
        let topics = catalog(era).topics;
        assert_eq!(topics.len(), expected.len());
        for (topic, &expected) in topics.iter().zip(expected) {
            let words = topic.paragraphs.iter().flat_map(|p| p.text.split_whitespace()).collect::<Vec<_>>().join(" ");
            let hash =
                words.bytes().fold(0xcbf29ce484222325u64, |h, byte| (h ^ u64::from(byte)).wrapping_mul(0x100000001b3));
            assert_eq!(hash, expected, "{era:?}: {}", topic.title);
        }
    }
}

/// What a key of the list must do in Bevel95's Calculator, and how it is sent.
enum Sent {
    Char(char),
    Key(Key, Mods),
}

fn sent(key: &str) -> Vec<Sent> {
    let plain = Mods::default();
    match key {
        "0-9" => ('0'..='9').map(Sent::Char).collect(),
        "A-F" => ('a'..='f').map(Sent::Char).collect(),
        ". or ," => vec![Sent::Char('.'), Sent::Char(',')],
        "ENTER" => vec![Sent::Key(Key::Enter, plain)],
        "ESC" => vec![Sent::Key(Key::Escape, plain)],
        "DEL" => vec![Sent::Key(Key::Delete, plain)],
        "INS" => vec![Sent::Key(Key::Other(0x2D), plain)],
        "BACKSP" | "Backspace" | "BACKSPACE" => vec![Sent::Key(Key::Other(0x08), plain)],
        "|(pipe)" => vec![Sent::Char('|')],
        "F4" => vec![Sent::Key(Key::F4, plain)],
        _ if key.starts_with("CTRL+") => {
            vec![Sent::Key(Key::Other(u16::from(key.as_bytes()[5])), Mods { ctrl: true, shift: false })]
        }
        _ if key.starts_with('F') && key.len() > 1 => {
            let n: u16 = key[1..].parse().unwrap();
            vec![Sent::Key(Key::Other(0x70 + n - 1), plain)]
        }
        _ => {
            assert_eq!(key.chars().count(), 1, "a single character: {key}");
            vec![Sent::Char(key.chars().next().unwrap())]
        }
    }
}

/// Every line of each look's keyboard list does what it says in Bevel95's Calculator: the key
/// pushes in the button of that caption (or chooses that option, ticks that box).
#[test]
fn the_keyboard_lists_agree_with_the_calculator() {
    for era in ERAS {
        for &(button, key) in keyboard_list(era) {
            let standard = matches!(button, "sqrt" | "%");
            for (n, how) in sent(key).into_iter().enumerate() {
                let mut w = CalculatorWindow::new(era);
                if !standard {
                    w.command(Command::Scientific);
                }
                let radix = match button {
                    "A-F" | "Qword" | "Dword" | "Word" | "Byte" | "Dec" => Radix::Hex,
                    _ => Radix::Dec,
                };
                w.command(Command::Radix(radix));
                if matches!(button, "Deg" | "Degrees") {
                    w.command(Command::Angle(Angle::Radians));
                }
                // Each option is chosen from another one (2000 starts in Qword, 98 in Dword).
                if matches!(button, "Dword" | "Qword") {
                    w.command(Command::Word(Word::Byte));
                }
                match how {
                    Sent::Char(ch) => {
                        w.typed(ch);
                    }
                    Sent::Key(k, mods) => {
                        w.key(k, mods);
                    }
                }
                let what = format!("{era:?} {button} = {key}");
                let e = &w.engine;
                match button {
                    "Hex" => assert_eq!(e.radix(), Radix::Hex, "{what}"),
                    "Dec" => assert_eq!(e.radix(), Radix::Dec, "{what}"),
                    "Oct" => assert_eq!(e.radix(), Radix::Oct, "{what}"),
                    "Bin" => assert_eq!(e.radix(), Radix::Bin, "{what}"),
                    "Deg" | "Degrees" => assert_eq!(e.angle(), Angle::Degrees, "{what}"),
                    "Rad" | "Radians" => assert_eq!(e.angle(), Angle::Radians, "{what}"),
                    "Grad" | "Gradients" | "Grads" => assert_eq!(e.angle(), Angle::Grads, "{what}"),
                    "Qword" => assert_eq!(e.word(), Word::Qword, "{what}"),
                    "Dword" => assert_eq!(e.word(), Word::Dword, "{what}"),
                    "Word" => assert_eq!(e.word(), Word::Word, "{what}"),
                    "Byte" => assert_eq!(e.word(), Word::Byte, "{what}"),
                    "Inv" => assert!(e.inv_ticked(), "{what}"),
                    "Hyp" => assert!(e.hyp_ticked(), "{what}"),
                    _ => {
                        let caption = match (button, era) {
                            ("0-9", _) => ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"][n],
                            ("A-F", _) => ["A", "B", "C", "D", "E", "F"][n],
                            ("Back", _) => "Backspace",
                            ("PI", _) => "pi",
                            _ => button,
                        };
                        assert_eq!(w.flashing(), Some(caption), "{what}");
                    }
                }
            }
        }
    }
}

/// Windows 98's book retained the Windows 95 labels for several renamed controls.
#[test]
fn the_keyboard_lists_name_every_button() {
    for era in ERAS {
        let listed: BTreeSet<&str> = keyboard_list(era).iter().map(|&(button, _)| button).collect();
        let mut w = CalculatorWindow::new(era);
        let mut captions: BTreeSet<&str> = w.captions().into_iter().collect();
        w.command(Command::Scientific);
        captions.extend(w.captions());
        captions.extend(w.option_captions());
        // The 2000 word sizes show in Hex; the angles in Dec (both are in the list).
        w.command(Command::Radix(Radix::Hex));
        captions.extend(w.option_captions());
        for caption in captions {
            let caption = match (caption, era) {
                ("Backspace", Era::Win95 | Era::Win98) => "Back",
                ("pi", Era::Win95 | Era::Win98) => "PI",
                ("Degrees", Era::Win98) => "Deg",
                ("Radians", Era::Win98) => "Rad",
                ("Gradients", Era::Win98) => "Grad",
                (c, _) if c.len() == 1 && c.chars().all(|ch| ch.is_ascii_digit()) => "0-9",
                (c, _) if c.len() == 1 && ('A'..='F').contains(&c.chars().next().unwrap()) && c != "C" => "A-F",
                (c, _) => c,
            };
            assert!(listed.contains(caption), "{era:?}: {caption} is in the list");
        }
        // C is both the Clear button and a hexadecimal digit.
        assert!(listed.contains("A-F") && listed.contains("C"));
    }
}

#[test]
fn the_pasted_sequences_do_what_the_books_say() {
    use crate::calculator::engine::paste_keys_in;
    assert_eq!(
        paste_keys_in(":c:m:p:q:r", false),
        [CalcKey::MemoryClear, CalcKey::MemoryStore, CalcKey::MemoryAdd, CalcKey::Clear, CalcKey::MemoryRecall]
    );
    assert_eq!(paste_keys_in(":e\\", true), [CalcKey::Exp, CalcKey::Dat]);
    // 2000's Notepad example includes a space between the number and the command.
    for era in ERAS {
        let mut w = CalculatorWindow::new(era);
        w.paste_text("123 :m");
        w.paste_text(":q:r");
        assert_eq!(w.engine.display().trim_end_matches([' ', '.']), "123", "{era:?}");
    }
}

fn shown(w: &CalculatorWindow) -> String {
    w.engine.display().trim().trim_end_matches('.').to_string()
}

#[test]
fn the_calculator_handles_percent_overflow_and_digit_grouping() {
    for era in ERAS {
        // 50 + 25 % shows 12.5, and = gives 62.5.
        let mut w = CalculatorWindow::new(era);
        w.paste_text("50+25%");
        assert_eq!(shown(&w), "12.5", "{era:?} percent");
        w.paste_text("=");
        assert_eq!(shown(&w), "62.5", "{era:?} percent result");
    }
    // 2000's large numbers: FFFF * 2 in Hex with Word shows FFFE.
    let mut w = CalculatorWindow::new(Era::Win2000);
    w.command(Command::Scientific);
    w.command(Command::Radix(Radix::Hex));
    w.command(Command::Word(Word::Word));
    w.paste_text("FFFF*2=");
    assert_eq!(shown(&w), "FFFE");
    // 2000's digit grouping: commas in decimal, a space every four hexadecimal digits.
    let mut w = CalculatorWindow::new(Era::Win2000);
    w.command(Command::DigitGrouping);
    w.paste_text("1234567");
    assert_eq!(shown(&w), "1,234,567");
    w.command(Command::Scientific);
    w.command(Command::Radix(Radix::Hex));
    w.paste_text("12345");
    assert_eq!(shown(&w), "1 2345");
}

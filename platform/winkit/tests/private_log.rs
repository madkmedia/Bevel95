//! No private text in the log (decisions.md, "Milestone 1 decisions"): people attach bevel95.log
//! to their reports. This test reads every log call of the bar, the shared Windows side and the
//! programs, and fails if one is given what was taken out of them on 2026-11-17: a window's
//! title, a notification icon's tooltip, a displayed path, what was typed into Run, a program's,
//! document's or desktop item's name, the places Windows names. A log call is `log(`,
//! `log_win32(` or `report(` with everything up to its closing parenthesis.
//!
//! The one exception is `bevel95 --list-windows` (`window_list::log_all`), which writes window
//! titles because the person runs it on purpose; its first line says so.
//!
//! It reads the sources, not types: a log call is a `&str`, so a title-carrying type cannot be
//! refused by the compiler without wrapping every call; the patterns below are the ones removed.

use std::fs;
use std::path::{Path, PathBuf};

/// What a log call must not contain, with why.
const FORBIDDEN: &[(&str, &str)] = &[
    ("title(hwnd", "a window's title"),
    ("info.title", "a window's title"),
    ("raw_title", "a window's title"),
    (".tip", "a notification icon's tooltip"),
    (".display()", "a path (use log::path_kind)"),
    ("{text}", "what was typed (use log::typed_kind)"),
    ("{path}", "a path (use log::path_kind or log::typed_kind)"),
    ("{name}\\\"", "a file's or folder's name"),
    ("{places:?}", "the names of places, a folder path among them"),
    (".name)", "a program's, document's or network's name"),
    (".name,", "a program's, document's or network's name"),
    ("doc.name", "a document's name"),
    ("{target}", "a program or document"),
    // An icon's or item's id is a file's whole path unless it is one of the look's own icons
    // ("::my-computer"); found 2026-11-21 in the desktop's "opening {id}" lines, which only an
    // `if id.starts_with("::")` before them kept from naming a file (log it by its kind).
    ("{id}", "an item's id, a path (use log::path_kind)"),
    ("{id:", "an item's id, a path (use log::path_kind)"),
    (".id)", "an item's id, a path (use log::path_kind)"),
    (".id,", "an item's id, a path (use log::path_kind)"),
    ("{folder}", "a folder's path (use log::path_kind)"),
    ("{file}", "a file's path or name (use log::path_kind)"),
];

/// The forbidden patterns `call` contains, with why.
fn forbidden_in(call: &str) -> Vec<(&'static str, &'static str)> {
    FORBIDDEN.iter().copied().filter(|(pattern, _)| call.contains(pattern)).collect()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Each log call in `source`: (line number, the call's text up to its closing parenthesis).
fn log_calls(source: &str) -> Vec<(usize, String)> {
    let bytes = source.as_bytes();
    let mut calls = Vec::new();
    for name in ["log(", "log_win32(", "report("] {
        let mut from = 0;
        while let Some(at) = source[from..].find(name) {
            let start = from + at;
            from = start + name.len();
            // A whole name (`log(`, `log::log(`, `crate::log::log(`), not part of another one.
            let before = source[..start].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            // Not a definition.
            if source[..start].trim_end().ends_with("fn") {
                continue;
            }
            let mut depth = 0;
            let mut end = start + name.len() - 1;
            let mut in_string = false;
            let mut i = end;
            while i < bytes.len() {
                let b = bytes[i];
                if in_string {
                    if b == b'\\' {
                        i += 1;
                    } else if b == b'"' {
                        in_string = false;
                    }
                } else if b == b'"' {
                    in_string = true;
                } else if b == b'(' {
                    depth += 1;
                } else if b == b')' {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                i += 1;
            }
            let line = source[..start].lines().count().max(1);
            calls.push((line, source[start..=end.min(bytes.len() - 1)].to_string()));
        }
    }
    calls
}

#[test]
fn log_calls_carry_no_private_text() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for dir in ["platform/win/src", "platform/winkit/src", "programs"] {
        rust_files(&root.join(dir), &mut files);
    }
    assert!(files.len() > 50, "the sources were found ({} files)", files.len());
    let mut found = Vec::new();
    let mut calls = 0;
    for file in &files {
        let source = fs::read_to_string(file).unwrap();
        // `--list-windows` writes titles on purpose: everything from its function on is left out.
        let source = match source.find("pub fn log_all()") {
            Some(at) if file.ends_with("window_list.rs") => &source[..at],
            _ => &source[..],
        };
        for (line, call) in log_calls(source) {
            calls += 1;
            for (pattern, what) in forbidden_in(&call) {
                found.push(format!("{}:{line}: {what} (`{pattern}`) in {call}", file.display()));
            }
        }
    }
    assert!(calls > 300, "the log calls were found ({calls})");
    assert!(found.is_empty(), "log calls with private text:\n{}", found.join("\n"));
}

#[test]
fn the_reader_finds_calls_and_their_arguments() {
    let calls = log_calls("fn log(text: &str) {}\n    log(&format!(\"Run: {text}\", f(x)));\n    catalog(1);\n");
    assert_eq!(calls, [(2, "log(&format!(\"Run: {text}\", f(x)))".to_string())]);
}

/// The lines found on 2026-11-21 would now be refused, however they are written; what replaced
/// them passes.
#[test]
fn an_items_id_is_refused() {
    for line in [
        "        log(&format!(\"desktop: opening {id}\"));\n",
        "        log(&format!(\"desktop: opening {id} in a folder window\"));\n",
        "        log(&format!(\"desktop: opening {id:?}\"));\n",
        "        log(&format!(\"desktop: opening {}\", item.id));\n",
        "        log(&format!(\"desktop: opening {} ({n})\", t.id, n = 1));\n",
    ] {
        let calls = log_calls(line);
        assert_eq!(calls.len(), 1, "{line}");
        assert!(!forbidden_in(&calls[0].1).is_empty(), "not refused: {line}");
    }
    let calls = log_calls("        log(&format!(\"desktop: opening {} in a folder window\", item_kind(id)));\n");
    assert!(forbidden_in(&calls[0].1).is_empty());
}

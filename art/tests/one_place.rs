//! Windows' pictures stay in one place: this crate. Fails when a picture file, a
//! picture written as Rust source or a file of sounds or fonts cut from Windows appears anywhere
//! else in the project; and checks that every picture here can be found and is whole.

use std::fs;
use std::path::{Path, PathBuf};

use bevel_art::{Art, Depth, Look};

/// Extensions of the files Windows' pictures, sounds and fonts are kept in.
const DATA_FILES: &[&str] =
    &["rgb", "rgba", "pic", "idx", "idx4", "bits", "pcm", "palrle", "i4", "glyphs", "bin", "bmp", "ico", "cur", "wav", "gif"];

/// The places allowed to hold such things although they are not in art/: the test pictures that
/// show them (golden images, and README's pictures made from them), the font fixtures, the
/// project's own drawing of its icon.
fn allowed(rel: &str) -> bool {
    rel.starts_with("art/")
        || rel.starts_with("core/tests/golden/")
        || rel.starts_with("core/tests/fixtures/")
        // README's pictures, composed only from the golden images.
        || rel.starts_with("docs/img/")
        // Bevel95's own silver B (drawn for the project, not taken from Windows).
        || rel == "core/src/bevel_icon.rs"
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// Every file of the project (not the build folder, the untracked private/ and tmp/, or git's own).
fn files(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        if p.is_dir() {
            if !matches!(rel.as_str(), "target" | "private" | "tmp" | ".git" | ".claude") && !rel.ends_with("/target") {
                files(&p, root, out);
            }
        } else {
            out.push((rel, p));
        }
    }
}

/// A line that is a row of a picture written as text: a string of at least 5 picture characters,
/// with see-through dots or ink.
fn picture_row(line: &str) -> bool {
    let t = line.trim().trim_end_matches(',');
    let Some(s) = t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else { return false };
    let file_name = s.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.contains('.') && (2..=4).contains(&ext.len()) && ext.bytes().all(|b| b.is_ascii_lowercase())
    });
    s.len() >= 5
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'#')
        && s.contains(['.', '#'])
        && !file_name
}

/// What in a Rust file looks like a picture taken from Windows: rows of a picture (6 or more in
/// a row), a palette (`(b'a', 0x808080)`), the bytes of a data file.
fn picture_source(text: &str) -> Option<String> {
    let mut run = 0;
    for (n, line) in text.lines().enumerate() {
        if line.contains("#[cfg(test)]") {
            break; // tests draw their own small pictures
        }
        run = if picture_row(line) { run + 1 } else { 0 };
        if run >= 6 {
            return Some(format!("line {}: rows of a picture", n + 1));
        }
        let palette = line.find("(b'").is_some_and(|i| line[i..].starts_with("(b'") && line[i + 5..].trim_start().starts_with(", 0x"))
            || line.contains("', rgb(0x");
        if palette {
            return Some(format!("line {}: a palette", n + 1));
        }
        if let Some(i) = line.find("include_bytes!(")
            && !line[i..].contains("art/assets/")
        {
            return Some(format!("line {}: a data file's bytes", n + 1));
        }
    }
    None
}

#[test]
fn windows_pictures_are_only_in_art() {
    let root = repo();
    let mut all = Vec::new();
    files(&root, &root, &mut all);
    assert!(all.len() > 300, "the project's files were found ({})", all.len());
    let mut wrong = Vec::new();
    for (rel, path) in &all {
        if allowed(rel) {
            continue;
        }
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if DATA_FILES.contains(&ext.as_str()) {
            wrong.push(format!("{rel}: a picture, sound or font file"));
        } else if ext == "png" {
            wrong.push(format!("{rel}: a picture (only core/tests/golden keeps PNGs)"));
        } else if ext == "rs"
            && !rel.contains("/tests/")
            && let Some(what) = picture_source(&fs::read_to_string(path).unwrap())
        {
            wrong.push(format!("{rel}: {what}"));
        }
    }
    assert!(wrong.is_empty(), "Windows' pictures belong in art/ (README.md, \"License\"):\n{}", wrong.join("\n"));
}

/// A lookup of a picture that would run while the program runs instead of while it is built: it
/// would put every picture of bevel-art into every program (about 1.6 MB each). The lookups belong
/// in `const` and `static` items, `const { }` blocks and `const fn`s.
fn runtime_lookup(text: &str) -> Option<String> {
    const CALLS: &[&str] = &[
        "art::rows(", "art::rows_at(", "art::bytes(", "art::bytes_at(", "art::exactly(", "art::colors(",
        "art::captions(", "art::places(", "ClassicIcon::art(", "Drawing::art(", "Banner::art(", "bevel_art::get(",
    ];
    let mut in_const_fn = false;
    let mut item_open = false; // a const or static item whose value goes on over more lines
    for (n, line) in text.lines().enumerate() {
        if line.contains("#[cfg(test)]") {
            break;
        }
        let t = line.trim_start();
        if t.contains("fn ") && !t.starts_with("//") {
            in_const_fn = t.contains("const fn ");
        }
        let item = ["const ", "static ", "pub const ", "pub static ", "pub(crate) const ", "pub(crate) static ", "pub(super) const "]
            .iter()
            .any(|p| t.starts_with(p));
        // A closure in a static (`LazyLock::new(|| ...)`) runs while the program runs.
        let closure = line.contains("||") || line.contains("|_|");
        let building = in_const_fn || ((item || item_open) && !closure) || line.contains("const {");
        if CALLS.iter().any(|c| line.contains(c)) && !building {
            return Some(format!("line {}: a lookup while the program runs", n + 1));
        }
        if item {
            item_open = !t.trim_end().ends_with(';');
        } else if item_open && t.trim_end().ends_with(';') {
            item_open = false;
        }
    }
    None
}

#[test]
fn pictures_are_looked_up_while_building() {
    let root = repo();
    let mut all = Vec::new();
    files(&root.join("core/src"), &root, &mut all);
    let wrong: Vec<String> = all
        .iter()
        .filter(|(rel, _)| rel.ends_with(".rs"))
        .filter_map(|(rel, p)| runtime_lookup(&fs::read_to_string(p).unwrap()).map(|w| format!("{rel}: {w}")))
        .collect();
    assert!(wrong.is_empty(), "look pictures up in a const (or `const {{ }}`):\n{}", wrong.join("\n"));
    assert!(runtime_lookup("fn f() {\n    let b = art::bytes(\"x\", Look::Win95);\n}\n").is_some());
    assert!(runtime_lookup("fn f() {\n    let b = const { art::bytes(\"x\", Look::Win95) };\n}\n").is_none());
    assert!(runtime_lookup("const B: &[u8] =\n    art::bytes(\"x\", Look::Win95);\n").is_none());
    assert!(runtime_lookup("static F: LazyLock<Font> =\n    LazyLock::new(|| load(art::bytes(\"x\", Look::Win98)));\n").is_some());
}

#[test]
fn the_guard_sees_pictures() {
    let rows = "    \"..##..\",\n".repeat(6);
    assert!(picture_source(&format!("const X: [&str; 6] = [\n{rows}];\n")).is_some());
    assert!(picture_source("    palette: &[(b'a', 0x808080)],\n").is_some());
    assert!(picture_source("    (b'W', rgb(0xFFFFFF)),\n").is_some());
    assert!(picture_source("const A: &[u8] = include_bytes!(\"../assets/x.rgb\");\n").is_some());
    assert!(picture_source("let s = \"Hello\";\nlet t = \"a.txt\";\n").is_none());
    let names = "    \"settings.txt\",\n".repeat(8);
    assert!(picture_source(&names).is_none(), "file names are no picture");
}

#[test]
fn every_picture_is_found_and_whole() {
    let mut n = 0;
    for e in bevel_art::entries() {
        n += 1;
        assert!(!e.looks.is_empty(), "{}", e.what);
        for &look in e.looks {
            // The first of the same name, look and depth is the one found: none is hidden by another.
            let found = bevel_art::get(e.what, look, e.depth).unwrap();
            assert!(same(found, e.art), "{} ({look:?}, {:?}) is listed twice", e.what, e.depth);
        }
        match e.art {
            Art::Rows(r) => {
                assert_eq!(r.rows.len() as i32, r.h, "{}", e.what);
                assert!(r.rows.iter().all(|row| row.len() as i32 <= r.w), "{}", e.what);
                if !r.palette.is_empty() {
                    assert!(r.rows.iter().all(|row| row.len() as i32 == r.w), "{}", e.what);
                }
            }
            Art::Bytes(b) => assert!(!b.is_empty(), "{}", e.what),
            Art::Text(t) => assert!(!t.is_empty(), "{}", e.what),
            Art::Colors(c) => assert!(!c.is_empty(), "{}", e.what),
            Art::Captions(c) => assert!(!c.is_empty(), "{}", e.what),
            Art::Places(p) => assert!(!p.is_empty(), "{}", e.what),
        }
    }
    assert!(n > 350, "{n} pictures");
    assert!(bevel_art::get("start/programs", Look::Win95, Depth::TrueColor).is_some());
    assert!(bevel_art::get("start/programs", Look::Win95, Depth::Sixteen).is_none());
    assert!(bevel_art::get("no/such picture", Look::Win98, Depth::TrueColor).is_none());
}

fn same(a: Art, b: Art) -> bool {
    match (a, b) {
        (Art::Rows(x), Art::Rows(y)) => std::ptr::eq(x, y),
        (Art::Bytes(x), Art::Bytes(y)) => std::ptr::eq(x, y),
        (Art::Text(x), Art::Text(y)) => std::ptr::eq(x, y),
        (Art::Colors(x), Art::Colors(y)) => std::ptr::eq(x, y),
        (Art::Captions(x), Art::Captions(y)) => std::ptr::eq(x, y),
        (Art::Places(x), Art::Places(y)) => std::ptr::eq(x, y),
        _ => false,
    }
}

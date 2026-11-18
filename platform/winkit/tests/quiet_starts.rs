//! No test, example or build script of Bevel95 shows a window of a program it starts (the owner,
//! 2026-11-18: "Why the many bursts of cmd windows periodically on my system?"): every process
//! they start goes through `bevel_winkit::quiet` (hidden: CREATE_NO_WINDOW). A test of the
//! sources: the test code of every crate (files under `tests` and `examples`, build scripts, and
//! each source file from its `#[cfg(test)]` on) is read for a process started any other way:
//! `Command::new(`, ShellExecute, WinExec, and a CreateProcess whose creation flags do not name
//! `quiet::NO_WINDOW`. The programs themselves are not read here: what they start is theirs.

use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "target") {
                sources(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The part of a source file that is test code: all of it (under `tests`, `examples`, `benches`,
/// a build script), or from the `#[cfg(test)]` of its test module on (`#[cfg(test)]` followed by
/// a `mod` line).
fn test_code(path: &Path, text: &str) -> Option<(usize, String)> {
    let whole = path.components().any(|c| matches!(c.as_os_str().to_str(), Some("tests" | "examples" | "benches")))
        || path.file_name().is_some_and(|n| n == "build.rs");
    if whole {
        return Some((0, text.to_string()));
    }
    let lines: Vec<&str> = text.lines().collect();
    let at = (0..lines.len()).find(|&i| {
        lines[i].trim() == "#[cfg(test)]"
            && lines[i + 1..].iter().map(|l| l.trim()).find(|l| !l.starts_with("#[")).is_some_and(|l| l.contains("mod "))
    })?;
    Some((at, lines[at..].join("\n")))
}

/// `line` without its comment and the text of its string literals (a checker's own examples and
/// a list of names start nothing).
fn code_only(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        match (in_string, c) {
            (true, '\\') => {
                chars.next();
            }
            (true, '"') => {
                in_string = false;
                out.push('"');
            }
            (true, _) => {}
            (false, '"') => {
                in_string = true;
                out.push('"');
            }
            (false, '/') if chars.peek() == Some(&'/') => break,
            (false, _) => out.push(c),
        }
    }
    out
}

/// The places in `code` (starting at line `first`) where a process is started other than
/// through `quiet`: (line number, what starts it).
fn loud_starts(first: usize, code: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    let lines: Vec<&str> = code.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let line = &code_only(line);
        for call in ["Command::new(", "ShellExecuteW(", "ShellExecuteExW(", "ShellExecuteA(", "WinExec("] {
            if line.contains(call) {
                found.push((first + i + 1, call.trim_end_matches('(')));
            }
        }
        for call in ["CreateProcessW(", "CreateProcessA(", "CreateProcessAsUserW("] {
            if let Some(at) = line.find(call) {
                // The whole call, to its closing parenthesis, must name quiet's NO_WINDOW.
                let rest = format!("{}\n{}", &line[at + call.len()..], lines[i + 1..].join("\n"));
                let mut depth = 1;
                let end = rest
                    .char_indices()
                    .find(|&(_, c)| {
                        match c {
                            '(' => depth += 1,
                            ')' => depth -= 1,
                            _ => {}
                        }
                        depth == 0
                    })
                    .map_or(rest.len(), |(e, _)| e);
                if !rest[..end].contains("quiet::NO_WINDOW") {
                    found.push((first + i + 1, call.trim_end_matches('(')));
                }
            }
        }
    }
    found
}

#[test]
fn the_tests_start_every_process_hidden() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut files = Vec::new();
    for dir in ["core", "platform", "programs"] {
        sources(&root.join(dir), &mut files);
    }
    assert!(files.len() > 60, "the sources were found ({})", files.len());
    let helper = root.join("platform").join("winkit").join("src").join("quiet.rs");
    let mut bad = Vec::new();
    let mut read = 0;
    for file in &files {
        if file.ends_with(Path::new("winkit").join("src").join("quiet.rs")) || file == &helper {
            continue;
        }
        let text = std::fs::read_to_string(file).unwrap();
        let Some((first, code)) = test_code(file, &text) else { continue };
        read += 1;
        for (line, by) in loud_starts(first, &code) {
            bad.push(format!("{}:{line}: {by}", file.display()));
        }
    }
    assert!(read > 30, "test code was found ({read} files)");
    assert!(bad.is_empty(), "a process started without bevel_winkit::quiet:\n{}", bad.join("\n"));
}

/// The checker finds each start the tests had before (2026-11-18) and lets the hidden ones pass.
#[test]
fn the_checker_finds_a_start_that_shows_a_window() {
    for code in [
        "        let mut short = std::process::Command::new(&copy).args([\"/c\", \"ping -n 2 127.0.0.1 >nul\"]).spawn().unwrap();",
        "    let out = Command::new(\"git\").arg(\"-C\").arg(dir).args(args).output().ok()?;",
        "        let r = unsafe { ShellExecuteW(null_mut(), null(), file.as_ptr(), null(), null(), SW_SHOWNORMAL) };",
        "        CreateProcessW(app.as_ptr(), line.as_mut_ptr(), null(), null(), 0, CREATE_UNICODE_ENVIRONMENT, env.as_ptr().cast(), null(), &info, &mut result)",
        "                CreateProcessW(\n                    exe.as_ptr(),\n                    1,\n                    0,\n                    null(),\n                )",
    ] {
        assert!(!loud_starts(0, code).is_empty(), "{code}");
    }
    for code in [
        "        let mut short = bevel_winkit::quiet::command(&copy).args([\"/c\", \"ping -n 2 127.0.0.1 >nul\"]).spawn().unwrap();",
        "        CreateProcessW(app.as_ptr(), line.as_mut_ptr(), null(), null(), 0, CREATE_UNICODE_ENVIRONMENT | bevel_winkit::quiet::NO_WINDOW, env.as_ptr().cast(), null(), &info, &mut result)",
        "                CreateProcessW(\n                    exe.as_ptr(),\n                    1,\n                    bevel_winkit::quiet::NO_WINDOW,\n                    null(),\n                )",
        "    // Command::new( in a comment",
        "use windows_sys::Win32::System::Threading::{CreateProcessW, GetCurrentThreadId};",
    ] {
        assert!(loud_starts(0, code).is_empty(), "{code}");
    }
    // Test code: a file under tests, a build script, a source file from its #[cfg(test)] on.
    assert_eq!(test_code(Path::new("x/tests/a.rs"), "a\nb").map(|c| c.0), Some(0));
    assert_eq!(test_code(Path::new("x/build.rs"), "a").map(|c| c.0), Some(0));
    assert_eq!(test_code(Path::new("x/src/a.rs"), "a\nb\n#[cfg(test)]\nmod tests {}").map(|c| c.0), Some(2));
    assert_eq!(test_code(Path::new("x/src/a.rs"), "a\nb"), None);
}

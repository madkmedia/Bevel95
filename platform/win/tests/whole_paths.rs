//! Windows' own programs and panels are started by their whole path in Windows' system folder,
//! never by a bare name that Windows would first look for in the working folder (the person's own
//! folder, where any program may write) or along the search path (the third safety review of
//! 2026-11-18, C1). A test of the sources: every program of Bevel95 is read for a start of a bare
//! name (`launch("control.exe")`, `shell_open("appwiz.cpl"...)`, `Command::new("x.exe")`, a
//! constant naming one, `rundll32 ... appwiz.cpl,Entry`). What the person types into Run or an
//! address bar is not in the sources and keeps Windows' own rules.

use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Whether `literal` (a string's text) names a program or panel without a folder.
fn bare_program(literal: &str) -> bool {
    let lower = literal.to_ascii_lowercase();
    !lower.contains(['\\', '/', ':', '{']) && [".exe", ".cpl", ".msc", ".com", ".bat", ".cmd"].iter().any(|e| lower.ends_with(e)) || {
        // A program started by a name alone, without an extension (`Command::new("cmd")`).
        !lower.is_empty() && lower.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }
}

/// The places on `line` where a bare name is started: (what starts it, the name).
fn bare_starts(line: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for call in ["launch(", "shell_open(", "Command::new(", "ShellExecuteW(", "spawn_program("] {
        let mut rest = line;
        while let Some(at) = rest.find(call) {
            rest = &rest[at + call.len()..];
            let arg = rest.trim_start().trim_start_matches('&');
            if let Some(text) = arg.strip_prefix('"')
                && let Some(end) = text.find('"')
                && bare_program(&text[..end])
            {
                found.push((call.trim_end_matches('(').to_string(), text[..end].to_string()));
            }
        }
    }
    // A constant naming a program or panel without its folder.
    let t = line.trim_start();
    if (t.starts_with("const ") || t.starts_with("pub const ")) && t.contains(": &str = \"") {
        let text = t.split(": &str = \"").nth(1).unwrap_or("");
        let text = &text[..text.find('"').unwrap_or(0)];
        let lower = text.to_ascii_lowercase();
        if !lower.contains(['\\', '/', ':']) && [".exe", ".cpl", ".msc"].iter().any(|e| lower.ends_with(e)) && !own_program(&lower) {
            found.push(("const".into(), text.to_string()));
        }
    }
    // rundll32's "panel,Entry" by a bare name.
    for (i, _) in line.match_indices(".cpl,") {
        let start = line[..i].rfind([' ', '"']).map_or(0, |s| s + 1);
        let name = &line[start..i + 4];
        if !name.contains(['\\', '{']) && !name.is_empty() {
            found.push(("rundll32".into(), name.to_string()));
        }
    }
    found
}

/// Bevel95's own programs, which are found beside the bar by their whole path (`companion_file`).
fn own_program(name: &str) -> bool {
    ["bevel95.exe", "folders.exe", "minefield.exe", "solitaire.exe", "calculator.exe", "bevel-help.exe"].contains(&name)
}

#[test]
fn windows_own_programs_are_started_by_their_whole_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut files = Vec::new();
    for dir in ["platform/win/src", "platform/winkit/src", "programs"] {
        sources(&root.join(dir), &mut files);
    }
    // Test code that starts helpers of its own is not Bevel95 starting Windows' programs.
    files.retain(|f| !f.components().any(|c| c.as_os_str() == "tests"));
    assert!(files.len() > 40, "the sources were found ({})", files.len());
    let mut bad = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for (by, name) in bare_starts(line) {
                bad.push(format!("{}:{}: {by} \"{name}\"", file.display(), n + 1));
            }
        }
    }
    assert!(bad.is_empty(), "started by a bare name:\n{}", bad.join("\n"));
}

/// The checker itself finds what the review found (each of these was in the sources before).
#[test]
fn the_checker_finds_a_bare_name() {
    for line in [
        "        Target::ControlPanel => launch(\"control.exe\"),",
        "    window_list::launch(\"sndvol.exe\");",
        "        Link::AddRemove => shell_open(\"appwiz.cpl\".into()),",
        "pub const CONNECTIONS: &str = \"ncpa.cpl\";",
        "    let mut line = wide(&format!(\"\\\"{rundll32}\\\" appwiz.cpl,NewLinkHere {}\", placeholder.display()));",
        "    std::process::Command::new(\"cmd\").arg(\"/c\");",
    ] {
        assert!(!bare_starts(line).is_empty(), "{line}");
    }
    for line in [
        "        Target::ControlPanel => launch(&bevel_winkit::shell::system_program(\"control.exe\")),",
        "        Target::Printers => launch(\"ms-settings:printers\"),",
        "        Some(StartEntry::Help) => window_list::launch(\"https://support.microsoft.com/windows\"),",
        "    let mut line = wide(&format!(\"\\\"{rundll32}\\\" {appwiz},NewLinkHere {}\", placeholder.display()));",
        "        file: \"minefield.exe\",",
    ] {
        assert!(bare_starts(line).is_empty(), "{line}");
    }
}

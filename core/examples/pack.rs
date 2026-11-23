//! Makes the one file of a release (`tools\release.ps1` runs it; `bevel_core::package` says how
//! it is laid out), or checks one.
//!
//!   cargo run --release -p bevel-core --example pack -- <out.exe> <bevel95.exe> <companion.exe>...
//!   cargo run --release -p bevel-core --example pack -- --check <file.exe>
//!   cargo run --release -p bevel-core --example pack -- --settings <settings.txt>
//!
//! `--settings` writes the settings.txt Setup's Portable choice writes into a new portable folder
//! (Bevel95's defaults, with its mark: `config::DEFAULT_FILE`), for the release's portable zip.

use std::path::Path;
use std::process::ExitCode;

use bevel_core::package;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [check, file] if check == "--check" => check_file(Path::new(file)),
        [settings, out] if settings == "--settings" => {
            std::fs::write(out, bevel_core::config::DEFAULT_FILE).map_err(|e| format!("cannot write {out}: {e}"))
        }
        [out, bar, programs @ ..] if !out.starts_with("--") && !programs.is_empty() => make(Path::new(out), bar, programs),
        _ => Err("usage: pack <out.exe> <bevel95.exe> <companion.exe>... | pack --check <file.exe>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pack: {e}");
            ExitCode::FAILURE
        }
    }
}

fn make(out: &Path, bar: &str, programs: &[String]) -> Result<(), String> {
    let read = |p: &str| std::fs::read(p).map_err(|e| format!("cannot read {p}: {e}"));
    let bar_bytes = read(bar)?;
    if package::is_packed(&bar_bytes) {
        return Err(format!("{bar} is a package already"));
    }
    refuse_test_build(bar, &bar_bytes)?;
    let mut files = Vec::new();
    for p in programs {
        let name = Path::new(p).file_name().and_then(|n| n.to_str()).ok_or(format!("{p} has no file name"))?;
        let bytes = read(p)?;
        refuse_test_build(p, &bytes)?;
        files.push((name.to_ascii_lowercase(), bytes));
    }
    let list: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    let bytes = package::pack(&bar_bytes, &list);
    // Read back through the same code Setup uses: the names are allowed, the fingerprints match.
    package::unpack(&bytes)?.ok_or("the package cannot be read back")?;
    std::fs::write(out, &bytes).map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    check_file(out)
}

/// A program built by `cargo test` (winkit's feature `test-scratch`) keeps its files in a scratch
/// folder: it is never packed (the third safety review of 2026-11-18, suspected 1). Its mark is that
/// folder's name, which only the tests' code holds (put together here, so that this example's own
/// file is the only one with it).
fn refuse_test_build(name: &str, bytes: &[u8]) -> Result<(), String> {
    let mark = ["bevel95", "tests", "localappdata"].join("-");
    if bytes.windows(mark.len()).any(|w| w == mark.as_bytes()) {
        return Err(format!("{name} is a test build (made by cargo test); run cargo build --release and try again"));
    }
    Ok(())
}

fn check_file(file: &Path) -> Result<(), String> {
    let bytes = std::fs::read(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let files = package::unpack(&bytes)?.ok_or(format!("{} is no package", file.display()))?;
    for (entry, _) in &files {
        println!("  {:<26} {:>10} bytes  sha256 {}", entry.name, entry.len, package::hex(&entry.sha256));
    }
    if let Some(stamp) = package::find_stamp(files[0].1) {
        println!("  build stamp: version {} build {} (commit time {})", stamp.version, stamp.build, stamp.time);
    }
    println!("  {} programs, every fingerprint matches", files.len());
    Ok(())
}

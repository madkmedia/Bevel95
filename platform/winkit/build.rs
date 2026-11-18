//! The build Bevel95 reports (About Bevel95, the log's first line): the commit it was built from,
//! asked of git when this crate is built. Without git, or outside a repository, it is "unknown"
//! and the build goes on. The commit's time goes into the build stamp (`STAMP`).

use std::path::{Path, PathBuf};

// Every process started without a window (`src/quiet.rs`, the same file the crate has).
#[path = "src/quiet.rs"]
#[allow(dead_code)]
mod quiet;

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = quiet::command("git").arg("-C").arg(dir).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn main() {
    let dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let build = git(&dir, &["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BEVEL95_BUILD={build}");
    // The commit's time (seconds since 1970) for the build stamp (`STAMP`), 0 without git.
    let time = git(&dir, &["log", "-1", "--format=%ct"]).filter(|t| t.bytes().all(|b| b.is_ascii_digit()));
    println!("cargo:rustc-env=BEVEL95_BUILD_TIME={}", time.unwrap_or_else(|| "0".to_string()));
    // Asked again when the commit changes: HEAD (a branch switch) and the index (every commit
    // writes it). Without a repository only this script's own changes do.
    println!("cargo:rerun-if-changed=build.rs");
    if let Some(git_dir) = git(&dir, &["rev-parse", "--absolute-git-dir"]) {
        let git_dir = PathBuf::from(git_dir);
        for file in ["HEAD", "index"] {
            println!("cargo:rerun-if-changed={}", git_dir.join(file).display());
        }
        if let Some(head) = git(&dir, &["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={}", git_dir.join(head).display());
        }
    }
}

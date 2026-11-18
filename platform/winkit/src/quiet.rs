//! Every process a test, an example or a build script of Bevel95 starts is started through here,
//! hidden: a console program (cmd, icacls, PowerShell, git, a copy of cmd standing for a program)
//! gets a console without a window, so nothing shows on the person's screen and nothing takes
//! the keyboard (the owner, 2026-11-18: "Why the many bursts of cmd windows periodically on my
//! system?": a test binary of a windowed program has no console of its own, so each console
//! program it started opened a new window). A program with windows of its own is started this
//! way too; the test shows them on a private desktop (Help's tests) or is `#[ignore]`d with the
//! reason (`cargo selftest`). `tests/quiet_starts.rs` reads the sources and fails on a process
//! started any other way.
//!
//! Only `std` is used here: the build script includes this file as it is (`#[path]`).

use std::ffi::OsStr;
use std::os::windows::process::CommandExt;
use std::process::Command;

/// CREATE_NO_WINDOW: a console program is given a console without a window. For a
/// CreateProcessW of a test's own, in its creation flags.
pub const NO_WINDOW: u32 = 0x0800_0000;

/// `Command::new(program)`, started without a window.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(NO_WINDOW);
    command
}

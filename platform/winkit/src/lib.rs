//! The Windows side that every Bevel95 program shares (the bar, and each program built on the
//! control kit): the fonts rendered by GDI into `bevel_core` glyph atlases, the window host for
//! `bevel_core::kit` forms, a program's menus, the look the bar is set to, small Win32 helpers
//! and the log. All of it is `unsafe` Win32 glue; what a window looks like and does lives in
//! `bevel-core`.

// Window and drawing handles (HWND, HDC) are typed as raw pointers in windows-sys, but they are
// opaque numbers that Windows checks itself: a stale one makes a call fail, nothing is read
// through it here. So functions taking them are safe to call.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

/// The commit these programs were built from (`build.rs` asks git), "unknown" without git.
pub const BUILD: &str = env!("BEVEL95_BUILD");

/// The build stamp in the program's file (`bevel_core::package::find_stamp` reads it back): the
/// version, the commit and the commit's time, so Bevel95 Setup can tell which of two copies is
/// newer without running either (a downloaded file's date is the day it was downloaded).
/// `log::started` uses it, so it is in every program's file.
pub static STAMP: &str = concat!(
    "\0Bevel95 build stamp ",
    env!("CARGO_PKG_VERSION"),
    " ",
    env!("BEVEL95_BUILD"),
    " ",
    env!("BEVEL95_BUILD_TIME"),
    "\0"
);

pub mod bitlocker;
pub mod clipboard;
pub mod com;
pub mod confirm;
pub mod copy;
pub mod dragdrop;
pub mod file_ops;
pub mod font;
pub mod help;
pub mod icon;
pub mod jobs;
pub mod kit;
pub mod log;
pub mod look;
pub mod menu;
pub mod panic;
pub mod pointer;
pub mod quiet;
pub mod undo;
pub mod shell;
pub mod shortcut_wizard;
pub mod state_file;
pub mod util;

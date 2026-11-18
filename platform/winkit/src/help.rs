//! Opens the shared Help program with a book supplied by the caller. Keep the viewer in
//! the calling window's state. Closing it (or exiting the caller) closes its pipe, which
//! tells Help to release any modal owner and exit.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use bevel_core::help::{Catalog, wire};
use bevel_core::theme::Era;
use windows_sys::Win32::Data::HtmlHelp::{HH_DISPLAY_TEXT_POPUP, HH_POPUP, HtmlHelpW};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, GetCursorPos};
#[path = "help_owner.rs"]
mod help_owner;
pub use help_owner::OwnerNotifications;

/// Display the What's This? text supplied by a dialog for the control under the pointer.
pub fn context_help(owner: HWND, text: &str) {
    let text = crate::util::wide(text);
    let mut pt = POINT::default();
    // SAFETY: the SDK declares pszText as LPCTSTR. HtmlHelpW takes a terminated UTF-16
    // string; windows-sys exposes the same pointer slot with the ANSI pointer type.
    unsafe {
        GetCursorPos(&mut pt);
        let popup = HH_POPUP {
            cbStruct: size_of::<HH_POPUP>() as i32,
            pszText: text.as_ptr().cast_mut().cast(),
            pt,
            clrForeground: u32::MAX,
            clrBackground: u32::MAX,
            rcMargins: RECT { left: -1, top: -1, right: -1, bottom: -1 },
            ..Default::default()
        };
        if HtmlHelpW(owner, std::ptr::null(), HH_DISPLAY_TEXT_POPUP as u32, &raw const popup as usize).is_null() {
            crate::log::log("Windows could not display the context help popup");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Contents,
    Index,
    Search,
    UsingHelp,
    Topic(usize),
}

impl Target {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "contents" => Some(Self::Contents),
            "index" => Some(Self::Index),
            "search" => Some(Self::Search),
            "using" => Some(Self::UsingHelp),
            _ => value.strip_prefix("topic-")?.parse().ok().map(Self::Topic),
        }
    }

    fn argument(self) -> String {
        match self {
            Self::Contents => "contents".into(),
            Self::Index => "index".into(),
            Self::Search => "search".into(),
            Self::UsingHelp => "using".into(),
            Self::Topic(index) => format!("topic-{index}"),
        }
    }

    /// After the initial catalog, a caller can navigate again by sending one little-endian word.
    fn code(self) -> u32 {
        match self {
            Self::Contents => 0,
            Self::Index => 1,
            Self::Search => 2,
            Self::UsingHelp => u32::MAX,
            Self::Topic(index) => 3 + index as u32,
        }
    }

    pub fn from_code(code: u32) -> Self {
        match code {
            0 => Self::Contents,
            1 => Self::Index,
            2 => Self::Search,
            u32::MAX => Self::UsingHelp,
            n => Self::Topic((n - 3) as usize),
        }
    }
}

#[derive(Default)]
pub struct HelpViewer {
    running: Option<Running>,
}

struct Running {
    _owner_notifications: Option<OwnerNotifications>,
    child: Child,
    book: Catalog,
    era: Era,
    scale: i32,
    owner: usize,
}

impl HelpViewer {
    /// `owner` is the caller's window, or null for an independent Help window.
    pub fn show(&mut self, owner: HWND, era: Era, scale: i32, book: Catalog, target: Target) -> io::Result<()> {
        if matches!(target, Target::Topic(i) if i >= book.topics.len() || i >= (u32::MAX - 3) as usize) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Help topic is outside the book"));
        }
        if !(1..=8).contains(&scale) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Help scale must be between 1 and 8"));
        }
        if let Some(running) = self.running.as_mut()
            && running.owner == owner as usize
            && running.era == era
            && running.scale == scale
            && running.book == book
            && running.child.try_wait()?.is_none()
        {
            // SAFETY: hand this foreground client's right to activate a window to its own helper.
            unsafe { AllowSetForegroundWindow(running.child.id()) };
            if let Some(stdin) = running.child.stdin.as_mut()
                && stdin.write_all(&target.code().to_le_bytes()).is_ok()
            {
                return Ok(());
            }
        }
        let exe = viewer_path()?;
        let modal_owner = era == Era::Win95 && !owner.is_null();
        let mut child = Command::new(&exe)
            .args([
                "--era",
                match era {
                    Era::Win95 => "95",
                    Era::Win98 => "98",
                    Era::Win2000 => "2000",
                },
            ])
            .args(["--scale", &scale.to_string(), "--open", &target.argument()])
            .args(["--owner", &(owner as usize).to_string(), "--parent", &std::process::id().to_string(), "--pipe"])
            // A portable copy's Help is told where its data is (`log::flag_for`).
            .args(crate::log::flag_for(&exe))
            .stdin(Stdio::piped())
            .stdout(if modal_owner { Stdio::piped() } else { Stdio::null() })
            .spawn()?;
        // SAFETY: this is the child just started for the user's Help command.
        unsafe { AllowSetForegroundWindow(child.id()) };
        let notifications = child.stdout.take().map(|pipe| OwnerNotifications::listen(owner, pipe)).transpose()?;
        let stdin = child.stdin.as_mut().expect("spawned with a piped stdin");
        wire::write(stdin, &book)?;
        self.running =
            Some(Running { _owner_notifications: notifications, child, book, era, scale, owner: owner as usize });
        Ok(())
    }

    pub fn close(&mut self) {
        self.running.take();
    }
}

/// Installed apps live beside the viewer in programs/. The bar lives one directory above
/// it. Cargo builds put all the programs together.
fn viewer_path() -> io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    let dir = current.parent().ok_or_else(|| io::Error::other("the caller has no program directory"))?;
    let nested = dir.join("programs/bevel-help.exe");
    let sibling = dir.join("bevel-help.exe");
    if nested.is_file() {
        Ok(nested)
    } else if sibling.is_file() {
        Ok(sibling)
    } else {
        Err(io::Error::new(io::ErrorKind::NotFound, "The Bevel95 Help program (bevel-help.exe) is missing."))
    }
}

//! The viewer's clipboard command and its caption-icon menu.

use std::ptr::null_mut;
use std::{
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    process::{Command, Stdio},
};

use bevel_core::paint::MenuStyle;
use bevel_core::theme::Era;
use bevel_core::window_menu::{self, WindowCommand, WindowState};
use bevel_winkit::kit::Look;
use bevel_winkit::log::{log, log_win32};
use bevel_winkit::menu::{self, Outcome, Request};
use bevel_winkit::util::{point, wide};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::UI::Controls::{EM_SETSEL, WC_EDITW};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetDoubleClickTime, ReleaseCapture};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, ES_AUTOHSCROLL, ES_MULTILINE, ES_READONLY, GetMessageTime, GetWindowRect,
    GetWindowTextLengthW, IsIconic, IsZoomed, PostMessageW, SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE, SC_MOVE, SC_RESTORE,
    SC_SIZE, SendMessageW, WM_COPY, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_SYSCOMMAND, WS_CHILD,
};

/// The Windows 2000 HTML Help Options menu opens the system Internet Properties panel.
pub fn internet_options() {
    let mut buffer = vec![0; 32768];
    // SAFETY: a writable buffer; Windows reports the path length excluding the terminator.
    let n = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if n == 0 || n >= buffer.len() {
        log_win32("GetSystemDirectoryW(Internet Options)");
        return;
    }
    let system = PathBuf::from(OsString::from_wide(&buffer[..n]));
    if let Err(error) = Command::new(system.join("control.exe"))
        .arg(system.join("inetcpl.cpl"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
    {
        log(&format!("Internet Options could not open: {error}"));
    }
}

/// Let the standard Unicode edit control perform its Copy command. It handles clipboard
/// ownership and formats itself. Fully prepare and check the text before asking it to copy.
pub fn copy(owner: HWND, text: &str) {
    if owner.is_null() || text.is_empty() {
        return;
    }
    let content = wide(&text.replace("\r\n", "\n").replace(['\r', '\n'], "\r\n"));
    // SAFETY: this private, never-shown child uses the standard Edit class. The UTF-16 buffer
    // stays valid through creation. Its synchronous Copy completes before the child is freed.
    unsafe {
        let edit = CreateWindowExW(
            0,
            WC_EDITW,
            content.as_ptr(),
            WS_CHILD | (ES_READONLY | ES_MULTILINE | ES_AUTOHSCROLL) as u32,
            0,
            0,
            1,
            1,
            owner,
            null_mut(),
            null_mut(),
            null_mut(),
        );
        if edit.is_null() {
            log_win32("Windows Help: creating the clipboard edit control");
            return;
        }
        if GetWindowTextLengthW(edit) as usize == content.len() - 1 {
            SendMessageW(edit, EM_SETSEL, 0, -1);
            let before = GetClipboardSequenceNumber();
            SendMessageW(edit, WM_COPY, 0, 0);
            if GetClipboardSequenceNumber() == before {
                log("Windows Help: the text could not be copied to the clipboard");
            }
        } else {
            log("Windows Help: the complete text could not be prepared for copying");
        }
        DestroyWindow(edit);
    }
}

pub fn window_menu(owner: HWND, look: &Look, era: Era) {
    let scale = look.scale.max(1);
    let mut bounds = RECT::default();
    // SAFETY: the caller's own viewer window, a valid rectangle, and message-time queries.
    let (opened, state) = unsafe {
        if GetWindowRect(owner, &mut bounds) == 0 {
            log_win32("GetWindowRect (Help caption menu)");
            return;
        }
        ReleaseCapture();
        (
            GetMessageTime() as u32,
            WindowState {
                minimized: IsIconic(owner) != 0,
                maximized: IsZoomed(owner) != 0,
                resizable: true,
                has_minimize_box: true,
                has_maximize_box: true,
                closable: true,
            },
        )
    };
    let mut popup = window_menu::program_menu(era, state);
    popup.show_keys = era != Era::Win2000;
    // The viewer's 4px sizing frame surrounds its 18px caption.
    let at = (bounds.left + 4 * scale, bounds.top + 22 * scale);
    let ended =
        menu::track(look, Request { owner, menu: popup, style: MenuStyle::Popup, at, above: None, titles: &[] });
    let command = match ended.outcome {
        Outcome::Chosen(i) => window_menu::command_at(i),
        _ => ended.pending.and_then(|msg| {
            let (x, y) = point(msg.lParam);
            let icon = msg.hwnd == owner
                && matches!(msg.message, WM_LBUTTONDOWN | WM_LBUTTONDBLCLK)
                && (6 * scale..22 * scale).contains(&x)
                && (5 * scale..21 * scale).contains(&y);
            if icon {
                // SAFETY: read the system's double-click interval.
                (msg.time.wrapping_sub(opened) <= unsafe { GetDoubleClickTime() }).then_some(WindowCommand::Close)
            } else {
                menu::dispatch(&msg);
                None
            }
        }),
    };
    let Some(command) = command else { return };
    let code = match command {
        WindowCommand::Restore => SC_RESTORE,
        WindowCommand::Move => SC_MOVE,
        WindowCommand::Size => SC_SIZE,
        WindowCommand::Minimize => SC_MINIMIZE,
        WindowCommand::Maximize => SC_MAXIMIZE,
        WindowCommand::Close => SC_CLOSE,
    };
    // SAFETY: this posts a standard system command to the viewer that opened the menu.
    unsafe { PostMessageW(owner, WM_SYSCOMMAND, code as usize, 0) };
}

//! The Start menu's dialogs as windows: Run, Shut Down Windows and (Windows 98 and 2000) Log
//! Off Windows. `bevel_core::dialog` draws them and handles their keys and mouse
//! (`DialogWindow`, a window of the control kit); this module carries out the choice.
//!
//! Shut Down first dims the whole screen with a checkerboard of black dots, as Windows 95,
//! 98 and 2000 did; the dimming goes away if you cancel.

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::{null, null_mut};

use bevel_core::dialog::{self, Dialog, DialogKind, DialogWindow, Part, ShutDownChoice};
use bevel_core::fb::{Framebuffer, Rect};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, LRESULT, LUID, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, EndPaint, GdiFlush, GetDC, PAINTSTRUCT, ReleaseDC, SRCCOPY, SelectObject,
};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Power::SetSuspendState;
use windows_sys::Win32::System::Shutdown::{EWX_LOGOFF, ExitWindowsEx, SHTDN_REASON_FLAG_PLANNED};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OPENFILENAMEW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, MB_OK, SW_HIDE, SW_SHOWNA, ShowWindow, WM_ERASEBKGND, WM_PAINT,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::bar::with_app;
use crate::kit::{self, Hosted};
use crate::log::{data_dir, log, log_win32};
use crate::util::{blit, rect_h, rect_w, wide};
use crate::window_list::launch;

pub const DIM_CLASS: &str = "Bevel95Dim";
/// How many Run commands are remembered (Up and Down in the Run box go through them).
const HISTORY_LEN: usize = 26;

/// The open dialog (one at a time).
pub struct DialogHost {
    form: DialogWindow,
}

thread_local! {
    /// Shut Down's dimming: the window over the screen and the picture it shows.
    static DIM: RefCell<Option<(HWND, Framebuffer)>> = const { RefCell::new(None) };
}

impl Hosted for DialogHost {
    type F = DialogWindow;
    fn form(&self) -> &DialogWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut DialogWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        activate(part);
    }
    fn closed(&mut self) {
        if let Some((dim, _)) = DIM.with(|d| d.borrow_mut().take()) {
            // SAFETY: our own window.
            unsafe { DestroyWindow(dim) };
        }
    }
}

/// Opens a dialog (closing one that is already open).
pub fn open(kind: DialogKind) {
    close();
    let Some((era, monitor, bar_top)) = with_app(|app| (app.era, app.monitor, app.bar_top_on_screen())) else {
        return;
    };
    let scale = with_app(|app| app.scale).unwrap_or_default();
    let screen = Rect::new(monitor.left, monitor.top, rect_w(&monitor), rect_h(&monitor));
    if kind == DialogKind::ShutDown
        && let Some(dim) = dim_screen(screen, scale)
    {
        DIM.with(|d| *d.borrow_mut() = Some(dim));
    }
    let history = if kind == DialogKind::Run { history() } else { Vec::new() };
    let mut form = DialogWindow::new(Dialog::new(kind, era), history);
    // Run's list opens above the field where it would reach past the screen's bottom edge
    // (Windows' rule, measured on the reference PCs: the screen's edge, not the taskbar's top).
    form.screen_bottom = scale.old(monitor.bottom);
    let opened = kit::open(DialogHost { form }, |w, h| match kind {
        // PROVISIONAL: Run opens in the lower left, above the Start button.
        DialogKind::Run => (screen.x + scale.len(12), bar_top - h - scale.len(12)),
        // Shut Down and Log Off: in the middle of the screen (Toasty Tech's screenshot).
        _ => (screen.x + (screen.w - w) / 2, screen.y + (screen.h - h) / 2),
    });
    if opened.is_none()
        && let Some((dim, _)) = DIM.with(|d| d.borrow_mut().take())
    {
        // SAFETY: our own window.
        unsafe { DestroyWindow(dim) };
    }
}

/// Closes the open dialog (and the dimming).
pub fn close() {
    kit::close::<DialogHost>();
}

/// A copy of the screen with every other pixel (counted in 1× pixels) black, shown over
/// everything, as Windows dimmed the screen behind Shut Down.
fn dim_screen(screen: Rect, scale: bevel_core::scale::Scale) -> Option<(HWND, Framebuffer)> {
    let mut shot = capture(screen)?;
    let s = scale;
    for y in 0..shot.height {
        for x in 0..shot.width {
            if (s.old(x) + s.old(y)) % 2 == 1 {
                shot.pixels[(y * shot.width + x) as usize] = 0xFF00_0000;
            }
        }
    }
    let class = wide(DIM_CLASS);
    // SAFETY: a registered class; never takes the focus.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            class.as_ptr(),
            null(),
            WS_POPUP,
            screen.x,
            screen.y,
            screen.w,
            screen.h,
            null_mut(),
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        )
    };
    if hwnd.is_null() {
        log_win32("CreateWindowExW(dim)");
        return None;
    }
    // SAFETY: our own window.
    unsafe { ShowWindow(hwnd, SW_SHOWNA) };
    Some((hwnd, shot))
}
/// The screen's pixels, read through GDI.
fn capture(r: Rect) -> Option<Framebuffer> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: r.w,
            biHeight: -r.h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: a private DIB the size of the screen, released before returning; the pixels
    // are copied out while it exists.
    unsafe {
        let screen = GetDC(null_mut());
        let dc = CreateCompatibleDC(screen);
        let mut bits: *mut c_void = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        let out = if bitmap.is_null() || bits.is_null() {
            None
        } else {
            let old = SelectObject(dc, bitmap);
            let ok = BitBlt(dc, 0, 0, r.w, r.h, screen, r.x, r.y, SRCCOPY) != 0;
            GdiFlush();
            let pixels = std::slice::from_raw_parts(bits.cast::<u32>(), (r.w * r.h) as usize)
                .iter()
                .map(|p| p | 0xFF00_0000)
                .collect();
            SelectObject(dc, old);
            ok.then_some(Framebuffer { width: r.w, height: r.h, pixels })
        };
        if !bitmap.is_null() {
            DeleteObject(bitmap);
        }
        DeleteDC(dc);
        ReleaseDC(null_mut(), screen);
        out
    }
}

/// Carries out a button (or the title bar's close button).
fn activate(part: Part) {
    let Some(d) = kit::with::<DialogHost, _>(|h| h.form.dialog.clone()) else {
        return;
    };
    if !d.enabled(part) {
        return;
    }
    match (d.kind, part) {
        (_, Part::TitleClose | Part::Cancel | Part::No) => close(),
        (_, Part::TitleHelp | Part::Help) => launch("https://support.microsoft.com/windows"),
        (DialogKind::Run, Part::Ok) => {
            if run_command(&d.text) {
                remember(&d.text);
                close();
            } else {
                // SAFETY: a plain beep.
                unsafe { MessageBeep(MB_OK) };
            }
        }
        (DialogKind::Run, Part::Browse) => browse(),
        (DialogKind::ShutDown, Part::Ok | Part::Yes) => {
            let choice = dialog::shut_down_choices(d.era).get(d.choice).map(|c| c.0);
            close();
            if let Some(choice) = choice {
                shut_down(choice);
            }
        }
        (DialogKind::LogOff, Part::Yes) => {
            close();
            shut_down(ShutDownChoice::LogOff);
        }
        _ => {}
    }
}
/// Shuts down, restarts, logs off or sleeps. (Windows' own shutdown.exe does the first two,
/// so Windows asks programs with unsaved work as usual.)
fn shut_down(choice: ShutDownChoice) {
    log(&format!("shut down dialog: {choice:?}"));
    let shutdown = |args: &str| {
        // Windows' own, by its whole path: never a file of that name in the working folder.
        let system = std::env::var_os("SystemRoot").map_or_else(|| std::path::PathBuf::from("C:\\Windows"), std::path::PathBuf::from);
        let exe = wide(&system.join("System32\\shutdown.exe").to_string_lossy());
        let args = wide(args);
        let verb = wide("open");
        // SAFETY: valid null-terminated strings.
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                null_mut(),
                verb.as_ptr(),
                exe.as_ptr(),
                args.as_ptr(),
                null(),
                SW_HIDE,
            )
        };
        if result as usize <= 32 {
            log(&format!("shutdown.exe could not start (ShellExecute error {})", result as usize));
        }
    };
    match choice {
        ShutDownChoice::ShutDown => shutdown("/s /t 0"),
        ShutDownChoice::Restart | ShutDownChoice::RestartDos => shutdown("/r /t 0"),
        // SAFETY: plain Win32 calls.
        ShutDownChoice::LogOff => unsafe {
            if ExitWindowsEx(EWX_LOGOFF, SHTDN_REASON_FLAG_PLANNED) == 0 {
                log_win32("ExitWindowsEx(log off)");
            }
        },
        // SAFETY: plain Win32 call.
        ShutDownChoice::StandBy => unsafe {
            enable_shutdown_privilege();
            if !SetSuspendState(false, false, false) {
                log_win32("SetSuspendState (stand by)");
            }
        },
    }
}

/// Switches on the permission to shut down for this process. Every user has it, but switched
/// off, and SetSuspendState refuses to put the PC to sleep without it (Microsoft's docs).
fn enable_shutdown_privilege() {
    let mut token: HANDLE = null_mut();
    // SAFETY: valid out-pointers; the token handle is closed below.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            log_win32("OpenProcessToken (stand by)");
            return;
        }
        let mut privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES { Luid: LUID { LowPart: 0, HighPart: 0 }, Attributes: SE_PRIVILEGE_ENABLED }],
        };
        if LookupPrivilegeValueW(null(), SE_SHUTDOWN_NAME, &mut privileges.Privileges[0].Luid) == 0 {
            log_win32("LookupPrivilegeValueW (stand by)");
        } else if AdjustTokenPrivileges(token, 0, &privileges, 0, null_mut(), null_mut()) == 0 {
            log_win32("AdjustTokenPrivileges (stand by)");
        }
        CloseHandle(token);
    }
}

/// Starts what was typed in Run: a program (with arguments), a document, a folder or an
/// address, as Windows' Run box does. Returns false if Windows could not start it.
fn run_command(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    // A relative name means one in the person's own folder, as in Windows' Run (which starts
    // programs there), then the PATH and the registered programs (ShellExecute); never one in
    // Bevel95's own folder (the second safety review of 2026-11-18, C2).
    let home = bevel_winkit::shell::profile_folder().unwrap_or_else(std::env::temp_dir);
    let exists = |p: &str| {
        let w = wide(&in_folder(&home, p).to_string_lossy());
        // SAFETY: a null-terminated path.
        unsafe { windows_sys::Win32::Storage::FileSystem::GetFileAttributesW(w.as_ptr()) != u32::MAX }
    };
    // "program" args, quoted; else the longest existing prefix before a space; else the first word.
    let (program, args) = if let Some(rest) = text.strip_prefix('"') {
        match rest.split_once('"') {
            Some((p, a)) => (p.to_string(), a.trim().to_string()),
            None => (rest.to_string(), String::new()),
        }
    } else if exists(text) || !text.contains(' ') {
        (text.to_string(), String::new())
    } else {
        let split = text.match_indices(' ').map(|(i, _)| i).rev().find(|&i| exists(&text[..i]));
        let i = split.unwrap_or_else(|| text.find(' ').unwrap_or(text.len()));
        (text[..i].to_string(), text[i..].trim().to_string())
    };
    // Found in the person's folder: handed over whole. Otherwise as typed (the PATH, App Paths).
    let program = if exists(&program) { in_folder(&home, &program).to_string_lossy().into_owned() } else { program };
    // One of a portable copy's own programs, typed by its path, is told where its data is.
    let args = match crate::log::flag_for(std::path::Path::new(&program)) {
        Some(flag) if args.is_empty() => flag.to_string(),
        Some(flag) => format!("{args} {flag}"),
        None => args,
    };
    let (verb, file, params, folder) = (wide("open"), wide(&program), wide(&args), wide(&home.to_string_lossy()));
    let params_ptr = if args.is_empty() { null() } else { params.as_ptr() };
    // SAFETY: valid null-terminated strings.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params_ptr,
            folder.as_ptr(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    if result as usize <= 32 {
        log(&format!("Run: could not start {} (ShellExecute error {})", crate::log::typed_kind(text), result as usize));
        return false;
    }
    log(&format!("Run: started {}", crate::log::typed_kind(text)));
    true
}

/// `name` in `folder` when it is relative (no drive, not starting with a backslash); a whole path,
/// a web address or a Windows place (shell:, ::) as it is.
fn in_folder(folder: &std::path::Path, name: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(name);
    let whole = p.is_absolute() || name.starts_with(['\\', '/']) || name.get(1..2) == Some(":") || name.contains("://");
    if whole || name.starts_with("shell:") || name.starts_with("::") { p.to_path_buf() } else { folder.join(name) }
}

fn history_path() -> std::path::PathBuf {
    data_dir().join("run-history.txt")
}

fn history() -> Vec<String> {
    bevel_winkit::state_file::read(&history_path()).text().unwrap_or_default().lines().map(str::to_string).collect()
}

/// The command goes first in the Run history (read again now; a history that cannot be read is
/// kept as it is and not saved over).
fn remember(command: &str) {
    let changed = bevel_winkit::state_file::change(&history_path(), "", |text| {
        let mut list: Vec<&str> = text.lines().collect();
        list.retain(|c| *c != command);
        list.insert(0, command);
        list.truncate(HISTORY_LEN);
        list.join("\n")
    });
    if let Err(e) = changed {
        log(&format!("could not save the Run history: {e}"));
    }
}


/// Browse...: Windows' file picker; the chosen file goes into the Run box.
fn browse() {
    let Some(owner) = kit::hwnd::<DialogHost>() else {
        return;
    };
    let filter: Vec<u16> = "Programs\0*.exe;*.com;*.bat;*.lnk\0All Files\0*.*\0\0".encode_utf16().collect();
    let mut file = [0u16; 1024];
    let mut ofn = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: filter.as_ptr(),
        lpstrFile: file.as_mut_ptr(),
        nMaxFile: file.len() as u32,
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_HIDEREADONLY,
        ..Default::default()
    };
    // SAFETY: a fully initialized struct whose buffers outlive the call.
    if unsafe { GetOpenFileNameW(&mut ofn) } == 0 {
        return;
    }
    let len = file.iter().position(|&c| c == 0).unwrap_or(0);
    let path = String::from_utf16_lossy(&file[..len]);
    let text = if path.contains(' ') { format!("\"{path}\"") } else { path };
    kit::update::<DialogHost>(|h| {
        let d = &mut h.form.dialog;
        d.caret = text.chars().count();
        d.anchor = None;
        d.text = text;
        d.focus = Part::Command;
    });
}

/// The dimming window: only paints.
pub unsafe extern "system" fn dim_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            // SAFETY: paired BeginPaint/EndPaint on our own window.
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            DIM.with(|d| {
                if let Some((dim, shot)) = d.borrow().as_ref()
                    && *dim == hwnd
                {
                    blit(hdc, shot, shot.width);
                }
            });
            // SAFETY: see above.
            unsafe { EndPaint(hwnd, &ps) };
            0
        }
        // SAFETY: default handling.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What Run types relative means the person's own folder; whole paths, addresses and
    /// Windows' places stay as typed (the second safety review of 2026-11-18, C2).
    #[test]
    fn run_finds_a_relative_name_in_the_persons_folder() {
        let home = std::path::Path::new("C:\\Users\\someone");
        assert_eq!(in_folder(home, "Documents"), home.join("Documents"));
        assert_eq!(in_folder(home, "notes\\a.txt"), home.join("notes\\a.txt"));
        for whole in ["D:\\Tools\\x.exe", "\\\\server\\share", "\\Windows", "C:x.txt", "https://example.com", "shell:startup", "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}"] {
            assert_eq!(in_folder(home, whole), std::path::PathBuf::from(whole), "{whole}");
        }
    }
}
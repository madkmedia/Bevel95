//! Where a copy keeps its data, decided once at a program's start (the fourth safety review of
//! 2026-11-18, B1 and C2), proven on a real program of the copy: this crate's `bevel-help.exe`,
//! copied into a scratch portable copy, started with and without the flag while the copy's
//! settings.txt is held open by another program and while it is free. Its windows (the message
//! of a start that cannot know) show on a private desktop, never on the person's.
#![cfg(windows)]

use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_ALL, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::StationsAndDesktops::{CloseDesktop, CreateDesktopW, GetThreadDesktop, SetThreadDesktop};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, GetCurrentThreadId, GetExitCodeProcess, PROCESS_INFORMATION, STARTUPINFOW, TerminateProcess,
    WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetWindowThreadProcessId, PostMessageW, WM_CLOSE};

const EXE: &str = env!("CARGO_BIN_EXE_bevel-help");

fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}

/// A scratch portable copy: bevel95.exe (a stand-in), programs\bevel-help.exe, settings.txt of
/// Bevel95's own, and a folder standing in for %LOCALAPPDATA%.
struct Copy {
    root: PathBuf,
}

impl Copy {
    fn new(name: &str) -> Copy {
        let root = std::env::temp_dir().join(format!("bevel95-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("programs")).unwrap();
        fs::create_dir_all(root.join("appdata")).unwrap();
        fs::write(root.join("bevel95.exe"), []).unwrap();
        fs::copy(EXE, root.join("programs").join("bevel-help.exe")).unwrap();
        fs::write(root.join("settings.txt"), bevel_core::config::DEFAULT_FILE).unwrap();
        Copy { root }
    }

    fn settings(&self) -> PathBuf {
        self.root.join("settings.txt")
    }

    fn portable_log(&self) -> PathBuf {
        self.root.join("bevel95.log")
    }

    fn installed(&self) -> PathBuf {
        self.root.join("appdata").join("Bevel95")
    }

    /// Everything in the copy's folder and the stand-in %LOCALAPPDATA% (names only).
    fn listing(&self) -> Vec<String> {
        let mut names = Vec::new();
        for dir in [self.root.clone(), self.root.join("programs"), self.root.join("appdata")] {
            for e in fs::read_dir(dir).unwrap().flatten() {
                names.push(e.path().strip_prefix(&self.root).unwrap().to_string_lossy().into_owned());
            }
        }
        names.sort();
        names
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A private desktop this thread is attached to while it lives.
struct Desktop {
    name: Vec<u16>,
    handle: windows_sys::Win32::System::StationsAndDesktops::HDESK,
    original: windows_sys::Win32::System::StationsAndDesktops::HDESK,
}

impl Desktop {
    fn new(tag: &str) -> Desktop {
        let name = wide(format!("Bevel95HomeTest-{tag}-{}", std::process::id()));
        // SAFETY: a terminated name, the default device, mode and security.
        unsafe {
            let original = GetThreadDesktop(GetCurrentThreadId());
            let handle = CreateDesktopW(name.as_ptr(), null(), null(), 0, GENERIC_ALL, null());
            assert!(!handle.is_null(), "create a private desktop");
            assert_ne!(SetThreadDesktop(handle), 0);
            Desktop { name, handle, original }
        }
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        // SAFETY: this thread made no window on it.
        unsafe {
            SetThreadDesktop(self.original);
            CloseDesktop(self.handle);
        }
    }
}

/// The copy's Help started on the private desktop with `args`, %LOCALAPPDATA% the copy's stand-in.
fn start(desktop: &mut Desktop, copy: &Copy, args: &str) -> (HANDLE, u32) {
    let exe = copy.root.join("programs").join("bevel-help.exe");
    let mut line = wide(format!("\"{}\" {args}", exe.display()));
    let app = wide(&exe);
    // The child's environment: this one with LOCALAPPDATA pointed at the stand-in.
    let mut env: Vec<u16> = Vec::new();
    for (k, v) in std::env::vars_os() {
        if k.eq_ignore_ascii_case("LOCALAPPDATA") {
            continue;
        }
        env.extend(k.encode_wide().chain("=".encode_utf16()).chain(v.encode_wide()).chain(Some(0)));
    }
    env.extend("LOCALAPPDATA=".encode_utf16().chain(copy.root.join("appdata").as_os_str().encode_wide()).chain(Some(0)));
    env.push(0);
    let info = STARTUPINFOW { cb: size_of::<STARTUPINFOW>() as u32, lpDesktop: desktop.name.as_mut_ptr(), ..Default::default() };
    let mut result = PROCESS_INFORMATION::default();
    const CREATE_UNICODE_ENVIRONMENT: u32 = 0x400;
    // SAFETY: every string and the environment block live through the call.
    let ok = unsafe {
        CreateProcessW(app.as_ptr(), line.as_mut_ptr(), null(), null(), 0, CREATE_UNICODE_ENVIRONMENT | bevel_winkit::quiet::NO_WINDOW, env.as_ptr().cast(), null(), &info, &mut result)
    };
    assert_ne!(ok, 0, "start the copy's Help: {}", std::io::Error::last_os_error());
    // SAFETY: the thread handle is ours and not needed.
    unsafe { CloseHandle(result.hThread) };
    (result.hProcess, result.dwProcessId)
}

/// The exit code once the process ends (at most 10 s).
fn ended(process: HANDLE) -> Option<u32> {
    // SAFETY: our own process handle and a place for the code.
    unsafe {
        if WaitForSingleObject(process, 10_000) != WAIT_OBJECT_0 {
            TerminateProcess(process, 1);
            CloseHandle(process);
            return None;
        }
        let mut code = 0;
        GetExitCodeProcess(process, &mut code);
        CloseHandle(process);
        Some(code)
    }
}

/// The message box of process `pid` on this thread's desktop, waited for up to 10 s.
fn message_box(pid: u32) -> Option<windows_sys::Win32::Foundation::HWND> {
    let class = wide("#32770");
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        let mut after = null_mut();
        loop {
            // SAFETY: a terminated class name; the windows of this thread's desktop.
            after = unsafe { FindWindowExW(null_mut(), after, class.as_ptr(), null()) };
            if after.is_null() {
                break;
            }
            let mut owner = 0;
            // SAFETY: any window handle; a place for its process.
            unsafe { GetWindowThreadProcessId(after, &mut owner) };
            if owner == pid {
                return Some(after);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

fn hold(path: &Path) -> fs::File {
    fs::OpenOptions::new().read(true).share_mode(0).open(path).unwrap()
}

/// A start by the person while settings.txt is held: not known, so the program says so and ends,
/// before anything is written anywhere (the log included). It used the installed folder, then
/// saved into the portable one once the file was free (B1).
#[test]
fn a_start_that_cannot_know_ends_with_nothing_written() {
    let mut desktop = Desktop::new("a");
    let copy = Copy::new("home-not-known");
    let before = copy.listing();
    let held = hold(&copy.settings());
    let (process, pid) = start(&mut desktop, &copy, "");
    let shown = message_box(pid).expect("the message that the settings.txt is in use");
    assert_eq!(copy.listing(), before, "nothing written while the message shows");
    // SAFETY: the box belongs to our own child, on our private desktop.
    unsafe { PostMessageW(shown, WM_CLOSE, 0, 0) };
    assert_eq!(ended(process), Some(0));
    drop(held);
    assert_eq!(copy.listing(), before, "nothing written: no log in either folder, no installed folder");
}

/// A flagged start (a portable copy's program starting its Help) reads no settings.txt: with the
/// file held it runs at once and keeps its log beside the copy's bevel95.exe; unflagged and free,
/// the mark makes it portable too; with no settings.txt it is the installed folder's.
#[test]
fn a_flagged_start_keeps_its_data_beside_the_copy_without_reading() {
    let mut desktop = Desktop::new("b");
    let copy = Copy::new("home-flagged");
    let held = hold(&copy.settings());
    let started = Instant::now();
    let (process, _) = start(&mut desktop, &copy, "--portable-copy");
    assert_eq!(ended(process), Some(0));
    // On the clock only on request (BEVEL95_TIMED=1): a busy PC must not fail the default run
    // (brief 104).
    if std::env::var_os("BEVEL95_TIMED").is_some() {
        assert!(started.elapsed() < Duration::from_millis(900), "no wait for the held file: {:?}", started.elapsed());
    }
    drop(held);
    assert!(copy.portable_log().is_file(), "the log beside the copy's bevel95.exe");
    assert!(!copy.installed().exists(), "nothing in the installed folder");
    // Free and unflagged: by the mark, portable.
    fs::remove_file(copy.portable_log()).unwrap();
    let (process, _) = start(&mut desktop, &copy, "");
    assert_eq!(ended(process), Some(0));
    assert!(copy.portable_log().is_file() && !copy.installed().exists());
    // No settings.txt: the installed folder.
    fs::remove_file(copy.portable_log()).unwrap();
    fs::remove_file(copy.settings()).unwrap();
    let (process, _) = start(&mut desktop, &copy, "");
    assert_eq!(ended(process), Some(0));
    assert!(!copy.portable_log().exists() && copy.installed().join("bevel95.log").is_file());
}

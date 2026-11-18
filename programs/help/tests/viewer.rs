//! Process-level checks for a caller that supplies its own Help book.
#![cfg(windows)]

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, PipeReader, PipeWriter, Read, Write, pipe};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::ptr::{null, null_mut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bevel_core::help::{Catalog, Entry, EntryIcon, Keyword, Paragraph, Style, Topic, wire};
use bevel_winkit::help::OwnerNotifications;
use windows_sys::Win32::Foundation::{GENERIC_ALL, HANDLE_FLAG_INHERIT, HWND, SetHandleInformation, WAIT_OBJECT_0};
use windows_sys::Win32::System::StationsAndDesktops::{
    CloseDesktop, CreateDesktopW, GetThreadDesktop, HDESK, SetThreadDesktop,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, GetCurrentThreadId, GetExitCodeProcess, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOW,
    TerminateProcess, WaitForSingleObject,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, FindWindowExW, GW_OWNER, GetWindow, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, MSG, PM_REMOVE, PeekMessageW, SW_MINIMIZE, ShowWindowAsync, TranslateMessage, WS_POPUP,
};

const EXE: &str = env!("CARGO_BIN_EXE_bevel-help");
const TIMEOUT: Duration = Duration::from_secs(10);

fn wide(s: impl AsRef<OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}

fn book() -> Catalog {
    Catalog {
        title: "Pocket Paint Help".into(),
        home: 0,
        topics: vec![Topic {
            title: "Choosing a brush".into(),
            welcome: false,
            paragraphs: vec![Paragraph { style: Style::Body, text: "Click a brush to paint with it.".into() }],
        }],
        contents: vec![Entry { label: "Brushes".into(), depth: 0, topic: Some(0), icon: EntryIcon::Page }],
        index: vec![Keyword { label: "brush".into(), topic: 0 }],
    }
}

#[test]
fn double_click_without_a_book_exits_without_waiting_for_stdin() {
    // Keep the pipe open. A viewer that tried to read a book here would block.
    let mut child = bevel_winkit::quiet::command(EXE).stdin(Stdio::piped()).spawn().unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            return;
        }
        if start.elapsed() > TIMEOUT {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("viewer waited for a catalog with no command-line arguments");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

// The private desktop is never switched onto the display. The same thread owns its
// test window, pumps its messages and returns to the original desktop before cleanup.
struct Desktop {
    name: Vec<u16>,
    handle: HDESK,
    original: HDESK,
}

impl Desktop {
    fn new() -> Self {
        let name = wide(format!("Bevel95HelpTest-{}", std::process::id()));
        // SAFETY: terminated name, default device, mode, and security descriptor.
        unsafe {
            let original = GetThreadDesktop(GetCurrentThreadId());
            let handle = CreateDesktopW(name.as_ptr(), null(), null(), 0, GENERIC_ALL, null());
            assert!(!handle.is_null(), "create private desktop: {}", io::Error::last_os_error());
            let desktop = Self { name, handle, original };
            assert_ne!(SetThreadDesktop(handle), 0, "attach thread to private desktop");
            desktop
        }
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        // SAFETY: all windows created by this thread have been destroyed.
        unsafe {
            SetThreadDesktop(self.original);
            CloseDesktop(self.handle);
        }
    }
}

struct Owner(HWND);

impl Owner {
    fn new() -> Self {
        // SAFETY: STATIC is a system class. The window exists only on the test desktop.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("Pocket Paint").as_ptr(),
                WS_POPUP,
                0,
                0,
                20,
                20,
                null_mut(),
                null_mut(),
                null_mut(),
                null(),
            )
        };
        assert!(!hwnd.is_null(), "create owner: {}", io::Error::last_os_error());
        Self(hwnd)
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: our own test window, on its creating thread.
        unsafe { DestroyWindow(self.0) };
    }
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        // Remove only the empty directory we created.
        let _ = fs::remove_dir(&self.0);
    }
}

struct Viewer {
    process: OwnedHandle,
    pid: u32,
    input: Option<PipeWriter>,
    _owner_notifications: OwnerNotifications,
    modal_enqueued: Arc<AtomicBool>,
    listener_closed: Arc<AtomicBool>,
}

// After reading the picker's status byte, the client queues the owner update before
// asking for the next byte. This makes the order of overlapping pickers deterministic.
struct PickerStatus {
    stream: PipeReader,
    saw_picker: bool,
    enqueued: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
}

impl Read for PickerStatus {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.saw_picker {
            self.enqueued.store(true, Ordering::Release);
        }
        let count = self.stream.read(bytes)?;
        self.saw_picker |= bytes[..count].contains(&1);
        Ok(count)
    }
}

impl Drop for PickerStatus {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
    }
}

impl Viewer {
    fn launch(desktop: &mut Desktop, cwd: &Path, era: u8, target: &str, owner: HWND) -> Self {
        let (reader, writer) = pipe().unwrap();
        let (notifications, status_writer) = pipe().unwrap();
        let stderr = File::create("NUL").unwrap();
        let exe = wide(EXE);
        let mut cmd = wide(format!(
            "\"{EXE}\" --era {era} --scale 1 --open {target} --owner {} --parent {} --pipe",
            owner as usize,
            std::process::id()
        ));
        // SAFETY: all strings and handles live through CreateProcessW. Only the child
        // end of the input pipe is inheritable. The parent owns the sole write end.
        unsafe {
            for handle in [reader.as_raw_handle(), status_writer.as_raw_handle(), stderr.as_raw_handle()] {
                assert_ne!(SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT), 0);
            }
            let info = STARTUPINFOW {
                cb: size_of::<STARTUPINFOW>() as u32,
                lpDesktop: desktop.name.as_mut_ptr(),
                dwFlags: STARTF_USESTDHANDLES,
                hStdInput: reader.as_raw_handle(),
                hStdOutput: status_writer.as_raw_handle(),
                hStdError: stderr.as_raw_handle(),
                ..Default::default()
            };
            let mut result = PROCESS_INFORMATION::default();
            assert_ne!(
                CreateProcessW(
                    exe.as_ptr(),
                    cmd.as_mut_ptr(),
                    null(),
                    null(),
                    1,
                    bevel_winkit::quiet::NO_WINDOW,
                    null(),
                    wide(cwd).as_ptr(),
                    &info,
                    &mut result
                ),
                0,
                "start viewer: {}",
                io::Error::last_os_error()
            );
            drop(OwnedHandle::from_raw_handle(result.hThread));
            drop(status_writer);
            let modal_enqueued = Arc::new(AtomicBool::new(false));
            let listener_closed = Arc::new(AtomicBool::new(false));
            let status = PickerStatus {
                stream: notifications,
                saw_picker: false,
                enqueued: modal_enqueued.clone(),
                closed: listener_closed.clone(),
            };
            let mut viewer = Self {
                process: OwnedHandle::from_raw_handle(result.hProcess),
                pid: result.dwProcessId,
                input: Some(writer),
                _owner_notifications: OwnerNotifications::listen(owner, status).unwrap(),
                modal_enqueued,
                listener_closed,
            };
            wire::write(viewer.input.as_mut().unwrap(), &book()).unwrap();
            viewer
        }
    }

    fn command(&mut self, code: u32) {
        self.input.as_mut().unwrap().write_all(&code.to_le_bytes()).unwrap();
    }

    fn window(&self, title: &str) -> HWND {
        let title = wide(title);
        let class = wide("Bevel95Window");
        let mut hwnd = null_mut();
        until("viewer's window", || {
            // SAFETY: searches only the thread's private desktop. Check the PID before use.
            unsafe {
                hwnd = null_mut();
                loop {
                    hwnd = FindWindowExW(null_mut(), hwnd, class.as_ptr(), title.as_ptr());
                    if hwnd.is_null() {
                        return false;
                    }
                    let mut pid = 0;
                    if GetWindowThreadProcessId(hwnd, &mut pid) != 0 && pid == self.pid && IsWindowVisible(hwnd) != 0 {
                        return true;
                    }
                }
            }
        });
        hwnd
    }

    fn close_pipe(&mut self) {
        drop(self.input.take());
        // SAFETY: our live child process handle and a DWORD for its exit code.
        unsafe {
            until("viewer exits on EOF", || WaitForSingleObject(self.process.as_raw_handle(), 0) == WAIT_OBJECT_0);
            let mut code = 1;
            assert_ne!(GetExitCodeProcess(self.process.as_raw_handle(), &mut code), 0);
            assert_eq!(code, 0);
        }
    }

    fn crash(&self) {
        // SAFETY: this handle identifies only the specific viewer spawned in this test.
        unsafe {
            assert_ne!(TerminateProcess(self.process.as_raw_handle(), 17), 0);
            until("terminated viewer exits", || WaitForSingleObject(self.process.as_raw_handle(), 0) == WAIT_OBJECT_0);
        }
    }
}

impl Drop for Viewer {
    fn drop(&mut self) {
        // SAFETY: terminate only our own hung child if an assertion unwinds the test.
        unsafe {
            if WaitForSingleObject(self.process.as_raw_handle(), 0) != WAIT_OBJECT_0 {
                TerminateProcess(self.process.as_raw_handle(), 1);
                WaitForSingleObject(self.process.as_raw_handle(), 1000);
            }
        }
    }
}

fn until(description: &str, mut condition: impl FnMut() -> bool) {
    let start = Instant::now();
    loop {
        // SAFETY: dispatch only this thread's messages. The viewer can synchronously
        // enable or disable our owner window while we wait for it.
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if condition() {
            return;
        }
        assert!(start.elapsed() < TIMEOUT, "timed out waiting for {description}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "opens viewers on a private Windows desktop; requires an interactive window station"]
fn caller_supplied_book_and_viewer_lifetimes() {
    let mut desktop = Desktop::new();
    let scratch = std::env::temp_dir().join(format!("bevel-help-test-{}", std::process::id()));
    fs::create_dir(&scratch).unwrap();
    let scratch = Scratch(scratch);
    let owner = Owner::new();
    let mut winhelp = Viewer::launch(&mut desktop, &scratch.0, 95, "contents", owner.0);
    let picker = winhelp.window("Help Topics: Pocket Paint Help");
    // SAFETY: the picker belongs to our viewer and the owner belongs to this test.
    assert_eq!(unsafe { GetWindow(picker, GW_OWNER) }, owner.0);
    until("owner disabled for contents", || unsafe { IsWindowEnabled(owner.0) == 0 });
    winhelp.command(3);
    let article = winhelp.window("Pocket Paint Help");
    assert!(unsafe { GetWindow(article, GW_OWNER) }.is_null());
    until("owner usable with modeless article", || unsafe { IsWindowEnabled(owner.0) != 0 });
    winhelp.command(1);
    winhelp.window("Help Topics: Pocket Paint Help");
    until("owner disabled for index", || unsafe { IsWindowEnabled(owner.0) == 0 });
    winhelp.close_pipe();
    until("owner enabled on EOF", || unsafe { IsWindowEnabled(owner.0) != 0 });

    let first = Viewer::launch(&mut desktop, &scratch.0, 95, "contents", owner.0);
    first.window("Help Topics: Pocket Paint Help");
    until("first picker owns the caller", || unsafe { IsWindowEnabled(owner.0) == 0 });
    let second = Viewer::launch(&mut desktop, &scratch.0, 95, "index", owner.0);
    second.window("Help Topics: Pocket Paint Help");
    until("second picker queued its modal status", || second.modal_enqueued.load(Ordering::Acquire));
    first.crash();
    until("first viewer's status reader closed", || first.listener_closed.load(Ordering::Acquire));
    // Observe a later transition on an independent owner. Once it arrives, the client's
    // shared worker has already applied the first viewer's EOF to the original owner.
    let witness = Owner::new();
    let (status, mut sender) = pipe().unwrap();
    let _witness_notifications = OwnerNotifications::listen(witness.0, status).unwrap();
    sender.write_all(&[1]).unwrap();
    until("client processed the first viewer's EOF", || unsafe { IsWindowEnabled(witness.0) == 0 });
    assert_eq!(unsafe { IsWindowEnabled(owner.0) }, 0, "second viewer still owns the caller");
    drop(sender);
    until("independent owner released", || unsafe { IsWindowEnabled(witness.0) != 0 });
    second.crash();
    until("caller recovered after both viewer crashes", || unsafe { IsWindowEnabled(owner.0) != 0 });

    let mut html = Viewer::launch(&mut desktop, &scratch.0, 98, "topic-0", null_mut());
    let hwnd = html.window("Pocket Paint Help");
    for target in [2, 3] {
        // A valid navigation request restores and reuses the existing HTML Help window.
        unsafe { ShowWindowAsync(hwnd, SW_MINIMIZE) };
        until("viewer minimized", || unsafe { IsIconic(hwnd) != 0 });
        html.command(target);
        until("viewer restored", || unsafe { IsIconic(hwnd) == 0 });
        assert_eq!(html.window("Pocket Paint Help"), hwnd);
    }
    html.close_pipe();
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0, "viewer wrote files in the caller's directory");
}

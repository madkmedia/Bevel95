//! Track modal WinHelp pickers in the caller. A viewer reports whether its picker is
//! open on stdout. Losing the viewer or closing its pipe releases its claim on the owner.

use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled};
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static WORKER: Mutex<Option<mpsc::Sender<Event>>> = Mutex::new(None);

enum Event {
    Register(u64, Ticket),
    Modal(u64, bool),
    End(u64),
}

struct Ticket {
    owner: usize,
    live: Arc<AtomicBool>,
    modal: bool,
}

struct Owner {
    pickers: usize,
    disabled_by_us: bool,
}

fn caller_window(owner: usize) -> bool {
    let mut pid = 0;
    // SAFETY: Windows checks this HWND; the process ID output is valid for the call.
    owner != 0 && unsafe { GetWindowThreadProcessId(owner as HWND, &mut pid) != 0 && pid == std::process::id() }
}

fn set_modal(owners: &mut HashMap<usize, Owner>, ticket: &mut Ticket, modal: bool) {
    if ticket.owner == 0 || ticket.modal == modal {
        return;
    }
    ticket.modal = modal;
    let hwnd = ticket.owner as HWND;
    if modal {
        let owner = owners.entry(ticket.owner).or_insert_with(|| {
            // EnableWindow synchronously notifies the UI thread. Only this worker waits
            // for those notifications, and it holds no mutex while Windows delivers them.
            let disabled_by_us =
                caller_window(ticket.owner) && unsafe { IsWindowEnabled(hwnd) != 0 && EnableWindow(hwnd, 0) == 0 };
            Owner { pickers: 0, disabled_by_us }
        });
        owner.pickers += 1;
    } else if let Some(owner) = owners.get_mut(&ticket.owner) {
        owner.pickers -= 1;
        if owner.pickers == 0 {
            let enable = owner.disabled_by_us;
            owners.remove(&ticket.owner);
            if enable && caller_window(ticket.owner) {
                // SAFETY: the last picker for this caller's window has gone away.
                unsafe { EnableWindow(hwnd, 1) };
            }
        }
    }
}

fn worker() -> io::Result<mpsc::Sender<Event>> {
    let mut shared = WORKER.lock().map_err(|_| io::Error::other("Help owner worker is unavailable"))?;
    if let Some(sender) = &*shared {
        return Ok(sender.clone());
    }
    let (sender, events) = mpsc::channel();
    thread::Builder::new().name("Help owner".into()).spawn(move || {
        let mut tickets = HashMap::new();
        let mut owners = HashMap::new();
        for event in events {
            match event {
                Event::Register(id, ticket) => {
                    tickets.insert(id, ticket);
                }
                Event::Modal(id, modal) => {
                    if let Some(ticket) = tickets.get_mut(&id)
                        && ticket.live.load(Ordering::Acquire)
                    {
                        set_modal(&mut owners, ticket, modal);
                    }
                }
                Event::End(id) => {
                    if let Some(mut ticket) = tickets.remove(&id) {
                        set_modal(&mut owners, &mut ticket, false);
                    }
                }
            }
        }
    })?;
    *shared = Some(sender.clone());
    Ok(sender)
}

/// One viewer's connection. Dropping it releases the owner even if that viewer is
/// still shutting down. The same cleanup runs automatically on process exit or pipe EOF.
pub struct OwnerNotifications {
    id: u64,
    live: Arc<AtomicBool>,
    sender: mpsc::Sender<Event>,
}

impl OwnerNotifications {
    pub fn listen(owner: HWND, mut stdout: impl Read + Send + 'static) -> io::Result<Self> {
        let sender = worker()?;
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let live = Arc::new(AtomicBool::new(true));
        sender
            .send(Event::Register(id, Ticket { owner: owner as usize, live: live.clone(), modal: false }))
            .map_err(|_| io::Error::other("Help owner worker stopped"))?;
        let guard = Self { id, live: live.clone(), sender: sender.clone() };
        thread::Builder::new().name("Help window notifications".into()).spawn(move || {
            let mut byte = [0];
            while live.load(Ordering::Acquire) {
                match stdout.read_exact(&mut byte) {
                    Ok(()) if byte[0] <= 1 => {
                        if sender.send(Event::Modal(id, byte[0] == 1)).is_err() {
                            break;
                        }
                    }
                    _ => break,
                }
            }
            live.store(false, Ordering::Release);
            let _ = sender.send(Event::End(id));
        })?;
        Ok(guard)
    }
}

impl Drop for OwnerNotifications {
    fn drop(&mut self) {
        self.live.store(false, Ordering::Release);
        let _ = self.sender.send(Event::End(self.id));
    }
}

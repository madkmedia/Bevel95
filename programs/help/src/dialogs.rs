//! Find, Print and the index keyword chooser for the HTML Help viewer.

use bevel_core::html_help::commands::{CommandDialog, Part};
use bevel_core::html_help::topics::{Part as TopicPart, TopicsFound};
use bevel_core::kit::Form;
use bevel_core::theme::Era;
use bevel_winkit::kit::{self, Hosted, Kind};
use bevel_winkit::log::log;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

use super::{Modern, bring_forward, on_screen};

struct FindWindow(CommandDialog);

impl Hosted for FindWindow {
    type F = CommandDialog;
    fn form(&self) -> &Self::F {
        &self.0
    }
    fn form_mut(&mut self) -> &mut Self::F {
        &mut self.0
    }

    fn activate(part: Part) {
        match part {
            Part::Cancel | Part::Close => Self::request_close(),
            Part::Ok => {
                let Some((text, options)) = kit::with::<Self, _>(|h| (h.0.query().to_owned(), h.0.find_options()))
                else {
                    return;
                };
                let mut found = false;
                kit::update::<Modern>(|h| found = h.form.find_next(&text, options));
                if !found {
                    show_finished();
                }
            }
            Part::Tip(text) => {
                if let Some(hwnd) = kit::hwnd::<Self>() {
                    bevel_winkit::help::context_help(hwnd, text);
                }
            }
            other => kit::update::<Self>(|h| h.0.activate(other)),
        }
    }

    fn request_close() {
        if let Some(text) = kit::with::<Self, _>(|h| h.0.query().to_owned()) {
            kit::with::<Modern, _>(|h| h.form.remember_find_text(text));
        }
        kit::close::<Self>();
    }
}

struct PrintWindow(CommandDialog);

impl Hosted for PrintWindow {
    type F = CommandDialog;
    fn form(&self) -> &Self::F {
        &self.0
    }
    fn form_mut(&mut self) -> &mut Self::F {
        &mut self.0
    }

    fn activate(part: Part) {
        match part {
            Part::Cancel | Part::Close => kit::close::<Self>(),
            Part::Ok => {
                let Some(children) = kit::with::<Self, _>(|h| h.0.include_subtopics()) else { return };
                kit::close::<Self>();
                if let Some(owner) = kit::hwnd::<Modern>() {
                    send_to_printer(owner, children);
                }
            }
            other => kit::update::<Self>(|h| h.0.activate(other)),
        }
    }
}

struct TopicWindow(TopicsFound);

impl Hosted for TopicWindow {
    type F = TopicsFound;
    fn form(&self) -> &Self::F {
        &self.0
    }
    fn form_mut(&mut self) -> &mut Self::F {
        &mut self.0
    }

    fn activate(part: TopicPart) {
        match part {
            TopicPart::Display => {
                let selected = kit::with::<Self, _>(|h| h.0.selection()).flatten();
                if let Some(topic) = selected {
                    kit::close::<Self>();
                    kit::update::<Modern>(|h| {
                        h.form.show_topic(topic);
                    });
                }
            }
            TopicPart::Close | TopicPart::Cancel => kit::close::<Self>(),
            other => kit::update::<Self>(|h| h.0.activate(other)),
        }
    }
}

struct FinishedWindow(CommandDialog);

impl Hosted for FinishedWindow {
    type F = CommandDialog;
    fn form(&self) -> &Self::F {
        &self.0
    }
    fn form_mut(&mut self) -> &mut Self::F {
        &mut self.0
    }
    fn activate(_: Part) {
        Self::request_close();
    }
    fn request_close() {
        kit::close::<Self>();
        kit::update::<FindWindow>(|h| h.0.select_query());
    }
}

fn show_finished() {
    let (Some(owner), Some(look), Some(era)) =
        (kit::hwnd::<FindWindow>(), kit::look_of::<FindWindow>(), kit::with::<Modern, _>(|h| h.era))
    else {
        return;
    };
    let Some((viewer, monitor)) = window_and_monitor(owner) else { return };
    let scale = look.scale;
    let form = FinishedWindow(CommandDialog::finished(era));
    let placed = kit::open_as(Kind::Dialog { owner }, form, look, |w, h| {
        // Both reference PCs offset the alert slightly from the middle of Find.
        let dx = if era == Era::Win98 { 5 } else { 4 };
        on_screen(
            monitor.rcWork,
            (
                viewer.left + (viewer.right - viewer.left - w) / 2 + dx * scale,
                viewer.top + (viewer.bottom - viewer.top - h) / 2 + 9 * scale,
            ),
            w,
            h,
        )
    });
    if placed.is_none() {
        log("Help could not show the end-of-search message");
    }
}

/// Close any owned command before replacing its viewer or releasing the caller.
pub(super) fn close() {
    kit::close::<FinishedWindow>();
    kit::close::<TopicWindow>();
    kit::close::<PrintWindow>();
    kit::close::<FindWindow>();
}

/// An Index keyword can name multiple topics. The child keeps the book's original topic IDs
/// even when their titles repeat; only Display changes the page in the parent.
pub(super) fn topics_found(owner: HWND) {
    if let Some(hwnd) = kit::hwnd::<TopicWindow>() {
        bring_forward(hwnd);
        return;
    }
    let Some((era, location, topics, (anchor, keyboard))) =
        kit::with::<Modern, _>(|h| (h.era, h.form.title().to_owned(), h.form.keyword_topics(), h.form.topics_anchor()))
    else {
        return;
    };
    if topics.len() < 2 {
        return;
    }
    let (Some(look), Some((viewer, monitor))) = (kit::look_of::<Modern>(), window_and_monitor(owner)) else { return };
    let scale = look.scale;
    let mut chooser = TopicsFound::new(era, location, topics);
    if keyboard {
        chooser.show_keys();
    }
    if kit::open_as(Kind::Dialog { owner }, TopicWindow(chooser), look, |w, h| {
        let x = viewer.left + anchor.x * scale + (anchor.w * scale - w) / 2;
        let y = viewer.top + anchor.y * scale + (anchor.h * scale - h) / 2;
        on_screen(monitor.rcWork, (x, y), w, h)
    })
    .is_none()
    {
        log("Help could not show the matching topics");
    }
}

fn window_and_monitor(owner: HWND) -> Option<(RECT, MONITORINFO)> {
    let mut monitor = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    let mut viewer = RECT::default();
    // SAFETY: the owner is our viewer and both outputs have the required sizes.
    unsafe {
        if GetMonitorInfoW(MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST), &mut monitor) == 0
            || GetWindowRect(owner, &mut viewer) == 0
        {
            log("Help could not measure its command dialog's screen");
            return None;
        }
    }
    Some((viewer, monitor))
}

fn open<H: Hosted<F = CommandDialog>>(owner: HWND, hosted: H, kind: Kind) {
    if let Some(hwnd) = kit::hwnd::<H>() {
        bring_forward(hwnd);
        return;
    }
    let Some(look) = kit::look_of::<Modern>() else { return };
    let Some((viewer, monitor)) = window_and_monitor(owner) else { return };
    let center = if hosted.form().is_find() { monitor.rcMonitor } else { viewer };
    let place = |w, h| {
        let pos =
            (center.left + (center.right - center.left - w) / 2, center.top + (center.bottom - center.top - h) / 2);
        on_screen(monitor.rcWork, pos, w, h)
    };
    if kit::open_as(kind, hosted, look, place).is_none() {
        log("Help could not open its command dialog");
    }
}

pub(super) fn find(owner: HWND) {
    if let Some((era, text)) = kit::with::<Modern, _>(|h| (h.era, h.form.last_find_text().to_owned())) {
        let mut form = CommandDialog::find(era);
        form.paste(&text);
        form.select_query();
        let kind = if era == Era::Win2000 { Kind::Owned { owner } } else { Kind::Dialog { owner } };
        open(owner, FindWindow(form), kind);
    }
}

pub(super) fn print(owner: HWND) {
    let Some((era, chooser)) = kit::with::<Modern, _>(|h| (h.era, h.form.print_uses_chooser())) else { return };
    if chooser {
        open(owner, PrintWindow(CommandDialog::print(era)), Kind::Dialog { owner });
    } else {
        send_to_printer(owner, false);
    }
}

fn send_to_printer(owner: HWND, children: bool) {
    if let Some((title, text)) =
        kit::with::<Modern, _>(|h| (h.form.title().to_owned(), h.form.print_selection(children)))
    {
        super::print::print(owner, &title, &text);
    }
}

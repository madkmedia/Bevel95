//! Date/Time Properties (a double-click on the clock, or Adjust Date/Time on its menu):
//! `bevel_core::datetime` draws it and handles its keys and mouse (`DateTimeWindow`, a window
//! of the control kit); this module opens it, lets its clock run and carries out OK and Apply.
//! OK and Apply set Bevel95's own clock: how far it runs from the time in its zone
//! (`bar::set_clock_offset`), and on the Time Zone page which zone that is and whether it
//! follows daylight saving time (`bar::set_clock_zone`). The PC's clock and zone are never
//! changed, so no administrator prompt and nothing else on the PC notices.

use bevel_core::datetime::{self, DateTime, DateTimeWindow, Moment, Part};
use bevel_core::timezone;

use crate::bar::{self, with_app};
use crate::kit::{self, Hosted};
use crate::log::log;
use crate::util::take_focus;
use crate::window_list::launch;
use crate::zones;

/// The timer that lets the page's clock run (a few times a second, so it turns over on time).
const TICK: usize = 2;

/// The window, while it is open (one at a time).
pub struct DateTimeHost {
    form: DateTimeWindow,
}

impl Hosted for DateTimeHost {
    type F = DateTimeWindow;
    fn form(&self) -> &DateTimeWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut DateTimeWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        activate(part);
    }
    fn timers() -> &'static [(usize, u32)] {
        &[(TICK, 250)]
    }
    /// The page's clock runs with Bevel95's (while the date or time is not being set).
    fn timer(&mut self, _id: usize) -> bool {
        let Some(offset) = with_app(|app| app.clock_offset) else {
            return false;
        };
        let now = bar::clock_now(offset);
        let d = &mut self.form.dt;
        if d.editing || d.shown == now {
            return false;
        }
        d.follow(now);
        true
    }
}

/// Opens the window (or brings it forward if it is already open).
pub fn open() {
    if let Some(hwnd) = kit::hwnd::<DateTimeHost>() {
        take_focus(hwnd);
        return;
    }
    let Some((era, offset, zone, dst)) = with_app(|app| (app.era, app.clock_offset, app.clock_zone, app.clock_dst))
    else {
        return;
    };
    let mut dt = DateTime::new(era, bar::clock_now(offset), zones::clock_zone_name());
    fill_zones(&mut dt, zone, dst);
    let Some(form) = with_app(|app| DateTimeWindow::new(dt, &app.fonts())) else {
        return;
    };
    // Where Windows put its first window of a kind: near the main screen's top left.
    let place = |w: i32, h: i32| {
        with_app(|app| {
            let s = app.scale;
            let x = (app.monitor.left + s.len(44)).min(app.monitor.right - w).max(app.monitor.left);
            let y = (app.monitor.top + s.len(44)).min(app.bar_top_on_screen() - h).max(app.monitor.top);
            (x, y)
        })
        .unwrap_or((0, 0))
    };
    if kit::open(DateTimeHost { form }, place).is_some() {
        log("opened Date/Time Properties");
    }
}
/// The Time Zone page's starting point: the zone Bevel95's clock keeps (`key`, or the PC's)
/// as an entry of the look's list (the first entry for that zone; failing that, the first
/// with the same offset from GMT), the daylight saving box, and which entries' zones have
/// daylight saving time on this Windows.
fn fill_zones(dt: &mut DateTime, key: Option<&str>, dst: bool) {
    let all = zones::all();
    let key = key.map_or_else(|| zones::key_of(&zones::pc_zone()), str::to_string);
    let find = |k: &str| all.iter().find(|z| zones::key_of(z).eq_ignore_ascii_case(k));
    let list = timezone::zones(dt.era);
    dt.zone_has_dst = list.iter().map(|entry| find(entry.key).is_some_and(zones::has_dst)).collect();
    dt.zone = timezone::index_of_key(dt.era, &key)
        .or_else(|| find(&key).and_then(|z| timezone::index_of_offset(dt.era, -z.Bias)))
        .unwrap_or(0);
    dt.dst = dst;
}

/// Closes the window (without applying anything).
pub fn close() {
    kit::close::<DateTimeHost>();
}

/// Changes the window's state, lays it out again and repaints it.
fn update(f: impl FnOnce(&mut DateTime)) {
    kit::update::<DateTimeHost>(|h| f(&mut h.form.dt));
}
/// Puts what the window shows into effect: first the zone (the PC's own zone is kept as "the
/// PC's"), then, if the date or time was set, how far Bevel95's clock runs from the time in
/// that zone. The page runs on from there.
fn apply(dt: &DateTime) {
    let pc_key = zones::key_of(&zones::pc_zone());
    let chosen = timezone::zones(dt.era).get(dt.zone).map(|z| z.key);
    let zone = chosen.filter(|k| !k.eq_ignore_ascii_case(&pc_key));
    if with_app(|app| (app.clock_zone, app.clock_dst)) != Some((zone, dt.dst)) {
        bar::set_clock_zone(zone, dt.dst);
    }
    if dt.editing {
        bar::set_clock_offset(dt.shown.to_seconds() - bar::real_now().to_seconds());
    }
    let name = zones::clock_zone_name();
    update(|d| {
        d.changed = false;
        d.editing = false;
        d.zone_name = name;
    });
}

/// Carries out a push button (or the title bar's buttons).
fn activate(part: Part) {
    let Some(d) = kit::with::<DateTimeHost, _>(|h| h.form.dt.clone()) else {
        return;
    };
    if !d.enabled(part) {
        return;
    }
    match part {
        Part::TitleClose | Part::Cancel => close(),
        Part::TitleHelp => launch("https://support.microsoft.com/windows"),
        Part::Ok => {
            if d.changed {
                apply(&d);
            }
            close();
        }
        Part::Apply => apply(&d),
        _ => {}
    }
}
/// The moment as Windows' SYSTEMTIME needs it, and back.
pub fn to_moment(t: &windows_sys::Win32::Foundation::SYSTEMTIME) -> Moment {
    Moment {
        year: i32::from(t.wYear),
        month: u32::from(t.wMonth),
        day: u32::from(t.wDay),
        hour: u32::from(t.wHour),
        minute: u32::from(t.wMinute),
        second: u32::from(t.wSecond),
    }
}

pub fn to_systemtime(m: Moment) -> windows_sys::Win32::Foundation::SYSTEMTIME {
    windows_sys::Win32::Foundation::SYSTEMTIME {
        wYear: m.year as u16,
        wMonth: m.month as u16,
        wDayOfWeek: datetime::weekday(m.year, m.month, m.day) as u16,
        wDay: m.day as u16,
        wHour: m.hour as u16,
        wMinute: m.minute as u16,
        wSecond: m.second as u16,
        wMilliseconds: 0,
    }
}

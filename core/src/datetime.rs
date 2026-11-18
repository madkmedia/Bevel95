//! Date/Time Properties, the window a double-click on the clock opened, drawn as each Windows
//! drew it. Read pixel for pixel from the reference PCs (real Windows 95, 98 and 2000; see
//! docs/theme-spec.md, "Date/Time Properties"): the same window in all three, 95 and 98
//! identical, 2000 with its own colors and Tahoma.
//!
//! In Bevel95 it sets Bevel95's own clock (the owner's choice): how far the bar's clock runs
//! from the real time, never the PC's own clock. This module only lays out and draws; the
//! platform layer owns the window, the real time and the saved offset.

use crate::dialog::{self, TitleButtons};
use crate::fb::{Framebuffer, Rect};
use crate::kit::{self, Effect, Key, Mods, Press};
use crate::text::{Font, Fonts};
use crate::theme::{Colors, Era, Theme, rgb};
use crate::timezone;

/// A date and time of day as the window shows it (no time zone attached).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moment {
    pub year: i32,
    /// 1 to 12.
    pub month: u32,
    /// 1 to the month's last day.
    pub day: u32,
    /// 0 to 23.
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Moment {
    /// Seconds since 1 January 1601 (Windows' own starting point), to tell how far apart two
    /// moments are.
    pub fn to_seconds(self) -> i64 {
        let days = days_before_year(self.year)
            + (1..self.month).map(|m| i64::from(days_in_month(self.year, m))).sum::<i64>()
            + i64::from(self.day)
            - 1;
        days * 86_400 + i64::from(self.hour * 3600 + self.minute * 60 + self.second)
    }

    /// The moment `seconds` after 1 January 1601 (see `to_seconds`).
    pub fn from_seconds(seconds: i64) -> Moment {
        let mut days = seconds.div_euclid(86_400);
        let in_day = seconds.rem_euclid(86_400) as u32;
        let mut year = 1601 + (days / 146_097 * 400) as i32;
        days %= 146_097;
        while days >= days_in_year(year) {
            days -= days_in_year(year);
            year += 1;
        }
        let mut month = 1;
        while days >= i64::from(days_in_month(year, month)) {
            days -= i64::from(days_in_month(year, month));
            month += 1;
        }
        Moment { year, month, day: days as u32 + 1, hour: in_day / 3600, minute: in_day / 60 % 60, second: in_day % 60 }
    }

    /// The same time of day on another date, the day kept within the month.
    pub fn with_date(self, year: i32, month: u32, day: u32) -> Moment {
        let year = year.clamp(1980, 2099);
        Moment { year, month, day: day.clamp(1, days_in_month(year, month)), ..self }
    }
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_year(year: i32) -> i64 {
    if is_leap(year) { 366 } else { 365 }
}

fn days_before_year(year: i32) -> i64 {
    let y = i64::from(year - 1601);
    y * 365 + y / 4 - y / 100 + y / 400
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The day of the week, 0 for Sunday (the calendar's first column).
pub fn weekday(year: i32, month: u32, day: u32) -> u32 {
    // 1 January 1601 was a Monday.
    ((Moment { year, month, day, hour: 0, minute: 0, second: 0 }.to_seconds() / 86_400 + 1) % 7) as u32
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];

/// The time as the window shows it, in Windows' US English format ("10:35:18 PM").
pub fn time_text(m: Moment) -> String {
    let h12 = if m.hour.is_multiple_of(12) { 12 } else { m.hour % 12 };
    format!("{h12}:{:02}:{:02} {}", m.minute, m.second, if m.hour < 12 { "AM" } else { "PM" })
}

/// The window's pages, in tab order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    DateTime,
    TimeZone,
}

pub const PAGES: [Page; 2] = [Page::DateTime, Page::TimeZone];

/// The parts of the window a click or the keyboard can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Tab(usize),
    Month,
    MonthDrop,
    /// A month in the month's open list (1 to 12).
    MonthItem(u32),
    Year,
    YearUp,
    YearDown,
    Day(u32),
    Time,
    /// One part of the time: 0 hour, 1 minutes, 2 seconds, 3 AM/PM.
    TimeField(usize),
    TimeUp,
    TimeDown,
    /// Time Zone page: the zone's drop-down list, its button, a row of the dropped list (index
    /// into the era's list), the dropped list's scroll bar parts, the daylight saving box.
    Zone,
    ZoneDrop,
    ZoneRow(usize),
    ZoneUp,
    ZoneDown,
    ZonePageUp,
    ZonePageDown,
    ZoneThumb,
    Dst,
    Ok,
    Cancel,
    Apply,
    TitleHelp,
    TitleClose,
}

/// How the chosen part of the time shows while the time has the focus (Windows 98).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimeEntry {
    /// Just chosen (by Tab, a click, Alt+T): highlighted.
    Selected,
    /// Changed with the arrows: its number unpadded, not highlighted; a digit starts afresh.
    Stepped,
    /// The digits typed into it so far (one or two), shown as typed.
    Typed(String),
}

/// The window's state.
#[derive(Clone, Debug)]
pub struct DateTime {
    pub era: Era,
    pub page: usize,
    /// What the page shows: Bevel95's clock running, or the date and time being set.
    pub shown: Moment,
    pub focus: Part,
    pub pressed: Option<Part>,
    /// Something was changed since the window opened or Apply was last clicked.
    pub changed: bool,
    /// Underline access keys (Windows 2000 shows them once the keyboard was used).
    pub show_keys: bool,
    /// Where the window's top left corner is on the screen: the dotted focus rectangles line up
    /// with the screen.
    pub screen: (i32, i32),
    /// The name of the zone Bevel95's clock keeps ("Pacific Standard Time"), on the Date & Time
    /// page.
    pub zone_name: String,
    /// Time Zone page: the chosen entry of the era's zone list (`timezone::zones`), whether
    /// its list is dropped down and the first row it shows, the daylight saving box, and for
    /// each entry whether its zone has daylight saving time at all (from Windows; missing
    /// entries count as having it).
    pub zone: usize,
    pub zone_list: bool,
    pub zone_top: usize,
    /// The dropped list's row under the pointer: highlighted instead of the chosen entry.
    pub zone_hover: Option<usize>,
    pub dst: bool,
    pub zone_has_dst: Vec<bool>,
    /// The part of the time the arrows change (see `Part::TimeField`).
    pub time_field: usize,
    /// The date or time was changed: it stays as set instead of running with the clock.
    pub editing: bool,
    /// The month's list is open.
    pub month_list: bool,
    /// How the chosen part of the time shows.
    pub entry: TimeEntry,
    /// The caret's blink phase: shown (drawn while the time has the focus).
    pub caret: bool,
}

impl DateTime {
    pub fn new(era: Era, shown: Moment, zone_name: String) -> DateTime {
        DateTime {
            era,
            page: 0,
            shown,
            focus: Part::Month,
            pressed: None,
            changed: false,
            show_keys: era.always_shows_keys(),
            screen: (0, 0),
            zone_name,
            zone: 0,
            zone_list: false,
            zone_top: 0,
            zone_hover: None,
            dst: true,
            zone_has_dst: Vec::new(),
            time_field: 0,
            editing: false,
            month_list: false,
            entry: TimeEntry::Selected,
            caret: true,
        }
    }

    /// Shows page `index`, the focus on its first control (as a click on a tab or Ctrl+Tab
    /// does): the month, or the zone's list.
    pub fn show_page(&mut self, index: usize) {
        self.page = index.min(PAGES.len() - 1);
        self.month_list = false;
        self.close_zones();
        self.focus = if self.current_page() == Page::TimeZone { Part::Zone } else { Part::Month };
    }

    /// Whether the chosen zone has daylight saving time (the check box is grayed out without,
    /// or in Windows 2000 left out).
    pub fn zone_dst_possible(&self) -> bool {
        self.zone_has_dst.get(self.zone).copied().unwrap_or(true)
    }

    /// Windows 2000 shows the daylight saving box only for zones that have it.
    pub fn dst_shown(&self) -> bool {
        self.era != Era::Win2000 || self.zone_dst_possible()
    }

    /// Chooses entry `i` of the zone list (as the arrow keys and the dropped list do).
    pub fn choose_zone(&mut self, i: usize) {
        let i = i.min(timezone::zones(self.era).len().saturating_sub(1));
        if i != self.zone {
            self.zone = i;
            self.changed = true;
        }
        self.keep_zone_in_view();
    }

    /// Moves the zone choice `delta` entries, stopping at the ends.
    pub fn step_zone(&mut self, delta: i32) {
        let last = timezone::zones(self.era).len() as i32 - 1;
        self.choose_zone((self.zone as i32 + delta).clamp(0, last.max(0)) as usize);
    }

    /// Drops the list down, the chosen entry its first row (as far as the list allows).
    pub fn open_zones(&mut self) {
        self.zone_list = true;
        self.zone_top = self.zone.min(self.max_zone_top());
    }

    /// Closes the dropped list.
    pub fn close_zones(&mut self) {
        self.zone_list = false;
        self.zone_hover = None;
    }

    /// Scrolls the dropped list `delta` rows.
    pub fn scroll_zones(&mut self, delta: i32) {
        self.zone_top = (self.zone_top as i32 + delta).clamp(0, self.max_zone_top() as i32) as usize;
    }

    fn max_zone_top(&self) -> usize {
        timezone::zones(self.era).len().saturating_sub(timezone::LIST_ROWS)
    }

    fn keep_zone_in_view(&mut self) {
        if self.zone < self.zone_top {
            self.zone_top = self.zone;
        } else if self.zone >= self.zone_top + timezone::LIST_ROWS {
            self.zone_top = self.zone + 1 - timezone::LIST_ROWS;
        }
    }

    /// Flips the daylight saving box (if the zone has daylight saving time).
    pub fn toggle_dst(&mut self) {
        if self.zone_dst_possible() {
            self.dst = !self.dst;
            self.changed = true;
        }
    }

    /// Chooses part `i` of the time (0 hour, 1 minutes, 2 seconds, 3 AM/PM): highlighted.
    pub fn select_time_part(&mut self, i: usize) {
        self.time_field = i.min(3);
        self.entry = TimeEntry::Selected;
        self.caret = true;
    }

    /// A digit typed into the chosen part of the time, as Windows 98 takes it: it replaces the
    /// part, or makes a two-digit number with the one digit typed before it if that is still a
    /// time; anything else is ignored (returns false). A typed hour keeps AM or PM, except that
    /// 12 always means noon.
    pub fn type_digit(&mut self, digit: u32) -> bool {
        if self.time_field > 2 || digit > 9 {
            return false;
        }
        let text = match &self.entry {
            TimeEntry::Typed(t) if t.len() == 1 => format!("{t}{digit}"),
            TimeEntry::Typed(_) => return false,
            _ => digit.to_string(),
        };
        let v: u32 = text.parse().unwrap_or(0);
        let valid = if self.time_field == 0 { (1..=12).contains(&v) } else { v <= 59 };
        if !valid {
            return false;
        }
        let mut m = self.shown;
        match self.time_field {
            0 => m.hour = if v == 12 { 12 } else { v + if m.hour >= 12 { 12 } else { 0 } },
            1 => m.minute = v,
            _ => m.second = v,
        }
        self.set(m);
        self.entry = TimeEntry::Typed(text);
        self.caret = true;
        true
    }

    /// "A" or "P" typed while AM/PM is chosen.
    pub fn type_am_pm(&mut self, pm: bool) -> bool {
        if self.time_field != 3 {
            return false;
        }
        let mut m = self.shown;
        m.hour = m.hour % 12 + if pm { 12 } else { 0 };
        self.set(m);
        true
    }

    /// The clock moved on: the page follows it unless the date or time is being set.
    pub fn follow(&mut self, now: Moment) {
        if !self.editing {
            self.shown = now;
        }
    }

    fn set(&mut self, m: Moment) {
        if m != self.shown {
            self.shown = m;
            self.changed = true;
        }
        self.editing = true;
    }

    pub fn choose_day(&mut self, day: u32) {
        let m = self.shown;
        self.set(m.with_date(m.year, m.month, day));
    }

    /// Another day, `delta` days on, staying within the month shown.
    pub fn step_day(&mut self, delta: i32) {
        let m = self.shown;
        let day = (m.day as i32 + delta).clamp(1, days_in_month(m.year, m.month) as i32) as u32;
        self.choose_day(day);
    }

    pub fn choose_month(&mut self, month: u32) {
        let m = self.shown;
        self.set(m.with_date(m.year, month.clamp(1, 12), m.day));
    }

    /// The month list's next or previous month (it does not run on into another year).
    pub fn step_month(&mut self, delta: i32) {
        self.choose_month((self.shown.month as i32 + delta).clamp(1, 12) as u32);
    }

    pub fn step_year(&mut self, delta: i32) {
        let m = self.shown;
        self.set(m.with_date(m.year + delta, m.month, m.day));
    }

    /// The chosen part of the time, `delta` on, wrapping around within that part.
    pub fn step_time(&mut self, delta: i32) {
        let mut m = self.shown;
        let wrap = |v: u32, n: i32| (v as i32 + delta).rem_euclid(n) as u32;
        match self.time_field {
            0 => m.hour = wrap(m.hour % 12, 12) + if m.hour >= 12 { 12 } else { 0 },
            1 => m.minute = wrap(m.minute, 60),
            2 => m.second = wrap(m.second, 60),
            _ => m.hour = (m.hour + 12) % 24,
        }
        self.set(m);
        if self.time_field < 3 {
            self.entry = TimeEntry::Stepped;
        }
        self.caret = true;
    }

    pub fn current_page(&self) -> Page {
        PAGES[self.page.min(PAGES.len() - 1)]
    }

    pub fn enabled(&self, part: Part) -> bool {
        match part {
            Part::Apply => self.changed,
            Part::Dst => self.zone_dst_possible(),
            _ => true,
        }
    }
}

/// Where everything is (window coordinates).
pub struct DateTimeLayout {
    pub width: i32,
    pub height: i32,
    pub tabs: Vec<Rect>,
    pub page: Rect,
    pub parts: Vec<(Part, Rect)>,
}

impl DateTimeLayout {
    pub fn rect(&self, part: Part) -> Option<Rect> {
        self.parts.iter().find(|(p, _)| *p == part).map(|&(_, r)| r)
    }

    pub fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.parts.iter().rev().find(|(_, r)| r.contains(x, y)).map(|&(p, _)| p)
    }
}

pub const TITLE: &str = "Date/Time Properties";
const SIZE: (i32, i32) = (404, 341);
const TAB_X: i32 = 11;
const TAB_Y: i32 = 31;
const TAB_H: i32 = 18;
const TAB_PAD: i32 = 6;
const PAGE: Rect = Rect::new(9, 49, 386, 253);
const BUTTONS: [(Part, i32); 3] = [(Part::Ok, 158), (Part::Cancel, 239), (Part::Apply, 320)];
const BUTTONS_Y: i32 = 308;
const DATE_GROUP: Rect = Rect::new(19, 60, 183, 197);
const TIME_GROUP: Rect = Rect::new(211, 60, 170, 197);
const MONTH: Rect = Rect::new(30, 79, 75, 21);
const MONTH_DROP: Rect = Rect::new(87, 81, 16, 17);
/// The month's highlight while it has the focus (the dotted rectangle runs around it).
const MONTH_ITEM: Rect = Rect::new(34, 83, 51, 13);
const YEAR: Rect = Rect::new(115, 79, 63, 21);
const YEAR_UP: Rect = Rect::new(178, 79, 16, 10);
const YEAR_DOWN: Rect = Rect::new(178, 89, 16, 10);
const CALENDAR: Rect = Rect::new(30, 111, 162, 130);
const CALENDAR_ROW_H: i32 = 18;
const ZONE_LABEL: (i32, i32) = (19, 272);

/// Where Windows 2000's window differs from 95's and 98's (its own copy of the window).
struct EraLayout {
    /// The time field and its two arrow buttons.
    time: Rect,
    time_up: Rect,
    time_down: Rect,
    /// The year's text, right of the field's inside edge.
    year_text_x: i32,
    zone_name_x: i32,
}

/// Where a tab's text goes, below the tab's usual top, for tab `i` while page `chosen` is
/// shown: Windows 2000 like its other property sheets (chosen 1, others 3); Windows 95 and 98
/// as measured on both pages (with Date & Time chosen 2 and 3, with Time Zone chosen 1 and 1).
fn tab_text_y(era: Era, chosen: usize, i: usize) -> i32 {
    match (era == Era::Win2000, chosen == i, chosen) {
        (true, true, _) => 1,
        (true, false, _) => 3,
        (false, true, 0) => 2,
        (false, false, 0) => 3,
        (false, _, _) => 1,
    }
}

fn era_layout(era: Era) -> EraLayout {
    if era == Era::Win2000 {
        EraLayout {
            time: Rect::new(252, 222, 86, 20),
            time_up: Rect::new(339, 222, 16, 9),
            time_down: Rect::new(339, 231, 16, 9),
            year_text_x: 4,
            zone_name_x: 115,
        }
    } else {
        EraLayout {
            time: Rect::new(256, 222, 77, 20),
            time_up: Rect::new(334, 222, 16, 9),
            time_down: Rect::new(334, 231, 16, 9),
            year_text_x: 1,
            zone_name_x: 110,
        }
    }
}

/// The clock face's twelve hour marks (4×4, top left corners) and 48 minute dimples (3×3),
/// as the reference PCs place them.
const HOUR_MARKS: [(i32, i32); 12] = [
    (293, 80),
    (323, 88),
    (346, 110),
    (354, 141),
    (346, 172),
    (323, 194),
    (293, 202),
    (262, 194),
    (240, 172),
    (232, 141),
    (240, 110),
    (263, 88),
];
const MINUTE_DOTS: [(i32, i32); 48] = [
    (288, 81),
    (300, 81),
    (281, 82),
    (307, 82),
    (275, 84),
    (313, 84),
    (269, 86),
    (319, 86),
    (258, 93),
    (330, 93),
    (253, 97),
    (335, 97),
    (249, 101),
    (339, 101),
    (245, 106),
    (343, 106),
    (238, 117),
    (350, 117),
    (236, 123),
    (352, 123),
    (234, 129),
    (354, 129),
    (233, 136),
    (355, 136),
    (233, 148),
    (355, 148),
    (234, 155),
    (354, 155),
    (236, 161),
    (352, 161),
    (238, 167),
    (350, 167),
    (245, 178),
    (343, 178),
    (249, 183),
    (339, 183),
    (253, 187),
    (335, 187),
    (258, 191),
    (330, 191),
    (269, 198),
    (319, 198),
    (275, 200),
    (313, 200),
    (281, 202),
    (307, 202),
    (288, 203),
    (300, 203),
];
/// The hour marks' colors: lit edge, face, shadow (the same in every look).
const MARK_COLORS: (u32, u32, u32) = (rgb(0x00FFFF), rgb(0x008080), rgb(0x000000));

pub fn page_title(page: Page) -> &'static str {
    match page {
        Page::DateTime => "Date & Time",
        Page::TimeZone => "Time Zone",
    }
}

fn button_label(part: Part) -> &'static str {
    match part {
        Part::Ok => "OK",
        Part::Cancel => "Cancel",
        _ => "&Apply",
    }
}

/// The calendar's column `col` (0 = Sunday): the inside split in seven, Windows' way.
fn column(col: i32) -> (i32, i32) {
    let inside_w = CALENDAR.w - 4;
    let left = CALENDAR.x + 2 + col * inside_w / 7;
    let right = CALENDAR.x + 2 + (col + 1) * inside_w / 7;
    (left, right - left)
}

/// The calendar cell of `day` in the month shown: column, and the row's top.
fn day_cell(m: Moment, day: u32) -> (i32, i32) {
    let pos = (weekday(m.year, m.month, 1) + day - 1) as i32;
    (pos % 7, CALENDAR.y + 2 + CALENDAR_ROW_H * (1 + pos / 7))
}

pub fn layout(d: &DateTime, fonts: &Fonts) -> DateTimeLayout {
    let font = fonts.regular;
    let mut tabs = Vec::new();
    let mut x = TAB_X;
    for &page in &PAGES {
        let w = font.measure(page_title(page)) + 2 * TAB_PAD;
        tabs.push(Rect::new(x, TAB_Y, w, TAB_H));
        x += w;
    }
    let mut parts: Vec<(Part, Rect)> = Vec::new();
    if d.current_page() == Page::DateTime {
        parts.extend([
            (Part::Month, MONTH),
            (Part::MonthDrop, MONTH_DROP),
            (Part::Year, YEAR),
            (Part::YearUp, YEAR_UP),
            (Part::YearDown, YEAR_DOWN),
            (Part::Time, era_layout(d.era).time),
            (Part::TimeUp, era_layout(d.era).time_up),
            (Part::TimeDown, era_layout(d.era).time_down),
        ]);
        for day in 1..=days_in_month(d.shown.year, d.shown.month) {
            let (col, top) = day_cell(d.shown, day);
            let (left, w) = column(col);
            parts.push((Part::Day(day), Rect::new(left, top, w, CALENDAR_ROW_H)));
        }
        let inside = era_layout(d.era).time.inset(2, 2, 2, 2);
        for (i, slot) in time_slots(font, inside).into_iter().enumerate() {
            parts.push((Part::TimeField(i), slot));
        }
        if d.month_list {
            for month in 1..=12 {
                parts.push((Part::MonthItem(month), month_item_rect(month)));
            }
        }
    }
    for (part, x) in BUTTONS {
        parts.push((part, Rect::new(x, BUTTONS_Y, 75, 23)));
    }
    for (i, &t) in tabs.iter().enumerate() {
        let r = if i == d.page { Rect::new(t.x - 2, t.y - 2, t.w + 4, t.h + 2) } else { t };
        parts.push((Part::Tab(i), r));
    }
    let (help, close) = dialog::title_button_rects(SIZE.0);
    parts.extend([(Part::TitleHelp, help), (Part::TitleClose, close)]);
    if d.current_page() == Page::TimeZone {
        parts.extend([(Part::Zone, timezone::COMBO), (Part::ZoneDrop, timezone::COMBO_DROP)]);
        if d.dst_shown() {
            let (x, y) = timezone::DST_CHECK;
            let w = 18 + font.measure_label(timezone::DST_LABEL) + 1;
            parts.push((Part::Dst, Rect::new(x, y - 1, w, 16)));
        }
        // Last, so that the dropped list takes clicks over what it covers.
        if d.zone_list {
            let rows = timezone::list_rows_rect();
            let total = timezone::zones(d.era).len();
            for row in 0..timezone::LIST_ROWS.min(total - d.zone_top.min(total)) {
                let r = Rect::new(rows.x, rows.y + row as i32 * timezone::ROW_H, rows.w, timezone::ROW_H);
                parts.push((Part::ZoneRow(d.zone_top + row), r));
            }
            let bar = timezone::scroll_bar_rect();
            let thumb = timezone::thumb_rect(total, d.zone_top);
            parts.extend([
                (Part::ZoneUp, Rect::new(bar.x, bar.y, 16, 16)),
                (Part::ZoneDown, Rect::new(bar.x, bar.bottom() - 16, 16, 16)),
                (Part::ZonePageUp, Rect::new(bar.x, bar.y + 16, 16, thumb.y - bar.y - 16)),
                (Part::ZonePageDown, Rect::new(bar.x, thumb.bottom(), 16, bar.bottom() - 16 - thumb.bottom())),
                (Part::ZoneThumb, thumb),
            ]);
        }
    }
    DateTimeLayout { width: SIZE.0, height: SIZE.1, tabs, page: PAGE, parts }
}

/// Text cut off at `clip`'s edges, as a control's text is.
pub(crate) fn draw_clipped(fb: &mut Framebuffer, font: &Font, (x, y): (i32, i32), text: &str, color: u32, clip: Rect) {
    let mut scratch = Framebuffer::new(clip.w, clip.h, 0);
    font.draw(&mut scratch, x - clip.x, y - clip.y, text, color);
    for j in 0..clip.h {
        for i in 0..clip.w {
            let p = scratch.get(i, j);
            if p != 0 {
                fb.set(clip.x + i, clip.y + j, p);
            }
        }
    }
}

/// The dotted focus rectangle Windows draws over a highlighted item: every other pixel of the
/// frame inverted against the item's colors, so the dots come out light on the window's white
/// and the rest black. Which pixels are dots depends on the position on the screen.
pub(crate) fn item_focus(fb: &mut Framebuffer, r: Rect, screen: (i32, i32), dot: u32) {
    let mut put = |x: i32, y: i32| {
        let odd = (x + y + screen.0 + screen.1).rem_euclid(2) == 1;
        fb.set(x, y, if odd { dot } else { rgb(0x000000) });
    };
    for x in r.x..r.right() {
        put(x, r.y);
        put(x, r.bottom() - 1);
    }
    for y in r.y + 1..r.bottom() - 1 {
        put(r.x, y);
        put(r.right() - 1, y);
    }
}

/// An arrow button of a spin control (16 wide; 10 or 9 high) with its 3×2 arrow.
fn spin_button(fb: &mut Framebuffer, c: &Colors, r: Rect, up: bool, pressed: bool) {
    fb.fill(r, c.face);
    if pressed {
        fb.frame(r, c.shadow, c.shadow);
    } else {
        fb.frame(r, c.light, c.dark_shadow);
        fb.frame(r.inset(1, 1, 1, 1), c.highlight, c.shadow);
    }
    let s = i32::from(pressed);
    let (x, y) = (r.x + 6 + s, r.y + s);
    // Where Windows' arrow glyph lands in a button this high.
    let (tip, base) = match (up, r.h) {
        (true, _) => (4, 5),
        (false, 10) => (5, 4),
        (false, _) => (4, 3),
    };
    fb.set(x + 1, y + tip, c.text);
    fb.hline(x, y + base, 3, c.text);
}

/// Paints the whole window filling the framebuffer (`layout`'s size).
pub fn paint(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, d: &DateTime, l: &DateTimeLayout) {
    let c = &theme.colors;
    let font = fonts.regular;
    let keys = d.show_keys;
    let held = |part: Part| d.pressed == Some(part);
    let buttons = TitleButtons {
        help: l.rect(Part::TitleHelp).map(|r| (r, held(Part::TitleHelp))),
        close: l.rect(Part::TitleClose).map(|r| (r, held(Part::TitleClose))),
    };
    dialog::window_frame(fb, theme, fonts, d.era, TITLE, &buttons);

    // Tabs as in the Properties window: the others, the page's frame, then the chosen one.
    // (Unlike the Properties window's newer tabs, the chosen tab's text does not move up.)
    let tab_text = |fb: &mut Framebuffer, i: usize, top: i32| {
        font.draw(fb, l.tabs[i].x + TAB_PAD, top, page_title(PAGES[i]), c.text);
    };
    for (i, &t) in l.tabs.iter().enumerate() {
        if i != d.page {
            crate::properties::tab(fb, c, t);
            tab_text(fb, i, t.y + tab_text_y(d.era, d.page, i));
        }
    }
    fb.frame(l.page, c.highlight, c.dark_shadow);
    fb.frame(l.page.inset(1, 1, 1, 1), c.light, c.shadow);
    if let Some(&t) = l.tabs.get(d.page) {
        let drawn = Rect::new(t.x - 2, t.y - 2, t.w + 4, t.h + 3);
        crate::properties::tab(fb, c, drawn);
        tab_text(fb, d.page, t.y + tab_text_y(d.era, d.page, d.page));
        // It also covers the page frame's inner line below it.
        // (Windows 95 and 98: from the tab's own left edge unless it is the first tab.)
        let from = if d.page > 0 && d.era != Era::Win2000 { drawn.x } else { drawn.x + 2 };
        fb.hline(from, l.page.y + 1, drawn.right() - from, c.face);
        if d.focus == Part::Tab(d.page) && keys {
            dialog::focus_dots(fb, t.inset(1, 1, 1, 1), (0, 0), c.text);
        }
    }

    if d.current_page() == Page::DateTime {
        paint_date_time(fb, c, font, d);
    } else {
        paint_time_zone(fb, c, font, d);
    }
    if d.month_list && d.current_page() == Page::DateTime {
        paint_month_list(fb, c, font, d.shown.month);
    }

    for (part, _) in BUTTONS {
        if let Some(r) = l.rect(part) {
            let focused = d.focus == part && keys;
            dialog::push_button(
                fb,
                theme,
                font,
                r,
                button_label(part),
                part == Part::Ok,
                held(part),
                focused,
                d.enabled(part),
                keys,
            );
        }
    }
    if d.zone_list && d.current_page() == Page::TimeZone {
        let pressed = |dir: i32| d.pressed == Some(if dir < 0 { Part::ZoneUp } else { Part::ZoneDown });
        timezone::paint_list(fb, c, font, d.era, (d.zone_top, d.zone_hover.unwrap_or(d.zone)), d.screen, pressed);
    }
}

/// The Time Zone page: the zone's drop-down list (highlighted with the dotted rectangle around
/// it while it has the focus, as the month's), the world map (Windows 2000 turned to the
/// chosen zone), and the daylight saving box: grayed out for a zone without daylight saving
/// time (95, 98), or left out (2000).
fn paint_time_zone(fb: &mut Framebuffer, c: &Colors, font: &Font, d: &DateTime) {
    let white = rgb(0xFFFFFF);
    let list = timezone::zones(d.era);
    let label = list.get(d.zone).map_or("", |z| z.label);
    let inside = dialog::well(fb, timezone::COMBO, c);
    let item = timezone::COMBO_ITEM;
    if d.focus == Part::Zone && !d.zone_list {
        fb.fill(item, c.menu_highlight);
        draw_clipped(fb, font, (item.x, item.y), label, c.menu_highlight_text, item);
        item_focus(fb, item.inset(-1, -1, -1, -1), d.screen, white ^ c.menu_highlight | rgb(0));
    } else {
        let clip = Rect::new(inside.x, inside.y, timezone::COMBO_DROP.x - inside.x, inside.h);
        draw_clipped(fb, font, (item.x, item.y), label, c.text, clip);
    }
    dialog::drop_down(fb, timezone::COMBO_DROP, c, d.pressed == Some(Part::ZoneDrop));

    let offset = list.get(d.zone).map_or(0, |z| timezone::label_offset(z.label));
    timezone::paint_map(fb, c, d.era, timezone::map_shift(d.era, offset));

    if d.dst_shown() {
        let (x, y) = timezone::DST_CHECK;
        let enabled = d.zone_dst_possible();
        crate::properties::check_box(fb, c, x, y, d.dst, d.pressed == Some(Part::Dst), enabled);
        let (tx, keys) = (x + 18, d.show_keys);
        if enabled {
            font.draw_label(fb, (tx, y), timezone::DST_LABEL, i32::MAX, c.text, keys);
        } else {
            font.draw_label(fb, (tx + 1, y + 1), timezone::DST_LABEL, i32::MAX, c.etch, keys);
            font.draw_label(fb, (tx, y), timezone::DST_LABEL, i32::MAX, c.shadow, keys);
        }
        if d.focus == Part::Dst && keys {
            let w = font.measure_label(timezone::DST_LABEL);
            dialog::focus_dots(fb, Rect::new(tx - 1, y - 1, w + 2, font.height + 3), (x, y - 1), c.text);
        }
    }
}

fn paint_date_time(fb: &mut Framebuffer, c: &Colors, font: &Font, d: &DateTime) {
    let keys = d.show_keys;
    let white = rgb(0xFFFFFF);
    let m = d.shown;
    let e = era_layout(d.era);
    crate::properties::group_box(fb, c, font, DATE_GROUP, "&Date", keys);
    crate::properties::group_box(fb, c, font, TIME_GROUP, "&Time", keys);

    // The month: a drop-down list; highlighted, with the dotted rectangle, while focused.
    let inside = dialog::well(fb, MONTH, c);
    let month = MONTHS[(m.month as usize).clamp(1, 12) - 1];
    let item_clip = Rect::new(inside.x, inside.y, MONTH_DROP.x - inside.x, inside.h);
    if d.focus == Part::Month {
        fb.fill(MONTH_ITEM, c.menu_highlight);
        draw_clipped(fb, font, (MONTH_ITEM.x, MONTH_ITEM.y), month, c.menu_highlight_text, MONTH_ITEM);
        item_focus(fb, MONTH_ITEM.inset(-1, -1, -1, -1), d.screen, white ^ c.menu_highlight | rgb(0));
    } else {
        draw_clipped(fb, font, (MONTH_ITEM.x, MONTH_ITEM.y), month, c.text, item_clip);
    }
    dialog::drop_down(fb, MONTH_DROP, c, d.pressed == Some(Part::MonthDrop));

    // The year, with its arrows.
    let inside = dialog::well(fb, YEAR, c);
    font.draw(fb, inside.x + e.year_text_x, inside.y + 1, &m.year.to_string(), c.text);
    spin_button(fb, c, YEAR_UP, true, d.pressed == Some(Part::YearUp));
    spin_button(fb, c, YEAR_DOWN, false, d.pressed == Some(Part::YearDown));

    // The calendar: the weekdays on a gray band, then the days; the chosen one highlighted.
    dialog::well(fb, CALENDAR, c);
    let band = Rect::new(CALENDAR.x + 2, CALENDAR.y + 2, CALENDAR.w - 4, CALENDAR_ROW_H);
    fb.fill(band, c.shadow);
    for (col, name) in WEEKDAYS.iter().enumerate() {
        let (left, _) = column(col as i32);
        font.draw(fb, left + 5, band.y + 3, name, c.face);
    }
    let digit_w = font.measure("0");
    for day in 1..=days_in_month(m.year, m.month) {
        let (col, top) = day_cell(m, day);
        let (left, w) = column(col);
        let text = day.to_string();
        let tw = font.measure(&text);
        let x = left + (w - tw) / 2;
        let y = top + 3;
        let mut ink = c.text;
        if day == m.day {
            // As wide as a two-digit day, ending where this day's digits end.
            let box_left = left + (w - 2 * digit_w) / 2;
            let r = Rect::new(box_left, y, x + tw - box_left, font.height);
            if d.focus == Part::Day(day) {
                fb.fill(r.inset(1, 1, 1, 1), c.menu_highlight);
                item_focus(fb, r, d.screen, white ^ c.menu_highlight | rgb(0));
            } else {
                fb.fill(r, c.menu_highlight);
            }
            ink = c.menu_highlight_text;
        }
        font.draw(fb, x, y, &text, ink);
    }

    // The clock face: lit hour marks and sunken minute dimples.
    let (lit, face, dark) = MARK_COLORS;
    for &(x, y) in &HOUR_MARKS {
        fb.fill(Rect::new(x, y, 4, 4), dark);
        fb.hline(x, y, 3, lit);
        fb.vline(x, y, 4, lit);
        fb.fill(Rect::new(x + 1, y + 1, 2, 2), face);
    }
    for &(x, y) in &MINUTE_DOTS {
        fb.hline(x, y, 2, c.shadow);
        fb.set(x, y + 1, c.shadow);
        fb.set(x + 2, y + 1, c.highlight);
        fb.hline(x + 1, y + 2, 2, c.highlight);
    }

    paint_hands(fb, c, m);

    // The time, with its arrows.
    let inside = dialog::well(fb, e.time, c);
    paint_time(fb, c, font, inside, d);
    spin_button(fb, c, e.time_up, true, d.pressed == Some(Part::TimeUp));
    spin_button(fb, c, e.time_down, false, d.pressed == Some(Part::TimeDown));

    font.draw(fb, ZONE_LABEL.0, ZONE_LABEL.1, "Current time zone:", c.text);
    font.draw(fb, e.zone_name_x, ZONE_LABEL.1, &d.zone_name, c.text);
}

/// The time as the window's time field draws it: each part in a slot of its own (two digits
/// wide), a colon's dot 2px after a slot, the next slot 6px after it, AM/PM 2px after the
/// seconds. A number ends 1px short of its slot's end, but never starts before the slot (so two
/// digits fill it). While the time has the focus (Windows 98): the chosen number is highlighted as
/// wide as its digits, or, once changed with the arrows or typed into, shows unpadded and not
/// highlighted; a 2px caret blinks over its slot's last two columns. A chosen AM/PM is
/// highlighted from the seconds' slot to the field's end, with a dotted rectangle and no caret.
fn paint_time(fb: &mut Framebuffer, c: &Colors, font: &Font, inside: Rect, d: &DateTime) {
    let m = d.shown;
    let slots = time_slots(font, inside);
    let chosen = (d.focus == Part::Time).then_some(d.time_field.min(3));
    let h12 = if m.hour.is_multiple_of(12) { 12 } else { m.hour % 12 };
    let numbers = [h12, m.minute, m.second];
    let y = slots[0].y;
    let highlighted = |i: usize| chosen == Some(i) && (i == 3 || d.entry == TimeEntry::Selected);
    for (i, &n) in numbers.iter().enumerate() {
        let text = match (&d.entry, chosen == Some(i)) {
            (TimeEntry::Typed(t), true) => t.clone(),
            (TimeEntry::Stepped, true) => n.to_string(),
            _ if i == 0 => n.to_string(),
            _ => format!("{n:02}"),
        };
        let w = font.measure(&text);
        let x = (slots[i].right() - 1 - w).max(slots[i].x);
        let ink = if highlighted(i) {
            fb.fill(Rect::new(x, y, w, font.height), c.menu_highlight);
            c.menu_highlight_text
        } else {
            c.text
        };
        font.draw(fb, x, y, &text, ink);
    }
    let colon_x = font.glyph(':').ink_x;
    font.draw(fb, slots[0].right() + 2 - colon_x, y, ":", c.text);
    font.draw(fb, slots[1].right() + 2 - colon_x, y, ":", c.text);
    let am_pm = if m.hour < 12 { "AM" } else { "PM" };
    if highlighted(3) {
        let r = Rect::new(slots[2].right(), y, inside.right() - 1 - slots[2].right(), font.height);
        fb.fill(r, c.menu_highlight);
        font.draw(fb, slots[3].x, y, am_pm, c.menu_highlight_text);
        invert_dots(fb, r, d.screen);
    } else {
        font.draw(fb, slots[3].x, y, am_pm, c.text);
    }
    if let Some(i @ 0..=2) = chosen
        && d.caret
    {
        let x = slots[i].right() - 2;
        for yy in y..y + font.height {
            for xx in x..x + 2 {
                fb.set(xx, yy, (fb.get(xx, yy) ^ 0x00FF_FFFF) | rgb(0));
            }
        }
    }
}

/// Windows' DrawFocusRect over a highlighted item: every other pixel of the frame inverted
/// (which ones depends on the position on the screen).
pub(crate) fn invert_dots(fb: &mut Framebuffer, r: Rect, screen: (i32, i32)) {
    let mut flip = |x: i32, y: i32| {
        if (x + y + screen.0 + screen.1).rem_euclid(2) == 1 {
            fb.set(x, y, (fb.get(x, y) ^ 0x00FF_FFFF) | rgb(0));
        }
    };
    for x in r.x..r.right() {
        flip(x, r.y);
        flip(x, r.bottom() - 1);
    }
    for y in r.y + 1..r.bottom() - 1 {
        flip(r.x, y);
        flip(r.right() - 1, y);
    }
}

/// Where the time field puts its parts: hour, minutes, seconds (each two digits wide, 6px
/// apart) and AM/PM (2px after the seconds), one text line high.
fn time_slots(font: &Font, inside: Rect) -> [Rect; 4] {
    let w = 2 * font.measure("0");
    let (x, y, h) = (inside.x + 1, inside.y + 1, font.height);
    [
        Rect::new(x, y, w, h),
        Rect::new(x + w + 6, y, w, h),
        Rect::new(x + 2 * (w + 6), y, w, h),
        Rect::new(x + 3 * w + 14, y, font.measure("PM"), h),
    ]
}

/// The month's open list: under the field, as wide, one row per month. PROVISIONAL: not
/// yet read from a reference PC.
fn month_item_rect(month: u32) -> Rect {
    Rect::new(MONTH.x + 1, MONTH.bottom() + 1 + 13 * (month as i32 - 1), MONTH.w - 2, 13)
}

fn paint_month_list(fb: &mut Framebuffer, c: &Colors, font: &Font, chosen: u32) {
    let r = Rect::new(MONTH.x, MONTH.bottom(), MONTH.w, 12 * 13 + 2);
    fb.frame(r, c.text, c.text);
    fb.fill(r.inset(1, 1, 1, 1), rgb(0xFFFFFF));
    for month in 1..=12 {
        let row = month_item_rect(month);
        let mut ink = c.text;
        if month == chosen {
            fb.fill(row, c.menu_highlight);
            ink = c.menu_highlight_text;
        }
        font.draw(fb, row.x + 2, row.y, MONTHS[month as usize - 1], ink);
    }
}

/// The clock's middle: the corner between four pixels (pixel centers sit at .5). The hands are
/// drawn 1/4 px lower (fitted: `HANDS_Y`); the second hand starts at the pixel right and below.
const CLOCK_CENTER: (f64, f64) = (295.0, 143.0);
const HANDS_Y: f64 = 143.25;

/// A hand's outline: its tip, a side on each hand, the end of its short tail (all along the
/// hand from the middle, sides `half` across it at `side_at`).
#[derive(Clone, Copy)]
struct Hand {
    tip: f64,
    half: f64,
    side_at: f64,
    tail: f64,
}

/// Fitted to 181 captures of Windows 98's clock (every minute, hour and second position):
/// about 135 of the face's 10816 pixels still differ per capture, along the hands' edges, so
/// Windows' own way of filling them is not found yet (private/todo.md).
const MINUTE_HAND: Hand = Hand { tip: 48.25, half: 3.0, side_at: 0.0, tail: 11.5 };
const HOUR_HAND: Hand = Hand { tip: 40.75, half: 4.0, side_at: -0.25, tail: 9.25 };

/// Where the second hand's line ends for each second, measured on Windows 98 (from the pixel
/// right and below the middle; that end is not drawn). Windows' own rounding is irregular, so
/// this is a table rather than a formula.
const SECOND_TIPS: [(i32, i32); 60] = [
    (0, -49), (5, -49), (10, -48), (15, -47), (20, -45), (24, -43), (29, -40), (33, -37), (36, -33), (40, -29),
    (42, -25), (45, -20), (47, -15), (48, -10), (49, -5), (49, 0), (49, 5), (48, 10), (47, 15), (45, 20),
    (42, 25), (40, 29), (36, 33), (33, 37), (29, 40), (24, 43), (20, 45), (15, 47), (10, 48), (5, 49),
    (0, 49), (-5, 49), (-10, 48), (-15, 47), (-20, 45), (-25, 43), (-29, 40), (-33, 37), (-36, 33), (-40, 29),
    (-42, 25), (-45, 20), (-47, 15), (-48, 10), (-49, 5), (-49, 0), (-49, -5), (-48, -10), (-47, -15), (-45, -20),
    (-42, -25), (-40, -29), (-36, -33), (-33, -37), (-29, -40), (-24, -43), (-20, -45), (-15, -47), (-10, -48), (-5, -49),
];

/// The hands, as Windows 98 draws them: the minute hand, then the hour hand on top, each a teal
/// shape over a white copy 2px up and left and a gray (shadow) copy 2px down and right; then the
/// second hand, a line that inverts what is under it.
fn paint_hands(fb: &mut Framebuffer, c: &Colors, m: Moment) {
    let hour = (m.hour % 12) * 5 + m.minute / 12;
    for (position, hand) in [(m.minute, MINUTE_HAND), (hour, HOUR_HAND)] {
        fill_polygon(fb, &hand_outline(position, hand, 2.0, 2.0), c.shadow);
        fill_polygon(fb, &hand_outline(position, hand, -2.0, -2.0), c.highlight);
        fill_polygon(fb, &hand_outline(position, hand, 0.0, 0.0), rgb(0x008080));
    }
    let start = (CLOCK_CENTER.0 as i32, CLOCK_CENTER.1 as i32);
    let (dx, dy) = SECOND_TIPS[m.second as usize % 60];
    for (x, y) in line_pixels(start, (start.0 + dx, start.1 + dy)) {
        fb.set(x, y, (fb.get(x, y) ^ 0x00FF_FFFF) | rgb(0));
    }
}

/// A hand's outline at `position` (sixtieths of a turn clockwise from 12), moved by (dx, dy).
fn hand_outline(position: u32, hand: Hand, dx: f64, dy: f64) -> [(f64, f64); 4] {
    let (s, k) = (f64::from(position) * std::f64::consts::TAU / 60.0).sin_cos();
    let at = |along: f64, across: f64| (CLOCK_CENTER.0 + along * s + across * k + dx, HANDS_Y - along * k + across * s + dy);
    [at(hand.tip, 0.0), at(hand.side_at, hand.half), at(-hand.tail, 0.0), at(hand.side_at, -hand.half)]
}

/// Fills a polygon by Windows' rule: pixels whose centers are inside (even-odd).
fn fill_polygon(fb: &mut Framebuffer, points: &[(f64, f64)], color: u32) {
    let top = points.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor() as i32;
    let bottom = points.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil() as i32;
    for y in top..bottom {
        let yc = f64::from(y) + 0.5;
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            if (a.1 <= yc) != (b.1 <= yc) {
                xs.push(a.0 + (yc - a.1) * (b.0 - a.0) / (b.1 - a.1));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks(2) {
            if let [l, r] = *pair {
                let (from, to) = ((l - 0.5).ceil() as i32, (r - 0.5).ceil() as i32);
                if to > from {
                    fb.hline(from, y, to - from, color);
                }
            }
        }
    }
}

/// The pixels of a 1px line from `from` to `to`, the end left out, as Windows draws it
/// (Bresenham; where the ideal line runs exactly between two pixels, the choice depends on the
/// direction, as measured on the second hand: steps toward the top are taken early).
fn line_pixels(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = ((to.0 - from.0).abs(), (to.1 - from.1).abs());
    let (sx, sy) = ((to.0 - from.0).signum(), (to.1 - from.1).signum());
    let y_major = dx < dy;
    let (major, minor) = if y_major { (dy, dx) } else { (dx, dy) };
    // At a tie the minor step is taken at once when the line goes up, late when it goes down.
    let tie_steps = sy < 0;
    let (mut x, mut y) = from;
    let mut e = 2 * minor - major;
    let mut out = Vec::with_capacity(major as usize);
    for _ in 0..major {
        out.push((x, y));
        if e > 0 || (e == 0 && tie_steps) {
            if y_major {
                x += sx;
            } else {
                y += sy;
            }
            e -= 2 * major;
        }
        e += 2 * minor;
        if y_major {
            y += sy;
        } else {
            x += sx;
        }
    }
    out
}

/// Date/Time Properties as a window of the control kit (`kit::Form`).
pub struct DateTimeWindow {
    pub dt: DateTime,
    pub layout: DateTimeLayout,
}

/// Rows the mouse wheel scrolls the dropped zone list per notch (Windows' default).
const WHEEL_ROWS: i32 = 3;

impl DateTimeWindow {
    pub fn new(dt: DateTime, fonts: &Fonts) -> DateTimeWindow {
        let layout = layout(&dt, fonts);
        DateTimeWindow { dt, layout }
    }

    /// Shows the other page; `focus_tab`: the focus on its tab (an arrow key on the tabs).
    fn page(&mut self, page: usize, focus_tab: bool) {
        self.dt.show_page(page);
        if focus_tab {
            self.dt.focus = Part::Tab(page);
        }
    }

    /// A click while the zone's list is dropped down: its rows choose and close it, its scroll
    /// bar scrolls it, anything else just closes it.
    fn press_in_zone_list(&mut self, hit: Option<Part>, x: i32, y: i32) -> Press<Part> {
        let rows = timezone::LIST_ROWS as i32;
        let d = &mut self.dt;
        match hit {
            Some(Part::ZoneRow(i)) => {
                d.choose_zone(i);
                d.close_zones();
            }
            Some(part @ (Part::ZoneUp | Part::ZoneDown)) => {
                d.scroll_zones(if part == Part::ZoneUp { -1 } else { 1 });
                return Press::Hold(part);
            }
            Some(Part::ZonePageUp) => d.scroll_zones(-rows),
            Some(Part::ZonePageDown) => d.scroll_zones(rows),
            Some(Part::ZoneThumb) => return Press::Grab(Part::ZoneThumb, d.zone_top as i32),
            // Inside the list's frame but on no row: nothing.
            _ if timezone::LIST.contains(x, y) => {}
            _ => d.close_zones(),
        }
        Press::Done
    }
}

impl kit::Form for DateTimeWindow {
    type Part = Part;

    fn title(&self) -> &str {
        TITLE
    }
    fn size(&self) -> (i32, i32) {
        (self.layout.width, self.layout.height)
    }
    fn layout(&mut self, fonts: &Fonts) {
        self.layout = layout(&self.dt, fonts);
    }
    fn paint(&self, fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts) {
        paint(fb, theme, fonts, &self.dt, &self.layout);
    }
    fn set_screen(&mut self, x: i32, y: i32) {
        self.dt.screen = (x, y);
    }

    fn hit(&self, x: i32, y: i32) -> Option<Part> {
        self.layout.hit(x, y)
    }
    fn rect(&self, part: Part) -> Option<Rect> {
        self.layout.rect(part)
    }
    fn enabled(&self, part: Part) -> bool {
        self.dt.enabled(part)
    }
    fn focus(&self) -> Part {
        self.dt.focus
    }
    /// Tab closes an open list; arriving at the time chooses its hour.
    fn set_focus(&mut self, part: Part) {
        let d = &mut self.dt;
        d.month_list = false;
        d.close_zones();
        d.focus = part;
        if part == Part::Time {
            d.select_time_part(0);
        }
    }
    /// The controls Tab goes through on the page shown, in Windows' order (grayed out ones left
    /// out).
    fn tab_stops(&self) -> Vec<Part> {
        let d = &self.dt;
        let mut stops = vec![Part::Tab(d.page)];
        if d.current_page() == Page::DateTime {
            stops.extend([Part::Month, Part::Year, Part::Day(d.shown.day), Part::Time]);
        } else {
            stops.push(Part::Zone);
            if d.dst_shown() && d.enabled(Part::Dst) {
                stops.push(Part::Dst);
            }
        }
        stops.extend([Part::Ok, Part::Cancel]);
        if d.enabled(Part::Apply) {
            stops.push(Part::Apply);
        }
        stops
    }
    /// The zone list's thumb is grabbed, not pushed in.
    fn set_pressed(&mut self, part: Option<Part>) {
        self.dt.pressed = part.filter(|&p| p != Part::ZoneThumb);
    }
    fn show_keys(&mut self) {
        self.dt.show_keys = true;
    }
    fn enter(&self) -> Part {
        if matches!(self.dt.focus, Part::Cancel | Part::Apply) { self.dt.focus } else { Part::Ok }
    }
    fn escape(&self) -> Part {
        Part::Cancel
    }

    fn key(&mut self, key: Key, mods: Mods) -> Option<Vec<Effect<Part>>> {
        let n_pages = PAGES.len();
        let (page, focus) = (self.dt.page, self.dt.focus);
        let next_page = |forward: bool| if forward { (page + 1) % n_pages } else { (page + n_pages - 1) % n_pages };
        let d = &mut self.dt;
        match key {
            Key::Tab if mods.ctrl => self.page(next_page(!mods.shift), false),
            Key::PageUp | Key::PageDown if mods.ctrl => self.page(next_page(true), false),
            Key::Escape | Key::Enter if d.month_list => d.month_list = false,
            Key::Escape | Key::Enter | Key::F4 if d.zone_list => d.close_zones(),
            Key::F4 if focus == Part::Zone => d.open_zones(),
            // The zone's list: the arrows, Page Up/Down, Home and End choose another entry at
            // once, whether the list is dropped down or not.
            Key::Up | Key::Left | Key::Down | Key::Right | Key::PageUp | Key::PageDown | Key::Home | Key::End
                if focus == Part::Zone =>
            {
                let rows = timezone::LIST_ROWS as i32 - 1;
                d.zone_hover = None;
                match key {
                    Key::Up | Key::Left => d.step_zone(-1),
                    Key::Down | Key::Right => d.step_zone(1),
                    Key::PageUp => d.step_zone(-rows),
                    Key::PageDown => d.step_zone(rows),
                    Key::Home => d.choose_zone(0),
                    _ => d.choose_zone(usize::MAX),
                }
            }
            // In the time, Tab goes through hour, minutes, seconds and AM/PM before leaving it.
            Key::Tab if focus == Part::Time && !mods.shift && d.time_field < 3 => d.select_time_part(d.time_field + 1),
            Key::Tab if focus == Part::Time && mods.shift && d.time_field > 0 => d.select_time_part(d.time_field - 1),
            Key::Up | Key::Down | Key::Left | Key::Right => {
                let forward = matches!(key, Key::Down | Key::Right);
                match focus {
                    Part::Tab(_) if matches!(key, Key::Left | Key::Right) => self.page(next_page(forward), true),
                    Part::Month => d.step_month(if forward { 1 } else { -1 }),
                    Part::Year => d.step_year(if forward { -1 } else { 1 }),
                    Part::Day(_) => {
                        d.step_day(match key {
                            Key::Left => -1,
                            Key::Right => 1,
                            Key::Up => -7,
                            _ => 7,
                        });
                        d.focus = Part::Day(d.shown.day);
                    }
                    Part::Time if matches!(key, Key::Up | Key::Down) => d.step_time(if key == Key::Up { 1 } else { -1 }),
                    // Windows 98's time does not move between its parts with Left and Right.
                    _ => {}
                }
            }
            _ => return None,
        }
        Some(Vec::new())
    }
    /// Alt+Up or Alt+Down on the zone's list drops it down or closes it.
    fn alt_key(&mut self, key: Key) -> bool {
        let d = &mut self.dt;
        if d.focus != Part::Zone || !matches!(key, Key::Up | Key::Down) {
            return false;
        }
        if d.zone_list { d.close_zones() } else { d.open_zones() }
        true
    }
    /// Digits go into the time's chosen part, A and P into its AM/PM; any other letter (and
    /// these anywhere else) is an access key.
    fn typed(&mut self, ch: char) -> Vec<Effect<Part>> {
        let d = &mut self.dt;
        if d.focus == Part::Time {
            if let Some(digit) = ch.to_digit(10) {
                d.type_digit(digit);
                return Vec::new();
            }
            if d.time_field == 3 && matches!(ch.to_ascii_lowercase(), 'a' | 'p') {
                d.type_am_pm(ch.eq_ignore_ascii_case(&'p'));
                return Vec::new();
            }
        }
        if !ch.is_alphabetic() {
            return Vec::new();
        }
        d.show_keys = true;
        self.access(ch)
    }
    fn access(&mut self, ch: char) -> Vec<Effect<Part>> {
        let d = &mut self.dt;
        match (d.current_page(), ch.to_ascii_lowercase()) {
            (Page::DateTime, 'd') => d.focus = Part::Month,
            (Page::DateTime, 't') => {
                d.focus = Part::Time;
                d.select_time_part(0);
            }
            // "Automatically adjust clock for &daylight saving changes": flips it.
            (Page::TimeZone, 'd') if d.dst_shown() && d.enabled(Part::Dst) => {
                d.close_zones();
                d.focus = Part::Dst;
                d.toggle_dst();
            }
            (_, 'a') => return kit::activate(self, Part::Apply),
            _ => {}
        }
        Vec::new()
    }
    fn space(&mut self) -> Vec<Effect<Part>> {
        match self.dt.focus {
            Part::Dst => self.dt.toggle_dst(),
            f @ (Part::Ok | Part::Cancel | Part::Apply) => return kit::activate(self, f),
            _ => {}
        }
        Vec::new()
    }
    fn press(&mut self, part: Option<Part>, x: i32, y: i32) -> Press<Part> {
        // A click anywhere closes an open month list (choosing a month if it was on one).
        if self.dt.month_list {
            self.dt.month_list = false;
            if let Some(Part::MonthItem(m)) = part {
                self.dt.choose_month(m);
            }
            return Press::Done;
        }
        if self.dt.zone_list {
            return self.press_in_zone_list(part, x, y);
        }
        let d = &mut self.dt;
        match part {
            Some(Part::Tab(i)) => self.page(i, true),
            Some(Part::Month | Part::MonthDrop) => {
                d.focus = Part::Month;
                d.month_list = true;
            }
            Some(Part::Zone | Part::ZoneDrop) => {
                d.focus = Part::Zone;
                d.open_zones();
            }
            Some(Part::Dst) if d.enabled(Part::Dst) => {
                d.focus = Part::Dst;
                return Press::Hold(Part::Dst);
            }
            Some(Part::Year) => d.focus = Part::Year,
            Some(Part::Day(day)) => {
                d.choose_day(day);
                d.focus = Part::Day(day);
            }
            Some(Part::TimeField(i)) => {
                d.focus = Part::Time;
                d.select_time_part(i);
            }
            Some(Part::Time) if d.focus != Part::Time => {
                d.focus = Part::Time;
                d.select_time_part(0);
            }
            // Arrow buttons act as they go down, like Windows' spin controls.
            Some(part @ (Part::YearUp | Part::YearDown)) => {
                d.focus = Part::Year;
                d.step_year(if part == Part::YearUp { 1 } else { -1 });
                return Press::Hold(part);
            }
            Some(part @ (Part::TimeUp | Part::TimeDown)) => {
                if d.focus != Part::Time {
                    d.focus = Part::Time;
                    d.select_time_part(0);
                }
                d.step_time(if part == Part::TimeUp { 1 } else { -1 });
                return Press::Hold(part);
            }
            Some(part @ (Part::Ok | Part::Cancel | Part::Apply | Part::TitleHelp | Part::TitleClose))
                if d.enabled(part) =>
            {
                if matches!(part, Part::Ok | Part::Cancel | Part::Apply) {
                    d.focus = part;
                }
                return Press::Hold(part);
            }
            Some(_) => {}
            None => return Press::Nothing,
        }
        Press::Done
    }
    fn release(&mut self, part: Part) -> Vec<Effect<Part>> {
        match part {
            Part::Dst => {
                self.dt.toggle_dst();
                Vec::new()
            }
            Part::Ok | Part::Cancel | Part::Apply | Part::TitleHelp | Part::TitleClose => vec![Effect::Activate(part)],
            _ => Vec::new(),
        }
    }
    /// Dragging the zone list's thumb: the rows follow, one per (track - thumb) / (rows - page).
    fn drag(&mut self, _part: Part, grab_y: i32, grab_top: i32, _x: i32, y: i32) -> bool {
        let d = &mut self.dt;
        let total = timezone::zones(d.era).len();
        let thumb = timezone::thumb_rect(total, d.zone_top).h;
        let track = timezone::scroll_bar_rect().h - 32;
        let span = (track - thumb).max(1);
        let range = total.saturating_sub(timezone::LIST_ROWS) as i32;
        let dy = y - grab_y;
        let delta = (dy * range + dy.signum() * span / 2) / span;
        let top = (grab_top + delta).clamp(0, range) as usize;
        let changed = top != d.zone_top;
        d.zone_top = top;
        changed
    }
    /// Over the dropped zone list, its row under the pointer is highlighted.
    fn hover(&mut self, x: i32, y: i32) -> bool {
        if !self.dt.zone_list {
            return false;
        }
        let hover = match self.layout.hit(x, y) {
            Some(Part::ZoneRow(i)) => Some(i),
            _ => self.dt.zone_hover,
        };
        let changed = hover != self.dt.zone_hover;
        self.dt.zone_hover = hover;
        changed
    }
    /// The dropped zone list scrolls; otherwise the focused list or field steps.
    fn wheel(&mut self, notches: i32) {
        let d = &mut self.dt;
        match d.focus {
            _ if d.zone_list => d.scroll_zones(-notches * WHEEL_ROWS),
            Part::Zone => d.step_zone(-notches),
            Part::Month => d.step_month(-notches),
            Part::Year => d.step_year(notches),
            Part::Time => d.step_time(notches),
            _ => {}
        }
    }
    fn has_caret(&self) -> bool {
        self.dt.focus == Part::Time
    }
    fn blink(&mut self, on: bool) {
        self.dt.caret = on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_arithmetic() {
        // 8 November 2026 is a Sunday; 1 January 2000 a Saturday; 29 February 2000 exists.
        assert_eq!(weekday(2026, 11, 8), 0);
        assert_eq!(weekday(2026, 11, 1), 0);
        assert_eq!(weekday(2000, 1, 1), 6);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2026, 11), 30);
        let m = Moment { year: 2026, month: 11, day: 8, hour: 22, minute: 35, second: 18 };
        assert_eq!(Moment::from_seconds(m.to_seconds()), m);
        assert_eq!(Moment::from_seconds(m.to_seconds() + 3 * 3600), Moment { day: 9, hour: 1, ..m });
        let leap = Moment { year: 2000, month: 2, day: 29, hour: 0, minute: 0, second: 0 };
        assert_eq!(Moment::from_seconds(leap.to_seconds()), leap);
        assert_eq!(m.with_date(2026, 2, 31).day, 28);
    }

    /// The keys typed on the Windows 98 reference PC, with what its time field did.
    #[test]
    fn typing_into_the_time_as_windows_98_does() {
        let start = Moment { year: 2026, month: 11, day: 9, hour: 7, minute: 54, second: 46 };
        let mut d = DateTime::new(Era::Win98, start, String::new());
        d.focus = Part::Time;
        d.select_time_part(0);
        assert!(d.type_digit(3), "a digit replaces the chosen part");
        assert!(!d.type_digit(1) && !d.type_digit(2), "31 and 32 are no hours: ignored");
        assert_eq!(d.shown.hour, 3);
        assert_eq!(d.entry, TimeEntry::Typed("3".to_string()));
        d.select_time_part(1);
        assert!(d.type_digit(5) && d.shown.minute == 5);
        assert!(d.type_digit(9) && d.shown.minute == 59);
        assert!(!d.type_digit(7), "a third digit is ignored");
        d.select_time_part(2);
        assert!(d.type_digit(0) && d.type_digit(4) && d.shown.second == 4);
        d.select_time_part(3);
        assert!(!d.type_digit(1), "AM/PM takes no digits");
        assert!(d.type_am_pm(true) && d.shown.hour == 15);
        assert!(d.type_am_pm(false) && d.shown.hour == 3);
        d.select_time_part(0);
        assert!(d.type_digit(1) && d.type_digit(1) && d.shown.hour == 11);
        d.select_time_part(0);
        assert!(d.type_digit(1) && d.type_digit(2) && d.shown.hour == 12, "12 is noon, even typed in the morning");
        assert!(!d.type_digit(0), "and 0 alone is no hour");
        assert!(d.changed && d.editing);
        d.select_time_part(1);
        d.step_time(1);
        assert_eq!((d.shown.minute, d.entry.clone()), (0, TimeEntry::Stepped));
        assert!(d.type_digit(7) && d.shown.minute == 7, "after the arrows a digit starts afresh");
    }

    #[test]
    fn time_reads_as_windows_wrote_it() {
        let m = Moment { year: 2026, month: 11, day: 8, hour: 22, minute: 35, second: 18 };
        assert_eq!(time_text(m), "10:35:18 PM");
        assert_eq!(time_text(Moment { hour: 0, minute: 48, second: 13, ..m }), "12:48:13 AM");
        assert_eq!(time_text(Moment { hour: 12, minute: 0, second: 0, ..m }), "12:00:00 PM");
    }

    #[test]
    fn calendar_columns_split_the_inside_in_seven() {
        let lefts: Vec<i32> = (0..7).map(|c| column(c).0).collect();
        assert_eq!(lefts, [32, 54, 77, 99, 122, 144, 167]);
        assert_eq!(column(6).0 + column(6).1, 190);
    }
}

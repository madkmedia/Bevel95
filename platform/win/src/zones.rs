//! Today's Windows time zones, for Date/Time Properties' Time Zone page and Bevel95's own clock,
//! which can keep another zone than the PC's (the PC's own zone and clock are never changed).

use std::sync::Mutex;

use windows_sys::Win32::Foundation::SYSTEMTIME;
use windows_sys::Win32::System::SystemInformation::{GetLocalTime, GetSystemTime};
use windows_sys::Win32::System::Time::{
    DYNAMIC_TIME_ZONE_INFORMATION, EnumDynamicTimeZoneInformation, GetDynamicTimeZoneInformation,
    GetTimeZoneInformationForYear, SystemTimeToTzSpecificLocalTimeEx, TIME_ZONE_INFORMATION,
};

/// The zone Bevel95's clock keeps, unless it simply shows the PC's local time (`None`).
static CLOCK: Mutex<Option<DYNAMIC_TIME_ZONE_INFORMATION>> = Mutex::new(None);

fn text(chars: &[u16]) -> String {
    let len = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
    String::from_utf16_lossy(&chars[..len])
}

/// A zone's registry key name ("Eastern Standard Time").
pub fn key_of(zone: &DYNAMIC_TIME_ZONE_INFORMATION) -> String {
    text(&zone.TimeZoneKeyName)
}

/// Every zone today's Windows knows.
pub fn all() -> Vec<DYNAMIC_TIME_ZONE_INFORMATION> {
    let mut out = Vec::new();
    for i in 0.. {
        let mut zone = DYNAMIC_TIME_ZONE_INFORMATION::default();
        // SAFETY: a valid out-pointer; a non-zero answer means no zone at this index.
        if unsafe { EnumDynamicTimeZoneInformation(i, &mut zone) } != 0 {
            break;
        }
        out.push(zone);
    }
    out
}

/// The zone with registry key name `key`, if this Windows has it.
pub fn find(key: &str) -> Option<DYNAMIC_TIME_ZONE_INFORMATION> {
    all().into_iter().find(|z| key_of(z).eq_ignore_ascii_case(key))
}

/// The PC's own zone.
pub fn pc_zone() -> DYNAMIC_TIME_ZONE_INFORMATION {
    let mut zone = DYNAMIC_TIME_ZONE_INFORMATION::default();
    // SAFETY: a valid out-pointer.
    unsafe { GetDynamicTimeZoneInformation(&mut zone) };
    zone
}

/// Whether `zone` has daylight saving time this year.
pub fn has_dst(zone: &DYNAMIC_TIME_ZONE_INFORMATION) -> bool {
    let mut now = SYSTEMTIME::default();
    // SAFETY: a valid out-pointer.
    unsafe { GetLocalTime(&mut now) };
    let mut tzi = TIME_ZONE_INFORMATION::default();
    // SAFETY: valid pointers; on failure the zone's own rule is used.
    if unsafe { GetTimeZoneInformationForYear(now.wYear, zone, &mut tzi) } != 0 {
        tzi.DaylightDate.wMonth != 0
    } else {
        zone.DaylightDate.wMonth != 0
    }
}

/// `zone` without daylight saving time (standard time all year).
fn without_dst(mut zone: DYNAMIC_TIME_ZONE_INFORMATION) -> DYNAMIC_TIME_ZONE_INFORMATION {
    zone.DaylightDate = SYSTEMTIME::default();
    zone.DaylightBias = 0;
    zone.DynamicDaylightTimeDisabled = true;
    zone
}

/// Sets the zone Bevel95's clock keeps: `key` (`None`: the PC's own), with or without daylight
/// saving time. The PC's own zone with daylight saving time is just the PC's local time.
pub fn set_clock(key: Option<&str>, dst: bool) {
    let zone = match key {
        Some(k) => find(k).or_else(|| {
            crate::log::log(&format!("time zone {k} is unknown to this Windows: the clock keeps the PC's"));
            None
        }),
        None if dst => None,
        None => Some(pc_zone()),
    };
    let zone = zone.map(|z| if dst { z } else { without_dst(z) });
    if let Ok(mut clock) = CLOCK.lock() {
        *clock = zone;
    }
}

fn local_in(zone: &DYNAMIC_TIME_ZONE_INFORMATION) -> Option<SYSTEMTIME> {
    let mut utc = SYSTEMTIME::default();
    let mut local = SYSTEMTIME::default();
    // SAFETY: valid pointers.
    unsafe { GetSystemTime(&mut utc) };
    // SAFETY: as above.
    (unsafe { SystemTimeToTzSpecificLocalTimeEx(zone, &utc, &mut local) } != 0).then_some(local)
}

/// The time in the zone Bevel95's clock keeps.
pub fn now() -> SYSTEMTIME {
    let zone = CLOCK.lock().ok().and_then(|c| *c);
    if let Some(local) = zone.as_ref().and_then(local_in) {
        return local;
    }
    let mut t = SYSTEMTIME::default();
    // SAFETY: a valid out-pointer.
    unsafe { GetLocalTime(&mut t) };
    t
}

/// The name of the zone Bevel95's clock keeps, as Windows gives it: its daylight name while
/// daylight saving time is on.
pub fn clock_zone_name() -> String {
    let zone = CLOCK.lock().ok().and_then(|c| *c).unwrap_or_else(pc_zone);
    let in_dst = match (local_in(&zone), local_in(&without_dst(zone))) {
        (Some(a), Some(b)) => (a.wHour, a.wMinute, a.wDay) != (b.wHour, b.wMinute, b.wDay),
        _ => false,
    };
    text(if in_dst { &zone.DaylightName } else { &zone.StandardName })
}

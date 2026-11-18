//! The Time Zone page of Date/Time Properties, read from the reference PCs (Windows 95 OSR 2.5,
//! 98 first edition, 2000 SP4; docs/theme-spec.md, "Time Zone page"): each Windows' own list of
//! zones, its world map (2000 turns it so the chosen zone is in the middle) and the daylight
//! saving check box. The page is laid out alike in all three.
//!
//! Bevel95 keeps its own clock (the owner's choice), so a zone chosen here only changes what
//! Bevel95's clock shows. Each entry names the zone of today's Windows (its registry key name)
//! that keeps the time of the places the entry lists.

use crate::dialog;
use crate::droplist::DropList;
use crate::fb::{Framebuffer, Rect};
use crate::text::Font;
use crate::theme::{Colors, Era, rgb};

/// An entry of a zone list: Windows' text, and the zone of today's Windows it stands for.
pub struct Zone {
    pub label: &'static str,
    pub key: &'static str,
}

const fn z(label: &'static str, key: &'static str) -> Zone {
    Zone { label, key }
}

/// Windows 95 OSR 2.5's list.
pub const ZONES_95: [Zone; 52] = [
    z("(GMT-12:00) Eniwetok, Kwajalein", "Dateline Standard Time"),
    z("(GMT-11:00) Midway Island, Samoa", "UTC-11"),
    z("(GMT-10:00) Hawaii", "Hawaiian Standard Time"),
    z("(GMT-09:00) Alaska", "Alaskan Standard Time"),
    z("(GMT-08:00) Pacific Time (US & Canada); Tijuana", "Pacific Standard Time"),
    z("(GMT-07:00) Arizona", "US Mountain Standard Time"),
    z("(GMT-07:00) Mountain Time (US & Canada)", "Mountain Standard Time"),
    z("(GMT-06:00) Central Time (US & Canada)", "Central Standard Time"),
    z("(GMT-06:00) Mexico City", "Central Standard Time (Mexico)"),
    z("(GMT-06:00) Saskatchewan", "Canada Central Standard Time"),
    z("(GMT-05:00) Bogota, Lima", "SA Pacific Standard Time"),
    z("(GMT-05:00) Eastern Time (US & Canada)", "Eastern Standard Time"),
    z("(GMT-05:00) Indiana (East)", "US Eastern Standard Time"),
    z("(GMT-04:00) Atlantic Time (Canada)", "Atlantic Standard Time"),
    z("(GMT-04:00) Caracas, La Paz", "SA Western Standard Time"),
    z("(GMT-03:30) Newfoundland", "Newfoundland Standard Time"),
    z("(GMT-03:00) Brasilia", "E. South America Standard Time"),
    z("(GMT-03:00) Buenos Aires, Georgetown", "SA Eastern Standard Time"),
    z("(GMT-02:00) Mid-Atlantic", "UTC-02"),
    z("(GMT-01:00) Azores, Cape Verde Is.", "Azores Standard Time"),
    z("(GMT) Greenwich Mean Time; Dublin, Edinburgh, London, Lisbon", "GMT Standard Time"),
    z("(GMT) Monrovia, Casablanca", "Greenwich Standard Time"),
    z("(GMT+01:00) Berlin, Stockholm, Rome, Bern, Brussels, Vienna, Amsterdam", "W. Europe Standard Time"),
    z("(GMT+01:00) Paris, Madrid", "Romance Standard Time"),
    z("(GMT+01:00) Prague, Bratislava", "Central Europe Standard Time"),
    z("(GMT+01:00) Warsaw", "Central European Standard Time"),
    z("(GMT+02:00) Athens, Helsinki, Istanbul", "GTB Standard Time"),
    z("(GMT+02:00) Cairo", "Egypt Standard Time"),
    z("(GMT+02:00) Eastern Europe", "E. Europe Standard Time"),
    z("(GMT+02:00) Harare, Pretoria", "South Africa Standard Time"),
    z("(GMT+02:00) Israel", "Israel Standard Time"),
    z("(GMT+03:00) Baghdad, Kuwait, Nairobi, Riyadh", "Arab Standard Time"),
    z("(GMT+03:00) Moscow, St. Petersburg", "Russian Standard Time"),
    z("(GMT+03:30) Tehran", "Iran Standard Time"),
    z("(GMT+04:00) Abu Dhabi, Muscat, Tbilisi, Kazan, Volgograd", "Arabian Standard Time"),
    z("(GMT+04:30) Kabul", "Afghanistan Standard Time"),
    z("(GMT+05:00) Islamabad, Karachi, Ekaterinburg, Tashkent", "West Asia Standard Time"),
    z("(GMT+05:30) Bombay, Calcutta, Madras, New Delhi, Colombo", "India Standard Time"),
    z("(GMT+06:00) Almaty, Dhaka", "Bangladesh Standard Time"),
    z("(GMT+07:00) Bangkok, Jakarta, Hanoi", "SE Asia Standard Time"),
    z("(GMT+08:00) Beijing, Chongqing, Urumqi", "China Standard Time"),
    z("(GMT+08:00) Hong Kong, Perth, Singapore, Taipei", "Singapore Standard Time"),
    z("(GMT+09:00) Tokyo, Osaka, Sapporo, Seoul, Yakutsk", "Tokyo Standard Time"),
    z("(GMT+09:30) Adelaide", "Cen. Australia Standard Time"),
    z("(GMT+09:30) Darwin", "AUS Central Standard Time"),
    z("(GMT+10:00) Brisbane", "E. Australia Standard Time"),
    z("(GMT+10:00) Canberra, Melbourne, Sydney", "AUS Eastern Standard Time"),
    z("(GMT+10:00) Guam, Port Moresby, Vladivostok", "West Pacific Standard Time"),
    z("(GMT+10:00) Hobart", "Tasmania Standard Time"),
    z("(GMT+11:00) Magadan, Solomon Is., New Caledonia", "Central Pacific Standard Time"),
    z("(GMT+12:00) Fiji, Kamchatka, Marshall Is.", "Fiji Standard Time"),
    z("(GMT+12:00) Wellington, Auckland", "New Zealand Standard Time"),
];

/// Windows 98 (first edition)'s list.
pub const ZONES_98: [Zone; 62] = [
    z("(GMT-12:00) Eniwetok, Kwajalein", "Dateline Standard Time"),
    z("(GMT-11:00) Midway Island, Samoa", "UTC-11"),
    z("(GMT-10:00) Hawaii", "Hawaiian Standard Time"),
    z("(GMT-09:00) Alaska", "Alaskan Standard Time"),
    z("(GMT-08:00) Pacific Time (US & Canada); Tijuana", "Pacific Standard Time"),
    z("(GMT-07:00) Arizona", "US Mountain Standard Time"),
    z("(GMT-07:00) Mountain Time (US & Canada)", "Mountain Standard Time"),
    z("(GMT-06:00) Central Time (US & Canada)", "Central Standard Time"),
    z("(GMT-06:00) Mexico City, Tegucigalpa", "Central Standard Time (Mexico)"),
    z("(GMT-06:00) Saskatchewan", "Canada Central Standard Time"),
    z("(GMT-05:00) Bogota, Lima, Quito", "SA Pacific Standard Time"),
    z("(GMT-05:00) Eastern Time (US & Canada)", "Eastern Standard Time"),
    z("(GMT-05:00) Indiana (East)", "US Eastern Standard Time"),
    z("(GMT-04:00) Atlantic Time (Canada)", "Atlantic Standard Time"),
    z("(GMT-04:00) Caracas, La Paz", "SA Western Standard Time"),
    z("(GMT-03:30) Newfoundland", "Newfoundland Standard Time"),
    z("(GMT-03:00) Brasilia", "E. South America Standard Time"),
    z("(GMT-03:00) Buenos Aires, Georgetown", "SA Eastern Standard Time"),
    z("(GMT-02:00) Mid-Atlantic", "UTC-02"),
    z("(GMT-01:00) Azores, Cape Verde Is.", "Azores Standard Time"),
    z("(GMT) Casablanca, Monrovia", "Greenwich Standard Time"),
    z("(GMT) Greenwich Mean Time : Dublin, Edinburgh, Lisbon, London", "GMT Standard Time"),
    z("(GMT+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna", "W. Europe Standard Time"),
    z("(GMT+01:00) Belgrade, Bratislava, Budapest, Ljubljana, Prague", "Central Europe Standard Time"),
    z("(GMT+01:00) Brussels, Copenhagen, Madrid, Paris, Vilnius", "Romance Standard Time"),
    z("(GMT+01:00) Sarajevo, Skopje, Sofija, Warsaw, Zagreb", "Central European Standard Time"),
    z("(GMT+02:00) Athens, Istanbul, Minsk", "GTB Standard Time"),
    z("(GMT+02:00) Bucharest", "E. Europe Standard Time"),
    z("(GMT+02:00) Cairo", "Egypt Standard Time"),
    z("(GMT+02:00) Harare, Pretoria", "South Africa Standard Time"),
    z("(GMT+02:00) Helsinki, Riga, Tallinn", "FLE Standard Time"),
    z("(GMT+02:00) Israel", "Israel Standard Time"),
    z("(GMT+03:00) Baghdad, Kuwait, Riyadh", "Arab Standard Time"),
    z("(GMT+03:00) Moscow, St. Petersburg, Volgograd", "Russian Standard Time"),
    z("(GMT+03:00) Nairobi", "E. Africa Standard Time"),
    z("(GMT+03:30) Tehran", "Iran Standard Time"),
    z("(GMT+04:00) Abu Dhabi, Muscat", "Arabian Standard Time"),
    z("(GMT+04:00) Baku, Tbilisi", "Caucasus Standard Time"),
    z("(GMT+04:30) Kabul", "Afghanistan Standard Time"),
    z("(GMT+05:00) Ekaterinburg", "Ekaterinburg Standard Time"),
    z("(GMT+05:00) Islamabad, Karachi, Tashkent", "West Asia Standard Time"),
    z("(GMT+05:30) Bombay, Calcutta, Madras, New Delhi", "India Standard Time"),
    z("(GMT+06:00) Almaty, Dhaka", "Bangladesh Standard Time"),
    z("(GMT+06:00) Colombo", "Sri Lanka Standard Time"),
    z("(GMT+07:00) Bangkok, Hanoi, Jakarta", "SE Asia Standard Time"),
    z("(GMT+08:00) Beijing, Chongqing, Hong Kong, Urumqi", "China Standard Time"),
    z("(GMT+08:00) Perth", "W. Australia Standard Time"),
    z("(GMT+08:00) Singapore", "Singapore Standard Time"),
    z("(GMT+08:00) Taipei", "Taipei Standard Time"),
    z("(GMT+09:00) Osaka, Sapporo, Tokyo", "Tokyo Standard Time"),
    z("(GMT+09:00) Seoul", "Korea Standard Time"),
    z("(GMT+09:00) Yakutsk", "Yakutsk Standard Time"),
    z("(GMT+09:30) Adelaide", "Cen. Australia Standard Time"),
    z("(GMT+09:30) Darwin", "AUS Central Standard Time"),
    z("(GMT+10:00) Brisbane", "E. Australia Standard Time"),
    z("(GMT+10:00) Canberra, Melbourne, Sydney", "AUS Eastern Standard Time"),
    z("(GMT+10:00) Guam, Port Moresby", "West Pacific Standard Time"),
    z("(GMT+10:00) Hobart", "Tasmania Standard Time"),
    z("(GMT+10:00) Vladivostok", "Vladivostok Standard Time"),
    z("(GMT+11:00) Magadan, Solomon Is., New Caledonia", "Central Pacific Standard Time"),
    z("(GMT+12:00) Auckland, Wellington", "New Zealand Standard Time"),
    z("(GMT+12:00) Fiji, Kamchatka, Marshall Is.", "Fiji Standard Time"),
];

/// Windows 2000 SP4's list.
pub const ZONES_2000: [Zone; 74] = [
    z("(GMT-12:00) Eniwetok, Kwajalein", "Dateline Standard Time"),
    z("(GMT-11:00) Midway Island, Samoa", "UTC-11"),
    z("(GMT-10:00) Hawaii", "Hawaiian Standard Time"),
    z("(GMT-09:00) Alaska", "Alaskan Standard Time"),
    z("(GMT-08:00) Pacific Time (US & Canada); Tijuana", "Pacific Standard Time"),
    z("(GMT-07:00) Arizona", "US Mountain Standard Time"),
    z("(GMT-07:00) Mountain Time (US & Canada)", "Mountain Standard Time"),
    z("(GMT-06:00) Central America", "Central America Standard Time"),
    z("(GMT-06:00) Central Time (US & Canada)", "Central Standard Time"),
    z("(GMT-06:00) Mexico City", "Central Standard Time (Mexico)"),
    z("(GMT-06:00) Saskatchewan", "Canada Central Standard Time"),
    z("(GMT-05:00) Bogota, Lima, Quito", "SA Pacific Standard Time"),
    z("(GMT-05:00) Eastern Time (US & Canada)", "Eastern Standard Time"),
    z("(GMT-05:00) Indiana (East)", "US Eastern Standard Time"),
    z("(GMT-04:00) Atlantic Time (Canada)", "Atlantic Standard Time"),
    z("(GMT-04:00) Caracas, La Paz", "SA Western Standard Time"),
    z("(GMT-04:00) Santiago", "Pacific SA Standard Time"),
    z("(GMT-03:30) Newfoundland", "Newfoundland Standard Time"),
    z("(GMT-03:00) Brasilia", "E. South America Standard Time"),
    z("(GMT-03:00) Buenos Aires, Georgetown", "SA Eastern Standard Time"),
    z("(GMT-03:00) Greenland", "Greenland Standard Time"),
    z("(GMT-02:00) Mid-Atlantic", "UTC-02"),
    z("(GMT-01:00) Azores", "Azores Standard Time"),
    z("(GMT-01:00) Cape Verde Is.", "Cape Verde Standard Time"),
    z("(GMT) Casablanca, Monrovia", "Greenwich Standard Time"),
    z("(GMT) Greenwich Mean Time : Dublin, Edinburgh, Lisbon, London", "GMT Standard Time"),
    z("(GMT+01:00) Amsterdam, Berlin, Bern, Rome, Stockholm, Vienna", "W. Europe Standard Time"),
    z("(GMT+01:00) Belgrade, Bratislava, Budapest, Ljubljana, Prague", "Central Europe Standard Time"),
    z("(GMT+01:00) Brussels, Copenhagen, Madrid, Paris", "Romance Standard Time"),
    z("(GMT+01:00) Sarajevo, Skopje, Sofija, Vilnius, Warsaw, Zagreb", "Central European Standard Time"),
    z("(GMT+01:00) West Central Africa", "W. Central Africa Standard Time"),
    z("(GMT+02:00) Athens, Istanbul, Minsk", "GTB Standard Time"),
    z("(GMT+02:00) Bucharest", "E. Europe Standard Time"),
    z("(GMT+02:00) Cairo", "Egypt Standard Time"),
    z("(GMT+02:00) Harare, Pretoria", "South Africa Standard Time"),
    z("(GMT+02:00) Helsinki, Riga, Tallinn", "FLE Standard Time"),
    z("(GMT+02:00) Jerusalem", "Israel Standard Time"),
    z("(GMT+03:00) Baghdad", "Arabic Standard Time"),
    z("(GMT+03:00) Kuwait, Riyadh", "Arab Standard Time"),
    z("(GMT+03:00) Moscow, St. Petersburg, Volgograd", "Russian Standard Time"),
    z("(GMT+03:00) Nairobi", "E. Africa Standard Time"),
    z("(GMT+03:30) Tehran", "Iran Standard Time"),
    z("(GMT+04:00) Abu Dhabi, Muscat", "Arabian Standard Time"),
    z("(GMT+04:00) Baku, Tbilisi, Yerevan", "Caucasus Standard Time"),
    z("(GMT+04:30) Kabul", "Afghanistan Standard Time"),
    z("(GMT+05:00) Ekaterinburg", "Ekaterinburg Standard Time"),
    z("(GMT+05:00) Islamabad, Karachi, Tashkent", "West Asia Standard Time"),
    z("(GMT+05:30) Calcutta, Chennai, Mumbai, New Delhi", "India Standard Time"),
    z("(GMT+05:45) Kathmandu", "Nepal Standard Time"),
    z("(GMT+06:00) Almaty, Novosibirsk", "N. Central Asia Standard Time"),
    z("(GMT+06:00) Astana, Dhaka", "Bangladesh Standard Time"),
    z("(GMT+06:00) Sri Jayawardenepura", "Sri Lanka Standard Time"),
    z("(GMT+06:30) Rangoon", "Myanmar Standard Time"),
    z("(GMT+07:00) Bangkok, Hanoi, Jakarta", "SE Asia Standard Time"),
    z("(GMT+07:00) Krasnoyarsk", "North Asia Standard Time"),
    z("(GMT+08:00) Beijing, Chongqing, Hong Kong, Urumqi", "China Standard Time"),
    z("(GMT+08:00) Irkutsk, Ulaan Bataar", "North Asia East Standard Time"),
    z("(GMT+08:00) Kuala Lumpur, Singapore", "Singapore Standard Time"),
    z("(GMT+08:00) Perth", "W. Australia Standard Time"),
    z("(GMT+08:00) Taipei", "Taipei Standard Time"),
    z("(GMT+09:00) Osaka, Sapporo, Tokyo", "Tokyo Standard Time"),
    z("(GMT+09:00) Seoul", "Korea Standard Time"),
    z("(GMT+09:00) Yakutsk", "Yakutsk Standard Time"),
    z("(GMT+09:30) Adelaide", "Cen. Australia Standard Time"),
    z("(GMT+09:30) Darwin", "AUS Central Standard Time"),
    z("(GMT+10:00) Brisbane", "E. Australia Standard Time"),
    z("(GMT+10:00) Canberra, Melbourne, Sydney", "AUS Eastern Standard Time"),
    z("(GMT+10:00) Guam, Port Moresby", "West Pacific Standard Time"),
    z("(GMT+10:00) Hobart", "Tasmania Standard Time"),
    z("(GMT+10:00) Vladivostok", "Vladivostok Standard Time"),
    z("(GMT+11:00) Magadan, Solomon Is., New Caledonia", "Central Pacific Standard Time"),
    z("(GMT+12:00) Auckland, Wellington", "New Zealand Standard Time"),
    z("(GMT+12:00) Fiji, Kamchatka, Marshall Is.", "Fiji Standard Time"),
    z("(GMT+13:00) Nuku'alofa", "Tonga Standard Time"),
];

/// The zone list of `era`.
pub fn zones(era: Era) -> &'static [Zone] {
    match era {
        Era::Win95 => &ZONES_95,
        Era::Win98 => &ZONES_98,
        Era::Win2000 => &ZONES_2000,
    }
}

/// The zone key `name` as the lists spell it (any case), if a list has it.
pub fn known_key(name: &str) -> Option<&'static str> {
    [&ZONES_95[..], &ZONES_98[..], &ZONES_2000[..]]
        .into_iter()
        .flatten()
        .map(|z| z.key)
        .find(|k| k.eq_ignore_ascii_case(name.trim()))
}

/// The first entry of `era`'s list that stands for zone `key`.
pub fn index_of_key(era: Era, key: &str) -> Option<usize> {
    zones(era).iter().position(|z| z.key.eq_ignore_ascii_case(key))
}

/// The first entry of `era`'s list whose label gives this offset from GMT (minutes).
pub fn index_of_offset(era: Era, minutes: i32) -> Option<usize> {
    zones(era).iter().position(|z| label_offset(z.label) == minutes)
}

/// The offset from GMT, in minutes, that an entry's label gives: "(GMT-03:30) ..." is -210,
/// "(GMT) ..." 0.
pub fn label_offset(label: &str) -> i32 {
    let Some(rest) = label.strip_prefix("(GMT") else { return 0 };
    let sign = match rest.chars().next() {
        Some('+') => 1,
        Some('-') => -1,
        _ => return 0,
    };
    let digits: Vec<i32> = rest[1..].chars().take(5).filter_map(|c| c.to_digit(10)).map(|d| d as i32).collect();
    match digits.as_slice() {
        [h1, h2, m1, m2] => sign * ((h1 * 10 + h2) * 60 + m1 * 10 + m2),
        _ => 0,
    }
}

// The page, the same in every look (window coordinates): the zone's drop-down list, the map
// in its sunken frame, the check box; the list dropped down below the field, 15 rows of 13px
// in a 1px black frame with a 16px scroll bar.
pub const COMBO: Rect = Rect::new(21, 58, 362, 21);
pub const COMBO_DROP: Rect = Rect::new(365, 60, 16, 17);
/// The field's highlight while it has the focus (the dotted rectangle runs around it).
pub const COMBO_ITEM: Rect = Rect::new(25, 62, 338, 13);
pub const MAP: Rect = Rect::new(22, 85, 360, 188);
pub const DST_CHECK: (i32, i32) = (21, 278);
pub const DST_LABEL: &str = "Automatically adjust clock for &daylight saving changes";
pub const LIST: Rect = Rect::new(21, 79, 362, 197);
pub const LIST_ROWS: usize = 15;
pub const ROW_H: i32 = 13;

/// The dropped list: the drop-down list every look shares (`droplist`), at its place here.
pub fn drop_list() -> DropList {
    DropList::new(LIST.x, LIST.y, LIST.w, LIST_ROWS)
}

/// The dropped list's rows (without the scroll bar) and its scroll bar.
pub fn list_rows_rect() -> Rect {
    drop_list().rows_rect()
}

pub fn scroll_bar_rect() -> Rect {
    drop_list().scroll_bar(LIST_ROWS + 1).bar
}

/// The scroll bar's thumb for `total` entries with `top` the first one shown: its length the
/// track's share of the rows shown, its place following `top` (Windows' MulDiv rounding).
pub fn thumb_rect(total: usize, top: usize) -> Rect {
    drop_list().scroll_bar(total).thumb(top)
}

/// The world map's pixels (1 = land), 356×184, cut from the reference PCs: Windows 98's (which
/// Windows 2000 shows too, turned) and Windows 95's (40 pixels differ).
const MAP_W: i32 = 356;
const MAP_H: i32 = 184;
static MAP_98: &[u8] = crate::art::bytes("time-zone/world", crate::art::Look::Win98);
static MAP_95: &[u8] = crate::art::bytes("time-zone/world", crate::art::Look::Win95);
const SEA: u32 = rgb(0x00007F);
const LAND: u32 = rgb(0x007F00);

/// How far Windows 2000 turns its map for a zone `minutes` from GMT: the map pixel shown at
/// column x is column (x + shift) of Windows 98's map, wrapping around (measured at nine zones;
/// C's integer division, which truncates). Windows 95 and 98 never turn it.
pub fn map_shift(era: Era, minutes: i32) -> i32 {
    if era == Era::Win2000 { (338 + minutes * MAP_W / 1440).rem_euclid(MAP_W) } else { 0 }
}

/// Paints the map in its sunken frame.
pub fn paint_map(fb: &mut Framebuffer, c: &Colors, era: Era, shift: i32) {
    let inside = dialog::well(fb, MAP, c);
    let bits = if era == Era::Win95 { MAP_95 } else { MAP_98 };
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            let n = (y * MAP_W + (x + shift).rem_euclid(MAP_W)) as usize;
            let land = bits.get(n / 8).is_some_and(|b| b & (0x80 >> (n % 8)) != 0);
            fb.set(inside.x + x, inside.y + y, if land { LAND } else { SEA });
        }
    }
}

/// Paints the dropped list: the entries from `top`, `selected` highlighted with the dotted
/// rectangle over it, and the scroll bar (its arrows drawn pushed in while `held`).
pub fn paint_list(
    fb: &mut Framebuffer,
    c: &Colors,
    font: &Font,
    era: Era,
    (top, selected): (usize, usize),
    screen: (i32, i32),
    held: impl Fn(i32) -> bool,
) {
    let labels: Vec<&str> = zones(era).iter().map(|z| z.label).collect();
    drop_list().paint(fb, c, font, &labels, (top, Some(selected)), Some(screen), held);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_give_offsets() {
        assert_eq!(label_offset("(GMT-03:30) Newfoundland"), -210);
        assert_eq!(label_offset("(GMT) Casablanca, Monrovia"), 0);
        assert_eq!(label_offset("(GMT+05:45) Kathmandu"), 345);
        assert_eq!(label_offset("(GMT+13:00) Nuku'alofa"), 780);
    }

    #[test]
    fn lists_are_in_order_and_known() {
        for era in Era::ALL {
            let list = zones(era);
            assert!(list.windows(2).all(|w| label_offset(w[0].label) <= label_offset(w[1].label)), "{era:?}");
            assert_eq!(index_of_key(era, "eastern standard time"), list.iter().position(|z| z.label.contains("Eastern Time")));
        }
        assert_eq!(known_key("pacific standard time"), Some("Pacific Standard Time"));
        assert_eq!(known_key("Mars Standard Time"), None);
        assert_eq!(index_of_offset(Era::Win98, -300), Some(10), "the first -05:00 entry: Bogota");
    }

    #[test]
    fn windows_2000_turns_its_map() {
        // Measured: Eastern, GMT, Cairo, Calcutta, Tokyo, Nuku'alofa, Eniwetok, Pacific.
        let cases = [(-300, 264), (0, 338), (120, 11), (330, 63), (540, 115), (780, 174), (-720, 160), (-480, 220)];
        for (minutes, shift) in cases {
            assert_eq!(map_shift(Era::Win2000, minutes), shift, "{minutes}");
        }
        assert_eq!(map_shift(Era::Win98, 540), 0);
        assert_eq!(MAP_98.len(), (MAP_W * MAP_H / 8) as usize);
        assert_eq!(MAP_95.len(), MAP_98.len());
    }
}

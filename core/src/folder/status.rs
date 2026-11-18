//! What a Windows 95 folder window's status bar says, in shell32's own words (its string table,
//! read from the Reference PC's SHELL32.DLL: ids 6466 to 6469, 4113 and 4114), and how big a new
//! folder window is and where it opens (measured on the Reference PC: `win95-truecolor-folder-*`).

/// The status bar's two parts: what is there or selected, and how much (size, or a drive's free
/// space).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub left: String,
    pub right: String,
}

/// "N object(s)", with "(plus N hidden)" when the folder holds files Explorer does not show.
pub fn objects(shown: usize, hidden: usize) -> String {
    if hidden > 0 { format!("{shown} object(s) (plus {hidden} hidden)") } else { format!("{shown} object(s)") }
}

/// "N object(s) selected".
pub fn selected(n: usize) -> String {
    format!("{n} object(s) selected")
}

/// A drive's free space and size: "Free Space: 284MB,  Capacity: 511MB" (two spaces, as shell32's
/// string has them).
pub fn free_space(free: u64, capacity: u64) -> String {
    format!("Free Space: {},  Capacity: {}", size(free), size(capacity))
}

/// A size as Windows 95's shell wrote it (StrFormatByteSize): under 1000 bytes in bytes ("489
/// bytes"); otherwise in KB, MB or GB with three digits, cut off rather than rounded ("939KB",
/// "19.9KB", "2.63MB"), the unit right after the number.
pub fn size(bytes: u64) -> String {
    if bytes < 1000 {
        return format!("{bytes} bytes");
    }
    let units = ["KB", "MB", "GB", "TB"];
    // In hundredths of the unit; 1000 or more of a unit is shown in the next one ("0.97MB").
    let mut unit = 0;
    let mut divisor: u64 = 1024;
    while bytes / divisor >= 1000 && unit + 1 < units.len() {
        divisor *= 1024;
        unit += 1;
    }
    let hundredths = (u128::from(bytes) * 100 / u128::from(divisor)) as u64;
    let whole = hundredths / 100;
    let text = if whole >= 100 {
        format!("{whole}")
    } else if whole >= 10 {
        format!("{whole}.{}", (hundredths % 100) / 10)
    } else {
        format!("{whole}.{:02}", hundredths % 100)
    };
    format!("{text}{}", units[unit])
}

/// The size Windows 95 gave a folder window opened for the first time, holding `n` items in
/// large icons, on a screen `screen` big (1× pixels; Bevel95 uses the work area): its icons in a
/// near square (as many columns as the square root of `n`, rounded up, and as many rows as they
/// need), 75 pixels each way per icon plus the window's frame, menu bar and status bar (40 across,
/// 69 down), at least 210 × 201, at most 65% of the screen's width and 412/600 of its height (the
/// Reference PC at 800 × 600: 520 × 412; a folder that would be bigger got a scroll bar).
pub fn first_size(n: usize, screen: (i32, i32)) -> (i32, i32) {
    let cols = (n as f64).sqrt().ceil() as i32;
    let rows = if cols == 0 { 0 } else { (n as i32 + cols - 1) / cols };
    let max = (screen.0 * 65 / 100, screen.1 * 412 / 600);
    let w = (40 + 75 * cols).clamp(210, max.0.max(210));
    let h = (69 + 75 * rows).clamp(201, max.1.max(201));
    (w, h)
}

/// One step of the cascade new folder windows open in (USER's place for a new window: the
/// height of a sizable window's caption and frame, 22 pixels at 1×).
pub const CASCADE_STEP: i32 = 22;

/// Where the next new folder window goes after one at `last` (from the work area's corner, 1×):
/// one step down and right, back to the corner once that would pass a quarter of the work area's
/// height (the Reference PC at 800 × 600: 0, 22, ... 132, then 0 again). The first window of a
/// start went to 44 there (two places had been taken by Windows' own windows).
pub fn next_cascade(last: Option<i32>, work_h: i32) -> i32 {
    match last {
        None => 2 * CASCADE_STEP,
        Some(at) if at + CASCADE_STEP > work_h / 4 => 0,
        Some(at) => at + CASCADE_STEP,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_as_windows_95_wrote_them() {
        assert_eq!(size(0), "0 bytes");
        assert_eq!(size(489), "489 bytes");
        assert_eq!(size(939 * 1024 + 500), "939KB");
        assert_eq!(size(20_377), "19.8KB", "cut off, not rounded");
        assert_eq!(size(2_760_000), "2.63MB");
        assert_eq!(size(132 * 1024), "132KB");
        assert_eq!(size(284 * 1024 * 1024), "284MB");
        assert_eq!(free_space(284 << 20, 511 << 20), "Free Space: 284MB,  Capacity: 511MB");
        assert_eq!(objects(5, 13), "5 object(s) (plus 13 hidden)");
        assert_eq!(objects(4, 0), "4 object(s)");
        assert_eq!(selected(1), "1 object(s) selected");
    }

    #[test]
    fn a_new_window_fits_its_icons_as_windows_95_sized_it() {
        // The Reference PC at 800 x 600 (its work area 800 x 572; the rule uses the screen).
        let screen = (800, 600);
        assert_eq!(first_size(0, screen), (210, 201), "an empty folder");
        assert_eq!(first_size(1, screen), (210, 201));
        assert_eq!(first_size(2, screen), (210, 201));
        assert_eq!(first_size(3, screen), (210, 219));
        assert_eq!(first_size(4, screen), (210, 219), "My Computer");
        assert_eq!(first_size(5, screen), (265, 219), "drive C:");
        assert_eq!(first_size(7, screen), (265, 294), "Program Files");
        assert_eq!(first_size(10, screen), (340, 294));
        assert_eq!(first_size(17, screen), (415, 369));
        assert_eq!(first_size(26, screen), (490, 412), "too tall: a scroll bar");
        assert_eq!(first_size(96, screen).0, 520);
    }

    #[test]
    fn new_windows_cascade_and_start_over() {
        let work_h = 572;
        let mut at = next_cascade(None, work_h);
        let mut seen = vec![at];
        for _ in 0..6 {
            at = next_cascade(Some(at), work_h);
            seen.push(at);
        }
        assert_eq!(seen, [44, 66, 88, 110, 132, 0, 22]);
    }
}

/// A size as `era`'s status bar wrote it: Windows 2000 puts a space before the unit ("59.2 KB",
/// `win2000-truecolor-explorer-webview-off`), 95 and 98 none ("1.02MB").
pub fn size_in(era: crate::theme::Era, bytes: u64) -> String {
    let s = size(bytes);
    if era != crate::theme::Era::Win2000 || bytes < 1000 {
        return s;
    }
    let at = s.len() - 2;
    format!("{} {}", &s[..at], &s[at..])
}

/// The Explorer's status bar adds the drive's free space: Windows 95 after the size ("939KB (Disk
/// free space: 284MB)", `win95-truecolor-tree-start`), 2000 after the count ("1 object(s) (Disk
/// free space: 2.05 GB)", `win2000-truecolor-tree-start`).
pub fn with_free_space(era: crate::theme::Era, text: &str, free: u64) -> String {
    format!("{text} (Disk free space: {})", size_in(era, free))
}

/// The size of a new Explorer window on a screen (work area) `screen` big (1×): Windows 95's and
/// 98's 600 × 411 at 800 × 600 (work area 800 × 572), 2000's 680 × 458 (`*-truecolor-tree-start`).
pub fn explorer_size(era: crate::theme::Era, screen: (i32, i32)) -> (i32, i32) {
    match era {
        crate::theme::Era::Win2000 => (screen.0 * 680 / 800, screen.1 * 458 / 572),
        _ => (screen.0 * 600 / 800, screen.1 * 411 / 572),
    }
}

/// Windows 2000's status bar with one file selected: "Type: Application Size: 59.2 KB".
pub fn type_and_size(type_name: &str, bytes: u64) -> String {
    format!("Type: {type_name} Size: {}", size_in(crate::theme::Era::Win2000, bytes))
}

/// The size of a new Windows 98 or 2000 folder window on a screen `screen` big (1×).
/// - 98 sized it by its icons as 95 did, a toolbar and an address bar taller and 9 pixels
///   narrower: 31 + 75 per column, 117 + 75 per row (`online-services-new`: 5 items, 256 × 267;
///   the `(C:)` window in its own-window mode: 8 items, 256 × 342). Its smallest and largest sizes
///   were not measured: Bevel95 keeps at least three columns and one row and at most My Computer's
///   512 × 461 of 800 × 600 (64% and 461/600 of the screen).
/// - 2000 opened every folder at the same size, 680 × 556 of 800 × 600 (85% and 556/600 of the
///   screen), whatever it held (`mydocs-new`: one item), cut to the work area where it opened.
pub fn first_size_in(era: crate::theme::Era, n: usize, screen: (i32, i32)) -> (i32, i32) {
    match era {
        crate::theme::Era::Win95 => first_size(n, screen),
        crate::theme::Era::Win98 => {
            let cols = ((n as f64).sqrt().ceil() as i32).max(3);
            let rows = ((n as i32 + cols - 1) / cols).max(1);
            let max = (screen.0 * 64 / 100, screen.1 * 461 / 600);
            ((31 + 75 * cols).min(max.0.max(256)), (117 + 75 * rows).min(max.1.max(192)))
        }
        crate::theme::Era::Win2000 => (screen.0 * 85 / 100, screen.1 * 556 / 600),
    }
}

#[cfg(test)]
mod later_tests {
    use super::*;
    use crate::theme::Era;

    #[test]
    fn windows_98_and_2000_sizes() {
        assert_eq!(size_in(Era::Win2000, 60_620), "59.1 KB");
        assert_eq!(size_in(Era::Win2000, 0), "0 bytes");
        assert_eq!(size_in(Era::Win98, 1_070_000), "1.02MB");
        assert_eq!(type_and_size("Application", 0), "Type: Application Size: 0 bytes");
        assert_eq!(first_size_in(Era::Win98, 5, (800, 600)), (256, 267));
        assert_eq!(first_size_in(Era::Win98, 8, (800, 600)), (256, 342));
        assert_eq!(first_size_in(Era::Win98, 400, (800, 600)), (512, 461));
        assert_eq!(first_size_in(Era::Win2000, 1, (800, 600)), (680, 556));
    }
}

//! Bevel95 keeps an eye on its own window resources: how many USER objects (windows, menus,
//! icons, cursors) and GDI objects (bitmaps, fonts, brushes, regions) it holds, as Windows
//! counts them for a program. One log line every ten minutes with both counts and their peaks,
//! a plain warning the first time either passes 2,000 and again at 5,000, and a hint when a
//! window could not be made because Windows ran out of memory (error 8). Windows lets one
//! program hold 10,000 of each; all programs together share one pool for windows (the desktop
//! heap), which can run out while Bevel95's own counts are small.

/// The counts Windows gives for a program (GetGuiResources): now and the highest so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub user: u32,
    pub gdi: u32,
    pub user_peak: u32,
    pub gdi_peak: u32,
}

/// The counts at which a warning is written, in order.
pub const WARN_AT: [u32; 2] = [2_000, 5_000];
/// What Windows lets one program hold of each kind.
pub const PROGRAM_LIMIT: u32 = 10_000;
/// Seconds between two regular log lines.
pub const LOG_EVERY_SECS: u64 = 600;

/// The regular log line.
pub fn line(c: Counts) -> String {
    format!(
        "window resources: {} USER objects (peak {}), {} GDI objects (peak {})",
        c.user, c.user_peak, c.gdi, c.gdi_peak
    )
}

/// Which warnings were written already.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    warned: usize,
}

impl Watch {
    pub const fn new() -> Watch {
        Watch { warned: 0 }
    }

    /// A warning to write now: the first time the larger count reaches 2,000, and again at
    /// 5,000 (one line for both when it jumps past both at once).
    pub fn warning(&mut self, c: Counts) -> Option<String> {
        let most = c.user.max(c.gdi);
        let passed = WARN_AT.iter().take_while(|&&at| most >= at).count();
        if passed <= self.warned {
            return None;
        }
        self.warned = passed;
        Some(format!(
            "WARNING: Bevel95 holds many window resources: {} USER and {} GDI objects (warned at {}; \
             Windows allows {} of each per program). Something in Bevel95 may be leaking them.",
            c.user,
            c.gdi,
            WARN_AT[passed - 1],
            PROGRAM_LIMIT
        ))
    }
}

/// What to add to the log line of a window that could not be made with Win32 error 8 (not
/// enough memory): the counts, and whose limit it probably is.
pub fn out_of_memory_hint(c: Counts) -> String {
    let near = c.user.max(c.gdi) >= PROGRAM_LIMIT * 9 / 10;
    let whose = if near {
        "Bevel95's own count is near Windows' limit of 10,000 per program"
    } else {
        "Windows' shared pool for windows (the desktop heap) may be empty: then it is not Bevel95's own limit"
    };
    format!("; Bevel95 holds {} USER and {} GDI objects: {whose}", c.user, c.gdi)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(user: u32, gdi: u32) -> Counts {
        Counts { user, gdi, user_peak: user, gdi_peak: gdi }
    }

    #[test]
    fn warnings_come_once_at_2000_and_once_at_5000() {
        let mut w = Watch::default();
        assert_eq!(w.warning(counts(300, 1_999)), None);
        assert!(w.warning(counts(300, 2_000)).unwrap().contains("warned at 2000"));
        assert_eq!(w.warning(counts(300, 2_500)), None, "only the first time");
        assert_eq!(w.warning(counts(1_000, 300)), None, "nor when it goes down and up again");
        assert!(w.warning(counts(5_001, 300)).unwrap().contains("warned at 5000"));
        assert_eq!(w.warning(counts(9_000, 300)), None);
        let mut jump = Watch::default();
        assert!(jump.warning(counts(6_000, 0)).unwrap().contains("warned at 5000"), "one line for both");
        assert_eq!(jump.warning(counts(6_000, 0)), None);
    }

    #[test]
    fn error_8_says_whose_limit_it_probably_is() {
        assert!(out_of_memory_hint(counts(250, 400)).contains("desktop heap"));
        assert!(out_of_memory_hint(counts(9_500, 400)).contains("near Windows' limit"));
        assert_eq!(
            line(Counts { user: 1, gdi: 2, user_peak: 3, gdi_peak: 4 }),
            "window resources: 1 USER objects (peak 3), 2 GDI objects (peak 4)"
        );
    }
}

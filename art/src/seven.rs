//! Windows 7 Basic's taskbar pictures (used by `core::seven`): the bar's fade and end zone, the
//! Start orb (normal and with the Start menu open), a running program's button, the button of the
//! window in front, the arrow that shows hidden notification icons, Show desktop.
//!
//! FIRST PASS: cut from two published screenshots (Windows 7 RTM and SP1 with the Windows 7 Basic
//! theme, 1920x1080 and 1440x900, PNG), read through an archive copy of a site that refused
//! automated requests; the owner allowed their use on 2026-11-25. To be replaced by the owner's
//! own captures (the project's rule: every picture from the owner's own Windows). The private
//! list of where each one came from is `private/probes/win7/art-inventory-7.md`.
//!
//! All are RGBA, straight alpha, rows top to bottom. Only the running button has a see-through
//! share, solved where the screenshots show it over two backgrounds; the rest are opaque (the
//! orb and the arrow with the bar's end zone behind them).

use crate::{Art, Depth, Entry, Look};

const SEVEN: &[Look] = &[Look::Win7];

pub(crate) const ENTRIES: &[Entry] = &[
    // 40x40: column 0 is the bar's body (rows 0, 1 its two top lines), columns 1..39 the fade
    // toward the darker end zone.
    Entry { what: "seven/bar-fade", looks: SEVEN, depth: Depth::TrueColor, art: Art::Bytes(include_bytes!("../assets/seven/bar-fade.rgba")) },
    // 1x40: a column of the end zones.
    Entry { what: "seven/bar-zone", looks: SEVEN, depth: Depth::TrueColor, art: Art::Bytes(include_bytes!("../assets/seven/bar-zone.rgba")) },
    // 56x80: the orb on the left end zone, normal (rows 0..39) and with the Start menu open.
    Entry { what: "seven/orb", looks: SEVEN, depth: Depth::TrueColor, art: Art::Bytes(include_bytes!("../assets/seven/orb.rgba")) },
    // 60x40: a running program's button (see-through share solved on its left 36 columns).
    Entry { what: "seven/button-running", looks: SEVEN, depth: Depth::TrueColor, art: Art::Bytes(include_bytes!("../assets/seven/button-running.rgba")) },
    // 60x40: the button of the window in front (opaque: seen over the body only).
    Entry { what: "seven/button-front", looks: SEVEN, depth: Depth::TrueColor, art: Art::Bytes(include_bytes!("../assets/seven/button-front.rgba")) },
    // 9x7: the arrow that shows the hidden notification icons, on the end zone.
    Entry { what: "seven/tray-arrow", looks: SEVEN, depth: Depth::TrueColor, art: Art::Bytes(include_bytes!("../assets/seven/tray-arrow.rgba")) },
    // 15x40: Show desktop, at the bar's right end.
    Entry { what: "seven/show-desktop", looks: SEVEN, depth: Depth::TrueColor, art: Art::Bytes(include_bytes!("../assets/seven/show-desktop.rgba")) },
];

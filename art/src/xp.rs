//! Windows XP's taskbar and Start menu (Luna, blue), for Bevel95's XP bar style (brief 112).
//!
//! A FIRST PASS FROM PUBLISHED SCREENSHOTS, not yet from the owner's own Windows: GUIdebook's and
//! Toasty Tech's pictures of XP's first release at 800x600 (`private/probes/xp/survey.md`,
//! `cuts/index.md`; each cut there was checked against a second installation, "confirmed" or
//! "single" as marked). The owner's own XP replaces them under the same names. Cut into one sheet
//! by `private/probes/xp/tools/to-art.ps1`, which also says what was cleaned (the network icon and
//! the clock's last letter under the notification area's pieces, painted over with the row's
//! colors between; the wallpaper in the Start menu's rounded corners made see-through).
//!
//! The sheet (`assets/xp/luna.rgba`) holds each picture's pixels one after the other, 4 bytes each
//! (red, green, blue, alpha); the table says where each begins and how big it is.

use crate::{Art, Depth, Entry, Look, Place};

#[rustfmt::skip]
const TABLE: &[Place] = &[
    ("column", 0, 1, 30),
    ("start_normal", 120, 100, 30),
    ("start_pressed", 12120, 100, 30),
    ("task_plain_left", 24120, 15, 25),
    ("task_plain_mid", 25620, 2, 25),
    ("task_plain_right", 25820, 4, 25),
    ("task_front_left", 26220, 15, 25),
    ("task_front_mid", 27720, 2, 25),
    ("task_front_right", 27920, 4, 25),
    ("tray_left", 28320, 23, 30),
    ("tray_fill", 31080, 1, 30),
    ("tray_right", 31200, 20, 30),
    ("menu_corner_left", 33600, 12, 12),
    ("menu_corner_right", 34176, 12, 12),
    ("menu_band_left", 34752, 10, 67),
    ("menu_band_column", 37432, 1, 67),
    ("menu_band_right", 37700, 35, 67),
    ("menu_lines", 47080, 380, 3),
    ("menu_body_row", 51640, 380, 1),
    ("menu_bottom", 53160, 380, 41),
    ("menu_sep_left", 115480, 191, 3),
    ("menu_sep_right", 117772, 190, 3),
    ("menu_all_programs", 120052, 188, 35),
    ("menu_all_programs_arrow", 146372, 18, 18),
    ("menu_user_frame", 147668, 60, 61),
    ("icon_my_documents", 162308, 24, 24),
    ("icon_my_recent_documents", 164612, 24, 24),
    ("icon_my_pictures", 166916, 24, 24),
    ("icon_my_music", 169220, 24, 24),
    ("icon_my_computer", 171524, 24, 24),
    ("icon_control_panel", 173828, 24, 24),
    ("icon_printers", 176132, 24, 24),
    ("icon_help", 178436, 24, 24),
    ("icon_search", 180740, 24, 24),
    ("icon_run", 183044, 24, 24),
];

pub(crate) const ENTRIES: &[Entry] = &[
    Entry { what: "xp/sheet", looks: &[Look::WinXp], depth: Depth::TrueColor, art: Art::Bytes(include_bytes!("../assets/xp/luna.rgba")) },
    Entry { what: "xp/places", looks: &[Look::WinXp], depth: Depth::TrueColor, art: Art::Places(TABLE) },
];

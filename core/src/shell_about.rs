//! The shell's About box. Both SOL and WINMINE pass their name, author credit and game icon
//! to ShellAbout. The shell supplies the rest of the dialog in the current Windows look.

use crate::fb::{Framebuffer, Icon, Rect};
use crate::text::Font;
use crate::theme::{Era, Theme, rgb};
use crate::window;

const BANNER: &[u8; 413 * 77 * 3] = crate::art::exactly("about/banner", crate::art::Look::Win2000);

fn position(x: i32, y: i32) -> (i32, i32) {
    (window::FRAME + (x * 6 + 2) / 4, window::CLIENT_Y + (y * 13 + 4) / 8)
}

pub(crate) fn size(era: Era) -> (i32, i32) {
    if era == Era::Win2000 { (419, 329) } else { (348, 283) }
}

pub(crate) fn button(era: Era) -> Rect {
    let (x, y) = if era == Era::Win2000 { position(220, 168) } else { position(170, 138) };
    Rect::new(x, y, 75, 23)
}

pub(crate) fn memory_text(kib: Option<u64>) -> String {
    let Some(kib) = kib else { return "N/A".to_owned() };
    let digits = kib.to_string();
    let mut memory = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            memory.push(',');
        }
        memory.push(ch);
    }
    memory.push_str(" KB");
    memory
}

pub(crate) struct App<'a> {
    pub name: &'a str,
    pub credit: &'a str,
    pub icon: &'a Icon,
    pub memory: &'a str,
}

pub(crate) fn paint(fb: &mut Framebuffer, era: Era, theme: &Theme, font: &Font, app: App<'_>) {
    let c = &theme.colors;
    let modern = era == Era::Win2000;
    let (x, first, info_y, value_x) = if modern { (70, 55, 142, 202) } else { (52, 11, 104, 184) };
    let (icon, separator, separator_width) = if modern {
        for (i, &[r, g, b]) in BANNER.as_chunks::<3>().0.iter().enumerate() {
            fb.set(
                3 + (i % 413) as i32,
                22 + (i / 413) as i32,
                rgb(u32::from_be_bytes([0, r, g, b])),
            );
        }
        (position(21, 55), position(70, 138), 298)
    } else {
        (position(7, 7), position(52, 97), 250)
    };
    fb.draw_icon(app.icon, icon.0, icon.1);
    fb.frame(Rect::new(separator.0, separator.1, separator_width, 2), c.shadow, c.highlight);
    let (version, copyright) = match era {
        Era::Win95 => ("Windows 95", "Copyright © 1981-1996 Microsoft Corp."),
        Era::Win98 => ("Windows 98", "Copyright (C) 1981-1997 Microsoft Corp."),
        Era::Win2000 => ("Version 5.0 (Build 2195: Service Pack 4)", "Copyright (C) 1981-1999 Microsoft Corp."),
    };
    let mut line = |x, y, text: &str| {
        let (x, y) = position(x, y);
        font.draw(fb, x, y, text, c.text);
    };
    let publisher = if era == Era::Win95 { "Microsoft" } else { "Microsoft (R)" };
    line(x, first, &format!("{publisher} {}", app.name));
    line(x, first + 10, version);
    line(x, first + 20, copyright);
    // The credit's static wraps: Windows 95's Calculator gives two lines.
    // (8 dialog units are exactly its 13-pixel lines.)
    for (i, part) in app.credit.split('\n').enumerate() {
        line(x, first + 30 + 8 * i as i32, part);
    }
    line(x, first + 50, "This product is licensed to:");
    line(x, first + 60, "Anonymous");
    line(
        x,
        info_y,
        if era == Era::Win95 {
            "Physical Memory Available to Windows:"
        } else {
            "Physical memory available to Windows:"
        },
    );
    line(value_x, info_y, app.memory);
    if !modern {
        line(x, 117, if era == Era::Win95 { "System Resources:" } else { "System resources:" });
        line(value_x, 117, "N/A");
    }
}

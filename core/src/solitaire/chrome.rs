//! Solitaire's resizable frame, menu bar and stock System-font status line.

use crate::fb::{Framebuffer, Icon, Rect};
use crate::solitaire::menu::MENUS;
use crate::text::Fonts;
use crate::theme::{Era, Theme, rgb};
use crate::window::{self, TitleButton, TitleLook};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Score {
    Standard(i32),
    Vegas(i32),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Status<'a> {
    pub score: Option<Score>,
    pub seconds: Option<u32>,
    pub message: Option<&'a str>,
    pub bonus: Option<i32>,
}

pub struct Chrome<'a> {
    pub era: Era,
    pub icon: &'a Icon,
    pub active: bool,
    pub maximized: bool,
    pub pressed: Option<TitleButton>,
    pub titles: [TitleLook; 2],
    pub show_keys: bool,
    pub status: Option<&'a Status<'a>>,
}

const STATUS_H: i32 = 17; // black rule, then 16 white rows

fn border(maximized: bool) -> i32 {
    window::sizable_border(maximized)
}

pub fn menu_offset(maximized: bool) -> (i32, i32) {
    let delta = border(maximized) - window::FRAME;
    (delta, delta)
}

fn client_rect(size: (i32, i32), maximized: bool) -> Rect {
    let frame = border(maximized);
    let y = frame + window::CLIENT_Y_WITH_MENU - window::FRAME;
    Rect::new(frame, y, (size.0 - 2 * frame).max(0), (size.1 - frame - y).max(0))
}

pub fn board_rect(era: Era, size: (i32, i32), status: bool, maximized: bool) -> Rect {
    let edge = if era == Era::Win2000 { 0 } else { 2 };
    client_rect(size, maximized).inset(edge, edge, edge, edge + if status { STATUS_H } else { 0 })
}

pub fn title_button_rect(width: i32, maximized: bool, button: TitleButton) -> Rect {
    window::sizable_button_rect(width, maximized, button)
}

pub fn icon_rect(maximized: bool) -> Rect {
    let frame = border(maximized);
    Rect::new(frame + 2, frame + 1, 16, 16)
}

// Printable ASCII from the stock VGA System font. Its glyphs match in all three
// guests' VGASYS.FON. Each index entry gives the advance and 16-row bitmap offset.
const SYSTEM: &[u8] = crate::art::bytes("solitaire/font-system", crate::art::Look::Win95);

fn glyph(b: u8) -> (i32, &'static [u8]) {
    let b = if (32..127).contains(&b) { b } else { b'?' };
    let index = usize::from(b - 32) * 3;
    let offset = usize::from(u16::from_le_bytes([SYSTEM[index + 1], SYSTEM[index + 2]]));
    (i32::from(SYSTEM[index]), &SYSTEM[95 * 3 + offset..])
}

fn measure(s: &str) -> i32 {
    s.bytes().map(|b| glyph(b).0).sum()
}

fn text(fb: &mut Framebuffer, s: &str, mut x: i32, area: Rect, ink: u32) {
    for b in s.bytes() {
        let (w, bits) = glyph(b);
        for dx in 0..w {
            let px = x + dx;
            if px < area.x || px >= area.right() {
                continue;
            }
            for dy in 0..16 {
                let byte = bits[(dx as usize / 8) * 16 + dy];
                if byte & (0x80 >> (dx % 8)) != 0 {
                    fb.set(px, area.y + dy as i32, ink);
                }
            }
        }
        x += w;
    }
}

fn status_line(fb: &mut Framebuffer, board: Rect, status: &Status) {
    fb.hline(board.x, board.bottom(), board.w, rgb(0));
    let line = Rect::new(board.x, board.bottom() + 1, board.w, 16);
    fb.fill(line, rgb(0xffffff));
    let mut left = line.right() - 4;
    if let Some(seconds) = status.seconds {
        let clock = format!("Time: {seconds}");
        left -= measure(&clock);
        text(fb, &clock, left, line, rgb(0));
    }
    if let Some(score) = status.score {
        let (points, ink) = match score {
            Score::Standard(n) => (n.to_string(), rgb(0)),
            Score::Vegas(n) if n < 0 => (format!("-${}", n.unsigned_abs()), rgb(0xff0000)),
            Score::Vegas(n) => (format!("${n}"), rgb(0)),
        };
        if status.seconds.is_some() {
            left -= measure(" ");
        }
        left -= measure(&points);
        text(fb, &points, left, line, ink);
        left -= measure("Score: ");
        text(fb, "Score: ", left, line, rgb(0));
    }
    let hint = Rect::new(line.x, line.y, (left - line.x).max(0), 16);
    let mut x = line.x + 4;
    if let Some(bonus) = status.bonus {
        let prefix = format!("Bonus: {bonus}  ");
        text(fb, &prefix, x, hint, rgb(0));
        x += measure(&prefix);
    }
    if let Some(message) = status.message {
        text(fb, message, x, hint, rgb(0));
    }
}

/// Draw the frame and status bar; the returned rectangle is the playing surface.
pub fn draw_chrome(fb: &mut Framebuffer, theme: &Theme, fonts: &Fonts, view: &Chrome) -> Rect {
    let c = &theme.colors;
    window::paint_sizable_frame(fb, theme, fonts, view.era, "Solitaire", view.icon, view.active, view.pressed, view.maximized);
    let (dx, dy) = menu_offset(view.maximized);
    let rects: Vec<_> =
        window::menu_bar_layout(fonts, &MENUS).into_iter().map(|r| Rect::new(r.x + dx, r.y + dy, r.w, r.h)).collect();
    window::paint_menu_bar(fb, theme, fonts, view.era, &MENUS, &rects, &view.titles, view.show_keys, view.active);
    let size = (fb.width, fb.height);
    if view.era != Era::Win2000 {
        let client = client_rect(size, view.maximized);
        fb.frame(client, c.shadow, c.highlight);
        fb.frame(client.inset(1, 1, 1, 1), c.dark_shadow, c.light);
    }
    let board = board_rect(view.era, size, view.status.is_some(), view.maximized);
    if let Some(status) = view.status {
        status_line(fb, board, status);
    }
    board
}

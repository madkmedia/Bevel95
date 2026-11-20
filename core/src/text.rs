//! Bitmap-font text. The platform layer renders the real font once (GDI on Windows) and
//! hands over a `Font`: one small 1-bit picture per character. Everything else (drawing,
//! measuring, "..." eliding) happens here, so it is identical on every platform.
//!
//! A `Font` can also be saved as a plain-text "fixture" (ASCII art, see `to_text`), which
//! is how the headless tests get real Windows glyphs without calling Windows.

use std::fmt::Write as _;
use std::sync::{LazyLock, OnceLock};

use crate::fb::Framebuffer;

/// The fixed MS Sans Serif face used by classic dialog templates and HTML Help controls,
/// even when the system caption font is Tahoma: the platform's (`use_platform_ms_sans_serif`),
/// else the bundled fixture (code page 1252 only, as in the headless tests).
pub(crate) fn ms_sans_serif() -> &'static Font {
    static FONT: LazyLock<Font> = LazyLock::new(|| {
        PLATFORM_MS_SANS_SERIF.get().and_then(|make| make()).unwrap_or_else(|| {
            Font::parse(include_str!("../tests/fixtures/ms-sans-serif-11px.font")).expect("bundled MS Sans Serif atlas")
        })
    });
    &FONT
}

static PLATFORM_MS_SANS_SERIF: OnceLock<fn() -> Option<Font>> = OnceLock::new();

/// Where `ms_sans_serif` gets its font in a program: `make`, called once when it is first
/// needed (the platform's MS Sans Serif 8pt with the letters of other alphabets too). The first
/// call counts; one made after `ms_sans_serif` was first used changes nothing.
pub fn use_platform_ms_sans_serif(make: fn() -> Option<Font>) {
    let _ = PLATFORM_MS_SANS_SERIF.set(make);
}

/// One character: how far it moves the pen, and its ink.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub ch: char,
    /// Pen movement in pixels (GDI's text extent for this character).
    pub advance: i32,
    /// Left edge of the ink relative to the pen (can be negative).
    pub ink_x: i32,
    /// Width of the ink; 0 for blank characters like space.
    pub ink_w: i32,
    /// `ink_w` × font height booleans, row by row, from the top of the font's cell.
    pub ink: Vec<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Font {
    pub name: String,
    pub bold: bool,
    /// Cell height (GDI `tmHeight`).
    pub height: i32,
    /// Cell top to baseline (GDI `tmAscent`).
    pub ascent: i32,
    /// Extra width Windows adds to a string of a bold it makes up itself (GDI `tmOverhang`:
    /// 1 for bold MS Sans Serif, which has no bold of its own). It shows in access-key
    /// underlines, which are that much longer ("&Close" in Windows 95's window menu), and in
    /// a menu whose widest entry is bold (Windows 95's speaker menu), which is that much wider.
    pub overhang: i32,
    /// Sorted by character.
    glyphs: Vec<Glyph>,
}

/// The regular and bold faces of one era's UI font.
#[derive(Clone, Copy, Debug)]
pub struct Fonts<'a> {
    pub regular: &'a Font,
    pub bold: &'a Font,
}

const MAGIC: &str = "bevel95-font 1";

/// Shown in place of characters the font does not have and that have no stand-in
/// (`stand_in`): what Windows 95 did too.
const FALLBACK: char = '?';

/// What an invisible mark the font lacks draws: nothing, 0 wide.
static NOTHING: Glyph = Glyph { ch: '\u{200B}', advance: 0, ink_x: 0, ink_w: 0, ink: Vec::new() };

/// Marks that only steer how text is laid out (zero-width spaces and joiners, direction marks
/// and embeddings, the word joiner and invisible operators, variation selectors, the byte
/// order mark): Bevel95's own rule draws them as nothing. Windows 95 showed most as "?".
fn invisible(ch: char) -> bool {
    matches!(
        ch,
        '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
    )
}

/// The character drawn for `ch` when the font lacks it: for a round dot (U+25CF black circle,
/// which some editors put before the title of an unsaved file, U+2B24 and U+26AB), the bullet
/// U+2022 (Bevel95's own rule; Windows' best fit has "?" for them); else Windows' own best fit
/// to code page 1252 (`best_fit`, what Windows 95 read titles through: U+0141 as "L").
fn stand_in(ch: char) -> Option<char> {
    match ch {
        '\u{25CF}' | '\u{2B24}' | '\u{26AB}' => Some('\u{2022}'),
        _ => crate::best_fit::best_fit(ch),
    }
}

/// Glyph rows that today's fonts draw differently from the Windows a look comes from, as
/// (font, bold, character, row, pixels). Today's Tahoma bends the tail of "y" one row lower
/// than Windows 2000's did ("Accessibility", "System Tools" in GUIdebook's screenshots).
/// The bold "x" in Windows 2000's Internet Explorer caption has two separate strokes on row 6.
const PERIOD_GLYPHS: &[(&str, bool, char, usize, &str)] = &[
    ("Tahoma", false, 'y', 11, ".#..."),
    ("Tahoma", true, 'x', 6, "##..##"),
    // The web view's title in Windows 98 (Verdana 16pt bold, win98-truecolor-web-v1-none): the
    // two strokes of "v" meet a row later.
    ("Verdana", true, 'v', 12, ".#####..#####."),
    // ... and its text (Verdana 8pt, the dates of -web-v1-file): "/" steps two rows at a time.
    ("Verdana", false, '/', 3, "....#"),
];

/// Glyphs of the bold font of Explorer's menus in Windows 2000 (the bold Open of every desktop
/// icon's menu, `private/reference/refs/vm/win2000-truecolor-item-menu-*`) that differ from today's Tahoma Bold:
/// a round "O" a pixel wider and an "e" whose bowl closes at the bottom right. Windows 2000's
/// own window menus and titles match today's Tahoma Bold (their compare-refs cases), so only
/// menus marked `Menu::explorer_bold` use these. As (character, wider by, rows from the glyph's
/// first inked row).
const EXPLORER_BOLD_SHAPES: &[(char, i32, &[&str])] = &[
    ('O', 1, &["..####..", ".##..##.", "##....##", "##....##", "##....##", "##....##", ".##..##.", "..####.."]),
    ('e', 0, &[".####.", "##..##", "######", "##....", "##..##", ".####."]),
];

/// A menu label without its access-key marker, and which character (by index) the marker
/// was on: "Sh&ut Down..." gives ("Shut Down...", Some(2)). "&&" stands for a plain "&".
pub fn split_access_key(label: &str) -> (String, Option<usize>) {
    let mut text = String::with_capacity(label.len());
    let mut key = None;
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            match chars.next() {
                Some('&') => text.push('&'),
                Some(next) => {
                    key.get_or_insert(text.chars().count());
                    text.push(next);
                }
                None => {}
            }
        } else {
            text.push(c);
        }
    }
    (text, key)
}

/// `text` with case taken out, for finding what a person types: lowercase, letter by letter, in
/// every alphabet the atlases hold (Cyrillic, Greek, Central European, Turkish, Baltic as well
/// as Latin), with the three letters whose lowercase is not one plain letter made so: Turkish
/// capital I with dot and dotless i are both "i" (so "istanbul" finds "İstanbul", and typing on
/// a keyboard of either kind finds both), and Greek final sigma is sigma (a word typed in
/// capitals, then continued, still finds it). One character in, one character out.
pub fn fold(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{0130}' | '\u{0131}' => 'i',
            '\u{03C2}' => '\u{03C3}',
            c => {
                let mut lower = c.to_lowercase();
                match (lower.next(), lower.next()) {
                    (Some(l), None) => l,
                    _ => c,
                }
            }
        })
        .collect()
}

/// Whether two names are the same name to Windows' file system, case aside: letter by letter,
/// each letter's uppercase where it is one letter (as Windows' own table has it: "Файл" and
/// "ФАЙЛ" are one name, "straße" and "STRASSE" are not).
pub fn same_name(a: &str, b: &str) -> bool {
    let upper = |c: char| {
        let mut u = c.to_uppercase();
        match (u.next(), u.next()) {
            (Some(u), None) => u,
            _ => c,
        }
    };
    a.chars().map(upper).eq(b.chars().map(upper))
}

/// `text` made safe to use as a menu label: its "&"s show as they are.
pub fn escape_access_keys(text: &str) -> String {
    text.replace('&', "&&")
}

/// File name of a font fixture, e.g. `ms-sans-serif-11px-bold.font`.
pub fn fixture_name(face: &str, px: i32, bold: bool) -> String {
    let slug: String =
        face.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    format!("{slug}-{px}px{}.font", if bold { "-bold" } else { "" })
}

impl Font {
    pub fn new(name: String, bold: bool, height: i32, ascent: i32, mut glyphs: Vec<Glyph>) -> Result<Font, String> {
        glyphs.sort_by_key(|g| g.ch);
        if let Some(w) = glyphs.windows(2).find(|w| w[0].ch == w[1].ch) {
            return Err(format!("{name}: character {:?} appears twice", w[0].ch));
        }
        if let Some(g) = glyphs.iter().find(|g| g.ink.len() != (g.ink_w.max(0) * height) as usize) {
            return Err(format!("{name}: ink of {:?} has the wrong size", g.ch));
        }
        if glyphs.binary_search_by_key(&FALLBACK, |g| g.ch).is_err() {
            return Err(format!("{name}: needs a '{FALLBACK}' glyph"));
        }
        Ok(Font { name, bold, height, ascent, overhang: 0, glyphs })
    }

    pub fn glyphs(&self) -> &[Glyph] {
        &self.glyphs
    }

    /// The same font with GDI's overhang (see `overhang`).
    pub fn with_overhang(self, overhang: i32) -> Font {
        Font { overhang, ..self }
    }

    /// The font as its era drew it: `PERIOD_GLYPHS` applied (rows that do not fit the glyph
    /// are left alone).
    pub fn with_period_glyphs(mut self) -> Font {
        for &(name, bold, ch, row, pixels) in PERIOD_GLYPHS {
            if !self.name.eq_ignore_ascii_case(name) || self.bold != bold || row >= self.height as usize {
                continue;
            }
            if let Some(g) = self.glyphs.iter_mut().find(|g| g.ch == ch && g.ink_w as usize == pixels.len()) {
                let w = g.ink_w as usize;
                for (cell, b) in g.ink[row * w..(row + 1) * w].iter_mut().zip(pixels.bytes()) {
                    *cell = b == b'#';
                }
            }
        }
        self
    }

    /// The bold of Explorer's menus in Windows 2000 (`EXPLORER_BOLD_SHAPES`), for Tahoma Bold;
    /// `None` for any other font (nothing differs).
    pub fn explorer_bold(&self) -> Option<Font> {
        if !self.name.eq_ignore_ascii_case("Tahoma") || !self.bold {
            return None;
        }
        let mut font = self.clone();
        let height = font.height as usize;
        for &(ch, wider, rows) in EXPLORER_BOLD_SHAPES {
            if let Some(g) = font.glyphs.iter_mut().find(|g| g.ch == ch) {
                let old_w = g.ink_w as usize;
                let Some(first) = (0..height).find(|&r| g.ink[r * old_w..(r + 1) * old_w].iter().any(|&p| p)) else { continue };
                let w = rows[0].len();
                if first + rows.len() > height {
                    continue;
                }
                let mut ink = vec![false; w * height];
                for (k, row) in rows.iter().enumerate() {
                    for (x, b) in row.bytes().enumerate() {
                        ink[(first + k) * w + x] = b == b'#';
                    }
                }
                g.ink = ink;
                g.ink_w = w as i32;
                g.advance += wider;
            }
        }
        Some(font)
    }

    /// The glyph for `ch`. If the font does not have it: nothing for an invisible mark
    /// (`invisible`), else the glyph of its stand-in if the font has that (`stand_in`), else
    /// '?'. Every width and every drawing goes through here, so labels, menus, tooltips and
    /// text fields all agree, one glyph for each character.
    pub fn glyph(&self, ch: char) -> &Glyph {
        let find = |c: char| self.glyphs.binary_search_by_key(&c, |g| g.ch).ok();
        if let Some(i) = find(ch) {
            return &self.glyphs[i];
        }
        if invisible(ch) {
            return &NOTHING;
        }
        let i = stand_in(ch).and_then(find).or_else(|| find(FALLBACK)).expect("Font::new checked that '?' exists");
        &self.glyphs[i]
    }

    /// Width of `text` in pixels (the sum of the advances).
    pub fn measure(&self, text: &str) -> i32 {
        text.chars().map(|c| self.glyph(c).advance).sum()
    }

    /// Draws `text` with the top of the font's cell at `y`. Returns the width drawn.
    pub fn draw(&self, fb: &mut Framebuffer, x: i32, y: i32, text: &str, color: u32) -> i32 {
        let mut pen = x;
        for ch in text.chars() {
            let g = self.glyph(ch);
            for row in 0..self.height {
                for col in 0..g.ink_w {
                    if g.ink[(row * g.ink_w + col) as usize] {
                        fb.set(pen + g.ink_x + col, y + row, color);
                    }
                }
            }
            pen += g.advance;
        }
        pen - x
    }

    /// Draws a menu label (see `split_access_key`) with the top left of the cell at `(x, y)`, shortened
    /// with "..." to `max_w`. With `show_key`, the access key is underlined the way Windows
    /// does it: on the cell's bottom row, across the character's whole advance plus the
    /// overhang (checked against GDI's DrawText by a test in the Windows crate). Returns the
    /// width drawn.
    pub fn draw_label(
        &self,
        fb: &mut Framebuffer,
        (x, y): (i32, i32),
        label: &str,
        max_w: i32,
        color: u32,
        show_key: bool,
    ) -> i32 {
        let (text, key) = split_access_key(label);
        let shown = self.elide(&text, max_w);
        let width = self.draw(fb, x, y, &shown, color);
        let cut_short = shown != text;
        if let (true, Some(k), false) = (show_key, key, cut_short) {
            let before: String = text.chars().take(k).collect();
            if let Some(ch) = text.chars().nth(k) {
                fb.hline(x + self.measure(&before), y + self.height - 1, self.glyph(ch).advance + self.overhang, color);
            }
        }
        width
    }

    /// Where `draw_label` underlines the access key: from this far right of the label's start,
    /// this wide (None: no access key, or the label is cut short to `max_w`).
    pub fn key_underline(&self, label: &str, max_w: i32) -> Option<(i32, i32)> {
        let (text, key) = split_access_key(label);
        if self.elide(&text, max_w) != text {
            return None;
        }
        let k = key?;
        let ch = text.chars().nth(k)?;
        let before: String = text.chars().take(k).collect();
        Some((self.measure(&before), self.glyph(ch).advance + self.overhang))
    }

    /// Width of a menu label without its access-key marker.
    pub fn measure_label(&self, label: &str) -> i32 {
        self.measure(&split_access_key(label).0)
    }

    /// `text` shortened to fit `max_w` pixels, ending in "..." like Windows' DrawText with
    /// DT_END_ELLIPSIS. Unchanged if it already fits.
    pub fn elide(&self, text: &str, max_w: i32) -> String {
        if self.measure(text) <= max_w {
            return text.to_string();
        }
        let dots_w = self.measure("...");
        let mut out = String::new();
        if dots_w > max_w {
            // Not even "..." fits: show as many dots as do.
            let dot = self.glyph('.').advance.max(1);
            out.extend(std::iter::repeat_n('.', (max_w / dot).clamp(0, 3) as usize));
            return out;
        }
        let mut w = 0;
        for ch in text.chars() {
            let a = self.glyph(ch).advance;
            if w + a + dots_w > max_w {
                break;
            }
            w += a;
            out.push(ch);
        }
        out.push_str("...");
        out
    }

    /// The fixture text format: a header, then for each glyph a line
    /// `glyph <code point> <advance> <ink_x> <ink_w>` followed by `height` rows of
    /// `#`/`.` (no rows when `ink_w` is 0).
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{MAGIC}");
        let _ = writeln!(s, "name {}", self.name);
        let _ = writeln!(s, "bold {}", u8::from(self.bold));
        let _ = writeln!(s, "height {}", self.height);
        let _ = writeln!(s, "ascent {}", self.ascent);
        if self.overhang != 0 {
            let _ = writeln!(s, "overhang {}", self.overhang);
        }
        for g in &self.glyphs {
            let _ = writeln!(s, "glyph {} {} {} {}", g.ch as u32, g.advance, g.ink_x, g.ink_w);
            if g.ink_w > 0 {
                for row in g.ink.chunks(g.ink_w as usize) {
                    s.extend(row.iter().map(|&on| if on { '#' } else { '.' }));
                    s.push('\n');
                }
            }
        }
        s
    }

    /// Reads the format written by `to_text`. Accepts CRLF line endings.
    pub fn parse(src: &str) -> Result<Font, String> {
        let lines: Vec<&str> = src.lines().collect();
        let err = |i: usize, msg: &str| format!("font fixture line {}: {msg}", i + 1);
        let field = |i: usize, key: &str| -> Result<&str, String> {
            let line = lines.get(i).ok_or_else(|| err(i, "unexpected end of file"))?;
            line.strip_prefix(key)
                .and_then(|rest| rest.strip_prefix(' '))
                .ok_or_else(|| err(i, &format!("expected `{key} ...`")))
        };
        let number = |i: usize, key: &str| -> Result<i32, String> {
            field(i, key)?.parse().map_err(|_| err(i, &format!("`{key}` is not a number")))
        };
        if lines.first() != Some(&MAGIC) {
            return Err(err(0, "not a bevel95 font fixture"));
        }
        let name = field(1, "name")?.to_string();
        let bold = number(2, "bold")? != 0;
        let height = number(3, "height")?;
        let ascent = number(4, "ascent")?;
        let mut i = 5;
        // Optional: only fonts that have one say so.
        let overhang = if lines.get(i).is_some_and(|l| l.starts_with("overhang ")) {
            i += 1;
            number(5, "overhang")?
        } else {
            0
        };
        let mut glyphs = Vec::new();
        while i < lines.len() {
            let nums: Vec<i32> = field(i, "glyph")?
                .split(' ')
                .map(str::parse)
                .collect::<Result<_, _>>()
                .map_err(|_| err(i, "glyph line has a bad number"))?;
            let &[code, advance, ink_x, ink_w] = nums.as_slice() else {
                return Err(err(i, "glyph line needs 4 numbers"));
            };
            let ch = u32::try_from(code).ok().and_then(char::from_u32).ok_or_else(|| err(i, "bad code point"))?;
            i += 1;
            let mut ink = Vec::new();
            if ink_w > 0 {
                for _ in 0..height {
                    let row = lines.get(i).ok_or_else(|| err(i, "missing glyph row"))?;
                    if row.len() != ink_w as usize {
                        return Err(err(i, "glyph row has the wrong width"));
                    }
                    for b in row.bytes() {
                        ink.push(match b {
                            b'#' => true,
                            b'.' => false,
                            _ => return Err(err(i, "glyph rows may only contain # and .")),
                        });
                    }
                    i += 1;
                }
            }
            glyphs.push(Glyph { ch, advance, ink_x, ink_w, ink });
        }
        Ok(Font { overhang, ..Font::new(name, bold, height, ascent, glyphs)? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3px-tall test font: "a" is 2 wide, "?" is 1 wide, "." is 1 wide, " " is blank.
    fn tiny() -> Font {
        let g = |ch, advance, ink_w, ink: &[u8]| Glyph {
            ch,
            advance,
            ink_x: 0,
            ink_w,
            ink: ink.iter().map(|&b| b == 1).collect(),
        };
        let glyphs = vec![
            g('a', 3, 2, &[1, 1, 1, 0, 1, 1]),
            g('?', 2, 1, &[1, 0, 1]),
            g('.', 2, 1, &[0, 0, 1]),
            g(' ', 2, 0, &[]),
        ];
        Font::new("Tiny".into(), false, 3, 2, glyphs).unwrap()
    }

    #[test]
    fn fixture_round_trip() {
        let f = tiny();
        let text = f.to_text();
        assert_eq!(Font::parse(&text).unwrap(), f);
        assert_eq!(Font::parse(&text.replace('\n', "\r\n")).unwrap(), f, "CRLF must parse too");
        let bold = Font { overhang: 1, ..tiny() };
        assert_eq!(Font::parse(&bold.to_text()).unwrap(), bold);
    }

    #[test]
    fn overhang_lengthens_the_access_key_underline() {
        let f = Font { overhang: 1, ..tiny() };
        let mut fb = Framebuffer::new(9, 3, 0);
        f.draw_label(&mut fb, (0, 0), "&aa", 100, 9, true);
        assert_eq!((0..5).map(|x| fb.get(x, 2)).collect::<Vec<_>>(), [9, 9, 9, 9, 9]);
        assert_eq!(fb.get(4, 1), 0, "the underline is one row; x = 4 has only the second a's ink at row 2");
    }

    #[test]
    fn missing_characters_fall_back_to_question_mark() {
        let f = tiny();
        assert_eq!(f.glyph('Ж').ch, '?');
        assert_eq!(f.measure("aЖ"), 3 + 2);
        // A stand-in the font lacks too: still "?" (tiny has no "L", no bullet).
        assert_eq!(f.glyph('\u{0141}').ch, '?');
        assert_eq!(f.glyph('\u{25CF}').ch, '?');
    }

    #[test]
    fn missing_characters_take_windows_best_fit() {
        // The cases Windows 95's best fit to code page 1252 turned into a near character.
        let cases = [
            ('\u{0141}', 'L'),
            ('\u{0107}', 'c'),
            ('\u{2011}', '-'),
            ('\u{2009}', ' '),
            ('\u{202F}', '\u{A0}'),
            ('\u{FF01}', '!'),
        ];
        for (from, to) in cases {
            assert_eq!(crate::best_fit::best_fit(from), Some(to), "U+{:04X}", from as u32);
        }
        // Not in the table: what has no near character, and what code page 1252 has itself.
        for ch in ['\u{25CF}', 'Ж', 'a', '\u{20AC}', '\u{1F600}'] {
            assert_eq!(crate::best_fit::best_fit(ch), None, "U+{:04X}", ch as u32);
        }
        let f = ms_sans_serif();
        assert_eq!(f.glyph('\u{0141}'), f.glyph('L'));
        assert_eq!(f.measure("\u{0141}\u{00F3}d\u{017A}"), f.measure("L\u{00F3}dz"));
        assert_eq!(f.glyph('\u{202F}'), f.glyph('\u{A0}'));
        assert_eq!(f.glyph('Ж').ch, '?');
    }

    #[test]
    fn round_dots_are_bullets_and_invisible_marks_nothing() {
        let f = ms_sans_serif();
        for dot in ['\u{25CF}', '\u{2B24}', '\u{26AB}'] {
            assert_eq!(f.glyph(dot), f.glyph('\u{2022}'));
        }
        // An editor's title of an unsaved file.
        assert_eq!(f.measure("\u{25CF} notes"), f.measure("\u{2022} notes"));
        let marks = ['\u{200B}', '\u{200F}', '\u{202A}', '\u{202E}', '\u{2060}', '\u{2064}', '\u{FE00}', '\u{FE0F}'];
        for mark in marks.into_iter().chain(['\u{FEFF}']) {
            assert_eq!((f.glyph(mark).advance, f.glyph(mark).ink_w), (0, 0), "U+{:04X}", mark as u32);
        }
        assert_eq!(f.measure("a\u{200B}b\u{FEFF}"), f.measure("ab"));
        let (mut with, mut without) = (Framebuffer::new(40, 13, 0), Framebuffer::new(40, 13, 0));
        assert_eq!(f.draw(&mut with, 0, 0, "\u{200E}ab\u{FE0F}", 9), f.draw(&mut without, 0, 0, "ab", 9));
        assert_eq!(with.pixels, without.pixels);
        // Next to them, the ranges' neighbours are not invisible: "?" (U+2010 is a hyphen by
        // best fit).
        assert_eq!(f.glyph('\u{2065}').ch, '?');
        assert_eq!(f.glyph('\u{2010}'), f.glyph('-'));
    }

    #[test]
    fn character_counts_stay_one_glyph_each() {
        // An access key after a character the font lacks is underlined under its own letter,
        // and a shortened label keeps whole characters.
        let f = ms_sans_serif();
        let (mut a, mut b) = (Framebuffer::new(80, 13, 0), Framebuffer::new(80, 13, 0));
        f.draw_label(&mut a, (0, 0), "\u{25CF}\u{200B}&Save", 100, 9, true);
        f.draw_label(&mut b, (0, 0), "\u{2022}&Save", 100, 9, true);
        assert_eq!(a.pixels, b.pixels);
        let short = f.elide("\u{25CF} a long title of an unsaved file", 60);
        assert!(short.starts_with('\u{25CF}') && short.ends_with("..."));
        assert!(f.measure(&short) <= 60);
    }

    #[test]
    fn draw_places_ink_and_returns_width() {
        let f = tiny();
        let mut fb = Framebuffer::new(6, 3, 0);
        assert_eq!(f.draw(&mut fb, 0, 0, "aa", 9), 6);
        #[rustfmt::skip]
        let want = [
            9, 9, 0, 9, 9, 0,
            9, 0, 0, 9, 0, 0,
            9, 9, 0, 9, 9, 0,
        ];
        assert_eq!(fb.pixels, want);
    }

    #[test]
    fn elide_matches_dt_end_ellipsis_shape() {
        let f = tiny(); // "a" = 3px, "." = 2px, so "..." = 6px
        assert_eq!(f.elide("aaa", 9), "aaa");
        assert_eq!(f.elide("aaaa", 9), "a...");
        assert_eq!(f.elide("aaaa", 6), "...");
        assert_eq!(f.elide("aaaa", 4), "..");
        assert_eq!(f.elide("aaaa", 0), "");
    }

    #[test]
    fn access_keys() {
        assert_eq!(split_access_key("Sh&ut Down..."), ("Shut Down...".to_string(), Some(2)));
        assert_eq!(split_access_key("&Programs"), ("Programs".to_string(), Some(0)));
        assert_eq!(split_access_key("Tom && Jerry"), ("Tom & Jerry".to_string(), None));
        assert_eq!(split_access_key(&escape_access_keys("AT&T")), ("AT&T".to_string(), None));
        assert_eq!(split_access_key("plain"), ("plain".to_string(), None));
    }

    #[test]
    fn access_key_underline_sits_on_the_bottom_row_under_the_whole_advance() {
        let f = tiny(); // 3px tall; "a" advances 3
        let mut fb = Framebuffer::new(9, 3, 0);
        f.draw_label(&mut fb, (0, 0), "a&a", 100, 9, true);
        // Bottom row (y = 2): the underline covers x 3..=5, the second a's whole advance.
        assert_eq!((3..6).map(|x| fb.get(x, 2)).collect::<Vec<_>>(), [9, 9, 9]);
        assert_eq!(fb.get(2, 2), 0, "nothing under the first a's empty third column");
    }

    #[test]
    fn fold_takes_case_out_in_every_alphabet() {
        assert_eq!(fold("Программы"), "программы");
        assert_eq!(fold("ΠΡΟΓΡΆΜΜΑΤΑ"), "προγράμματα");
        assert_eq!(fold("ŁÓDŹ Česká"), "łódź česká");
        assert_eq!(fold("İstanbul"), "istanbul");
        assert_eq!(fold("ISPARTA ılık"), "isparta ilik");
        assert_eq!(fold("Ελληνικός"), fold("ΕΛΛΗΝΙΚΌΣ"));
        assert_eq!(fold("ĖĮŲ Āā"), "ėįų āā");
        for s in ["İı", "ΣΑΣ", "Straße", "Ǆ"] {
            assert_eq!(fold(s).chars().count(), s.chars().count(), "{s}");
        }
    }

    #[test]
    fn same_name_as_windows_compares_names() {
        assert!(same_name("Файл", "ФАЙЛ"));
        assert!(same_name("Ελληνικά", "ΕΛΛΗΝΙΚΆ"));
        assert!(same_name("łódź", "ŁÓDŹ"));
        assert!(same_name("Readme", "README"));
        assert!(!same_name("straße", "STRASSE"));
        assert!(!same_name("Файл", "Файлы"));
    }

    #[test]
    fn fixture_names() {
        assert_eq!(fixture_name("MS Sans Serif", 11, false), "ms-sans-serif-11px.font");
        assert_eq!(fixture_name("Tahoma", 11, true), "tahoma-11px-bold.font");
    }
}

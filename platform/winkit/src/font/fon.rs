//! The second way to MS Sans Serif's letters (issue 6, "Bevel95 Setup could not be shown"): read
//! straight from Windows' own bitmap font file SSERIFE.FON, when GDI does not hand the font out
//! as asked. The file is only read (never loaded as code): a 16-bit executable whose FONT
//! resources are Windows 3 font files (`.FNT`, versions 2 and 3); the 8pt one at 96 DPI is the
//! 13-pixel cell the looks use. Its bold is the one GDI makes up (MS Sans Serif has none of its
//! own): every row drawn again one pixel to the right, the letter a pixel wider (`tmOverhang` 1).
//! `font::tests::the_font_file_gives_gdis_pixels` proves both against GDI, letter for letter.

use bevel_core::text::Glyph;

use super::{Cell1252, charset_1252, windows_font_file};

/// The cell the looks' MS Sans Serif has (8pt at 96 DPI).
const CELL: u16 = 13;
/// Its baseline, rows below the cell's top.
const ASCENT: u16 = 11;

/// The letters of code page 1252 of MS Sans Serif 8pt from Windows' own SSERIFE.FON.
pub(super) fn ms_sans_serif_1252(bold: bool) -> Result<Cell1252, String> {
    let path = windows_font_file("SSERIFE.FON").ok_or("Windows' folder was not found")?;
    let bytes = std::fs::read(&path).map_err(|e| format!("SSERIFE.FON: {e}"))?;
    let fnt = fonts(&bytes)?.into_iter().find(|f| f.height == CELL && f.vert_res == 96).ok_or("SSERIFE.FON holds no 8pt font at 96 DPI")?;
    // Taken only with the cell's own baseline, as the GDI way requires (the review of 2026-11-23,
    // finding 9).
    if fnt.ascent != ASCENT {
        return Err(format!("SSERIFE.FON's 8pt font has its baseline {} rows down, not {ASCENT}", fnt.ascent));
    }
    let mut glyphs = Vec::new();
    for ch in charset_1252() {
        let Some(byte) = cp1252_byte(ch) else { continue };
        let (width, rows) = fnt.glyph(byte).ok_or_else(|| format!("SSERIFE.FON has no letter {:#04X}", byte))?;
        // Bold as GDI makes it up: each row ORed with itself a pixel to the right.
        let (width, rows): (i32, Vec<Vec<bool>>) = if bold {
            (width + 1, rows.iter().map(|r| (0..=r.len()).map(|x| r.get(x).copied().unwrap_or(false) || (x > 0 && r[x - 1])).collect()).collect())
        } else {
            (width, rows)
        };
        let columns: Vec<usize> = (0..width as usize).filter(|&x| rows.iter().any(|r| r.get(x).copied().unwrap_or(false))).collect();
        let (ink_x, ink_w) = match (columns.first(), columns.last()) {
            (Some(&a), Some(&b)) => (a as i32, (b - a + 1) as i32),
            _ => (0, 0),
        };
        let mut ink = Vec::with_capacity((ink_w * i32::from(CELL)) as usize);
        for r in &rows {
            for x in ink_x..ink_x + ink_w {
                ink.push(r.get(x as usize).copied().unwrap_or(false));
            }
        }
        glyphs.push(Glyph { ch, advance: width, ink_x, ink_w, ink });
    }
    Ok(Cell1252 { glyphs, height: i32::from(CELL), ascent: i32::from(fnt.ascent), overhang: i32::from(bold) })
}

/// The byte of code page 1252 for `ch` (`charset_1252`'s order).
fn cp1252_byte(ch: char) -> Option<u8> {
    const HIGH: [(u32, u8); 27] = [
        (0x20AC, 0x80), (0x201A, 0x82), (0x0192, 0x83), (0x201E, 0x84), (0x2026, 0x85), (0x2020, 0x86), (0x2021, 0x87),
        (0x02C6, 0x88), (0x2030, 0x89), (0x0160, 0x8A), (0x2039, 0x8B), (0x0152, 0x8C), (0x017D, 0x8E), (0x2018, 0x91),
        (0x2019, 0x92), (0x201C, 0x93), (0x201D, 0x94), (0x2022, 0x95), (0x2013, 0x96), (0x2014, 0x97), (0x02DC, 0x98),
        (0x2122, 0x99), (0x0161, 0x9A), (0x203A, 0x9B), (0x0153, 0x9C), (0x017E, 0x9E), (0x0178, 0x9F),
    ];
    let u = ch as u32;
    match u {
        0x20..=0x7E | 0xA0..=0xFF => Some(u as u8),
        _ => HIGH.iter().find(|(c, _)| *c == u).map(|&(_, b)| b),
    }
}

/// One Windows 3 font of a .FON file (what is needed of its header).
struct Fnt<'a> {
    data: &'a [u8],
    version: u16,
    height: u16,
    ascent: u16,
    vert_res: u16,
    first: u8,
    last: u8,
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*d.get(at)?, *d.get(at + 1)?, *d.get(at + 2)?, *d.get(at + 3)?]))
}

impl Fnt<'_> {
    /// Letter `byte`'s width and rows (`height` rows of `width` pixels), or `None` outside the
    /// font's range or past the data.
    fn glyph(&self, byte: u8) -> Option<(i32, Vec<Vec<bool>>)> {
        if byte < self.first || byte > self.last {
            return None;
        }
        let k = usize::from(byte - self.first);
        let (width, offset) = if self.version == 0x200 {
            let e = 118 + 4 * k;
            (u16_at(self.data, e)?, usize::from(u16_at(self.data, e + 2)?))
        } else {
            let e = 148 + 6 * k;
            (u16_at(self.data, e)?, u32_at(self.data, e + 2)? as usize)
        };
        let (w, h) = (usize::from(width), usize::from(self.height));
        // Stored in columns of 8 pixels, each column `height` bytes, top row first.
        let mut rows = vec![vec![false; w]; h];
        for (y, row) in rows.iter_mut().enumerate() {
            for (x, px) in row.iter_mut().enumerate() {
                let b = *self.data.get(offset + (x / 8) * h + y)?;
                *px = b & (0x80 >> (x % 8)) != 0;
            }
        }
        Some((i32::from(width), rows))
    }
}

/// Every FONT resource of a 16-bit executable (.FON), as Windows 3 font files.
fn fonts(file: &[u8]) -> Result<Vec<Fnt<'_>>, String> {
    let bad = || "not a font file Windows 3 could read".to_string();
    if file.get(0..2) != Some(b"MZ") {
        return Err(bad());
    }
    let ne = u32_at(file, 0x3C).ok_or_else(bad)? as usize;
    if file.get(ne..ne + 2) != Some(b"NE") {
        return Err(bad());
    }
    let table = ne + usize::from(u16_at(file, ne + 0x24).ok_or_else(bad)?);
    let shift = u16_at(file, table).ok_or_else(bad)?;
    if shift > 16 {
        return Err(bad());
    }
    let mut at = table + 2;
    let mut out = Vec::new();
    loop {
        let kind = u16_at(file, at).ok_or_else(bad)?;
        if kind == 0 {
            break;
        }
        let count = usize::from(u16_at(file, at + 2).ok_or_else(bad)?);
        at += 8;
        for _ in 0..count {
            let offset = usize::from(u16_at(file, at).ok_or_else(bad)?) << shift;
            let length = usize::from(u16_at(file, at + 2).ok_or_else(bad)?) << shift;
            at += 12;
            // 0x8008: RT_FONT.
            if kind == 0x8008
                && let Some(data) = file.get(offset..offset.saturating_add(length).min(file.len()))
                && let Some(f) = fnt(data)
            {
                out.push(f);
            }
        }
    }
    Ok(out)
}

/// The header of one .FNT (a bitmap font, versions 2 and 3).
fn fnt(data: &[u8]) -> Option<Fnt<'_>> {
    let version = u16_at(data, 0)?;
    let vector = u16_at(data, 66)? & 1 != 0;
    if !(version == 0x200 || version == 0x300) || vector {
        return None;
    }
    Some(Fnt {
        data,
        version,
        vert_res: u16_at(data, 70)?,
        ascent: u16_at(data, 74)?,
        height: u16_at(data, 88)?,
        first: *data.get(95)?,
        last: *data.get(96)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_page_bytes_follow_charset_1252() {
        let chars = charset_1252();
        let bytes: Vec<u8> = chars.iter().map(|&c| cp1252_byte(c).unwrap()).collect();
        assert_eq!(bytes.len(), 218);
        assert!(bytes.windows(2).all(|w| w[0] < w[1]), "in the code page's order, each once");
        assert_eq!(cp1252_byte('\u{20AC}'), Some(0x80));
        assert_eq!(cp1252_byte('\u{0416}'), None);
    }

    #[test]
    fn a_damaged_file_is_an_error_never_a_panic() {
        for bytes in [&b""[..], b"MZ", b"MZ\0\0", &[0u8; 64][..]] {
            assert!(fonts(bytes).is_err());
        }
        let mut f = vec![0u8; 0x100];
        f[0..2].copy_from_slice(b"MZ");
        f[0x3C] = 0x40;
        f[0x40..0x42].copy_from_slice(b"NE");
        f[0x64] = 0xF0; // the resource table past the end
        assert!(fonts(&f).is_err());
    }
}

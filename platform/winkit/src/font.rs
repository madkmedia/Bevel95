//! Renders the classic UI fonts with GDI into `bevel_core` glyph atlases: each character is
//! drawn once, white on black, into a private memory bitmap and read back as 1-bit ink.

use std::ffi::c_void;
use std::ptr::null_mut;

use bevel_core::text::{Font, Glyph};
use windows_sys::Win32::Foundation::SIZE;
use windows_sys::Win32::Graphics::Gdi::{
    ANSI_CHARSET, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection,
    CreateFontW, DEFAULT_PITCH, DIB_RGB_COLORS, DeleteDC, DeleteObject, FF_DONTCARE, FW_BOLD, FW_NORMAL, GdiFlush,
    GetTextExtentPoint32W, GetTextFaceW, GetTextMetricsW, HBITMAP, HDC, HFONT, HGDIOBJ, NONANTIALIASED_QUALITY,
    OUT_DEFAULT_PRECIS, SelectObject, SetBkMode, SetTextColor, TEXTMETRICW, TMPF_TRUETYPE, TRANSPARENT, TextOutW,
};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_UNAWARE, SetThreadDpiAwarenessContext,
};

/// Where a character is drawn inside its scratch bitmap (room for ink left of the pen).
const ORIGIN_X: i32 = 16;
const ORIGIN_Y: i32 = 8;
/// Scratch bitmap size for one character.
const GLYPH_W: i32 = 64;
const GLYPH_H: i32 = 40;

/// The characters every atlas holds: printable Windows-1252, i.e. what a Windows 95 window
/// title could contain. Anything else is drawn as its stand-in, as Windows 95's best fit to
/// this code page had it, or as '?' (`bevel_core::text::Font::glyph`).
pub fn charset() -> Vec<char> {
    // Windows-1252 0x80..0x9F as Unicode (the five unassigned slots left out).
    const CP1252_80_9F: [u32; 27] = [
        0x20AC, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x017D, 0x2018,
        0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x017E, 0x0178,
    ];
    (0x20..=0x7E).chain(CP1252_80_9F).chain(0xA0..=0xFF).filter_map(char::from_u32).collect()
}

/// The fonts of the web view panel of folder windows (Windows 98 and 2000), as (face, px, bold),
/// measured against the Reference PCs' captures (`private/reference/refs/vm/*-truecolor-web-*`, no smoothing):
/// 98's text in Verdana 8pt and its bold, its title Verdana 16pt bold (21 px); 2000's text is the
/// menu font (Tahoma 8pt, the look's own), its title Tahoma 13pt bold (17 px).
pub const WEB_FONTS: [(&str, i32, bool); 4] = [("Verdana", 11, false), ("Verdana", 11, true), ("Verdana", 21, true), ("Tahoma", 17, true)];

/// Renders `face` at `px` pixels per em (GDI lfHeight = -px), non-antialiased.
pub fn render(face: &str, px: i32, bold: bool) -> Result<Font, String> {
    let _dpi = DpiUnaware::enter();
    let mut canvas = Canvas::new(GLYPH_W, GLYPH_H, face, px, bold)?;
    let tm = canvas.metrics;
    if ORIGIN_Y + tm.tmHeight > GLYPH_H {
        return Err(format!("{face} {px}px is too tall for the scratch bitmap"));
    }
    let mut glyphs = Vec::new();
    for ch in charset() {
        let mut buf = [0u16; 2];
        let units = ch.encode_utf16(&mut buf);
        canvas.clear();
        canvas.text_out(ORIGIN_X, ORIGIN_Y, units);
        let advance = canvas.extent(units);

        let (mut min_x, mut max_x) = (i32::MAX, i32::MIN);
        for y in 0..GLYPH_H {
            for x in 0..GLYPH_W {
                if canvas.ink(x, y) {
                    if y < ORIGIN_Y || y >= ORIGIN_Y + tm.tmHeight {
                        return Err(format!("{face}: {ch:?} draws outside its cell"));
                    }
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                }
            }
        }
        if min_x == 0 || max_x == GLYPH_W - 1 {
            return Err(format!("{face}: {ch:?} may be cut off by the scratch bitmap"));
        }
        let (ink_x, ink_w) = if max_x < min_x { (0, 0) } else { (min_x - ORIGIN_X, max_x - min_x + 1) };
        let mut ink = Vec::with_capacity((ink_w * tm.tmHeight) as usize);
        for y in 0..tm.tmHeight {
            for x in 0..ink_w {
                ink.push(canvas.ink(min_x + x, ORIGIN_Y + y));
            }
        }
        glyphs.push(Glyph { ch, advance, ink_x, ink_w, ink });
    }
    Ok(Font::new(face.to_string(), bold, tm.tmHeight, tm.tmAscent, glyphs)?.with_overhang(tm.tmOverhang))
}

/// While alive, GDI on this thread behaves as if the screen were 96 DPI, so font
/// selection picks the 96-DPI raster font file (SSERIFE.FON) even on a high-DPI monitor.
pub struct DpiUnaware(DPI_AWARENESS_CONTEXT);

impl DpiUnaware {
    pub fn enter() -> DpiUnaware {
        // SAFETY: plain Win32 call with a predefined context constant.
        DpiUnaware(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_UNAWARE) })
    }
}

impl Drop for DpiUnaware {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: restores the context returned by the earlier call.
            unsafe { SetThreadDpiAwarenessContext(self.0) };
        }
    }
}

/// A memory DC with a 32-bit top-down DIB and the requested font selected; text is drawn
/// white on black. Everything is released on drop.
struct Canvas {
    dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    font: HFONT,
    old_font: HGDIOBJ,
    bits: *mut u32,
    width: i32,
    height: i32,
    metrics: TEXTMETRICW,
    truetype: bool,
}

impl Canvas {
    fn new(width: i32, height: i32, face: &str, px: i32, bold: bool) -> Result<Canvas, String> {
        let face_w: Vec<u16> = face.encode_utf16().chain([0]).collect();
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // negative = top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: every handle is checked before use; `Drop` releases what was created
        // (a partially built Canvas is never returned, so each error path cleans up here).
        unsafe {
            let dc = CreateCompatibleDC(null_mut());
            if dc.is_null() {
                return Err("CreateCompatibleDC failed".into());
            }
            let mut bits: *mut c_void = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(dc);
                return Err("CreateDIBSection failed".into());
            }
            let old_bitmap = SelectObject(dc, bitmap);
            let weight = if bold { FW_BOLD } else { FW_NORMAL };
            let font = CreateFontW(
                -px,
                0,
                0,
                0,
                weight as i32,
                0,
                0,
                0,
                u32::from(ANSI_CHARSET),
                u32::from(OUT_DEFAULT_PRECIS),
                u32::from(CLIP_DEFAULT_PRECIS),
                u32::from(NONANTIALIASED_QUALITY),
                u32::from(DEFAULT_PITCH | FF_DONTCARE),
                face_w.as_ptr(),
            );
            if font.is_null() {
                SelectObject(dc, old_bitmap);
                DeleteObject(bitmap);
                DeleteDC(dc);
                return Err(format!("CreateFontW({face}) failed"));
            }
            let old_font = SelectObject(dc, font);
            let mut canvas = Canvas {
                dc,
                bitmap,
                old_bitmap,
                font,
                old_font,
                bits: bits.cast(),
                width,
                height,
                metrics: std::mem::zeroed(),
                truetype: false,
            };
            SetTextColor(dc, 0x00FF_FFFF);
            SetBkMode(dc, TRANSPARENT as i32);
            if GetTextMetricsW(dc, &mut canvas.metrics) == 0 {
                return Err("GetTextMetricsW failed".into());
            }
            canvas.truetype = canvas.metrics.tmPitchAndFamily & TMPF_TRUETYPE != 0;
            let mut name = [0u16; 64];
            let n = GetTextFaceW(dc, name.len() as i32, name.as_mut_ptr()).clamp(0, 64) as usize;
            let got = String::from_utf16_lossy(&name[..n]);
            let got = got.trim_end_matches('\0');
            if !got.eq_ignore_ascii_case(face) {
                return Err(format!("asked GDI for \"{face}\" but got \"{got}\" (font not installed?)"));
            }
            Ok(canvas)
        }
    }

    fn clear(&mut self) {
        // SAFETY: `bits` points at the DIB's width*height 32-bit pixels, which live as long
        // as `self`; `&mut self` means nothing else is reading them.
        unsafe { std::slice::from_raw_parts_mut(self.bits, (self.width * self.height) as usize) }.fill(0);
    }

    fn ink(&self, x: i32, y: i32) -> bool {
        assert!(x >= 0 && y >= 0 && x < self.width && y < self.height);
        // SAFETY: in bounds (checked above) of the DIB's pixels, which live as long as `self`.
        let p = unsafe { self.bits.add((y * self.width + x) as usize).read() };
        p & 0x00FF_FFFF != 0
    }

    fn text_out(&self, x: i32, y: i32, text: &[u16]) {
        // SAFETY: valid DC and a pointer/length pair from a live slice. GdiFlush makes sure
        // the drawing reached the bitmap before we read it.
        unsafe {
            TextOutW(self.dc, x, y, text.as_ptr(), text.len() as i32);
            GdiFlush();
        }
    }

    fn extent(&self, text: &[u16]) -> i32 {
        let mut size = SIZE { cx: 0, cy: 0 };
        // SAFETY: valid DC, live slice, and a valid out-pointer.
        unsafe { GetTextExtentPoint32W(self.dc, text.as_ptr(), text.len() as i32, &mut size) };
        size.cx
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        // SAFETY: deselect before deleting, in reverse order of creation.
        unsafe {
            SelectObject(self.dc, self.old_font);
            DeleteObject(self.font);
            SelectObject(self.dc, self.old_bitmap);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use bevel_core::fb::Framebuffer;
    use bevel_core::text::fixture_name;
    use bevel_core::theme::{WIN95, WIN2000};
    use windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2;

    use super::*;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../core/tests/fixtures")
    }

    /// (face, px, bold) for every font the themes use.
    fn all_fonts() -> Vec<(&'static str, i32, bool)> {
        let mut all: Vec<_> = [&WIN95, &WIN2000].into_iter().flat_map(|t| [false, true].map(|b| (t.font_face, t.font_px, b))).collect();
        all.extend(WEB_FONTS);
        all
    }

    /// `text` drawn by GDI in one TextOutW call, as 1/0 pixels.
    fn gdi_string(face: &str, px: i32, bold: bool, text: &str) -> Framebuffer {
        let _dpi = DpiUnaware::enter();
        let mut canvas = Canvas::new(400, GLYPH_H, face, px, bold).unwrap();
        let units: Vec<u16> = text.encode_utf16().collect();
        canvas.clear();
        canvas.text_out(ORIGIN_X, ORIGIN_Y, &units);
        let mut fb = Framebuffer::new(canvas.width, canvas.height, 0);
        for y in 0..canvas.height {
            for x in 0..canvas.width {
                fb.set(x, y, u32::from(canvas.ink(x, y)));
            }
        }
        fb
    }

    fn art(fb: &Framebuffer) -> String {
        let mut s = String::new();
        for y in 0..fb.height {
            s.extend((0..fb.width.min(120)).map(|x| if fb.get(x, y) != 0 { '#' } else { '.' }));
            s.push('\n');
        }
        s
    }

    #[test]
    #[ignore = "writes files; run it with `cargo regen-fonts`"]
    fn regenerate_font_fixtures() {
        std::fs::create_dir_all(fixture_dir()).unwrap();
        for (face, px, bold) in all_fonts() {
            let font = render(face, px, bold).unwrap();
            let path = fixture_dir().join(fixture_name(face, px, bold));
            std::fs::write(&path, font.to_text()).unwrap();
            println!("wrote {}", path.display());
        }
    }

    #[test]
    fn fixtures_match_this_pcs_fonts() {
        for (face, px, bold) in all_fonts() {
            let path = fixture_dir().join(fixture_name(face, px, bold));
            let on_disk = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}. Run `cargo regen-fonts`.", path.display()));
            let fresh = render(face, px, bold).unwrap();
            assert!(
                Font::parse(&on_disk).unwrap() == fresh,
                "{} no longer matches this PC's fonts: run `cargo regen-fonts`, then `cargo regen-golden`",
                path.display()
            );
        }
    }

    #[test]
    fn atlas_draws_strings_exactly_like_gdi() {
        let samples = [
            "Start",
            "3:45 PM",
            "Microsoft Word - Document1",
            "My Computer",
            "Inbox \u{2014} Mail \u{2022} 3 unread\u{2026}",
            "fiji jjj ff Tf Ty AV Wa",
            "\u{dc}n\u{ef}c\u{f6}d\u{e9} \u{201c}quotes\u{201d} \u{20ac}5",
        ];
        for (face, px, bold) in all_fonts() {
            let font = render(face, px, bold).unwrap();
            for s in samples {
                let gdi = gdi_string(face, px, bold, s);
                let mut ours = Framebuffer::new(gdi.width, gdi.height, 0);
                font.draw(&mut ours, ORIGIN_X, ORIGIN_Y, s, 1);
                assert!(
                    ours == gdi,
                    "{face} {px}px bold={bold}: {s:?} differs from GDI\nGDI:\n{}ours:\n{}",
                    art(&gdi),
                    art(&ours)
                );
            }
        }
    }

    #[test]
    fn dpi_awareness_does_not_change_the_atlas() {
        // The taskbar runs per-monitor DPI aware; tests run DPI unaware. Same glyphs either way.
        let unaware = render("MS Sans Serif", 11, false).unwrap();
        // SAFETY: plain Win32 calls; the old context is restored right after.
        let old = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let aware = render("MS Sans Serif", 11, false);
        unsafe { SetThreadDpiAwarenessContext(old) };
        assert!(aware.unwrap() == unaware);
    }

    /// Where Windows draws a menu access-key underline ("&F" underlines the F): the rows
    /// that "&F" drawn by DrawTextW has and a plain "F" does not, from the top of the cell.
    fn gdi_underline_rows(face: &str, px: i32, bold: bool) -> Vec<(i32, i32, i32)> {
        use windows_sys::Win32::Foundation::RECT;
        use windows_sys::Win32::Graphics::Gdi::{DT_LEFT, DT_NOCLIP, DT_SINGLELINE, DT_TOP, DrawTextW};
        let _dpi = DpiUnaware::enter();
        let mut canvas = Canvas::new(GLYPH_W, GLYPH_H, face, px, bold).unwrap();
        canvas.clear();
        canvas.text_out(ORIGIN_X, ORIGIN_Y, &[u16::from(b'F')]);
        let plain: Vec<bool> = (0..GLYPH_W * GLYPH_H).map(|i| canvas.ink(i % GLYPH_W, i / GLYPH_W)).collect();
        canvas.clear();
        let text: Vec<u16> = "&F".encode_utf16().collect();
        let mut rect = RECT { left: ORIGIN_X, top: ORIGIN_Y, right: GLYPH_W - 2, bottom: GLYPH_H - 2 };
        // SAFETY: valid DC, string and rect.
        unsafe {
            DrawTextW(
                canvas.dc,
                text.as_ptr(),
                text.len() as i32,
                &mut rect,
                DT_SINGLELINE | DT_NOCLIP | DT_LEFT | DT_TOP,
            );
            GdiFlush();
        }
        let mut rows = Vec::new();
        for y in 0..GLYPH_H {
            let xs: Vec<i32> =
                (0..GLYPH_W).filter(|&x| canvas.ink(x, y) && !plain[(y * GLYPH_W + x) as usize]).collect();
            if let (Some(first), Some(last)) = (xs.first(), xs.last()) {
                rows.push((y - ORIGIN_Y, first - ORIGIN_X, last - ORIGIN_X));
            }
        }
        rows
    }

    /// Windows draws access-key underlines on the cell's bottom row, under the character's
    /// whole advance; core draws them the same way (`text::Font::draw_label`). Windows 95 and
    /// 98 made them longer by the overhang of a bold they make up themselves (bold MS Sans
    /// Serif: "&Close" in their window menus, seen in the v86 emulator); today's Windows
    /// does not, so here only the overhang itself is checked for bold.
    #[test]
    fn access_key_underline_matches_gdi() {
        for t in [&WIN95, &WIN2000] {
            let font = render(t.font_face, t.font_px, false).unwrap();
            let rows = gdi_underline_rows(t.font_face, t.font_px, false);
            assert_eq!(rows, [(font.height - 1, 0, font.glyph('F').advance - 1)], "{}", t.font_face);
            let bold = render(t.font_face, t.font_px, true).unwrap();
            let rows = gdi_underline_rows(t.font_face, t.font_px, true);
            assert_eq!(rows, [(bold.height - 1, 0, bold.glyph('F').advance - 1)], "{} bold", t.font_face);
        }
        assert_eq!(render("MS Sans Serif", 11, true).unwrap().overhang, 1, "a made-up bold");
        assert_eq!(render("Tahoma", 11, true).unwrap().overhang, 0, "Tahoma has a bold of its own");
    }

    #[test]
    fn ms_sans_serif_is_the_raster_font() {
        let _dpi = DpiUnaware::enter();
        let c = Canvas::new(GLYPH_W, GLYPH_H, "MS Sans Serif", 11, false).unwrap();
        assert!(!c.truetype, "expected the bitmap font SSERIFE.FON, got a TrueType substitute");
        assert_eq!((c.metrics.tmHeight, c.metrics.tmAscent), (13, 11), "expected the 8pt / 96 DPI size");
    }
}

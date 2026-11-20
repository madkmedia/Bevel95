//! Renders the classic UI fonts with GDI into `bevel_core` glyph atlases: each character is
//! drawn once, white on black, into a private memory bitmap and read back as 1-bit ink.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::Mutex;

use bevel_core::text::{Font, Glyph};
use windows_sys::Win32::Foundation::SIZE;
use windows_sys::Win32::Graphics::Gdi::{
    ANSI_CHARSET, AddFontResourceExW, BALTIC_CHARSET, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DEFAULT_PITCH, DIB_RGB_COLORS, DeleteDC, DeleteObject,
    EASTEUROPE_CHARSET, FF_DONTCARE, FR_PRIVATE, FW_BOLD, FW_NORMAL, GREEK_CHARSET, GdiFlush, GetTextExtentPoint32W,
    GetTextFaceW, GetTextMetricsW, HBITMAP, HDC, HFONT, HGDIOBJ, NONANTIALIASED_QUALITY, OUT_DEFAULT_PRECIS, OUT_RASTER_PRECIS,
    RUSSIAN_CHARSET, SelectObject, SetBkMode, SetTextColor, TEXTMETRICW, TMPF_TRUETYPE, TRANSPARENT, TURKISH_CHARSET,
    TextOutW,
};
use windows_sys::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;

use crate::log::{log, log_win32};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_UNAWARE, SetThreadDpiAwarenessContext,
};

/// Where a character is drawn inside its scratch bitmap (room for ink left of the pen).
const ORIGIN_X: i32 = 16;
const ORIGIN_Y: i32 = 8;
/// Scratch bitmap size for one character.
const GLYPH_W: i32 = 64;
const GLYPH_H: i32 = 40;

/// The characters of code page 1252 (printable Windows-1252), what a Windows 95 window title
/// could contain: every atlas draws them from the font asked with `ANSI_CHARSET`, as before
/// other alphabets came (the font fixtures hold exactly these).
pub fn charset_1252() -> Vec<char> {
    // Windows-1252 0x80..0x9F as Unicode (the five unassigned slots left out).
    const CP1252_80_9F: [u32; 27] = [
        0x20AC, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x017D, 0x2018,
        0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x017E, 0x0178,
    ];
    (0x20..=0x7E).chain(CP1252_80_9F).chain(0xA0..=0xFF).filter_map(char::from_u32).collect()
}

/// One of the other European code pages a Windows 95, 98 or 2000 of another language had: its
/// GDI character set, the file of MS Sans Serif 8pt in it (96 DPI, as `SSERIFE.FON` is for
/// 1252; Windows still has them all in its Fonts folder, but registers only its own language's),
/// and the letters it has that 1252 has not (every character of the code page 0x80..0xFF not in
/// `charset_1252`, each listed under the first code page here that has it).
pub struct CodePage {
    pub number: u16,
    pub charset: u8,
    pub ms_sans_serif_file: &'static str,
    pub letters: &'static [u16],
}

/// The letters beyond 1252 the atlases hold: Central European, Cyrillic, Greek, Turkish and
/// Baltic (the brief of 2026-11-19). Chinese, Japanese, Korean, Hebrew, Arabic and Thai, and
/// combining marks, are not here: they stay '?' (`bevel_core::text::Font::glyph`). The lists
/// were asked of Windows' own code page tables (.NET's `Encoding.GetEncoding(n)` for bytes
/// 0x80..0xFF, undefined bytes left out).
pub const OTHER_CODE_PAGES: [CodePage; 5] = [
    CodePage {
        number: 1250,
        charset: EASTEUROPE_CHARSET,
        ms_sans_serif_file: "SSERIFEE.FON",
        letters: &[
            0x015A, 0x0164, 0x0179, 0x015B, 0x0165, 0x017A, 0x02C7, 0x02D8, 0x0141, 0x0104, 0x015E, 0x017B, 0x02DB, 0x0142,
            0x0105, 0x015F, 0x013D, 0x02DD, 0x013E, 0x017C, 0x0154, 0x0102, 0x0139, 0x0106, 0x010C, 0x0118, 0x011A, 0x010E,
            0x0110, 0x0143, 0x0147, 0x0150, 0x0158, 0x016E, 0x0170, 0x0162, 0x0155, 0x0103, 0x013A, 0x0107, 0x010D, 0x0119,
            0x011B, 0x010F, 0x0111, 0x0144, 0x0148, 0x0151, 0x0159, 0x016F, 0x0171, 0x0163, 0x02D9,
        ],
    },
    CodePage {
        number: 1251,
        charset: RUSSIAN_CHARSET,
        ms_sans_serif_file: "SSERIFER.FON",
        letters: &[
            0x0402, 0x0403, 0x0453, 0x0409, 0x040A, 0x040C, 0x040B, 0x040F, 0x0452, 0x0459, 0x045A, 0x045C, 0x045B, 0x045F,
            0x040E, 0x045E, 0x0408, 0x0490, 0x0401, 0x0404, 0x0407, 0x0406, 0x0456, 0x0491, 0x0451, 0x2116, 0x0454, 0x0458,
            0x0405, 0x0455, 0x0457, 0x0410, 0x0411, 0x0412, 0x0413, 0x0414, 0x0415, 0x0416, 0x0417, 0x0418, 0x0419, 0x041A,
            0x041B, 0x041C, 0x041D, 0x041E, 0x041F, 0x0420, 0x0421, 0x0422, 0x0423, 0x0424, 0x0425, 0x0426, 0x0427, 0x0428,
            0x0429, 0x042A, 0x042B, 0x042C, 0x042D, 0x042E, 0x042F, 0x0430, 0x0431, 0x0432, 0x0433, 0x0434, 0x0435, 0x0436,
            0x0437, 0x0438, 0x0439, 0x043A, 0x043B, 0x043C, 0x043D, 0x043E, 0x043F, 0x0440, 0x0441, 0x0442, 0x0443, 0x0444,
            0x0445, 0x0446, 0x0447, 0x0448, 0x0449, 0x044A, 0x044B, 0x044C, 0x044D, 0x044E, 0x044F,
        ],
    },
    CodePage {
        number: 1253,
        charset: GREEK_CHARSET,
        ms_sans_serif_file: "SSERIFEG.FON",
        letters: &[
            0x0385, 0x0386, 0x2015, 0x0384, 0x0388, 0x0389, 0x038A, 0x038C, 0x038E, 0x038F, 0x0390, 0x0391, 0x0392, 0x0393,
            0x0394, 0x0395, 0x0396, 0x0397, 0x0398, 0x0399, 0x039A, 0x039B, 0x039C, 0x039D, 0x039E, 0x039F, 0x03A0, 0x03A1,
            0x03A3, 0x03A4, 0x03A5, 0x03A6, 0x03A7, 0x03A8, 0x03A9, 0x03AA, 0x03AB, 0x03AC, 0x03AD, 0x03AE, 0x03AF, 0x03B0,
            0x03B1, 0x03B2, 0x03B3, 0x03B4, 0x03B5, 0x03B6, 0x03B7, 0x03B8, 0x03B9, 0x03BA, 0x03BB, 0x03BC, 0x03BD, 0x03BE,
            0x03BF, 0x03C0, 0x03C1, 0x03C2, 0x03C3, 0x03C4, 0x03C5, 0x03C6, 0x03C7, 0x03C8, 0x03C9, 0x03CA, 0x03CB, 0x03CC,
            0x03CD, 0x03CE,
        ],
    },
    // Turkish: its other letters (Ş ş) are 1250's too.
    CodePage { number: 1254, charset: TURKISH_CHARSET, ms_sans_serif_file: "SSERIFET.FON", letters: &[0x011E, 0x0130, 0x011F, 0x0131] },
    CodePage {
        number: 1257,
        charset: BALTIC_CHARSET,
        ms_sans_serif_file: "SSEE1257.FON",
        letters: &[
            0x0156, 0x0157, 0x012E, 0x0100, 0x0112, 0x0116, 0x0122, 0x0136, 0x012A, 0x013B, 0x0145, 0x014C, 0x0172, 0x016A,
            0x012F, 0x0101, 0x0113, 0x0117, 0x0123, 0x0137, 0x012B, 0x013C, 0x0146, 0x014D, 0x0173, 0x016B,
        ],
    },
];

/// Every character the program atlases hold (`render`): code page 1252 and the letters of
/// `OTHER_CODE_PAGES`. Anything else is drawn as its stand-in, as Windows 95's best fit to
/// 1252 had it, or as '?' (`bevel_core::text::Font::glyph`).
pub fn charset() -> Vec<char> {
    let others = OTHER_CODE_PAGES.iter().flat_map(|cp| cp.letters.iter().filter_map(|&u| char::from_u32(u32::from(u))));
    charset_1252().into_iter().chain(others).collect()
}

/// The fonts of the web view panel of folder windows (Windows 98 and 2000), as (face, px, bold),
/// measured against the Reference PCs' captures (`private/reference/refs/vm/*-truecolor-web-*`, no smoothing):
/// 98's text in Verdana 8pt and its bold, its title Verdana 16pt bold (21 px); 2000's text is the
/// menu font (Tahoma 8pt, the look's own), its title Tahoma 13pt bold (17 px).
pub const WEB_FONTS: [(&str, i32, bool); 4] = [("Verdana", 11, false), ("Verdana", 11, true), ("Verdana", 21, true), ("Tahoma", 17, true)];

/// Renders `face` at `px` pixels per em (GDI lfHeight = -px), non-antialiased, with every
/// character of `charset`: the letters of code page 1252 exactly as `render_1252` draws them
/// (the font asked with `ANSI_CHARSET`), and each other code page's letters from the same face
/// asked with that code page's character set (`other_letters`), set on the same baseline. What
/// every program draws with.
pub fn render(face: &str, px: i32, bold: bool) -> Result<Font, String> {
    give_core_ms_sans_serif();
    let _dpi = DpiUnaware::enter();
    let mut canvas = Canvas::new(GLYPH_W, GLYPH_H, face, px, bold)?;
    let tm = canvas.metrics;
    let mut glyphs = canvas.glyphs(face, &charset_1252(), tm.tmAscent, tm.tmHeight, true)?;
    drop(canvas);
    for cp in &OTHER_CODE_PAGES {
        glyphs.extend(other_letters(cp, face, px, bold, tm.tmAscent, tm.tmHeight));
    }
    Ok(Font::new(face.to_string(), bold, tm.tmHeight, tm.tmAscent, glyphs)?.with_overhang(tm.tmOverhang))
}

/// `render` with the letters of code page 1252 only: what the font fixtures in
/// `core/tests/fixtures` hold, and the font of what shows only Bevel95's own words.
pub fn render_1252(face: &str, px: i32, bold: bool) -> Result<Font, String> {
    let _dpi = DpiUnaware::enter();
    let mut canvas = Canvas::new(GLYPH_W, GLYPH_H, face, px, bold)?;
    let tm = canvas.metrics;
    let glyphs = canvas.glyphs(face, &charset_1252(), tm.tmAscent, tm.tmHeight, true)?;
    Ok(Font::new(face.to_string(), bold, tm.tmHeight, tm.tmAscent, glyphs)?.with_overhang(tm.tmOverhang))
}

/// The glyphs of `cp`'s letters in `face`, for a font whose cell is `height` tall with its
/// baseline `ascent` below the top. The face is asked with the code page's character set, as a
/// Windows of that language asked it. For MS Sans Serif, that code page's own bitmap file
/// (`CodePage::ms_sans_serif_file`) is loaded for this process alone before the face is first
/// asked in that code page (`AddFontResourceExW` with `FR_PRIVATE`: nothing in Windows changes;
/// a Windows of that language, which has it already, has it twice, which changes nothing either),
/// and it is asked as a bitmap font is sized, by its cell (`Ask::bitmap_cell`). Loaded only after
/// an ask had failed, as first built, the next ask in the same process got other letters under
/// the same name (the bar: a TrueType font 14 px tall; the tests: wider letters), and only the
/// asks after that the file's own (2026-11-19). Where a PC has no such
/// font, Windows' substitute is taken, drawn without smoothing at the same size and cut to the
/// cell; where nothing can be drawn, nothing is returned and those letters stay '?'. What came
/// instead of the face asked for is logged once.
fn other_letters(cp: &CodePage, face: &str, px: i32, bold: bool, ascent: i32, height: i32) -> Vec<Glyph> {
    let letters: Vec<char> = cp.letters.iter().filter_map(|&u| char::from_u32(u32::from(u))).collect();
    let bitmap_face = face.eq_ignore_ascii_case("MS Sans Serif");
    let ask = if bitmap_face { Ask::bitmap_cell(height, cp.charset) } else { Ask::em(px, cp.charset) };
    let _dpi = DpiUnaware::enter();
    let open = || Canvas::asked(GLYPH_W, GLYPH_H, face, bold, ask);
    // MS Sans Serif's own: the face, a bitmap font, the cell of the code page 1252 one.
    let is_own = |c: &Canvas| c.face.eq_ignore_ascii_case(face) && (!bitmap_face || (!c.truetype && c.metrics.tmHeight == height));
    let fail = |e: String| log_once(format!("font: {face} for code page {}: {e}; its letters show as '?'", cp.number));
    if bitmap_face {
        load_privately(cp);
    }
    let mut canvas = match open() {
        Ok(c) => c,
        Err(e) => {
            fail(e);
            return Vec::new();
        }
    };
    if !is_own(&canvas) {
        let kind = if canvas.truetype { "TrueType" } else { "bitmap" };
        log_once(format!(
            "font: {face} for code page {} is not on this PC; Windows gives {} ({kind}, {} px tall)",
            cp.number, canvas.face, canvas.metrics.tmHeight
        ));
    }
    canvas.glyphs(face, &letters, ascent, height, false).unwrap_or_else(|e| {
        fail(e);
        Vec::new()
    })
}

/// Loads `cp`'s MS Sans Serif file from Windows' Fonts folder for this process alone, once
/// (whether it worked is remembered). False when there is no such file or it would not load.
fn load_privately(cp: &CodePage) -> bool {
    static TRIED: Mutex<Vec<(u16, bool)>> = Mutex::new(Vec::new());
    let mut tried = TRIED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(&(_, ok)) = tried.iter().find(|(n, _)| *n == cp.number) {
        return ok;
    }
    let mut dir = [0u16; 260];
    // SAFETY: a valid buffer and its length.
    let n = unsafe { GetSystemWindowsDirectoryW(dir.as_mut_ptr(), dir.len() as u32) } as usize;
    let ok = n > 0 && n < dir.len() && {
        let path = PathBuf::from(String::from_utf16_lossy(&dir[..n])).join("Fonts").join(cp.ms_sans_serif_file);
        path.is_file() && {
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
            // SAFETY: a null-terminated path; FR_PRIVATE keeps the font to this process, and it
            // goes with the process (never removed earlier: atlases are made again on a change).
            let loaded = unsafe { AddFontResourceExW(wide.as_ptr(), FR_PRIVATE, null()) };
            if loaded == 0 {
                log_win32(&format!("AddFontResourceExW({})", cp.ms_sans_serif_file));
            }
            loaded > 0
        }
    };
    tried.push((cp.number, ok));
    ok
}

/// Logs `line` the first time this process has it (a program makes its atlases again whenever
/// the look changes).
fn log_once(line: String) {
    static SAID: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let mut said = SAID.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !said.contains(&line) {
        log(&line);
        said.push(line);
    }
}

/// Gives the core MS Sans Serif with every letter of `charset` for the windows that use it
/// whatever the look (`bevel_core::text::ms_sans_serif`: classic dialog templates and Help's
/// controls), in place of its 1252-only fixture. Once per process, before anything is painted
/// (every program renders its look's font first).
fn give_core_ms_sans_serif() {
    bevel_core::text::use_platform_ms_sans_serif(|| render("MS Sans Serif", 11, false).ok());
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

/// How a font is asked of GDI: the LOGFONT height, the character set, and whether bitmap fonts
/// come first.
#[derive(Clone, Copy)]
struct Ask {
    lf_height: i32,
    charset: u8,
    raster: bool,
}

impl Ask {
    /// `px` pixels per em (GDI lfHeight = -px), any kind of font: how every atlas asks for code
    /// page 1252, and Tahoma and Verdana for every code page.
    fn em(px: i32, charset: u8) -> Ask {
        Ask { lf_height: -px, charset, raster: false }
    }

    /// A bitmap font as Windows sizes one, by its whole cell (a positive lfHeight), bitmap fonts
    /// first (`OUT_RASTER_PRECIS`): MS Sans Serif in another code page, so that nothing but the
    /// code page's own bitmap file at the cell of the 1252 one answers.
    fn bitmap_cell(cell: i32, charset: u8) -> Ask {
        Ask { lf_height: cell, charset, raster: true }
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
    /// The face GDI really gave.
    face: String,
}

impl Canvas {
    /// `face` asked with `ANSI_CHARSET`; an error if GDI gives another face.
    fn new(width: i32, height: i32, face: &str, px: i32, bold: bool) -> Result<Canvas, String> {
        let canvas = Canvas::asked(width, height, face, bold, Ask::em(px, ANSI_CHARSET))?;
        if !canvas.face.eq_ignore_ascii_case(face) {
            return Err(format!("asked GDI for \"{face}\" but got \"{}\" (font not installed?)", canvas.face));
        }
        Ok(canvas)
    }

    /// The glyph of each of `chars`, for a cell `height` tall with its baseline `ascent` below
    /// the top (this canvas's font is set on that baseline). `strict`: ink outside the cell is
    /// an error (the font of the cell itself); else it is cut off (a substitute's taller letters).
    fn glyphs(&mut self, face: &str, chars: &[char], ascent: i32, height: i32, strict: bool) -> Result<Vec<Glyph>, String> {
        let top = ORIGIN_Y + ascent - self.metrics.tmAscent;
        if ORIGIN_Y + height > GLYPH_H || top < 0 || top + self.metrics.tmHeight > GLYPH_H {
            return Err(format!("{face} is too tall for the scratch bitmap"));
        }
        let in_cell = |y: i32| y >= ORIGIN_Y && y < ORIGIN_Y + height;
        let mut glyphs = Vec::with_capacity(chars.len());
        for &ch in chars {
            let mut buf = [0u16; 2];
            let units = ch.encode_utf16(&mut buf);
            self.clear();
            self.text_out(ORIGIN_X, top, units);
            let advance = self.extent(units);

            let (mut min_x, mut max_x) = (i32::MAX, i32::MIN);
            for y in 0..GLYPH_H {
                for x in 0..GLYPH_W {
                    if self.ink(x, y) {
                        if !in_cell(y) {
                            if strict {
                                return Err(format!("{face}: {ch:?} draws outside its cell"));
                            }
                            continue;
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
            let mut ink = Vec::with_capacity((ink_w * height) as usize);
            for y in 0..height {
                for x in 0..ink_w {
                    ink.push(self.ink(min_x + x, ORIGIN_Y + y));
                }
            }
            glyphs.push(Glyph { ch, advance, ink_x, ink_w, ink });
        }
        Ok(glyphs)
    }

    /// `face` asked as `ask` says, whatever face GDI gives (`face` says which).
    fn asked(width: i32, height: i32, face: &str, bold: bool, ask: Ask) -> Result<Canvas, String> {
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
            let precision = if ask.raster { OUT_RASTER_PRECIS } else { OUT_DEFAULT_PRECIS };
            let font = CreateFontW(
                ask.lf_height,
                0,
                0,
                0,
                weight as i32,
                0,
                0,
                0,
                u32::from(ask.charset),
                u32::from(precision),
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
                face: String::new(),
            };
            SetTextColor(dc, 0x00FF_FFFF);
            SetBkMode(dc, TRANSPARENT as i32);
            if GetTextMetricsW(dc, &mut canvas.metrics) == 0 {
                return Err("GetTextMetricsW failed".into());
            }
            canvas.truetype = canvas.metrics.tmPitchAndFamily & TMPF_TRUETYPE != 0;
            let mut name = [0u16; 64];
            let n = GetTextFaceW(dc, name.len() as i32, name.as_mut_ptr()).clamp(0, 64) as usize;
            canvas.face = String::from_utf16_lossy(&name[..n]).trim_end_matches('\0').to_string();
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
        gdi_string_in(face, bold, Ask::em(px, ANSI_CHARSET), text)
    }

    /// `gdi_string` with the font asked as `ask` says.
    fn gdi_string_in(face: &str, bold: bool, ask: Ask, text: &str) -> Framebuffer {
        let _dpi = DpiUnaware::enter();
        let mut canvas = Canvas::asked(400, GLYPH_H, face, bold, ask).unwrap();
        assert!(canvas.face.eq_ignore_ascii_case(face), "asked for {face}, got {}", canvas.face);
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
            let font = render_1252(face, px, bold).unwrap();
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
            let fresh = render_1252(face, px, bold).unwrap();
            assert!(
                Font::parse(&on_disk).unwrap() == fresh,
                "{} no longer matches this PC's fonts: run `cargo regen-fonts`, then `cargo regen-golden`",
                path.display()
            );
        }
    }

    /// The other alphabets change no letter of code page 1252: `render` holds `render_1252`'s
    /// glyphs unchanged, with the same cell, and only adds.
    #[test]
    fn other_alphabets_change_nothing_of_1252() {
        for (face, px, bold) in all_fonts() {
            let (all, old) = (render(face, px, bold).unwrap(), render_1252(face, px, bold).unwrap());
            assert_eq!((all.height, all.ascent, all.overhang), (old.height, old.ascent, old.overhang), "{face} {px} {bold}");
            for g in old.glyphs() {
                assert!(all.glyph(g.ch) == g, "{face} {px}px bold={bold}: {:?} changed", g.ch);
            }
            assert_eq!(all.glyphs().len(), old.glyphs().len() + 250, "{face} {px}px bold={bold}: every letter added once");
        }
    }

    /// The test letters: Cyrillic Zhe, Greek Omega, Polish l with stroke, Turkish g with breve and
    /// dotless i, Latvian/Lithuanian e with dot, each with its capital.
    const TEST_LETTERS: [char; 12] = [
        '\u{0416}', '\u{0436}', '\u{03A9}', '\u{03C9}', '\u{0141}', '\u{0142}', '\u{011E}', '\u{011F}', '\u{0131}', '\u{0130}',
        '\u{0116}', '\u{0117}',
    ];

    /// On this PC, a Cyrillic, a Greek, a Polish, a Turkish and a Baltic letter each get a glyph of
    /// their own, not the '?' one, in each look's font and its bold.
    #[test]
    fn other_alphabets_get_glyphs_of_their_own() {
        for t in [&WIN95, &WIN2000] {
            for bold in [false, true] {
                let font = render(t.font_face, t.font_px, bold).unwrap();
                let question = font.glyph('?');
                for ch in TEST_LETTERS {
                    let g = font.glyph(ch);
                    assert_eq!(g.ch, ch, "{} bold={bold}: U+{:04X} has no glyph", t.font_face, ch as u32);
                    assert!(g.ink_w > 0 && g.ink != question.ink, "{} bold={bold}: U+{:04X} looks like '?'", t.font_face, ch as u32);
                }
                // Not this step: Hebrew, Chinese.
                assert_eq!(font.glyph('\u{05D0}').ch, '?');
                assert_eq!(font.glyph('\u{4E2D}').ch, '?');
            }
        }
    }

    /// For 95 and 98, MS Sans Serif's other letters come from that code page's own bitmap file
    /// (loaded for this process alone where Windows does not give it), at the 8pt cell of
    /// SSERIFE.FON; for 2000, Tahoma has them itself.
    #[test]
    fn other_alphabets_come_from_the_looks_own_face() {
        render("MS Sans Serif", 11, false).unwrap();
        let _dpi = DpiUnaware::enter();
        for cp in &OTHER_CODE_PAGES {
            let c = Canvas::asked(GLYPH_W, GLYPH_H, "MS Sans Serif", false, Ask::bitmap_cell(13, cp.charset)).unwrap();
            assert_eq!((c.face.as_str(), c.truetype), ("MS Sans Serif", false), "code page {}", cp.number);
            assert_eq!((c.metrics.tmHeight, c.metrics.tmAscent), (13, 11), "code page {}", cp.number);
            assert_eq!(u32::from(c.metrics.tmCharSet), u32::from(cp.charset), "code page {}", cp.number);
            let t = Canvas::asked(GLYPH_W, GLYPH_H, "Tahoma", false, Ask::em(11, cp.charset)).unwrap();
            assert_eq!(t.face, "Tahoma", "code page {}", cp.number);
        }
    }

    /// A word of each code page, drawn from the atlas, is what GDI draws in one TextOutW call with
    /// the font asked in that code page.
    #[test]
    fn other_alphabets_draw_strings_exactly_like_gdi() {
        let words = [
            (1250, "Kraj\u{f3}w \u{141}\u{f3}d\u{17a} \u{10c}esk\u{e1} \u{15e}\u{165}"),
            (1251, "\u{41f}\u{443}\u{441}\u{43a} \u{41f}\u{440}\u{43e}\u{433}\u{440}\u{430}\u{43c}\u{43c}\u{44b} \u{401}\u{436}"),
            (1253, "\u{388}\u{3bd}\u{3b1}\u{3c1}\u{3be}\u{3b7} \u{3a0}\u{3c1}\u{3bf}\u{3b3}\u{3c1}\u{3ac}\u{3bc}\u{3bc}\u{3b1}\u{3c4}\u{3b1}"),
            (1254, "Ba\u{15f}lat \u{130}stanbul \u{11f}\u{131}"),
            (1257, "Pr\u{101}gr\u{101}mas \u{117}\u{12f}\u{173}"),
        ];
        for (face, px, bold) in all_fonts() {
            let font = render(face, px, bold).unwrap();
            for (number, word) in words {
                let cp = OTHER_CODE_PAGES.iter().find(|cp| cp.number == number).unwrap();
                let ask = if face == "MS Sans Serif" { Ask::bitmap_cell(font.height, cp.charset) } else { Ask::em(px, cp.charset) };
                let gdi = gdi_string_in(face, bold, ask, word);
                let mut ours = Framebuffer::new(gdi.width, gdi.height, 0);
                font.draw(&mut ours, ORIGIN_X, ORIGIN_Y, word, 1);
                assert!(ours == gdi, "{face} {px}px bold={bold}: {word:?} differs from GDI\nGDI:\n{}ours:\n{}", art(&gdi), art(&ours));
            }
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

    /// What fetching a look's two fonts costs at a program's start (run it in release:
    /// `cargo test -p bevel-winkit --release --lib atlas_timing -- --ignored --nocapture`).
    #[test]
    #[ignore = "a measurement, not a check"]
    fn atlas_timing() {
        let looks = [("95/98", WIN95.font_face, WIN95.font_px), ("2000", WIN2000.font_face, WIN2000.font_px)];
        for _ in 0..2 {
            for (look, face, px) in looks {
                let mut times = Vec::new();
                for _ in 0..10 {
                    let t = std::time::Instant::now();
                    let (r, b) = (render(face, px, false).unwrap(), render(face, px, true).unwrap());
                    times.push(t.elapsed().as_secs_f64() * 1000.0);
                    std::hint::black_box((r, b));
                }
                times.sort_by(f64::total_cmp);
                println!("{look} ({face}): regular and bold in {:.1} ms (median of 10; fastest {:.1}, slowest {:.1})", times[5], times[0], times[9]);
            }
        }
    }

    #[test]
    fn ms_sans_serif_is_the_raster_font() {
        let _dpi = DpiUnaware::enter();
        let c = Canvas::new(GLYPH_W, GLYPH_H, "MS Sans Serif", 11, false).unwrap();
        assert!(!c.truetype, "expected the bitmap font SSERIFE.FON, got a TrueType substitute");
        assert_eq!((c.metrics.tmHeight, c.metrics.tmAscent), (13, 11), "expected the 8pt / 96 DPI size");
    }
}

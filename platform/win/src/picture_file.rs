//! Reading a wallpaper's picture file: Windows 2000's JPEGs (their bytes in `bevel-art`) and a
//! person's own picture (BMP, DIB, JPEG, PNG, GIF's first picture), decoded by Windows itself
//! through GDI+'s flat functions (`gdiplus.dll`, part of every Windows; no crate).
//!
//! Why GDI+ and not Windows' imaging component (WIC): both decode every kind asked for and give
//! the camera's orientation; GDI+ is a set of plain C functions, WIC a set of COM interfaces that
//! windows-sys does not describe (each method would be called through a hand-written table). GDI+
//! takes the bytes from a stream in memory (`SHCreateMemStream`), so the file is read by Bevel95
//! first, its size checked before anything is decoded, and never held open by GDI+.
//!
//! - Upright: the orientation a camera writes into a photo (EXIF 0x0112) turns the picture
//!   (`display::Picture::upright`).
//! - A file that is not a picture, is cut short, or cannot be opened gives `Unreadable`, never a
//!   crash; a picture beyond the limits (`display::MAX_SIDE`, `MAX_PIXELS`, `MAX_FILE_BYTES`) is
//!   refused the same way, before it is decoded.
//! - Called only on a thread of its own (`pictures`): a file on a slow or gone drive holds up
//!   nobody else.
//! - Nothing of a file's name or path is logged here.

use std::io::Read;
use std::path::Path;
use std::ptr::null_mut;
use std::sync::OnceLock;

use bevel_core::display::{self, Picture};
use windows_sys::Win32::Graphics::GdiPlus::{
    BitmapData, GdipBitmapLockBits, GdipBitmapUnlockBits, GdipCreateBitmapFromStream, GdipDisposeImage, GdipGetImageHeight,
    GdipGetImageWidth, GdipGetPropertyItem, GdipGetPropertyItemSize, GdiplusStartup, GdiplusStartupInput, GdiplusStartupOutput,
    GpBitmap, GpImage, ImageLockModeRead, Ok as GDIP_OK, PropertyItem, PropertyTagOrientation, Rect as GpRect,
};
use windows_sys::Win32::UI::Shell::SHCreateMemStream;

/// GDI+'s 32 bits a pixel with its see-through value, not premultiplied (0xAARRGGBB in memory as
/// B, G, R, A: the framebuffer's own order). Not among windows-sys' constants.
const PIXEL_FORMAT_32BPP_ARGB: i32 = 0x0026_200A;

/// Why a picture could not be taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The file could not be opened or read, is no picture Windows can decode, or is cut short.
    Unreadable,
    /// It is larger than Bevel95 takes.
    TooLarge,
}

/// Starts GDI+ once for the process (it is never shut down: the process ends with it).
fn gdiplus() -> bool {
    static STARTED: OnceLock<bool> = OnceLock::new();
    *STARTED.get_or_init(|| {
        let input = GdiplusStartupInput { GdiplusVersion: 1, DebugEventCallback: 0, SuppressBackgroundThread: 0, SuppressExternalCodecs: 0 };
        let mut output = GdiplusStartupOutput::default();
        let mut token = 0usize;
        // SAFETY: valid in and out structs; the token is kept by nobody (never shut down).
        unsafe { GdiplusStartup(&mut token, &input, &mut output) == GDIP_OK }
    })
}

/// Reads the file at `path` whole (at most `display::MAX_FILE_BYTES`) and decodes it (the tests'
/// way in; the bar reads in two steps, `load_file` then `read_owned`, and looks between them).
#[cfg(test)]
pub fn read_file(path: &Path) -> Result<Picture, Refused> {
    read_owned(std::borrow::Cow::Owned(load_file(path)?))
}

/// The file's bytes, whole (at most `display::MAX_FILE_BYTES`), not yet decoded.
pub fn load_file(path: &Path) -> Result<Vec<u8>, Refused> {
    let file = std::fs::File::open(path).map_err(|_| Refused::Unreadable)?;
    let len = file.metadata().map_err(|_| Refused::Unreadable)?.len();
    if len > display::MAX_FILE_BYTES {
        return Err(Refused::TooLarge);
    }
    let mut bytes = Vec::with_capacity(len as usize);
    file.take(display::MAX_FILE_BYTES + 1).read_to_end(&mut bytes).map_err(|_| Refused::Unreadable)?;
    if bytes.len() as u64 > display::MAX_FILE_BYTES {
        return Err(Refused::TooLarge);
    }
    Ok(bytes)
}

/// Decodes a picture file's bytes; bytes of its own go as soon as the stream has its copy of them.
pub fn read_owned(bytes: std::borrow::Cow<'_, [u8]>) -> Result<Picture, Refused> {
    let stream = Stream::of(&bytes)?;
    drop(bytes);
    stream.decode()
}

/// The kinds of file taken, told by their first bytes (never by the file's name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Bmp,
    Jpeg,
    Png,
    Gif,
}

fn kind(bytes: &[u8]) -> Option<Kind> {
    if bytes.starts_with(b"BM") {
        Some(Kind::Bmp)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Kind::Jpeg)
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(Kind::Png)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Kind::Gif)
    } else {
        None
    }
}

/// Whether the file has its end: GDI+ decodes a picture cut short as far as it goes and fills the
/// rest, without saying so. Each kind is walked part by part to where the picture ends, so that
/// bytes after that end are no harm (a phone writes a short film after a photo's end; tools leave
/// padding): a PNG to its IEND part, a GIF to its last block (0x3B), a JPEG through its last scan
/// to its end mark (FF D9; a small picture of the camera's own inside its header is skipped with
/// the header), a BMP holds all the rows its header promises. A file that runs out before its end
/// is not whole.
fn whole(kind: Kind, b: &[u8]) -> bool {
    match kind {
        Kind::Png => png_whole(b),
        Kind::Gif => gif_whole(b),
        Kind::Jpeg => jpeg_whole(b),
        Kind::Bmp => bmp_whole(b),
    }
}

/// A PNG's parts (length, kind, data, check) from after its 8 first bytes to the end of IEND.
fn png_whole(b: &[u8]) -> bool {
    let mut at = 8usize;
    loop {
        let Some(len) = b.get(at..at + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]])) else { return false };
        let Some(kind) = b.get(at + 4..at + 8) else { return false };
        // The part's data and its four check bytes.
        let Some(end) = (at + 12).checked_add(len as usize) else { return false };
        if end > b.len() {
            return false;
        }
        if kind == b"IEND" {
            return true;
        }
        at = end;
    }
}

/// A GIF's blocks after its screen descriptor (and its colors) to its last block, 0x3B.
fn gif_whole(b: &[u8]) -> bool {
    /// Skips data sub-blocks (a size byte, that many bytes, ...) to the one of size 0.
    fn sub_blocks(b: &[u8], mut at: usize) -> Option<usize> {
        loop {
            let n = usize::from(*b.get(at)?);
            at += 1;
            if n == 0 {
                return Some(at);
            }
            at += n;
        }
    }
    let colors = |packed: u8| if packed & 0x80 != 0 { 3usize << ((packed & 7) + 1) } else { 0 };
    let Some(&packed) = b.get(10) else { return false };
    let mut at = 13 + colors(packed);
    loop {
        match b.get(at) {
            Some(0x3B) => return true,
            // An extension: its label, then its sub-blocks.
            Some(0x21) => match sub_blocks(b, at + 2) {
                Some(next) => at = next,
                None => return false,
            },
            // A picture: its descriptor (9 bytes), its own colors, the code size, its sub-blocks.
            Some(0x2C) => {
                let Some(&packed) = b.get(at + 9) else { return false };
                match sub_blocks(b, at + 10 + colors(packed) + 1) {
                    Some(next) => at = next,
                    None => return false,
                }
            }
            _ => return false,
        }
    }
}

/// A JPEG's parts from after its start mark: each part with a length is skipped whole (the APP1
/// part a camera writes, with its own small picture inside, among them); after a scan's header the
/// coded data runs to the next mark (FF followed by neither 00 nor a restart mark D0..D7). The end
/// mark FF D9 reached that way is the picture's end. A file that runs out first is not whole.
fn jpeg_whole(b: &[u8]) -> bool {
    let mut at = 2usize;
    let mut scanned = false;
    loop {
        // A mark: FF (any number of them, as fill), then its kind.
        if b.get(at) != Some(&0xFF) {
            return false;
        }
        while b.get(at) == Some(&0xFF) {
            at += 1;
        }
        let Some(&mark) = b.get(at) else { return false };
        at += 1;
        match mark {
            0xD9 => return scanned,
            // Marks without a length.
            0x01 | 0xD0..=0xD7 => continue,
            _ => {}
        }
        let Some(len) = b.get(at..at + 2).map(|s| usize::from(u16::from_be_bytes([s[0], s[1]]))) else { return false };
        if len < 2 || at + len > b.len() {
            return false;
        }
        at += len;
        if mark == 0xDA {
            // The coded data, to the next mark.
            scanned = true;
            loop {
                let Some(p) = b.get(at..).and_then(|rest| rest.iter().position(|&c| c == 0xFF)) else { return false };
                at += p;
                match b.get(at + 1) {
                    None => return false,
                    Some(0x00 | 0xD0..=0xD7) => at += 2,
                    Some(_) => break,
                }
            }
        }
    }
}

/// A BMP holds the rows its header promises (sums checked: a header that says more than any file
/// can hold is not whole).
fn bmp_whole(b: &[u8]) -> bool {
    let u32_at = |i: usize| b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    let (Some(off), Some(header)) = (u32_at(10), u32_at(14)) else { return false };
    if header < 40 {
        // The old OS/2 header: its rows are not counted here; GDI+ decides.
        return true;
    }
    let (Some(w), Some(h), Some(compression), Some(size)) = (u32_at(18), u32_at(22), u32_at(30), u32_at(34)) else {
        return false;
    };
    let bits = b.get(28..30).map_or(0, |s| u64::from(u16::from_le_bytes([s[0], s[1]])));
    let (w, h) = (u64::from((w as i32).max(0) as u32), u64::from((h as i32).unsigned_abs()));
    let need = match compression {
        // Plain rows (and bit fields): each row padded to four bytes.
        0 | 3 => w.checked_mul(bits).map(|v| v.div_ceil(32)).and_then(|v| v.checked_mul(4)).and_then(|v| v.checked_mul(h)),
        _ => Some(u64::from(size)),
    };
    need.and_then(|n| u64::from(off).checked_add(n)).is_some_and(|end| end <= b.len() as u64)
}

/// Decodes a picture file's bytes (its first picture), upright: a BMP or DIB, JPEG, PNG or GIF,
/// told by its first bytes; anything else (another kind GDI+ would read, a metafile) is refused.
#[cfg(test)]
pub fn read_bytes(bytes: &[u8]) -> Result<Picture, Refused> {
    Stream::of(bytes)?.decode()
}

/// A picture file's bytes in a stream of GDI+'s (`SHCreateMemStream`, which keeps its own copy of
/// them), checked first: a kind taken, the file whole. Released when dropped.
struct Stream(*mut core::ffi::c_void);

impl Stream {
    fn of(bytes: &[u8]) -> Result<Stream, Refused> {
        if bytes.len() as u64 > display::MAX_FILE_BYTES {
            return Err(Refused::TooLarge);
        }
        let Some(kind) = kind(bytes) else { return Err(Refused::Unreadable) };
        if !whole(kind, bytes) || !gdiplus() {
            return Err(Refused::Unreadable);
        }
        // SAFETY: the stream copies the bytes; it is released when dropped.
        let stream = unsafe { SHCreateMemStream(bytes.as_ptr(), bytes.len() as u32) };
        if stream.is_null() {
            return Err(Refused::Unreadable);
        }
        Ok(Stream(stream))
    }

    /// Decodes the picture (its first), upright. GDI+'s bitmap and the stream go before the
    /// picture is turned, so that a turned photo is never held three times over.
    fn decode(self) -> Result<Picture, Refused> {
        let mut bitmap: *mut GpBitmap = null_mut();
        // SAFETY: a live stream; GDI+ keeps its own reference while the bitmap lives.
        let made = unsafe { GdipCreateBitmapFromStream(self.0 as _, &mut bitmap) } == GDIP_OK && !bitmap.is_null();
        let result = if made { decode(bitmap) } else { Err(Refused::Unreadable) };
        if !bitmap.is_null() {
            // SAFETY: the bitmap made above, disposed once.
            unsafe { GdipDisposeImage(bitmap as *mut GpImage) };
        }
        drop(self);
        result.map(|(picture, orientation)| picture.upright(orientation))
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: our reference to the stream (IUnknown::Release, the third entry of its table),
        // released once.
        unsafe {
            let table = *(self.0 as *const *const usize);
            let release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32 = std::mem::transmute(*table.add(2));
            release(self.0);
        }
    }
}

/// The bitmap's pixels as 0xAARRGGBB, its size checked first, with the orientation a camera wrote
/// (the caller turns it upright once GDI+'s bitmap is gone).
fn decode(bitmap: *mut GpBitmap) -> Result<(Picture, u16), Refused> {
    let image = bitmap as *mut GpImage;
    let (mut w, mut h) = (0u32, 0u32);
    // SAFETY: a live bitmap; plain queries into locals.
    unsafe {
        if GdipGetImageWidth(image, &mut w) != GDIP_OK || GdipGetImageHeight(image, &mut h) != GDIP_OK {
            return Err(Refused::Unreadable);
        }
    }
    if !display::size_taken(i64::from(w), i64::from(h)) {
        return Err(if w == 0 || h == 0 { Refused::Unreadable } else { Refused::TooLarge });
    }
    let (w, h) = (w as i32, h as i32);
    let orientation = orientation(image);
    let mut pixels = vec![0u32; (w as usize) * (h as usize)];
    let rect = GpRect { X: 0, Y: 0, Width: w, Height: h };
    // GDI+ writes straight into `pixels` (ImageLockModeUserInputBuf: 4), rows of w * 4 bytes.
    let mut data = BitmapData { Width: w as u32, Height: h as u32, Stride: w * 4, PixelFormat: PIXEL_FORMAT_32BPP_ARGB, Scan0: pixels.as_mut_ptr().cast(), Reserved: 0 };
    // SAFETY: `data` describes `pixels` exactly (w * h 32-bit pixels, stride w * 4); a short or
    // broken file makes the lock fail, which is checked.
    let locked = unsafe { GdipBitmapLockBits(bitmap, &rect, (ImageLockModeRead | 4) as u32, PIXEL_FORMAT_32BPP_ARGB, &mut data) } == GDIP_OK;
    if !locked {
        return Err(Refused::Unreadable);
    }
    // SAFETY: the lock taken above.
    if unsafe { GdipBitmapUnlockBits(bitmap, &mut data) } != GDIP_OK {
        return Err(Refused::Unreadable);
    }
    Ok((Picture { width: w, height: h, pixels }, orientation))
}

/// The orientation a camera wrote into the picture (EXIF 0x0112), 1 when there is none.
fn orientation(image: *mut GpImage) -> u16 {
    let mut size = 0u32;
    // SAFETY: a live image; a plain query.
    if unsafe { GdipGetPropertyItemSize(image, PropertyTagOrientation, &mut size) } != GDIP_OK
        || (size as usize) < size_of::<PropertyItem>() + 2
        || size > 4096
    {
        return 1;
    }
    // A buffer of u64s, so that the item at its start is aligned.
    let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
    let item = buffer.as_mut_ptr() as *mut PropertyItem;
    // SAFETY: `buffer` holds `size` bytes as GDI+ asked; its value points inside the buffer.
    unsafe {
        if GdipGetPropertyItem(image, PropertyTagOrientation, size, item) != GDIP_OK {
            return 1;
        }
        let item = &*item;
        // Type 3 is a 16-bit number.
        if item.r#type != 3 || item.length < 2 || item.value.is_null() {
            return 1;
        }
        std::ptr::read_unaligned(item.value as *const u16)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! The tests write their own pictures into the scratch folder: never a picture of anyone's.
    use super::*;
    use std::ptr::null;
    use windows_sys::Win32::Graphics::GdiPlus::{GdipCreateBitmapFromScan0, GdipGetImageEncoders, GdipGetImageEncodersSize, GdipSaveImageToFile, ImageCodecInfo};

    /// A test picture: a gradient with fine lines (every pixel says where it is).
    pub(crate) fn test_pixels(w: i32, h: i32) -> Vec<u32> {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| if x % 7 == 0 { 0xFF00_0000 } else { 0xFF00_0000 | ((x * 255 / w.max(1)) as u32) << 16 | ((y * 255 / h.max(1)) as u32) << 8 | 0x40 }))
            .collect()
    }

    /// Saves `pixels` as a file of the kind whose MIME type is `mime` ("image/png", ...) through
    /// GDI+'s own encoders.
    pub(crate) fn save(path: &Path, w: i32, h: i32, pixels: &[u32], mime: &str) {
        assert!(gdiplus());
        let mut bitmap: *mut GpBitmap = null_mut();
        let wide: Vec<u16> = path.as_os_str().encode_wide_nul();
        // SAFETY: test-only; `pixels` outlives the bitmap; every buffer sized as GDI+ says.
        unsafe {
            assert_eq!(GdipCreateBitmapFromScan0(w, h, w * 4, PIXEL_FORMAT_32BPP_ARGB, pixels.as_ptr().cast(), &mut bitmap), GDIP_OK);
            let (mut n, mut size) = (0u32, 0u32);
            GdipGetImageEncodersSize(&mut n, &mut size);
            let mut buf = vec![0u64; (size as usize).div_ceil(8)];
            let codecs = buf.as_mut_ptr() as *mut ImageCodecInfo;
            GdipGetImageEncoders(n, size, codecs);
            let clsid = (0..n as usize)
                .map(|i| &*codecs.add(i))
                .find(|c| {
                    let mut len = 0;
                    while *c.MimeType.add(len) != 0 {
                        len += 1;
                    }
                    String::from_utf16_lossy(std::slice::from_raw_parts(c.MimeType, len)) == mime
                })
                .map(|c| c.Clsid)
                .expect("an encoder");
            assert_eq!(GdipSaveImageToFile(bitmap as *mut GpImage, wide.as_ptr(), &clsid, null()), GDIP_OK);
            GdipDisposeImage(bitmap as *mut GpImage);
        }
    }

    trait Wide {
        fn encode_wide_nul(&self) -> Vec<u16>;
    }
    impl Wide for std::ffi::OsStr {
        fn encode_wide_nul(&self) -> Vec<u16> {
            use std::os::windows::ffi::OsStrExt;
            self.encode_wide().chain([0]).collect()
        }
    }

    /// A BMP written by hand (24 bits, bottom-up), so that the reader is tried on a file no
    /// encoder of Windows' made.
    pub(crate) fn bmp(w: i32, h: i32, pixels: &[u32]) -> Vec<u8> {
        let row = ((w * 3 + 3) / 4 * 4) as usize;
        let mut out = Vec::new();
        let size = 54 + row * h as usize;
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&(size as u32).to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&54u32.to_le_bytes());
        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&h.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&24u16.to_le_bytes());
        out.extend_from_slice(&[0; 24]);
        for y in (0..h).rev() {
            let start = out.len();
            for x in 0..w {
                let p = pixels[(y * w + x) as usize];
                out.extend_from_slice(&[p as u8, (p >> 8) as u8, (p >> 16) as u8]);
            }
            out.resize(start + row, 0);
        }
        out
    }

    /// A JPEG's bytes with an EXIF block saying `orientation` put in right after its start.
    pub(crate) fn with_orientation(jpeg: &[u8], orientation: u16) -> Vec<u8> {
        // TIFF header (little-endian), one IFD entry: tag 0x0112, type 3 (SHORT), count 1, value.
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II*\0");
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&orientation.to_le_bytes());
        tiff.extend_from_slice(&[0, 0]);
        tiff.extend_from_slice(&0u32.to_le_bytes());
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(&tiff);
        let mut out = jpeg[..2].to_vec();
        out.extend_from_slice(&[0xFF, 0xE1]);
        out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        out.extend_from_slice(&app1);
        out.extend_from_slice(&jpeg[2..]);
        out
    }

    pub(crate) fn scratch(name: &str) -> std::path::PathBuf {
        let dir = crate::log::data_dir().join("picture-tests");
        assert!(!dir.to_string_lossy().contains("AppData\\Local\\Bevel95\\"), "a scratch data folder");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn every_kind_of_file_is_read() {
        let (w, h) = (37, 23);
        let pixels = test_pixels(w, h);
        // BMP (by hand, and a DIB is the same thing under another ending).
        for name in ["hand.bmp", "hand.dib"] {
            let path = scratch(name);
            std::fs::write(&path, bmp(w, h, &pixels)).unwrap();
            let p = read_file(&path).unwrap();
            assert_eq!((p.width, p.height), (w, h));
            assert_eq!(p.pixels, pixels, "{name}: every pixel");
        }
        // PNG and GIF's first picture through Windows' encoders; PNG keeps every pixel.
        let path = scratch("made.png");
        save(&path, w, h, &pixels, "image/png");
        assert_eq!(read_file(&path).unwrap().pixels, pixels);
        let path = scratch("made.gif");
        save(&path, w, h, &pixels, "image/gif");
        let p = read_file(&path).unwrap();
        assert_eq!((p.width, p.height), (w, h));
        assert_eq!(p.get(0, 5), 0xFF00_0000, "a black line stays black in 256 colors");
        // JPEG: close, not exact (it is a JPEG).
        let path = scratch("made.jpg");
        save(&path, w, h, &pixels, "image/jpeg");
        let p = read_file(&path).unwrap();
        assert_eq!((p.width, p.height), (w, h));
        let worst = p.pixels.iter().zip(&pixels).map(|(a, b)| (0..3).map(|k| ((a >> (8 * k)) & 0xFF).abs_diff((b >> (8 * k)) & 0xFF)).max().unwrap()).max().unwrap();
        assert!(worst < 96, "a JPEG comes close ({worst})");
        assert!(p.pixels.iter().all(|&px| px >> 24 == 0xFF));
        // Windows 2000's own JPEGs from bevel-art.
        // Windows 2000's own JPEGs from bevel-art, each with its file's size (survey.md, section 5).
        let sizes = [
            ("Boiling Point", 163, 293),
            ("Chateau", 800, 600),
            ("Fall Memories", 210, 185),
            ("Fly Away", 281, 188),
            ("Gold Petals", 800, 600),
            ("Ocean Wave", 800, 600),
            ("Paradise", 800, 600),
            ("Purple Sponge", 74, 124),
            ("Snow Trees", 800, 600),
            ("Solar Eclipse", 800, 600),
            ("Water Color", 218, 162),
            ("Windows 2000", 800, 600),
        ];
        for (name, w, h) in sizes {
            let id = bevel_core::display::PictureId::Windows(bevel_core::theme::Era::Win2000, name);
            let p = read_bytes(id.bytes().unwrap()).unwrap();
            assert_eq!((p.width, p.height), (w, h), "{name}");
        }
    }

    #[test]
    fn a_photo_is_read_upright() {
        let (w, h) = (24, 16);
        let pixels = test_pixels(w, h);
        let path = scratch("orient.jpg");
        save(&path, w, h, &pixels, "image/jpeg");
        let jpeg = std::fs::read(&path).unwrap();
        let plain = read_bytes(&jpeg).unwrap();
        assert_eq!((plain.width, plain.height), (24, 16));
        for o in 1..=8u16 {
            let p = read_bytes(&with_orientation(&jpeg, o)).unwrap();
            let want = plain.clone().upright(o);
            assert_eq!((p.width, p.height), (want.width, want.height), "orientation {o}");
            assert_eq!(p.pixels, want.pixels, "orientation {o}");
        }
        // 6: turned a quarter right, so it stands 16 wide and 24 high.
        let p = read_bytes(&with_orientation(&jpeg, 6)).unwrap();
        assert_eq!((p.width, p.height), (16, 24));
    }

    #[test]
    fn what_is_no_picture_is_refused_plainly() {
        let path = scratch("words.jpg");
        std::fs::write(&path, b"These are words, not a picture.").unwrap();
        assert_eq!(read_file(&path), Err(Refused::Unreadable));
        assert_eq!(read_bytes(&[]), Err(Refused::Unreadable));
        assert_eq!(read_file(&scratch("there is no such file.png")), Err(Refused::Unreadable));
        assert_eq!(read_file(&crate::log::data_dir()), Err(Refused::Unreadable), "a folder");
        // Cut short: the first part of a picture file.
        let (w, h) = (64, 48);
        let pixels = test_pixels(w, h);
        for (mime, name) in [("image/png", "cut.png"), ("image/jpeg", "cut.jpg"), ("image/gif", "cut.gif")] {
            let path = scratch(name);
            save(&path, w, h, &pixels, mime);
            let whole = std::fs::read(&path).unwrap();
            for keep in [whole.len() / 2, 40, 10] {
                let r = read_bytes(&whole[..keep]);
                assert_eq!(r, Err(Refused::Unreadable), "{name} cut to {keep} of {} bytes", whole.len());
            }
        }
        let cut = bmp(w, h, &pixels);
        assert_eq!(read_bytes(&cut[..cut.len() / 2]), Err(Refused::Unreadable), "a BMP cut short");
        // Another kind GDI+ would read (TIFF) is not one of the kinds taken.
        let path = scratch("other.tif");
        save(&path, w, h, &pixels, "image/tiff");
        assert_eq!(read_file(&path), Err(Refused::Unreadable));
    }

    /// Bytes after a picture's end are no harm (a phone's short film after a photo, a tool's
    /// padding); a JPEG with a small picture of the camera's own inside its header is read whole;
    /// a file cut short is still refused (the review of 2026-11-21, 8).
    #[test]
    fn bytes_after_the_end_are_taken_and_a_cut_file_is_not() {
        let (w, h) = (64, 48);
        let pixels = test_pixels(w, h);
        // What a phone puts after a photo: something like a film, with marks of a JPEG's in it.
        let film: Vec<u8> = (0..4000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).chain([0xFF, 0xDA, 0x00, 0x08, 0xFF, 0xD8, 0x3B]).collect();
        for (mime, name) in [("image/png", "after.png"), ("image/jpeg", "after.jpg"), ("image/gif", "after.gif")] {
            let path = scratch(name);
            save(&path, w, h, &pixels, mime);
            let whole = std::fs::read(&path).unwrap();
            let plain = read_bytes(&whole).unwrap();
            for tail in [&b"\0\0\0\0"[..], &film] {
                let mut longer = whole.clone();
                longer.extend_from_slice(tail);
                assert_eq!(read_bytes(&longer).as_ref(), Ok(&plain), "{name} with {} bytes after its end", tail.len());
            }
            // Cut anywhere before its end: refused.
            for keep in [whole.len() - 1, whole.len() - 3, whole.len() * 3 / 4, whole.len() / 2] {
                assert_eq!(read_bytes(&whole[..keep]), Err(Refused::Unreadable), "{name} cut to {keep} of {}", whole.len());
            }
        }
        // A camera's photo: its header (APP1) holds a small JPEG of its own, start to end mark.
        let path = scratch("thumb.jpg");
        save(&path, w, h, &pixels, "image/jpeg");
        let photo = std::fs::read(&path).unwrap();
        save(&path, 16, 12, &test_pixels(16, 12), "image/jpeg");
        let small = std::fs::read(&path).unwrap();
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(&small);
        let mut camera = photo[..2].to_vec();
        camera.extend_from_slice(&[0xFF, 0xE1]);
        camera.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        camera.extend_from_slice(&app1);
        camera.extend_from_slice(&photo[2..]);
        let p = read_bytes(&camera).unwrap();
        assert_eq!((p.width, p.height), (w, h), "the photo, not its small picture");
        assert_eq!(p, read_bytes(&photo).unwrap());
        // Cut after the small picture's end, inside the photo's own scan: refused.
        let cut = 2 + 4 + app1.len() + (photo.len() - 2) * 3 / 4;
        assert_eq!(read_bytes(&camera[..cut]), Err(Refused::Unreadable));
        // A JPEG without its end mark (written so by some tools) is still refused.
        assert_eq!(read_bytes(&photo[..photo.len() - 2]), Err(Refused::Unreadable));
        // A BMP with bytes after its rows.
        let mut b = bmp(w, h, &pixels);
        b.extend_from_slice(&film);
        assert_eq!(read_bytes(&b).unwrap().pixels, pixels);
    }

    /// A BMP header whose sums run over (the review of 2026-11-21, 7): width and height near 2^31
    /// and 0xFFFF bits a pixel, an offset near 2^32: not whole, and no panic.
    #[test]
    fn a_bmp_header_that_runs_over_is_not_whole() {
        let mut b = bmp(2, 2, &[0xFF00_0000; 4]);
        b[18..22].copy_from_slice(&0x7FFF_FFFFi32.to_le_bytes());
        b[22..26].copy_from_slice(&i32::MIN.to_le_bytes());
        b[28..30].copy_from_slice(&0xFFFFu16.to_le_bytes());
        assert!(!whole(Kind::Bmp, &b));
        assert_eq!(read_bytes(&b), Err(Refused::Unreadable));
        // Sums that fit, but an offset near 2^32 with rows after it.
        let mut b = bmp(2, 2, &[0xFF00_0000; 4]);
        b[10..14].copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
        assert!(!whole(Kind::Bmp, &b));
        // Compressed, its size near 2^32 and the offset too.
        let mut b = bmp(2, 2, &[0xFF00_0000; 4]);
        b[10..14].copy_from_slice(&u32::MAX.to_le_bytes());
        b[30..34].copy_from_slice(&1u32.to_le_bytes());
        b[34..38].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(!whole(Kind::Bmp, &b));
    }

    #[test]
    fn a_picture_beyond_the_limit_is_refused() {
        // A BMP's header saying 20,000 x 2 (a side beyond 16,384): refused before decoding.
        let mut wide = bmp(2, 2, &[0xFF00_0000; 4]);
        wide[18..22].copy_from_slice(&20_000i32.to_le_bytes());
        wide.resize(54 + 60_000 * 2, 0);
        let n = wide.len() as u32;
        wide[2..6].copy_from_slice(&n.to_le_bytes());
        assert_eq!(read_bytes(&wide), Err(Refused::TooLarge));
        // 10,000 x 5,001 (50,010,000 pixels): beyond 50 million. A PNG of one color is small.
        let path = scratch("many.png");
        let one = vec![0xFF12_3456u32; 10_000 * 5_001];
        save(&path, 10_000, 5_001, &one, "image/png");
        drop(one);
        assert_eq!(read_file(&path), Err(Refused::TooLarge));
    }

    /// How close Windows 2000's JPEGs decoded here come to what Windows 2000 itself drew (the
    /// Reference PC's captures, read only): the desktop right of the icons and above the taskbar
    /// (x 100..799, y 0..570). Run on request:
    /// `cargo test -p bevel95 --bin bevel95 -- --ignored --nocapture how_close`.
    #[test]
    #[ignore = "needs the private reference captures; prints numbers"]
    fn how_close_2000s_jpegs_come() {
        use bevel_core::display::{Choice, Show};
        use bevel_core::theme::{Era, WIN2000};
        let refs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../private/reference/refs/vm");
        for (capture, name, show) in [
            ("win2000-truecolor-display-desk-boilingpoint-tile-activedesktop.png", "Boiling Point", Show::Tile),
            ("win2000-truecolor-display-desk-flyaway-stretch-activedesktop.png", "Fly Away", Show::Stretch),
        ] {
            let Ok(shot) = read_file(&refs.join(capture)) else {
                println!("{capture}: not found");
                continue;
            };
            let id = bevel_core::display::PictureId::Windows(Era::Win2000, name);
            let picture = read_bytes(id.bytes().unwrap()).unwrap();
            let choice = Choice { wallpaper: Some(name.into()), show, pattern: None };
            let ours = bevel_core::display::background_on(&WIN2000, &choice, Some(&picture), &[], 800, 600).unwrap();
            let (mut n, mut differ, mut worst, mut sum) = (0u64, 0u64, 0u32, 0u64);
            let mut within = [0u64; 4];
            for y in 0..571 {
                for x in 100..800 {
                    let (a, b) = (ours.get(x, y), shot.get(x, y));
                    let d = (0..3).map(|k| ((a >> (8 * k)) & 0xFF).abs_diff((b >> (8 * k)) & 0xFF)).max().unwrap();
                    n += 1;
                    if d > 0 {
                        differ += 1;
                    }
                    for (i, limit) in [2, 4, 8, 16].into_iter().enumerate() {
                        if d <= limit {
                            within[i] += 1;
                        }
                    }
                    worst = worst.max(d);
                    sum += u64::from(d);
                }
            }
            println!(
                "{name} ({show:?}): {differ} of {n} pixels differ; within 2: {}, 4: {}, 8: {}, 16: {}; worst {worst}, mean {:.2}",
                within[0], within[1], within[2], within[3], sum as f64 / n as f64
            );
            // Measured 2026-11-21: only the mouse pointer's 17 pixels differ in each.
            assert!(differ <= 17, "{name}: {differ} pixels differ");
        }
    }

    /// About 20 pictures read and let go: the process's handles and GDI objects come back. Run
    /// in a process of its own (other tests beside it open handles of their own).
    #[test]
    fn reading_leaves_nothing_behind() {
        let me = std::env::current_exe().unwrap();
        let out = bevel_winkit::quiet::command(me)
            .args(["--ignored", "--exact", "picture_file::tests::reading_leaves_nothing_behind_alone", "--test-threads=1"])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success() && text.contains("1 passed"), "{text}{}", String::from_utf8_lossy(&out.stderr));
    }

    #[test]
    #[ignore = "run alone by reading_leaves_nothing_behind"]
    fn reading_leaves_nothing_behind_alone() {
        let (w, h) = (300, 200);
        let path = scratch("leak.png");
        save(&path, w, h, &test_pixels(w, h), "image/png");
        let bytes = std::fs::read(&path).unwrap();
        let _ = read_bytes(&bytes);
        let handles = || {
            let mut n = 0u32;
            // SAFETY: a plain query about this process.
            unsafe { windows_sys::Win32::System::Threading::GetProcessHandleCount(windows_sys::Win32::System::Threading::GetCurrentProcess(), &mut n) };
            n
        };
        let gdi = || {
            // SAFETY: a plain query about this process.
            unsafe { windows_sys::Win32::System::Threading::GetGuiResources(windows_sys::Win32::System::Threading::GetCurrentProcess(), 0) }
        };
        let (h0, g0) = (handles(), gdi());
        for _ in 0..20 {
            assert!(read_bytes(&bytes).is_ok());
            assert!(read_file(&path).is_ok());
        }
        let (h1, g1) = (handles(), gdi());
        // A little room for Windows' own (a thread pool's), far less than one a reading.
        assert!(h1 <= h0 + 4, "handles {h0} -> {h1}");
        assert_eq!(g0, g1, "GDI objects");
        println!("handles {h0} -> {h1}, GDI objects {g0} -> {g1}");
    }
}

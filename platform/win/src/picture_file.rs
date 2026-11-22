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

/// Reads the file at `path` whole (at most `display::MAX_FILE_BYTES`) and decodes it.
pub fn read_file(path: &Path) -> Result<Picture, Refused> {
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
    read_bytes(&bytes)
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
/// rest, without saying so. A PNG ends with its IEND part, a GIF with its last byte 0x3B, a JPEG
/// has its end mark (FF D9) after the start of its last scan, a BMP holds all the rows its header
/// promises.
fn whole(kind: Kind, b: &[u8]) -> bool {
    let u32_at = |i: usize| b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    match kind {
        Kind::Png => b.len() >= 12 && &b[b.len() - 8..b.len() - 4] == b"IEND",
        Kind::Gif => b.last() == Some(&0x3B),
        Kind::Jpeg => {
            let last_scan = b.windows(2).rposition(|w| w == [0xFF, 0xDA]);
            last_scan.is_some_and(|s| b[s..].windows(2).any(|w| w == [0xFF, 0xD9]))
        }
        Kind::Bmp => {
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
                0 | 3 => (w * bits).div_ceil(32) * 4 * h,
                _ => u64::from(size),
            };
            u64::from(off) + need <= b.len() as u64
        }
    }
}

/// Decodes a picture file's bytes (its first picture), upright: a BMP or DIB, JPEG, PNG or GIF,
/// told by its first bytes; anything else (another kind GDI+ would read, a metafile) is refused.
pub fn read_bytes(bytes: &[u8]) -> Result<Picture, Refused> {
    if bytes.len() as u64 > display::MAX_FILE_BYTES {
        return Err(Refused::TooLarge);
    }
    let Some(kind) = kind(bytes) else { return Err(Refused::Unreadable) };
    if !whole(kind, bytes) || !gdiplus() {
        return Err(Refused::Unreadable);
    }
    // SAFETY: the stream copies the bytes; it is released below in every case.
    let stream = unsafe { SHCreateMemStream(bytes.as_ptr(), bytes.len() as u32) };
    if stream.is_null() {
        return Err(Refused::Unreadable);
    }
    let mut bitmap: *mut GpBitmap = null_mut();
    // SAFETY: a live stream; GDI+ keeps its own reference while the bitmap lives.
    let made = unsafe { GdipCreateBitmapFromStream(stream, &mut bitmap) } == GDIP_OK && !bitmap.is_null();
    let result = if made { decode(bitmap) } else { Err(Refused::Unreadable) };
    if !bitmap.is_null() {
        // SAFETY: the bitmap made above, disposed once.
        unsafe { GdipDisposeImage(bitmap as *mut GpImage) };
    }
    // SAFETY: our reference to the stream (IUnknown::Release, the third entry of its table).
    unsafe {
        let table = *(stream as *const *const usize);
        let release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32 = std::mem::transmute(*table.add(2));
        release(stream);
    }
    result
}

/// The bitmap's pixels as 0xAARRGGBB, its size checked first, upright.
fn decode(bitmap: *mut GpBitmap) -> Result<Picture, Refused> {
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
    Ok(Picture { width: w, height: h, pixels }.upright(orientation(image)))
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

    fn scratch(name: &str) -> std::path::PathBuf {
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
        use std::os::windows::process::CommandExt;
        let me = std::env::current_exe().unwrap();
        let out = std::process::Command::new(me)
            .args(["--ignored", "--exact", "picture_file::tests::reading_leaves_nothing_behind_alone", "--test-threads=1"])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
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

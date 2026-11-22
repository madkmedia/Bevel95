//! The classic desktop's wallpapers and patterns, and Display Properties' Background page that
//! chooses them (`form`, `pattern`), each look with its own Windows' pictures and rules.
//!
//! Measured on the Reference PCs (private/probes/display/survey.md; docs/theme-spec.md, "Display
//! Properties and wallpapers"), every rule exact against the PCs' own files:
//! - the desktop is the desktop color, or a pattern's 8x8 bits from the screen's top-left corner
//!   (a set bit black, a clear one the desktop color);
//! - a wallpaper is tiled from the top-left corner, centered on the whole screen (cut equally when
//!   larger, the pattern or the color around it when smaller) or stretched over the whole screen
//!   by Windows' own stepping (98 and 2000; 95 had no Stretch); with Tile or Stretch no pattern
//!   shows;
//! - icon names stand in a box of the desktop color (`desktop_icons` draws them so already).
//!
//! "The whole screen" is the classic desktop's own area in the look's 1x pixels, which Bevel95
//! enlarges like everything else.

pub mod form;
pub mod pattern;

use crate::art::{self, Look};
use crate::fb::{Framebuffer, Rect};
use crate::theme::{Depth, Era, Theme};

/// How a wallpaper is shown (95's Tile and Center buttons; 98's Display and 2000's Picture
/// Display lists: Center, Tile, Stretch).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Show {
    #[default]
    Center,
    Tile,
    Stretch,
}

impl Show {
    pub const ALL: [Show; 3] = [Show::Center, Show::Tile, Show::Stretch];

    /// Its word in background.txt.
    pub fn word(self) -> &'static str {
        match self {
            Show::Center => "center",
            Show::Tile => "tile",
            Show::Stretch => "stretch",
        }
    }

    pub fn from_word(word: &str) -> Option<Show> {
        Show::ALL.into_iter().find(|s| s.word().eq_ignore_ascii_case(word.trim()))
    }

    /// Its name in the look's list or on its button.
    pub fn label(self) -> &'static str {
        match self {
            Show::Center => "Center",
            Show::Tile => "Tile",
            Show::Stretch => "Stretch",
        }
    }

    /// The ways the look offers: Windows 95 had Tile and Center only.
    pub fn offered(era: Era) -> &'static [Show] {
        match era {
            Era::Win95 => &[Show::Tile, Show::Center],
            _ => &Show::ALL,
        }
    }
}

/// How a wallpaper of Windows' is kept in `bevel-art`.
#[derive(Clone, Copy, Debug)]
enum File {
    /// The BMPs, pixel for pixel in bevel-art's compact form: decoded here.
    Compact(&'static [u8]),
    /// Windows 2000's JPEGs, the files as they are: decoded by Windows on the bar's side (read on
    /// a thread of their own, like a person's own picture).
    Jpeg(&'static [u8]),
}

/// One of a look's wallpapers: its name in the list and its picture (`bevel-art`).
#[derive(Debug)]
pub struct Wallpaper {
    pub name: &'static str,
    file: File,
}

macro_rules! wallpaper {
    ($name:literal, $look:ident) => {
        Wallpaper { name: $name, file: File::Compact(const { art::bytes(concat!("wallpaper/", $name), Look::$look) }) }
    };
}

macro_rules! jpeg {
    ($name:literal) => {
        Wallpaper { name: $name, file: File::Jpeg(const { art::bytes(concat!("wallpaper/", $name), Look::Win2000) }) }
    };
}

/// Each look's wallpapers (the order they are listed in is `windows_order`'s, not this one).
/// Left out: Windows 2000's Sample (the PC user's own picture), 98's HTML page (it needs Active
/// Desktop) and its Desktop Themes pictures (Windows' own list did not have them either). 2000's
/// JPEGs are drawn without the old question about Active Desktop (PROVISIONAL, brief 73).
static WIN95_WALLPAPERS: [Wallpaper; 12] = [
    wallpaper!("Black Thatch", Win95),
    wallpaper!("Blue Rivets", Win95),
    wallpaper!("Bubbles", Win95),
    wallpaper!("Carved Stone", Win95),
    wallpaper!("Circles", Win95),
    wallpaper!("Houndstooth", Win95),
    wallpaper!("Pinstripe", Win95),
    wallpaper!("Setup", Win95),
    wallpaper!("Straw Mat", Win95),
    wallpaper!("Tiles", Win95),
    wallpaper!("Triangles", Win95),
    wallpaper!("Waves", Win95),
];
static WIN98_WALLPAPERS: [Wallpaper; 20] = [
    wallpaper!("1stboot", Win98),
    wallpaper!("Black Thatch", Win98),
    wallpaper!("Blue Rivets", Win98),
    wallpaper!("Bubbles", Win98),
    wallpaper!("Carved Stone", Win98),
    wallpaper!("Circles", Win98),
    wallpaper!("Clouds", Win98),
    wallpaper!("Forest", Win98),
    wallpaper!("Gold Weave", Win98),
    wallpaper!("Houndstooth", Win98),
    wallpaper!("Metal Links", Win98),
    wallpaper!("Pinstripe", Win98),
    wallpaper!("Red Blocks", Win98),
    wallpaper!("Sandstone", Win98),
    wallpaper!("Setup", Win98),
    wallpaper!("Stitches", Win98),
    wallpaper!("Straw Mat", Win98),
    wallpaper!("Tiles", Win98),
    wallpaper!("Triangles", Win98),
    wallpaper!("Waves", Win98),
];
static WIN2000_WALLPAPERS: [Wallpaper; 23] = [
    jpeg!("Boiling Point"),
    jpeg!("Chateau"),
    jpeg!("Fall Memories"),
    jpeg!("Fly Away"),
    jpeg!("Gold Petals"),
    jpeg!("Ocean Wave"),
    jpeg!("Paradise"),
    jpeg!("Purple Sponge"),
    jpeg!("Snow Trees"),
    jpeg!("Solar Eclipse"),
    jpeg!("Water Color"),
    jpeg!("Windows 2000"),
    wallpaper!("Blue Lace 16", Win2000),
    wallpaper!("Coffee Bean", Win2000),
    wallpaper!("FeatherTexture", Win2000),
    wallpaper!("Gone Fishing", Win2000),
    wallpaper!("Greenstone", Win2000),
    wallpaper!("Prairie Wind", Win2000),
    wallpaper!("Rhododendron", Win2000),
    wallpaper!("River Sumida", Win2000),
    wallpaper!("Santa Fe Stucco", Win2000),
    wallpaper!("Soap Bubbles", Win2000),
    wallpaper!("Zapotec", Win2000),
];

/// Windows' order of the wallpaper list: by name, letters compared without case (the PCs listed
/// "1stboot" first and "FeatherTexture" after "Fall Memories"). An entry added to a look's table
/// takes its place by this rule.
pub fn windows_order(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

/// A look's wallpapers in its list's order (without "(None)", which every list has first).
pub fn wallpapers(era: Era) -> Vec<&'static Wallpaper> {
    let table: &'static [Wallpaper] = match era {
        Era::Win95 => &WIN95_WALLPAPERS,
        Era::Win98 => &WIN98_WALLPAPERS,
        Era::Win2000 => &WIN2000_WALLPAPERS,
    };
    let mut list: Vec<&Wallpaper> = table.iter().collect();
    list.sort_by(|a, b| windows_order(a.name, b.name));
    list
}

/// The look's wallpaper of that name.
pub fn wallpaper(era: Era, name: &str) -> Option<&'static Wallpaper> {
    wallpapers(era).into_iter().find(|w| w.name == name)
}

/// A pattern: its name and its eight rows (top to bottom, bit 7 the left pixel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub name: &'static str,
    pub rows: [u8; 8],
}

impl Pattern {
    /// Whether the pattern's pixel at (x, y) of its 8x8 is set (black).
    pub fn set(&self, x: i32, y: i32) -> bool {
        (self.rows[y.rem_euclid(8) as usize] >> (7 - x.rem_euclid(8))) & 1 != 0
    }
}

const PATTERNS_9X: &str = art::text("display/patterns", Look::Win95);
const PATTERNS_2000: &str = art::text("display/patterns", Look::Win2000);

/// A look's patterns in its list's order (without "(None)"): 95 and 98 the same 19, 2000 its 13.
pub fn patterns(era: Era) -> Vec<Pattern> {
    let text = if era == Era::Win2000 { PATTERNS_2000 } else { PATTERNS_9X };
    text.lines()
        .filter_map(|line| {
            let (name, numbers) = line.split_once('\t')?;
            let mut rows = [0u8; 8];
            let mut n = numbers.split(' ').map(|v| v.parse::<u8>().ok());
            for r in &mut rows {
                *r = n.next()??;
            }
            Some(Pattern { name, rows })
        })
        .collect()
}

/// The look's pattern of that name.
pub fn pattern(era: Era, name: &str) -> Option<Pattern> {
    patterns(era).into_iter().find(|p| p.name == name)
}

/// What a look's Background page chose: a wallpaper and how it shows, a pattern. Kept by name
/// (remembered by what it is): a name the look does not have means none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choice {
    pub wallpaper: Option<String>,
    pub show: Show,
    pub pattern: Option<String>,
}

impl Choice {
    /// The plain desktop, as a person who never opened the window has it.
    pub fn plain() -> Choice {
        Choice::default()
    }

    /// Nothing drawn but the desktop color (no wallpaper, no pattern the look has).
    pub fn is_plain(&self, era: Era) -> bool {
        !self.has_wallpaper(era) && self.pattern_in(era).is_none()
    }

    /// The look's own wallpaper of Windows' chosen (none for a person's own picture).
    pub fn wallpaper_in(&self, era: Era) -> Option<&'static Wallpaper> {
        self.wallpaper.as_deref().and_then(|n| wallpaper(era, n))
    }

    /// The person's own picture chosen: its file's full path.
    pub fn own(&self) -> Option<&str> {
        self.wallpaper.as_deref().filter(|v| is_own(v))
    }

    /// The wallpaper as the log may name it: Windows' own by name, a person's own picture never by
    /// its file's name or path.
    pub fn wallpaper_said(&self) -> &str {
        match self.wallpaper.as_deref() {
            None => NONE,
            Some(v) if is_own(v) => "the person's own picture",
            Some(name) => name,
        }
    }

    /// A wallpaper is chosen: one of the look's own, or a person's own picture.
    pub fn has_wallpaper(&self, era: Era) -> bool {
        self.wallpaper_in(era).is_some() || self.own().is_some()
    }

    /// The picture that must be read (on the bar's side, on a thread of its own) before the
    /// wallpaper can be drawn: one of Windows 2000's JPEGs, or a person's own picture. None for
    /// the BMPs (decoded here at once) and for no wallpaper.
    pub fn to_read(&self, era: Era) -> Option<PictureId> {
        if let Some(path) = self.own() {
            return Some(PictureId::Own(path.to_string()));
        }
        let w = self.wallpaper_in(era)?;
        matches!(w.file, File::Jpeg(_)).then_some(PictureId::Windows(era, w.name))
    }

    pub fn pattern_in(&self, era: Era) -> Option<Pattern> {
        self.pattern.as_deref().and_then(|n| pattern(era, n))
    }

    /// The pattern that shows: none under a tiled or stretched wallpaper (98 and 2000 gray their
    /// Pattern... button then; 95 keeps its list, and the wallpaper covers it).
    pub fn pattern_shown(&self, era: Era) -> Option<Pattern> {
        match (self.has_wallpaper(era), self.show) {
            (true, Show::Tile | Show::Stretch) => None,
            _ => self.pattern_in(era),
        }
    }
}

/// A picture that is read from a file before it is drawn, by what it is: one of a look's JPEGs of
/// Windows' (by name), or a person's own picture (by its file's full path). A reading that comes
/// back for a picture that is no longer wanted is dropped by this, never by order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PictureId {
    Windows(Era, &'static str),
    Own(String),
}

impl PictureId {
    /// The file's bytes of one of Windows' JPEGs (`bevel-art`); None for a person's own picture.
    pub fn bytes(&self) -> Option<&'static [u8]> {
        match self {
            PictureId::Windows(era, name) => match wallpaper(*era, name)?.file {
                File::Jpeg(bytes) => Some(bytes),
                File::Compact(_) => None,
            },
            PictureId::Own(_) => None,
        }
    }
}

/// Whether a wallpaper value is a person's own picture: a file's full path (`X:\...` or a
/// network path `\\server\...`), never one of Windows' names (they hold no backslash or colon).
/// Text that cannot be a file's path (control characters, longer than Windows' paths) is not, nor
/// a device's or a pipe's name (`\\.\...`, `\\?\...`: reading one could wait for ever; the review
/// before 0.1.4, C4).
pub fn is_own(value: &str) -> bool {
    let mut c = value.chars();
    let drive = matches!((c.next(), c.next(), c.next()), (Some(d), Some(':'), Some('\\')) if d.is_ascii_alphabetic());
    let device = value.starts_with("\\\\.\\") || value.starts_with("\\\\?\\");
    let network = value.starts_with("\\\\") && value.len() > 2 && !device;
    (drive || network) && value.chars().count() <= 32_767 && !value.chars().any(char::is_control)
}

/// A person's own picture's name in the list: its file's name without the ending ("My Photo.2024
/// .jpg" is "My Photo.2024"), cut at characters, never at bytes.
pub fn own_name(path: &str) -> &str {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    match file.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => file,
    }
}

/// What the bar takes from a person's own picture, and from no more: a side of at most 16,384
/// pixels, at most 50 million pixels (a 50-megapixel camera's photo; 200 MB while decoded at four
/// bytes a pixel, held while it is the choice), a file of at most 256 MB. Beyond it the picture is
/// refused like one that cannot be read.
pub const MAX_SIDE: i32 = 16_384;
pub const MAX_PIXELS: i64 = 50_000_000;
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// Whether a picture of `w` x `h` pixels is within the limits.
pub fn size_taken(w: i64, h: i64) -> bool {
    (1..=i64::from(MAX_SIDE)).contains(&w) && (1..=i64::from(MAX_SIDE)).contains(&h) && w * h <= MAX_PIXELS
}

/// A picture as decoded: 0xFFRRGGBB, or 0 where see-through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u32>,
}

impl Picture {
    pub fn get(&self, x: i32, y: i32) -> u32 {
        self.pixels[(y * self.width + x) as usize]
    }

    /// The picture as it was taken, upright: `orientation` is the number a camera writes into a
    /// photo (EXIF 0x0112): 1 as it is, 2 mirrored, 3 turned half round, 4 mirrored upside down, 5
    /// mirrored and turned a quarter right, 6 turned a quarter right, 7 mirrored and turned a
    /// quarter left, 8 turned a quarter left (each says how the stored picture must be turned to
    /// stand upright). Any other number: as it is.
    pub fn upright(self, orientation: u16) -> Picture {
        if !(2..=8).contains(&orientation) {
            return self;
        }
        let (w, h) = (self.width, self.height);
        let turned = orientation >= 5;
        let (nw, nh) = if turned { (h, w) } else { (w, h) };
        let mut pixels = vec![0u32; self.pixels.len()];
        for y in 0..nh {
            for x in 0..nw {
                // The stored pixel shown at (x, y) of the upright picture.
                let (sx, sy) = match orientation {
                    2 => (w - 1 - x, y),
                    3 => (w - 1 - x, h - 1 - y),
                    4 => (x, h - 1 - y),
                    5 => (y, x),
                    6 => (y, h - 1 - x),
                    7 => (w - 1 - y, h - 1 - x),
                    _ => (w - 1 - y, x),
                };
                pixels[(y * nw + x) as usize] = self.get(sx, sy);
            }
        }
        Picture { width: nw, height: nh, pixels }
    }
}

/// Decodes bevel-art's compact form (art/src/display.rs): the size, the colors used, the pixels
/// packed at `bits` each, run-length coded or not.
pub fn decode(file: &[u8]) -> Picture {
    let u16_at = |i: usize| i32::from(u16::from_le_bytes([file[i], file[i + 1]]));
    let (w, h, n) = (u16_at(0), u16_at(2), u16_at(4) as usize);
    let (bits, rle, see) = (u32::from(file[6]), file[7] != 0, usize::from(file[8]));
    let palette: Vec<u32> = (0..n)
        .map(|k| {
            let p = 9 + 3 * k;
            if k == see { 0 } else { 0xFF00_0000 | u32::from(file[p]) << 16 | u32::from(file[p + 1]) << 8 | u32::from(file[p + 2]) }
        })
        .collect();
    let data = &file[9 + 3 * n..];
    let raw: std::borrow::Cow<[u8]> = if rle {
        let mut out = Vec::new();
        let mut i = 0;
        while i < data.len() {
            let tag = usize::from(data[i]);
            if tag < 128 {
                out.extend_from_slice(&data[i + 1..i + 2 + tag]);
                i += 2 + tag;
            } else {
                out.extend(std::iter::repeat_n(data[i + 1], tag - 127));
                i += 2;
            }
        }
        out.into()
    } else {
        data.into()
    };
    let row_bytes = (w as usize * bits as usize).div_ceil(8);
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for y in 0..h as usize {
        let row = &raw[y * row_bytes..(y + 1) * row_bytes];
        for x in 0..w as usize {
            let mut v = 0usize;
            for k in 0..bits as usize {
                let bit = x * bits as usize + k;
                v = (v << 1) | usize::from((row[bit / 8] >> (7 - bit % 8)) & 1);
            }
            pixels.push(palette[v]);
        }
    }
    Picture { width: w, height: h, pixels }
}

impl Wallpaper {
    /// Its picture, decoded here: a BMP's. None for a JPEG, which the bar's side reads
    /// (`Choice::to_read`).
    pub fn picture(&self) -> Option<Picture> {
        match self.file {
            File::Compact(bytes) => Some(decode(bytes)),
            File::Jpeg(_) => None,
        }
    }

    /// A JPEG of Windows' (its small picture in the list is a picture file's, not a bitmap's).
    pub fn is_jpeg(&self) -> bool {
        matches!(self.file, File::Jpeg(_))
    }
}

/// Which source pixel each of `dst` pixels shows when `src` pixels are stretched (or squeezed) to
/// `dst`, by `era`'s own stepping. Stretched (the desktop's Stretch, measured on 98's Clouds
/// 640x480 and 2000's Zapotec 96x96 stretched to 800x600, every pixel equal): source pixel s covers
/// the pixels from round(s * dst / src) (halves up) to where the next one starts, the columns
/// counted from the left and the rows from the bottom (`from_end`; a BMP lies bottom-up); a plain
/// nearest rule differs by one source pixel in about 15% of them. Squeezed (the little monitor,
/// every centered close-up of the Reference PCs equal): 95 and 98 show the last source pixel of
/// each span, its end rounded with halves and a little less going up (98's Forest, Clouds, Metal
/// Links, 1stboot, 95's Bubbles); 2000 the source pixel under each pixel's centre (its Blue Lace
/// 16, Zapotec, Prairie Wind).
pub fn steps(era: Era, src: i32, dst: i32, from_end: bool) -> Vec<i32> {
    let (src, dst) = (i64::from(src.max(1)), i64::from(dst.max(0)));
    let mut map = vec![0i32; dst as usize];
    if dst < src {
        for (d, m) in map.iter_mut().enumerate() {
            let d = d as i64;
            let s = if era == Era::Win2000 { (2 * d + 1) * src / (2 * dst) } else { (2 * (d + 1) * src + dst + 1) / (2 * dst) - 1 };
            *m = s.clamp(0, src - 1) as i32;
        }
        return map;
    }
    for s in 0..src {
        let (a, b) = ((2 * s * dst + src) / (2 * src), (2 * (s + 1) * dst + src) / (2 * src));
        for d in a..b.min(dst) {
            map[d as usize] = s as i32;
        }
    }
    if from_end {
        let n = map.len();
        let mut turned = vec![0; n];
        for d in 0..n {
            turned[d] = (src - 1) as i32 - map[n - 1 - d];
        }
        return turned;
    }
    map
}

/// `picture` stretched (or squeezed) to `w` x `h` by `era`'s stepping.
pub fn stretched(era: Era, picture: &Picture, w: i32, h: i32) -> Picture {
    // 2000 counts the columns from the right too (its monitor's Zapotec stretched to 152x112; on the
    // desktop both ways give the same 800 columns).
    let (mx, my) = (steps(era, picture.width, w, era == Era::Win2000), steps(era, picture.height, h, true));
    let mut pixels = Vec::with_capacity((w.max(0) * h.max(0)) as usize);
    for &sy in &my {
        for &sx in &mx {
            pixels.push(picture.get(sx, sy));
        }
    }
    Picture { width: w, height: h, pixels }
}
/// Where a picture `w` x `h` is centered in an area `area_w` x `area_h`: half the difference, cut
/// toward zero as Windows' C did (a picture larger than the area is cut equally on all sides).
pub fn centered(area_w: i32, area_h: i32, w: i32, h: i32) -> (i32, i32) {
    ((area_w - w) / 2, (area_h - h) / 2)
}

/// Black: a pattern's set bits (Windows' window text color on its background color; black on the
/// desktop color with the standard schemes, measured).
const PATTERN_INK: u32 = 0xFF00_0000;

/// A pixel of a picture as the theme's screen shows it: as it is in True Color; at 16 and 256
/// colors by the look's rule for a picture without its own drawing (`colors16::convert`; Windows
/// 2000 keeps its face and desktop at 256, `colors256::kept`). Nothing was measured for wallpapers
/// at these depths.
fn on_screen(theme: &Theme, p: u32) -> u32 {
    if theme.depth == Depth::TrueColor || p >> 24 == 0 || crate::colors256::kept(theme, p) {
        p
    } else {
        crate::colors16::convert(theme.era, p)
    }
}

/// Fills `fb` (an area whose top-left is `origin` of the pattern's grid) with the desktop color or
/// the pattern.
fn fill_ground(fb: &mut Framebuffer, theme: &Theme, pattern: Option<Pattern>, origin: (i32, i32)) {
    let desktop = theme.colors.desktop;
    match pattern {
        None => fb.pixels.fill(desktop),
        Some(p) => {
            for y in 0..fb.height {
                for x in 0..fb.width {
                    let c = if p.set(x + origin.0, y + origin.1) { PATTERN_INK } else { desktop };
                    fb.pixels[(y * fb.width + x) as usize] = c;
                }
            }
        }
    }
}

/// Lays `picture` into `fb` with its top-left at (x, y) (clipped), opaque pixels only.
fn lay(fb: &mut Framebuffer, theme: &Theme, picture: &Picture, x: i32, y: i32) {
    lay_in(fb, theme, picture, x, y, Rect::new(0, 0, fb.width, fb.height));
}

/// `lay`, cut to `clip` (a screen of several) as well as to `fb`.
fn lay_in(fb: &mut Framebuffer, theme: &Theme, picture: &Picture, x: i32, y: i32, clip: Rect) {
    let (x0, y0) = (x.max(0).max(clip.x), y.max(0).max(clip.y));
    let (x1, y1) = ((x + picture.width).min(fb.width).min(clip.right()), (y + picture.height).min(fb.height).min(clip.bottom()));
    for fy in y0..y1 {
        for fx in x0..x1 {
            let p = picture.get(fx - x, fy - y);
            let a = p >> 24;
            let at = (fy * fb.width + fx) as usize;
            if a == 0xFF {
                fb.pixels[at] = on_screen(theme, p);
            } else if a != 0 {
                // A person's own picture may be partly see-through (a PNG's or a GIF's edge): mixed
                // with what lies under it. Windows' own pictures are opaque or see-through.
                fb.pixels[at] = on_screen(theme, over(p, fb.pixels[at]));
            }
        }
    }
}

/// `p` (with its see-through value) over the opaque `under`.
fn over(p: u32, under: u32) -> u32 {
    let a = p >> 24;
    let mix = |shift: u32| (((p >> shift) & 0xFF) * a + ((under >> shift) & 0xFF) * (255 - a) + 127) / 255;
    0xFF00_0000 | mix(16) << 16 | mix(8) << 8 | mix(0)
}

/// Draws `choice` over an area `w` x `h` that is one screen: the desktop color or the pattern,
/// then the wallpaper tiled from the corner, centered or stretched (a BMP of the look's own).
pub fn draw(fb: &mut Framebuffer, theme: &Theme, choice: &Choice) {
    let whole = Rect::new(0, 0, fb.width, fb.height);
    draw_on(fb, theme, choice, None, &[whole]);
}

/// The wallpaper's picture to draw: a BMP of the look's decoded here; a JPEG or a person's own
/// picture as read (`read`), or none while it has not been read (or cannot be).
fn picture_of<'a>(choice: &Choice, era: Era, read: Option<&'a Picture>) -> Option<std::borrow::Cow<'a, Picture>> {
    if let Some(p) = choice.wallpaper_in(era).and_then(Wallpaper::picture) {
        return Some(std::borrow::Cow::Owned(p));
    }
    choice.to_read(era).and(read).map(std::borrow::Cow::Borrowed)
}

/// Draws `choice` over the classic desktop's whole area `fb`, which holds the `screens` (their
/// rectangles in `fb`'s pixels, the main screen first). As the old Windows did it on two screens
/// (PROVISIONAL: the Reference PCs had one screen, nothing was measured): the pattern and the
/// tiles start at the main screen's top-left corner and run over every screen; Center and Stretch
/// are done for each screen by itself, within its own screen. With one screen covering the whole
/// area it is the one-screen drawing the Reference PCs showed. No screens: the whole area is one.
/// `read`: the picture of a JPEG or of a person's own picture once it has been read; until then
/// (or when it cannot be) only the desktop color or the pattern shows (`picture_of`).
pub fn draw_on(fb: &mut Framebuffer, theme: &Theme, choice: &Choice, read: Option<&Picture>, screens: &[Rect]) {
    let whole = [Rect::new(0, 0, fb.width, fb.height)];
    let screens = if screens.is_empty() { &whole[..] } else { screens };
    let main = screens[0];
    let era = theme.era;
    fill_ground(fb, theme, choice.pattern_shown(era), (-main.x, -main.y));
    let Some(picture) = picture_of(choice, era, read) else { return };
    let picture: &Picture = &picture;
    if picture.width <= 0 || picture.height <= 0 {
        return;
    }
    // 95 had no Stretch: such a choice is no choice (`parse` never gives one); centered.
    let show = if era == Era::Win95 && choice.show == Show::Stretch { Show::Center } else { choice.show };
    match show {
        Show::Tile => {
            // From the main screen's corner over the whole area, the tiles left of and above it too.
            let (pw, ph) = (picture.width, picture.height);
            let first = |main: i32, step: i32| main - (main.max(0) + step - 1) / step * step;
            let (x0, y0) = (first(main.x, pw), first(main.y, ph));
            for y in (y0..fb.height).step_by(ph as usize) {
                for x in (x0..fb.width).step_by(pw as usize) {
                    lay(fb, theme, picture, x, y);
                }
            }
        }
        Show::Center => {
            for s in screens {
                let (x, y) = centered(s.w, s.h, picture.width, picture.height);
                lay_in(fb, theme, picture, s.x + x, s.y + y, *s);
            }
        }
        Show::Stretch => {
            for s in screens {
                lay_in(fb, theme, &stretched(era, picture, s.w, s.h), s.x, s.y, *s);
            }
        }
    }
}

/// The classic desktop's picture for `choice` over its whole area (`w` x `h`, 1x pixels), which
/// is one screen, or `None` for the plain desktop.
pub fn background(theme: &Theme, choice: &Choice, w: i32, h: i32) -> Option<Framebuffer> {
    background_on(theme, choice, None, &[Rect::new(0, 0, w.max(0), h.max(0))], w, h)
}

/// The classic desktop's picture for `choice` over its whole area (`w` x `h`, 1x pixels) holding
/// `screens` (`draw_on`; `read`: a JPEG's or a person's own picture as read), or `None` for the
/// plain desktop, which is drawn as before without one (nothing of the plain desktop changes: not
/// its pixels, its memory or its time). While a picture to read is not there and no pattern shows,
/// the desktop is plain too.
pub fn background_on(theme: &Theme, choice: &Choice, read: Option<&Picture>, screens: &[Rect], w: i32, h: i32) -> Option<Framebuffer> {
    let era = theme.era;
    let missing = choice.to_read(era).is_some() && read.is_none();
    if choice.is_plain(era) || (missing && choice.pattern_shown(era).is_none()) {
        return None;
    }
    let mut fb = Framebuffer::new(w.max(0), h.max(0), theme.colors.desktop);
    draw_on(&mut fb, theme, choice, read, screens);
    Some(fb)
}

/// The little monitor on the Background page: one 184x170 picture in all three looks (its gray
/// C0C0C0 even in 2000), see-through around it.
const MONITOR: &[u8] = art::bytes("display/monitor", Look::Win95);
/// Its screen inside the picture.
pub const MONITOR_SCREEN: Rect = Rect::new(16, 17, 152, 112);
pub const MONITOR_SIZE: (i32, i32) = (184, 170);

/// The whole screen the monitor stands for: the Reference PCs' 800x600, by which each Windows
/// scaled the wallpaper into the monitor's screen (152/800 across, 112/600 down).
const SCREEN: (i32, i32) = (800, 600);

/// A picture's size in the monitor: scaled as the screen is, each side rounded (halves up).
pub fn in_monitor(w: i32, h: i32) -> (i32, i32) {
    let side = |v: i32, small: i32, screen: i32| ((2 * v * small + screen) / (2 * screen)).max(1);
    (side(w, MONITOR_SCREEN.w, SCREEN.0), side(h, MONITOR_SCREEN.h, SCREEN.1))
}

/// Paints the monitor with its top-left at (x, y), its screen showing `choice` as each Windows
/// drew it there: the desktop color; the pattern 1:1, lined up with the picture's top-left; the
/// wallpaper scaled as the screen is (`in_monitor`), centered, tiled or stretched over the small
/// screen. A JPEG and a person's own picture (`read`, once read) by the same rule as the BMPs.
pub fn paint_monitor(fb: &mut Framebuffer, theme: &Theme, x: i32, y: i32, choice: &Choice, read: Option<&Picture>) {
    let era = theme.era;
    let layer = picture_of(choice, era, read).map(|p| monitor_layer(era, &p, choice.show));
    paint_monitor_layer(fb, theme, x, y, choice, layer.as_ref());
}

/// `paint_monitor` with the wallpaper's small copy made already (`monitor_layer`): Display
/// Properties makes it once per picture and way of showing, not at every paint.
pub fn paint_monitor_layer(fb: &mut Framebuffer, theme: &Theme, x: i32, y: i32, choice: &Choice, layer: Option<&Picture>) {
    let monitor = decode(MONITOR);
    lay(fb, theme, &monitor, x, y);
    let s = MONITOR_SCREEN;
    let mut screen = Framebuffer::new(s.w, s.h, theme.colors.desktop);
    fill_ground(&mut screen, theme, choice.pattern_shown(theme.era), (s.x, s.y));
    if let Some(layer) = layer {
        lay(&mut screen, theme, layer, 0, 0);
    }
    fb.blit(&screen, x + s.x, y + s.y);
}

/// What the monitor's small screen shows of the wallpaper `picture` shown `show` in `era`: its
/// 152 x 112 pixels, see-through (0) where the wallpaper does not reach (the ground shows there).
/// The picture scaled as the screen is (`in_monitor`) and centered or tiled, or stretched over the
/// small screen (98, 2000; 95's Stretch is no choice: centered). Only the pixels that show are
/// made, so a large photo gives no large copy.
pub fn monitor_layer(era: Era, picture: &Picture, show: Show) -> Picture {
    let s = MONITOR_SCREEN;
    let mut pixels = vec![0u32; (s.w * s.h) as usize];
    if picture.width > 0 && picture.height > 0 {
        // 2000 counts the columns from the right too (`stretched`).
        let from_right = era == Era::Win2000;
        let (w, h) = in_monitor(picture.width, picture.height);
        let mut put = |x: i32, y: i32, p: u32| pixels[(y * s.w + x) as usize] = p;
        match show {
            Show::Stretch if era != Era::Win95 => {
                let (mx, my) = (steps(era, picture.width, s.w, from_right), steps(era, picture.height, s.h, true));
                for y in 0..s.h {
                    for x in 0..s.w {
                        put(x, y, picture.get(mx[x as usize], my[y as usize]));
                    }
                }
            }
            Show::Tile => {
                let (mx, my) = (steps(era, picture.width, w, from_right), steps(era, picture.height, h, true));
                for y in 0..s.h {
                    for x in 0..s.w {
                        put(x, y, picture.get(mx[(x % w) as usize], my[(y % h) as usize]));
                    }
                }
            }
            _ => {
                let (mx, my) = (steps(era, picture.width, w, from_right), steps(era, picture.height, h, true));
                let (cx, cy) = centered(s.w, s.h, w, h);
                for y in cy.max(0)..(cy + h).min(s.h) {
                    for x in cx.max(0)..(cx + w).min(s.w) {
                        put(x, y, picture.get(mx[(x - cx) as usize], my[(y - cy) as usize]));
                    }
                }
            }
        }
    }
    Picture { width: s.w, height: s.h, pixels }
}

/// The name of background.txt, where each look's choice is kept (the data folder).
pub const FILE: &str = "background.txt";

/// The keys of a look's three lines in background.txt.
fn keys(era: Era) -> [String; 3] {
    let e = era.setting();
    [format!("{e} wallpaper"), format!("{e} display"), format!("{e} pattern")]
}

/// The words for none in background.txt: Windows' own list entry.
const NONE: &str = "(None)";

/// Each look's choice in background.txt's text. A look whose lines name a wallpaper or pattern it
/// does not have, or a way it does not offer (95 had no Stretch), or say nothing, has the plain
/// desktop; the other looks keep theirs. The file itself is never changed by reading it.
pub fn parse(text: &str) -> [Choice; 3] {
    Era::ALL.map(|era| parse_one(text, era).unwrap_or_default())
}

/// `text` without the plain spaces and tabs at its ends (nothing else: `str::trim` would also take
/// other alphabets' spaces, which a file's name may end in).
fn plain_trim(text: &str) -> &str {
    text.trim_matches([' ', '\t'])
}

fn parse_one(text: &str, era: Era) -> Option<Choice> {
    let [wk, dk, pk] = keys(era);
    let mut choice = Choice::default();
    for line in text.lines() {
        // Only plain spaces and tabs are taken off the ends: a file's name may end in another
        // alphabet's space (U+00A0, U+3000), which is part of its path.
        let line = plain_trim(line);
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, value) = (plain_trim(key), plain_trim(value));
        if key.eq_ignore_ascii_case(&wk) {
            // "(None)", a person's own picture by its file's full path (kept whole, whether the
            // file is there now or not: it is read again at each start), or a name of the look's.
            choice.wallpaper = if value == NONE {
                None
            } else if is_own(value) {
                Some(value.to_string())
            } else {
                Some(wallpaper(era, value)?.name.to_string())
            };
        } else if key.eq_ignore_ascii_case(&dk) {
            let show = Show::from_word(value)?;
            if !Show::offered(era).contains(&show) {
                return None;
            }
            choice.show = show;
        } else if key.eq_ignore_ascii_case(&pk) {
            choice.pattern = if value == NONE { None } else { Some(pattern(era, value)?.name.to_string()) };
        }
    }
    Some(choice)
}

/// The first lines of a new background.txt.
const HEADER: &str = "# Bevel95's wallpaper and pattern for each look (desktop menu > Properties, Background page).\n# Each look keeps its own: (None), a name from that look's lists or a picture file's full path; display = center, tile or stretch.\n# A note needs a line of its own, starting with #: after a value it would be taken as part of it.\n";

/// `text` with `era`'s three lines set to `choice` (added if missing): the first line of each key
/// takes the value and later lines of the same key go, so that exactly one line per key is left
/// (reading takes a key's last line: a second one would win at the next start). Every other line
/// is kept as it was.
pub fn with_choice(text: &str, era: Era, choice: &Choice) -> String {
    let [wk, dk, pk] = keys(era);
    let values = [
        choice.wallpaper.clone().unwrap_or_else(|| NONE.to_string()),
        choice.show.word().to_string(),
        choice.pattern.clone().unwrap_or_else(|| NONE.to_string()),
    ];
    let mut done = [false; 3];
    let base = if text.trim().is_empty() { HEADER } else { text };
    let mut lines: Vec<String> = base
        .lines()
        .filter_map(|line| {
            let key = line.split_once('=').map(|(k, _)| k.trim().to_string()).unwrap_or_default();
            for (i, k) in [&wk, &dk, &pk].into_iter().enumerate() {
                if !line.trim_start().starts_with('#') && key.eq_ignore_ascii_case(k) {
                    if done[i] {
                        return None;
                    }
                    done[i] = true;
                    return Some(format!("{k} = {}", values[i]));
                }
            }
            Some(line.to_string())
        })
        .collect();
    for (i, k) in [&wk, &dk, &pk].into_iter().enumerate() {
        if !done[i] {
            lines.push(format!("{k} = {}", values[i]));
        }
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests;

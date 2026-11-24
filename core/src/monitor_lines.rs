//! Monitor Lines: the faint dark lines of an old monitor over the whole screen (brief 102,
//! PROVISIONAL wording). One see-through window over the bar's screen darkens every n-th screen
//! row at size n, counted from the screen's top (the last row of each old pixel row of what starts
//! at a multiple of n from the top), black with the strength as its alpha; every other row fully
//! see-through. Nothing else: no color change, no
//! mask. This module makes the picture and says when there is one; the window is the platform's
//! (`platform/win/src/monitor_lines.rs`).

use crate::fb::Framebuffer;
use crate::scale::Scale;

/// How dark the lines are (the setting `monitor_lines`; the Bevel95 menu's Monitor Lines).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Strength {
    #[default]
    Off,
    Light,
    /// The owner's choice in the try-out (2026-11-24): 23 percent.
    Medium,
    Strong,
}

impl Strength {
    /// In the menu's order.
    pub const ALL: [Strength; 4] = [Strength::Off, Strength::Light, Strength::Medium, Strength::Strong];

    /// What settings.txt says.
    pub fn setting(self) -> &'static str {
        match self {
            Strength::Off => "off",
            Strength::Light => "light",
            Strength::Medium => "medium",
            Strength::Strong => "strong",
        }
    }

    /// The strength `value` names (lowercase, as `config::parse` gives it).
    pub fn from_setting(value: &str) -> Option<Strength> {
        Strength::ALL.into_iter().find(|s| s.setting() == value)
    }

    /// The menu's word for it.
    pub fn title(self) -> &'static str {
        match self {
            Strength::Off => "Off",
            Strength::Light => "Light",
            Strength::Medium => "Medium",
            Strength::Strong => "Strong",
        }
    }

    /// The darkening of a dark row, as a fraction: Light 0.15, Medium 0.23, Strong 0.35.
    pub fn fraction(self) -> f64 {
        match self {
            Strength::Off => 0.0,
            Strength::Light => 0.15,
            Strength::Medium => 0.23,
            Strength::Strong => 0.35,
        }
    }

    /// The same as a pixel's alpha (0 to 255, rounded as the try-out rounded it): 0, 38, 59, 89.
    pub fn alpha(self) -> u8 {
        (self.fraction() * 255.0).round() as u8
    }

    /// The index of `self` in `ALL` (the menu's entry, the bar's message).
    pub fn index(self) -> usize {
        Strength::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }
}

/// Every how many screen rows a line goes at `scale`: the size itself, a whole number from 2 up.
/// At size 1 there is no row between the old pixels' rows to put it on, and between whole numbers
/// the old rows are of uneven heights: no lines (`None`).
pub fn period(scale: Scale) -> Option<u32> {
    match scale.whole_number() {
        Some(n) if n >= 2 => Some(n as u32),
        _ => None,
    }
}

/// Whether the lines can show at `scale` (the menu's entries are grey where not).
pub fn possible(scale: Scale) -> bool {
    period(scale).is_some()
}

/// Whether screen row `y` (counted from the screen's top, from 0) is dark at period `n`: the last
/// screen row of each old pixel row.
pub fn dark_row(y: u32, n: u32) -> bool {
    n >= 2 && y % n == n - 1
}

/// A dark row's pixel, premultiplied BGRA as `UpdateLayeredWindow` takes it: black, so its color
/// is 0 whatever the alpha (premultiplied: color times alpha), and the strength as its alpha.
pub fn dark_pixel(strength: Strength) -> u32 {
    u32::from(strength.alpha()) << 24
}

/// One row of the picture, `width` pixels: a dark row's or a see-through row's (all 0).
pub fn row(width: usize, y: u32, n: u32, strength: Strength) -> Vec<u32> {
    let pixel = if dark_row(y, n) { dark_pixel(strength) } else { 0 };
    vec![pixel; width]
}

/// Fills `pixels` (rows of `width`, top to bottom, the screen's top row first) with the picture
/// for period `n` and `strength`: the window's whole picture, made once.
pub fn fill(pixels: &mut [u32], width: usize, n: u32, strength: Strength) {
    if width == 0 {
        return;
    }
    let dark = dark_pixel(strength);
    for (y, line) in pixels.chunks_mut(width).enumerate() {
        let pixel = if dark_row(y as u32, n) { dark } else { 0 };
        line.fill(pixel);
    }
}

/// The picture for a screen of `width` x `height` at period `n` (`fill`).
pub fn picture(width: i32, height: i32, n: u32, strength: Strength) -> Framebuffer {
    let mut fb = Framebuffer::new(width.max(0), height.max(0), 0);
    fill(&mut fb.pixels, width.max(0) as usize, n, strength);
    fb
}

/// What the screen shows through the lines: `screen` with the picture laid over it, as Windows
/// lays a premultiplied layered window over what is under it (out = P + S x (1 - A); P is black
/// here). For the owner's sheet and the tests; the window itself is drawn by Windows.
pub fn lay_over(screen: &mut Framebuffer, n: u32, strength: Strength) {
    let width = screen.width.max(0) as usize;
    if width == 0 {
        return;
    }
    let keep = 255 - u32::from(strength.alpha());
    for (y, line) in screen.pixels.chunks_mut(width).enumerate() {
        if !dark_row(y as u32, n) {
            continue;
        }
        for p in line {
            let channel = |shift: u32| ((((*p >> shift) & 0xFF) * keep + 127) / 255) << shift;
            *p = (*p & 0xFF00_0000) | channel(16) | channel(8) | channel(0);
        }
    }
}

/// The picture's size in bytes (32 bits a pixel), for the memory the window takes.
pub fn bytes(width: i32, height: i32) -> u64 {
    width.max(0) as u64 * height.max(0) as u64 * 4
}

/// A window of another program that comes back above the lines this soon after they were put
/// back over it is insisting (another program keeping itself on top, as every always-on-top
/// window does); one the person brings forward now and then is not.
pub const QUICK_RETURN_MS: u64 = 2_000;
/// After this many quick returns in a row the lines stop going over that window: no fight
/// without end (brief 102).
pub const FIGHTS_BEFORE_GIVING_UP: u32 = 5;
/// A window given up on that has not been seen above the lines for this long is looked at afresh
/// (brief 104: three minutes); a remembered count that old is forgotten too.
pub const GIVEN_UP_FOR_MS: u64 = 180_000;

/// A window found above the lines, front first (`place`, `Fights::tick`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Above {
    /// A window of Bevel95's own copy (the bar's, a companion program's): always goes under the
    /// lines, never counted.
    Own,
    /// Another program's window, by its handle, and whether it was given up on (it insists on
    /// being on top: it stays above the lines).
    Other { id: u64, given_up: bool },
}

/// Where the lines' window goes among the windows (`place`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Where it is: nothing above it to cover.
    Stay,
    /// On top of every window.
    Top,
    /// Just under this window (one given up on) and above every window under it.
    Under(u64),
}

/// Where the lines go, given the windows above them (front first): just under the lowest window
/// given up on, so that every window given up on stays above them and everything under that one
/// is covered; on top of everything when none is given up on; nowhere new when nothing needs
/// covering. Also the other programs' windows this covers (to be counted: `Fights::tick`).
/// Windows in front of the lowest given-up one stay above the lines whatever they are: one
/// window cannot go between them without moving another program's window, which Bevel95 never
/// does.
pub fn place(above: &[Above]) -> (Place, Vec<u64>) {
    let lowest = above.iter().rposition(|a| matches!(a, Above::Other { given_up: true, .. }));
    let (place, behind) = match lowest {
        Some(i) => {
            let Above::Other { id, .. } = above[i] else { unreachable!() };
            (Place::Under(id), &above[i + 1..])
        }
        None => (Place::Top, above),
    };
    if behind.is_empty() {
        return (Place::Stay, Vec::new());
    }
    let covered = behind
        .iter()
        .filter_map(|a| match a {
            Above::Other { id, given_up: false } => Some(*id),
            _ => None,
        })
        .collect();
    (place, covered)
}

/// What the bar's tick does about the windows found above the lines (`Fights::tick`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub place: Place,
    /// A window was given up on just now: how many have been so far in this run (for the log).
    pub gave_up: Option<u32>,
}

/// The other programs' windows that came above the lines, by their handles (as numbers): how often
/// each came back quickly, and those given up on. Bevel95's own windows are never counted.
#[derive(Clone, Debug, Default)]
pub struct Fights {
    windows: std::collections::HashMap<u64, Fight>,
    given_up: u32,
}

#[derive(Clone, Copy, Debug)]
struct Fight {
    times: u32,
    /// When it was last covered (counting) or, given up on, last seen above the lines.
    last_ms: u64,
    given_up: bool,
}

impl Fights {
    /// The windows above the lines as `place` takes them: each other program's window marked
    /// given up on or not. `above`: front first, `None` for Bevel95's own, else the handle.
    fn marked(&self, above: &[Option<u64>]) -> Vec<Above> {
        above
            .iter()
            .map(|a| match a {
                None => Above::Own,
                Some(id) => Above::Other { id: *id, given_up: self.windows.get(id).is_some_and(|f| f.given_up) },
            })
            .collect()
    }

    /// Where the lines go now, counting nothing (the bar's own windows coming up: `raise`).
    pub fn place_now(&self, above: &[Option<u64>]) -> Place {
        place(&self.marked(above)).0
    }

    /// The bar's tick found `above` (front first; `None` for Bevel95's own) at `now_ms`: marks
    /// not seen for `GIVEN_UP_FOR_MS` end, the windows given up on that are above are seen, every
    /// other program's window the lines cover is counted (a quick return, `QUICK_RETURN_MS`,
    /// counts on; more than `FIGHTS_BEFORE_GIVING_UP` in a row gives it up, and then it is not
    /// covered), and where the lines go.
    pub fn tick(&mut self, above: &[Option<u64>], now_ms: u64) -> Verdict {
        for id in above.iter().flatten() {
            if let Some(f) = self.windows.get_mut(id)
                && f.given_up
            {
                f.last_ms = now_ms;
            }
        }
        self.windows.retain(|_, f| now_ms.saturating_sub(f.last_ms) <= GIVEN_UP_FOR_MS);
        let (_, covered) = place(&self.marked(above));
        let mut gave_up = None;
        for id in covered {
            let fight = self.windows.entry(id).or_insert(Fight { times: 0, last_ms: 0, given_up: false });
            let quick = fight.times > 0 && now_ms.saturating_sub(fight.last_ms) <= QUICK_RETURN_MS;
            fight.times = if quick { fight.times + 1 } else { 1 };
            fight.last_ms = now_ms;
            if fight.times > FIGHTS_BEFORE_GIVING_UP {
                fight.given_up = true;
                self.given_up += 1;
                gave_up = Some(self.given_up);
            }
        }
        // A window given up on just now is not covered once more: it would only come back again.
        Verdict { place: place(&self.marked(above)).0, gave_up }
    }

    /// The windows given up on (the platform ends a mark whose window is gone or hidden: `forget`).
    pub fn given_up_ids(&self) -> Vec<u64> {
        self.windows.iter().filter(|(_, f)| f.given_up).map(|(&id, _)| id).collect()
    }

    /// Forgets window `id` (gone or hidden: a window shown again is looked at afresh).
    pub fn forget(&mut self, id: u64) {
        self.windows.remove(&id);
    }

    /// How many windows are remembered.
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}

/// The folder of the Bevel95 copy a program file belongs to (lower case, `\` only): the program's
/// folder, or the one above a `programs` folder (where a copy keeps its companion programs).
/// Two programs whose copy folders are equal belong to one copy (brief 104: the bar treats the
/// windows of its own copy's programs as its own). Pure.
pub fn copy_folder(program: &str) -> String {
    let path = program.replace('/', "\\").to_lowercase();
    let dir = match path.rfind('\\') {
        Some(i) => &path[..i],
        None => "",
    };
    match dir.rsplit_once('\\') {
        Some((parent, "programs")) => parent.to_string(),
        _ => dir.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strengths_and_their_settings() {
        assert_eq!(Strength::ALL.map(Strength::alpha), [0, 38, 59, 89]);
        assert_eq!(Strength::ALL.map(Strength::setting), ["off", "light", "medium", "strong"]);
        assert_eq!(Strength::ALL.map(Strength::title), ["Off", "Light", "Medium", "Strong"]);
        for s in Strength::ALL {
            assert_eq!(Strength::from_setting(s.setting()), Some(s));
            assert_eq!(Strength::ALL[s.index()], s);
        }
        assert_eq!(Strength::from_setting("23"), None);
        assert_eq!(Strength::default(), Strength::Off);
        assert_eq!(Strength::Medium.fraction(), 0.23, "the owner's number");
    }

    #[test]
    fn lines_only_at_whole_sizes_from_two() {
        assert_eq!(period(Scale::whole(1)), None);
        assert_eq!(period(Scale::whole(2)), Some(2));
        assert_eq!(period(Scale::whole(3)), Some(3));
        assert_eq!(period(Scale::whole(4)), Some(4));
        for quarters in [5, 6, 7, 9, 10, 11, 13, 14, 15] {
            assert_eq!(period(Scale::from_quarters(quarters)), None, "{quarters} quarters");
        }
        assert!(!possible(Scale::from_quarters(7)), "1.75");
        assert!(possible(Scale::whole(8)));
    }

    #[test]
    fn the_last_row_of_each_old_pixel_row_is_dark() {
        let dark = |n: u32| (0..12).filter(|&y| dark_row(y, n)).collect::<Vec<_>>();
        assert_eq!(dark(2), [1, 3, 5, 7, 9, 11]);
        assert_eq!(dark(3), [2, 5, 8, 11]);
        assert_eq!(dark(4), [3, 7, 11]);
        assert!(dark(1).is_empty() && dark(0).is_empty());
    }

    #[test]
    fn the_picture_for_each_period_and_strength() {
        for n in [2, 3, 4] {
            for s in Strength::ALL {
                let fb = picture(7, 13, n, s);
                assert_eq!((fb.width, fb.height, fb.pixels.len()), (7, 13, 91));
                for (y, line) in fb.pixels.chunks(7).enumerate() {
                    let want = if y as u32 % n == n - 1 { u32::from(s.alpha()) << 24 } else { 0 };
                    assert!(line.iter().all(|&p| p == want), "n {n}, {s:?}, row {y}");
                }
                // Premultiplied black: no color in any pixel, only alpha.
                assert!(fb.pixels.iter().all(|p| p & 0x00FF_FFFF == 0));
                assert_eq!(row(7, n - 1, n, s), vec![dark_pixel(s); 7]);
                assert_eq!(row(7, 0, n, s), vec![0; 7]);
            }
        }
        // Medium at size 2: every second row 59 of 255.
        let fb = picture(2, 4, 2, Strength::Medium);
        assert_eq!(fb.pixels, [0, 0, 0x3B00_0000, 0x3B00_0000, 0, 0, 0x3B00_0000, 0x3B00_0000]);
    }

    #[test]
    fn laid_over_the_screen_it_darkens_the_dark_rows_only() {
        let mut fb = Framebuffer::new(2, 4, 0xFF80_C0FF);
        lay_over(&mut fb, 2, Strength::Medium);
        // 255 - 59 = 196 kept of 255: 0x80 -> 98 (0x62), 0xC0 -> 148 (0x94), 0xFF -> 196 (0xC4).
        assert_eq!(fb.pixels[..2], [0xFF80_C0FF; 2]);
        assert_eq!(fb.pixels[2..4], [0xFF62_94C4; 2]);
        assert_eq!(fb.pixels[4..6], [0xFF80_C0FF; 2]);
        let mut off = Framebuffer::new(3, 3, 0xFF12_3456);
        lay_over(&mut off, 2, Strength::Off);
        assert!(off.pixels.iter().all(|&p| p == 0xFF12_3456), "Off changes nothing");
    }

    fn other(id: u64) -> Above {
        Above::Other { id, given_up: false }
    }
    fn insisting(id: u64) -> Above {
        Above::Other { id, given_up: true }
    }

    #[test]
    fn the_lines_go_under_the_lowest_insisting_window_only() {
        // Nothing above: stay.
        assert_eq!(place(&[]), (Place::Stay, vec![]));
        // An ordinary window, the bar's own: on top, the other counted.
        assert_eq!(place(&[other(1), Above::Own]), (Place::Top, vec![1]));
        // One insisting window and one ordinary window behind it: just under the insisting one,
        // the ordinary one covered.
        assert_eq!(place(&[insisting(7), other(1)]), (Place::Under(7), vec![1]));
        // The ordinary one in front of the insisting one stays above (it cannot be put between).
        assert_eq!(place(&[other(1), insisting(7)]), (Place::Stay, vec![]));
        // Two insisting windows: under the lower one, both stay above.
        assert_eq!(place(&[insisting(7), insisting(8)]), (Place::Stay, vec![]));
        assert_eq!(place(&[insisting(7), other(2), insisting(8), other(3), Above::Own]), (Place::Under(8), vec![3]));
        // The bar's own window opening while an insisting window shows: under that window, not
        // on top of it.
        assert_eq!(place(&[insisting(7), Above::Own]), (Place::Under(7), vec![]));
        assert_eq!(place(&[Above::Own, insisting(7)]), (Place::Stay, vec![]));
    }

    #[test]
    fn an_insisting_window_is_given_up_on_and_others_are_still_covered() {
        let mut f = Fights::default();
        assert_eq!(f.tick(&[], 0), Verdict { place: Place::Stay, gave_up: None });
        // The bar's own windows always go under the lines, never counted.
        for t in 0..50 {
            assert_eq!(f.tick(&[None], t * 100).place, Place::Top);
        }
        assert!(f.is_empty());
        // Another program's window back above every tick: over it five times, then left above.
        let (mut covered, mut gave_up) = (0, None);
        for t in 0..20 {
            let v = f.tick(&[Some(7)], 1_000 + t * 100);
            covered += u32::from(v.place == Place::Top);
            gave_up = gave_up.or(v.gave_up);
        }
        assert_eq!((covered, gave_up), (FIGHTS_BEFORE_GIVING_UP, Some(1)));
        assert_eq!(f.given_up_ids(), [7]);
        // An ordinary window behind it is still covered, the lines just under the insisting one.
        assert_eq!(f.tick(&[Some(7), Some(9)], 4_000).place, Place::Under(7));
        // The bar's own window opening meanwhile: the same, the insisting one not covered again.
        assert_eq!(f.place_now(&[Some(7), None]), Place::Under(7));
        assert_eq!(f.place_now(&[Some(7)]), Place::Stay);
        // A window the person brings forward now and then (seconds apart) is never given up on.
        for t in 0..100 {
            assert_eq!(f.tick(&[Some(7), Some(9)], 10_000 + t * 5_000).place, Place::Under(7));
            assert!(f.tick(&[Some(7)], 10_000 + t * 5_000 + 500).gave_up.is_none());
        }
    }

    #[test]
    fn two_insisting_windows_and_the_mark_expiring() {
        let mut f = Fights::default();
        for t in 0..10 {
            f.tick(&[Some(7)], t * 100);
        }
        let last = (0..10).filter_map(|t| f.tick(&[Some(7), Some(8)], 2_000 + t * 100).gave_up).last();
        assert_eq!(last, Some(2), "the second one given up on too");
        let mut ids = f.given_up_ids();
        ids.sort();
        assert_eq!(ids, [7, 8]);
        assert_eq!(f.tick(&[Some(7), Some(8)], 4_000).place, Place::Stay, "both left above");
        // Seen above: the mark lasts. Not seen for three minutes: it ends, and the window is
        // looked at afresh.
        assert_eq!(f.tick(&[Some(8)], 4_001 + GIVEN_UP_FOR_MS).place, Place::Stay);
        assert_eq!(f.given_up_ids(), [8]);
        assert_eq!(f.tick(&[Some(7)], 4_000 + GIVEN_UP_FOR_MS + 200).place, Place::Top, "7 covered again");
        // Gone or hidden: the platform forgets it.
        f.forget(8);
        assert!(f.given_up_ids().is_empty());
        assert_eq!(f.len(), 1, "only 7's fresh count is remembered");
        // The list stays small: counts not renewed for three minutes go by themselves.
        f.tick(&[], 10_000 + 2 * GIVEN_UP_FOR_MS);
        assert!(f.is_empty());
    }

    #[test]
    fn the_copy_a_program_belongs_to() {
        let installed = copy_folder(r"C:\Users\x\AppData\Local\Bevel95\bevel95.exe");
        assert_eq!(installed, r"c:\users\x\appdata\local\bevel95");
        assert_eq!(copy_folder(r"C:\Users\x\AppData\Local\Bevel95\programs\calculator.exe"), installed);
        assert_eq!(copy_folder(r"c:/users/x/appdata/local/BEVEL95/Programs/folders.exe"), installed);
        assert_ne!(copy_folder(r"D:\Portable\Bevel95\programs\calculator.exe"), installed);
        assert_ne!(copy_folder(r"C:\Users\x\AppData\Local\Bevel95\other\calculator.exe"), installed);
        assert_ne!(copy_folder(r"C:\Users\x\AppData\Local\calculator.exe"), installed);
    }

    #[test]
    fn what_the_picture_takes() {
        assert_eq!(bytes(2560, 1440), 14_745_600);
        assert_eq!(bytes(3840, 2160), 33_177_600);
    }
}

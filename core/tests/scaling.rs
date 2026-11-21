//! The four ways of drawing a size between whole numbers, against their definition: the
//! pictures in `fixtures/scaling` were computed by the definition the owner chose from (a C#
//! program kept with the project's private mock-ups) for two small pictures at every quarter
//! size from 1.25 to 3.75 and at 7.75: for each, the four ways stacked from top to bottom
//! (nearest, even, even-light, smooth), each as wide and high as the definition makes it,
//! floor(w·f) by floor(h·f). Bevel95's picture is ceil(w·f) by ceil(h·f) (its last part-pixel
//! drawn as if the picture went on); every pixel the definition has must be the same.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use bevel_core::fb::Framebuffer;
use bevel_core::scale::{Scale, Way, upscale};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("scaling")
}

/// An 8-bit RGB or RGBA picture as 0xFFRRGGBB pixels.
fn read(path: &Path) -> Framebuffer {
    let file = BufReader::new(File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let mut reader = png::Decoder::new(file).read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "{}", path.display());
    let step = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => panic!("{}: {other:?}", path.display()),
    };
    let mut pixels = Vec::new();
    for y in 0..info.height as usize {
        let line = &buf[y * info.line_size..][..info.width as usize * step];
        pixels.extend(line.chunks(step).map(|c| 0xFF00_0000 | u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2])));
    }
    Framebuffer { width: info.width as i32, height: info.height as i32, pixels }
}

const WAYS: [Way; 4] = [Way::Nearest, Way::Even, Way::EvenLight, Way::Smooth];
const QUARTERS: [i32; 10] = [5, 6, 7, 9, 10, 11, 13, 14, 15, 31];

#[test]
fn the_four_ways_give_what_their_definition_gives() {
    let mut checked = 0;
    for name in ["dots", "menu"] {
        let src = read(&fixtures().join(format!("{name}.png")));
        for q in QUARTERS {
            let sheet = read(&fixtures().join(format!("{name}-q{q}.png")));
            let (w, h) = (sheet.width, sheet.height / 4);
            assert_eq!((w, h), (src.width * q / 4, src.height * q / 4), "{name} q{q}: the definition's size");
            for (n, way) in WAYS.into_iter().enumerate() {
                let s = Scale::from_quarters(q).with_way(way);
                let big = s.enlarge(&src);
                assert_eq!((big.width, big.height), (s.len(src.width), s.len(src.height)), "{name} q{q} {way:?}");
                let mut wrong = Vec::new();
                for y in 0..h {
                    for x in 0..w {
                        let (want, got) = (sheet.get(x, n as i32 * h + y), big.get(x, y) | 0xFF00_0000);
                        if want != got {
                            wrong.push(format!("{x},{y}: {want:06X} not {got:06X}"));
                        }
                        checked += 1;
                    }
                }
                assert!(wrong.is_empty(), "{name} q{q} {way:?}: {} pixels differ, first {:?}", wrong.len(), &wrong[..wrong.len().min(5)]);
            }
        }
    }
    assert!(checked > 100_000, "{checked}");
}

#[test]
fn the_last_part_pixel_goes_on_from_the_edge() {
    // Past the definition's floor(w·f), the picture is drawn as if its last column and row went
    // on: in every way the extra pixels equal what a picture one pixel wider and higher, with its
    // last column and row repeated, gives there.
    let src = read(&fixtures().join("dots.png"));
    let mut wider = Framebuffer::new(src.width + 1, src.height + 1, 0);
    for y in 0..wider.height {
        for x in 0..wider.width {
            wider.set(x, y, src.get(x.min(src.width - 1), y.min(src.height - 1)));
        }
    }
    for q in QUARTERS {
        for way in WAYS {
            let s = Scale::from_quarters(q).with_way(way);
            let (big, more) = (s.enlarge(&src), s.enlarge(&wider));
            for y in 0..big.height {
                for x in 0..big.width {
                    assert_eq!(big.get(x, y), more.get(x, y), "q{q} {way:?} {x},{y}");
                }
            }
        }
    }
}

/// How long enlarging takes for a window as big as a 2560×1440 screen (a maximized folder
/// window, the work area 2560×1392) and for the bar (2560 wide), at 1.75 in each way beside
/// today's 2. The 1× pictures are a golden folder window repeated, so that edges are as many
/// as in a real window. Run: cargo test -p bevel-core --release --test scaling -- --ignored --nocapture
#[test]
#[ignore]
fn how_long_enlarging_takes() {
    let golden = read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden").join("win98-folder-explorer.png"));
    let tiled = |w: i32, h: i32| {
        let mut fb = Framebuffer::new(w, h, 0);
        for y in 0..h {
            for x in 0..w {
                fb.set(x, y, golden.get(x % golden.width, y % golden.height));
            }
        }
        fb
    };
    let time = |label: &str, src: &Framebuffer, s: Scale| {
        let runs = 20;
        let mut best = f64::MAX;
        let mut total = 0.0;
        let mut size = (0, 0);
        for _ in 0..runs {
            let t = std::time::Instant::now();
            let big = s.enlarge(src);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            size = (big.width, big.height);
            best = best.min(ms);
            total += ms;
        }
        println!("{label:<34} {:>4}x{:<4} -> {:>4}x{:<4}  mean {:>6.2} ms  best {:>6.2} ms", src.width, src.height, size.0, size.1, total / f64::from(runs), best);
    };
    let s175 = Scale::from_quarters(7);
    let window_175 = tiled(s175.old_cover(2560), s175.old_cover(1392));
    let window_2 = tiled(1280, 696);
    let bar_175 = tiled(s175.old_cover(2560), 28);
    let bar_2 = tiled(1280, 28);
    println!("Maximized folder window (work area 2560x1392):");
    time("  2 (today, whole pixels)", &window_2, Scale::whole(2));
    for way in Way::ALL {
        time(&format!("  1.75 {}", way.setting()), &window_175, s175.with_way(way));
    }
    println!("The bar (2560 wide):");
    time("  2 (today, whole pixels)", &bar_2, Scale::whole(2));
    for way in Way::ALL {
        time(&format!("  1.75 {}", way.setting()), &bar_175, s175.with_way(way));
    }
}

#[test]
fn whole_sizes_are_byte_for_byte_as_today() {
    for name in ["dots", "menu"] {
        let src = read(&fixtures().join(format!("{name}.png")));
        for n in 1..=8 {
            for way in Way::ALL {
                assert_eq!(Scale::whole(n).with_way(way).enlarge(&src), upscale(&src, n), "{name} {n}");
            }
        }
    }
}

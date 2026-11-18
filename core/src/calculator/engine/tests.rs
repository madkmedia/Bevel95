use super::*;

/// The behaviour tables of the Reference PCs (tests/fixtures/calculator), key by key.
fn sequences(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut out: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') {
            out.push((line.trim().to_string(), Vec::new()));
            continue;
        }
        let line = line.trim();
        let (key, shown) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        out.last_mut().expect("a sequence").1.push((key.to_string(), shown.trim().to_string()));
    }
    out
}

/// What a row of a table does.
enum Step {
    Key(Key),
    View(bool),
    /// Sta: the Statistics Box opens (empty: the tables' boxes were fresh ones).
    Stats,
    Nothing,
}

fn step_of(name: &str, engine: &Engine) -> Step {
    let dec = engine.radix() == Radix::Dec;
    Step::Key(match name {
        "C(Esc)" => Key::Clear,
        "CE(Del)" => Key::ClearEntry,
        "sqrt(@)" => Key::Sqrt,
        "1/x(r)" => Key::Reciprocal,
        "+/-(F9)" => Key::Negate,
        "Back" => Key::Back,
        "MS" => Key::MemoryStore,
        "MR" => Key::MemoryRecall,
        "MC" => Key::MemoryClear,
        "M+" => Key::MemoryAdd,
        "Mod(%)" => Key::Mod,
        "And(&)" => Key::And,
        "Or(|)" => Key::Or,
        "Xor(^)" => Key::Xor,
        "Lsh(<)" => Key::Lsh,
        "Not(~)" => Key::Not,
        "Int(;)" => Key::Int,
        "sin(s)" => Key::Sin,
        "cos(o)" => Key::Cos,
        "tan(t)" => Key::Tan,
        "ln(n)" => Key::Ln,
        "log(l)" => Key::Log,
        "x^y(y)" => Key::Power,
        "x^2(@)" => Key::Square,
        "x^3(#)" => Key::Cube,
        "n!(!)" => Key::Factorial,
        "pi(p)" => Key::Pi,
        "dms(m)" => Key::Dms,
        "F-E(v)" => Key::FixedExp,
        "Exp(x)" => Key::Exp,
        "Inv(i)" => Key::Inv,
        "Hyp(h)" => Key::Hyp,
        "Dat(Ins)" => Key::Dat,
        "Ave(^A)" => Key::Ave,
        "Sum(^T)" => Key::Sum,
        "s(^D)" => Key::Dev,
        "Hex(F5)" => Key::Radix(Radix::Hex),
        "Dec(F6)" => Key::Radix(Radix::Dec),
        "Oct(F7)" => Key::Radix(Radix::Oct),
        "Bin(F8)" => Key::Radix(Radix::Bin),
        "F2" if dec => Key::Angle(Angle::Degrees),
        "F3" if dec => Key::Angle(Angle::Radians),
        "F4" if dec => Key::Angle(Angle::Grads),
        "F2" => Key::Word(Word::Dword),
        "F3" => Key::Word(Word::Word),
        "F4" => Key::Word(Word::Byte),
        "F12" => Key::Word(Word::Qword),
        "(" => Key::Open,
        ")" => Key::Close,
        "View-Std" => return Step::View(false),
        "View-Sci" => return Step::View(true),
        "Sta(^S)" => return Step::Stats,
        "Focus-Calc" => return Step::Nothing,
        other => {
            let mut chars = other.chars();
            let ch = chars.next().expect("a key");
            assert!(chars.next().is_none(), "unknown key {other}");
            let key = if engine.scientific() { key_for_char_scientific(ch) } else { key_for_char(ch) };
            key.unwrap_or_else(|| panic!("unknown key {other}"))
        }
    })
}

/// The display and the marks the tables write after it.
fn shown(engine: &Engine) -> String {
    let mut shown = engine.display();
    if engine.memory_shown() {
        shown.push_str(" [M]");
    }
    // The Standard view has no parentheses box and no check boxes.
    if !engine.scientific() {
        return shown;
    }
    if engine.open_parens() > 0 {
        shown.push_str(&format!(" [(={}]", engine.open_parens()));
    }
    if engine.inv_ticked() {
        shown.push_str(" [Inv]");
    }
    if engine.hyp_ticked() {
        shown.push_str(" [Hyp]");
    }
    shown
}

type Differ = Vec<(String, usize, String, String)>;

/// Runs every sequence of one table; returns the rows that differ as (sequence, row).
fn run(era: Era, table: &str, scientific: bool) -> (usize, usize, Differ) {
    let mut differ = Vec::new();
    let all = sequences(table);
    let mut engine = Engine::new(era);
    engine.set_view(scientific);
    let mut rows = 0;
    for (name, keys) in &all {
        // Windows 2000's PC was put back to Qword before each sequence (not recorded).
        if era == Era::Win2000 {
            engine.word = Word::Qword;
        }
        // Each Scientific sequence started with the memory empty, Inv and Hyp off, Degrees and
        // no error (the measuring put the PC there between the sequences).
        if scientific {
            // (95's PC was cleared before these: their first rows show 0 where 98 carried the
            // number of the sequence before.)
            const CLEARED_95: &[&str] = &[
                "b05-hex-arith", "b06-word-overflow", "b07-hex-neg", "b08-hex-not-lsh", "b10-bin-typing",
                "b19-hex-memory", "v03-hex-to-std", "v05-std-no-precedence", "v06-inv-to-std",
            ];
            if engine.error.is_some() || (era == Era::Win95 && CLEARED_95.contains(&name.as_str())) {
                engine.press(Key::Clear);
            }
            engine.memory = Engine::zero_of(era);
            engine.untick(true, true);
            engine.angle = Angle::Degrees;
        }
        for (i, (key, expected)) in keys.iter().enumerate() {
            match step_of(key, &engine) {
                Step::Key(key) => engine.press(key),
                Step::View(scientific) => {
                    engine.set_view(scientific);
                }
                Step::Stats => {
                    engine.close_stats();
                    engine.open_stats();
                }
                Step::Nothing => {}
            }
            rows += 1;
            let shown = shown(&engine);
            if &shown != expected {
                differ.push((name.clone(), i + 1, expected.clone(), shown));
            }
        }
    }
    (all.len(), rows, differ)
}

/// The rows the Reference PCs showed that Bevel95 does not reproduce (docs/decisions.md,
/// "Calculator", and private/todo.md). Windows 98's and 2000's numbers were a few digits off past
/// about the 40th (their square root of 2, pi, e), which shows where a result is all but zero;
/// Windows 95's digits past the 15th (sqrt 2 − 1.414213562373) and its x87 sine of a huge angle.
const KNOWN: &[(Era, &str, usize)] = &[
    (Era::Win98, "s18-2sqrt-tee", 8),
    (Era::Win2000, "s18-2sqrt-tee", 8),
];

fn check(era: Era, table: &str, scientific: bool, at_least: usize, known: &[(Era, &str, usize)]) {
    let (count, _, differ) = run(era, table, scientific);
    assert!(count >= at_least, "{era:?}: {count} sequences");
    let unexpected: Vec<_> =
        differ.iter().filter(|(name, row, ..)| !known.contains(&(era, name.as_str(), *row))).collect();
    let list: Vec<String> =
        unexpected.iter().map(|(name, row, want, got)| format!("{name}:{row} want [{want}] got [{got}]")).collect();
    assert!(unexpected.is_empty(), "{era:?}: {} rows differ:\n{}", unexpected.len(), list.join("\n"));
    for &(known_era, name, row) in known.iter().filter(|k| k.0 == era) {
        assert!(
            differ.iter().any(|(n, r, ..)| n == name && *r == row),
            "{known_era:?} {name} row {row} matches now: take it off the known list"
        );
    }
}

#[test]
fn windows_95_key_by_key() {
    check(Era::Win95, include_str!("../../../tests/fixtures/calculator/behaviour-win95.txt"), false, 39, KNOWN);
}

#[test]
fn windows_98_key_by_key() {
    check(Era::Win98, include_str!("../../../tests/fixtures/calculator/behaviour-win98.txt"), false, 39, KNOWN);
}

#[test]
fn windows_2000_key_by_key() {
    check(Era::Win2000, include_str!("../../../tests/fixtures/calculator/behaviour-win2000.txt"), false, 39, KNOWN);
}

/// The Scientific view's rows Bevel95 does not reproduce (private/todo.md):
/// - 98 and 2000: what is left after subtracting the 32 digits shown of sqrt 2, 1/3, pi and e
///   (their sqrt 2 was off by about 1e-38 on 98 and 4e-38 on 2000, pi right to 46 digits, e to
///   44, 1/3 kept as a fraction; Bevel95's 48 digits are right), so also sqrt 2 squared − 2;
/// - 95: the same leftovers (its doubles were cut in a way not found: 9.5e-14 showed as
///   0.0000000000001) and the x87 sine of 1e20° and 36000001°;
/// - 95 and 98: F4 right after the Radians sequence showed 0. (95) and 45. (98), a repaint of
///   something earlier that was not worked out.
#[rustfmt::skip]
const KNOWN_SCIENTIFIC: &[(Era, &str, usize)] = &[
    (Era::Win95, "f05-grad", 1), (Era::Win95, "f23-trig-big", 6), (Era::Win95, "f23-trig-big", 16),
    (Era::Win95, "h01-sqrt2", 39), (Era::Win95, "h02-third", 40), (Era::Win95, "h03-pi", 37),
    (Era::Win95, "h04-e", 39), (Era::Win95, "std-h01", 39), (Era::Win95, "std-h01", 40),
    (Era::Win95, "std-h02", 1),
    (Era::Win98, "f05-grad", 1), (Era::Win98, "f22-sqrt-hidden", 8), (Era::Win98, "h01-sqrt2", 39),
    (Era::Win98, "h02-third", 40), (Era::Win98, "h03-pi", 37), (Era::Win98, "h04-e", 39),
    (Era::Win98, "std-h01", 39), (Era::Win98, "std-h02", 9), (Era::Win98, "std-h02", 10),
    (Era::Win98, "std-h02", 44),
    (Era::Win2000, "f22-sqrt-hidden", 8), (Era::Win2000, "h01-sqrt2", 39), (Era::Win2000, "h02-third", 40),
    (Era::Win2000, "h03-pi", 37), (Era::Win2000, "h04-e", 39), (Era::Win2000, "std-h01", 39),
    (Era::Win2000, "std-h02", 9), (Era::Win2000, "std-h02", 10), (Era::Win2000, "std-h02", 44),
];

#[test]
fn windows_95_scientific_key_by_key() {
    let table = include_str!("../../../tests/fixtures/calculator/behaviour-sci-win95.txt");
    check(Era::Win95, table, true, 90, KNOWN_SCIENTIFIC);
}

#[test]
fn windows_98_scientific_key_by_key() {
    let table = include_str!("../../../tests/fixtures/calculator/behaviour-sci-win98.txt");
    check(Era::Win98, table, true, 90, KNOWN_SCIENTIFIC);
}

#[test]
fn windows_2000_scientific_key_by_key() {
    let table = include_str!("../../../tests/fixtures/calculator/behaviour-sci-win2000.txt");
    check(Era::Win2000, table, true, 90, KNOWN_SCIENTIFIC);
}

#[test]
fn digit_grouping() {
    let mut e = Engine::new(Era::Win2000);
    e.grouping = true;
    for ch in "1234567.89".chars() {
        e.press(key_for_char(ch).unwrap());
    }
    assert_eq!(e.display(), "1,234,567.89");
    e.press(Key::Clear);
    for ch in "4096".chars() {
        e.press(key_for_char(ch).unwrap());
    }
    e.press(Key::Negate);
    assert_eq!(e.display(), "-4,096.");
    e.press(Key::Clear);
    for ch in "12345678901234567890123456789012".chars() {
        e.press(key_for_char(ch).unwrap());
    }
    assert_eq!(e.display(), "12,345,678,901,234,567,890,123,456,789,012.");
    assert_eq!(group("123."), "123.");
    assert_eq!(group("1234."), "1,234.");
    assert_eq!(group("2.5e+40"), "2.5e+40");
    // Hex and Bin by four, with a space
    e.set_view(true);
    for key in paste_keys_in("0-1=", true) {
        e.press(key);
    }
    e.press(Key::Radix(Radix::Hex));
    assert_eq!(e.display(), "FFFF FFFF FFFF FFFF");
    e.press(Key::Word(Word::Byte));
    e.press(Key::Radix(Radix::Bin));
    assert_eq!(e.display(), "1111 1111");
}

#[test]
fn paste_types_keys_and_codes() {
    assert_eq!(paste_keys("12+3="), [Key::Digit(1), Key::Digit(2), Key::Add, Key::Digit(3), Key::Equals]);
    assert_eq!(paste_keys("5:m:q:r :p:c:x"), [
        Key::Digit(5),
        Key::MemoryStore,
        Key::Clear,
        Key::MemoryRecall,
        Key::MemoryAdd,
        Key::MemoryClear
    ]);
    assert_eq!(paste_keys_in("1:e5\\@a", true), [Key::Digit(1), Key::Exp, Key::Digit(5), Key::Dat, Key::Square, Key::Digit(10)]);
    let mut e = Engine::new(Era::Win98);
    for key in paste_keys("2*3=") {
        e.press(key);
    }
    assert_eq!(e.display(), "6.");
    assert_eq!(e.copy_text(), "6");
}

#[test]
fn a_look_change_keeps_the_number() {
    let mut e = Engine::new(Era::Win98);
    for key in paste_keys("1/4=") {
        e.press(key);
    }
    let e95 = e.with_era(Era::Win95);
    assert_eq!(e95.display(), "0.25");
    assert_eq!(e95.with_era(Era::Win2000).display(), "0.25");
}

#[test]
fn statistics() {
    let mut e = Engine::new(Era::Win98);
    e.set_view(true);
    e.open_stats();
    for key in paste_keys_in("5\\7\\2\\", true) {
        e.press(key);
    }
    assert_eq!(e.stats_lines(), ["5.", "7.", "2."]);
    e.stats_remove(2);
    e.stats_load(0);
    assert_eq!(e.display(), "5.");
    e.press(Key::Ave);
    assert_eq!(e.display(), "6.");
    e.stats_clear();
    assert!(e.stats_lines().is_empty());
    e.close_stats();
    assert!(!e.stats_open());
}

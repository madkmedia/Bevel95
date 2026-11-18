//! What the Calculator computes and shows, key by key, as each Windows' did (measured on the
//! Reference PCs, every key read off the display: docs/decisions.md, "Calculator").
//!
//! Shared by all three: no precedence in the Standard view (2 + 3 × 4 = 20); an operator right
//! after another replaces it; "=" again repeats the last operation with the same right-hand
//! number (2 + 3 = = gives 8); "%" makes the number that percent of the left-hand one (also with
//! no operator: then of the last left-hand number, 0 after C); +/- while typing keeps typing;
//! Backspace only takes back typed digits; CE clears the number being typed, C everything but the
//! memory; while an error shows only C and CE do something.
//!
//! Where they differ:
//! - Windows 95: the C runtime's double, each result of + − × ÷ rounded to 15 significant digits
//!   (which is why 0.1 + 0.2 − 0.3 and 2 sqrt × = − 2 show 0 there: an inference that fits every
//!   sequence measured), 13 digits shown and typed; a number typed after "=" ends
//!   the repeated operation (2 + 3 = 4 = gives 4); "." starting a number does not show "0." until
//!   the next digit; its own error texts.
//! - Windows 98 and 2000: `Decimal` (48 digits kept), 32 characters typed; "=" after a number
//!   typed following "=" repeats the last operation on the last result and the typed number is
//!   lost (2 + 3 = 4 = gives 8, as their CalcEngine did); 98 shows 33 significant digits and
//!   shortens the exponent form to fit, 2000 shows 32 and lets a long exponent form run off the
//!   display's left edge; 2000 can group digits.
//!
//! The Scientific view adds precedence (Or Xor < And < + − < × ÷ Mod Lsh < x^y) with up to 25
//! open parentheses, number bases with word sizes, the functions (with Inv and Hyp), the
//! exponent typed after Exp, F-E and the statistics. Windows 95 keeps its numbers as 32-bit whole
//! numbers in Hex, Oct and Bin (the word size only cuts what is shown); 98 and 2000 cut the
//! number itself to the word size (2000 has Qword). Keys of buttons 98 and 2000 gray out do
//! nothing there; 95 grays nothing and mostly ignores them.

use super::number::{Decimal, Fault};
use crate::theme::Era;
use std::cmp::Ordering;

/// A key of the Calculator (the buttons, or what the keyboard and Paste give).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// 0 to 15 (A to F in Hex).
    Digit(u8),
    Point,
    Add,
    Subtract,
    Multiply,
    Divide,
    Equals,
    Sqrt,
    Percent,
    Reciprocal,
    Negate,
    Back,
    ClearEntry,
    Clear,
    MemoryClear,
    MemoryRecall,
    MemoryStore,
    MemoryAdd,
    // The Scientific view's.
    Open,
    Close,
    Mod,
    And,
    Or,
    Xor,
    Lsh,
    Not,
    Int,
    Sin,
    Cos,
    Tan,
    Ln,
    Log,
    Power,
    Square,
    Cube,
    Factorial,
    Pi,
    Dms,
    FixedExp,
    Exp,
    Inv,
    Hyp,
    /// Sta: the Statistics Box opens (the host shows it).
    Sta,
    Ave,
    Sum,
    Dev,
    Dat,
    Radix(Radix),
    Angle(Angle),
    Word(Word),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Radix {
    Hex,
    Dec,
    Oct,
    Bin,
}

impl Radix {
    pub fn base(self) -> u32 {
        match self {
            Radix::Hex => 16,
            Radix::Dec => 10,
            Radix::Oct => 8,
            Radix::Bin => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Angle {
    Degrees,
    Radians,
    Grads,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Word {
    Qword,
    Dword,
    Word,
    Byte,
}

impl Word {
    pub fn bits(self) -> u32 {
        match self {
            Word::Qword => 64,
            Word::Dword => 32,
            Word::Word => 16,
            Word::Byte => 8,
        }
    }
}

/// A number of one edition.
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Double(f64),
    Decimal(Decimal),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
    Mod,
    And,
    Or,
    Xor,
    Lsh,
    Rsh,
    Power,
    Root,
}

impl Op {
    /// The Scientific view's precedence (their CalcEngine's table).
    fn precedence(self) -> u8 {
        match self {
            Op::Or | Op::Xor => 0,
            Op::And => 1,
            Op::Add | Op::Subtract => 2,
            Op::Multiply | Op::Divide | Op::Mod | Op::Lsh | Op::Rsh => 3,
            Op::Power | Op::Root => 4,
        }
    }
}

/// What an open parenthesis put aside.
#[derive(Clone, Debug)]
struct Level {
    last: Value,
    op: Option<Op>,
    pending: Vec<(Value, Op)>,
    fresh_op: bool,
    hold: Value,
}

/// The Calculator's arithmetic and display, for one Windows.
#[derive(Clone, Debug)]
pub struct Engine {
    era: Era,
    /// The number shown (the right-hand one), when nothing is being typed.
    current: Value,
    /// The left-hand number.
    last: Value,
    /// The right-hand number "=" repeats.
    hold: Value,
    op: Option<Op>,
    /// Lower operators still waiting (Scientific: 2 + 3 × keeps 2 + here).
    pending: Vec<(Value, Op)>,
    /// What each open parenthesis put aside.
    parens: Vec<Level>,
    /// No "=" since the operator: the next "=" takes the shown number as its right-hand one.
    fresh_op: bool,
    /// The last key was an operator: another one replaces it.
    change_op: bool,
    /// The last key was "=" (or a key that only showed its result since).
    after_equals: bool,
    /// The last key was "(": 98 and 2000 keep showing the parentheses through an operator.
    after_paren: bool,
    /// The digits being typed ("-12.5", or "FF" in Hex), if a number is being typed.
    entry: Option<String>,
    /// The exponent being typed after Exp: its sign (true: minus) and digits.
    exponent: Option<(bool, String)>,
    /// What the display shows for now instead of the number (95's "." starting a number, the
    /// parentheses, an operator replaced).
    stale: Option<String>,
    error: Option<Fault>,
    memory: Value,
    /// Windows 2000's View > Digit grouping.
    pub grouping: bool,
    scientific: bool,
    radix: Radix,
    angle: Angle,
    word: Word,
    inv: bool,
    /// The Inv check box (98 and 2000 untick it on a change of view and keep Inv on).
    inv_shown: bool,
    hyp: bool,
    /// F-E: numbers in the exponent form.
    fixed_exp: bool,
    /// The Statistics Box's numbers, while it is open.
    stats: Option<Vec<Value>>,
}

impl Engine {
    pub fn new(era: Era) -> Engine {
        let zero = Engine::zero_of(era);
        Engine {
            era,
            current: zero.clone(),
            last: zero.clone(),
            hold: zero.clone(),
            op: None,
            pending: Vec::new(),
            parens: Vec::new(),
            fresh_op: true,
            change_op: false,
            after_equals: false,
            after_paren: false,
            entry: None,
            exponent: None,
            stale: None,
            error: None,
            memory: zero,
            grouping: false,
            scientific: false,
            radix: Radix::Dec,
            angle: Angle::Degrees,
            word: Engine::default_word(era),
            inv: false,
            inv_shown: false,
            hyp: false,
            fixed_exp: false,
            stats: None,
        }
    }

    fn default_word(era: Era) -> Word {
        if era == Era::Win2000 { Word::Qword } else { Word::Dword }
    }

    fn zero_of(era: Era) -> Value {
        if era == Era::Win95 { Value::Double(0.0) } else { Value::Decimal(Decimal::zero()) }
    }

    pub fn era(&self) -> Era {
        self.era
    }

    pub fn scientific(&self) -> bool {
        self.scientific
    }

    pub fn radix(&self) -> Radix {
        self.radix
    }

    pub fn angle(&self) -> Angle {
        self.angle
    }

    pub fn word(&self) -> Word {
        self.word
    }

    /// The Inv and Hyp check boxes.
    pub fn inv_ticked(&self) -> bool {
        self.inv_shown
    }

    pub fn hyp_ticked(&self) -> bool {
        self.hyp
    }

    /// How many parentheses are open ("(=n" in its box).
    pub fn open_parens(&self) -> usize {
        self.parens.len()
    }

    /// A number is being typed (98 and 2000 gray Exp while not).
    pub fn typing(&self) -> bool {
        self.entry.is_some()
    }

    /// The same numbers in another Windows' arithmetic (a look change): what shows stays, as
    /// near as the other can keep it; an error or a number being typed is cleared.
    pub fn with_era(&self, era: Era) -> Engine {
        let mut e = Engine::new(era);
        e.grouping = self.grouping;
        e.scientific = self.scientific;
        if self.error.is_none() && self.radix == Radix::Dec {
            e.current = e.convert(&self.value_shown());
            e.memory = e.convert(&self.memory);
        }
        e
    }

    fn convert(&self, v: &Value) -> Value {
        match (self.era, v) {
            (Era::Win95, Value::Decimal(d)) => {
                let (digits, exp) = d.round_to(17);
                let text: String = digits.iter().map(|d| char::from(b'0' + d)).collect();
                let x = format!("{}0.{text}e{}", if d.is_negative() { "-" } else { "" }, exp + 1).parse().unwrap_or(0.0);
                Value::Double(if x == 0.0 || !f64::is_finite(x) { 0.0 } else { x })
            }
            (Era::Win95, v) => v.clone(),
            (_, Value::Double(x)) => Value::Decimal(decimal_of(*x)),
            (_, v) => v.clone(),
        }
    }

    /// The memory holds a number other than zero (the "M" shows).
    pub fn memory_shown(&self) -> bool {
        !self.is_zero(&self.memory)
    }

    pub fn error(&self) -> Option<Fault> {
        self.error
    }

    fn is_zero(&self, v: &Value) -> bool {
        match v {
            Value::Double(x) => *x == 0.0,
            Value::Decimal(d) => d.is_zero(),
        }
    }

    fn parse(&self, text: &str) -> Value {
        match self.era {
            Era::Win95 => Value::Double(text.parse().unwrap_or(0.0)),
            _ => Value::Decimal(Decimal::parse(text).unwrap_or_else(Decimal::zero)),
        }
    }

    fn int(&self, n: i128) -> Value {
        match self.era {
            Era::Win95 => Value::Double(n as f64),
            _ => Value::Decimal(Decimal::from_i128(n)),
        }
    }

    /// The typed text as a number (in the base shown).
    fn parse_entry(&self, text: &str) -> Result<Value, Fault> {
        if self.radix != Radix::Dec {
            let (neg, digits) = match text.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, text),
            };
            let n = u128::from_str_radix(digits, self.radix.base()).unwrap_or(0) as i128;
            return self.whole(self.int(if neg { -n } else { n }));
        }
        let mut v = self.parse(text);
        if let Some((neg, digits)) = &self.exponent {
            let e: i64 = digits.parse().unwrap_or(0);
            let e = if *neg { -e } else { e };
            v = match v {
                Value::Double(x) => Value::Double(format!("{x}e{e}").parse().unwrap_or(f64::INFINITY)),
                Value::Decimal(d) => Value::Decimal(d.mul(&Decimal::from_int(10).powi(e)?)?),
            };
            if let Value::Double(x) = v
                && !x.is_finite()
            {
                return Err(Fault::Overflow);
            }
        }
        Ok(v)
    }

    /// The number shown, as a value (the typed one, if a number is being typed).
    fn value_shown(&self) -> Value {
        match &self.entry {
            Some(text) => self.parse_entry(text).unwrap_or_else(|_| Engine::zero_of(self.era)),
            None => self.current.clone(),
        }
    }

    /// Ends typing: the typed number becomes the shown value.
    fn take_entry(&mut self) {
        if let Some(text) = self.entry.take() {
            match self.parse_entry(&text) {
                Ok(v) => self.current = v,
                Err(fault) => self.error = Some(fault),
            }
        }
        self.exponent = None;
        self.stale = None;
    }

    /// What the display shows.
    pub fn display(&self) -> String {
        if let Some(fault) = self.error {
            return error_text(self.era, fault).to_string();
        }
        if let Some(shown) = &self.stale {
            return shown.clone();
        }
        if self.radix != Radix::Dec {
            let text = match (&self.entry, self.era) {
                (Some(text), Era::Win95) => {
                    // 95 takes more digits than the word size shows: it shows the cut value.
                    let v = self.parse_entry(text).unwrap_or_else(|_| Engine::zero_of(self.era));
                    self.in_base(&v)
                }
                (Some(text), _) => text.clone(),
                (None, _) => self.in_base(&self.current),
            };
            return if self.grouping && self.era == Era::Win2000 && matches!(self.radix, Radix::Hex | Radix::Bin) {
                group_by_four(&text)
            } else {
                text
            };
        }
        let text = match &self.entry {
            Some(text) => {
                let mantissa = if text.contains('.') { text.clone() } else { format!("{text}.") };
                match &self.exponent {
                    None => mantissa,
                    Some((neg, digits)) => {
                        let sign = if *neg { '-' } else { '+' };
                        if self.era == Era::Win95 {
                            format!("{mantissa}e{sign}{digits:0>3}")
                        } else {
                            format!("{mantissa}e{sign}{}", if digits.is_empty() { "0" } else { digits })
                        }
                    }
                }
            }
            None if self.fixed_exp && self.scientific => format_exponent(self.era, &self.current),
            None => format_value(self.era, &self.current),
        };
        if self.grouping && self.era == Era::Win2000 { group(&text) } else { text }
    }

    /// A whole number in the base shown, cut to the word size.
    fn in_base(&self, v: &Value) -> String {
        let n = match v {
            Value::Double(x) => *x as i128,
            Value::Decimal(d) => d.trunc().to_i128().unwrap_or(0),
        };
        let mask: u128 = (1u128 << self.word.bits()) - 1;
        let n = (n as u128) & mask;
        match self.radix {
            Radix::Hex => format!("{n:X}"),
            Radix::Oct => format!("{n:o}"),
            Radix::Bin => format!("{n:b}"),
            Radix::Dec => n.to_string(),
        }
    }

    /// The text Edit > Copy puts on the clipboard: the display without a point at its end.
    pub fn copy_text(&self) -> String {
        let text = self.display();
        if self.error.is_some() {
            return text;
        }
        text.strip_suffix('.').map(str::to_string).unwrap_or(text)
    }

    /// A whole number in Hex, Oct or Bin: 95 keeps 32 bits (signed), 98 and 2000 cut it to the
    /// word size (unsigned).
    fn whole(&self, v: Value) -> Result<Value, Fault> {
        if self.radix == Radix::Dec {
            return Ok(v);
        }
        match v {
            Value::Double(x) => {
                let x = x.trunc();
                if !(-2147483648.0..4294967296.0).contains(&x) {
                    return Err(Fault::Overflow);
                }
                Ok(Value::Double(if x >= 2147483648.0 { x - 4294967296.0 } else { x }))
            }
            Value::Decimal(d) => Ok(Value::Decimal(wrap(&d.trunc(), self.word.bits()))),
        }
    }

    fn arith(&self, op: Op, a: &Value, b: &Value) -> Result<Value, Fault> {
        let v = match (a, b) {
            (Value::Double(a), Value::Double(b)) => self.arith_double(op, *a, *b)?,
            (Value::Decimal(a), Value::Decimal(b)) => Value::Decimal(self.arith_decimal(op, a, b)?),
            _ => return Err(Fault::Domain),
        };
        self.whole(v)
    }

    fn arith_double(&self, op: Op, a: f64, b: f64) -> Result<Value, Fault> {
        // 95's whole-number operators work on 32 bits.
        let int = |x: f64| -> Result<i64, Fault> {
            let x = x.trunc();
            if (-2147483648.0..4294967296.0).contains(&x) { Ok(x as i64) } else { Err(Fault::Overflow) }
        };
        let r = match op {
            Op::Add => a + b,
            Op::Subtract => a - b,
            Op::Multiply => a * b,
            Op::Divide | Op::Mod if b == 0.0 => return Err(Fault::DivideByZero),
            Op::Divide if self.radix != Radix::Dec => (a / b).trunc(),
            Op::Divide => a / b,
            Op::Mod => a % b,
            Op::And => (int(a)? & int(b)?) as i32 as f64,
            Op::Or => (int(a)? | int(b)?) as i32 as f64,
            Op::Xor => (int(a)? ^ int(b)?) as i32 as f64,
            Op::Lsh => ((int(a)? as i32).wrapping_shl(int(b)? as u32 & 31)) as f64,
            Op::Rsh => ((int(a)? as i32).wrapping_shr(int(b)? as u32 & 31)) as f64,
            Op::Power => power95(a, b)?,
            Op::Root => {
                if b == 0.0 || a < 0.0 {
                    return Err(Fault::Invalid);
                }
                a.powf(1.0 / b)
            }
        };
        double(r)
    }

    fn arith_decimal(&self, op: Op, a: &Decimal, b: &Decimal) -> Result<Decimal, Fault> {
        let zero_divisor = || {
            if a.is_zero() {
                Fault::Undefined
            } else if a.is_negative() {
                Fault::DivideByZeroNegative
            } else {
                Fault::DivideByZero
            }
        };
        let bits = |f: fn(i128, i128) -> i128| -> Result<Decimal, Fault> {
            // 98 and 2000: a number with a fraction gives 0.
            match (a.to_i128(), b.to_i128()) {
                (Some(x), Some(y)) => Ok(Decimal::from_i128(f(x, y))),
                _ if a.is_integer() && b.is_integer() => Err(Fault::Overflow),
                _ => Ok(Decimal::zero()),
            }
        };
        match op {
            Op::Add => a.add(b),
            Op::Subtract => a.sub(b),
            Op::Multiply => a.mul(b),
            Op::Divide if b.is_zero() => Err(zero_divisor()),
            Op::Divide if self.radix != Radix::Dec => Ok(a.div(b)?.trunc()),
            Op::Divide => a.div(b),
            Op::Mod if b.is_zero() => Err(Fault::Undefined),
            Op::Mod => a.sub(&b.mul(&a.div(b)?.trunc())?),
            Op::And => bits(|x, y| x & y),
            Op::Or => bits(|x, y| x | y),
            Op::Xor => bits(|x, y| x ^ y),
            Op::Lsh => a.mul(&Decimal::from_int(2).powi(shift(b)?)?),
            Op::Rsh => a.div(&Decimal::from_int(2).powi(shift(b)?)?),
            Op::Power => {
                let r = a.pow(b)?;
                if r.exponent() > 9999 { Err(Fault::Invalid) } else { Ok(r) }
            }
            Op::Root => {
                if b.is_zero() {
                    return Err(Fault::Invalid);
                }
                let inverse = Decimal::from_int(1).div(b)?;
                if a.is_negative() {
                    // 98: an odd root of a negative number is negative; 2000 refuses.
                    let odd = b.to_i128().is_some_and(|n| n % 2 != 0);
                    if self.era == Era::Win2000 || !odd {
                        return Err(Fault::Invalid);
                    }
                    return Ok(a.negate().pow(&inverse)?.negate());
                }
                a.pow(&inverse)
            }
        }
    }

    /// One key.
    pub fn press(&mut self, key: Key) {
        if self.error.is_some() {
            if matches!(key, Key::Clear | Key::ClearEntry) {
                self.clear();
            }
            return;
        }
        if self.ignored(key) {
            return;
        }
        let paren = std::mem::take(&mut self.after_paren);
        let shown_before = self.stale.take();
        match key {
            Key::Digit(d) => self.digit(d),
            Key::Point => self.point(shown_before),
            Key::Add => self.operator(Op::Add, shown_before, paren),
            Key::Subtract => self.operator(Op::Subtract, shown_before, paren),
            Key::Multiply => self.operator(Op::Multiply, shown_before, paren),
            Key::Divide => self.operator(Op::Divide, shown_before, paren),
            Key::Mod => self.operator(Op::Mod, shown_before, paren),
            Key::And => self.operator(Op::And, shown_before, paren),
            Key::Or => self.operator(Op::Or, shown_before, paren),
            Key::Xor => self.operator(Op::Xor, shown_before, paren),
            Key::Lsh => {
                let op = if self.inv { Op::Rsh } else { Op::Lsh };
                self.untick(true, false);
                self.operator(op, shown_before, paren)
            }
            Key::Power => {
                let op = if self.inv { Op::Root } else { Op::Power };
                self.operator(op, shown_before, paren)
            }
            Key::Equals => self.equals(),
            Key::Open => self.open(shown_before),
            Key::Close => self.close(shown_before),
            Key::Sqrt => self.unary(|e, v| e.sqrt(v)),
            Key::Reciprocal => self.unary(|e, v| e.arith(Op::Divide, &e.one(), v)),
            Key::Percent => self.unary(|e, v| {
                let product = e.arith(Op::Multiply, v, &e.last)?;
                e.arith(Op::Divide, &product, &e.parse("100"))
            }),
            Key::Negate => self.negate(shown_before),
            Key::Back => self.back(shown_before),
            Key::ClearEntry => {
                self.entry = None;
                self.exponent = None;
                self.current = Engine::zero_of(self.era);
            }
            Key::Clear => self.clear(),
            Key::MemoryClear => {
                self.stale = shown_before;
                self.memory = Engine::zero_of(self.era);
            }
            Key::MemoryRecall => {
                self.entry = None;
                self.exponent = None;
                self.current = self.memory.clone();
                self.change_op = false;
            }
            Key::MemoryStore => {
                self.take_entry();
                self.memory = self.current.clone();
            }
            Key::MemoryAdd => {
                self.take_entry();
                match self.arith(Op::Add, &self.memory, &self.current) {
                    Ok(sum) => self.memory = sum,
                    Err(fault) => self.error = Some(fault),
                }
            }
            Key::Inv => {
                self.stale = shown_before;
                self.inv = !self.inv_shown;
                self.inv_shown = self.inv;
            }
            Key::Hyp => {
                self.stale = shown_before;
                self.hyp = !self.hyp;
            }
            Key::FixedExp => {
                // A number being typed is ended; the one shown changes its form.
                if self.entry.is_some() {
                    self.take_entry();
                    self.change_op = false;
                } else {
                    self.stale = shown_before;
                }
                self.fixed_exp = !self.fixed_exp;
            }
            Key::Exp => self.exp_key(shown_before),
            Key::Radix(radix) => self.set_radix(radix),
            Key::Angle(angle) => {
                self.stale = shown_before;
                self.angle = angle;
            }
            Key::Word(word) => self.set_word(word, shown_before),
            Key::Not => self.unary(|e, v| e.not(v)),
            Key::Int => {
                let frac = self.inv;
                self.untick(true, false);
                self.unary(move |e, v| e.int_part(v, frac))
            }
            Key::Sin | Key::Cos | Key::Tan => self.trig(key),
            Key::Ln | Key::Log => {
                let (inv, ten) = (self.inv, key == Key::Log);
                self.function(true, false, move |e, v| e.logarithm(v, inv, ten));
            }
            Key::Square => {
                let inv = self.inv;
                self.function(true, false, move |e, v| if inv { e.sqrt(v) } else { e.arith(Op::Multiply, v, v) });
            }
            Key::Cube => {
                let inv = self.inv;
                self.function(true, false, move |e, v| e.cube(v, inv));
            }
            Key::Factorial => self.unary(|e, v| e.factorial(v)),
            Key::Pi => {
                let two = self.inv;
                self.untick(true, false);
                if self.radix != Radix::Dec {
                    self.stale = shown_before;
                    return;
                }
                self.entry = None;
                self.exponent = None;
                self.current = match self.era {
                    Era::Win95 => Value::Double(if two { 2.0 * std::f64::consts::PI } else { std::f64::consts::PI }),
                    _ => {
                        let pi = Decimal::pi();
                        Value::Decimal(if two { pi.mul(&Decimal::from_int(2)).unwrap_or(pi) } else { pi })
                    }
                };
                self.change_op = false;
            }
            Key::Dms => {
                let inv = self.inv;
                if self.radix != Radix::Dec {
                    self.stale = shown_before;
                    return;
                }
                self.function(true, false, move |e, v| e.dms(v, inv));
            }
            Key::Sta => {
                self.stale = shown_before;
                self.open_stats();
            }
            Key::Dat => {
                self.take_entry();
                if let Some(data) = self.stats.as_mut() {
                    data.push(self.current.clone());
                }
            }
            Key::Ave | Key::Sum | Key::Dev => {
                if self.stats.is_none() {
                    // Windows 95 (nothing is grayed): the number typed is ended, nothing else.
                    self.take_entry();
                    return;
                }
                let inv = self.inv;
                self.function(true, false, move |e, _| e.statistic(key, inv));
            }
        }
    }

    /// Keys of buttons Windows 98 and 2000 gray out (Windows 95 ignores a few of its own).
    fn ignored(&self, key: Key) -> bool {
        let dec = self.radix == Radix::Dec;
        match key {
            Key::Digit(d) => u32::from(d) >= self.radix.base(),
            Key::Point => !dec && self.era != Era::Win95,
            Key::Exp => !dec,
            Key::Sin | Key::Cos | Key::Tan | Key::Pi | Key::Dms => !dec && self.era != Era::Win95,
            Key::Ave | Key::Sum | Key::Dev | Key::Dat => self.stats.is_none() && self.era != Era::Win95,
            _ => false,
        }
    }

    fn one(&self) -> Value {
        self.parse("1")
    }

    /// Inv and Hyp come off after a function that used them.
    fn untick(&mut self, inv: bool, hyp: bool) {
        if inv {
            self.inv = false;
            self.inv_shown = false;
        }
        if hyp {
            self.hyp = false;
        }
    }

    fn clear(&mut self) {
        let fresh = Engine::new(self.era);
        let keep = std::mem::replace(self, fresh);
        self.memory = keep.memory;
        self.grouping = keep.grouping;
        self.scientific = keep.scientific;
        self.radix = keep.radix;
        self.angle = keep.angle;
        self.word = keep.word;
        self.stats = keep.stats;
        if self.radix != Radix::Dec && self.era == Era::Win95 {
            self.current = Value::Double(0.0);
        }
    }

    /// How many characters a number typed in decimal may have.
    fn fits(&self, entry: &str) -> bool {
        let body = entry.trim_start_matches('-');
        if self.era == Era::Win95 {
            body.chars().filter(char::is_ascii_digit).count() <= 13
        } else {
            body.len() <= 32
        }
    }

    fn digit(&mut self, d: u8) {
        if let Some((_, digits)) = self.exponent.as_mut() {
            // 98 and 2000 take four digits; 95 an exponent up to 307 (1e308 was refused).
            let longer = format!("{digits}{d}");
            let fits = match self.era {
                Era::Win95 => longer.parse::<u32>().is_ok_and(|e| e <= 307),
                _ => longer.len() <= 4,
            };
            if fits {
                *digits = longer;
            }
            return;
        }
        if self.entry.is_none() {
            // Windows 95: a number typed after "=" ends the repeated operation.
            if self.after_equals && self.era == Era::Win95 {
                self.op = None;
            }
            self.entry = Some(String::new());
        }
        self.change_op = false;
        self.after_equals = false;
        let ch = char::from_digit(u32::from(d), 16).expect("a digit").to_ascii_uppercase();
        let mut entry = self.entry.clone().expect("typing");
        if entry.trim_start_matches('-') == "0" {
            // "0" then a digit: the zero goes (more zeros stay one).
            entry.pop();
        }
        entry.push(ch);
        if self.radix == Radix::Dec {
            if !self.fits(&entry) {
                return;
            }
        } else {
            let digits = entry.trim_start_matches('-');
            let n = u128::from_str_radix(digits, self.radix.base()).unwrap_or(u128::MAX);
            if self.era == Era::Win95 {
                if n > u128::from(u32::MAX) {
                    self.entry = None;
                    self.error = Some(Fault::Overflow);
                    return;
                }
            } else if n >> self.word.bits() != 0 {
                return;
            }
        }
        self.entry = Some(entry);
    }

    fn point(&mut self, shown_before: Option<String>) {
        if self.exponent.is_some() {
            self.stale = shown_before;
            return;
        }
        if self.entry.is_none() {
            if self.era == Era::Win95 {
                self.stale = Some(shown_before.unwrap_or_else(|| self.display()));
            }
            if self.after_equals && self.era == Era::Win95 {
                self.op = None;
            }
            self.entry = Some("0".to_string());
        }
        self.change_op = false;
        self.after_equals = false;
        let entry = self.entry.as_mut().expect("typing");
        if entry.is_empty() {
            entry.push('0');
        }
        if !entry.contains('.') {
            entry.push('.');
        }
    }

    fn exp_key(&mut self, shown_before: Option<String>) {
        if self.exponent.is_some() {
            self.stale = shown_before;
            return;
        }
        if self.entry.is_none() {
            // 98 and 2000 start from 0, 95 from 1.
            self.entry = Some(if self.era == Era::Win95 { "1" } else { "0" }.to_string());
        }
        self.change_op = false;
        self.after_equals = false;
        self.exponent = Some((false, String::new()));
    }

    /// The operators, with the Scientific view's precedence.
    fn operator(&mut self, op: Op, shown_before: Option<String>, after_paren: bool) {
        let replacing = self.change_op;
        self.take_entry();
        if self.error.is_some() {
            return;
        }
        if replacing {
            if !self.scientific {
                self.op = Some(op);
                self.stale = shown_before;
                return;
            }
            let shown = shown_before.unwrap_or_else(|| self.display());
            // The replaced operator never happened: its left-hand number is back.
            self.current = self.last.clone();
            match self.pending.pop() {
                Some((last, prev)) => {
                    self.last = last;
                    self.op = Some(prev);
                }
                None => self.op = None,
            }
            self.push_operator(op);
            if self.error.is_none() {
                self.stale = Some(shown);
            }
            return;
        }
        self.push_operator(op);
        if after_paren && self.era != Era::Win95 && self.error.is_none() {
            self.stale = shown_before;
        }
    }

    fn push_operator(&mut self, op: Op) {
        if self.fresh_op {
            while let Some(pending) = self.op {
                if self.scientific && pending.precedence() < op.precedence() {
                    break;
                }
                match self.reduce_one(pending) {
                    Ok(()) => {}
                    Err(fault) => {
                        self.error = Some(fault);
                        return;
                    }
                }
            }
        } else {
            // After "=" the operator was only kept for repeating.
            self.op = None;
            self.pending.clear();
        }
        if let Some(lower) = self.op {
            self.pending.push((self.last.clone(), lower));
        }
        self.last = self.current.clone();
        self.op = Some(op);
        self.change_op = true;
        self.fresh_op = true;
        self.after_equals = false;
    }

    /// The pending operator `op` done: its result is the current number, the operator below
    /// it (if any) comes up.
    fn reduce_one(&mut self, op: Op) -> Result<(), Fault> {
        if matches!(op, Op::Power | Op::Root) {
            self.untick(true, false);
        }
        let v = self.arith(op, &self.last, &self.current)?;
        self.current = v;
        match self.pending.pop() {
            Some((last, lower)) => {
                self.last = last;
                self.op = Some(lower);
            }
            None => self.op = None,
        }
        Ok(())
    }

    fn equals(&mut self) {
        // Windows 98 and 2000: a number typed since the last "=" is dropped (their CalcEngine
        // takes the held right-hand number back), so 2 + 3 = 4 = gives 8.
        if !self.fresh_op && self.op.is_some() {
            self.entry = None;
            self.exponent = None;
        }
        self.take_entry();
        if self.error.is_some() {
            return;
        }
        if self.fresh_op {
            // Everything above the last operator first (Scientific: 2 + 3 × 4 does 3 × 4).
            while !self.pending.is_empty() {
                let op = self.op.expect("an operator under the pending ones");
                if let Err(fault) = self.reduce_one(op) {
                    self.error = Some(fault);
                    return;
                }
            }
        }
        if let Some(op) = self.op {
            if self.fresh_op {
                self.hold = self.current.clone();
            } else {
                self.current = self.hold.clone();
            }
            if matches!(op, Op::Power | Op::Root) {
                self.untick(true, false);
            }
            match self.arith(op, &self.last, &self.current) {
                Ok(v) => {
                    self.current = v.clone();
                    self.last = v;
                }
                Err(fault) => {
                    self.error = Some(fault);
                    return;
                }
            }
            self.fresh_op = false;
        }
        self.change_op = false;
        self.after_equals = true;
    }

    fn open(&mut self, shown_before: Option<String>) {
        if self.parens.len() >= 25 {
            self.stale = shown_before;
            return;
        }
        // 98 and 2000 drop a number being typed; 95 goes on typing it.
        if self.era != Era::Win95 {
            self.entry = None;
            self.exponent = None;
        }
        let zero = Engine::zero_of(self.era);
        self.parens.push(Level {
            last: std::mem::replace(&mut self.last, zero.clone()),
            op: self.op.take(),
            pending: std::mem::take(&mut self.pending),
            fresh_op: self.fresh_op,
            hold: self.hold.clone(),
        });
        if self.era != Era::Win95 || self.entry.is_none() {
            self.current = zero;
        }
        self.fresh_op = true;
        self.change_op = false;
        self.after_equals = false;
        self.after_paren = true;
        self.stale = Some("(".repeat(self.parens.len()));
    }

    fn close(&mut self, shown_before: Option<String>) {
        if self.parens.is_empty() {
            self.stale = shown_before;
            return;
        }
        self.take_entry();
        if self.error.is_some() {
            return;
        }
        if self.fresh_op {
            while let Some(op) = self.op {
                if let Err(fault) = self.reduce_one(op) {
                    self.error = Some(fault);
                    return;
                }
            }
        }
        let level = self.parens.pop().expect("an open parenthesis");
        self.last = level.last;
        self.op = level.op;
        self.pending = level.pending;
        self.fresh_op = level.fresh_op;
        self.hold = level.hold;
        self.change_op = false;
        self.after_equals = false;
    }

    fn unary(&mut self, f: impl Fn(&Engine, &Value) -> Result<Value, Fault>) {
        self.take_entry();
        if self.error.is_some() {
            return;
        }
        match f(self, &self.current).and_then(|v| self.whole(v)) {
            Ok(v) => self.current = v,
            Err(fault) => self.error = Some(fault),
        }
        self.change_op = false;
    }

    /// A function that uses Inv (and Hyp): they come off after it; Windows 2000 leaves them on
    /// when the function fails.
    fn function(&mut self, inv: bool, hyp: bool, f: impl Fn(&Engine, &Value) -> Result<Value, Fault>) {
        self.unary(f);
        if self.error.is_none() || self.era != Era::Win2000 {
            self.untick(inv, hyp);
        }
    }

    fn trig(&mut self, key: Key) {
        let (inv, hyp, angle) = (self.inv, self.hyp, self.angle);
        // 95 computes in Hex too (in the angle chosen before), and cuts the result.
        self.function(true, true, move |e, v| e.trig_value(key, v, inv, hyp, angle));
    }

    fn negate(&mut self, shown_before: Option<String>) {
        if let Some((neg, _)) = self.exponent.as_mut() {
            *neg = !*neg;
            return;
        }
        if let Some(entry) = self.entry.as_mut() {
            if shown_before.is_some() && self.era == Era::Win95 {
                self.stale = shown_before;
                return;
            }
            if entry.chars().any(|c| c.is_ascii_alphanumeric() && c != '0') {
                if let Some(rest) = entry.strip_prefix('-') {
                    *entry = rest.to_string();
                } else {
                    entry.insert(0, '-');
                }
            }
            return;
        }
        let negated = match &self.current {
            Value::Double(x) => Value::Double(if *x == 0.0 { 0.0 } else { -x }),
            Value::Decimal(d) => Value::Decimal(d.negate()),
        };
        match self.whole(negated) {
            Ok(v) => self.current = v,
            Err(fault) => self.error = Some(fault),
        }
        self.change_op = false;
    }

    fn back(&mut self, shown_before: Option<String>) {
        if shown_before.is_some() && self.entry.is_some() && self.era == Era::Win95 {
            self.stale = shown_before;
            return;
        }
        if let Some((neg, digits)) = self.exponent.as_mut() {
            digits.pop();
            if digits.is_empty() && self.era != Era::Win95 {
                *neg = false;
            }
            return;
        }
        let Some(entry) = self.entry.as_mut() else {
            self.stale = shown_before;
            return;
        };
        entry.pop();
        if entry.is_empty() || entry == "-" {
            *entry = "0".to_string();
        }
    }

    fn set_radix(&mut self, radix: Radix) {
        self.take_entry();
        if self.error.is_some() || radix == self.radix {
            return;
        }
        let v = self.current.clone();
        self.radix = radix;
        if radix != Radix::Dec {
            match self.whole(v) {
                Ok(v) => self.current = v,
                Err(fault) => self.error = Some(fault),
            }
        }
    }

    fn set_word(&mut self, word: Word, shown_before: Option<String>) {
        if self.radix == Radix::Dec || (word == Word::Qword && self.era != Era::Win2000) {
            self.stale = shown_before;
            return;
        }
        self.take_entry();
        self.word = word;
        if self.era != Era::Win95 {
            match self.whole(self.current.clone()) {
                Ok(v) => self.current = v,
                Err(fault) => self.error = Some(fault),
            }
        }
    }

    /// View > Standard and View > Scientific. False: the real one did nothing (an error shows).
    pub fn set_view(&mut self, scientific: bool) -> bool {
        if self.error.is_some() {
            return false;
        }
        if self.scientific == scientific {
            return true;
        }
        self.stats = None;
        if self.era == Era::Win95 {
            // 95 keeps everything; a number in another base shows as 0 until Dec is chosen again.
            let stale = self.stale.take();
            if self.entry.is_some() {
                self.take_entry();
            } else {
                self.stale = stale;
            }
            self.scientific = scientific;
            if self.radix != Radix::Dec {
                self.radix = Radix::Dec;
                self.stale = Some("0.".to_string());
            }
            return true;
        }
        // 98 and 2000 start again (the memory stays), in Dec; Inv stays on, its box unticked.
        let inv = self.inv;
        let fresh = Engine::new(self.era);
        let keep = std::mem::replace(self, fresh);
        self.memory = keep.memory;
        self.grouping = keep.grouping;
        self.scientific = scientific;
        self.inv = inv;
        true
    }

    /// The Statistics Box opened (Sta): its numbers start empty.
    pub fn open_stats(&mut self) {
        if self.stats.is_none() {
            self.stats = Some(Vec::new());
        }
    }

    /// The Statistics Box closed: its numbers are gone.
    pub fn close_stats(&mut self) {
        self.stats = None;
    }

    pub fn stats_open(&self) -> bool {
        self.stats.is_some()
    }

    /// The Statistics Box's lines.
    pub fn stats_lines(&self) -> Vec<String> {
        let Some(data) = &self.stats else { return Vec::new() };
        data.iter()
            .map(|v| {
                let text = format_value(self.era, v);
                if self.grouping && self.era == Era::Win2000 { group(&text) } else { text }
            })
            .collect()
    }

    /// LOAD: number `i` of the box into the display.
    pub fn stats_load(&mut self, i: usize) {
        if self.error.is_some() {
            return;
        }
        if let Some(v) = self.stats.as_ref().and_then(|d| d.get(i)).cloned() {
            self.entry = None;
            self.exponent = None;
            self.stale = None;
            self.current = v;
            self.change_op = false;
        }
    }

    /// CD: number `i` taken out.
    pub fn stats_remove(&mut self, i: usize) {
        if let Some(data) = self.stats.as_mut()
            && i < data.len()
        {
            data.remove(i);
        }
    }

    /// CAD: all taken out.
    pub fn stats_clear(&mut self) {
        if let Some(data) = self.stats.as_mut() {
            data.clear();
        }
    }

    // The functions.

    fn sqrt(&self, v: &Value) -> Result<Value, Fault> {
        match v {
            Value::Double(x) if *x < 0.0 => Err(Fault::Domain),
            // Not rounded: 2 sqrt × = − 2 = gives 0 only so.
            Value::Double(x) => Ok(Value::Double(x.sqrt())),
            Value::Decimal(d) => d.sqrt().map(Value::Decimal),
        }
    }

    fn cube(&self, v: &Value, inv: bool) -> Result<Value, Fault> {
        if !inv {
            let square = self.arith(Op::Multiply, v, v)?;
            return self.arith(Op::Multiply, &square, v);
        }
        match v {
            Value::Double(x) => double(x.cbrt()),
            Value::Decimal(d) if d.is_negative() && self.era == Era::Win2000 => Err(Fault::Invalid),
            Value::Decimal(d) => {
                let third = Decimal::from_int(1).div(&Decimal::from_int(3))?;
                let r = d.abs().pow(&third)?;
                // A whole cube root comes out whole (27 gives 3, not 3.000…01).
                let near = round_to(&r, 40);
                let r = if near.mul(&near)?.mul(&near)? == d.abs() { near } else { r };
                Ok(Value::Decimal(if d.is_negative() { r.negate() } else { r }))
            }
        }
    }

    fn logarithm(&self, v: &Value, inv: bool, ten: bool) -> Result<Value, Fault> {
        match v {
            Value::Double(x) => {
                let r = match (inv, ten) {
                    (true, true) => 10f64.powf(*x),
                    (true, false) => x.exp(),
                    _ if *x <= 0.0 => return Err(Fault::Domain),
                    (false, true) => x.log10(),
                    (false, false) => x.ln(),
                };
                double(r)
            }
            Value::Decimal(d) => Ok(Value::Decimal(match (inv, ten) {
                (true, true) => Decimal::from_int(10).pow(d)?,
                (true, false) => d.exp_fn()?,
                (false, true) => {
                    // log of a power of ten is whole
                    let r = d.log10()?;
                    let near = round_to(&r, 40);
                    if near.is_integer() && Decimal::from_int(10).pow(&near)? == *d { near } else { r }
                }
                (false, false) => d.ln()?,
            })),
        }
    }

    fn factorial(&self, v: &Value) -> Result<Value, Fault> {
        match v {
            Value::Double(x) => {
                if *x < 0.0 {
                    return Err(Fault::Invalid);
                }
                // 95 has no factorial of a number that is not whole.
                if x.fract() != 0.0 || *x > 170.0 {
                    return Err(Fault::Overflow);
                }
                double((2..=*x as u32).fold(1.0, |p, i| p * f64::from(i)))
            }
            Value::Decimal(d) => d.factorial().map(Value::Decimal),
        }
    }

    fn not(&self, v: &Value) -> Result<Value, Fault> {
        match v {
            Value::Double(x) => {
                let x = x.trunc();
                if !(-2147483648.0..4294967296.0).contains(&x) {
                    return Err(Fault::Overflow);
                }
                Ok(Value::Double(f64::from(!(x as i64 as i32))))
            }
            Value::Decimal(d) => {
                let n = d.trunc().to_i128().ok_or(Fault::Overflow)?;
                Ok(Value::Decimal(Decimal::from_i128(!n)))
            }
        }
    }

    fn int_part(&self, v: &Value, frac: bool) -> Result<Value, Fault> {
        match v {
            Value::Double(x) => double(if frac { x.fract() } else { x.trunc() }),
            Value::Decimal(d) => Ok(Value::Decimal(if frac { d.sub(&d.trunc())? } else { d.trunc() })),
        }
    }

    fn dms(&self, v: &Value, inv: bool) -> Result<Value, Fault> {
        match v {
            Value::Double(x) => {
                let (neg, a) = (*x < 0.0, x.abs());
                let r = if inv {
                    let d = a.trunc();
                    let m = ((a - d) * 100.0 + 1e-9).trunc();
                    let s = (a - d) * 10000.0 - m * 100.0;
                    d + m / 60.0 + s / 3600.0
                } else {
                    let d = a.trunc();
                    let minutes = (a - d) * 60.0;
                    let m = (minutes + 1e-9).trunc();
                    let s = (minutes - m) * 60.0;
                    d + m / 100.0 + s / 10000.0
                };
                double(if neg { -r } else { r })
            }
            Value::Decimal(x) => {
                let a = x.abs();
                let d = a.trunc();
                let f = a.sub(&d)?;
                let n = Decimal::from_int;
                let r = if inv {
                    let m = f.mul(&n(100))?.trunc();
                    let s = f.mul(&n(10000))?.sub(&m.mul(&n(100))?)?;
                    d.add(&m.div(&n(60))?)?.add(&s.div(&n(3600))?)?
                } else {
                    let minutes = f.mul(&n(60))?;
                    let m = minutes.trunc();
                    let s = minutes.sub(&m)?.mul(&n(60))?;
                    d.add(&m.div(&n(100))?)?.add(&s.div(&n(10000))?)?
                };
                Ok(Value::Decimal(if x.is_negative() { r.negate() } else { r }))
            }
        }
    }

    fn trig_value(&self, key: Key, v: &Value, inv: bool, hyp: bool, angle: Angle) -> Result<Value, Fault> {
        match v {
            Value::Double(x) => trig95(key, *x, inv, hyp, angle),
            Value::Decimal(x) => trig_decimal(key, x, inv, hyp, angle).map(Value::Decimal),
        }
    }

    fn statistic(&self, key: Key, inv: bool) -> Result<Value, Fault> {
        let data = self.stats.as_deref().unwrap_or(&[]);
        let n = data.len();
        let zero = Engine::zero_of(self.era);
        let mut sum = zero.clone();
        let mut squares = zero.clone();
        for v in data {
            sum = self.arith(Op::Add, &sum, v)?;
            squares = self.arith(Op::Add, &squares, &self.arith(Op::Multiply, v, v)?)?;
        }
        let count = self.int(n as i128);
        match key {
            Key::Sum => Ok(if inv { squares } else { sum }),
            Key::Ave if n == 0 => Ok(zero),
            Key::Ave => self.arith(Op::Divide, if inv { &squares } else { &sum }, &count),
            _ => {
                // s: the deviation over n − 1 (Inv: over n).
                let divisor = if inv { n } else { n.saturating_sub(1) };
                if divisor == 0 {
                    return Ok(zero);
                }
                let mean = self.arith(Op::Divide, &sum, &count)?;
                let spread = self.arith(Op::Subtract, &squares, &self.arith(Op::Multiply, &mean, &sum)?)?;
                let variance = self.arith(Op::Divide, &spread, &self.int(divisor as i128))?;
                match variance {
                    Value::Decimal(d) if d.is_negative() => Ok(zero),
                    Value::Double(x) if x < 0.0 => Ok(zero),
                    v => self.sqrt(&v),
                }
            }
        }
    }
}

/// d rounded to n significant digits.
fn round_to(d: &Decimal, n: usize) -> Decimal {
    if d.is_zero() {
        return Decimal::zero();
    }
    let (digits, e) = d.round_to(n);
    let text: String = digits.iter().map(|d| char::from(b'0' + d)).collect();
    let v = Decimal::parse(&format!("0.{text}")).unwrap_or_else(Decimal::zero);
    let scaled = v.mul(&Decimal::from_int(10).powi(e + 1).unwrap_or_else(|_| Decimal::zero())).unwrap_or(v);
    if d.is_negative() { scaled.negate() } else { scaled }
}

/// A shift count: a whole number not too large.
fn shift(b: &Decimal) -> Result<i64, Fault> {
    let n = b.trunc().to_i128().ok_or(Fault::Invalid)?;
    if n.abs() > 100_000 {
        return Err(Fault::Invalid);
    }
    Ok(n as i64)
}

/// A whole number cut to `bits` (two's complement, shown unsigned).
fn wrap(d: &Decimal, bits: u32) -> Decimal {
    let modulus = Decimal::from_i128(1i128 << bits.min(100));
    if let Some(n) = d.to_i128() {
        let m = 1i128 << bits;
        return Decimal::from_i128(n.rem_euclid(m));
    }
    let q = d.div(&modulus).map(|q| q.floor()).unwrap_or_else(|_| Decimal::zero());
    d.sub(&q.mul(&modulus).unwrap_or_else(|_| Decimal::zero())).unwrap_or_else(|_| Decimal::zero())
}

fn decimal_of(x: f64) -> Decimal {
    Decimal::parse(&format!("{x:.17e}").pipe_plain()).unwrap_or_else(Decimal::zero)
}

/// x^y on Windows 95.
fn power95(a: f64, b: f64) -> Result<f64, Fault> {
    if a < 0.0 && b.fract() != 0.0 {
        return Err(Fault::Invalid);
    }
    if a == 0.0 && b < 0.0 {
        return Err(Fault::Invalid);
    }
    Ok(a.powf(b))
}

/// Windows 95's trigonometry in doubles; sin, cos and tan rounded to 15 places (sin 180° is 0
/// there, and tan 90° undefined).
fn trig95(key: Key, x: f64, inv: bool, hyp: bool, angle: Angle) -> Result<Value, Fault> {
    use std::f64::consts::PI;
    let to_rad = match angle {
        Angle::Degrees => PI / 180.0,
        Angle::Radians => 1.0,
        Angle::Grads => PI / 200.0,
    };
    let places = |v: f64| -> f64 { format!("{v:.15}").parse().unwrap_or(v) };
    let r = match (inv, hyp, key) {
        (false, false, Key::Sin) => places((x * to_rad).sin()),
        (false, false, Key::Cos) => places((x * to_rad).cos()),
        (false, false, _) => {
            let c = places((x * to_rad).cos());
            if c == 0.0 {
                return Err(Fault::Domain);
            }
            places((x * to_rad).sin()) / c
        }
        (true, false, Key::Sin) | (true, false, Key::Cos) => {
            if x.abs() > 1.0 {
                return Err(Fault::Invalid);
            }
            (if key == Key::Sin { x.asin() } else { x.acos() }) / to_rad
        }
        (true, false, _) => x.atan() / to_rad,
        (false, true, Key::Sin) => x.sinh(),
        (false, true, Key::Cos) => x.cosh(),
        (false, true, _) => x.tanh(),
        (true, true, Key::Sin) => x.asinh(),
        (true, true, Key::Cos) => {
            if x < 1.0 {
                return Err(Fault::Invalid);
            }
            x.acosh()
        }
        (true, true, _) => {
            if x.abs() >= 1.0 {
                return Err(Fault::Invalid);
            }
            x.atanh()
        }
    };
    double(r)
}

/// The trigonometric and hyperbolic functions in `Decimal`.
fn trig_decimal(key: Key, x: &Decimal, inv: bool, hyp: bool, angle: Angle) -> Result<Decimal, Fault> {
    let n = Decimal::from_int;
    let one = n(1);
    let pi = Decimal::pi();
    // (a full turn, a quarter turn) in the unit chosen
    let (turn, quarter) = match angle {
        Angle::Degrees => (n(360), n(90)),
        Angle::Grads => (n(400), n(100)),
        Angle::Radians => (pi.mul(&n(2))?, pi.div(&n(2))?),
    };
    let to_unit = |radians: Decimal| -> Result<Decimal, Fault> {
        match angle {
            Angle::Radians => Ok(radians),
            _ => radians.mul(&turn)?.div(&pi.mul(&n(2))?),
        }
    };
    if hyp {
        return match (inv, key) {
            (false, Key::Sin) | (false, Key::Cos) => {
                if x.abs().compare(&one) == Ordering::Less && key == Key::Sin {
                    // the series: no cancellation near 0
                    let x2 = x.mul(x)?;
                    let (mut sum, mut term) = (x.clone(), x.clone());
                    for k in 1..80 {
                        term = term.mul(&x2)?.div(&n((2 * k) * (2 * k + 1)))?;
                        sum = sum.add(&term)?;
                        if term.is_zero() || term.exponent() < sum.exponent() - 52 {
                            break;
                        }
                    }
                    return Ok(sum);
                }
                let e = x.exp_fn()?;
                let back = one.div(&e)?;
                let r = if key == Key::Sin { e.sub(&back)? } else { e.add(&back)? };
                r.div(&n(2))
            }
            (false, _) => {
                let sinh = trig_decimal(Key::Sin, x, false, true, angle)?;
                let cosh = trig_decimal(Key::Cos, x, false, true, angle)?;
                sinh.div(&cosh)
            }
            (true, Key::Sin) => {
                let a = x.abs();
                let r = a.add(&a.mul(&a)?.add(&one)?.sqrt()?)?.ln()?;
                Ok(if x.is_negative() { r.negate() } else { r })
            }
            (true, Key::Cos) => {
                if x.compare(&one) == Ordering::Less {
                    return Err(Fault::Invalid);
                }
                x.add(&x.mul(x)?.sub(&one)?.sqrt()?)?.ln()
            }
            (true, _) => {
                if x.abs().compare(&one) != Ordering::Less {
                    return Err(Fault::Invalid);
                }
                one.add(x)?.div(&one.sub(x)?)?.ln()?.div(&n(2))
            }
        };
    }
    if inv {
        return match key {
            Key::Sin => to_unit(x.asin()?),
            Key::Cos => {
                if x.abs().compare(&one) == Ordering::Greater {
                    return Err(Fault::Invalid);
                }
                to_unit(pi.div(&n(2))?.sub(&x.asin()?)?)
            }
            _ => to_unit(x.atan()?),
        };
    }
    // Reduced to one turn; whole quarters are exact (sin 180° is 0, tan 90° invalid).
    let r = x.sub(&x.div(&turn)?.floor().mul(&turn)?)?;
    let quarters = r.div(&quarter)?;
    if angle != Angle::Radians && quarters.is_integer() {
        let q = quarters.to_i128().unwrap_or(0) % 4;
        let (s, c) = [(0, 1), (1, 0), (0, -1), (-1, 0)][q as usize];
        return match key {
            Key::Sin => Ok(n(s)),
            Key::Cos => Ok(n(c)),
            _ if c == 0 => Err(Fault::Domain),
            _ => Ok(n(s * c)),
        };
    }
    let radians = match angle {
        Angle::Radians => r,
        _ => r.mul(&pi.mul(&n(2))?)?.div(&turn)?,
    };
    let (s, c) = radians.sin_cos()?;
    match key {
        Key::Sin => Ok(s),
        Key::Cos => Ok(c),
        _ if c.is_zero() => Err(Fault::Domain),
        _ => s.div(&c),
    }
}

/// A double result, rounded to 15 significant digits as Windows 95's results evidently were.
fn double(x: f64) -> Result<Value, Fault> {
    if !x.is_finite() {
        return Err(Fault::Overflow);
    }
    let rounded: f64 = format!("{x:.14e}").parse().unwrap_or(x);
    if !rounded.is_finite() {
        return Err(Fault::Overflow);
    }
    Ok(Value::Double(rounded))
}

trait PipePlain {
    fn pipe_plain(&self) -> String;
}

impl PipePlain for String {
    /// "1.25e-3" as plain digits, for `Decimal::parse`.
    fn pipe_plain(&self) -> String {
        let Some((mantissa, exp)) = self.split_once('e') else { return self.clone() };
        let exp: i64 = exp.parse().unwrap_or(0);
        let (neg, mantissa) = match mantissa.strip_prefix('-') {
            Some(m) => ("-", m),
            None => ("", mantissa),
        };
        let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
        let point = 1 + exp;
        let text = if point <= 0 {
            format!("0.{}{digits}", "0".repeat((-point) as usize))
        } else if point as usize >= digits.len() {
            format!("{digits}{}", "0".repeat(point as usize - digits.len()))
        } else {
            format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
        };
        format!("{neg}{text}")
    }
}

/// The error each Windows wrote in the display.
pub fn error_text(era: Era, fault: Fault) -> &'static str {
    match (era, fault) {
        (Era::Win95, Fault::DivideByZero | Fault::DivideByZeroNegative | Fault::Undefined) => "Cannot divide by zero.",
        (Era::Win95, Fault::Domain) => "Result of function is undefined.",
        (Era::Win95, Fault::Overflow) => "Result is too large.",
        (Era::Win98, Fault::DivideByZero) => "Error: Positive Infinity.",
        (Era::Win98, Fault::DivideByZeroNegative) => "Error: Negative Infinity.",
        (Era::Win2000, Fault::DivideByZero | Fault::DivideByZeroNegative) => "Cannot divide by zero.",
        (_, Fault::Undefined) => "Result of function is undefined.",
        (_, Fault::Domain | Fault::Overflow | Fault::Invalid) => "Invalid input for function.",
    }
}

/// The digits of a number rounded for the display, and the exponent of the first.
fn shown_digits(era: Era, v: &Value) -> Option<(bool, Vec<u8>, i64)> {
    match v {
        Value::Double(x) => {
            if *x == 0.0 {
                return None;
            }
            let text = format!("{:.12e}", x.abs());
            let (mantissa, exp) = text.split_once('e').expect("exponent form");
            let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
            while digits.last() == Some(&0) {
                digits.pop();
            }
            Some((*x < 0.0, digits, exp.parse::<i64>().unwrap_or(0)))
        }
        Value::Decimal(d) => {
            if d.is_zero() {
                return None;
            }
            let (digits, e) = d.round_to(if era == Era::Win98 { 33 } else { 32 });
            Some((d.is_negative(), digits, e))
        }
    }
}

/// A number in the exponent form F-E asks for: "1.23e+2", "5.e-1", 98's "0.e+0".
fn format_exponent(era: Era, v: &Value) -> String {
    let Some((neg, digits, e)) = shown_digits(era, v) else {
        return if era == Era::Win95 { "0." } else { "0.e+0" }.to_string();
    };
    let all: String = digits.iter().map(|d| char::from(b'0' + d)).collect();
    format!("{}{}.{}e{}{}", if neg { "-" } else { "" }, &all[..1], &all[1..], if e < 0 { '-' } else { '+' }, e.abs())
}

/// A number as each Windows wrote it: a point always ("5."), the exponent form past the
/// digits shown ("1.524157875019e+16", "1.e+308").
fn format_value(era: Era, v: &Value) -> String {
    let Some((neg, digits, e)) = shown_digits(era, v) else {
        return "0.".to_string();
    };
    let shown = match era {
        Era::Win95 => 13,
        Era::Win98 => 33,
        Era::Win2000 => 32,
    };
    let sign = if neg { "-" } else { "" };
    let text = |digits: &[u8]| digits.iter().map(|d| char::from(b'0' + d)).collect::<String>();
    // 98 and 2000 write a small number in the exponent form once its zeros and digits after
    // the point pass the digits shown (by one): 1e-12 stays plain, 6.98…e-31 does not.
    let crowded = era != Era::Win95 && e < 0 && (-e - 1) as usize + digits.len() > shown + 1;
    if e > -(shown as i64) && e < shown as i64 && !crowded {
        let all = text(&digits);
        let body = if e >= 0 {
            let int_len = e as usize + 1;
            if all.len() <= int_len {
                format!("{all}{}.", "0".repeat(int_len - all.len()))
            } else {
                format!("{}.{}", &all[..int_len], &all[int_len..])
            }
        } else {
            format!("0.{}{all}", "0".repeat((-e - 1) as usize))
        };
        return format!("{sign}{body}");
    }
    let exponent = format!("e{}{}", if e < 0 { '-' } else { '+' }, e.abs());
    let digits = match (era, v) {
        // Windows 98 shortens the digits until the text (without its sign) fits 37 characters.
        (Era::Win98, Value::Decimal(d)) => {
            let mut n = 33;
            loop {
                let (digits, _) = d.round_to(n);
                if n <= 1 || 1 + digits.len() + exponent.len() <= 37 {
                    break digits;
                }
                n -= 1;
            }
        }
        _ => digits,
    };
    let all = text(&digits);
    format!("{sign}{}.{}{exponent}", &all[..1], &all[1..])
}

/// Windows 2000's digit grouping: a comma every three digits before the point.
pub fn group(text: &str) -> String {
    if text.contains('e') {
        return text.to_string();
    }
    let (sign, body) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let (int, rest) = match body.find('.') {
        Some(at) => body.split_at(at),
        None => (body, ""),
    };
    let mut out = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{sign}{out}{rest}")
}

/// Windows 2000's grouping in Hex and Bin: a space every four digits.
fn group_by_four(text: &str) -> String {
    let (sign, body) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let mut out = String::new();
    for (i, ch) in body.chars().enumerate() {
        if i > 0 && (body.len() - i).is_multiple_of(4) {
            out.push(' ');
        }
        out.push(ch);
    }
    format!("{sign}{out}")
}

/// The key a typed character is in the Standard view (the keyboard and Paste). Letters are
/// lowercase here.
pub fn key_for_char(ch: char) -> Option<Key> {
    Some(match ch {
        '0'..='9' => Key::Digit(ch as u8 - b'0'),
        '.' | ',' => Key::Point,
        '+' => Key::Add,
        '-' => Key::Subtract,
        '*' => Key::Multiply,
        '/' => Key::Divide,
        '=' | '\r' => Key::Equals,
        '@' => Key::Sqrt,
        '%' => Key::Percent,
        'r' | 'R' => Key::Reciprocal,
        _ => return None,
    })
}

/// The key a typed character is in the Scientific view (the Help's list).
pub fn key_for_char_scientific(ch: char) -> Option<Key> {
    Some(match ch.to_ascii_lowercase() {
        'a'..='f' => Key::Digit(ch.to_ascii_lowercase() as u8 - b'a' + 10),
        '@' => Key::Square,
        '%' => Key::Mod,
        '(' => Key::Open,
        ')' => Key::Close,
        '&' => Key::And,
        '|' => Key::Or,
        '^' => Key::Xor,
        '<' => Key::Lsh,
        '~' => Key::Not,
        ';' => Key::Int,
        's' => Key::Sin,
        'o' => Key::Cos,
        't' => Key::Tan,
        'n' => Key::Ln,
        'l' => Key::Log,
        'y' => Key::Power,
        '#' => Key::Cube,
        '!' => Key::Factorial,
        'p' => Key::Pi,
        'm' => Key::Dms,
        'v' => Key::FixedExp,
        'x' => Key::Exp,
        'i' => Key::Inv,
        'h' => Key::Hyp,
        _ => return key_for_char(ch),
    })
}

/// What pasted text types, key by key, as the real one did: each character as its key, and
/// the Help's sequences ":c" (MC), ":m" (MS), ":p" (M+), ":q" (C), ":r" (MR); in the Scientific
/// view also ":e" (Exp) and "\" (Dat).
pub fn paste_keys(text: &str) -> Vec<Key> {
    paste_keys_in(text, false)
}

pub fn paste_keys_in(text: &str, scientific: bool) -> Vec<Key> {
    let mut keys = Vec::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == ':' {
            let code = chars.next().map(|c| c.to_ascii_lowercase());
            keys.extend(match code {
                Some('c') => Some(Key::MemoryClear),
                Some('m') => Some(Key::MemoryStore),
                Some('p') => Some(Key::MemoryAdd),
                Some('q') => Some(Key::Clear),
                Some('r') => Some(Key::MemoryRecall),
                Some('e') if scientific => Some(Key::Exp),
                _ => None,
            });
            continue;
        }
        if scientific {
            if ch == '\\' {
                keys.push(Key::Dat);
                continue;
            }
            keys.extend(key_for_char_scientific(ch));
        } else {
            keys.extend(key_for_char(ch));
        }
    }
    keys
}

#[cfg(test)]
mod tests;

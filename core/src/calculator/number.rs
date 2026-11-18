//! The decimal numbers of the Windows 98 and 2000 Calculators, which kept "at least 32 digits"
//! (their Help) where Windows 95's had the C runtime's double. Bevel95's own type: a sign, up to
//! `PRECISION` significant decimal digits and an exponent that does not run out (the real ones
//! squared their way past 10^132571 without an error). Every result is rounded half up to
//! `PRECISION` digits: 16 guard digits beyond what the display shows, so that a value typed or
//! computed exactly (0.1 + 0.2, 123456789^2, 1/3 × 3) shows exactly, and twelve squarings in a
//! row still show the 32 digits the real one showed.

use std::cmp::Ordering;

/// Significant digits every result keeps.
pub const PRECISION: usize = 48;

/// Why an operation has no result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Dividing by zero (1/x of zero too).
    DivideByZero,
    /// Dividing a negative number by zero (Windows 98 says so: "Negative Infinity").
    DivideByZeroNegative,
    /// 0 / 0 and x Mod 0 (98 and 2000: "Result of function is undefined.").
    Undefined,
    /// No result exists (the square root of a negative number, log 0, tan 90°): Windows 95
    /// called it undefined, 98 and 2000 invalid input.
    Domain,
    /// Invalid input in all three (the arc sine of 2, a negative factorial).
    Invalid,
    /// Too large to keep.
    Overflow,
}

/// value = (-1)^neg × 0.d1 d2 d3 … × 10^exp. `digits` has no leading or trailing zeros; zero has
/// no digits (and is never negative).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decimal {
    neg: bool,
    digits: Vec<u8>,
    exp: i64,
}

/// Exponents beyond this are an overflow (the real ones kept theirs in 32 bits).
const MAX_EXP: i64 = 1 << 31;

impl Decimal {
    pub fn zero() -> Decimal {
        Decimal { neg: false, digits: Vec::new(), exp: 0 }
    }

    pub fn from_int(n: i64) -> Decimal {
        Decimal::parse(&n.to_string()).unwrap_or_else(Decimal::zero)
    }

    /// From typed text: an optional "-", digits, at most one ".".
    pub fn parse(text: &str) -> Option<Decimal> {
        let (neg, body) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let mut digits = Vec::new();
        let mut point: Option<usize> = None;
        for ch in body.chars() {
            match ch {
                '0'..='9' => digits.push(ch as u8 - b'0'),
                '.' if point.is_none() => point = Some(digits.len()),
                _ => return None,
            }
        }
        let exp = point.unwrap_or(digits.len()) as i64;
        Some(Decimal::make(neg, digits, exp))
    }

    /// Normalizes: strips zeros, rounds half up to `PRECISION` digits.
    fn make(neg: bool, mut digits: Vec<u8>, mut exp: i64) -> Decimal {
        let lead = digits.iter().take_while(|&&d| d == 0).count();
        digits.drain(..lead);
        exp -= lead as i64;
        if digits.len() > PRECISION {
            let up = digits[PRECISION] >= 5;
            digits.truncate(PRECISION);
            if up {
                let mut i = PRECISION;
                loop {
                    if i == 0 {
                        digits.insert(0, 1);
                        digits.truncate(PRECISION);
                        exp += 1;
                        break;
                    }
                    i -= 1;
                    if digits[i] == 9 {
                        digits[i] = 0;
                    } else {
                        digits[i] += 1;
                        break;
                    }
                }
            }
        }
        while digits.last() == Some(&0) {
            digits.pop();
        }
        if digits.is_empty() {
            return Decimal::zero();
        }
        Decimal { neg, digits, exp }
    }

    pub fn is_zero(&self) -> bool {
        self.digits.is_empty()
    }

    pub fn is_negative(&self) -> bool {
        self.neg
    }

    pub fn negate(&self) -> Decimal {
        Decimal { neg: !self.neg && !self.is_zero(), ..self.clone() }
    }

    fn check(self) -> Result<Decimal, Fault> {
        if self.exp.abs() > MAX_EXP { Err(Fault::Overflow) } else { Ok(self) }
    }

    fn cmp_abs(&self, other: &Decimal) -> Ordering {
        match (self.is_zero(), other.is_zero()) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
        self.exp.cmp(&other.exp).then_with(|| {
            let n = self.digits.len().max(other.digits.len());
            let at = |d: &Decimal, i: usize| d.digits.get(i).copied().unwrap_or(0);
            (0..n).map(|i| at(self, i).cmp(&at(other, i))).find(|o| o.is_ne()).unwrap_or(Ordering::Equal)
        })
    }

    pub fn add(&self, other: &Decimal) -> Result<Decimal, Fault> {
        if self.is_zero() {
            return Ok(other.clone());
        }
        if other.is_zero() {
            return Ok(self.clone());
        }
        let (big, small) = if self.cmp_abs(other) == Ordering::Less { (other, self) } else { (self, other) };
        // A number more than the kept digits smaller cannot change the rounded result.
        if big.exp - small.exp > PRECISION as i64 + 2 {
            return Ok(big.clone());
        }
        let lo = (big.exp - big.digits.len() as i64).min(small.exp - small.digits.len() as i64);
        let width = (big.exp - lo) as usize + 1;
        let place = |d: &Decimal| {
            let mut v = vec![0i32; width];
            for (i, &digit) in d.digits.iter().enumerate() {
                v[1 + (big.exp - d.exp) as usize + i] = i32::from(digit);
            }
            v
        };
        let (a, b) = (place(big), place(small));
        let mut out = vec![0u8; width];
        let mut carry = 0;
        let sign = if big.neg == small.neg { 1 } else { -1 };
        for i in (0..width).rev() {
            let mut v = a[i] + sign * b[i] + carry;
            carry = 0;
            if v >= 10 {
                v -= 10;
                carry = 1;
            } else if v < 0 {
                v += 10;
                carry = -1;
            }
            out[i] = v as u8;
        }
        Decimal::make(big.neg, out, big.exp + 1).check()
    }

    pub fn sub(&self, other: &Decimal) -> Result<Decimal, Fault> {
        self.add(&other.negate())
    }

    pub fn mul(&self, other: &Decimal) -> Result<Decimal, Fault> {
        if self.is_zero() || other.is_zero() {
            return Ok(Decimal::zero());
        }
        let mut acc = vec![0u32; self.digits.len() + other.digits.len()];
        for (i, &a) in self.digits.iter().enumerate() {
            for (j, &b) in other.digits.iter().enumerate() {
                acc[i + j + 1] += u32::from(a) * u32::from(b);
            }
        }
        for i in (1..acc.len()).rev() {
            let carry = acc[i] / 10;
            acc[i] %= 10;
            acc[i - 1] += carry;
        }
        let digits = acc.into_iter().map(|d| d as u8).collect();
        Decimal::make(self.neg != other.neg, digits, self.exp + other.exp).check()
    }

    pub fn div(&self, other: &Decimal) -> Result<Decimal, Fault> {
        if other.is_zero() {
            return Err(Fault::DivideByZero);
        }
        if self.is_zero() {
            return Ok(Decimal::zero());
        }
        // Long division of the digit strings, PRECISION + 2 quotient digits past the first.
        let divisor: Vec<u8> = other.digits.clone();
        let mut rem: Vec<u8> = Vec::new();
        let mut quotient = Vec::new();
        let wanted = PRECISION + 2 + divisor.len();
        let mut i = 0;
        while quotient.len() < wanted {
            rem.push(self.digits.get(i).copied().unwrap_or(0));
            i += 1;
            trim(&mut rem);
            let mut q = 0;
            while !less(&rem, &divisor) {
                subtract(&mut rem, &divisor);
                q += 1;
            }
            quotient.push(q);
        }
        // With A, B the digit strings (la, lb digits) and i digits brought down, `quotient` (i
        // digits, leading zeros included) is floor(A × 10^(i - la) / B), so the value is
        // quotient × 10^(ea - eb + lb - i): as 0.q1q2… that is 10^(ea - eb + lb).
        debug_assert_eq!(quotient.len(), i);
        let exp = self.exp - other.exp + divisor.len() as i64;
        Decimal::make(self.neg != other.neg, quotient, exp).check()
    }

    pub fn sqrt(&self) -> Result<Decimal, Fault> {
        if self.neg {
            return Err(Fault::Domain);
        }
        if self.is_zero() {
            return Ok(Decimal::zero());
        }
        // A first guess from the leading digits, then Newton's x = (x + a/x) / 2 until it stands.
        let even = self.exp - self.exp.rem_euclid(2);
        let lead: f64 = self.digits.iter().take(17).enumerate().map(|(i, &d)| f64::from(d) * 10f64.powi(-(i as i32) - 1)).sum();
        let scaled = lead * 10f64.powi((self.exp - even) as i32);
        let first = format!("{:.15e}", scaled.sqrt());
        let mut x = parse_e(&first).ok_or(Fault::Domain)?;
        x.exp += even / 2;
        let half = Decimal::parse("0.5").expect("a number");
        for _ in 0..12 {
            let next = x.add(&self.div(&x)?)?.mul(&half)?;
            if next == x {
                break;
            }
            x = next;
        }
        Ok(x)
    }

    /// The digits rounded half up to `n` significant ones, trailing zeros dropped, and the
    /// exponent of the first: value ≈ d1.d2d3… × 10^e.
    pub fn round_to(&self, n: usize) -> (Vec<u8>, i64) {
        if self.is_zero() {
            return (Vec::new(), 0);
        }
        let mut digits = self.digits.clone();
        let mut exp = self.exp;
        if digits.len() > n {
            let up = digits[n] >= 5;
            digits.truncate(n);
            if up {
                let mut i = n;
                loop {
                    if i == 0 {
                        digits.insert(0, 1);
                        digits.truncate(n);
                        exp += 1;
                        break;
                    }
                    i -= 1;
                    if digits[i] == 9 {
                        digits[i] = 0;
                    } else {
                        digits[i] += 1;
                        break;
                    }
                }
            }
        }
        while digits.last() == Some(&0) {
            digits.pop();
        }
        (digits, exp - 1)
    }
}

/// Constants to 64 digits (rounded to `PRECISION` when parsed).
const PI: &str = "3.141592653589793238462643383279502884197169399375105820974944592";
const LN2: &str = "0.6931471805599453094172321214581765680755001343602552541206800094";
const LN10: &str = "2.302585092994045684017991454684364207601101488628772976033327901";

/// The Scientific view's functions. Each keeps `PRECISION` digits (a few of the last ones may
/// be off; the display shows 32 or 33).
impl Decimal {
    pub fn pi() -> Decimal {
        Decimal::parse(PI).expect("a number")
    }

    fn constant(text: &str) -> Decimal {
        Decimal::parse(text).expect("a number")
    }

    fn one() -> Decimal {
        Decimal::from_int(1)
    }

    /// From a whole number.
    pub fn from_i128(n: i128) -> Decimal {
        Decimal::parse(&n.to_string()).unwrap_or_else(Decimal::zero)
    }

    /// The decimal exponent: value = 0.d1d2… × 10^exp.
    pub fn exponent(&self) -> i64 {
        self.exp
    }

    pub fn abs(&self) -> Decimal {
        Decimal { neg: false, ..self.clone() }
    }

    pub fn compare(&self, other: &Decimal) -> Ordering {
        match (self.neg, other.neg) {
            (false, true) => {
                if self.is_zero() && other.is_zero() { Ordering::Equal } else { Ordering::Greater }
            }
            (true, false) => Ordering::Less,
            (false, false) => self.cmp_abs(other),
            (true, true) => other.cmp_abs(self),
        }
    }

    /// Whether it has no digits after the point.
    pub fn is_integer(&self) -> bool {
        self.digits.len() as i64 <= self.exp
    }

    /// The whole part (towards zero).
    pub fn trunc(&self) -> Decimal {
        if self.is_integer() {
            return self.clone();
        }
        if self.exp <= 0 {
            return Decimal::zero();
        }
        Decimal::make(self.neg, self.digits[..self.exp as usize].to_vec(), self.exp)
    }

    /// The whole part (towards minus infinity).
    pub fn floor(&self) -> Decimal {
        let t = self.trunc();
        if self.neg && t != *self { t.sub(&Decimal::one()).unwrap_or(t) } else { t }
    }

    /// As a whole number, if it is one and fits.
    pub fn to_i128(&self) -> Option<i128> {
        if !self.is_integer() || self.exp > 38 {
            return None;
        }
        let mut n: i128 = 0;
        for i in 0..self.exp as usize {
            n = n.checked_mul(10)?.checked_add(i128::from(self.digits.get(i).copied().unwrap_or(0)))?;
        }
        Some(if self.neg { -n } else { n })
    }

    /// Half of it, a third of it: divided by a small whole number.
    fn div_int(&self, n: i64) -> Decimal {
        self.div(&Decimal::from_int(n)).unwrap_or_else(|_| Decimal::zero())
    }

    /// x^n for a whole n (by squaring).
    pub fn powi(&self, n: i64) -> Result<Decimal, Fault> {
        if n < 0 {
            return Decimal::one().div(&self.powi(-n)?);
        }
        let mut result = Decimal::one();
        let mut base = self.clone();
        let mut n = n as u64;
        while n > 0 {
            if n & 1 == 1 {
                result = result.mul(&base)?;
            }
            n >>= 1;
            if n > 0 {
                base = base.mul(&base)?;
            }
        }
        Ok(result)
    }

    /// Whether it is smaller than 10^-n in size.
    fn below(&self, n: i64) -> bool {
        self.is_zero() || self.exp < -n
    }

    /// e^x.
    pub fn exp_fn(&self) -> Result<Decimal, Fault> {
        if self.is_zero() {
            return Ok(Decimal::one());
        }
        if self.exp > 12 {
            return if self.neg { Ok(Decimal::zero()) } else { Err(Fault::Overflow) };
        }
        let ln2 = Decimal::constant(LN2);
        let k = self.div(&ln2)?.add(&Decimal::parse("0.5").expect("a number"))?.floor();
        let r = self.sub(&k.mul(&ln2)?)?;
        // r/1024, the series, then squared ten times.
        let small = r.div_int(1024);
        let mut sum = Decimal::one();
        let mut term = Decimal::one();
        for i in 1..60 {
            term = term.mul(&small)?.div_int(i);
            sum = sum.add(&term)?;
            if term.below(PRECISION as i64 + 4) {
                break;
            }
        }
        for _ in 0..10 {
            sum = sum.mul(&sum)?;
        }
        let k = k.to_i128().ok_or(Fault::Overflow)? as i64;
        sum.mul(&Decimal::from_int(2).powi(k)?)?.check()
    }

    /// The natural logarithm.
    pub fn ln(&self) -> Result<Decimal, Fault> {
        if self.neg || self.is_zero() {
            return Err(Fault::Domain);
        }
        if *self == Decimal::one() {
            return Ok(Decimal::zero());
        }
        // x = m × 10^e with m in [0.1, 1), then m × 2^j in [0.7, 1.4).
        let e = self.exp;
        let mut t = Decimal { neg: false, digits: self.digits.clone(), exp: 0 };
        let mut j = 0;
        while t.compare(&Decimal::parse("0.7").expect("a number")) == Ordering::Less {
            t = t.mul(&Decimal::from_int(2))?;
            j += 1;
        }
        let z = t.sub(&Decimal::one())?.div(&t.add(&Decimal::one())?)?;
        let z2 = z.mul(&z)?;
        let mut sum = z.clone();
        let mut power = z;
        for k in (3..400).step_by(2) {
            power = power.mul(&z2)?;
            let term = power.div_int(k);
            sum = sum.add(&term)?;
            if term.below(PRECISION as i64 + 4) {
                break;
            }
        }
        let ln_t = sum.mul(&Decimal::from_int(2))?;
        ln_t.sub(&Decimal::constant(LN2).mul(&Decimal::from_int(j))?)?.add(&Decimal::constant(LN10).mul(&Decimal::from_int(e))?)
    }

    pub fn log10(&self) -> Result<Decimal, Fault> {
        self.ln()?.div(&Decimal::constant(LN10))
    }

    /// sin and cos of x radians.
    pub fn sin_cos(&self) -> Result<(Decimal, Decimal), Fault> {
        if self.exp > 40 {
            return Err(Fault::Invalid);
        }
        let half_pi = Decimal::pi().div_int(2);
        let q = self.div(&half_pi)?.floor();
        let r = self.sub(&q.mul(&half_pi)?)?;
        let quadrant = q.sub(&q.div_int(4).floor().mul(&Decimal::from_int(4))?)?.to_i128().unwrap_or(0);
        let (mut s, mut c) = (Decimal::zero(), Decimal::one());
        // the series of sin and cos at r (0 <= r < pi/2)
        let r2 = r.mul(&r)?;
        let mut term = r.clone();
        s = s.add(&term)?;
        for k in 1..80 {
            term = term.mul(&r2)?.div_int((2 * k) * (2 * k + 1)).negate();
            s = s.add(&term)?;
            if term.below(PRECISION as i64 + 4) {
                break;
            }
        }
        let mut term = Decimal::one();
        for k in 1..80 {
            term = term.mul(&r2)?.div_int((2 * k - 1) * (2 * k)).negate();
            c = c.add(&term)?;
            if term.below(PRECISION as i64 + 4) {
                break;
            }
        }
        let (s, c) = match quadrant {
            0 => (s, c),
            1 => (c, s.negate()),
            2 => (s.negate(), c.negate()),
            _ => (c.negate(), s),
        };
        // What is left of a multiple of pi/2 is no number (sin pi is 0).
        let snap = |v: Decimal| if v.below(PRECISION as i64 - 6) { Decimal::zero() } else { v };
        Ok((snap(s), snap(c)))
    }

    /// The arc tangent, in radians.
    pub fn atan(&self) -> Result<Decimal, Fault> {
        if self.is_zero() {
            return Ok(Decimal::zero());
        }
        let one = Decimal::one();
        if self.cmp_abs(&one) == Ordering::Greater {
            let inner = one.div(self)?.atan()?;
            let half_pi = Decimal::pi().div_int(2);
            let half = if self.neg { half_pi.negate() } else { half_pi };
            return half.sub(&inner);
        }
        // atan x = 2 atan(x / (1 + sqrt(1 + x²))), three times
        let mut x = self.clone();
        for _ in 0..3 {
            x = x.div(&one.add(&one.add(&x.mul(&x)?)?.sqrt()?)?)?;
        }
        let x2 = x.mul(&x)?;
        let mut sum = x.clone();
        let mut power = x;
        for k in 1..200 {
            power = power.mul(&x2)?.negate();
            let term = power.div_int(2 * k + 1);
            sum = sum.add(&term)?;
            if term.below(PRECISION as i64 + 4) {
                break;
            }
        }
        sum.mul(&Decimal::from_int(8))
    }

    /// The arc sine, in radians (|x| <= 1).
    pub fn asin(&self) -> Result<Decimal, Fault> {
        let one = Decimal::one();
        match self.cmp_abs(&one) {
            Ordering::Greater => Err(Fault::Invalid),
            Ordering::Equal => {
                let half_pi = Decimal::pi().div_int(2);
                Ok(if self.neg { half_pi.negate() } else { half_pi })
            }
            Ordering::Less => self.div(&one.sub(&self.mul(self)?)?.sqrt()?)?.atan(),
        }
    }

    /// x^y for a y that need not be whole.
    pub fn pow(&self, y: &Decimal) -> Result<Decimal, Fault> {
        if let Some(n) = y.to_i128().filter(|n| n.abs() < 1 << 31) {
            if self.is_zero() && n < 0 {
                return Err(Fault::Invalid);
            }
            return self.powi(n as i64);
        }
        if self.is_zero() {
            return if y.neg { Err(Fault::Invalid) } else { Ok(Decimal::zero()) };
        }
        if self.neg {
            return Err(Fault::Invalid);
        }
        y.mul(&self.ln()?)?.exp_fn()
    }

    /// ln Γ(z) for z >= 100 (Stirling's series).
    fn ln_gamma_large(z: &Decimal) -> Result<Decimal, Fault> {
        #[rustfmt::skip]
        const BERNOULLI: [(&str, &str); 15] = [
            ("1", "6"), ("-1", "30"), ("1", "42"), ("-1", "30"), ("5", "66"), ("-691", "2730"),
            ("7", "6"), ("-3617", "510"), ("43867", "798"), ("-174611", "330"),
            ("854513", "138"), ("-236364091", "2730"), ("8553103", "6"),
            ("-23749461029", "870"), ("8615841276005", "14322"),
        ];
        let half = Decimal::parse("0.5").expect("a number");
        let two_pi = Decimal::pi().mul(&Decimal::from_int(2))?;
        let mut sum = z.sub(&half)?.mul(&z.ln()?)?.sub(z)?.add(&half.mul(&two_pi.ln()?)?)?;
        let z2 = z.mul(z)?;
        let mut power = z.clone();
        for (k, (num, den)) in BERNOULLI.iter().enumerate() {
            let k = k as i64 + 1;
            let b = Decimal::parse(num).expect("a number").div(&Decimal::parse(den).expect("a number"))?;
            let term = b.div(&Decimal::from_int(2 * k * (2 * k - 1)))?.div(&power)?;
            sum = sum.add(&term)?;
            power = power.mul(&z2)?;
        }
        Ok(sum)
    }

    /// n! (Γ(n + 1) when n is not whole).
    pub fn factorial(&self) -> Result<Decimal, Fault> {
        if let Some(n) = self.to_i128() {
            if n < 0 {
                return Err(Fault::Invalid);
            }
            if n <= 300 {
                let mut product = Decimal::one();
                for i in 2..=n as i64 {
                    product = product.mul(&Decimal::from_int(i))?;
                }
                return Ok(product);
            }
        }
        // Γ(x) = Γ(x + N) / (x (x + 1) … (x + N − 1)), with x + N past 100.
        let x = self.add(&Decimal::one())?;
        let mut z = x.clone();
        let mut product = Decimal::one();
        while z.compare(&Decimal::from_int(100)) == Ordering::Less {
            if z.is_zero() {
                return Err(Fault::Invalid);
            }
            product = product.mul(&z)?;
            z = z.add(&Decimal::one())?;
        }
        Decimal::ln_gamma_large(&z)?.exp_fn()?.div(&product)
    }
}

/// Text from Rust's `{:e}` formatting: "1.414e0", "2.5e-3".
fn parse_e(text: &str) -> Option<Decimal> {
    let (mantissa, exp) = text.split_once('e')?;
    let mut d = Decimal::parse(mantissa)?;
    if !d.is_zero() {
        d.exp += exp.parse::<i64>().ok()?;
    }
    Some(d)
}

fn trim(v: &mut Vec<u8>) {
    let lead = v.iter().take_while(|&&d| d == 0).count();
    v.drain(..lead);
}

/// a < b for digit strings without leading zeros.
fn less(a: &[u8], b: &[u8]) -> bool {
    a.len() < b.len() || (a.len() == b.len() && a < b)
}

/// a -= b, for a ≥ b (digit strings without leading zeros).
fn subtract(a: &mut Vec<u8>, b: &[u8]) {
    let offset = a.len() - b.len();
    let mut borrow = 0i8;
    for i in (0..a.len()).rev() {
        let sub = if i >= offset { b[i - offset] as i8 } else { 0 };
        let mut v = a[i] as i8 - sub - borrow;
        borrow = 0;
        if v < 0 {
            v += 10;
            borrow = 1;
        }
        a[i] = v as u8;
    }
    trim(a);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Decimal {
        Decimal::parse(text).unwrap()
    }

    fn show(x: &Decimal, n: usize) -> String {
        let (digits, e) = x.round_to(n);
        let text: String = digits.iter().map(|d| char::from(b'0' + d)).collect();
        format!("{}{text}e{e}", if x.is_negative() { "-" } else { "" })
    }

    #[test]
    fn adds_and_subtracts_exactly() {
        assert_eq!(d("0.1").add(&d("0.2")).unwrap(), d("0.3"));
        assert!(d("0.1").add(&d("0.2")).unwrap().sub(&d("0.3")).unwrap().is_zero());
        assert_eq!(d("2").sub(&d("5")).unwrap(), d("-3"));
        assert_eq!(d("999").add(&d("1")).unwrap(), d("1000"));
        assert_eq!(d("1000").sub(&d("0.001")).unwrap(), d("999.999"));
        assert_eq!(d("-4").add(&d("4")).unwrap(), Decimal::zero());
        // far apart: the small one is lost in the rounding
        let big = d("1").mul(&Decimal { neg: false, digits: vec![1], exp: 100 }).unwrap();
        assert_eq!(big.add(&d("1")).unwrap(), big);
    }

    #[test]
    fn multiplies_and_divides() {
        assert_eq!(d("123456789").mul(&d("123456789")).unwrap(), d("15241578750190521"));
        assert_eq!(d("1").div(&d("4")).unwrap(), d("0.25"));
        assert_eq!(d("10").div(&d("4")).unwrap(), d("2.5"));
        assert_eq!(d("0.000001").div(&d("1000000")).unwrap(), d("0.000000000001"));
        assert_eq!(d("16").div(&d("2")).unwrap(), d("8"));
        assert_eq!(d("-7").div(&d("0.5")).unwrap(), d("-14"));
        let third = d("1").div(&d("3")).unwrap();
        assert_eq!(show(&third, 32), "33333333333333333333333333333333e-1");
        assert_eq!(show(&third.mul(&d("3")).unwrap(), 32), "1e0");
        assert_eq!(show(&d("2").div(&d("3")).unwrap(), 5), "66667e-1");
        assert_eq!(d("1").div(&Decimal::zero()), Err(Fault::DivideByZero));
    }

    #[test]
    fn square_roots() {
        assert_eq!(d("9").sqrt().unwrap(), d("3"));
        assert_eq!(d("0.25").sqrt().unwrap(), d("0.5"));
        assert_eq!(show(&d("2").sqrt().unwrap(), 33), "14142135623730950488016887242097e0");
        assert_eq!(show(&d("20").sqrt().unwrap(), 10), "4472135955e0");
        assert_eq!(show(&d("0.002").sqrt().unwrap(), 10), "4472135955e-2");
        assert_eq!(d("-4").sqrt(), Err(Fault::Domain));
    }

    #[test]
    fn rounds_for_the_display() {
        assert_eq!(show(&d("9.9999"), 3), "1e1");
        assert_eq!(show(&d("0.00012345"), 4), "1235e-4");
        assert_eq!(show(&d("-2.5"), 1), "-3e0");
        let mut x = d("123456789").mul(&d("123456789")).unwrap();
        for _ in 0..12 {
            x = x.mul(&x).unwrap();
        }
        // 123456789^8192, as Windows 2000 showed it.
        assert_eq!(show(&x, 32), "49056094599201101450033992009915e66285");
    }
}

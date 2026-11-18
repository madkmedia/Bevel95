//! The one file a stranger downloads (decisions.md, "Bevel95 Setup"): the bar's own program with
//! its five companions appended, each with its SHA-256 fingerprint. Windows runs the front part
//! (the bar, a complete program as built) and never looks at what follows it, so the one file is
//! the bar itself: started, it shows Bevel95 Setup, which unpacks the others from its own file.
//!
//! The layout, written by `tools\release.ps1` (through `core/examples/pack.rs`, which calls
//! `pack`):
//! - the bar's program, unchanged, from byte 0;
//! - each companion's program, one after the other;
//! - the index: lines of text, `Bevel95 package 1`, then one `file <name> <offset> <length>
//!   <sha-256>` per program (the bar first, at offset 0; companions as `programs/<file>`);
//! - the trailer, the last 32 bytes: `MAGIC`, then the index's offset and length (8 bytes each,
//!   little-endian).
//!
//! Also here: the build stamp every Bevel95 program carries (`stamp_text`, `find_stamp`), which
//! says which of two copies is newer without running either.

/// The first 16 bytes of the trailer.
pub const MAGIC: &[u8; 16] = b"Bevel95 package\0";
/// The trailer's length: the magic and two 8-byte numbers.
pub const TRAILER: usize = 32;
/// The index's first line.
const HEADER: &str = "Bevel95 package 1";
/// The bar's name in the index (and in the installed folder).
pub const BAR: &str = "bevel95.exe";

/// One program in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `bevel95.exe`, or `programs/<file>` for a companion.
    pub name: String,
    pub offset: u64,
    pub len: u64,
    pub sha256: [u8; 32],
}

/// The bar's bytes followed by `programs` (each `(file name, bytes)`, installed as
/// `programs\<file name>`), the index and the trailer.
pub fn pack(bar: &[u8], programs: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = bar.to_vec();
    let mut index = format!("{HEADER}\nfile {BAR} 0 {} {}\n", bar.len(), hex(&sha256(bar)));
    for (file, bytes) in programs {
        index.push_str(&format!("file programs/{file} {} {} {}\n", out.len(), bytes.len(), hex(&sha256(bytes))));
        out.extend_from_slice(bytes);
    }
    let index_offset = out.len() as u64;
    out.extend_from_slice(index.as_bytes());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&index_offset.to_le_bytes());
    out.extend_from_slice(&(index.len() as u64).to_le_bytes());
    out
}

/// Whether the file's last bytes (at least `TRAILER` of them) are a package's trailer.
pub fn is_packed(tail: &[u8]) -> bool {
    tail.len() >= TRAILER && &tail[tail.len() - TRAILER..tail.len() - 16] == MAGIC
}

/// The programs a package lists, checked against the file's size; `Ok(None)` for a file that is
/// no package (a program as built). Fingerprints are not checked here: `unpack` does.
pub fn entries(data: &[u8]) -> Result<Option<Vec<Entry>>, String> {
    if !is_packed(data) {
        return Ok(None);
    }
    let tail = &data[data.len() - 16..];
    let number = |b: &[u8]| u64::from_le_bytes(b.try_into().expect("8 bytes"));
    let (index_offset, index_len) = (number(&tail[..8]), number(&tail[8..]));
    let index_end = index_offset.checked_add(index_len).ok_or("the index's place is impossible")?;
    if index_end != (data.len() - TRAILER) as u64 {
        return Err("the index is not where the trailer says".into());
    }
    let index = std::str::from_utf8(&data[index_offset as usize..index_end as usize]).map_err(|_| "the index is not text")?;
    let mut lines = index.lines();
    if lines.next() != Some(HEADER) {
        return Err("the index is of an unknown kind".into());
    }
    let mut out: Vec<Entry> = Vec::new();
    for line in lines {
        let fields: Vec<&str> = line.split(' ').collect();
        let [kind, name, offset, len, sha] = fields[..] else { return Err("an index line is not understood".into()) };
        let (Ok(offset), Ok(len)) = (offset.parse::<u64>(), len.parse::<u64>()) else {
            return Err("an index line has a bad number".into());
        };
        let sha256 = unhex(sha).ok_or("an index line has a bad fingerprint")?;
        if kind != "file" || !good_name(name) {
            return Err("an index line names an unexpected file".into());
        }
        if offset.checked_add(len).is_none_or(|end| end > index_offset) {
            return Err("a program lies outside the file".into());
        }
        if out.iter().any(|e| e.name.eq_ignore_ascii_case(name)) {
            return Err("a program is listed twice".into());
        }
        out.push(Entry { name: name.to_string(), offset, len, sha256 });
    }
    match out.first() {
        Some(first) if first.name == BAR && first.offset == 0 => Ok(Some(out)),
        _ => Err("the bar is not the file's first program".into()),
    }
}

/// `bevel95.exe` or `programs/<letters, digits and dashes>.exe`: nothing that could reach outside
/// the folder it is unpacked into.
fn good_name(name: &str) -> bool {
    if name == BAR {
        return true;
    }
    let Some(file) = name.strip_prefix("programs/").and_then(|f| f.strip_suffix(".exe")) else { return false };
    !file.is_empty() && file.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A program of a package and its bytes.
pub type Unpacked<'a> = (Entry, &'a [u8]);

/// Every program of a package with its bytes, each checked against its fingerprint; `Ok(None)`
/// for a file that is no package. An error says what is wrong (a damaged download).
pub fn unpack(data: &[u8]) -> Result<Option<Vec<Unpacked<'_>>>, String> {
    let Some(entries) = entries(data)? else { return Ok(None) };
    let mut out = Vec::new();
    for entry in entries {
        let bytes = &data[entry.offset as usize..(entry.offset + entry.len) as usize];
        if sha256(bytes) != entry.sha256 {
            return Err(format!("{} does not match its fingerprint", entry.name));
        }
        out.push((entry, bytes));
    }
    Ok(Some(out))
}

/// The build stamp a Bevel95 program carries in its file: `version`, the commit it was built
/// from and that commit's time (seconds since 1970), between two NULs.
pub const STAMP_START: &[u8] = b"\0Bevel95 build stamp ";

/// What a build stamp says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub version: String,
    pub build: String,
    /// When the commit was made, in seconds since 1970 (0 when unknown).
    pub time: u64,
}

/// The stamp's text for a program (the Windows side puts it into every program it builds).
pub fn stamp_text(version: &str, build: &str, time: &str) -> String {
    format!("{}{version} {build} {time}\0", String::from_utf8_lossy(STAMP_START))
}

/// The stamp in a program's bytes, if it has one (programs built before 2026-11-17 have none).
pub fn find_stamp(bytes: &[u8]) -> Option<Stamp> {
    let at = bytes.windows(STAMP_START.len()).position(|w| w == STAMP_START)? + STAMP_START.len();
    let end = at + bytes[at..].iter().take(80).position(|&b| b == 0)?;
    let text = std::str::from_utf8(&bytes[at..end]).ok()?;
    let mut fields = text.split(' ');
    let (version, build, time) = (fields.next()?, fields.next()?, fields.next()?);
    Some(Stamp { version: version.to_string(), build: build.to_string(), time: time.parse().unwrap_or(0) })
}

/// Lowercase hexadecimal.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 || !text.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// SHA-256 (FIPS 180-4).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
        0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
        0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
        0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] =
        [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut block = |chunk: &[u8]| {
        let mut w = [0u32; 64];
        for (i, word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            (hh, g, f, e, d, c, b, a) = (g, f, e, d.wrapping_add(t1), c, b, a, t1.wrapping_add(t2));
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    };
    let (chunks, rest) = data.as_chunks::<64>();
    for chunk in chunks {
        block(chunk);
    }
    let mut last = [0u8; 128];
    last[..rest.len()].copy_from_slice(rest);
    last[rest.len()] = 0x80;
    let n = if rest.len() < 56 { 64 } else { 128 };
    last[n - 8..n].copy_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in last[..n].as_chunks::<64>().0 {
        block(chunk);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_answers() {
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // A million a's: many blocks, and the length past one block's worth of bits.
        assert_eq!(
            hex(&sha256(&vec![b'a'; 1_000_000])),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        // (The 56-byte message above is the padding's case of a second block.)
    }

    #[test]
    fn a_package_unpacks_to_what_was_packed() {
        let bar = b"MZ the bar".to_vec();
        let package = pack(&bar, &[("minefield.exe", b"MZ the game"), ("bevel-help.exe", b"MZ help")]);
        assert!(package.starts_with(&bar), "the bar comes first, unchanged: Windows runs it");
        let files = unpack(&package).unwrap().unwrap();
        let names: Vec<&str> = files.iter().map(|(e, _)| e.name.as_str()).collect();
        assert_eq!(names, ["bevel95.exe", "programs/minefield.exe", "programs/bevel-help.exe"]);
        assert_eq!(files[0].1, bar.as_slice());
        assert_eq!(files[1].1, b"MZ the game");
        assert_eq!(files[2].1, b"MZ help");
        // A program as built is no package.
        assert_eq!(unpack(&bar).unwrap(), None);
        assert!(!is_packed(b"short"));
    }

    #[test]
    fn a_damaged_package_is_refused() {
        let package = pack(b"MZ the bar", &[("solitaire.exe", b"MZ cards")]);
        // One byte of a program changed: its fingerprint no longer matches.
        let mut damaged = package.clone();
        damaged[12] ^= 1;
        assert_eq!(unpack(&damaged).unwrap_err(), "programs/solitaire.exe does not match its fingerprint");
        // Cut short (a download that stopped): no trailer at the end, so it is just a program.
        assert_eq!(entries(&package[..package.len() - 5]).unwrap(), None);
        // A trailer whose index does not fit.
        let mut moved = package.clone();
        let n = moved.len();
        moved[n - 16] ^= 1;
        assert!(entries(&moved).is_err());
        // Names that would reach outside the folder.
        for bad in ["programs/../x.exe", "..\\x.exe", "programs/x.dll", "c:/x.exe", "programs/.exe", "programs/X.exe"] {
            assert!(!good_name(bad), "{bad}");
        }
        assert!(good_name("programs/bevel-help.exe") && good_name("bevel95.exe"));
        let mut renamed = package.clone();
        let at = renamed.windows(11).position(|w| w == b"/solitaire.").unwrap();
        renamed[at + 1..at + 3].copy_from_slice(b"..");
        assert_eq!(entries(&renamed).unwrap_err(), "an index line names an unexpected file");
    }

    #[test]
    fn the_build_stamp_is_found_in_a_program() {
        let mut bytes = b"MZ...".to_vec();
        bytes.extend_from_slice(stamp_text("0.1.0", "1a2b3c4", "1795000000").as_bytes());
        bytes.extend_from_slice(b"more");
        assert_eq!(
            find_stamp(&bytes),
            Some(Stamp { version: "0.1.0".into(), build: "1a2b3c4".into(), time: 1_795_000_000 })
        );
        assert_eq!(find_stamp(b"MZ no stamp"), None);
        let unknown = stamp_text("0.1.0", "unknown", "0");
        assert_eq!(find_stamp(unknown.as_bytes()).unwrap().time, 0);
    }
}

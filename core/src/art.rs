//! The core's side of the way into `bevel_art`, where Windows' own pictures, sounds and logos are
//! kept. Each lookup runs while the core is compiled (`const`), so a picture that
//! is not there stops the build, and the pictures end up inside the programs as before.
//!
//! Look pictures up only in `const` and `static` items, `const { }` blocks and `const fn`s: a
//! lookup while the program runs would put every picture of bevel-art into every program (about
//! 1.6 MB each; `art/tests/one_place.rs` checks).

pub(crate) use bevel_art::{Look, Rows};
use bevel_art::{Art, Caption, Depth, Place};

/// The look to name for a picture every look had alike (any look finds it).
pub(crate) const ANY: Look = Look::Win95;

const fn find(what: &str, look: Look, depth: Depth) -> Art {
    match bevel_art::get(what, look, depth) {
        Some(art) => art,
        None => panic!("a picture missing from bevel-art"),
    }
}

/// A file's bytes (True Color).
pub(crate) const fn bytes(what: &str, look: Look) -> &'static [u8] {
    bytes_at(what, look, Depth::TrueColor)
}

/// A file's bytes at `depth`.
pub(crate) const fn bytes_at(what: &str, look: Look, depth: Depth) -> &'static [u8] {
    match find(what, look, depth) {
        Art::Bytes(b) => b,
        _ => panic!("not a file in bevel-art"),
    }
}

/// A file's bytes, which must be exactly `N` of them.
pub(crate) const fn exactly<const N: usize>(what: &str, look: Look) -> &'static [u8; N] {
    let b = bytes(what, look);
    assert!(b.len() == N, "a file in bevel-art of another size");
    match b.first_chunk::<N>() {
        Some(a) => a,
        None => panic!("a file in bevel-art of another size"),
    }
}

/// A picture written as text (True Color).
pub(crate) const fn rows(what: &str, look: Look) -> &'static Rows {
    rows_at(what, look, Depth::TrueColor)
}

/// A picture written as text, at `depth`.
pub(crate) const fn rows_at(what: &str, look: Look, depth: Depth) -> &'static Rows {
    match find(what, look, depth) {
        Art::Rows(r) => r,
        _ => panic!("not a picture written as text in bevel-art"),
    }
}

/// Colors (a palette, or one color).
pub(crate) const fn colors(what: &str, look: Look) -> &'static [u32] {
    match find(what, look, Depth::TrueColor) {
        Art::Colors(c) => c,
        _ => panic!("not colors in bevel-art"),
    }
}

/// A file of text.
pub(crate) const fn text(what: &str, look: Look) -> &'static str {
    match find(what, look, Depth::TrueColor) {
        Art::Text(t) => t,
        _ => panic!("not text in bevel-art"),
    }
}

/// Captions cut as pixels.
pub(crate) const fn captions(what: &str, look: Look) -> &'static [Caption] {
    match find(what, look, Depth::TrueColor) {
        Art::Captions(c) => c,
        _ => panic!("not captions in bevel-art"),
    }
}

/// The places of a sheet's pictures.
pub(crate) const fn places(what: &str, look: Look) -> &'static [Place] {
    places_at(what, look, Depth::TrueColor)
}

/// The places of a sheet's pictures at `depth`.
pub(crate) const fn places_at(what: &str, look: Look, depth: Depth) -> &'static [Place] {
    match find(what, look, depth) {
        Art::Places(p) => p,
        _ => panic!("not a sheet's places in bevel-art"),
    }
}


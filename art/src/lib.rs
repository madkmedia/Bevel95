//! Windows 95's, 98's and 2000's own pictures, sounds and logos that Bevel95 shows, in one place
//! (they are Microsoft's, not covered by Bevel95's licence: README.md, "License").
//!
//! Data and nothing else: no drawing, no rules, no dependencies. The one way in is [`get`]: a
//! picture looked up by what it is, for which look and at which color depth. What a picture is
//! called is "group/name" (`"start/programs"`); the look is the Windows it was taken from (the
//! core decides where else it shows it: Windows 98's Windows Update entry shows 2000's picture,
//! for one). Where the same picture served all three Windows, it answers for each.
//!
//! `get` is a `const fn`: `bevel-core` looks its pictures up while it is compiled, so a name that
//! is not here stops the build, and the programs are built with the pictures inside them, as
//! before.
#![no_std]
#![forbid(unsafe_code)]

mod assets;
mod banner;
mod calculator;
mod colors16;
mod controls;
mod desktop;
mod explorer;
mod folder;
mod html_help;
mod pointers;
mod quick_launch;
mod start;
mod taskbar;
mod tray;
mod winhelp;

/// The Windows a picture was taken from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Look {
    Win95,
    Win98,
    Win2000,
}

/// The screen it was taken from: True Color, or that Windows on a 16-color screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Depth {
    TrueColor,
    Sixteen,
}

/// The three Windows, for a picture each of them had alike.
pub const EVERY_LOOK: &[Look] = &[Look::Win95, Look::Win98, Look::Win2000];

/// A picture written as text: rows top to bottom, one character per pixel. Where it has a
/// palette, a character is a color in it (0xRRGGBB) and any other is see-through; where it has
/// none, the core says what each character is (an outline, a shadow, ink).
#[derive(Debug)]
pub struct Rows {
    pub w: i32,
    pub h: i32,
    pub palette: &'static [(u8, u32)],
    pub rows: &'static [&'static str],
}

/// A caption cut as pixels: where its ink starts (x, y) and its rows ("#" ink).
pub type Caption = (i32, i32, &'static [&'static str]);

/// Where one picture lies in a sheet of pictures: its name, its first byte, width, height.
pub type Place = (&'static str, usize, i32, i32);

/// What a lookup finds.
#[derive(Clone, Copy, Debug)]
pub enum Art {
    /// A picture written as text.
    Rows(&'static Rows),
    /// A file's bytes, as cut (the core knows each file's layout: raw RGB, RGBA, palette indexes,
    /// bit planes, sound samples, a font's glyphs).
    Bytes(&'static [u8]),
    /// A file of text.
    Text(&'static str),
    /// Colors, 0xRRGGBB: a picture's palette, or a picture's one color.
    Colors(&'static [u32]),
    /// Captions cut as pixels.
    Captions(&'static [Caption]),
    /// The places of the pictures in a sheet.
    Places(&'static [Place]),
}

/// One picture: what it is, the Windows it answers for, its depth.
#[derive(Debug)]
pub struct Entry {
    pub what: &'static str,
    pub looks: &'static [Look],
    pub depth: Depth,
    pub art: Art,
}

/// Every group of pictures.
const GROUPS: &[&[Entry]] = &[
    assets::ENTRIES,
    start::ENTRIES,
    banner::ENTRIES,
    desktop::ENTRIES,
    taskbar::ENTRIES,
    quick_launch::ENTRIES,
    tray::ENTRIES,
    folder::ENTRIES,
    explorer::ENTRIES,
    controls::ENTRIES,
    pointers::ENTRIES,
    html_help::ENTRIES,
    winhelp::ENTRIES,
    calculator::ENTRIES,
    colors16::ENTRIES,
];

/// The picture `what` of `look` at `depth`, if there is one.
pub const fn get(what: &str, look: Look, depth: Depth) -> Option<Art> {
    let mut g = 0;
    while g < GROUPS.len() {
        let group = GROUPS[g];
        let mut i = 0;
        while i < group.len() {
            let e = &group[i];
            if e.depth as u8 == depth as u8 && same(e.what, what) && answers(e.looks, look) {
                return Some(e.art);
            }
            i += 1;
        }
        g += 1;
    }
    None
}

/// Every picture, for listing them (the inventory's counts, tests).
pub fn entries() -> impl Iterator<Item = &'static Entry> {
    GROUPS.iter().flat_map(|g| g.iter())
}

const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

const fn answers(looks: &[Look], look: Look) -> bool {
    let mut i = 0;
    while i < looks.len() {
        if looks[i] as u8 == look as u8 {
            return true;
        }
        i += 1;
    }
    false
}

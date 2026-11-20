//! A program's first atlas holds the same letters as every later one: in a process of its own,
//! as a program starts. (Loaded only after a failed ask, the code pages' MS Sans Serif files
//! gave other letters to the first ask after loading: `font::other_letters`.)

use bevel_winkit::font::render;

#[test]
fn the_first_atlas_of_a_process_is_like_every_later_one() {
    for bold in [false, true] {
        let first = render("MS Sans Serif", 11, bold).unwrap();
        let later = render("MS Sans Serif", 11, bold).unwrap();
        for (a, b) in first.glyphs().iter().zip(later.glyphs()) {
            assert!(a == b, "bold={bold}: U+{:04X} differs between the first atlas and a later one", a.ch as u32);
        }
        assert_eq!(first.glyphs().len(), later.glyphs().len());
    }
}

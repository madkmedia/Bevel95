//! Cargo build script for the Windows programs. The MSVC linker accepts a .res
//! alongside its object files, so a plain cargo build embeds the icons from the core.

use bevel_core::bevel_icon;
use bevel_core::calculator;
use bevel_core::fb::Icon;
use bevel_core::icons::{WIN95_HELP, WIN95_SMALL_HELP};
use bevel_core::minefield::art;
use bevel_core::solitaire;
use bevel_core::theme::Era;
use std::{env, fs, path::PathBuf};

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const SIZES: [usize; 5] = [16, 32, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=../../tools/build/program_icon.rs");
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let program = env::var("CARGO_PKG_NAME").unwrap();
    let (small, large) = match program.as_str() {
        "bevel95" => (bevel_icon::icon(16), bevel_icon::icon(32)),
        "bevel-help" => (WIN95_SMALL_HELP.icon(), WIN95_HELP.icon()),
        "minefield" => (art::icon(Era::Win95, 16), art::icon(Era::Win95, 32)),
        "solitaire" => (solitaire::art::icon(Era::Win95, 16), solitaire::art::icon(Era::Win95, 32)),
        "calculator" => (calculator::art::icon(Era::Win95, 16), calculator::art::icon(Era::Win95, 32)),
        "folders" => (
            bevel_core::folder::art::small(Era::Win95, bevel_core::folder::art::Small::Folder),
            bevel_core::desktop_art::picture(Era::Win95, bevel_core::desktop_art::Art::Folder, false, 0).unwrap(),
        ),
        _ => panic!("no executable icon for {program}"),
    };

    let mut res = Vec::new();
    resource(&mut res, 0, 0, 0, &[]); // The Win32 .res signature is an empty resource.
    let mut group = Vec::from([0, 0, 1, 0]); // GRPICONDIR: reserved, type ICON
    group.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    for (i, size) in SIZES.into_iter().enumerate() {
        let source = if size.is_multiple_of(32) { &large } else { &small };
        let image = bitmap(source, size);
        let id = (i + 1) as u16;
        resource(&mut res, RT_ICON, id, 0x1010, &image);
        // A group entry names its RT_ICON. Zero in a dimension byte means 256.
        group.extend_from_slice(&[size as u8, size as u8, 16, 0]);
        group.extend_from_slice(&1u16.to_le_bytes()); // planes
        group.extend_from_slice(&4u16.to_le_bytes()); // bits per pixel
        group.extend_from_slice(&(image.len() as u32).to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    resource(&mut res, RT_GROUP_ICON, 1, 0x1030, &group);
    let path = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("program-icon.res");
    fs::write(&path, res).expect("write executable icon resource");
    println!("cargo:rustc-link-arg-bin={program}={}", path.display());
}

/// A Win32 resource with numeric type and name, neutral language, and DWORD alignment.
fn resource(out: &mut Vec<u8>, kind: u16, id: u16, flags: u16, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes()); // header size for numeric identifiers
    for word in [0xffff, kind, 0xffff, id] {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // data version
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&[0; 10]); // language, version, characteristics
    out.extend_from_slice(data);
    out.resize(out.len().next_multiple_of(4), 0);
}

/// An icon DIB has a bottom-up color bitmap followed by a 1-bit transparency mask.
/// The programs' icons fit a 16-color palette and have fully opaque or transparent pixels.
fn bitmap(icon: &Icon, size: usize) -> Vec<u8> {
    let side = icon.width as usize;
    assert_eq!(icon.height, icon.width);
    assert!(size.is_multiple_of(side));
    let scale = size / side;
    let mut palette: Vec<_> = icon.pixels.iter().map(|&p| p & 0x00ff_ffff).collect();
    palette.sort_unstable();
    palette.dedup();
    assert!(palette.len() <= 16);

    let mut data = Vec::new();
    data.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER
    data.extend_from_slice(&(size as u32).to_le_bytes());
    data.extend_from_slice(&(2 * size as u32).to_le_bytes()); // includes the mask
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&4u16.to_le_bytes());
    data.extend_from_slice(&[0; 24]); // uncompressed, full 16-entry color table
    for i in 0..16 {
        data.extend_from_slice(&palette.get(i).copied().unwrap_or(0).to_le_bytes());
    }
    let color_stride = size.div_ceil(8) * 4;
    let mask_stride = size.div_ceil(32) * 4;
    let color_start = data.len();
    let mask_start = color_start + color_stride * size;
    data.resize(mask_start + mask_stride * size, 0);
    for y in 0..size {
        let row = size - 1 - y;
        for x in 0..size {
            let pixel = icon.pixels[(y / scale) * side + x / scale];
            if pixel >> 24 == 0 {
                data[mask_start + row * mask_stride + x / 8] |= 0x80 >> (x % 8);
            } else {
                let index = palette.binary_search(&(pixel & 0x00ff_ffff)).unwrap() as u8;
                data[color_start + row * color_stride + x / 2] |= if x % 2 == 0 { index << 4 } else { index };
            }
        }
    }
    data
}

//! A Windows icon (HICON) as pixels: drawn on black and on white, the difference giving each
//! pixel's transparency. This works for old icons with a mask and new ones with soft (alpha)
//! edges alike. Shared by the bar (task buttons, the notification area, the Start menu) and the
//! folder windows (the shell's pictures of the owner's files).

use std::ffi::c_void;
use std::ptr::null_mut;

use bevel_core::fb::Icon;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GdiFlush, GetObjectW, SelectObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DrawIconEx, GetIconInfo, HICON, ICONINFO};

/// The icon drawn at 16×16.
pub fn icon_pixels(icon: HICON) -> Option<Icon> {
    icon_pixels_at(icon, 16)
}

/// The icon drawn at its own size (programs hand the notification area icons made for the
/// screen's scale, 24×24 at 150%), then shrunk to 16×16 by averaging (`Icon::downscaled`),
/// which keeps fine lines that drawing it straight at 16×16 would drop.
pub fn icon_pixels_fitted(icon: HICON) -> Option<Icon> {
    let size = icon_size(icon).unwrap_or(16).clamp(16, 256);
    Some(icon_pixels_at(icon, size)?.downscaled(16))
}

/// The icon drawn at `size` × `size`.
pub fn icon_pixels_at(icon: HICON, size: i32) -> Option<Icon> {
    let on_black = draw_icon_on(icon, 0x0000_0000, size)?;
    let on_white = draw_icon_on(icon, 0x00FF_FFFF, size)?;
    let pixels = on_black.iter().zip(&on_white).map(|(&b, &w)| unblend(b, w)).collect();
    Some(Icon { width: size, height: size, pixels })
}

/// An icon's width in pixels, from its color bitmap (or its mask for a black-and-white one).
fn icon_size(icon: HICON) -> Option<i32> {
    let mut info = ICONINFO::default();
    // SAFETY: valid out-structs; the two bitmaps GetIconInfo makes are ours to delete.
    unsafe {
        if GetIconInfo(icon, &mut info) == 0 {
            return None;
        }
        let source = if info.hbmColor.is_null() { info.hbmMask } else { info.hbmColor };
        let mut bitmap = BITMAP::default();
        let ok = GetObjectW(source, size_of::<BITMAP>() as i32, (&mut bitmap as *mut BITMAP).cast()) != 0;
        if !info.hbmColor.is_null() {
            DeleteObject(info.hbmColor);
        }
        if !info.hbmMask.is_null() {
            DeleteObject(info.hbmMask);
        }
        (ok && bitmap.bmWidth > 0).then_some(bitmap.bmWidth)
    }
}

fn draw_icon_on(icon: HICON, background: u32, size: i32) -> Option<Vec<u32>> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: a private size×size DIB on a private DC, both released before returning; the
    // pixel slice is only used while the DIB exists.
    unsafe {
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            return None;
        }
        let mut bits: *mut c_void = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bitmap.is_null() || bits.is_null() {
            DeleteDC(dc);
            return None;
        }
        let old = SelectObject(dc, bitmap);
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), (size * size) as usize);
        pixels.fill(background);
        let drawn = DrawIconEx(dc, 0, 0, icon, size, size, 0, null_mut(), DI_NORMAL) != 0;
        GdiFlush();
        let out = drawn.then(|| pixels.to_vec());
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        out
    }
}

/// A pixel's color and transparency from the same pixel drawn over black (`b`) and white (`w`).
fn unblend(b: u32, w: u32) -> u32 {
    let channel = |pixel: u32, shift: u32| ((pixel >> shift) & 0xFF) as i32;
    let difference: i32 = [0, 8, 16].iter().map(|&s| channel(w, s) - channel(b, s)).sum::<i32>() / 3;
    let alpha = (255 - difference).clamp(0, 255);
    if alpha == 0 {
        return 0;
    }
    let color = |shift: u32| ((channel(b, shift) * 255 + alpha / 2) / alpha).clamp(0, 255) as u32;
    ((alpha as u32) << 24) | (color(16) << 16) | (color(8) << 8) | color(0)
}

#[cfg(test)]
mod tests {
    use super::unblend;

    #[test]
    fn unblend_recovers_alpha_and_color() {
        // Opaque red: the same over black and white.
        assert_eq!(unblend(0x00FF_0000, 0x00FF_0000), 0xFFFF_0000);
        // Fully transparent: black over black, white over white.
        assert_eq!(unblend(0x0000_0000, 0x00FF_FFFF), 0);
        // Half-transparent white: 128 over black, 255 over white.
        assert_eq!(unblend(0x0080_8080, 0x00FF_FFFF), 0x80FF_FFFF);
    }
}

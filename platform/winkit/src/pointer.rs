//! The mouse pointers a window asks for (`bevel_core::kit::Pointer`): Windows' own where
//! today's Windows still has the shape (the I-beam, the hand, the two-sided arrow, the header
//! control's divider pointer in comctl32), Windows 95 Explorer's split pointer made from its
//! measured pixels. Each is made once per thread and kept (never destroyed: shared or for the
//! program's life).

use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::null_mut;

use bevel_core::kit::{Pointer, PointerArt, SPLIT95};
use windows_sys::Win32::System::LibraryLoader::{LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE, LoadLibraryExW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateCursor, GetSystemMetrics, HCURSOR, IDC_HAND, IDC_IBEAM, IDC_SIZEWE, IMAGE_CURSOR, LR_DEFAULTCOLOR, LoadCursorW,
    LoadImageW, SM_CXCURSOR, SetCursor,
};

use crate::log::log_win32;

/// comctl32's own pointer for a header's divider (its resource 106, the same shape since
/// Windows 95).
const HEADER_DIVIDER: u16 = 106;

thread_local! {
    static MADE: RefCell<HashMap<Pointer, HCURSOR>> = RefCell::new(HashMap::new());
}

/// Shows `pointer`; false for the arrow (the window class's own is left to Windows).
pub fn show(pointer: Pointer) -> bool {
    if pointer == Pointer::Arrow {
        return false;
    }
    let made = MADE.with(|m| m.borrow().get(&pointer).copied());
    let cursor = made.unwrap_or_else(|| {
        let c = make(pointer);
        MADE.with(|m| m.borrow_mut().insert(pointer, c));
        c
    });
    if cursor.is_null() {
        return false;
    }
    // SAFETY: a cursor made or loaded above.
    unsafe { SetCursor(cursor) };
    true
}

/// How many times the measured pixels are drawn: as big as Windows draws its own pointers.
fn factor() -> i32 {
    // SAFETY: a plain query.
    let size = unsafe { GetSystemMetrics(SM_CXCURSOR) };
    ((size + 16) / 32).max(1)
}

fn make(pointer: Pointer) -> HCURSOR {
    // SAFETY: shared system cursors; comctl32 opened as a resource file only; CreateCursor gets
    // two planes of the size given.
    unsafe {
        let c = match pointer {
            Pointer::Arrow => return null_mut(),
            Pointer::Text => LoadCursorW(null_mut(), IDC_IBEAM),
            Pointer::Hand => LoadCursorW(null_mut(), IDC_HAND),
            Pointer::SizeWE => LoadCursorW(null_mut(), IDC_SIZEWE),
            Pointer::HeaderDivider => {
                let name: Vec<u16> = "comctl32.dll\0".encode_utf16().collect();
                let module = LoadLibraryExW(name.as_ptr(), null_mut(), LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE);
                let size = 32 * factor();
                let c = if module.is_null() {
                    null_mut()
                } else {
                    LoadImageW(module, HEADER_DIVIDER as usize as *const u16, IMAGE_CURSOR, size, size, LR_DEFAULTCOLOR)
                };
                // Without comctl32's pointer: the two-sided arrow.
                if c.is_null() {
                    log_win32("LoadImageW (comctl32's header divider pointer)");
                    LoadCursorW(null_mut(), IDC_SIZEWE)
                } else {
                    c
                }
            }
            Pointer::Split95 => from_art(&SPLIT95),
        };
        if c.is_null() {
            log_win32("a mouse pointer");
        }
        c
    }
}

/// A black-and-white pointer from measured pixels, each drawn `factor()` times as big.
fn from_art(art: &PointerArt) -> HCURSOR {
    let k = factor();
    let (w, h) = art.size();
    // Cursor planes are 32 pixels wide at least here, a multiple of 16 (word-aligned rows).
    let side = ((w.max(h) * k + 31) / 32 * 32).max(32);
    let row_bytes = (side / 8) as usize;
    // AND plane: 1 = see-through; XOR plane: 1 = white (with AND 0), 0 = black.
    let mut and = vec![0xFFu8; row_bytes * side as usize];
    let mut xor = vec![0u8; row_bytes * side as usize];
    for y in 0..side {
        for x in 0..side {
            let Some(black) = art.pixel(x / k, y / k) else { continue };
            let at = y as usize * row_bytes + (x / 8) as usize;
            let bit = 0x80u8 >> (x % 8);
            and[at] &= !bit;
            if !black {
                xor[at] |= bit;
            }
        }
    }
    let (hx, hy) = art.hot;
    // SAFETY: two planes of side × side bits each.
    unsafe { CreateCursor(null_mut(), hx * k + k / 2, hy * k + k / 2, side, side, and.as_ptr().cast(), xor.as_ptr().cast()) }
}

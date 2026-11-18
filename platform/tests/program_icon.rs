//! Extract the actual executable's default icon the way a Windows shortcut does.
#![cfg(all(windows, target_env = "msvc"))]

use std::ptr::null_mut;
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, PrivateExtractIconsW};

#[test]
fn shell_can_extract_each_embedded_size() {
    let exe = env!(concat!("CARGO_BIN_EXE_", env!("CARGO_PKG_NAME")));
    let path: Vec<u16> = exe.encode_utf16().chain([0]).collect();
    for (i, size) in [16, 32, 48, 64, 256].into_iter().enumerate() {
        let mut icon = null_mut();
        let mut id = 0;
        // SAFETY: terminated path and writable output slots for one icon.
        let count = unsafe { PrivateExtractIconsW(path.as_ptr(), 0, size, size, &mut icon, &mut id, 1, 0) };
        assert_eq!(count, 1, "{size}px icon in {exe}");
        assert!(!icon.is_null());
        // SAFETY: the extraction call handed ownership of this handle to us.
        unsafe { DestroyIcon(icon) };
        assert_eq!(id, (i + 1) as u32, "Windows should find a native {size}px frame");
    }
}

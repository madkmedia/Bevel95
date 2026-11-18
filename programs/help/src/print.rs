//! Print one Help topic using the printer chosen in Windows' common Print dialog.

use std::ptr::{null, null_mut};

use bevel_winkit::log::log;
use bevel_winkit::util::wide;
use windows_sys::Win32::Foundation::{ERROR_CANCELLED, GetLastError, GlobalFree, HWND, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DeleteDC, DeleteObject, GetDeviceCaps, GetTextExtentExPointW, GetTextMetricsW, HDC, HORZRES,
    LOGFONTW, LOGPIXELSX, LOGPIXELSY, SP_USERABORT, SelectObject, TEXTMETRICW, TextOutW, VERTRES,
};
use windows_sys::Win32::Storage::Xps::{AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
use windows_sys::Win32::UI::Controls::Dialogs::{
    CommDlgExtendedError, PD_NOPAGENUMS, PD_NOSELECTION, PD_PRINTTOFILE, PD_RETURNDC, PD_USEDEVMODECOPIESANDCOLLATE,
    PRINTDLGW, PrintDlgW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};

pub fn print(owner: HWND, title: &str, text: &str) {
    let mut dialog = PRINTDLGW {
        lStructSize: size_of::<PRINTDLGW>() as u32,
        hwndOwner: owner,
        Flags: PD_RETURNDC | PD_NOSELECTION | PD_NOPAGENUMS | PD_USEDEVMODECOPIESANDCOLLATE,
        nCopies: 1,
        ..Default::default()
    };
    // SAFETY: the dialog and both strings live through these synchronous calls. The dialog
    // returns the printer DC and global allocations to us, including on some error paths.
    unsafe {
        let result = if PrintDlgW(&mut dialog) != 0 {
            let title = wide(title);
            let file_port = wide("FILE:");
            let doc = DOCINFOW {
                cbSize: size_of::<DOCINFOW>() as i32,
                lpszDocName: title.as_ptr(),
                lpszOutput: if dialog.Flags & PD_PRINTTOFILE != 0 { file_port.as_ptr() } else { null() },
                ..Default::default()
            };
            print_topic(dialog.hDC, &doc, text).map_err(str::to_owned)
        } else {
            match CommDlgExtendedError() {
                0 => Ok(()), // Cancel.
                code => Err(format!("The print dialog failed (0x{code:X}).")),
            }
        };
        if !dialog.hDC.is_null() {
            DeleteDC(dialog.hDC);
        }
        for handle in [dialog.hDevMode, dialog.hDevNames] {
            if !handle.is_null() {
                GlobalFree(handle);
            }
        }
        if let Err(message) = result {
            log(&format!("Help: {message}"));
            MessageBoxW(owner, wide(&message).as_ptr(), wide(title).as_ptr(), MB_OK | MB_ICONERROR);
        }
    }
}

// Wrap to the actual printer font's advances, preserving paragraph breaks.
fn wrap(dc: HDC, text: &str, width: i32) -> Result<Vec<Vec<u16>>, &'static str> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        // TextOutW has no tab stops. Keep neighboring table cells separated on the
        // printed line and measure the same spaces that the printer will draw.
        let encoded: Vec<u16> = paragraph.replace('\t', "    ").encode_utf16().collect();
        let mut rest = &encoded[..];
        if rest.is_empty() {
            lines.push(Vec::new());
        }
        while !rest.is_empty() {
            let mut fit = 0;
            let mut measured = SIZE::default();
            // SAFETY: rest contains the indicated number of UTF-16 units; both outputs are valid.
            let ok = unsafe {
                GetTextExtentExPointW(dc, rest.as_ptr(), rest.len() as i32, width, &mut fit, null_mut(), &mut measured)
            };
            if ok == 0 || fit <= 0 {
                return Err("The printer could not fit the Help text on the page.");
            }
            let mut end = fit as usize;
            if end < rest.len() {
                if rest[end] != u16::from(b' ') {
                    end = rest[..end].iter().rposition(|&c| c == u16::from(b' ')).filter(|&i| i > 0).unwrap_or(end);
                }
                if (0xD800..=0xDBFF).contains(&rest[end - 1]) {
                    end -= 1;
                }
                if end == 0 {
                    return Err("The printer could not fit a character on the page.");
                }
            }
            lines.push(rest[..end].to_vec());
            rest = &rest[end..];
            while rest.first() == Some(&u16::from(b' ')) {
                rest = &rest[1..];
            }
        }
    }
    Ok(lines)
}

fn print_topic(dc: HDC, doc: &DOCINFOW, text: &str) -> Result<(), &'static str> {
    // SAFETY: PrintDlgW returned this printer DC. All font selections are restored before
    // the font is destroyed, and any document that fails during printing is aborted.
    unsafe {
        let dpi_x = GetDeviceCaps(dc, LOGPIXELSX as i32);
        let dpi_y = GetDeviceCaps(dc, LOGPIXELSY as i32);
        let (x, y) = (dpi_x / 2, dpi_y / 2);
        let width = GetDeviceCaps(dc, HORZRES as i32) - 2 * x;
        let height = GetDeviceCaps(dc, VERTRES as i32) - 2 * y;
        if width <= 0 || height <= 0 || dpi_x <= 0 || dpi_y <= 0 {
            return Err("The printer reported an unusable page size.");
        }
        let mut spec = LOGFONTW { lfHeight: -(dpi_y * 10 / 72).max(1), ..Default::default() };
        let face = wide("Arial");
        spec.lfFaceName[..face.len()].copy_from_slice(&face);
        let font = CreateFontIndirectW(&spec);
        if font.is_null() {
            return Err("The printer could not create the Help font.");
        }
        let old = SelectObject(dc, font);
        let result = if old.is_null() {
            Err("The printer could not select the Help font.")
        } else {
            let result = print_pages(dc, doc, text, x, y, width, height);
            SelectObject(dc, old);
            result
        };
        DeleteObject(font);
        result
    }
}

fn print_pages(
    dc: HDC,
    doc: &DOCINFOW,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(), &'static str> {
    let lines = wrap(dc, text, width)?;
    if lines.is_empty() {
        return Ok(());
    }
    let mut metrics = TEXTMETRICW::default();
    // SAFETY: the DC and selected font remain valid throughout the job. TextOutW uses
    // each line's explicit length. The driver handles the copies requested in PrintDlgW.
    unsafe {
        if GetTextMetricsW(dc, &mut metrics) == 0 {
            return Err("The printer could not measure the Help font.");
        }
        let step = (metrics.tmHeight + metrics.tmExternalLeading).max(1);
        let rows = height / step;
        if rows == 0 {
            return Err("The printable area is too small for the Help text.");
        }
        let job = StartDocW(dc, doc);
        if job <= 0 {
            return if job == SP_USERABORT || GetLastError() == ERROR_CANCELLED {
                Ok(())
            } else {
                Err("The printer could not start the document.")
            };
        }
        for page in lines.chunks(rows as usize) {
            let printed = StartPage(dc) > 0
                && page.iter().enumerate().all(|(n, line)| {
                    line.is_empty() || TextOutW(dc, x, y + n as i32 * step, line.as_ptr(), line.len() as i32) != 0
                })
                && EndPage(dc) > 0;
            if !printed {
                AbortDoc(dc);
                return Err("The printer could not finish the page.");
            }
        }
        if EndDoc(dc) <= 0 {
            AbortDoc(dc);
            return Err("The printer could not finish the document.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Graphics::Gdi::{CreateCompatibleDC, GetTextExtentPoint32W};

    #[test]
    fn topic_text_wraps_to_the_device_and_keeps_blank_lines() {
        // A memory DC exercises the same GDI measurements without opening a printer.
        unsafe {
            let dc = CreateCompatibleDC(null_mut());
            assert!(!dc.is_null());
            let word: Vec<u16> = "mine".encode_utf16().collect();
            let mut size = SIZE::default();
            let measured = GetTextExtentPoint32W(dc, word.as_ptr(), word.len() as i32, &mut size);
            let lines = wrap(dc, "mine mine\r\n\r\nmine", size.cx);
            DeleteDC(dc);
            assert_ne!(measured, 0);
            assert_eq!(lines.unwrap(), [word.clone(), word.clone(), Vec::new(), word]);
        }
    }
}

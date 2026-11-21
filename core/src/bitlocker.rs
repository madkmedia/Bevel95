//! A drive locked by BitLocker, Windows' own disk lock: it cannot be read until the person gives
//! its password to Windows. Windows 95, 98 and 2000 had no BitLocker, so the folder windows do
//! what today's Windows does: Open and Explore on a locked drive start Windows' own unlock window
//! for it (`bdeunlock.exe`, the program Windows registers for its "Unlock Drive..." entry), and
//! the drive's menu has that entry while the drive is locked. The password never passes through
//! Bevel95.
//!
//! How Windows says a drive is locked (no guess; the brief of 2026-11-21, job 2):
//! - a read of the drive fails with `FVE_E_LOCKED_VOLUME` (0x80310000, "This drive is locked by
//!   BitLocker Drive Encryption", windows-sys `Win32::Foundation::FVE_E_LOCKED_VOLUME`), the
//!   value `GetLastError` gives for a call on a locked volume;
//! - the shell's property `System.Volume.BitLockerProtection` is 6, "Locked" (Windows' own list
//!   of its values, read on this PC by `private/probes/bitlocker/enum.ps1`: 1 On, 2 Off,
//!   3 Encrypting, 4 Decrypting, 5 Suspended, 6 Locked, 7 Not encryptable, 8 Waiting for
//!   activation), the very condition Windows registers for its own Unlock Drive entry
//!   (`HKCR\Drive\shell\unlock-bde`, `AppliesTo = System.Volume.BitLockerProtection:=6`).
//!
//! A drive that can be read is never locked, whatever else is said of it.

/// `FVE_E_LOCKED_VOLUME` as `GetLastError` gives it.
pub const LOCKED_VOLUME: u32 = 0x8031_0000;

/// `System.Volume.BitLockerProtection`'s value "Locked".
pub const PROTECTION_LOCKED: u32 = 6;

/// `ERROR_NOT_READY`: a drive without a disk (a card reader, an empty CD drive).
pub const NOT_READY: u32 = 21;

/// The words of Windows' own entry (its resource `bdeunlock.exe,-100`: "&Unlock Drive...").
pub const UNLOCK_LABEL: &str = "&Unlock Drive...";

/// Whether, after a read of a drive failed with `error`, the shell is worth asking about the
/// drive's lock (the error does not already say: locked, or no disk in the drive).
pub fn ask_shell_after(error: u32) -> bool {
    error != 0 && error != LOCKED_VOLUME && error != NOT_READY
}

/// Whether a drive is locked by BitLocker: `read_error` is `None` when its read worked, else the
/// error the read gave; `protection` is the shell's `System.Volume.BitLockerProtection`, if it
/// was asked and answered.
pub fn is_locked(read_error: Option<u32>, protection: Option<u32>) -> bool {
    match read_error {
        None => false,
        Some(LOCKED_VOLUME) => true,
        Some(_) => protection == Some(PROTECTION_LOCKED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_answer_for_a_locked_drive_means_locked() {
        assert!(is_locked(Some(0x8031_0000), None), "FVE_E_LOCKED_VOLUME");
        assert!(is_locked(Some(5), Some(6)), "a failed read and the shell's Locked");
        assert!(!ask_shell_after(LOCKED_VOLUME), "the error says it already");
    }

    #[test]
    fn every_other_answer_means_not_locked() {
        // A drive that reads is not locked, even if the shell said so a moment ago.
        assert!(!is_locked(None, Some(6)));
        assert!(!is_locked(None, None));
        // No disk in the drive: as today (the look's "not accessible" message).
        assert!(!is_locked(Some(NOT_READY), None));
        assert!(!ask_shell_after(NOT_READY));
        // Other errors, with every other value of the shell's property, or none.
        for error in [2, 3, 5, 87, 1005, 1392, 0x8031_0001] {
            assert!(!is_locked(Some(error), None), "{error}");
            for value in [0, 1, 2, 3, 4, 5, 7, 8, 9] {
                assert!(!is_locked(Some(error), Some(value)), "{error} {value}");
            }
        }
        assert!(ask_shell_after(1005));
        assert!(!ask_shell_after(0));
    }

    #[test]
    fn the_entry_has_windows_own_words() {
        assert_eq!(UNLOCK_LABEL, "&Unlock Drive...");
    }
}

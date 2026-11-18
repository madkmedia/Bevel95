//! Windows 11's own flyouts (its list of Wi-Fi networks, quick settings, the notification
//! center, Safely Remove Hardware), which the bar opens or which open over it. Windows closes
//! one when it stops being the window in front. The bar never becomes the window in front, so a
//! press on it did not close them and Windows refused to bring any other window forward while
//! one was open. Measured on this Windows 11 (25H2): the Wi-Fi list is ShellHost.exe's window
//! of class "ControlCenterWindow", the window in front while it is open.

/// A Windows flyout in front, by the window that holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flyout {
    /// ShellHost's "ControlCenterWindow": the Wi-Fi list (the network icon's click) and quick
    /// settings.
    ControlCenter,
    /// Another of Windows' shell flyouts (a "Windows.UI.Core.CoreWindow" of ShellHost or
    /// ShellExperienceHost: the notification center, Safely Remove Hardware, the calendar).
    Other,
}

/// Which flyout the window in front is, from its class and its program file's name (any case,
/// with or without a folder); `None` for any other window.
pub fn flyout(class: &str, exe: &str) -> Option<Flyout> {
    let exe = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    let shell_host = exe.eq_ignore_ascii_case("ShellHost.exe");
    let experience_host = exe.eq_ignore_ascii_case("ShellExperienceHost.exe");
    match class {
        "ControlCenterWindow" if shell_host => Some(Flyout::ControlCenter),
        "Windows.UI.Core.CoreWindow" if shell_host || experience_host => Some(Flyout::Other),
        _ => None,
    }
}

/// Whether a press on one of the bar's notification icons still does its own thing after
/// the press closed `closed` (if a flyout was in front). The network icon opens the Wi-Fi list,
/// so while that list was open, the press only closes it (as on Windows' own taskbar);
/// any other icon goes on as usual.
pub fn icon_press_acts(network_icon: bool, closed: Option<Flyout>) -> bool {
    !(network_icon && closed == Some(Flyout::ControlCenter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_flyouts_are_known_by_class_and_program() {
        let host = r"C:\Windows\SystemApps\ShellHost\ShellHost.exe";
        assert_eq!(flyout("ControlCenterWindow", host), Some(Flyout::ControlCenter));
        assert_eq!(flyout("ControlCenterWindow", "shellhost.exe"), Some(Flyout::ControlCenter));
        assert_eq!(flyout("Windows.UI.Core.CoreWindow", host), Some(Flyout::Other));
        assert_eq!(flyout("Windows.UI.Core.CoreWindow", "ShellExperienceHost.exe"), Some(Flyout::Other));
        // A program's own window of the same class is not a flyout.
        assert_eq!(flyout("Windows.UI.Core.CoreWindow", "SomeApp.exe"), None);
        assert_eq!(flyout("ControlCenterWindow", "SomeApp.exe"), None);
        assert_eq!(flyout("Notepad", host), None);
    }

    #[test]
    fn the_network_icon_only_closes_its_open_list() {
        assert!(!icon_press_acts(true, Some(Flyout::ControlCenter)));
        assert!(icon_press_acts(true, Some(Flyout::Other)));
        assert!(icon_press_acts(true, None));
        assert!(icon_press_acts(false, Some(Flyout::ControlCenter)));
    }
}

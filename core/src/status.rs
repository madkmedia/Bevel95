//! Bevel95's own notification icons for the network connection and the battery: Windows 11 hands
//! out neither as a notification icon, so the bar shows each look's own (the owner's choice,
//! 2026-11-09). Pictures, wording and menus come from each Windows' own files, read from the
//! reference PCs' disks (95's Dial-Up Networking from its CD) with `tools/dev/icons.ps1`:
//! - network: Dial-Up Networking's RNAAPP.EXE (95 and 98: two computers, icons 110 to 114) and
//!   Windows 2000's NETSHELL.DLL (190 to 193, 195). None of them knew Wi-Fi; the picture stands
//!   for whatever connection the PC uses.
//! - battery: 95's SYSTRAY.EXE; 98's and 2000's POWERCFG.CPL (the plug), BATMETER.DLL (its
//!   16-pixel strip of batteries) and 2000's STOBJECT.DLL (charging).
//!
//! A computer's screen lights while data moves, as on the originals (the back one sending, the
//! front one receiving: which is which is not documented, PROVISIONAL). Where the batteries sit
//! inside their 16×16 square in 98 and 2000 is not known either (they come from a strip, not
//! from icons): centered, PROVISIONAL.
//!
//! The pictures themselves are Windows' and kept in bevel-art (`art/src/tray.rs`); this
//! module chooses and draws them.

use crate::art::Look;
use crate::icons::ClassicIcon;
use crate::fb::Icon;
use crate::paint::{Menu, MenuEntry};
use crate::theme::Era;

/// What Windows says about the connection the PC reaches the internet through.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Network {
    pub connected: bool,
    /// The connection's name ("Wi-Fi (HomeNet)", "Ethernet").
    pub name: String,
    /// It is a cable (Windows 2000 then says "Network cable unplugged" when it is gone).
    pub cable: bool,
    pub speed_bps: u64,
    pub sent_packets: u64,
    pub received_packets: u64,
    /// Data went out or came in since the last look (the screens light up).
    pub sending: bool,
    pub receiving: bool,
}

/// What Windows says about the power supply (`None`: Windows does not know).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Power {
    /// The PC has a battery (the icon shows only then, as in Windows 95 to 2000).
    pub battery: bool,
    pub on_ac: Option<bool>,
    pub charging: bool,
    pub percent: Option<u8>,
    pub seconds_left: Option<u32>,
}

/// The network icon's picture.
pub fn network_icon(era: Era, n: &Network) -> Icon {
    let set: [&ClassicIcon; 5] = match era {
        Era::Win95 => [&WIN95_NET_IDLE, &WIN95_NET_SEND, &WIN95_NET_RECEIVE, &WIN95_NET_BOTH, &WIN95_NET_OFF],
        Era::Win98 => [&WIN98_NET_IDLE, &WIN98_NET_SEND, &WIN98_NET_RECEIVE, &WIN98_NET_BOTH, &WIN98_NET_OFF],
        Era::Win2000 => [&WIN2000_NET_IDLE, &WIN2000_NET_SEND, &WIN2000_NET_RECEIVE, &WIN2000_NET_BOTH, &WIN2000_NET_OFF],
    };
    let i = match (n.connected, n.sending, n.receiving) {
        (false, _, _) => 4,
        (true, false, false) => 0,
        (true, true, false) => 1,
        (true, false, true) => 2,
        (true, true, true) => 3,
    };
    set[i].icon()
}

/// The network icon's tooltip, in that Windows' words: Dial-Up Networking's "Connected to %s"
/// (95, 98), Windows 2000's name, speed and packet counts (NETSHELL.DLL strings 2202 to 2207).
/// "Not connected" is Bevel95's own: the dial-up icon was gone then.
pub fn network_tooltip(era: Era, n: &Network) -> String {
    match (era, n.connected) {
        (Era::Win2000, true) => format!(
            "{}\nSpeed: {}\nSent: {} packets\nReceived: {} packets",
            n.name,
            speed(n.speed_bps),
            grouped(n.sent_packets),
            grouped(n.received_packets)
        ),
        (Era::Win2000, false) if n.cable => format!("{}\nNetwork cable unplugged", n.name),
        (_, true) => format!("Connected to {}", n.name),
        (_, false) => "Not connected".to_string(),
    }
}

/// The network icon's menu: Dial-Up Networking's Status and Disconnect (95, 98; Disconnect is
/// grayed: Bevel95 does not hang up a connection), Windows 2000's Status and Open Network and
/// Dial-up Connections. Status (bold) is the double-click's.
pub fn network_menu(era: Era, show_keys: bool) -> Menu {
    let mut status = MenuEntry::item("&Status", None);
    status.bold = true;
    let second = if era == Era::Win2000 {
        MenuEntry::item("&Open Network and Dial-up Connections", None)
    } else {
        MenuEntry { disabled: true, ..MenuEntry::item("&Disconnect", None) }
    };
    Menu { show_keys, ..Menu::new(vec![status, second]) }
}

/// What the network menu's entry `index` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkCommand {
    Status,
    Connections,
}

pub fn network_command(era: Era, index: usize) -> Option<NetworkCommand> {
    match index {
        0 => Some(NetworkCommand::Status),
        1 if era == Era::Win2000 => Some(NetworkCommand::Connections),
        _ => None,
    }
}

/// The battery icon's picture.
pub fn power_icon(era: Era, p: &Power) -> Icon {
    let (ac, charging, high, medium, low, unknown) = match era {
        Era::Win95 => (
            &WIN95_POWER_AC,
            &WIN95_POWER_CHARGING,
            &WIN95_BATTERY_HIGH,
            &WIN95_BATTERY_MEDIUM,
            &WIN95_BATTERY_LOW,
            &WIN95_BATTERY_UNKNOWN,
        ),
        Era::Win98 | Era::Win2000 => (
            &WIN98_POWER_AC,
            if era == Era::Win2000 { &WIN2000_POWER_CHARGING } else { &WIN98_POWER_CHARGING },
            &WIN98_BATTERY_HIGH,
            &WIN98_BATTERY_MEDIUM,
            &WIN98_BATTERY_LOW,
            &WIN98_BATTERY_UNKNOWN,
        ),
    };
    let picture = match (p.on_ac, p.charging, p.percent) {
        (Some(true), true, _) => charging,
        (Some(true), false, _) => ac,
        (_, _, Some(n)) if n > 66 => high,
        (_, _, Some(n)) if n > 33 => medium,
        (_, _, Some(_)) => low,
        (_, _, None) => unknown,
    };
    picture.icon()
}

/// The battery icon's tooltip, in that Windows' words (95's SYSTRAY.EXE; 98's SYSTRAY.EXE and
/// 2000's STOBJECT.DLL, the same strings).
pub fn power_tooltip(era: Era, p: &Power) -> String {
    let percent = p.percent.map(u32::from);
    if era == Era::Win95 {
        return match (p.on_ac, p.charging, percent) {
            (Some(true), true, Some(n)) => format!("{n}% (charging)"),
            (Some(true), _, _) => "AC power".to_string(),
            (_, _, Some(n)) => format!("{n}% remaining"),
            (_, _, None) => "Battery remaining: unknown".to_string(),
        };
    }
    match (p.on_ac, p.charging, percent, p.seconds_left) {
        (Some(true), true, Some(n), _) => format!("{n}% remaining (charging)"),
        (Some(true), _, _, _) => "On AC power".to_string(),
        (_, _, Some(n), Some(s)) if s >= 3600 => format!("{}:{:02} hours ({n}%) remaining", s / 3600, s % 3600 / 60),
        (_, _, Some(n), Some(s)) => format!("{} min ({n}%) remaining", s / 60),
        (_, _, Some(n), None) => format!("{n}% remaining"),
        (_, _, None, _) => "Unknown remaining".to_string(),
    }
}

/// The battery icon's menu: Adjust Power Properties and Open Power Meter (95: Open Battery
/// Meter). 95's three check items (low battery warning, time or percentage) are left out.
pub fn power_menu(era: Era, show_keys: bool) -> Menu {
    let meter = if era == Era::Win95 { "&Open Battery Meter" } else { "&Open Power Meter" };
    Menu { show_keys, ..Menu::new(vec![MenuEntry::item("&Adjust Power Properties", None), MenuEntry::item(meter, None)]) }
}

/// A link speed as Windows 2000 wrote it ("100.0 Mbps").
fn speed(bps: u64) -> String {
    match bps {
        b if b >= 1_000_000_000 => format!("{:.1} Gbps", b as f64 / 1e9),
        b if b >= 1_000_000 => format!("{:.1} Mbps", b as f64 / 1e6),
        b if b >= 1_000 => format!("{:.1} Kbps", b as f64 / 1e3),
        b => format!("{b} bps"),
    }
}

/// A count with thousands separators ("12,345"), as Windows wrote counts in English.
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

static WIN95_NET_IDLE: ClassicIcon = ClassicIcon::art("tray/net-idle", Look::Win95);

static WIN95_NET_SEND: ClassicIcon = ClassicIcon::art("tray/net-send", Look::Win95);

static WIN95_NET_RECEIVE: ClassicIcon = ClassicIcon::art("tray/net-receive", Look::Win95);

static WIN95_NET_BOTH: ClassicIcon = ClassicIcon::art("tray/net-both", Look::Win95);

static WIN95_NET_OFF: ClassicIcon = ClassicIcon::art("tray/net-off", Look::Win95);

static WIN98_NET_IDLE: ClassicIcon = ClassicIcon::art("tray/net-idle", Look::Win98);

static WIN98_NET_SEND: ClassicIcon = ClassicIcon::art("tray/net-send", Look::Win98);

static WIN98_NET_RECEIVE: ClassicIcon = ClassicIcon::art("tray/net-receive", Look::Win98);

static WIN98_NET_BOTH: ClassicIcon = ClassicIcon::art("tray/net-both", Look::Win98);

static WIN98_NET_OFF: ClassicIcon = ClassicIcon::art("tray/net-off", Look::Win98);

static WIN2000_NET_IDLE: ClassicIcon = ClassicIcon::art("tray/net-idle", Look::Win2000);

static WIN2000_NET_SEND: ClassicIcon = ClassicIcon::art("tray/net-send", Look::Win2000);

static WIN2000_NET_RECEIVE: ClassicIcon = ClassicIcon::art("tray/net-receive", Look::Win2000);

static WIN2000_NET_BOTH: ClassicIcon = ClassicIcon::art("tray/net-both", Look::Win2000);

static WIN2000_NET_OFF: ClassicIcon = ClassicIcon::art("tray/net-off", Look::Win2000);

static WIN95_POWER_AC: ClassicIcon = ClassicIcon::art("tray/power-ac", Look::Win95);

static WIN95_POWER_CHARGING: ClassicIcon = ClassicIcon::art("tray/power-charging", Look::Win95);

static WIN95_BATTERY_HIGH: ClassicIcon = ClassicIcon::art("tray/battery-high", Look::Win95);

static WIN95_BATTERY_MEDIUM: ClassicIcon = ClassicIcon::art("tray/battery-medium", Look::Win95);

static WIN95_BATTERY_LOW: ClassicIcon = ClassicIcon::art("tray/battery-low", Look::Win95);

static WIN95_BATTERY_UNKNOWN: ClassicIcon = ClassicIcon::art("tray/battery-unknown", Look::Win95);

static WIN98_POWER_AC: ClassicIcon = ClassicIcon::art("tray/power-ac", Look::Win98);

static WIN98_POWER_CHARGING: ClassicIcon = ClassicIcon::art("tray/power-charging", Look::Win98);

static WIN2000_POWER_CHARGING: ClassicIcon = ClassicIcon::art("tray/power-charging", Look::Win2000);

static WIN98_BATTERY_HIGH: ClassicIcon = ClassicIcon::art("tray/battery-high", Look::Win98);

static WIN98_BATTERY_MEDIUM: ClassicIcon = ClassicIcon::art("tray/battery-medium", Look::Win98);

static WIN98_BATTERY_LOW: ClassicIcon = ClassicIcon::art("tray/battery-low", Look::Win98);

static WIN98_BATTERY_UNKNOWN: ClassicIcon = ClassicIcon::art("tray/battery-unknown", Look::Win98);

#[cfg(test)]
mod tests {
    use super::*;

    fn online() -> Network {
        Network {
            connected: true,
            name: "Wi-Fi (HomeNet)".into(),
            speed_bps: 866_700_000,
            sent_packets: 12_345,
            received_packets: 1_234_567,
            ..Network::default()
        }
    }

    #[test]
    fn network_pictures_follow_the_traffic() {
        let mut n = online();
        for era in Era::ALL {
            let idle = network_icon(era, &n);
            n.sending = true;
            let send = network_icon(era, &n);
            n.receiving = true;
            let both = network_icon(era, &n);
            n.sending = false;
            let receive = network_icon(era, &n);
            n.connected = false;
            let off = network_icon(era, &n);
            n.receiving = false;
            n.connected = true;
            let all = [&idle, &send, &both, &receive, &off];
            for (i, a) in all.iter().enumerate() {
                assert_eq!((a.width, a.height), (16, 16));
                for b in &all[i + 1..] {
                    assert_ne!(a.pixels, b.pixels, "{era:?}");
                }
            }
        }
    }

    #[test]
    fn network_tooltips_in_each_windows_words() {
        let n = online();
        assert_eq!(network_tooltip(Era::Win98, &n), "Connected to Wi-Fi (HomeNet)");
        assert_eq!(
            network_tooltip(Era::Win2000, &n),
            "Wi-Fi (HomeNet)\nSpeed: 866.7 Mbps\nSent: 12,345 packets\nReceived: 1,234,567 packets"
        );
        let cable = Network { connected: false, cable: true, name: "Ethernet".into(), ..Network::default() };
        assert_eq!(network_tooltip(Era::Win2000, &cable), "Ethernet\nNetwork cable unplugged");
        assert_eq!(network_tooltip(Era::Win95, &cable), "Not connected");
        assert_eq!(speed(100_000_000), "100.0 Mbps");
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1000), "1,000");
    }

    #[test]
    fn battery_pictures_and_tooltips() {
        let on_battery = |percent, seconds| Power {
            battery: true,
            on_ac: Some(false),
            charging: false,
            percent: Some(percent),
            seconds_left: seconds,
        };
        assert_eq!(power_tooltip(Era::Win2000, &on_battery(85, Some(5400))), "1:30 hours (85%) remaining");
        assert_eq!(power_tooltip(Era::Win2000, &on_battery(20, Some(900))), "15 min (20%) remaining");
        assert_eq!(power_tooltip(Era::Win98, &on_battery(85, None)), "85% remaining");
        assert_eq!(power_tooltip(Era::Win95, &on_battery(85, Some(5400))), "85% remaining");
        let charging = Power { on_ac: Some(true), charging: true, ..on_battery(40, None) };
        assert_eq!(power_tooltip(Era::Win2000, &charging), "40% remaining (charging)");
        assert_eq!(power_tooltip(Era::Win95, &charging), "40% (charging)");
        let full = Power { on_ac: Some(true), ..on_battery(100, None) };
        assert_eq!(power_tooltip(Era::Win2000, &full), "On AC power");
        assert_eq!(power_tooltip(Era::Win95, &full), "AC power");
        for era in Era::ALL {
            let pictures = [
                power_icon(era, &on_battery(90, None)),
                power_icon(era, &on_battery(50, None)),
                power_icon(era, &on_battery(10, None)),
                power_icon(era, &charging),
                power_icon(era, &full),
                power_icon(era, &Power { percent: None, ..on_battery(0, None) }),
            ];
            for (i, a) in pictures.iter().enumerate() {
                for b in &pictures[i + 1..] {
                    assert_ne!(a.pixels, b.pixels, "{era:?}");
                }
            }
        }
    }

    #[test]
    fn menus() {
        assert_eq!(network_menu(Era::Win2000, false).entries.len(), 2);
        assert!(network_menu(Era::Win98, false).entries[1].disabled);
        assert_eq!(network_command(Era::Win98, 1), None);
        assert_eq!(network_command(Era::Win2000, 1), Some(NetworkCommand::Connections));
        assert_eq!(power_menu(Era::Win95, false).entries[1].label, "&Open Battery Meter");
    }
}

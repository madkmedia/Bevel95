//! What the network and battery icons show (`bevel_core::status`), read from Windows:
//! - the connection the PC reaches the internet through: IP Helper's best interface towards a
//!   public address, its counters (GetIfEntry2) and, for Wi-Fi, the network's name from the
//!   Network List Manager (NLM: not Windows' Wi-Fi API, whose network names Windows 11 keeps
//!   behind the location permission);
//! - the battery: GetSystemPowerStatus.

use std::ffi::c_void;
use std::ptr::null_mut;
use std::time::{Duration, Instant};

use bevel_core::status::{Network, Power};
use windows_sys::Win32::Foundation::SysFreeString;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetBestInterface, GetIfEntry2, GetIfTable2, MIB_IF_ROW2, MIB_IF_TABLE2,
};
use windows_sys::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::core::{BSTR, GUID, HRESULT};

use crate::audio::{method, release};

/// What the icons open: Windows 11's own list of Wi-Fi networks (its taskbar's), its network
/// status page, Network Connections (Control Panel), Power Options, and its battery page.
pub const WIFI_LIST: &str = "ms-availablenetworks:";
pub const NETWORK_STATUS: &str = "ms-settings:network-status";
pub const POWER_METER: &str = "ms-settings:batterysaver";

/// Network Connections, by its whole path (`shell::system_program`).
pub fn connections() -> String {
    bevel_winkit::shell::system_program("ncpa.cpl")
}

/// Power Options, by its whole path.
pub fn power_options() -> String {
    bevel_winkit::shell::system_program("powercfg.cpl")
}

/// Interface types (ipifcons.h): Ethernet and Wi-Fi.
const IF_TYPE_ETHERNET: u32 = 6;
const IF_TYPE_WIFI: u32 = 71;
/// MIB_IF_ROW2's flag bits: a real adapter (not a virtual one), with a connector.
const HARDWARE_INTERFACE: u8 = 0x1;
const CONNECTOR_PRESENT: u8 = 0x4;
/// NET_IF_MEDIA_CONNECT_STATE's MediaConnectStateConnected.
const MEDIA_CONNECTED: i32 = 1;
/// 8.8.8.8: any public address; the best interface towards it is the one used for the internet
/// (only the routing table is asked, nothing is sent).
const PUBLIC_ADDRESS: u32 = 0x0808_0808;
/// Packets between two looks (the bar looks every 200 ms) that light a screen. The originals lit
/// at every packet, but a dial-up line was quiet when nobody used it; today's PCs exchange a few
/// packets nearly every quarter second in the background (measured: 0 to 26), which would keep
/// the screens flickering all the time. Bevel95's own threshold: page loads and downloads light
/// them, background chatter does not.
pub const ACTIVITY: u64 = 16;
/// How often the Wi-Fi network's name is asked again (it is asked at once when the connection
/// changes).
const NAME_AGAIN: Duration = Duration::from_secs(10);

/// CLSID_NetworkListManager and IID_INetworkListManager (netlistmgr.h).
const CLSID_NETWORK_LIST_MANAGER: GUID = GUID::from_u128(0xDCB00C01_570F_4A9B_8D69_199FDBA5723B);
const IID_NETWORK_LIST_MANAGER: GUID = GUID::from_u128(0xDCB00000_570F_4A9B_8D69_199FDBA5723B);
/// Method slots after IUnknown's 3 and IDispatch's 4. INetworkListManager: GetNetworks,
/// GetNetwork, GetNetworkConnections (9). IEnumNetworkConnections: get__NewEnum, Next (8).
/// INetworkConnection: GetNetwork (7), ..., GetAdapterId (12). INetwork: GetName (7).
const GET_NETWORK_CONNECTIONS: usize = 9;
const NEXT: usize = 8;
const GET_NETWORK: usize = 7;
const GET_ADAPTER_ID: usize = 12;
const GET_NAME: usize = 7;

type GetObject = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;
type Next = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void, *mut u32) -> HRESULT;
type GetGuid = unsafe extern "system" fn(*mut c_void, *mut GUID) -> HRESULT;
type GetName = unsafe extern "system" fn(*mut c_void, *mut BSTR) -> HRESULT;

/// Reads the connection, remembering the last counters (to light the screens) and the Wi-Fi
/// network's name.
#[derive(Default)]
pub struct NetworkReader {
    /// The interface and its sent and received packet counts at the last look.
    last: Option<(u32, u64, u64)>,
    /// The adapter, its network's name, and when that was asked.
    name: Option<(GUID, Option<String>, Instant)>,
}

impl NetworkReader {
    /// The connection now. Data moved since the last call lights the screens.
    pub fn read(&mut self) -> Network {
        let mut index = 0;
        // SAFETY: a valid out-pointer.
        if unsafe { GetBestInterface(PUBLIC_ADDRESS, &mut index) } != 0 {
            self.last = None;
            return offline();
        }
        // SAFETY: a zeroed MIB_IF_ROW2 is valid input; GetIfEntry2 reads the index.
        let mut row: MIB_IF_ROW2 = unsafe { std::mem::zeroed() };
        row.InterfaceIndex = index;
        // SAFETY: as above.
        if unsafe { GetIfEntry2(&mut row) } != 0 {
            self.last = None;
            return offline();
        }
        let sent = row.OutUcastPkts + row.OutNUcastPkts;
        let received = row.InUcastPkts + row.InNUcastPkts;
        let (sending, receiving) = match self.last {
            Some((i, s, r)) if i == index => (sent >= s + ACTIVITY, received >= r + ACTIVITY),
            _ => (false, false),
        };
        self.last = Some((index, sent, received));
        let mut name = text(&row.Alias);
        if row.Type == IF_TYPE_WIFI
            && let Some(network) = self.network_name(row.InterfaceGuid)
            && network != name
        {
            name = format!("{name} ({network})");
        }
        Network {
            connected: true,
            name,
            cable: row.Type == IF_TYPE_ETHERNET,
            speed_bps: row.ReceiveLinkSpeed.max(row.TransmitLinkSpeed).min(u64::MAX / 2),
            sent_packets: sent,
            received_packets: received,
            sending,
            receiving,
        }
    }

    /// The name of the network the adapter is on (for Wi-Fi: usually the network's own name),
    /// asked again when the adapter changes and every `NAME_AGAIN`.
    fn network_name(&mut self, adapter: GUID) -> Option<String> {
        let fresh = self.name.as_ref().is_some_and(|(a, _, at)| same_guid(a, &adapter) && at.elapsed() < NAME_AGAIN);
        if !fresh {
            self.name = Some((adapter, nlm_name(adapter), Instant::now()));
        }
        self.name.as_ref().and_then(|(_, name, _)| name.clone())
    }
}

/// No route to the internet: the main adapter's name (Wi-Fi first, then a cable), for Windows
/// 2000's "Network cable unplugged".
fn offline() -> Network {
    let mut table: *mut MIB_IF_TABLE2 = null_mut();
    let mut best: Option<(u32, String)> = None;
    // SAFETY: GetIfTable2 allocates the table; it is read within its NumEntries and freed.
    unsafe {
        if GetIfTable2(&mut table) == 0 && !table.is_null() {
            let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
            for row in rows {
                let flags = row.InterfaceAndOperStatusFlags._bitfield;
                let real = flags & HARDWARE_INTERFACE != 0 && flags & CONNECTOR_PRESENT != 0;
                if real && (row.Type == IF_TYPE_WIFI || row.Type == IF_TYPE_ETHERNET) {
                    // Wi-Fi first; a cable that is plugged in (but has no route) after it
                    let rank = if row.Type == IF_TYPE_WIFI { 2 } else { 1 + u32::from(row.MediaConnectState == MEDIA_CONNECTED) };
                    if best.as_ref().is_none_or(|(r, _)| rank > *r) {
                        best = Some((rank, text(&row.Alias)));
                    }
                }
            }
            FreeMibTable(table as *const c_void);
        }
    }
    let cable = best.as_ref().is_some_and(|(rank, _)| *rank != 2);
    Network { connected: false, name: best.map(|(_, name)| name).unwrap_or_default(), cable, ..Network::default() }
}

/// The name of the network an adapter is connected to, from the Network List Manager. COM must
/// be initialized on this thread.
fn nlm_name(adapter: GUID) -> Option<String> {
    let mut manager: *mut c_void = null_mut();
    // SAFETY: every object is used through its documented method table and released; strings
    // returned as BSTR are freed.
    unsafe {
        let hr = CoCreateInstance(&CLSID_NETWORK_LIST_MANAGER, null_mut(), CLSCTX_ALL, &IID_NETWORK_LIST_MANAGER, &mut manager);
        if hr < 0 || manager.is_null() {
            return None;
        }
        let mut connections: *mut c_void = null_mut();
        let hr = method::<GetObject>(manager, GET_NETWORK_CONNECTIONS)(manager, &mut connections);
        release(manager);
        if hr < 0 || connections.is_null() {
            return None;
        }
        let mut found = None;
        loop {
            let (mut connection, mut fetched): (*mut c_void, u32) = (null_mut(), 0);
            if method::<Next>(connections, NEXT)(connections, 1, &mut connection, &mut fetched) != 0 || fetched == 0 {
                break;
            }
            let mut id = GUID::from_u128(0);
            let ours = method::<GetGuid>(connection, GET_ADAPTER_ID)(connection, &mut id) >= 0 && same_guid(&id, &adapter);
            if ours {
                let mut network: *mut c_void = null_mut();
                if method::<GetObject>(connection, GET_NETWORK)(connection, &mut network) >= 0 && !network.is_null() {
                    let mut name: BSTR = null_mut();
                    if method::<GetName>(network, GET_NAME)(network, &mut name) >= 0 && !name.is_null() {
                        let len = (0..).take_while(|&i| *name.add(i) != 0).count();
                        found = Some(String::from_utf16_lossy(std::slice::from_raw_parts(name, len)));
                        SysFreeString(name);
                    }
                    release(network);
                }
            }
            release(connection);
            if ours {
                break;
            }
        }
        release(connections);
        found.filter(|n| !n.is_empty())
    }
}

fn same_guid(a: &GUID, b: &GUID) -> bool {
    (a.data1, a.data2, a.data3, a.data4) == (b.data1, b.data2, b.data3, b.data4)
}

fn text(chars: &[u16]) -> String {
    let len = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
    String::from_utf16_lossy(&chars[..len])
}

/// The power supply and battery now.
pub fn power() -> Power {
    let mut s = SYSTEM_POWER_STATUS::default();
    // SAFETY: a valid out-pointer.
    if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
        return Power::default();
    }
    Power {
        // 128: no system battery; 255: unknown status
        battery: s.BatteryFlag != 128 && s.BatteryFlag != 255,
        on_ac: match s.ACLineStatus {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        charging: s.BatteryFlag & 8 != 0,
        percent: (s.BatteryLifePercent <= 100).then_some(s.BatteryLifePercent),
        seconds_left: (s.BatteryLifeTime != u32::MAX).then_some(s.BatteryLifeTime),
    }
}

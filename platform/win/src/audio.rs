//! The speakers' volume, for the classic volume control: Windows' Core Audio, the default
//! output device's IAudioEndpointVolume. Called through the documented COM interfaces' method
//! tables, like the shell's icon list in start_menu.rs (windows-sys has the class id and
//! constants but no COM wrappers; no other crate is needed).

use std::ffi::c_void;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Media::Audio::{MMDeviceEnumerator, eMultimedia, eRender};
use windows_sys::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
use windows_sys::core::{GUID, HRESULT};

use crate::log::log;

/// IID_IMMDeviceEnumerator and IID_IAudioEndpointVolume, from mmdeviceapi.h and
/// endpointvolume.h.
const IID_DEVICE_ENUMERATOR: GUID = GUID::from_u128(0xA95664D2_9614_4F35_A746_DE8DB63617E6);
const IID_ENDPOINT_VOLUME: GUID = GUID::from_u128(0x5CDF2C82_841E_4546_9722_0CF74078229A);

/// Method slots: every interface starts with IUnknown's QueryInterface, AddRef, Release.
/// IMMDeviceEnumerator: EnumAudioEndpoints, GetDefaultAudioEndpoint. IMMDevice: Activate.
/// IAudioEndpointVolume: Register-/UnregisterControlChangeNotify, GetChannelCount,
/// SetMasterVolumeLevel, SetMasterVolumeLevelScalar, GetMasterVolumeLevel,
/// GetMasterVolumeLevelScalar, four channel methods, SetMute, GetMute.
const GET_DEFAULT_AUDIO_ENDPOINT: usize = 4;
const ACTIVATE: usize = 3;
const SET_MASTER_SCALAR: usize = 7;
const GET_MASTER_SCALAR: usize = 9;
const SET_MUTE: usize = 14;
const GET_MUTE: usize = 15;

type GetDefaultAudioEndpoint = unsafe extern "system" fn(*mut c_void, i32, i32, *mut *mut c_void) -> HRESULT;
type Activate = unsafe extern "system" fn(*mut c_void, *const GUID, u32, *const c_void, *mut *mut c_void) -> HRESULT;
type SetScalar = unsafe extern "system" fn(*mut c_void, f32, *const GUID) -> HRESULT;
type GetScalar = unsafe extern "system" fn(*mut c_void, *mut f32) -> HRESULT;
type SetMute = unsafe extern "system" fn(*mut c_void, i32, *const GUID) -> HRESULT;
type GetMute = unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT;

/// COM method calls, shared with the other programs (`bevel_winkit::com`).
pub(crate) use bevel_winkit::com::{method, release};

/// Makes COM usable on the calling thread (Core Audio and the shell's icons need it): the
/// bar's, and the one reading the Start menu's icons ahead. Harmless to repeat.
pub fn init() {
    // SAFETY: plain call; an apartment-threaded thread fits a window thread.
    unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
}

/// The default output device's volume control, as it is when opened (opened afresh each time
/// it is needed, so a new default device is followed).
pub struct Volume(*mut c_void);

impl Volume {
    /// Opens the default output device's volume, or `None` if there is no such device.
    pub fn open() -> Option<Volume> {
        // SAFETY: documented COM calls with valid in/out pointers; every reference obtained is
        // released (the volume's when the `Volume` is dropped).
        unsafe {
            let mut enumerator: *mut c_void = null_mut();
            let hr = CoCreateInstance(&MMDeviceEnumerator, null_mut(), CLSCTX_INPROC_SERVER, &IID_DEVICE_ENUMERATOR, &mut enumerator);
            if hr != 0 || enumerator.is_null() {
                log(&format!("volume: no audio device list (error {hr:#x})"));
                return None;
            }
            let mut device: *mut c_void = null_mut();
            let get_default = method::<GetDefaultAudioEndpoint>(enumerator, GET_DEFAULT_AUDIO_ENDPOINT);
            let hr = get_default(enumerator, eRender, eMultimedia, &mut device);
            release(enumerator);
            if hr != 0 || device.is_null() {
                log(&format!("volume: no default output device (error {hr:#x})"));
                return None;
            }
            let mut volume: *mut c_void = null_mut();
            let hr = method::<Activate>(device, ACTIVATE)(device, &IID_ENDPOINT_VOLUME, CLSCTX_INPROC_SERVER, null(), &mut volume);
            release(device);
            if hr != 0 || volume.is_null() {
                log(&format!("volume: the output device has no volume control (error {hr:#x})"));
                return None;
            }
            Some(Volume(volume))
        }
    }

    /// The volume, 0.0 to 1.0 (as Windows' own slider shows it).
    pub fn level(&self) -> f32 {
        let mut level = 0.0;
        // SAFETY: a live IAudioEndpointVolume and a valid out-pointer.
        unsafe { method::<GetScalar>(self.0, GET_MASTER_SCALAR)(self.0, &mut level) };
        level
    }

    pub fn set_level(&self, level: f32) {
        // SAFETY: a live IAudioEndpointVolume; no event context.
        let hr = unsafe { method::<SetScalar>(self.0, SET_MASTER_SCALAR)(self.0, level.clamp(0.0, 1.0), null()) };
        if hr != 0 {
            log(&format!("volume: could not set the volume (error {hr:#x})"));
        }
    }

    pub fn muted(&self) -> bool {
        let mut muted = 0;
        // SAFETY: a live IAudioEndpointVolume and a valid out-pointer.
        unsafe { method::<GetMute>(self.0, GET_MUTE)(self.0, &mut muted) };
        muted != 0
    }

    pub fn set_muted(&self, muted: bool) {
        // SAFETY: a live IAudioEndpointVolume; no event context.
        let hr = unsafe { method::<SetMute>(self.0, SET_MUTE)(self.0, i32::from(muted), null()) };
        if hr != 0 {
            log(&format!("volume: could not switch mute (error {hr:#x})"));
        }
    }
}

impl Drop for Volume {
    fn drop(&mut self) {
        // SAFETY: the reference Volume::open got.
        unsafe { release(self.0) };
    }
}

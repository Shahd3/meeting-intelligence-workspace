//! Windows-only WASAPI adapter. Every COM object stays on its owning capture thread.
use super::{Device, Devices, Selection};
use windows::{
    core::{Interface, PCWSTR},
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Media::Audio::*,
        System::{
            Com::{
                StructuredStorage::{PropVariantClear, PropVariantToStringAlloc},
                *,
            },
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        },
    },
};

pub struct Com;
impl Com {
    pub fn enter() -> Result<Self, String> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }
            .map_err(|_| "Windows audio initialization failed.")?;
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() }
    }
}
fn audio_error(e: windows::core::Error) -> String {
    // HRESULT is useful for diagnosis without exposing endpoint names or captured content.
    format!("Windows audio operation failed ({:#010x}). Check the selected device, desktop microphone permission, and exclusive-mode use. No alternate source was substituted.", e.code().0 as u32)
}
pub fn now() -> Result<u64, String> {
    let (mut count, mut frequency) = (0i64, 0i64);
    unsafe {
        QueryPerformanceCounter(&mut count).map_err(audio_error)?;
        QueryPerformanceFrequency(&mut frequency).map_err(audio_error)?;
    }
    if frequency <= 0 || count < 0 {
        return Err("Windows audio clock is unavailable.".into());
    }
    Ok((count as u128 * 10_000_000 / frequency as u128) as u64)
}
fn enumerator() -> Result<IMMDeviceEnumerator, String> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(audio_error)
}
fn descriptor(device: &IMMDevice) -> Result<Device, String> {
    unsafe {
        let pointer = device.GetId().map_err(audio_error)?;
        let id = pointer.to_string();
        CoTaskMemFree(Some(pointer.0.cast()));
        let id = id.map_err(|_| "Unable to read audio device identity.")?;
        let store = device.OpenPropertyStore(STGM_READ).map_err(audio_error)?;
        let mut value = store
            .GetValue(&PKEY_Device_FriendlyName)
            .map_err(audio_error)?;
        let pointer = PropVariantToStringAlloc(&value);
        let _ = PropVariantClear(&mut value);
        let pointer = pointer.map_err(audio_error)?;
        let name = pointer.to_string();
        CoTaskMemFree(Some(pointer.0.cast()));
        let name = name.map_err(|_| "Unable to read audio device name.")?;
        Ok(Device { id, name })
    }
}
pub fn devices() -> Result<Devices, String> {
    let _com = Com::enter()?;
    let enumerator = enumerator()?;
    let mut result = Devices {
        microphones: vec![],
        outputs: vec![],
    };
    for (direction, list) in [
        (eCapture, &mut result.microphones),
        (eRender, &mut result.outputs),
    ] {
        let collection = unsafe { enumerator.EnumAudioEndpoints(direction, DEVICE_STATE_ACTIVE) }
            .map_err(audio_error)?;
        let count = unsafe { collection.GetCount() }.map_err(audio_error)?;
        for index in 0..count {
            list.push(descriptor(
                &unsafe { collection.Item(index) }.map_err(audio_error)?,
            )?);
        }
    }
    Ok(result)
}
pub struct Packet {
    pub pcm: Vec<u8>,
    pub timestamp: u64,
    pub discontinuity: bool,
}
pub struct Source {
    pub role: String,
    pub device: Device,
    client: IAudioClient,
    capture: IAudioCaptureClient,
}
impl Source {
    pub fn open(role: &str, id: Option<&str>) -> Result<Self, String> {
        let enumerator = enumerator()?;
        let direction = if role == "microphone" {
            eCapture
        } else {
            eRender
        };
        let device = if let Some(id) = id.filter(|id| !id.is_empty()) {
            let wide: Vec<u16> = id.encode_utf16().chain([0]).collect();
            unsafe { enumerator.GetDevice(PCWSTR(wide.as_ptr())) }.map_err(audio_error)?
        } else {
            unsafe { enumerator.GetDefaultAudioEndpoint(direction, eConsole) }
                .map_err(audio_error)?
        };
        let endpoint: IMMEndpoint = device.cast().map_err(audio_error)?;
        if unsafe { endpoint.GetDataFlow() }.map_err(audio_error)? != direction {
            return Err("Selected audio endpoint has the wrong direction.".into());
        }
        let descriptor = descriptor(&device)?;
        let client: IAudioClient =
            unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(audio_error)?;
        let format = WAVEFORMATEX {
            wFormatTag: 1,
            nChannels: 2,
            nSamplesPerSec: 48_000,
            nAvgBytesPerSec: 192_000,
            nBlockAlign: 4,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let flags = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
            | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
            | if role == "system" {
                AUDCLNT_STREAMFLAGS_LOOPBACK
            } else {
                0
            };
        unsafe { client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 2_000_000, 0, &format, None) }
            .map_err(audio_error)?;
        let capture = unsafe { client.GetService() }.map_err(audio_error)?;
        Ok(Self {
            role: role.into(),
            device: descriptor,
            client,
            capture,
        })
    }
    pub fn start(&self) -> Result<(), String> {
        unsafe { self.client.Start() }.map_err(audio_error)
    }
    pub fn pause(&self) -> Result<(), String> {
        unsafe { self.client.Stop() }.map_err(audio_error)
    }
    pub fn reset(&self) -> Result<(), String> {
        unsafe { self.client.Reset() }.map_err(audio_error)
    }
    pub fn alive(&self) -> Result<(), String> {
        // Do not follow default-device changes or replace an unplugged endpoint mid-session.
        let enumerator = enumerator()?;
        let wide: Vec<u16> = self.device.id.encode_utf16().chain([0]).collect();
        let device = unsafe { enumerator.GetDevice(PCWSTR(wide.as_ptr())) }.map_err(audio_error)?;
        if unsafe { device.GetState() }.map_err(audio_error)? != DEVICE_STATE_ACTIVE {
            return Err("A selected recording device became unavailable. Available audio has been retained.".into());
        }
        Ok(())
    }
    pub fn packet(&self) -> Result<Option<Packet>, String> {
        let available = unsafe { self.capture.GetNextPacketSize() }.map_err(audio_error)?;
        if available == 0 {
            return Ok(None);
        }
        let (mut pointer, mut frames, mut flags, mut timestamp) =
            (std::ptr::null_mut(), 0u32, 0u32, 0u64);
        unsafe {
            self.capture.GetBuffer(
                &mut pointer,
                &mut frames,
                &mut flags,
                None,
                Some(&mut timestamp),
            )
        }
        .map_err(audio_error)?;
        // Copy only valid buffers. SILENT permits a null pointer; never dereference it.
        let result: Result<Vec<u8>, String> = if frames > 48_000 {
            Err("Windows returned an unexpectedly large audio packet.".into())
        } else if flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0 {
            Err("Windows reported an invalid audio timestamp; audio was retained without guessing synchronization.".into())
        } else if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
            Ok(vec![0; frames as usize * 4])
        } else if frames == 0 {
            Ok(vec![])
        } else if pointer.is_null() {
            Err("Windows returned an invalid audio buffer.".into())
        } else {
            Ok(unsafe { std::slice::from_raw_parts(pointer, frames as usize * 4) }.to_vec())
        };
        unsafe { self.capture.ReleaseBuffer(frames) }.map_err(audio_error)?;
        let pcm = result?;
        Ok(Some(Packet {
            pcm,
            timestamp,
            discontinuity: flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0,
        }))
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        let _ = unsafe { self.client.Stop() };
    }
}
pub fn open(selection: &Selection) -> Result<Vec<Source>, String> {
    let mut sources = vec![];
    if selection.microphone {
        sources.push(Source::open(
            "microphone",
            selection.microphone_device.as_deref(),
        )?);
    }
    if selection.system {
        sources.push(Source::open("system", selection.system_device.as_deref())?);
    }
    Ok(sources)
}

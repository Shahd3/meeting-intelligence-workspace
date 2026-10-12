//! Windows OS-backed secrets. No plaintext fallback, enumeration, Debug, or serialization.
use zeroize::Zeroizing;
pub trait Vault {
    fn read(&self, id: &str) -> Result<Option<Zeroizing<String>>, String>;
    fn write(&self, id: &str, key: &str) -> Result<(), String>;
    fn delete(&self, id: &str) -> Result<(), String>;
}
pub struct WindowsVault;
fn target(id: &str) -> Result<Vec<u16>, String> {
    let uuid = uuid::Uuid::parse_str(id).map_err(|_| "Invalid credential reference.")?;
    Ok(format!("MeetingIntelligenceWorkspace/AI/{uuid}")
        .encode_utf16()
        .chain(Some(0))
        .collect())
}
pub fn valid_key(key: &str) -> Result<(), String> {
    if key.is_empty() || key.len() > 2500 || key.chars().any(char::is_control) {
        return Err(
            "Enter a valid API key without control characters (maximum 2500 bytes).".into(),
        );
    }
    Ok(())
}
#[cfg(windows)]
impl Vault for WindowsVault {
    fn read(&self, id: &str) -> Result<Option<Zeroizing<String>>, String> {
        use windows::{
            core::PCWSTR,
            Win32::{Foundation::ERROR_NOT_FOUND, Security::Credentials::*},
        };
        let name = target(id)?;
        let mut pointer = std::ptr::null_mut();
        unsafe {
            if let Err(error) =
                CredReadW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, None, &mut pointer)
            {
                return if error.code() == ERROR_NOT_FOUND.to_hresult() {
                    Ok(None)
                } else {
                    Err("Windows Credential Manager could not read the saved key.".into())
                };
            }
            let record = &*pointer;
            let result = if record.CredentialBlobSize == 0
                || record.CredentialBlobSize > 2500
                || record.CredentialBlob.is_null()
            {
                Err("The saved credential is invalid.".into())
            } else {
                let bytes = std::slice::from_raw_parts(
                    record.CredentialBlob,
                    record.CredentialBlobSize as usize,
                );
                String::from_utf8(bytes.to_vec())
                    .map(Zeroizing::new)
                    .map(Some)
                    .map_err(|_| "The saved credential is invalid.".into())
            };
            if !record.CredentialBlob.is_null() {
                std::slice::from_raw_parts_mut(
                    record.CredentialBlob,
                    record.CredentialBlobSize as usize,
                )
                .fill(0);
            }
            CredFree(pointer as *const _);
            result
        }
    }
    fn write(&self, id: &str, key: &str) -> Result<(), String> {
        use windows::{core::PWSTR, Win32::Security::Credentials::*};
        valid_key(key)?;
        let mut name = target(id)?;
        let mut bytes = Zeroizing::new(key.as_bytes().to_vec());
        let record = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(name.as_mut_ptr()),
            CredentialBlobSize: bytes.len() as u32,
            CredentialBlob: bytes.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        unsafe { CredWriteW(&record, 0) }.map_err(|_| {
            "Windows Credential Manager could not save the key. Settings were not changed.".into()
        })
    }
    fn delete(&self, id: &str) -> Result<(), String> {
        use windows::{
            core::PCWSTR,
            Win32::{Foundation::ERROR_NOT_FOUND, Security::Credentials::*},
        };
        let name = target(id)?;
        match unsafe { CredDeleteW(PCWSTR(name.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERROR_NOT_FOUND.to_hresult() => Ok(()),
            Err(_) => Err("Windows Credential Manager could not delete the saved key.".into()),
        }
    }
}
#[cfg(not(windows))]
impl Vault for WindowsVault {
    fn read(&self, _: &str) -> Result<Option<Zeroizing<String>>, String> {
        Err("Remote credentials currently require Windows Credential Manager.".into())
    }
    fn write(&self, _: &str, _: &str) -> Result<(), String> {
        Err("Remote credentials currently require Windows Credential Manager.".into())
    }
    fn delete(&self, _: &str) -> Result<(), String> {
        Err("Remote credentials currently require Windows Credential Manager.".into())
    }
}

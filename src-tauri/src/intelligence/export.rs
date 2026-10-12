//! Export only to a path selected by the user in the native Save dialog.
#[cfg(windows)]
pub fn markdown(contents: String) -> Result<bool, String> {
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::UI::Controls::Dialogs::{
            CommDlgExtendedError, GetSaveFileNameW, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT,
            OFN_PATHMUSTEXIST, OPENFILENAMEW,
        },
    };
    if contents.len() > 1024 * 1024 {
        return Err("Minutes exceed the export size limit.".into());
    }
    let mut buffer = vec![0u16; 32768];
    let name: Vec<u16> = "meeting-minutes.md".encode_utf16().collect();
    buffer[..name.len()].copy_from_slice(&name);
    let filter: Vec<u16> = "Markdown (*.md)\0*.md\0\0".encode_utf16().collect();
    let extension: Vec<u16> = "md\0".encode_utf16().collect();
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrDefExt: PCWSTR(extension.as_ptr()),
        Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // This blocking call runs off the Tauri async executor.
    if !unsafe { GetSaveFileNameW(&mut dialog) }.as_bool() {
        return if unsafe { CommDlgExtendedError().0 } == 0 {
            Ok(false)
        } else {
            Err("Unable to open the Markdown save dialog.".into())
        };
    }
    let end = buffer
        .iter()
        .position(|c| *c == 0)
        .ok_or("Invalid export path.")?;
    let path = String::from_utf16(&buffer[..end]).map_err(|_| "Invalid export path.")?;
    std::fs::write(path, contents.as_bytes())
        .map_err(|_| "Unable to write Minutes to the selected file.")?;
    Ok(true)
}
#[cfg(not(windows))]
pub fn markdown(_: String) -> Result<bool, String> {
    Err("Native Markdown export currently requires Windows.".into())
}

//! Bounded PCM WAV writes; repair derives the payload length, never trusts a stale header.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
pub const RATE: u32 = 48_000;
pub const ALIGN: u64 = 4; // stereo PCM16
pub const MAX_FRAMES: u64 = RATE as u64 * 60 * 60 * 4;
pub type Result<T> = std::result::Result<T, String>;
fn io_error(_: std::io::Error) -> String {
    "Unable to preserve recording audio. Check disk space and file access.".into()
}

pub struct Wav {
    file: File,
    pub frames: u64,
}
impl Wav {
    pub fn create(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(io_error)?;
        let mut writer = Self { file, frames: 0 };
        writer.checkpoint()?;
        Ok(writer)
    }
    pub fn append_at(&mut self, target: u64, pcm: &[u8]) -> Result<()> {
        if pcm.len() as u64 % ALIGN != 0 {
            return Err("Invalid audio frame alignment.".into());
        }
        let incoming = pcm.len() as u64 / ALIGN;
        if target.saturating_add(incoming) > MAX_FRAMES {
            return Err("The four-hour WAV recording limit was reached.".into());
        }
        self.pad_to(target)?;
        let skipped = self.frames.saturating_sub(target).min(incoming);
        self.file.seek(SeekFrom::End(0)).map_err(io_error)?;
        self.file
            .write_all(&pcm[(skipped * ALIGN) as usize..])
            .map_err(io_error)?;
        self.frames += incoming - skipped;
        Ok(())
    }
    pub fn pad_to(&mut self, target: u64) -> Result<()> {
        if target > MAX_FRAMES {
            return Err("Recording exceeds the WAV size limit.".into());
        }
        self.file.seek(SeekFrom::End(0)).map_err(io_error)?;
        let silence = [0u8; 8192];
        while self.frames < target {
            let frames = (target - self.frames).min(silence.len() as u64 / ALIGN);
            self.file
                .write_all(&silence[..(frames * ALIGN) as usize])
                .map_err(io_error)?;
            self.frames += frames;
        }
        Ok(())
    }
    pub fn checkpoint(&mut self) -> Result<()> {
        self.file.seek(SeekFrom::Start(0)).map_err(io_error)?;
        self.file
            .write_all(&header(self.frames)?)
            .map_err(io_error)?;
        self.file.sync_all().map_err(io_error)
    }
    pub fn finish(mut self, frames: u64) -> Result<u64> {
        self.pad_to(frames)?;
        if self.frames > frames {
            self.file.set_len(44 + frames * ALIGN).map_err(io_error)?;
            self.frames = frames;
        }
        self.checkpoint()?;
        Ok(self.frames)
    }
}
pub fn header(frames: u64) -> Result<[u8; 44]> {
    if frames > MAX_FRAMES {
        return Err("Recording exceeds the WAV size limit.".into());
    }
    let mut b = [0u8; 44];
    b[0..4].copy_from_slice(b"RIFF");
    b[4..8].copy_from_slice(&(36 + frames as u32 * 4).to_le_bytes());
    b[8..16].copy_from_slice(b"WAVEfmt ");
    b[16..20].copy_from_slice(&16u32.to_le_bytes());
    b[20..22].copy_from_slice(&1u16.to_le_bytes());
    b[22..24].copy_from_slice(&2u16.to_le_bytes());
    b[24..28].copy_from_slice(&RATE.to_le_bytes());
    b[28..32].copy_from_slice(&(RATE * 4).to_le_bytes());
    b[32..34].copy_from_slice(&4u16.to_le_bytes());
    b[34..36].copy_from_slice(&16u16.to_le_bytes());
    b[36..40].copy_from_slice(b"data");
    b[40..44].copy_from_slice(&(frames as u32 * 4).to_le_bytes());
    Ok(b)
}
pub fn inspect(path: &Path) -> Result<u64> {
    let mut file = File::open(path).map_err(io_error)?;
    let length = file.metadata().map_err(io_error)?.len();
    let mut b = [0u8; 44];
    file.read_exact(&mut b).map_err(io_error)?;
    // Compare immutable fields; allow stale RIFF/data lengths after an interrupted write.
    let template = header(0)?;
    if b[0..4] != template[0..4] || b[8..40] != template[8..40] || length < 44 {
        return Err("Recording format is invalid; the original file was retained.".into());
    }
    let frames = (length - 44) / ALIGN;
    if frames > MAX_FRAMES {
        return Err("Recording is too large to recover safely.".into());
    }
    Ok(frames)
}
pub fn repair_copy(source: &Path, destination: &Path) -> Result<u64> {
    let frames = inspect(source)?;
    if destination.exists() {
        if inspect(destination)? == frames {
            return Ok(frames);
        }
        return Err("Recovery copy conflicts with the original; both were retained.".into());
    }
    let mut input = File::open(source).map_err(io_error)?;
    // A crash during copying may leave scratch bytes; retry only this derived copy.
    // The original and any completed recovery artifact are never overwritten.
    let scratch = destination.with_extension("wav.recovery.tmp");
    let mut output = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&scratch)
        .map_err(io_error)?;
    output.write_all(&header(frames)?).map_err(io_error)?;
    input.seek(SeekFrom::Start(44)).map_err(io_error)?;
    std::io::copy(&mut input.take(frames * ALIGN), &mut output).map_err(io_error)?;
    output.sync_all().map_err(io_error)?;
    drop(output);
    std::fs::rename(scratch, destination).map_err(io_error)?;
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps_pad_and_overlaps_trim_without_corrupting_frames() {
        let directory = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("test.part");
        let mut wav = Wav::create(&path).unwrap();
        wav.append_at(2, &[1, 0, 2, 0, 3, 0, 4, 0]).unwrap();
        wav.append_at(3, &[3, 0, 4, 0, 5, 0, 6, 0]).unwrap();
        assert_eq!(wav.finish(6).unwrap(), 6);
        assert_eq!(inspect(&path).unwrap(), 6);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[44..52], &[0; 8]);
        assert_eq!(bytes.len(), 68);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn crash_repair_preserves_original_and_ignores_stale_header_and_partial_frame() {
        let directory = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("original.part");
        let repaired = directory.join("recovered.wav");
        let mut bytes = header(0).unwrap().to_vec();
        bytes.extend([1, 0, 2, 0, 3]);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(repair_copy(&path, &repaired).unwrap(), 1);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(std::fs::read(&repaired).unwrap().len(), 48);
        assert_eq!(repair_copy(&path, &repaired).unwrap(), 1);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(repaired).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}

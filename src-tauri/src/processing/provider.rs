use super::{Segment, Source};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
pub const CHUNK_MS: u64 = 60_000;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub executable: String,
    pub model: String,
    pub language: String,
    #[serde(default)]
    pub executable_sha256: String,
    #[serde(default)]
    pub model_sha256: String,
}
#[derive(Debug)]
pub struct Failure {
    pub category: &'static str,
    pub message: String,
    pub retryable: bool,
    pub retry_after_secs: Option<u64>,
}
impl Failure {
    pub fn configuration(message: &str) -> Self {
        Self {
            category: "configuration",
            message: message.into(),
            retryable: false,
            retry_after_secs: None,
        }
    }
    pub fn input(message: &str) -> Self {
        Self {
            category: "input",
            message: message.into(),
            retryable: false,
            retry_after_secs: None,
        }
    }
    pub fn provider(message: &str) -> Self {
        Self {
            category: "provider",
            message: message.into(),
            retryable: true,
            retry_after_secs: None,
        }
    }
    pub fn interrupted() -> Self {
        Self {
            category: "interrupted",
            message: "Processing was interrupted. Audio and checkpoints were retained.".into(),
            retryable: true,
            retry_after_secs: None,
        }
    }
}
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self {
            category: "storage",
            message,
            retryable: true,
            retry_after_secs: None,
        }
    }
}
fn digest_file(path: &Path) -> std::result::Result<String, String> {
    let mut file = File::open(path).map_err(|_| "The executable or model cannot be read.")?;
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        let n = file
            .read(&mut bytes)
            .map_err(|_| "Unable to verify local model files.")?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
impl Config {
    pub fn validate_and_fingerprint(&mut self) -> std::result::Result<(), String> {
        for path in [&self.executable, &self.model] {
            if path.len() > 4096
                || path.contains('\0')
                || !Path::new(path).is_absolute()
                || !Path::new(path).is_file()
                || path.starts_with("\\\\")
            {
                return Err("Choose readable local files with absolute paths (network paths are not supported).".into());
            }
        }
        if !(self.language == "auto"
            || (self.language.len() == 2 && self.language.bytes().all(|c| c.is_ascii_lowercase())))
        {
            return Err(
                "Use auto or a two-letter lowercase language code, such as en or ar.".into(),
            );
        }
        #[cfg(windows)]
        if !self.executable.to_lowercase().ends_with("whisper-cli.exe") {
            return Err(
                "Select a trusted whisper-cli.exe from the official whisper.cpp release.".into(),
            );
        }
        self.executable_sha256 = digest_file(Path::new(&self.executable))?;
        self.model_sha256 = digest_file(Path::new(&self.model))?;
        Ok(())
    }
    pub fn verify(&self) -> std::result::Result<(), Failure> {
        let mut current = self.clone();
        current
            .validate_and_fingerprint()
            .map_err(|e| Failure::configuration(&e))?;
        if current.executable_sha256 != self.executable_sha256
            || current.model_sha256 != self.model_sha256
        {
            return Err(Failure::configuration("The configured runtime or model changed. Save configuration again to authorize the new files."));
        }
        Ok(())
    }
}
pub fn fingerprint(config: &Config, pcm: &[i16]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"miw-stt-v1-60s-box-filter");
    hash.update(&config.executable_sha256);
    hash.update(&config.model_sha256);
    hash.update(&config.language);
    for sample in pcm {
        hash.update(sample.to_le_bytes());
    }
    format!("{:x}", hash.finalize())
}
/// Fixed-size reads from the recorder's canonical PCM16/48kHz/stereo WAV.
/// Three stereo frames are averaged into one 16kHz mono frame; timeline is retained.
pub struct Audio {
    file: File,
    remaining: u64,
}
impl Audio {
    pub fn open(path: &Path, frames: u64) -> std::result::Result<Self, Failure> {
        let mut file = File::open(path)
            .map_err(|_| Failure::input("A source WAV cannot be read. Audio was retained."))?;
        let mut header = [0u8; 44];
        file.read_exact(&mut header)
            .map_err(|_| Failure::input("A source WAV is truncated."))?;
        let expected = crate::recording::wav::header(frames).map_err(|e| Failure::input(&e))?;
        if header != expected
            || file
                .metadata()
                .map_err(|_| Failure::input("Unable to inspect source WAV."))?
                .len()
                != 44 + frames * 4
        {
            return Err(Failure::input(
                "Source WAV format or length does not match finalized recording metadata.",
            ));
        }
        Ok(Self {
            file,
            remaining: frames * 4,
        })
    }
    pub fn next_chunk(&mut self) -> std::result::Result<Option<Vec<i16>>, Failure> {
        if self.remaining == 0 {
            return Ok(None);
        }
        let n = self.remaining.min(CHUNK_MS * 48_000 * 4 / 1000) as usize;
        let mut bytes = vec![0; n];
        self.file
            .read_exact(&mut bytes)
            .map_err(|_| Failure::input("Source audio became unreadable."))?;
        self.remaining -= n as u64;
        let mut mono = Vec::with_capacity(n / 12 + 1);
        for group in bytes.chunks(12) {
            let mut sum = 0i32;
            let mut count = 0;
            for pair in group.chunks_exact(2) {
                sum += i16::from_le_bytes([pair[0], pair[1]]) as i32;
                count += 1;
            }
            mono.push((sum / count) as i16);
        }
        Ok(Some(mono))
    }
}
fn write_wav(path: &Path, pcm: &[i16]) -> std::result::Result<(), Failure> {
    let mut header = crate::recording::wav::header(0).map_err(Failure::from)?;
    let len = pcm.len() as u32 * 2;
    header[4..8].copy_from_slice(&(36 + len).to_le_bytes());
    header[22..24].copy_from_slice(&1u16.to_le_bytes());
    header[24..28].copy_from_slice(&16000u32.to_le_bytes());
    header[28..32].copy_from_slice(&32000u32.to_le_bytes());
    header[32..34].copy_from_slice(&2u16.to_le_bytes());
    header[40..44].copy_from_slice(&len.to_le_bytes());
    let mut file = File::create(path)
        .map_err(|_| Failure::from(String::from("Unable to write derived audio.")))?;
    file.write_all(&header)
        .map_err(|_| Failure::from(String::from("Unable to write derived audio.")))?;
    let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    file.write_all(&bytes)
        .map_err(|_| Failure::from(String::from("Unable to write derived audio.")))?;
    Ok(())
}
pub trait Provider {
    fn transcribe(
        &self,
        pcm: &[i16],
        scratch: &Path,
        source: &str,
        offset: u64,
    ) -> std::result::Result<Vec<Segment>, Failure>;
}
pub struct WhisperCli<'a> {
    pub config: &'a Config,
    pub shutdown: &'a AtomicBool,
}
struct Process(Child);
#[cfg(windows)]
struct ProcessJob(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(windows)]
fn contain(child: &Child) -> std::result::Result<ProcessJob, Failure> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{Foundation::HANDLE, System::JobObjects::*};
    unsafe {
        let job = ProcessJob(CreateJobObjectW(None, None).map_err(|_| {
            Failure::configuration("Unable to create safe Windows transcription process ownership.")
        })?);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of_val(&limits) as u32,
        )
        .map_err(|_| {
            Failure::configuration(
                "Unable to configure safe Windows transcription process ownership.",
            )
        })?;
        AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle())).map_err(|_| {
            Failure::configuration(
                "Unable to contain the local transcription process. Audio was retained.",
            )
        })?;
        Ok(job)
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[derive(Deserialize)]
struct Output {
    transcription: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    offsets: Offsets,
    text: String,
}
#[derive(Deserialize)]
struct Offsets {
    from: u64,
    to: u64,
}
pub fn validate(
    segments: &[Segment],
    source: &str,
    offset: u64,
    duration: u64,
) -> std::result::Result<(), Failure> {
    let mut previous = offset;
    if segments.len() > 10000 {
        return Err(Failure::input("Too many transcript segments."));
    }
    for s in segments {
        if s.text.trim().is_empty()
            || s.text.len() > 10000
            || s.text.contains('\0')
            || s.sources.len() != 1
            || s.sources[0].role != source
            || s.sources[0].start_ms != s.start_ms
            || s.sources[0].end_ms != s.end_ms
        {
            return Err(Failure::input(
                "Provider returned invalid transcript segments.",
            ));
        }
        match (s.start_ms, s.end_ms) {
            (Some(start), Some(end))
                if start >= previous && end >= start && end <= offset + duration =>
            {
                previous = start;
            }
            (None, None) => {}
            _ => {
                return Err(Failure::input(
                    "Provider returned invalid transcript timestamps.",
                ))
            }
        }
    }
    Ok(())
}
impl Provider for WhisperCli<'_> {
    fn transcribe(
        &self,
        pcm: &[i16],
        scratch: &Path,
        source: &str,
        offset: u64,
    ) -> std::result::Result<Vec<Segment>, Failure> {
        // Digital silence is exact, not a claimed speech detector. Low-level audio
        // still goes to Whisper rather than silently dropping quiet speech.
        if pcm.iter().all(|s| *s == 0) {
            return Ok(vec![]);
        }
        let wav = scratch.join("chunk.wav");
        let output = scratch.join("output");
        let json = output.with_extension("json");
        if json.exists() {
            std::fs::remove_file(&json)
                .map_err(|_| Failure::from(String::from("Unable to clear derived output.")))?;
        }
        write_wav(&wav, pcm)?;
        let mut command = Command::new(&self.config.executable);
        command
            .args([
                "-m",
                &self.config.model,
                "-l",
                &self.config.language,
                "-t",
                "4",
                "-ng",
                "-oj",
                "-of",
            ])
            .arg(&output)
            .arg("-f")
            .arg(&wav)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = Process(command.spawn().map_err(|_| {
            Failure::configuration(
                "Unable to launch whisper-cli. Check its DLLs and Windows runtime requirements.",
            )
        })?);
        #[cfg(windows)]
        let _job = contain(&child.0)?;
        let deadline = Instant::now() + Duration::from_secs(600);
        loop {
            if self.shutdown.load(Ordering::Acquire) {
                return Err(Failure::interrupted());
            }
            if Instant::now() >= deadline {
                return Err(Failure::provider(
                    "Local transcription timed out after ten minutes for one chunk.",
                ));
            }
            if let Some(status) = child
                .0
                .try_wait()
                .map_err(|_| Failure::provider("Unable to monitor the transcription process."))?
            {
                if !status.success() {
                    return Err(Failure::provider("whisper-cli failed. Check model compatibility, runtime libraries, and available memory."));
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let file = File::open(&json)
            .map_err(|_| Failure::provider("whisper-cli did not produce JSON output."))?;
        if file
            .metadata()
            .map_err(|_| Failure::provider("Unable to inspect provider output."))?
            .len()
            > 4 * 1024 * 1024
        {
            return Err(Failure::input("Provider output exceeded its size limit."));
        }
        let output: Output = serde_json::from_reader(file)
            .map_err(|_| Failure::input("whisper-cli produced malformed output."))?;
        let duration = pcm.len() as u64 * 1000 / 16000;
        let mut segments = vec![];
        for entry in output.transcription {
            let text = entry.text.trim();
            if text.is_empty() {
                continue;
            }
            // Whisper may round its final 10ms timestamp to the next tick.
            if entry.offsets.to > duration + 20
                || entry.offsets.from > duration
                || entry.offsets.to < entry.offsets.from
            {
                return Err(Failure::input(
                    "Provider timestamps are outside the audio chunk.",
                ));
            }
            let start_ms = Some(offset + entry.offsets.from);
            let end_ms = Some(offset + entry.offsets.to.min(duration));
            segments.push(Segment {
                text: text.into(),
                start_ms,
                end_ms,
                sources: vec![Source {
                    role: source.into(),
                    start_ms,
                    end_ms,
                }],
            });
        }
        validate(&segments, source, offset, duration)?;
        Ok(segments)
    }
}

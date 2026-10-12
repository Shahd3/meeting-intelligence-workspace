use super::{
    credentials::{Vault, WindowsVault},
    provider::{Failure, Provider},
    settings::{self, Engine, Remote},
    Segment, Source,
};
use reqwest::blocking::{multipart, Client};
use serde_json::Value;
use std::{io::Read, path::Path, time::Duration};
pub fn client() -> Result<Client, Failure> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| Failure::configuration("Unable to initialize secure provider requests."))
}
pub fn status_failure(status: u16, retry_after: Option<u64>) -> Failure {
    let (category,message,retryable)=match status {
        401|403=>("authentication","The provider rejected the saved API key or access permissions.",false),
        429=>("rate_limit","The provider rate limit was reached. A bounded retry is scheduled when eligible.",true),
        408|500..=599=>("provider","The provider is temporarily unavailable.",true),
        300..=399=>("configuration","The provider attempted a redirect. Credentials and audio were not forwarded. Configure the final HTTPS API base URL.",false),
        _=>("unsupported","The provider rejected this API request. Check its model, endpoint, and supported response format.",false),
    };
    Failure {
        category,
        message: message.into(),
        retryable,
        retry_after_secs: retry_after.map(|s| s.min(900)),
    }
}
fn response(mut response: reqwest::blocking::Response) -> Result<Value, Failure> {
    if !response.status().is_success() {
        let retry = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        return Err(status_failure(response.status().as_u16(), retry));
    }
    let mut bytes = vec![];
    response
        .by_ref()
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::provider("Unable to read the provider response."))?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(Failure::input("Provider output exceeded its size limit."));
    }
    serde_json::from_slice(&bytes).map_err(|_| Failure {
        category: "unsupported",
        message: "The provider returned unsupported or malformed JSON.".into(),
        retryable: false,
        retry_after_secs: None,
    })
}
fn send_error(error: reqwest::Error) -> Failure {
    if error.is_timeout() {
        Failure {
            category: "timeout",
            message: "The provider request timed out. Audio and checkpoints were retained.".into(),
            retryable: true,
            retry_after_secs: None,
        }
    } else {
        Failure::provider("Unable to contact the configured provider securely. Check connection and provider availability.")
    }
}
pub fn form(remote: &Remote, pcm: &[i16]) -> Result<multipart::Form, Failure> {
    let mut wav = crate::recording::wav::header(0)
        .map_err(Failure::from)?
        .to_vec();
    let bytes = pcm.len() as u32 * 2;
    wav[4..8].copy_from_slice(&(36 + bytes).to_le_bytes());
    wav[22..24].copy_from_slice(&1u16.to_le_bytes());
    wav[24..28].copy_from_slice(&16000u32.to_le_bytes());
    wav[28..32].copy_from_slice(&32000u32.to_le_bytes());
    wav[32..34].copy_from_slice(&2u16.to_le_bytes());
    wav[40..44].copy_from_slice(&bytes.to_le_bytes());
    wav.extend(pcm.iter().flat_map(|s| s.to_le_bytes()));
    let file = multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|_| Failure::input("Unable to format audio upload."))?;
    let mut form = multipart::Form::new()
        .text("model", remote.model.clone())
        .text("response_format", remote.response_format.clone())
        .part("file", file);
    if remote.language != "auto" {
        form = form.text("language", remote.language.clone());
    }
    if remote.response_format == "verbose_json" {
        form = form.text("timestamp_granularities[]", "segment");
    }
    Ok(form)
}
fn seconds(value: &Value, duration: u64) -> Result<u64, Failure> {
    let seconds = value
        .as_f64()
        .ok_or_else(|| Failure::input("Provider timestamps have an unsupported type."))?;
    if !seconds.is_finite() || seconds < 0.0 || seconds * 1000.0 > duration as f64 + 20.0 {
        return Err(Failure::input(
            "Provider timestamps are outside the source chunk.",
        ));
    }
    Ok((seconds * 1000.0).round() as u64)
}
pub fn parse(
    value: Value,
    source: &str,
    offset: u64,
    duration: u64,
) -> Result<Vec<Segment>, Failure> {
    let text = value
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::input("Provider response must contain a text string."))?;
    if let Some(entries) = value.get("segments").and_then(Value::as_array) {
        if entries.len() > 10000 {
            return Err(Failure::input("Too many transcript segments."));
        }
        if !entries.is_empty() {
            let mut segments = vec![];
            for entry in entries {
                let text = entry
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Failure::input("A provider segment has no text."))?
                    .trim();
                if text.is_empty() {
                    continue;
                }
                let (start_ms, end_ms) = match (entry.get("start"), entry.get("end")) {
                    (Some(start), Some(end)) if !start.is_null() && !end.is_null() => {
                        let start = seconds(start, duration)?;
                        let end = seconds(end, duration)?;
                        if start > duration || end < start {
                            return Err(Failure::input("Provider segment timestamps are invalid."));
                        }
                        (Some(offset + start), Some(offset + end.min(duration)))
                    }
                    (None, None) | (Some(Value::Null), Some(Value::Null)) => (None, None),
                    _ => {
                        return Err(Failure::input(
                            "Provider returned incomplete segment timestamps.",
                        ))
                    }
                };
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
            super::provider::validate(&segments, source, offset, duration)?;
            return Ok(segments);
        }
    } else if value.get("segments").is_some() {
        return Err(Failure::input("Provider segments must be an array."));
    }
    if text.trim().is_empty() {
        return Ok(vec![]);
    }
    let segments = vec![Segment {
        text: text.trim().into(),
        start_ms: None,
        end_ms: None,
        sources: vec![Source {
            role: source.into(),
            start_ms: None,
            end_ms: None,
        }],
    }];
    super::provider::validate(&segments, source, offset, duration)?;
    Ok(segments)
}
pub struct RemoteProvider<'a> {
    pub settings: &'a Remote,
    pub credential_id: &'a str,
}
impl Provider for RemoteProvider<'_> {
    fn transcribe(
        &self,
        pcm: &[i16],
        _: &Path,
        source: &str,
        offset: u64,
    ) -> Result<Vec<Segment>, Failure> {
        if pcm.iter().all(|s| *s == 0) {
            return Ok(vec![]);
        }
        self.request(pcm, source, offset)
    }
}
impl RemoteProvider<'_> {
    pub fn request(&self, pcm: &[i16], source: &str, offset: u64) -> Result<Vec<Segment>, Failure> {
        let url = settings::base_url(&self.settings.base_url)
            .map_err(|s| Failure::configuration(&s))?
            .join("audio/transcriptions")
            .map_err(|_| Failure::configuration("Invalid audio endpoint."))?;
        let key = WindowsVault.read(self.credential_id)?.ok_or_else(|| {
            Failure::configuration(
                "The saved transcription key was deleted. Replace it in AI Processing settings.",
            )
        })?;
        let reply = client()?
            .post(url)
            .bearer_auth(key.as_str())
            .multipart(form(self.settings, pcm)?)
            .send()
            .map_err(send_error)?;
        let value = response(reply)?;
        fn contains_key(value: &Value, key: &str) -> bool {
            match value {
                Value::String(s) => s.contains(key),
                Value::Array(values) => values.iter().any(|v| contains_key(v, key)),
                Value::Object(values) => values
                    .iter()
                    .any(|(name, v)| name.contains(key) || contains_key(v, key)),
                _ => false,
            }
        }
        if contains_key(&value, key.as_str()) {
            return Err(Failure{category:"unsupported",message:"The provider unexpectedly echoed credential material. Its response was rejected and not persisted.".into(),retryable:false,retry_after_secs:None});
        }
        parse(value, source, offset, pcm.len() as u64 * 1000 / 16000)
    }
}
pub fn connection_test(engine: &Engine, kind: &str) -> Result<String, Failure> {
    match engine {
        Engine::Local { local, .. } => {
            local.verify()?;
            Ok("Local executable/model fingerprints verified. This check does not run speech recognition.".into())
        }
        Engine::Remote {
            remote,
            credential_id,
            ..
        } => {
            let id = credential_id
                .as_deref()
                .ok_or_else(|| Failure::configuration("Save an API key before testing."))?;
            if kind == "transcription" {
                let segments = RemoteProvider {
                    settings: remote,
                    credential_id: id,
                }
                .request(&vec![0; 16000], "microphone", 0)?;
                Ok(if segments.iter().any(|s|s.start_ms.is_none()){"Audio endpoint responded; segment timestamps are unavailable."}else{"Audio endpoint accepted a one-second synthetic silence upload. Real speech quality is not verified."}.into())
            } else {
                let url = settings::base_url(&remote.base_url)
                    .map_err(|s| Failure::configuration(&s))?
                    .join("models")
                    .map_err(|_| Failure::configuration("Invalid model-list endpoint."))?;
                let key = WindowsVault.read(id)?.ok_or_else(|| {
                    Failure::configuration("The saved summarization key is unavailable.")
                })?;
                let value = response(
                    client()?
                        .get(url)
                        .bearer_auth(key.as_str())
                        .send()
                        .map_err(send_error)?,
                )?;
                let entries = value.get("data").and_then(Value::as_array).ok_or_else(|| {
                    Failure::input(
                        "The gateway does not support an OpenAI-compatible model-list test.",
                    )
                })?;
                if !entries
                    .iter()
                    .any(|e| e.get("id").and_then(Value::as_str) == Some(&remote.model))
                {
                    return Err(Failure::configuration("The configured model was not listed. This gateway may restrict model enumeration."));
                }
                Ok("Model access verified using GET /models. Chat completions compatibility has not been tested; no meeting summary was generated.".into())
            }
        }
    }
}

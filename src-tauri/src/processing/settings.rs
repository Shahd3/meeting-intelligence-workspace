use super::{
    credentials::{Vault, WindowsVault},
    db_error,
    provider::{Config, Failure},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;
pub fn fingerprint(engine: &Engine, pcm: &[i16]) -> String {
    use sha2::{Digest, Sha256};
    // Keep legacy local checkpoint identity compatible with the tested implementation.
    if let Engine::Local { local, .. } = engine {
        return super::provider::fingerprint(local, pcm);
    }
    let mut hash = Sha256::new();
    hash.update(b"miw-remote-v1");
    hash.update(serde_json::to_vec(engine).unwrap_or_default());
    for sample in pcm {
        hash.update(sample.to_le_bytes());
    }
    format!("{:x}", hash.finalize())
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remote {
    pub base_url: String,
    pub model: String,
    pub response_format: String,
    pub language: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Engine {
    Local {
        local: Config,
        revision: String,
    },
    Remote {
        remote: Remote,
        revision: String,
        credential_id: Option<String>,
    },
}
impl Engine {
    pub fn revision(&self) -> &str {
        match self {
            Self::Local { revision, .. } | Self::Remote { revision, .. } => revision,
        }
    }
    fn credential(&self) -> Option<&str> {
        match self {
            Self::Remote { credential_id, .. } => credential_id.as_deref(),
            _ => None,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    pub kind: String,
    pub mode: String,
    pub local: Option<Config>,
    pub remote: Option<Remote>,
    pub key_action: String,
    pub api_key: Option<String>,
    #[serde(default)]
    pub automatic_transcription: bool,
    #[serde(default)]
    pub automatic_reviewed: bool,
    #[serde(default)]
    pub automatic_generation: bool,
    #[serde(default = "default_language")]
    pub output_language: String,
    #[serde(default = "default_format")]
    pub generation_format: String,
}
fn default_language() -> String {
    "en".into()
}
fn default_format() -> String {
    "json_object".into()
}
#[derive(Serialize)]
pub struct PublicSettings {
    pub mode: String,
    pub revision: String,
    pub local: Option<Config>,
    pub remote: Option<Remote>,
    pub key_saved: bool,
    pub automatic_transcription: bool,
    pub automatic_generation: bool,
    pub output_language: String,
    pub generation_format: String,
}
#[derive(Serialize)]
pub struct View {
    pub transcription: Option<PublicSettings>,
    pub summarization: Option<PublicSettings>,
    pub credential_cleanup_pending: u32,
}
pub fn base_url(base: &str) -> Result<reqwest::Url, String> {
    if base.len() > 2048 {
        return Err("API base URL is too long.".into());
    }
    let mut url = reqwest::Url::parse(base).map_err(|_| "Enter a valid absolute API base URL.")?;
    let host = url.host_str().ok_or("API base URL needs a host.")?;
    let local = host == "localhost"
        || host
            .trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .map(|ip| ip.is_loopback())
            .unwrap_or(false);
    if (url.scheme() != "https" && !(url.scheme() == "http" && local))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Use HTTPS (or HTTP on loopback only), without embedded credentials, query parameters, or fragments.".into());
    }
    let path = format!("{}/", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url)
}
pub fn validate_remote(remote: &mut Remote, kind: &str) -> Result<(), String> {
    remote.base_url = base_url(&remote.base_url)?.to_string();
    if remote.model.trim().is_empty()
        || remote.model.len() > 200
        || remote.model.chars().any(char::is_control)
    {
        return Err("Enter a model name without control characters (maximum 200 bytes).".into());
    }
    if kind == "transcription"
        && !["verbose_json", "json"].contains(&remote.response_format.as_str())
    {
        return Err("Select verbose_json or json transcription output.".into());
    }
    if !(remote.language == "auto"
        || (remote.language.len() == 2 && remote.language.bytes().all(|c| c.is_ascii_lowercase())))
    {
        return Err("Use auto or a two-letter lowercase language code.".into());
    }
    Ok(())
}
pub fn load(db: &Connection, kind: &str) -> Result<Option<Engine>, String> {
    let json: Option<String> = db
        .query_row(
            "SELECT config FROM ai_settings WHERE kind=?1",
            [kind],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    if let Some(json) = json {
        return serde_json::from_str(&json)
            .map(Some)
            .map_err(|_| "AI settings are invalid.".into());
    }
    if kind == "transcription" {
        return super::config(db).map(|local| {
            local.map(|local| Engine::Local {
                local,
                revision: "legacy-local".into(),
            })
        });
    }
    Ok(None)
}
pub fn parse_job(json: &str) -> Result<Engine, String> {
    serde_json::from_str(json)
        .or_else(|_| {
            serde_json::from_str::<Config>(json).map(|local| Engine::Local {
                local,
                revision: "legacy-local".into(),
            })
        })
        .map_err(|_| "Saved job configuration is invalid.".into())
}
pub fn automatic_enabled(db: &Connection, engine: &Engine) -> Result<bool, String> {
    let revision: Option<String> = db
        .query_row(
            "SELECT automatic_revision FROM transcription_preferences WHERE id=1",
            [],
            |r| r.get(0),
        )
        .map_err(db_error)?;
    Ok(revision.as_deref() == Some(engine.revision()))
}
fn review_required(enabled: bool, reviewed: bool, already_authorized: bool) -> Result<(), String> {
    if enabled && !reviewed && !already_authorized {
        return Err("Review and save Automatically transcribe saved recordings for these provider settings before enabling it.".into());
    }
    Ok(())
}
fn same_provider(left: &Engine, right: &Engine) -> bool {
    match (left, right) {
        (Engine::Local { local: a, .. }, Engine::Local { local: b, .. }) => a == b,
        (
            Engine::Remote {
                remote: a,
                credential_id: ka,
                ..
            },
            Engine::Remote {
                remote: b,
                credential_id: kb,
                ..
            },
        ) => a == b && ka == kb,
        _ => false,
    }
}
fn public(engine: Engine, vault: &impl Vault, automatic: bool) -> Result<PublicSettings, String> {
    let key_saved = engine
        .credential()
        .map(|id| vault.read(id).map(|key| key.is_some()))
        .transpose()?
        .unwrap_or(false);
    Ok(match engine {
        Engine::Local { local, revision } => PublicSettings {
            mode: "local".into(),
            revision,
            local: Some(local),
            remote: None,
            key_saved: false,
            automatic_transcription: automatic,
            automatic_generation: false,
            output_language: "en".into(),
            generation_format: "json_object".into(),
        },
        Engine::Remote {
            remote, revision, ..
        } => PublicSettings {
            mode: "remote".into(),
            revision,
            local: None,
            remote: Some(remote),
            key_saved,
            automatic_transcription: automatic,
            automatic_generation: false,
            output_language: "en".into(),
            generation_format: "json_object".into(),
        },
    })
}
pub fn view(db: &Connection) -> Result<View, String> {
    Ok(View {
        credential_cleanup_pending: db
            .query_row("SELECT count(*) FROM ai_credential_cleanup", [], |r| {
                r.get(0)
            })
            .map_err(db_error)?,
        transcription: load(db, "transcription")?
            .map(|c| {
                let automatic = automatic_enabled(db, &c)?;
                public(c, &WindowsVault, automatic)
            })
            .transpose()?,
        summarization: load(db, "summarization")?
            .map(|c| {
                let enabled = crate::intelligence::automatic(db, &c)?;
                let (language, format) = db
                    .query_row(
                        "SELECT language,format FROM intelligence_preferences WHERE id=1",
                        [],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                    )
                    .map_err(db_error)?;
                let mut p = public(c, &WindowsVault, false)?;
                p.automatic_generation = enabled;
                p.output_language = language;
                p.generation_format = format;
                Ok::<PublicSettings, String>(p)
            })
            .transpose()?,
    })
}
pub fn save(db: &mut Connection, mut request: Save, vault: &impl Vault) -> Result<(), String> {
    // Secret request fields deliberately have no Debug/Serialize implementations.
    let key = request.api_key.take().map(Zeroizing::new);
    if !["transcription", "summarization"].contains(&request.kind.as_str()) {
        return Err("Invalid AI settings section.".into());
    }
    if !["keep", "replace", "delete"].contains(&request.key_action.as_str()) {
        return Err("Invalid credential action.".into());
    }
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(db_error)?;
    let old = load(&tx, &request.kind)?;
    let was_automatic = old
        .as_ref()
        .map(|c| automatic_enabled(&tx, c))
        .transpose()?
        .unwrap_or(false);
    let old_id = old
        .as_ref()
        .and_then(Engine::credential)
        .map(str::to_string);
    let (old_language, old_format): (String, String) = tx
        .query_row(
            "SELECT language,format FROM intelligence_preferences WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(db_error)?;
    let generation_options_unchanged =
        old_language == request.output_language && old_format == request.generation_format;
    if request.kind == "summarization" {
        if !["en", "ar", "auto"].contains(&request.output_language.as_str())
            || !["json_object", "json_schema"].contains(&request.generation_format.as_str())
        {
            return Err(
                "Select a supported output language and structured response format.".into(),
            );
        }
        let unchanged = request.key_action == "keep"
            && generation_options_unchanged
            && matches!((&old,&request.remote),(Some(Engine::Remote{remote,..}),Some(next)) if remote==next);
        let already = old
            .as_ref()
            .map(|e| crate::intelligence::automatic(&tx, e))
            .transpose()?
            .unwrap_or(false);
        if request.automatic_generation && !request.automatic_reviewed && !(already && unchanged) {
            return Err(
                "Review and save automatic transcript sharing for this summarization provider."
                    .into(),
            );
        }
        if request.automatic_generation
            && (request.key_action == "delete"
                || (request.key_action == "keep" && old_id.is_none()))
        {
            return Err("Save a summarization key before authorizing generation.".into());
        }
    }
    let revision = uuid::Uuid::new_v4().to_string();
    let mut new_id = None;
    let mut engine = match request.mode.as_str() {
        "local" if request.kind == "transcription" => {
            let mut local = request
                .local
                .ok_or("Local executable and model settings are required.")?;
            local.validate_and_fingerprint()?;
            let unchanged =
                matches!(&old,Some(Engine::Local{local:previous,..}) if previous==&local);
            review_required(
                request.automatic_transcription,
                request.automatic_reviewed,
                was_automatic && unchanged,
            )?;
            Engine::Local { local, revision }
        }
        "remote" => {
            let mut remote = request.remote.ok_or("Remote API settings are required.")?;
            validate_remote(&mut remote, &request.kind)?;
            if request.kind == "transcription" {
                let unchanged = request.key_action == "keep"
                    && matches!(&old,Some(Engine::Remote{remote:previous,..}) if previous==&remote);
                review_required(
                    request.automatic_transcription,
                    request.automatic_reviewed,
                    was_automatic && unchanged,
                )?;
                if request.automatic_transcription
                    && (request.key_action == "delete"
                        || (request.key_action == "keep" && old_id.is_none()))
                {
                    return Err(
                        "Save an API key before enabling automatic remote transcription.".into(),
                    );
                }
            }
            if request.key_action == "keep" {
                if let Some(Engine::Remote {
                    remote: previous, ..
                }) = &old
                {
                    if previous.base_url != remote.base_url && old_id.is_some() {
                        return Err("The endpoint changed. Re-enter the key explicitly for this destination, or delete it.".into());
                    }
                }
            }
            let credential_id = match request.key_action.as_str() {
                "replace" => {
                    let key = key.as_ref().ok_or("Enter the replacement API key.")?;
                    let id = uuid::Uuid::new_v4().to_string();
                    vault.write(&id, key)?;
                    new_id = Some(id.clone());
                    Some(id)
                }
                "delete" => None,
                _ => old_id.clone(),
            };
            Engine::Remote {
                remote,
                revision,
                credential_id,
            }
        }
        _ => return Err(
            "Select Local Whisper or Remote API; summarization supports remote configuration only."
                .into(),
        ),
    };
    // Saving an unchanged form must not invalidate already approved jobs.
    if let Some(previous) = &old {
        if same_provider(previous, &engine)
            && (request.kind != "summarization" || generation_options_unchanged)
        {
            match &mut engine {
                Engine::Local { revision, .. } | Engine::Remote { revision, .. } => {
                    *revision = previous.revision().into()
                }
            }
        }
    }
    let serialized = serde_json::to_string(&engine).map_err(|_| "Unable to encode AI settings.")?;
    let result = (|| {
        tx.execute("INSERT INTO ai_settings(kind,config) VALUES (?1,?2) ON CONFLICT(kind) DO UPDATE SET config=excluded.config",params![request.kind,serialized]).map_err(db_error)?;
        if let Engine::Local { local, .. } = &engine {
            tx.execute(
                "UPDATE transcription_settings SET config=?1 WHERE id=1",
                [serde_json::to_string(local).map_err(|_| "Invalid local settings.")?],
            )
            .map_err(db_error)?;
        }
        if let Some(id) = &old_id {
            if engine.credential() != Some(id.as_str()) {
                tx.execute(
                    "INSERT OR IGNORE INTO ai_credential_cleanup(id) VALUES (?1)",
                    [id],
                )
                .map_err(db_error)?;
            }
        }
        // Only unbound configuration-blocked jobs can resume automatically. Remote
        // upload still requires per-job consent for this new revision.
        if request.kind == "transcription" {
            if request.automatic_transcription {
                let already = was_automatic
                    && old
                        .as_ref()
                        .map(|previous| same_provider(previous, &engine))
                        .unwrap_or(false);
                tx.execute("UPDATE transcription_preferences SET automatic_revision=?1,after_recording_rowid=CASE WHEN ?2 THEN after_recording_rowid ELSE (SELECT COALESCE(max(rowid),0) FROM recordings) END WHERE id=1",params![engine.revision(),already]).map_err(db_error)?;
            } else {
                tx.execute(
                    "UPDATE transcription_preferences SET automatic_revision=NULL WHERE id=1",
                    [],
                )
                .map_err(db_error)?;
            }
            tx.execute("UPDATE processing_jobs SET state='queued',error=NULL,error_category=NULL WHERE ready=1 AND state='failed' AND error_category='configuration' AND config IS NULL AND attempts<5",[]).map_err(db_error)?;
        }
        if request.kind == "summarization" {
            let prior = old
                .as_ref()
                .map(|e| crate::intelligence::automatic(&tx, e))
                .transpose()?
                .unwrap_or(false);
            let same = prior
                && old
                    .as_ref()
                    .map(|e| e.revision() == engine.revision())
                    .unwrap_or(false);
            tx.execute("UPDATE intelligence_preferences SET language=?1,format=?2,automatic_revision=?3,after_recording_rowid=CASE WHEN ?4 THEN after_recording_rowid ELSE (SELECT COALESCE(max(rowid),0) FROM recordings) END WHERE id=1",params![request.output_language,request.generation_format,if request.automatic_generation{Some(engine.revision())}else{None},same]).map_err(db_error)?;
        }
        tx.commit().map_err(db_error)
    })();
    if result.is_err() {
        if let Some(id) = new_id {
            if vault.delete(&id).is_err() {
                let _ = db.execute(
                    "INSERT OR IGNORE INTO ai_credential_cleanup(id) VALUES (?1)",
                    [id],
                );
            }
        }
        return result;
    }
    cleanup(db, vault)
}
pub fn cleanup(db: &Connection, vault: &impl Vault) -> Result<(), String> {
    let ids: Vec<String> = {
        let mut q = db
            .prepare("SELECT id FROM ai_credential_cleanup")
            .map_err(db_error)?;
        let rows = q
            .query_map([], |r| r.get(0))
            .map_err(db_error)?
            .collect::<rusqlite::Result<_>>()
            .map_err(db_error)?;
        rows
    };
    let mut failed = false;
    for id in ids {
        if vault.delete(&id).is_ok() {
            db.execute("DELETE FROM ai_credential_cleanup WHERE id=?1", [id])
                .map_err(db_error)?;
        } else {
            failed = true;
        }
    }
    if failed {
        Err("Settings saved, but retired credential cleanup is pending. Save settings again to retry cleanup; retired keys cannot authorize new requests.".into())
    } else {
        Ok(())
    }
}

pub fn ensure_remote_current(db: &Connection, engine: &Engine) -> Result<(), Failure> {
    if let Engine::Remote { revision, .. } = engine {
        let current = load(db, "transcription")?
            .ok_or_else(|| Failure::configuration("Transcription settings are unavailable."))?;
        if current.revision() != revision || !matches!(current, Engine::Remote { .. }) {
            return Err(Failure::configuration("The provider settings changed. Use latest settings and explicitly authorize this recording again."));
        }
    }
    Ok(())
}
pub fn rebind(
    db: &mut Connection,
    id: &str,
    revision: &str,
    authorize: bool,
) -> Result<(), String> {
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(db_error)?;
    let current = load(&tx, "transcription")?.ok_or("Save transcription settings first.")?;
    if current.revision() != revision {
        return Err("Settings changed. Review the current destination before retrying.".into());
    }
    if matches!(current, Engine::Remote { .. }) && !authorize {
        return Err("Explicit audio upload approval is required.".into());
    }
    if let Engine::Remote { credential_id, .. } = &current {
        if credential_id.is_none() {
            return Err("Save a transcription API key before approving an upload.".into());
        }
    }
    let changed=tx.execute("UPDATE processing_jobs SET ready=1,state='queued',stage='waiting',config=?1,upload_revision=?2,error=NULL,error_category=NULL,next_attempt_at=NULL,owner=NULL WHERE id=?3 AND state IN ('failed','retryable','queued') AND (ready=0 OR state!='queued') AND attempts<5",params![serde_json::to_string(&current).map_err(|_|"Invalid settings.")?,if authorize{Some(revision)}else{None},id]).map_err(db_error)?;
    if changed == 0 {
        return Err("The job is active, completed, missing, or exhausted its retry budget.".into());
    }
    tx.execute(
        "DELETE FROM transcription_checkpoints WHERE job_id=?1",
        [id],
    )
    .map_err(db_error)?;
    tx.commit().map_err(db_error)
}

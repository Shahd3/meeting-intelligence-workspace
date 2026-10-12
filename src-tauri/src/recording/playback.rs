//! Read-only, ID-based, bounded range playback. No frontend-controlled filesystem paths.
use super::*;
use std::io::{Read, Seek, SeekFrom};
use tauri::http::{Request, Response, StatusCode};
const CHUNK: u64 = 2 * 1024 * 1024;
fn fail(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .body(message.as_bytes().to_vec())
        .unwrap()
}
fn range(value: Option<&str>, length: u64) -> Result<(u64, u64)> {
    if length == 0 {
        return Err("Audio file is empty.".into());
    }
    let Some(value) = value else {
        return Ok((0, length - 1));
    };
    let value = value.strip_prefix("bytes=").ok_or("Unsupported range.")?;
    if value.contains(',') {
        return Err("Multiple ranges are unsupported.".into());
    }
    let (start, end) = value.split_once('-').ok_or("Invalid range.")?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().map_err(|_| "Invalid range.")?;
        if suffix == 0 {
            return Err("Invalid range.".into());
        }
        return Ok((length.saturating_sub(suffix.min(CHUNK)), length - 1));
    }
    let start = start.parse::<u64>().map_err(|_| "Invalid range.")?;
    let end = if end.is_empty() {
        length - 1
    } else {
        end.parse::<u64>().map_err(|_| "Invalid range.")?
    };
    if start >= length || end < start {
        return Err("Range is outside the audio file.".into());
    }
    Ok((start, end.min(length - 1).min(start + CHUNK - 1)))
}
pub fn response(root: &Path, db_path: &Path, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() != "GET" && request.method() != "HEAD" {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "Read-only audio playback.");
    }
    let origin = request
        .headers()
        .get("Origin")
        .and_then(|v| v.to_str().ok());
    let allowed = [
        "http://127.0.0.1:1420",
        "http://localhost:1420",
        "http://tauri.localhost",
        "https://tauri.localhost",
        "tauri://localhost",
    ];
    if origin.is_some_and(|value| !allowed.contains(&value)) {
        return fail(
            StatusCode::FORBIDDEN,
            "Audio playback origin is not allowed.",
        );
    }
    let result = (|| -> Result<_> {
        let parts: Vec<_> = request.uri().path().trim_matches('/').split('/').collect();
        if parts.len() != 2 {
            return Err("Invalid audio reference.".into());
        }
        valid_id(parts[0])?;
        if !["microphone", "system"].contains(&parts[1]) {
            return Err("Invalid track reference.".into());
        }
        let db = crate::storage::open(db_path)?;
        let filename:String=db.query_row("SELECT t.filename FROM recording_tracks t JOIN recordings r ON r.id=t.recording_id WHERE r.id=?1 AND t.role=?2 AND r.finalized=1",params![parts[0],parts[1]],|r|r.get(0)).optional().map_err(db_error)?.ok_or("Finalized recording is unavailable.")?;
        if !valid_filename(parts[1], &filename) || filename.ends_with(".part") {
            return Err("Invalid playback reference.".into());
        }
        let path = root
            .join(parts[0])
            .join(filename)
            .canonicalize()
            .map_err(|_| "Recording file is unavailable.")?;
        let canonical_root = root
            .canonicalize()
            .map_err(|_| "Recording storage is unavailable.")?;
        if !path.starts_with(canonical_root) {
            return Err("Audio path is outside recording storage.".into());
        }
        let file = File::open(path).map_err(|_| "Recording file is unavailable.")?;
        let length = file
            .metadata()
            .map_err(|_| "Unable to inspect audio file.")?
            .len();
        Ok((file, length))
    })();
    let (mut file, length) = match result {
        Ok(pair) => pair,
        Err(_) => {
            return fail(
                StatusCode::NOT_FOUND,
                "Recording is unavailable. Reload or recover local audio.",
            )
        }
    };
    let range_header = request.headers().get("Range").and_then(|v| v.to_str().ok());
    let (start, end) = match range(range_header, length) {
        Ok(pair) => pair,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header("Content-Range", format!("bytes */{length}"))
                .body(vec![])
                .unwrap()
        }
    };
    if range_header.is_none() && length > 8 * 1024 * 1024 {
        return fail(
            StatusCode::BAD_REQUEST,
            "Use range requests for large recordings.",
        );
    }
    let count = end - start + 1;
    let mut bytes = vec![];
    if request.method() != "HEAD" {
        if file.seek(SeekFrom::Start(start)).is_err()
            || file.take(count).read_to_end(&mut bytes).is_err()
        {
            return fail(StatusCode::INTERNAL_SERVER_ERROR, "Unable to read audio.");
        }
    }
    let mut builder = Response::builder()
        .status(if range_header.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header("Content-Type", "audio/wav")
        .header("Accept-Ranges", "bytes")
        .header("Content-Length", count.to_string())
        .header("Cache-Control", "no-store")
        .header(
            "Access-Control-Allow-Origin",
            origin.unwrap_or("http://tauri.localhost"),
        );
    if range_header.is_some() {
        builder = builder.header("Content-Range", format!("bytes {start}-{end}/{length}"));
    }
    builder.body(bytes).unwrap()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_parsing_bounds_memory_and_rejects_invalid_ranges() {
        assert_eq!(range(Some("bytes=0-"), 10_000_000).unwrap(), (0, CHUNK - 1));
        assert_eq!(range(Some("bytes=10-20"), 100).unwrap(), (10, 20));
        assert_eq!(range(Some("bytes=-10"), 100).unwrap(), (90, 99));
        assert!(range(Some("bytes=100-"), 100).is_err());
        assert!(range(Some("bytes=0-1,4-5"), 100).is_err());
    }
}

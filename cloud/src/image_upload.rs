use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use chrono::Local;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::time_util::now_rfc3339;

const MAX_UPLOAD_SIZE_BYTES: usize = 10 * 1024 * 1024;
const FILE_FIELD_NAMES: [&str; 3] = ["file", "image", "photo"];

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ImageUploadTag {
    pub(crate) device_id: String,
    pub(crate) ts: String,
    pub(crate) location: String,
    pub(crate) crop_type: String,
    pub(crate) farm_note: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ImageUploadOkResponse {
    pub(crate) status: String,
    pub(crate) message: String,
    pub(crate) upload_id: String,
    pub(crate) saved_path: String,
    pub(crate) tag: ImageUploadTag,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ImageUploadErrorResponse {
    pub(crate) status: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedFilePart {
    pub(crate) filename: Option<String>,
    pub(crate) body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
struct ImageIndexRecord {
    ts: String,
    upload_id: String,
    saved_path: String,
    file_size: usize,
    sha256: String,
    image_type: String,
    tag: ImageUploadTag,
}

pub(crate) fn parse_tag(query: &HashMap<String, String>) -> Result<ImageUploadTag, String> {
    let device_id = required_query(query, "device_id")?;
    let ts = required_query(query, "ts")?;
    let location = query.get("location").cloned().unwrap_or_default();
    let crop_type = query.get("crop_type").cloned().unwrap_or_default();
    let farm_note = query.get("farm_note").cloned().unwrap_or_default();

    Ok(ImageUploadTag {
        device_id,
        ts,
        location,
        crop_type,
        farm_note,
    })
}

pub(crate) fn parse_multipart_file(
    content_type: &str,
    body: &[u8],
) -> Result<ParsedFilePart, String> {
    if body.is_empty() {
        return Err("request body is empty".to_string());
    }
    if body.len() > MAX_UPLOAD_SIZE_BYTES {
        return Err(format!(
            "request body exceeds {} bytes limit",
            MAX_UPLOAD_SIZE_BYTES
        ));
    }

    let boundary = parse_boundary(content_type)
        .ok_or_else(|| "missing multipart boundary in Content-Type".to_string())?;

    let delimiter = format!("--{boundary}").into_bytes();
    if !body.starts_with(&delimiter) {
        return Err("multipart body does not start with expected boundary".to_string());
    }

    let mut cursor = delimiter.len();
    loop {
        if let Some(rest) = body.get(cursor..) {
            if rest.starts_with(b"--") {
                break;
            }
        }

        if !matches!(body.get(cursor..cursor + 2), Some(v) if v == b"\r\n") {
            return Err("invalid multipart format: missing CRLF after boundary".to_string());
        }
        cursor += 2;

        let header_end = find_subslice(body, b"\r\n\r\n", cursor)
            .ok_or_else(|| "invalid multipart format: missing header terminator".to_string())?;
        let header_text = String::from_utf8_lossy(&body[cursor..header_end]);
        cursor = header_end + 4;

        let next_marker = find_subslice(body, b"\r\n--", cursor)
            .ok_or_else(|| "invalid multipart format: missing part boundary".to_string())?;
        let part_body = body[cursor..next_marker].to_vec();

        cursor = next_marker + 2;

        let name = extract_content_disposition_attr(&header_text, "name");
        let filename = extract_content_disposition_attr(&header_text, "filename");

        if let Some(field_name) = name {
            if FILE_FIELD_NAMES
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(field_name.as_str()))
            {
                if part_body.is_empty() {
                    return Err("uploaded file content is empty".to_string());
                }
                return Ok(ParsedFilePart {
                    filename,
                    body: part_body,
                });
            }
        }
    }

    Err("multipart file field is missing (expected file/image/photo)".to_string())
}

pub(crate) fn persist_image(
    image_store_path: &str,
    image_index_path: &str,
    tag: &ImageUploadTag,
    file: &ParsedFilePart,
) -> Result<ImageUploadOkResponse, String> {
    let image_type = detect_image_type(&file.body)?;
    let ext = match image_type.as_str() {
        "jpeg" => "jpg",
        "png" => "png",
        _ => return Err("unsupported image format".to_string()),
    };

    let upload_id = generate_upload_id();
    let date_part = Local::now().format("%Y-%m-%d").to_string();
    let device_dir = sanitize_path_component(&tag.device_id);
    let relative_path = format!("{device_dir}/{date_part}/{upload_id}.{ext}");
    let saved_path = Path::new(image_store_path)
        .join(&relative_path)
        .to_string_lossy()
        .to_string();

    if let Some(parent) = Path::new(&saved_path).parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create image directory {}: {e}", parent.display()))?;
    }

    fs::write(&saved_path, &file.body)
        .map_err(|e| format!("failed to write uploaded image to {saved_path}: {e}"))?;

    let mut sha = Sha256::new();
    sha.update(&file.body);
    let sha256 = format!("{:x}", sha.finalize());
    let record = ImageIndexRecord {
        ts: now_rfc3339(),
        upload_id: upload_id.clone(),
        saved_path: saved_path.clone(),
        file_size: file.body.len(),
        sha256,
        image_type,
        tag: tag.clone(),
    };
    append_index_record(image_index_path, &record)?;

    Ok(ImageUploadOkResponse {
        status: "success".to_string(),
        message: format!(
            "image upload accepted{}",
            file.filename
                .as_deref()
                .map(|v| format!(" ({v})"))
                .unwrap_or_default()
        ),
        upload_id,
        saved_path,
        tag: tag.clone(),
    })
}

pub(crate) fn parse_boundary(content_type: &str) -> Option<String> {
    for part in content_type.split(';') {
        let trimmed = part.trim();
        let Some(raw) = trimmed.strip_prefix("boundary=") else {
            continue;
        };
        let boundary = raw.trim().trim_matches('"');
        if !boundary.is_empty() {
            return Some(boundary.to_string());
        }
    }
    None
}

fn required_query(query: &HashMap<String, String>, key: &str) -> Result<String, String> {
    query
        .get(key)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("missing required query: {key}"))
}

fn sanitize_path_component(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "unknown_device".to_string()
    } else {
        cleaned
    }
}

fn append_index_record(path: &str, record: &ImageIndexRecord) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "failed to create image index parent {}: {e}",
                    parent.display()
                )
            })?;
        }
    }

    let line = serde_json::to_string(record)
        .map_err(|e| format!("failed to serialize image index record: {e}"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("failed to open image index {path}: {e}"))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("failed to append image index record: {e}"))?;
    file.write_all(b"\n")
        .map_err(|e| format!("failed to finalize image index record: {e}"))?;
    Ok(())
}

fn detect_image_type(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Ok("jpeg".to_string());
    }
    if bytes.len() >= 8 && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return Ok("png".to_string());
    }
    Err("unsupported file type: only jpg/png accepted".to_string())
}

fn extract_content_disposition_attr(headers: &str, key: &str) -> Option<String> {
    for line in headers.lines() {
        if !line
            .to_ascii_lowercase()
            .starts_with("content-disposition:")
        {
            continue;
        }
        for token in line.split(';') {
            let token = token.trim();
            let Some((left, right)) = token.split_once('=') else {
                continue;
            };
            if left.trim().eq_ignore_ascii_case(key) {
                let value = right.trim().trim_matches('"');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn find_subslice(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    if needle.is_empty() || start >= haystack.len() || needle.len() > haystack.len() {
        return None;
    }
    haystack[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| start + offset)
}

fn generate_upload_id() -> String {
    let stamp = Local::now().format("%Y%m%d%H%M%S%3f").to_string();
    let suffix: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();
    format!("img_{stamp}_{suffix}")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{parse_boundary, parse_multipart_file, parse_tag};

    #[test]
    fn parse_boundary_supports_quoted_and_plain() {
        assert_eq!(
            parse_boundary("multipart/form-data; boundary=----abc"),
            Some("----abc".to_string())
        );
        assert_eq!(
            parse_boundary("multipart/form-data; boundary=\"----abc\""),
            Some("----abc".to_string())
        );
        assert_eq!(parse_boundary("application/json"), None);
    }

    #[test]
    fn parse_tag_requires_device_and_ts() {
        let mut query = HashMap::new();
        query.insert("device_id".to_string(), "dev_01".to_string());
        query.insert("ts".to_string(), "2026-04-17T12:34:56+08:00".to_string());
        let tag = parse_tag(&query).expect("tag should parse");
        assert_eq!(tag.device_id, "dev_01");
        assert_eq!(tag.ts, "2026-04-17T12:34:56+08:00");
    }

    #[test]
    fn parse_multipart_file_accepts_file_field() {
        let body = b"--test-boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"x.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n\xFF\xD8\xFF\xE0\x00\x10\r\n--test-boundary--\r\n";
        let part = parse_multipart_file("multipart/form-data; boundary=test-boundary", body)
            .expect("multipart should parse");
        assert_eq!(part.filename.as_deref(), Some("x.jpg"));
        assert!(part.body.starts_with(&[0xFF, 0xD8, 0xFF]));
    }
}

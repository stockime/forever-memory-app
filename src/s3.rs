//! Just enough S3 for backups: signed PUT and HEAD (AWS Signature V4, path
//! style), which works with AWS, Hetzner, Backblaze, Cloudflare R2, MinIO and
//! other S3-compatible storage.

use crate::config::S3;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(key).expect("any key length");
    m.update(data.as_bytes());
    m.finalize().into_bytes().to_vec()
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Percent-encodes a key path, keeping the slashes.
fn encode_path(key: &str) -> String {
    let mut out = String::new();
    for b in key.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn host(cfg: &S3) -> String {
    let e = cfg.endpoint.trim();
    let e = e
        .strip_prefix("https://")
        .or_else(|| e.strip_prefix("http://"))
        .unwrap_or(e);
    e.trim_end_matches('/').to_string()
}

fn scheme(cfg: &S3) -> &'static str {
    if cfg.endpoint.trim().starts_with("http://") {
        "http"
    } else {
        "https"
    }
}

/// The headers that sign one request.
fn sign(
    cfg: &S3,
    method: &str,
    path: &str,
    payload_hash: &str,
    extra: &[(&str, String)],
) -> Vec<(String, String)> {
    let now = chrono::Utc::now();
    let date = now.format("%Y%m%d").to_string();
    let stamp = now.format("%Y%m%dT%H%M%SZ").to_string();
    let region = if cfg.region.trim().is_empty() {
        "us-east-1"
    } else {
        cfg.region.trim()
    };
    let mut headers: Vec<(String, String)> = vec![
        ("host".into(), host(cfg)),
        ("x-amz-content-sha256".into(), payload_hash.into()),
        ("x-amz-date".into(), stamp.clone()),
    ];
    for (k, v) in extra {
        headers.push((k.to_lowercase(), v.trim().to_string()));
    }
    headers.sort();
    let canonical_headers: String = headers.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed: String = headers
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let request = format!("{method}\n{path}\n\n{canonical_headers}\n{signed}\n{payload_hash}");
    let scope = format!("{date}/{region}/s3/aws4_request");
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{stamp}\n{scope}\n{}",
        sha256_hex(request.as_bytes())
    );
    let k = hmac(format!("AWS4{}", cfg.secret_key.trim()).as_bytes(), &date);
    let k = hmac(&k, region);
    let k = hmac(&k, "s3");
    let k = hmac(&k, "aws4_request");
    let signature = hex::encode(hmac(&k, &to_sign));
    headers.push((
        "authorization".into(),
        format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed}, Signature={signature}",
            cfg.access_key.trim()
        ),
    ));
    headers.retain(|(k, _)| k != "host");
    headers
}

fn error(mut resp: ureq::http::Response<ureq::Body>) -> String {
    let status = resp.status();
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    let code = body
        .split("<Code>")
        .nth(1)
        .and_then(|s| s.split("</Code>").next())
        .unwrap_or("");
    let msg = body
        .split("<Message>")
        .nth(1)
        .and_then(|s| s.split("</Message>").next())
        .unwrap_or("");
    format!("{status} {code} {msg}").trim().to_string()
}

/// Uploads one object and checks the stored size.
pub fn put(
    cfg: &S3,
    key: &str,
    body: &[u8],
    content_type: &str,
    meta: &[(&str, &str)],
) -> Result<(), String> {
    let path = format!("/{}/{}", cfg.bucket.trim(), encode_path(key));
    // Buckets with object lock refuse uploads without an MD5.
    use base64::Engine;
    let md5 = base64::engine::general_purpose::STANDARD.encode(md5::Md5::digest(body));
    let mut extra: Vec<(&str, String)> = vec![
        ("content-type", content_type.to_string()),
        ("content-md5", md5),
    ];
    let meta_keys: Vec<String> = meta
        .iter()
        .map(|(k, _)| format!("x-amz-meta-{k}"))
        .collect();
    for ((_, v), k) in meta.iter().zip(&meta_keys) {
        extra.push((k.as_str(), v.to_string()));
    }
    let headers = sign(cfg, "PUT", &path, &sha256_hex(body), &extra);
    let mut req = ureq::put(format!("{}://{}{path}", scheme(cfg), host(cfg)));
    for (k, v) in &headers {
        req = req.header(k, v);
    }
    let resp = req
        .config()
        .timeout_global(Some(Duration::from_secs(1800)))
        .http_status_as_error(false)
        .build()
        .send(body)
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(error(resp));
    }
    match head(cfg, key)? {
        Some(n) if n == body.len() as u64 => Ok(()),
        Some(n) => Err(format!("uploaded {} bytes, stored {n}", body.len())),
        None => Err("the upload is missing".into()),
    }
}

/// The stored size of an object, or None if it isn't there.
pub fn head(cfg: &S3, key: &str) -> Result<Option<u64>, String> {
    let path = format!("/{}/{}", cfg.bucket.trim(), encode_path(key));
    let headers = sign(cfg, "HEAD", &path, &sha256_hex(b""), &[]);
    let mut req = ureq::head(format!("{}://{}{path}", scheme(cfg), host(cfg)));
    for (k, v) in &headers {
        req = req.header(k, v);
    }
    let resp = req
        .config()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .call()
        .map_err(|e| e.to_string())?;
    match resp.status().as_u16() {
        200 => Ok(resp
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok()?.parse().ok())),
        404 => Ok(None),
        s => Err(format!("{s}")),
    }
}

/// Checks the settings by writing and reading back a small object.
pub fn check(cfg: &S3) -> Result<(), String> {
    put(
        cfg,
        "forever-memory-check.txt",
        b"Forever Memory can write here.\n",
        "text/plain",
        &[],
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn paths() {
        assert_eq!(
            super::encode_path("git/1-ab c.bundle"),
            "git/1-ab%20c.bundle"
        );
    }
}

//! S3-compatible storage client with AWS v4 signing.
//! Handles prefix automatically for all operations.

use base64::Engine;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{Client, Url};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

// ============================================================================
// ObjectStorage trait — shared interface for S3 and presigned uploads
// ============================================================================

pub trait ObjectStorage: Send + Sync {
    /// HEAD check. Returns (checksum_sha256_b64, has_etag) if object exists.
    fn head(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Option<(Option<String>, bool)>, String>> + Send;

    /// PUT an object. Returns true on success.
    fn put(
        &self,
        key: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> impl std::future::Future<Output = Result<bool, String>> + Send;
}

/// Upload assets with 10-way concurrency and HEAD-check deduplication.
/// Each asset is (key, content, content_type, sha256_hex).
/// Returns (uploaded, skipped).
pub async fn upload_assets(
    storage: &impl ObjectStorage,
    assets: &[(String, Vec<u8>, String, String)],
) -> (usize, usize) {
    use colored::Colorize;
    use futures::stream::{self, StreamExt};
    use std::sync::atomic::{AtomicUsize, Ordering};

    let uploaded = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);

    stream::iter(assets.iter().map(|(path, content, ct, hash_hex)| {
        (
            path.clone(),
            content.clone(),
            ct.clone(),
            hex_to_base64(hash_hex),
        )
    }))
    .for_each_concurrent(10, |(path, content, ct, hash_b64)| {
        let uploaded = &uploaded;
        let skipped = &skipped;

        async move {
            let mut should_upload = true;
            let mut has_etag = false;

            if let Ok(Some((remote_checksum, etag))) = storage.head(&path).await {
                has_etag = etag;

                if let Some(ref remote_hash) = remote_checksum {
                    if remote_hash == &hash_b64 {
                        println!(
                            "  {} {} {}",
                            "⎿".dimmed(),
                            path,
                            "(skipped, checksum match)".dimmed()
                        );
                        skipped.fetch_add(1, Ordering::Relaxed);
                        should_upload = false;
                    }
                }
            }

            if should_upload {
                match storage.put(&path, content, &ct).await {
                    Ok(true) => {
                        let reason = if has_etag { "checksum changed" } else { "new" };
                        println!("  {} {} ({})", "⎿".dimmed(), path, reason);
                        uploaded.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(false) => eprintln!("  {} {} (upload failed)", "⎿".red(), path),
                    Err(e) => eprintln!("  {} {} ({})", "⎿".red(), path, e),
                }
            }
        }
    })
    .await;

    (
        uploaded.load(Ordering::Relaxed),
        skipped.load(Ordering::Relaxed),
    )
}

fn hex_to_base64(hex_str: &str) -> String {
    let bytes = hex::decode(hex_str).unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// ============================================================================
// S3Client — signed requests (for DB backend / direct access)
// ============================================================================

pub struct S3Config {
    pub bucket: String,
    pub endpoint: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
    pub prefix: Option<String>,
}

pub struct S3Client {
    client: Client,
    config: S3Config,
}

impl S3Client {
    pub fn new(config: S3Config) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// Build the full key with prefix.
    fn full_key(&self, key: &str) -> String {
        match &self.config.prefix {
            Some(prefix) => format!("{}/{}", prefix, key),
            None => key.to_string(),
        }
    }

    /// Build URL for a key.
    fn url(&self, key: &str) -> String {
        format!(
            "{}/{}/{}",
            self.config.endpoint,
            self.config.bucket,
            self.full_key(key)
        )
    }

    /// Sign a string using AWS v4 signing.
    fn sign(&self, date_stamp: &str, string_to_sign: &str) -> Result<String, String> {
        let k_date = hmac_sha256(
            format!("AWS4{}", self.config.secret_access_key).as_bytes(),
            date_stamp.as_bytes(),
        )?;
        let k_region = hmac_sha256(&k_date, self.config.region.as_bytes())?;
        let k_service = hmac_sha256(&k_region, b"s3")?;
        let k_signing = hmac_sha256(&k_service, b"aws4_request")?;

        let signature = hmac_sha256(&k_signing, string_to_sign.as_bytes())?;

        Ok(hex::encode(signature))
    }
}

impl ObjectStorage for S3Client {
    async fn head(&self, key: &str) -> Result<Option<(Option<String>, bool)>, String> {
        let url = self.url(key);
        let now = Utc::now();
        let date_stamp = now.format("%Y%m%d").to_string();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

        let parsed_url = Url::parse(&url).map_err(|e| e.to_string())?;
        let host = parsed_url.host_str().ok_or("No host in URL")?;
        let path = parsed_url.path();

        let payload_hash = hex::encode(Sha256::digest(b""));

        let canonical_headers = format!(
            "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
            host, payload_hash, amz_date
        );
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";

        let canonical_request = format!(
            "HEAD\n{}\n\n{}\n{}\n{}",
            path, canonical_headers, signed_headers, payload_hash
        );

        let algorithm = "AWS4-HMAC-SHA256";
        let credential_scope = format!("{}/{}/s3/aws4_request", date_stamp, self.config.region);
        let canonical_request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm, amz_date, credential_scope, canonical_request_hash
        );

        let signature = self.sign(&date_stamp, &string_to_sign)?;

        let authorization = format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, self.config.access_key_id, credential_scope, signed_headers, signature
        );

        let response = self
            .client
            .head(&url)
            .header("Host", host)
            .header("x-amz-content-sha256", &payload_hash)
            .header("x-amz-date", &amz_date)
            .header("Authorization", authorization)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let checksum = response
            .headers()
            .get("x-amz-checksum-sha256")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let has_etag = response.headers().get("etag").is_some();

        Ok(Some((checksum, has_etag)))
    }

    async fn put(&self, key: &str, body: Vec<u8>, content_type: &str) -> Result<bool, String> {
        let url = self.url(key);
        let now = Utc::now();
        let date_stamp = now.format("%Y%m%d").to_string();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

        let parsed_url = Url::parse(&url).map_err(|e| e.to_string())?;
        let host = parsed_url.host_str().ok_or("No host in URL")?;
        let path = parsed_url.path();

        let payload_hash = hex::encode(Sha256::digest(&body));
        let checksum_b64 = base64_encode(&Sha256::digest(&body));

        let canonical_headers = format!(
            "content-type:{}\nhost:{}\nx-amz-checksum-sha256:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
            content_type, host, checksum_b64, payload_hash, amz_date
        );
        let signed_headers =
            "content-type;host;x-amz-checksum-sha256;x-amz-content-sha256;x-amz-date";

        let canonical_request = format!(
            "PUT\n{}\n\n{}\n{}\n{}",
            path, canonical_headers, signed_headers, payload_hash
        );

        let algorithm = "AWS4-HMAC-SHA256";
        let credential_scope = format!("{}/{}/s3/aws4_request", date_stamp, self.config.region);
        let canonical_request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm, amz_date, credential_scope, canonical_request_hash
        );

        let signature = self.sign(&date_stamp, &string_to_sign)?;

        let authorization = format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, self.config.access_key_id, credential_scope, signed_headers, signature
        );

        let response = self
            .client
            .put(&url)
            .header("Content-Type", content_type)
            .header("Host", host)
            .header("x-amz-checksum-sha256", &checksum_b64)
            .header("x-amz-content-sha256", &payload_hash)
            .header("x-amz-date", &amz_date)
            .header("Authorization", authorization)
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        Ok(response.status().is_success())
    }
}

// ============================================================================
// PresignedClient — raw HTTP to presigned URLs (for API backend)
// ============================================================================

pub struct PresignedClient {
    client: Client,
    urls: HashMap<String, (String, String)>, // key -> (head_url, put_url)
}

impl PresignedClient {
    pub fn new(urls: HashMap<String, (String, String)>) -> Self {
        Self {
            client: Client::new(),
            urls,
        }
    }
}

impl ObjectStorage for PresignedClient {
    async fn head(&self, key: &str) -> Result<Option<(Option<String>, bool)>, String> {
        let (head_url, _) = self
            .urls
            .get(key)
            .ok_or_else(|| format!("No URL for key '{}'", key))?;

        let response = self
            .client
            .head(head_url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let checksum = response
            .headers()
            .get("x-amz-checksum-sha256")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let has_etag = response.headers().get("etag").is_some();

        Ok(Some((checksum, has_etag)))
    }

    async fn put(&self, key: &str, body: Vec<u8>, content_type: &str) -> Result<bool, String> {
        let (_, put_url) = self
            .urls
            .get(key)
            .ok_or_else(|| format!("No URL for key '{}'", key))?;

        let checksum_b64 = base64_encode(&Sha256::digest(&body));

        let response = self
            .client
            .put(put_url)
            .header("Content-Type", content_type)
            .header("Content-Length", body.len())
            .header("x-amz-checksum-sha256", &checksum_b64)
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        Ok(response.status().is_success())
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn base64_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|e| e.to_string())?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

/// Get MIME type from file extension.
pub fn get_mime_type(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");

    match ext.to_lowercase().as_str() {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "eot" => "application/vnd.ms-fontobject",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

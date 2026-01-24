//! S3-compatible storage client with AWS v4 signing.
//! Handles prefix automatically for all operations.

use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{Client, Url};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

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

    /// Upload a file.
    pub async fn put(&self, key: &str, body: Vec<u8>, content_type: &str) -> Result<bool, String> {
        let url = self.url(key);
        let now = Utc::now();
        let date_stamp = now.format("%Y%m%d").to_string();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

        // Parse URL to get host and path
        let parsed_url = Url::parse(&url).map_err(|e| e.to_string())?;
        let host = parsed_url.host_str().ok_or("No host in URL")?;
        let path = parsed_url.path();

        // Hash the payload
        let payload_hash = hex::encode(Sha256::digest(&body));

        // Create canonical request
        let canonical_headers = format!(
            "content-type:{}\nhost:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
            content_type, host, payload_hash, amz_date
        );
        let signed_headers = "content-type;host;x-amz-content-sha256;x-amz-date";

        let canonical_request = format!(
            "PUT\n{}\n\n{}\n{}\n{}",
            path, canonical_headers, signed_headers, payload_hash
        );

        // Create string to sign
        let algorithm = "AWS4-HMAC-SHA256";
        let credential_scope = format!("{}/{}/s3/aws4_request", date_stamp, self.config.region);
        let canonical_request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm, amz_date, credential_scope, canonical_request_hash
        );

        // Calculate signature
        let signature = self.sign(&date_stamp, &string_to_sign)?;

        // Create authorization header
        let authorization = format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, self.config.access_key_id, credential_scope, signed_headers, signature
        );

        // Make request
        let response = self
            .client
            .put(&url)
            .header("Content-Type", content_type)
            .header("Host", host)
            .header("x-amz-content-sha256", &payload_hash)
            .header("x-amz-date", &amz_date)
            .header("Authorization", authorization)
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        Ok(response.status().is_success())
    }

    /// Sign a string using AWS v4 signing.
    fn sign(&self, date_stamp: &str, string_to_sign: &str) -> Result<String, String> {
        // Step 1: Create signing key
        let k_date = hmac_sha256(
            format!("AWS4{}", self.config.secret_access_key).as_bytes(),
            date_stamp.as_bytes(),
        )?;
        let k_region = hmac_sha256(&k_date, self.config.region.as_bytes())?;
        let k_service = hmac_sha256(&k_region, b"s3")?;
        let k_signing = hmac_sha256(&k_service, b"aws4_request")?;

        // Step 2: Sign the string
        let signature = hmac_sha256(&k_signing, string_to_sign.as_bytes())?;

        Ok(hex::encode(signature))
    }
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

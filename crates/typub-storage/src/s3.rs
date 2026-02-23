//! S3-compatible storage client.
//!
//! Per [[RFC-0004:C-UPLOAD-TRACKING]].

use anyhow::{Context, Result};
use s3::creds::Credentials;
use s3::{Bucket, Region};
use sha2::{Digest, Sha256};
use std::path::Path;
use typub_config::StorageConfig;

use crate::mime_type_from_path;

/// S3-compatible storage client
/// Per [[RFC-0004:C-UPLOAD-TRACKING]]
pub struct S3Storage {
    bucket: Box<Bucket>,
    url_prefix: String,
    config_id: String,
}

/// Result of an asset upload
#[derive(Debug, Clone)]
pub struct UploadResult {
    /// Remote object key
    pub remote_key: String,
    /// Public URL of the uploaded asset
    pub remote_url: String,
    /// Content hash (SHA-256, lowercase hex, 64 chars)
    pub content_hash: String,
    /// Normalized extension (lowercase alphanumeric only)
    pub extension: String,
}

impl S3Storage {
    /// Create a new S3Storage client from config
    pub fn new(config: &StorageConfig) -> Result<Self> {
        config.validate()?;

        let bucket_name = config
            .bucket
            .as_ref()
            .context("bucket is required")?
            .clone();

        let region = match (&config.endpoint, &config.region) {
            (Some(endpoint), Some(region)) => Region::Custom {
                region: region.clone(),
                endpoint: endpoint.clone(),
            },
            (Some(endpoint), None) => Region::Custom {
                region: "auto".to_string(), // R2 uses "auto"
                endpoint: endpoint.clone(),
            },
            (None, Some(region)) => region.parse().unwrap_or_else(|_| Region::Custom {
                region: region.clone(),
                endpoint: format!("https://s3.{}.amazonaws.com", region),
            }),
            (None, None) => Region::UsEast1,
        };

        // Resolve credentials with fallback to environment variables
        let access_key = config
            .access_key_id
            .clone()
            .or_else(|| std::env::var("S3_ACCESS_KEY_ID").ok())
            .or_else(|| std::env::var("AWS_ACCESS_KEY_ID").ok());

        let secret_key = config
            .secret_access_key
            .clone()
            .or_else(|| std::env::var("S3_SECRET_ACCESS_KEY").ok())
            .or_else(|| std::env::var("AWS_SECRET_ACCESS_KEY").ok());

        let credentials = Credentials::new(
            access_key.as_deref(),
            secret_key.as_deref(),
            None, // security_token
            None, // session_token
            None, // profile
        )
        .context("Failed to create S3 credentials")?;

        let bucket = Bucket::new(&bucket_name, region, credentials)
            .context("Failed to create S3 bucket client")?
            .with_path_style(); // Required for some S3-compatible services

        let url_prefix = config
            .normalized_url_prefix()
            .context("url_prefix is required")?;

        let config_id = config.config_id();

        Ok(Self {
            bucket,
            url_prefix,
            config_id,
        })
    }

    /// Get the storage configuration identifier
    pub fn config_id(&self) -> &str {
        &self.config_id
    }

    /// Get the URL prefix
    pub fn url_prefix(&self) -> &str {
        &self.url_prefix
    }

    /// Upload an asset to S3
    /// Per [[RFC-0004:C-UPLOAD-TRACKING]] and [[RFC-0004:C-URL-CONSTRUCTION]]
    pub async fn upload(&self, local_path: &Path, data: &[u8]) -> Result<UploadResult> {
        let content_hash = Self::compute_hash(data);
        let extension = Self::normalize_extension(local_path);
        let remote_key = Self::build_object_key(&content_hash, &extension);
        let mime_type = mime_type_from_path(local_path);

        // Upload to S3
        let response = self
            .bucket
            .put_object_with_content_type(&remote_key, data, mime_type)
            .await;

        match response {
            Ok(_) => {}
            Err(e) => {
                // Check if it's an AlreadyExists-like error (treat as success per RFC-0004)
                let err_str = e.to_string();
                if !err_str.contains("PreconditionFailed")
                    && !err_str.contains("AlreadyExists")
                    && !err_str.contains("409")
                {
                    return Err(e).context(format!(
                        "Failed to upload asset '{}' to S3",
                        local_path.display()
                    ));
                }
                // AlreadyExists is success for content-addressable keys
            }
        }

        let remote_url = Self::build_url(&self.url_prefix, &remote_key);

        Ok(UploadResult {
            remote_key,
            remote_url,
            content_hash,
            extension,
        })
    }

    /// Check if an object exists in S3
    pub async fn exists(&self, key: &str) -> Result<bool> {
        match self.bucket.head_object(key).await {
            Ok(_) => Ok(true),
            Err(s3::error::S3Error::HttpFailWithBody(404, _)) => Ok(false),
            Err(e) => Err(e).context(format!("Failed to check if object '{}' exists", key)),
        }
    }

    /// Compute SHA-256 hash of data (lowercase hex, 64 chars)
    /// Per [[RFC-0004:C-UPLOAD-TRACKING]]
    pub fn compute_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Normalize file extension per [[RFC-0004:C-UPLOAD-TRACKING]]
    /// 1. Extract extension from filename
    /// 2. Convert to lowercase
    /// 3. Remove non-alphanumeric characters
    /// 4. Return empty string if result is empty
    pub fn normalize_extension(path: &Path) -> String {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                e.to_lowercase()
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect::<String>()
            })
            .unwrap_or_default()
    }

    /// Build object key per [[RFC-0004:C-UPLOAD-TRACKING]]
    /// Format: {content_hash}.{extension} or {content_hash} if no extension
    pub fn build_object_key(content_hash: &str, extension: &str) -> String {
        if extension.is_empty() {
            content_hash.to_string()
        } else {
            format!("{}.{}", content_hash, extension)
        }
    }

    /// Build public URL per [[RFC-0004:C-URL-CONSTRUCTION]]
    /// Format: {url_prefix}/{object_key}
    pub fn build_url(url_prefix: &str, object_key: &str) -> String {
        format!("{}/{}", url_prefix, object_key)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn test_compute_hash() {
        // SHA-256 of "hello"
        let hash = S3Storage::compute_hash(b"hello");
        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_normalize_extension() {
        assert_eq!(
            S3Storage::normalize_extension(Path::new("image.PNG")),
            "png"
        );
        assert_eq!(
            S3Storage::normalize_extension(Path::new("photo.JPEG")),
            "jpeg"
        );
        assert_eq!(S3Storage::normalize_extension(Path::new("noext")), "");
    }

    #[test]
    fn test_build_object_key() {
        let hash = "abc123";
        assert_eq!(S3Storage::build_object_key(hash, "png"), "abc123.png");
        assert_eq!(S3Storage::build_object_key(hash, ""), "abc123");
    }

    #[test]
    fn test_build_url() {
        assert_eq!(
            S3Storage::build_url("https://cdn.example.com", "abc123.png"),
            "https://cdn.example.com/abc123.png"
        );
    }
}

//! Ghost Admin API client.
//!
//! Uses JWT authentication per Ghost Admin API documentation.
//! The API key format is `id:secret` where id and secret are hex strings.
use typub_adapters_core::debug;

use crate::types::{
    GhostImageUploadResponse, GhostPost, GhostPostData, GhostPostRequest, GhostPostResponse,
    GhostPostsListResponse, GhostTagInput,
};
use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use jwt::SignWithKey;
use reqwest::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use reqwest::multipart::{Form, Part};
use sha2::Sha256;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use typub_adapters_core::http_utils;
use typub_storage::mime_type_from_path;

/// Ghost Admin API client.
pub struct GhostClient<'a> {
    pub client: &'a Client,
    pub base_url: &'a str,
    pub api_key: Option<&'a str>,
    pub published: bool,
}

impl<'a> GhostClient<'a> {
    pub fn new(
        client: &'a Client,
        base_url: &'a str,
        api_key: Option<&'a str>,
        published: bool,
    ) -> Self {
        Self {
            client,
            base_url,
            api_key,
            published,
        }
    }

    pub fn auth_key(&self) -> Result<&str> {
        self.api_key
            .ok_or_else(|| anyhow::anyhow!("ghost.api_key or GHOST_ADMIN_API_KEY is required"))
    }

    /// Parse the API key into (id, secret) tuple.
    /// Ghost Admin API keys are in format `id:secret` where both are hex strings.
    fn parse_api_key(&self) -> Result<(&str, Vec<u8>)> {
        let key = self.auth_key()?;
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            anyhow::bail!("Ghost API key must be in format 'id:secret'");
        }
        let id = parts[0];
        let secret =
            hex::decode(parts[1]).context("Ghost API secret must be a valid hex string")?;
        Ok((id, secret))
    }

    /// Generate a JWT token for Ghost Admin API authentication.
    fn generate_jwt(&self) -> Result<String> {
        let (id, secret) = self.parse_api_key()?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("System time before UNIX epoch")?
            .as_secs();

        let mut claims: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
        claims.insert("iat", serde_json::json!(now));
        claims.insert("exp", serde_json::json!(now + 300)); // 5 minutes
        claims.insert("aud", serde_json::json!("/admin/"));

        // Create HMAC-SHA256 key
        let key: Hmac<Sha256> =
            Hmac::new_from_slice(&secret).context("Failed to create HMAC key")?;

        // Build header with kid
        let header = jwt::Header {
            algorithm: jwt::AlgorithmType::Hs256,
            key_id: Some(id.to_string()),
            ..Default::default()
        };

        // Sign the token
        let token = jwt::Token::new(header, claims)
            .sign_with_key(&key)
            .context("Failed to sign JWT")?;

        Ok(token.as_str().to_string())
    }

    fn api_url(&self, path: &str) -> String {
        format!(
            "{}/ghost/api/admin/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    pub fn auth_headers(&self) -> Result<HeaderMap> {
        let token = self.generate_jwt()?;
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Ghost {}", token))
                .context("Invalid Ghost authorization header")?,
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_static("typub/0.1"));
        Ok(headers)
    }

    /// Auth headers for multipart uploads (no Content-Type — reqwest sets the boundary).
    fn auth_headers_multipart(&self) -> Result<HeaderMap> {
        let token = self.generate_jwt()?;
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Ghost {}", token))
                .context("Invalid Ghost authorization header")?,
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(USER_AGENT, HeaderValue::from_static("typub/0.1"));
        Ok(headers)
    }

    fn status_string(&self) -> &'static str {
        if self.published { "published" } else { "draft" }
    }

    pub async fn create_post(
        &self,
        title: &str,
        lexical: &str,
        tags: &[String],
    ) -> Result<GhostPost> {
        let request = GhostPostRequest {
            posts: vec![GhostPostData {
                title: title.to_string(),
                lexical: lexical.to_string(),
                status: self.status_string().to_string(),
                tags: tags
                    .iter()
                    .map(|t| GhostTagInput { name: t.clone() })
                    .collect(),
                updated_at: None,
            }],
        };

        let response = self
            .client
            .post(self.api_url("posts/"))
            .headers(self.auth_headers()?)
            .json(&request)
            .send()
            .await
            .context("Failed to create Ghost post request")?;

        let response = http_utils::ensure_success(response, "create Ghost post").await?;
        let parsed: GhostPostResponse = response
            .json()
            .await
            .context("Failed to parse create Ghost post response")?;

        parsed
            .posts
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Ghost API returned empty posts array"))
    }

    pub async fn update_post(
        &self,
        post_id: &str,
        title: &str,
        lexical: &str,
        tags: &[String],
        updated_at: &str,
    ) -> Result<Option<GhostPost>> {
        let request = GhostPostRequest {
            posts: vec![GhostPostData {
                title: title.to_string(),
                lexical: lexical.to_string(),
                status: self.status_string().to_string(),
                tags: tags
                    .iter()
                    .map(|t| GhostTagInput { name: t.clone() })
                    .collect(),
                updated_at: Some(updated_at.to_string()),
            }],
        };

        let response = self
            .client
            .put(self.api_url(&format!("posts/{}/", post_id)))
            .headers(self.auth_headers()?)
            .json(&request)
            .send()
            .await
            .context("Failed to update Ghost post request")?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response = http_utils::ensure_success(response, "update Ghost post").await?;
        let parsed: GhostPostResponse = response
            .json()
            .await
            .context("Failed to parse update Ghost post response")?;

        Ok(parsed.posts.into_iter().next())
    }

    /// Get a post by ID to retrieve current updated_at for conflict prevention.
    pub async fn get_post(&self, post_id: &str) -> Result<Option<GhostPost>> {
        let response = self
            .client
            .get(self.api_url(&format!("posts/{}/", post_id)))
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("Failed to get Ghost post")?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response = http_utils::ensure_success(response, "get Ghost post").await?;
        let parsed: GhostPostResponse = response
            .json()
            .await
            .context("Failed to parse get Ghost post response")?;

        Ok(parsed.posts.into_iter().next())
    }

    /// Find a post by title (deterministic lookup per RFC-0003).
    /// Paginates through all posts to find exact title match.
    pub async fn find_post_by_title(&self, title: &str) -> Result<Option<GhostPost>> {
        let mut page = 1u32;
        loop {
            let url = format!(
                "{}?limit=100&page={}&filter=status:[draft,published,scheduled]",
                self.api_url("posts/"),
                page
            );
            let response = self
                .client
                .get(&url)
                .headers(self.auth_headers()?)
                .send()
                .await
                .context("Failed to list Ghost posts for title lookup")?;

            let response = http_utils::ensure_success(response, "Ghost list posts").await?;
            let parsed: GhostPostsListResponse = response
                .json()
                .await
                .context("Failed to parse Ghost posts list")?;

            if let Some(hit) = parsed.posts.iter().find(|p| p.title == title) {
                return Ok(Some(hit.clone()));
            }

            // Check pagination
            if let Some(meta) = &parsed.meta {
                if page >= meta.pagination.pages {
                    return Ok(None);
                }
            } else if parsed.posts.is_empty() {
                return Ok(None);
            }
            page += 1;
        }
    }

    /// Upload an image via Ghost's `/images/upload` endpoint.
    /// Returns the hosted URL of the uploaded image.
    pub async fn upload_image(&self, file_path: &Path) -> Result<String> {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid image filename: {}", file_path.display()))?;

        let file_data = std::fs::read(file_path)
            .with_context(|| format!("Failed to read image: {}", file_path.display()))?;

        let mime_type = mime_type_from_path(file_path);

        let part = Part::bytes(file_data)
            .file_name(file_name.to_string())
            .mime_str(mime_type)
            .context("Invalid MIME type for image upload")?;

        let form = Form::new().part("file", part);

        let response = self
            .client
            .post(self.api_url("images/upload/"))
            .headers(self.auth_headers_multipart()?)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("Failed to upload image: {}", file_path.display()))?;

        let response = http_utils::ensure_success(response, "Ghost image upload").await?;
        let parsed: GhostImageUploadResponse = response
            .json()
            .await
            .context("Failed to parse Ghost image upload response")?;

        parsed
            .images
            .into_iter()
            .next()
            .map(|img| img.url)
            .ok_or_else(|| anyhow::anyhow!("Ghost image upload returned empty images array"))
    }

    /// Title lookup then update, or create as last resort.
    /// Shared logic for RFC-0003 steps 2–3.
    pub async fn update_or_create_by_title(
        &self,
        title: &str,
        lexical: &str,
        tags: &[String],
    ) -> Result<GhostPost> {
        if let Some(found) = self.find_post_by_title(title).await? {
            debug!("Found existing Ghost post by title: id={}", found.id);
            let updated_at = found
                .updated_at
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Ghost post missing updated_at field"))?;

            self.update_post(&found.id, title, lexical, tags, updated_at)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("Title-matched Ghost post {} also returned 404", found.id)
                })
        } else {
            self.create_post(title, lexical, tags).await
        }
    }
}

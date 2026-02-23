//! WordPress REST API client - HTTP methods for media, posts, and terms.

use super::types::{WpMediaResponse, WpTermResponse};
use anyhow::{Context, Result};
use reqwest::Client;
use reqwest::header::{CONTENT_DISPOSITION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::json;
use std::path::Path;
use typub_adapters_core::http_utils;
use typub_storage::mime_type_from_path;

pub(super) const WP_AUTH_HINT: &str = "Check wordpress.api_key or WORDPRESS_API_KEY env var.";

/// WordPress API client methods.
///
/// These are implemented as methods on WordPressAdapter but grouped here
/// for clarity. The actual implementation is in the adapter module.
pub(super) struct WordPressClient<'a> {
    pub client: &'a Client,
    pub base_url: &'a str,
    pub api_key: Option<&'a str>,
}

impl<'a> WordPressClient<'a> {
    pub fn new(client: &'a Client, base_url: &'a str, api_key: Option<&'a str>) -> Self {
        Self {
            client,
            base_url,
            api_key,
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}/wp-json/wp/v2/{}", self.base_url, path)
    }

    fn auth_token(&self) -> Result<&str> {
        self.api_key
            .ok_or_else(|| anyhow::anyhow!("WORDPRESS_API_KEY not configured"))
    }

    pub fn parse_post_id(v: &serde_json::Value) -> Result<String> {
        if let Some(id) = v["id"].as_u64() {
            return Ok(id.to_string());
        }
        if let Some(id) = v["id"].as_str() {
            return Ok(id.to_string());
        }
        anyhow::bail!("No post ID in WordPress response")
    }

    pub fn parse_post_url(v: &serde_json::Value) -> Result<String> {
        if let Some(url) = v["link"].as_str() {
            return Ok(url.to_string());
        }
        anyhow::bail!("No post URL in WordPress response")
    }

    pub async fn find_post_by_slug(&self, slug: &str) -> Result<Option<(String, String)>> {
        let token = self.auth_token()?;
        let url = format!(
            "{}?slug={}&context=edit&status=any&per_page=1",
            self.api_url("posts"),
            urlencoding::encode(slug)
        );
        let response = self
            .client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to search WordPress post by slug")?;
        let response =
            http_utils::ensure_success_with_auth_hint(response, "WordPress search", WP_AUTH_HINT)
                .await?;

        let posts: Vec<serde_json::Value> = response
            .json()
            .await
            .context("Failed to parse WordPress search response")?;

        if let Some(first) = posts.first() {
            let id = Self::parse_post_id(first)?;
            let url = Self::parse_post_url(first)?;
            Ok(Some((id, url)))
        } else {
            Ok(None)
        }
    }

    pub async fn find_post_by_id(&self, post_id: &str) -> Result<Option<(String, String)>> {
        let token = self.auth_token()?;
        let url = format!(
            "{}?context=edit",
            self.api_url(&format!("posts/{}", post_id))
        );
        let response = self
            .client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to query WordPress post by id")?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = http_utils::ensure_success_with_auth_hint(
            response,
            "WordPress query-by-id",
            WP_AUTH_HINT,
        )
        .await?;

        let v: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse WordPress query-by-id response")?;
        Ok(Some((Self::parse_post_id(&v)?, Self::parse_post_url(&v)?)))
    }

    /// Create or update a WordPress post. When `post_id` is `Some`, the
    /// request targets `posts/{id}` (update); otherwise `posts` (create).
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_post(
        &self,
        post_id: Option<&str>,
        title: &str,
        slug: &str,
        content_html: &str,
        tag_ids: &[u64],
        category_ids: &[u64],
        status: &str,
    ) -> Result<(String, String)> {
        let token = self.auth_token()?;
        let mut payload = serde_json::Map::new();
        payload.insert("title".to_string(), json!(title));
        payload.insert("slug".to_string(), json!(slug));
        payload.insert("content".to_string(), json!(content_html));
        payload.insert("status".to_string(), json!(status));
        payload.insert("tags".to_string(), json!(tag_ids));
        if !category_ids.is_empty() {
            payload.insert("categories".to_string(), json!(category_ids));
        }

        let endpoint = match post_id {
            Some(id) => format!("posts/{id}"),
            None => "posts".to_string(),
        };
        let operation = if post_id.is_some() {
            "WordPress update post"
        } else {
            "WordPress create post"
        };

        let response = self
            .client
            .post(self.api_url(&endpoint))
            .bearer_auth(token)
            .json(&serde_json::Value::Object(payload))
            .send()
            .await
            .with_context(|| format!("Failed to execute {operation}"))?;
        let response =
            http_utils::ensure_success_with_auth_hint(response, operation, WP_AUTH_HINT).await?;

        let v: serde_json::Value = response
            .json()
            .await
            .with_context(|| format!("Failed to parse {operation} response"))?;
        Ok((Self::parse_post_id(&v)?, Self::parse_post_url(&v)?))
    }

    pub async fn upload_media(&self, file_path: &Path) -> Result<String> {
        let token = self.auth_token()?;
        let data = std::fs::read(file_path)
            .with_context(|| format!("Failed to read asset: {}", file_path.display()))?;
        let filename = file_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid asset filename: {}", file_path.display()))?;
        let mime = mime_type_from_path(file_path);

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
                .context("Invalid filename for Content-Disposition")?,
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(mime).context("Invalid MIME type")?,
        );

        let response = self
            .client
            .post(self.api_url("media"))
            .bearer_auth(token)
            .headers(headers)
            .body(data)
            .send()
            .await
            .with_context(|| format!("Failed to upload media: {}", file_path.display()))?;
        let response = http_utils::ensure_success_with_auth_hint(
            response,
            &format!("WordPress media upload ({})", file_path.display()),
            WP_AUTH_HINT,
        )
        .await?;

        let v: WpMediaResponse = response
            .json()
            .await
            .context("Failed to parse WordPress media upload response")?;
        Ok(v.source_url)
    }

    pub async fn find_term_id_by_name(&self, endpoint: &str, name: &str) -> Result<Option<u64>> {
        let token = self.auth_token()?;
        let url = format!(
            "{}?search={}&per_page=100&context=edit",
            self.api_url(endpoint),
            urlencoding::encode(name)
        );
        let response = self
            .client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to query WordPress tags")?;
        let response = http_utils::ensure_success_with_auth_hint(
            response,
            &format!("WordPress {endpoint} query"),
            WP_AUTH_HINT,
        )
        .await?;

        let terms: Vec<WpTermResponse> = response
            .json()
            .await
            .with_context(|| format!("Failed to parse WordPress {} response", endpoint))?;

        let normalized_slug = name.trim().to_lowercase().replace(' ', "-");
        Ok(terms
            .into_iter()
            .find(|t| {
                t.name.trim().eq_ignore_ascii_case(name)
                    || t.slug.trim().eq_ignore_ascii_case(&normalized_slug)
            })
            .map(|t| t.id))
    }

    pub async fn create_term(&self, endpoint: &str, name: &str) -> Result<u64> {
        let token = self.auth_token()?;
        let response = self
            .client
            .post(self.api_url(endpoint))
            .bearer_auth(token)
            .json(&json!({ "name": name }))
            .send()
            .await
            .with_context(|| format!("Failed to create WordPress {}", endpoint))?;
        let response = http_utils::ensure_success_with_auth_hint(
            response,
            &format!("WordPress create {endpoint}"),
            WP_AUTH_HINT,
        )
        .await?;

        let term: WpTermResponse = response
            .json()
            .await
            .with_context(|| format!("Failed to parse WordPress create {} response", endpoint))?;
        Ok(term.id)
    }

    pub async fn resolve_term_ids(&self, endpoint: &str, terms: &[String]) -> Result<Vec<u64>> {
        let normalized = normalize_terms(terms);

        let mut ids = Vec::new();
        for tag_name in normalized {
            if let Some(id) = self.find_term_id_by_name(endpoint, &tag_name).await? {
                ids.push(id);
            } else {
                let id = self.create_term(endpoint, &tag_name).await?;
                ids.push(id);
            }
        }
        Ok(ids)
    }

    pub async fn resolve_tag_ids(&self, tags: &[String]) -> Result<Vec<u64>> {
        self.resolve_term_ids("tags", tags).await
    }

    pub async fn resolve_category_ids(&self, categories: &[String]) -> Result<Vec<u64>> {
        self.resolve_term_ids("categories", categories).await
    }
}

fn normalize_terms(terms: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = terms
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    normalized.sort_by_key(|s| s.to_lowercase());
    normalized.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    normalized
}

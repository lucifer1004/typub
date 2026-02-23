//! Dev.to REST API client.
use typub_adapters_core::debug;

use crate::model::{DevtoArticleListItem, DevtoArticleResponse};
use anyhow::{Context, Result};
use reqwest::Client;
use reqwest::header::{ACCEPT, HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use serde_json::json;
use typub_adapters_core::http_utils;

/// Dev.to REST API client.
pub(super) struct DevtoClient<'a> {
    pub client: &'a Client,
    pub base_url: &'a str,
    pub api_key: Option<&'a str>,
    pub published: bool,
}

impl<'a> DevtoClient<'a> {
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
            .ok_or_else(|| anyhow::anyhow!("devto.api_key or DEVTO_API_KEY is required"))
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    pub fn auth_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("api-key"),
            HeaderValue::from_str(self.auth_key()?)
                .context("Invalid Dev.to API key header value")?,
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.forem.api-v1+json"),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("typub/0.1"));
        Ok(headers)
    }

    pub async fn create_article(
        &self,
        title: &str,
        body_markdown: &str,
        tags: &[String],
    ) -> Result<DevtoArticleResponse> {
        let payload = json!({
            "article": {
                "title": title,
                "body_markdown": body_markdown,
                "published": self.published,
                "tags": tags.join(",")
            }
        });
        let response = self
            .client
            .post(self.api_url("articles"))
            .headers(self.auth_headers()?)
            .json(&payload)
            .send()
            .await
            .context("Failed to create Dev.to article request")?;
        let response = http_utils::ensure_success(response, "create Dev.to article").await?;
        response
            .json::<DevtoArticleResponse>()
            .await
            .context("Failed to parse create Dev.to article response")
    }

    pub async fn update_article(
        &self,
        article_id: &str,
        title: &str,
        body_markdown: &str,
        tags: &[String],
    ) -> Result<Option<DevtoArticleResponse>> {
        let payload = json!({
            "article": {
                "title": title,
                "body_markdown": body_markdown,
                "published": self.published,
                "tags": tags.join(",")
            }
        });
        let response = self
            .client
            .put(self.api_url(&format!("articles/{article_id}")))
            .headers(self.auth_headers()?)
            .json(&payload)
            .send()
            .await
            .context("Failed to update Dev.to article request")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = http_utils::ensure_success(response, "update Dev.to article").await?;
        let article = response
            .json::<DevtoArticleResponse>()
            .await
            .context("Failed to parse update Dev.to article response")?;
        Ok(Some(article))
    }

    /// Deterministic title-based lookup per [[RFC-0003:C-DECISION-KEY]] step 2.
    /// Paginates through the authenticated user's articles and returns the
    /// first exact title match, if any.
    pub async fn find_article_by_title(&self, title: &str) -> Result<Option<DevtoArticleResponse>> {
        let mut page = 1u32;
        loop {
            let url = format!(
                "{}?per_page=100&page={}",
                self.api_url("articles/me/all"),
                page
            );
            let response = self
                .client
                .get(&url)
                .headers(self.auth_headers()?)
                .send()
                .await
                .context("Failed to list Dev.to articles for title lookup")?;
            let response = http_utils::ensure_success(response, "Dev.to list articles").await?;
            let articles: Vec<DevtoArticleListItem> = response
                .json()
                .await
                .context("Failed to parse Dev.to article list")?;

            if articles.is_empty() {
                return Ok(None);
            }
            if let Some(hit) = articles.iter().find(|a| a.title == title) {
                return Ok(Some(DevtoArticleResponse {
                    id: hit.id,
                    url: hit.url.clone(),
                }));
            }
            page += 1;
        }
    }

    /// Title lookup then update, or create as last resort.
    ///
    /// Shared logic for both the "stale cached ID" and "no cached ID" branches
    /// of [[RFC-0003:C-DECISION-KEY]] steps 2–3.
    pub async fn update_or_create_by_title(
        &self,
        title: &str,
        body_markdown: &str,
        tags: &[String],
    ) -> Result<DevtoArticleResponse> {
        if let Some(found) = self.find_article_by_title(title).await? {
            debug!("Found existing Dev.to article by title: id={}", found.id);
            self.update_article(&found.id.to_string(), title, body_markdown, tags)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Title-matched Dev.to article {} also returned 404",
                        found.id
                    )
                })
        } else {
            self.create_article(title, body_markdown, tags).await
        }
    }
}
